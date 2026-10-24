// A Map surface whose picture doesn't visibly repeat (#304): the standard PBR fragment, with its
// colour and normal map taken from a picture that is laid out in hex tiles instead of in one grid,
// and its colour and roughness nudged by a large picture that lies over the world.
//
// Roughness, occlusion, metal and wetness (#320): a picture of the three (red occlusion, green
// roughness, blue metal) is looked up through the same tiles as the colour, and a surface can be
// wet. The wet numbers are the ones `surface_look::wet` makes: keep the two in step.
//
// Two materials on one mesh (#321): a second material (B) is looked up the same way as the
// surface's own (A), on its own tiles, and shows through A as much as the mesh's vertex colour
// says (its alpha, 0 to 1), with the edge between them broken up by a noise and by the two
// pictures' own light and dark (a height blend: B's stones show first through A's gaps).
//
// Decals (#321): a decal is drawn by this shader too, with its one picture (no tiles), cut out by
// its mask, blended over the surface under it (premultiplied alpha). Its own colour shows as much
// as `cover` says; where it is wet, the surface under it is darkened and given the sheen of a
// film of water, so a damp patch with no colour of its own shows the floor's picture, wet.
//
// Hex tiling: the surface's picture coordinates fall on a triangular grid. Every corner of the
// grid has a random look (a shift of the picture, and for some materials a turn and a mirror),
// and a pixel blends the three pictures of the three corners round its triangle. Each corner's
// look comes from its whole-number position alone, so it is the same in every frame and every
// pixel. The blend uses the weights to a high power, so most of a triangle is one clean copy of
// the picture and the blends are narrow, and it puts back the contrast that blending takes out.
//
// The numbers a corner's look is made from (`cell_look`) are the same as `cell_look` in
// `repeat_breaking.rs`, which the readable checks use: keep the two in step.
//
// Reimplemented from the idea of hex-grid tiling (Mikkelsen, "Practical Real-Time Hex-Tiling",
// JCGT 2022); nothing here is copied from his code.
import bevy_pbr::render::{
    pbr_types,
    pbr_lighting,
    mesh_view_bindings,
    pbr_functions::{
        alpha_discard,
        apply_normal_mapping,
        apply_pbr_lighting,
        calculate_tbn_mikktspace,
        main_pass_post_lighting_processing,
    },
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
};

struct Breaker {
    // x, y: the shifts are whole multiples of 1/x and 1/y of the picture (0 for any shift);
    // z: 1 if corners also turn and mirror the picture; w: 1 to break the repeat at all.
    snap_turn: vec4<f32>,
    // x: how strongly the large picture shows (0 for none); y: its repeats to a metre.
    wide: vec4<f32>,
    // x: 1 if the surface picture is bound; y: how wet the surface is, 0 to 1; z: 1 if the picture
    // holds roughness alone, in its one (red) channel.
    surface: vec4<f32>,
    // What the surface reflects of a baked Map's sky: the zenith's colour and the sky-lit
    // ground's brightness; the horizon's colour and 1 if there is a sky; the ground's colour.
    sky_zenith: vec4<f32>,
    sky_haze: vec4<f32>,
    sky_ground: vec4<f32>,
    // The second material, if there is one (#321): its tiles as `snap_turn` says them; x: 1 if
    // there is one, y: its repeats to one of A's, z: its roughness number, w: its wetness; x: 1 if
    // its surface picture is bound, y: 1 if that holds roughness alone, z: its metal number.
    blend_tiles: vec4<f32>,
    blend: vec4<f32>,
    blend_surface: vec4<f32>,
    // x: 1 if this is a decal; y: how much of its colour shows where its mask is whole.
    decal: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> breaker: Breaker;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var diffuse_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var diffuse_sampler: sampler;  // the material's own pictures and the second material's share it (#339)
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var wide_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var wide_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var surface_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(109) var blend_diffuse_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111) var blend_normal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(113) var blend_surface_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(115) var mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(116) var mask_sampler: sampler;

// Triangle corners to one repeat of the picture: 2 * sqrt(3), so a hexagon is about 0.58 of a repeat across.
const CELLS_PER_REPEAT: f32 = 3.4641016;
// How narrow the blends are: the weights go to this power.
const SHARPNESS: f32 = 7.0;

