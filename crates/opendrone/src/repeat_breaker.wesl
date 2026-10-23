// A Map surface whose picture doesn't visibly repeat (#304): the standard PBR fragment, with its
// colour and normal map taken from a picture that is laid out in hex tiles instead of in one grid,
// and its colour and roughness nudged by a large picture that lies over the world.
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
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> breaker: Breaker;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var diffuse_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var diffuse_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var normal_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var wide_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var wide_sampler: sampler;

// Triangle corners to one repeat of the picture: 2 * sqrt(3), so a hexagon is about 0.58 of a repeat across.
const CELLS_PER_REPEAT: f32 = 3.4641016;
// How narrow the blends are: the weights go to this power.
const SHARPNESS: f32 = 7.0;

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

fn look_at(corner: vec2<i32>, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> Look {
    let r = cell_look(corner);
    let centre = vec2<f32>(f32(corner.x) + 0.5 * f32(corner.y), 0.8660254 * f32(corner.y)) / CELLS_PER_REPEAT;
    let snap = breaker.snap_turn.xy;
    let shift = vec2<f32>(
        select(r.x, floor(r.x * snap.x) / snap.x, snap.x > 0.0),
        select(r.y, floor(r.y * snap.y) / snap.y, snap.y > 0.0),
    );
    var turn = mat2x2<f32>(1.0, 0.0, 0.0, 1.0);
    if breaker.snap_turn.z > 0.0 {
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

struct Surface {
    colour: vec3<f32>,
    // The normal in the surface's own frame, as the normal map holds it, from -1 to 1.
    normal: vec3<f32>,
}

fn surface_at(uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> Surface {
    var out: Surface;
    if breaker.snap_turn.w < 0.5 {
        out.colour = textureSampleGrad(diffuse_texture, diffuse_sampler, uv, ddx, ddy).rgb;
        out.normal = textureSampleGrad(normal_texture, normal_sampler, uv, ddx, ddy).rgb * 2.0 - 1.0;
        return out;
    }
    let corners = corners_of(uv);
    var w = pow(corners.weights, vec3<f32>(SHARPNESS));
    w = w / (w.x + w.y + w.z);
    let a = look_at(corners.a, uv, ddx, ddy);
    let b = look_at(corners.b, uv, ddx, ddy);
    let c = look_at(corners.c, uv, ddx, ddy);
    let colour_a = textureSampleGrad(diffuse_texture, diffuse_sampler, a.uv, a.ddx, a.ddy).rgb;
    let colour_b = textureSampleGrad(diffuse_texture, diffuse_sampler, b.uv, b.ddx, b.ddy).rgb;
    let colour_c = textureSampleGrad(diffuse_texture, diffuse_sampler, c.uv, c.ddx, c.ddy).rgb;
    let normal_a = textureSampleGrad(normal_texture, normal_sampler, a.uv, a.ddx, a.ddy).rgb * 2.0 - 1.0;
    let normal_b = textureSampleGrad(normal_texture, normal_sampler, b.uv, b.ddx, b.ddy).rgb * 2.0 - 1.0;
    let normal_c = textureSampleGrad(normal_texture, normal_sampler, c.uv, c.ddx, c.ddy).rgb * 2.0 - 1.0;
    // A normal turns with the picture it came from (the turn is a rotation and perhaps a mirror,
    // so its transpose is its inverse); `v * m` is `transpose(m) * v`.
    let slope = (normal_a.xy * a.turn) * w.x + (normal_b.xy * b.turn) * w.y + (normal_c.xy * c.turn) * w.z;
    // Blending three unrelated pictures flattens them, by the square root of the sum of the
    // squared weights; put the spread round the picture's average colour back.
    let restore = inverseSqrt(dot(w, w));
    let average = textureSampleLevel(diffuse_texture, diffuse_sampler, vec2<f32>(0.5, 0.5), 16.0).rgb;
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
    let n = abs(normal);
    var flat_point: vec2<f32>;
    if n.y >= n.x && n.y >= n.z {
        flat_point = world.xz;
    } else if n.x >= n.z {
        flat_point = world.zy;
    } else {
        flat_point = world.xy;
    }
    let per_metre = breaker.wide.y;
    let first = flat_point * per_metre;
    let second = vec2<f32>(0.8 * flat_point.x - 0.6 * flat_point.y, 0.6 * flat_point.x + 0.8 * flat_point.y) * (per_metre * 0.43) + vec2<f32>(0.31, 0.57);
    let a = textureSampleLevel(wide_texture, wide_sampler, first, 0.0).rgb * 2.0;
    let b = textureSampleLevel(wide_texture, wide_sampler, second, 0.0).rgb * 2.0;
    return a * b;
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
    let surface = surface_at(in.uv, ddx, ddy);
    var colour = surface.colour;
    if breaker.wide.x > 0.0 {
        let multiplier = mix(vec3<f32>(1.0), wide_multiplier(in.world_position.xyz, in.world_normal), breaker.wide.x);
        colour = colour * multiplier;
        // Lighter patches are a little rougher (dry, bleached) and darker ones smoother (damp).
        let lightness = dot(multiplier, vec3<f32>(0.3, 0.55, 0.15));
        pbr_input.material.perceptual_roughness = clamp(pbr_input.material.perceptual_roughness + (lightness - 1.0) * 0.5, 0.2, 1.0);
    }
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * colour, pbr_input.material.base_color.a);
@if(VERTEX_TANGENTS) {
    let double_sided = (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    let tbn = calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
    pbr_input.N = apply_normal_mapping(pbr_input.material.flags, tbn, double_sided, is_front, surface.normal * 0.5 + 0.5);
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
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
