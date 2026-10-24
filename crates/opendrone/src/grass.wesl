// Real grass (#338): the corners of a Map's clumps of grass. Each corner carries its clump's root
// and rank, its colour and how far up its blade it is. The shader thins the clumps with distance
// (a clump whose rank is over `keep` at its distance shrinks into the ground and goes), sways the
// blades in the wind, more at the tip, and hands the standard fragment the corner's colour and the
// ground's normal, so a clump is lit as the ground it stands on is: by the atlas at its root on a
// Map with baked light, by the sun and sky on one without.
//
// The thinning numbers are `FULL_UNTIL`, `TAPER_FROM` and `DRAW_DISTANCE` in `grass.rs`, whose
// `keep` the readable checks use: keep the two in step.
import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
    forward_io::VertexOutput,
};

// x, y: the direction the wind blows towards (Bevy's x and z); z: how far a tip sways, in
// metres; w: how fast the gusts come.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: vec4<f32>;

const FULL_UNTIL: f32 = 15.0;
const TAPER_FROM: f32 = 42.0;
const DRAW_DISTANCE: f32 = 60.0;
// How much rank a clump shrinks over as its distance goes past where it is kept.
const SHRINK: f32 = 0.08;

struct GrassVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @if(VERTEX_UVS_B)
    @location(3) uv_b: vec2<f32>,
    @location(8) root: vec4<f32>,
    @location(9) look: vec4<f32>,
};

// The share of clumps drawn at a distance: all up to FULL_UNTIL, then one over the distance,
// to none at DRAW_DISTANCE.
fn keep(distance: f32) -> f32 {
    let share = min(FULL_UNTIL / max(distance, 1e-3), 1.0);
    return share * (1.0 - smoothstep(TAPER_FROM, DRAW_DISTANCE, distance));
}

@vertex
fn vertex(v: GrassVertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let root = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.root.xyz, 1.0)).xyz;
    var at = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0)).xyz;

    // The wind: two slow waves rolling across the field along the wind, so neighbours sway
    // together, and a quick flutter of the clump's own.
    let along = dot(root.xz, wind.xy);
    let t = globals.time * wind.w;
    let gust = 0.65 + 0.35 * sin(t * 0.7 - along * 0.35) + 0.25 * sin(t * 1.9 - along * 0.9 + root.x * 0.3);
    let flutter = 0.12 * sin(t * 6.0 + v.root.w * 40.0);
    let tip = v.look.a * v.look.a;
    at = at + vec3<f32>(wind.x, 0.0, wind.y) * (wind.z * (gust + flutter) * tip);

    // Thinning: a clump past its distance shrinks into the ground, then is gone.
    let distance = length(view.world_position - root);
    let grow = clamp((keep(distance) - v.root.w) / SHRINK, 0.0, 1.0);
    at = root + (at - root) * grow;

    out.world_position = vec4<f32>(at, 1.0);
    out.position = position_world_to_clip(at);
    // Lit as the ground is: up.
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
    @if(VERTEX_UVS_A)
    out.uv = vec2<f32>(0.0, 0.0);
    @if(VERTEX_UVS_B)
    out.uv_b = v.uv_b;
    @if(VERTEX_COLORS)
    out.color = vec4<f32>(v.look.rgb, 1.0);
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = v.instance_index;
    @if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        v.instance_index, world_from_local[3]);
    return out;
}