// What being wet does (`surface_look.rs`): a fully wet surface keeps this share of its colour, has
// roughness GLOSS_FLOOR + GLOSS_SCALE * its dry roughness, reflects WET_REFLECTANCE instead of
// 0.5, and keeps this share of its normal map's slope.
const WET_KEEPS: f32 = 0.6;
const WET_GLOSS_FLOOR: f32 = 0.05;
const WET_GLOSS_SCALE: f32 = 0.30;
const DRY_REFLECTANCE: f32 = 0.5;
const WET_REFLECTANCE: f32 = 0.65;
const WET_KEEPS_SLOPE: f32 = 0.6;

// The edge between two blended materials (`surface_look.rs` has the same numbers): how far the
// noise pushes the vertex weight in the middle of the edge, its two sizes in metres, how much
// the pictures' light and dark (height) count, and how wide the blend between the two is.
const EDGE_WANDER: f32 = 0.5;
const EDGE_NOISE_COARSE: f32 = 0.55;
const EDGE_NOISE_FINE: f32 = 0.16;
const HEIGHT_GAIN: f32 = 3.0;
const BLEND_DEPTH: f32 = 0.2;

// A decal's mask: below this it is nothing (the noise of the JPG); a film of water's roughness, and
// how much of the sky it shows: a damp surface mostly darkens.
const MASK_FLOOR: f32 = 0.03;
const FILM_ROUGHNESS: f32 = 0.2;
const FILM_SHEEN: f32 = 0.35;

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Four random numbers in [0, 1) for a corner of the grid.
fn cell_look(cell: vec2<i32>) -> vec4<f32> {
    let h0 = pcg(bitcast<u32>(cell.x) ^ pcg(bitcast<u32>(cell.y)));
    let h1 = pcg(h0 + 1u);
    let h2 = pcg(h0 + 2u);
    let h3 = pcg(h0 + 3u);
    return vec4<f32>(
        f32(h0 >> 8u), f32(h1 >> 8u), f32(h2 >> 8u), f32(h3 >> 8u),
    ) * (1.0 / 16777216.0);
}

struct Corners {
    weights: vec3<f32>,
    a: vec2<i32>,
    b: vec2<i32>,
    c: vec2<i32>,
}

// The three corners round the triangle that `uv` is in, and how near it is to each.
fn corners_of(uv: vec2<f32>) -> Corners {
    let st = uv * CELLS_PER_REPEAT;
    let skew = vec2<f32>(st.x - 0.57735027 * st.y, 1.15470054 * st.y);
    let base = floor(skew);
    let t = skew - base;
    let at = vec2<i32>(base);
    let z = 1.0 - t.x - t.y;
    var out: Corners;
    if z > 0.0 {
        out.weights = vec3<f32>(z, t.y, t.x);
        out.a = at;
        out.b = at + vec2<i32>(0, 1);
        out.c = at + vec2<i32>(1, 0);
    } else {
        out.weights = vec3<f32>(-z, 1.0 - t.y, 1.0 - t.x);
        out.a = at + vec2<i32>(1, 1);
        out.b = at + vec2<i32>(1, 0);
        out.c = at + vec2<i32>(0, 1);
    }
    return out;
}

// One corner's picture at this pixel: where to look in the picture, and the turn that took it there.
struct Look {
    uv: vec2<f32>,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
    turn: mat2x2<f32>,
}

fn look_at(corner: vec2<i32>, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, tiles: vec4<f32>) -> Look {
    let r = cell_look(corner);
    let centre = vec2<f32>(f32(corner.x) + 0.5 * f32(corner.y), 0.8660254 * f32(corner.y)) / CELLS_PER_REPEAT;
    let snap = tiles.xy;
    let shift = vec2<f32>(
        select(r.x, floor(r.x * snap.x) / snap.x, snap.x > 0.0),
        select(r.y, floor(r.y * snap.y) / snap.y, snap.y > 0.0),
    );
    var turn = mat2x2<f32>(1.0, 0.0, 0.0, 1.0);
    if tiles.z > 0.0 {
        let angle = r.z * 6.2831853;
        let flip = select(1.0, -1.0, r.w > 0.5);
        let rotation = mat2x2<f32>(cos(angle), sin(angle), -sin(angle), cos(angle));
        turn = rotation * mat2x2<f32>(flip, 0.0, 0.0, 1.0);
    }
    var out: Look;
    out.uv = turn * (uv - centre) + centre + shift;
    out.ddx = turn * ddx;
    out.ddy = turn * ddy;
    out.turn = turn;
    return out;
}

// One material's pictures, and what the shader is told about them: its tiles (`snap_turn`), and
// its surface picture's flags (x: bound, z: roughness alone).
struct Pictures {
    tiles: vec4<f32>,
    surface: vec4<f32>,
}

// A surface picture's occlusion, roughness and metal where `uv` is.
fn surface_sample(t: texture_2d<f32>, s: sampler, roughness_only: bool, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec3<f32> {
    let texel = textureSampleGrad(t, s, uv, ddx, ddy).rgb;
    return select(texel, vec3<f32>(1.0, texel.r, 1.0), roughness_only);
}

struct Surface {
    colour: vec3<f32>,
    // The normal in the surface's own frame, as the normal map holds it, from -1 to 1.
    normal: vec3<f32>,
    // Occlusion, roughness and metal, from the surface picture (1, 1, 1 when there is none).
    pbr: vec3<f32>,
}

fn surface_at(
    diffuse_t: texture_2d<f32>, diffuse_s: sampler,
    normal_t: texture_2d<f32>, normal_s: sampler,
    surface_t: texture_2d<f32>, surface_s: sampler,
    pictures: Pictures,
    uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>,
) -> Surface {
    var out: Surface;
    out.pbr = vec3<f32>(1.0);
    let has_surface = pictures.surface.x > 0.5;
    let roughness_only = pictures.surface.z > 0.5;
    if pictures.tiles.w < 0.5 {
        out.colour = textureSampleGrad(diffuse_t, diffuse_s, uv, ddx, ddy).rgb;
        out.normal = textureSampleGrad(normal_t, normal_s, uv, ddx, ddy).rgb * 2.0 - 1.0;
        if has_surface {
            out.pbr = surface_sample(surface_t, surface_s, roughness_only, uv, ddx, ddy);
        }
        return out;
    }
    let corners = corners_of(uv);
    var w = pow(corners.weights, vec3<f32>(SHARPNESS));
    w = w / (w.x + w.y + w.z);
    let a = look_at(corners.a, uv, ddx, ddy, pictures.tiles);
    let b = look_at(corners.b, uv, ddx, ddy, pictures.tiles);
    let c = look_at(corners.c, uv, ddx, ddy, pictures.tiles);
    let colour_a = textureSampleGrad(diffuse_t, diffuse_s, a.uv, a.ddx, a.ddy).rgb;
    let colour_b = textureSampleGrad(diffuse_t, diffuse_s, b.uv, b.ddx, b.ddy).rgb;
    let colour_c = textureSampleGrad(diffuse_t, diffuse_s, c.uv, c.ddx, c.ddy).rgb;
    let normal_a = textureSampleGrad(normal_t, normal_s, a.uv, a.ddx, a.ddy).rgb * 2.0 - 1.0;
    let normal_b = textureSampleGrad(normal_t, normal_s, b.uv, b.ddx, b.ddy).rgb * 2.0 - 1.0;
    let normal_c = textureSampleGrad(normal_t, normal_s, c.uv, c.ddx, c.ddy).rgb * 2.0 - 1.0;
    // A normal turns with the picture it came from (the turn is a rotation and perhaps a mirror,
    // so its transpose is its inverse); `v * m` is `transpose(m) * v`.
    let slope = (normal_a.xy * a.turn) * w.x + (normal_b.xy * b.turn) * w.y + (normal_c.xy * c.turn) * w.z;
    // Blending three unrelated pictures flattens them, by the square root of the sum of the
    // squared weights; put the spread round the picture's average colour back.
    let restore = inverseSqrt(dot(w, w));
    let average = textureSampleLevel(diffuse_t, diffuse_s, vec2<f32>(0.5, 0.5), 16.0).rgb;
    if has_surface {
        // Occlusion, roughness and metal are blended as they are: they have no spread to put back.
        let pbr_a = surface_sample(surface_t, surface_s, roughness_only, a.uv, a.ddx, a.ddy);
        let pbr_b = surface_sample(surface_t, surface_s, roughness_only, b.uv, b.ddx, b.ddy);
        let pbr_c = surface_sample(surface_t, surface_s, roughness_only, c.uv, c.ddx, c.ddy);
        out.pbr = pbr_a * w.x + pbr_b * w.y + pbr_c * w.z;
    }
    let blended = colour_a * w.x + colour_b * w.y + colour_c * w.z;
    out.colour = max(average + (blended - average) * restore, vec3<f32>(0.0));
    var xy = slope * restore;
    let length_squared = dot(xy, xy);
    if length_squared > 0.9 {
        xy = xy * sqrt(0.9 / length_squared);
    }
    out.normal = vec3<f32>(xy, sqrt(1.0 - dot(xy, xy)));
    return out;
}

// The large picture's multiplier where a point of the world is, looked up on the flat that the
// surface faces (a floor by east and north, a wall by along-the-wall and up), twice, at two sizes
// and turns, so that its own repeat doesn't show either. 1 is no change.
fn wide_multiplier(world: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let flat_point = flat_point_of(world, normal);
    let per_metre = breaker.wide.y;
    let first = flat_point * per_metre;
    let second = vec2<f32>(0.8 * flat_point.x - 0.6 * flat_point.y, 0.6 * flat_point.x + 0.8 * flat_point.y) * (per_metre * 0.43) + vec2<f32>(0.31, 0.57);
    let a = textureSampleLevel(wide_texture, wide_sampler, first, 0.0).rgb * 2.0;
    let b = textureSampleLevel(wide_texture, wide_sampler, second, 0.0).rgb * 2.0;
    return a * b;
}

// Where a point of the world is on the flat its surface faces, in metres: a floor by east and
// north, a wall by along-the-wall and up.
fn flat_point_of(world: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let n = abs(normal);
    if n.y >= n.x && n.y >= n.z {
        return world.xz;
    } else if n.x >= n.z {
        return world.zy;
    }
    return world.xy;
}

// Smooth noise from 0 to 1 over whole-number cells, from the same hash as the tiles.
fn value_noise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = p - floor(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = cell_look(i).x;
    let b = cell_look(i + vec2<i32>(1, 0)).x;
    let c = cell_look(i + vec2<i32>(0, 1)).x;
    let d = cell_look(i + vec2<i32>(1, 1)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// How much of the second material shows, from the vertex weight `w`: the weight pushed about by
// a noise in the middle of the edge (not where it is 0 or 1), then a height blend, in which the
// lighter parts of each picture (a stone, a tuft) stand higher and show first.
fn blend_weight(w: f32, world: vec3<f32>, normal: vec3<f32>, a: vec3<f32>, a_average: vec3<f32>, b: vec3<f32>, b_average: vec3<f32>) -> f32 {
    let at = flat_point_of(world, normal);
    let noise = value_noise(at / EDGE_NOISE_COARSE) * 0.65 + value_noise(at / EDGE_NOISE_FINE + vec2<f32>(17.3, 5.1)) * 0.35;
    let pushed = clamp(w + (noise * 2.0 - 1.0) * EDGE_WANDER * 4.0 * w * (1.0 - w), 0.0, 1.0);
    let luminance = vec3<f32>(0.2126, 0.7152, 0.0722);
    let height_a = dot(a - a_average, luminance) * HEIGHT_GAIN;
    let height_b = dot(b - b_average, luminance) * HEIGHT_GAIN;
    let level_a = (1.0 - pushed) + height_a;
    let level_b = pushed + height_b;
    let top = max(level_a, level_b) - BLEND_DEPTH;
    let share_a = max(level_a - top, 0.0);
    let share_b = max(level_b - top, 0.0);
    return share_b / max(share_a + share_b, 0.000001);
}

// The light from a direction (Bevy's axes) in the gradient of the sky: `Sky::clear_sky_toward` in
// `sky.rs`, without the sun and the clouds (the sun is a light of its own).
fn sky_toward(direction: vec3<f32>) -> vec3<f32> {
    let up = clamp(direction.y, -1.0, 1.0);
    let high = pow(1.0 - max(up, 0.0), 3.5);
    let above = mix(breaker.sky_zenith.rgb, breaker.sky_haze.rgb, high);
    let ground = 1.0 - smoothstep(-0.08, 0.0, up);
    return mix(above, breaker.sky_ground.rgb, ground);
}

// What a surface of a baked Map mirrors of the sky (`SkyReflection` in `surface_look.rs`), before
// the camera's exposure: the sky where the camera's view bounces off the surface, as a surface
// with this roughness and metal reflects it, and as much as the atlas says the spot is open to
// the sky. A rough surface sees a smeared sky, the average of the gradient.
fn sky_reflection(pbr_input: pbr_types::PbrInput) -> vec3<f32> {
    return sky_reflection_of(pbr_input, pbr_input.N, pbr_input.material.perceptual_roughness, pbr_input.material.metallic, pbr_input.material.reflectance);
}

fn sky_reflection_of(pbr_input: pbr_types::PbrInput, n: vec3<f32>, roughness: f32, metallic: f32, reflectance: vec3<f32>) -> vec3<f32> {
    let v = pbr_input.V;
    let n_dot_v = max(dot(n, v), 0.0001);
    let f0 = 0.16 * reflectance * reflectance * (1.0 - metallic) + pbr_input.material.base_color.rgb * metallic;
    let brdf = pbr_lighting::EnvBRDFApprox(f0, pbr_lighting::F_AB(roughness, n_dot_v));
    let sharp = sky_toward(reflect(-v, n));
    let smeared = 0.5 * (breaker.sky_zenith.rgb + breaker.sky_haze.rgb);
    let seen = mix(sharp, smeared, smoothstep(0.2, 0.8, roughness));
    let light = dot(pbr_input.lightmap_light, vec3<f32>(0.2126, 0.7152, 0.0722));
    let open = clamp(light / max(breaker.sky_zenith.w, 0.000001), 0.0, 1.0);
    return seen * brdf * open * pbr_input.specular_occlusion;
}

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let in = vertex_output;
    // Before anything takes a branch: the pixel's slope over the picture, for the mip level.
    let ddx = dpdx(in.uv);
    let ddy = dpdy(in.uv);

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var own: Pictures;
    own.tiles = breaker.snap_turn;
    own.surface = breaker.surface;
    var surface = surface_at(diffuse_texture, diffuse_sampler, normal_texture, diffuse_sampler, surface_texture, diffuse_sampler, own, in.uv, ddx, ddy);
    // Roughness and metal of the surface's own material: its picture's, or its numbers.
    var roughness = select(pbr_input.material.perceptual_roughness, surface.pbr.g, breaker.surface.x > 0.5);
    var metallic = pbr_input.material.metallic * surface.pbr.b;
    var wetness = clamp(breaker.surface.y, 0.0, 1.0);
@if(VERTEX_COLORS) {
    // The second material, as much as the vertex colour's alpha says (#321).
    let w = in.color.a;
    if breaker.blend.x > 0.5 && w > 0.0 {
        var other: Pictures;
        other.tiles = breaker.blend_tiles;
        other.surface = vec4<f32>(breaker.blend_surface.x, 0.0, breaker.blend_surface.y, 0.0);
        let scale = breaker.blend.y;
        let second = surface_at(blend_diffuse_texture, diffuse_sampler, blend_normal_texture, diffuse_sampler, blend_surface_texture, diffuse_sampler, other, in.uv * scale, ddx * scale, ddy * scale);
        var share = 1.0;
        if w < 1.0 {
            let average_a = textureSampleLevel(diffuse_texture, diffuse_sampler, vec2<f32>(0.5, 0.5), 16.0).rgb;
            let average_b = textureSampleLevel(blend_diffuse_texture, diffuse_sampler, vec2<f32>(0.5, 0.5), 16.0).rgb;
            share = blend_weight(w, in.world_position.xyz, in.world_normal, surface.colour, average_a, second.colour, average_b);
        }
        let roughness_b = select(breaker.blend.z, second.pbr.g, breaker.blend_surface.x > 0.5);
        surface.colour = mix(surface.colour, second.colour, share);
        let slope = mix(surface.normal.xy, second.normal.xy, share);
        surface.normal = vec3<f32>(slope, sqrt(max(1.0 - dot(slope, slope), 0.0)));
        surface.pbr = vec3<f32>(mix(surface.pbr.r, second.pbr.r, share), 1.0, 1.0);
        roughness = mix(roughness, roughness_b, share);
        metallic = mix(metallic, breaker.blend_surface.z * second.pbr.b, share);
        wetness = mix(wetness, clamp(breaker.blend.w, 0.0, 1.0), share);
    }
}
    // A decal's mask, with the noise of its JPG below MASK_FLOOR taken out (#321).
    var mask = 1.0;
    if breaker.decal.x > 0.5 {
        let m = textureSampleGrad(mask_texture, mask_sampler, in.uv, ddx, ddy).r;
        mask = clamp((m - MASK_FLOOR) / (1.0 - MASK_FLOOR), 0.0, 1.0);
        if mask <= 0.0 {
            discard;
        }
        wetness = wetness * mask;
    }
    let film_wetness = wetness;
    var colour = surface.colour;
    var normal = surface.normal;
    pbr_input.material.perceptual_roughness = roughness;
    pbr_input.material.metallic = metallic;
    if breaker.surface.x > 0.5 || breaker.blend.x > 0.5 {
        // Bevy's lightmap light isn't dimmed by occlusion, so it is dimmed here, with the rest.
        pbr_input.diffuse_occlusion = vec3<f32>(surface.pbr.r);
        pbr_input.specular_occlusion = surface.pbr.r;
        pbr_input.lightmap_light = pbr_input.lightmap_light * surface.pbr.r;
    }
    if breaker.wide.x > 0.0 {
        let multiplier = mix(vec3<f32>(1.0), wide_multiplier(in.world_position.xyz, in.world_normal), breaker.wide.x);
        colour = colour * multiplier;
        // Lighter patches are a little rougher (dry, bleached) and darker ones smoother (damp).
        let lightness = dot(multiplier, vec3<f32>(0.3, 0.55, 0.15));
        pbr_input.material.perceptual_roughness = clamp(pbr_input.material.perceptual_roughness + (lightness - 1.0) * 0.5, 0.2, 1.0);
    }
    if wetness > 0.0 {
        let dry = pbr_input.material.perceptual_roughness;
        pbr_input.material.perceptual_roughness = mix(dry, WET_GLOSS_FLOOR + WET_GLOSS_SCALE * dry, wetness);
        pbr_input.material.reflectance = vec3<f32>(mix(DRY_REFLECTANCE, WET_REFLECTANCE, wetness));
        colour = colour * mix(1.0, WET_KEEPS, wetness);
        normal = vec3<f32>(normal.xy * mix(1.0, WET_KEEPS_SLOPE, wetness), normal.z);
    }
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * colour, pbr_input.material.base_color.a);
@if(VERTEX_TANGENTS) {
    let double_sided = (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    let tbn = calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
    pbr_input.N = apply_normal_mapping(pbr_input.material.flags, tbn, double_sided, is_front, normal * 0.5 + 0.5);
}

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    if breaker.sky_haze.w > 0.5 {
        out.color = vec4<f32>(out.color.rgb + sky_reflection(pbr_input) * mesh_view_bindings::view.exposure, out.color.a);
    }
    if breaker.decal.x > 0.5 {
        // Premultiplied: the decal's own colour where it covers, and over the rest a film of
        // water that darkens the surface under it and adds the sheen of the sky.
        let cover = clamp(breaker.decal.y, 0.0, 1.0) * mask;
        let darken = film_wetness * (1.0 - WET_KEEPS);
        var sheen = vec3<f32>(0.0);
        if breaker.sky_haze.w > 0.5 && film_wetness > 0.0 {
            let film = sky_reflection_of(pbr_input, normalize(pbr_input.world_normal), FILM_ROUGHNESS, 0.0, vec3<f32>(WET_REFLECTANCE));
            sheen = film * film_wetness * FILM_SHEEN * mesh_view_bindings::view.exposure;
        }
        out.color = vec4<f32>(out.color.rgb * cover + sheen * (1.0 - cover), cover + (1.0 - cover) * darken);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
