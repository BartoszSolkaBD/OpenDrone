//! Surfaces whose picture doesn't visibly repeat (#304), and that read their
//! roughness, occlusion and metal from pictures and can be wet (#320).
//!
//! A material's picture is one 1 to 3 m tile laid edge to edge, which shows
//! as blocks on a big floor and as the same stripe again and again on a long
//! wall, from above and from far. A material that asks for it in
//! `assets-src/materials.toml` is drawn by [`RepeatBreaker`] instead of the
//! plain material:
//!
//! - **No visible tiles.** The surface's picture coordinates fall on a
//!   triangular grid, every corner of which has a random look (a shift of the
//!   picture, and for some materials a turn and a mirror too), and a pixel
//!   blends the three pictures round its triangle (hex tiling). The blend is
//!   narrow, so most of a triangle is one clean copy of the picture, and the
//!   spread of its colour is put back, so it isn't flattened. The picture's
//!   detail stays sharp. The look of a corner comes from its whole-number
//!   position alone: [`cell_look`] is the same numbers the shader makes.
//! - **Large-scale variation.** A picture made by a generator, 80 m wide,
//!   lies over the world and multiplies the colour of every material that
//!   asks, by where a surface is, so stained patches, clean and dirty areas
//!   and bleached or damp zones carry on from one mesh and wall to the next
//!   ([`wide_lookups`] says where in the picture a point looks).
//!
//! - **Roughness, occlusion, metal and wetness.** A material that asks for
//!   them ([`crate::surface_look`]) is drawn by the same shader, with its
//!   picture of the three (red occlusion, green roughness, blue metal) looked
//!   up the same way as the colour, so the tiles agree, and its wetness
//!   applied last. A material that asks for these and not for its repeat to be
//!   broken is drawn by it with the repeat left alone.
//!
//! - **Two materials on one mesh (#321).** A part whose `.glb` blends a
//!   second material into its own (`od_blend` and its vertex colours) is
//!   drawn with both, the second looked up on its own tiles and shown as much
//!   as the vertex colour says, with an edge that a noise and the two
//!   pictures' light and dark break up ([`crate::decals::blend_weight`]).
//! - **Decals (#321).** A decal is drawn by the same shader with its one
//!   picture and its mask, over the surface under it.
//!
//! The shader is `repeat_breaker.wesl`. Nothing here touches the Simulation.

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// A [`StandardMaterial`] whose picture is drawn with the repeat broken.
pub type VariedMaterial = ExtendedMaterial<StandardMaterial, RepeatBreaker>;

/// How many triangle corners fit in one repeat of a picture (2 times the
/// square root of 3), so a hexagon is about 0.58 of a repeat across.
pub const CELLS_PER_REPEAT: f32 = 3.464_101_6;

/// Where the shader is, as Bevy's embedded assets name it.
const SHADER: &str = "embedded://opendrone/repeat_breaker.wesl";

/// What the shader is told about the surface, in two groups of four numbers.
#[derive(ShaderType, Reflect, Clone, Copy, Debug, PartialEq)]
pub struct BreakerSettings {
    /// `x` and `y`: a corner shifts the picture by whole multiples of 1/`x`
    /// and 1/`y` of it, or any amount for 0. `z`: 1 if corners turn and
    /// mirror the picture too. `w`: 1 to break the repeat at all.
    pub snap_turn: Vec4,
    /// `x`: how strongly the large picture shows (0 for none). `y`: its
    /// repeats to a metre.
    pub wide: Vec4,
    /// `x`: 1 if the surface picture (occlusion, roughness, metal) is bound.
    /// `y`: how wet the surface is, 0 to 1. `z`: 1 if the picture holds
    /// roughness alone, in its one channel.
    pub surface: Vec4,
    /// What the surface reflects of the Map's sky, for a baked Map
    /// ([`crate::surface_look::SkyReflection`]): the zenith's colour and, in
    /// its `w`, how bright the sky-lit ground is; the horizon's colour and, in
    /// its `w`, 1 if there is a sky to reflect; the ground's colour.
    pub sky_zenith: Vec4,
    pub sky_haze: Vec4,
    pub sky_ground: Vec4,
    /// The second material, if one shows through (#321): its tiles, as
    /// `snap_turn` says them for the first.
    pub blend_tiles: Vec4,
    /// `x`: 1 if a second material shows through; `y`: its repeats to one of
    /// the first's (the two pictures' sizes differ); `z`: its roughness
    /// number; `w`: its wetness.
    pub blend: Vec4,
    /// `x`: 1 if its surface picture is bound; `y`: 1 if that holds
    /// roughness alone; `z`: how metal it is.
    pub blend_surface: Vec4,
    /// `x`: 1 if the part is a decal; `y`: how much of its own colour shows
    /// where its mask is whole (`cover`).
    pub decal: Vec4,
}

impl BreakerSettings {
    /// What a surface that asks for nothing says: no tiles, no large
    /// picture, no surface picture, no sky, no second material and no decal.
    pub const NOTHING: BreakerSettings = BreakerSettings {
        snap_turn: Vec4::ZERO,
        wide: Vec4::ZERO,
        surface: Vec4::ZERO,
        sky_zenith: Vec4::ZERO,
        sky_haze: Vec4::ZERO,
        sky_ground: Vec4::ZERO,
        blend_tiles: Vec4::ZERO,
        blend: Vec4::ZERO,
        blend_surface: Vec4::ZERO,
        decal: Vec4::ZERO,
    };
}

/// The extension of the standard material that breaks the repeat of its
/// colour and normal pictures and lays the large picture over them.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct RepeatBreaker {
    // The base material's bindings are 0 to 99.
    #[uniform(100)]
    pub settings: BreakerSettings,
    /// Every picture but `wide` and `mask` is read through this one sampler:
    /// they all repeat and filter alike, and a sampler each made 17 in the
    /// fragment stage with the base material's, over the 16 a Mac allows
    /// (#339).
    #[texture(101)]
    #[sampler(102)]
    pub diffuse: Handle<Image>,
    #[texture(103)]
    pub normal: Handle<Image>,
    #[texture(105)]
    #[sampler(106)]
    pub wide: Option<Handle<Image>>,
    /// Red occlusion, green roughness, blue metal, packed by
    /// [`crate::surface_look::pack_surface`].
    #[texture(107)]
    pub surface: Option<Handle<Image>>,
    /// The second material's colour, normal map and surface picture (#321).
    #[texture(109)]
    pub blend_diffuse: Option<Handle<Image>>,
    #[texture(111)]
    pub blend_normal: Option<Handle<Image>>,
    #[texture(113)]
    pub blend_surface: Option<Handle<Image>>,
    /// A decal's mask: how much of it covers the surface under it (#321).
    #[texture(115)]
    #[sampler(116)]
    pub mask: Option<Handle<Image>>,
}

impl MaterialExtension for RepeatBreaker {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Puts the shader in the game and draws the materials that use it.
pub struct RepeatBreakingPlugin;

impl Plugin for RepeatBreakingPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "repeat_breaker.wesl");
        app.add_plugins(MaterialPlugin::<VariedMaterial>::default());
    }
}

/// A number from a whole-number position, as the shader makes it (PCG).
fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

/// Four random numbers in [0, 1) for a corner of the triangular grid: a shift
/// across and up (before any snapping), a turn and a mirror. The same numbers
/// the shader makes, from the corner's position alone.
pub fn cell_look(cell: [i32; 2]) -> [f32; 4] {
    let h0 = pcg((cell[0] as u32) ^ pcg(cell[1] as u32));
    let hashes = [
        h0,
        pcg(h0.wrapping_add(1)),
        pcg(h0.wrapping_add(2)),
        pcg(h0.wrapping_add(3)),
    ];
    hashes.map(|h| (h >> 8) as f32 * (1.0 / 16_777_216.0))
}

/// How far a corner shifts the picture across and up, in repeats, for a
/// picture whose shifts are snapped to whole multiples of 1/`snap` (a snap
/// of 0 for any shift): what the shader does with [`cell_look`].
pub fn cell_shift(cell: [i32; 2], snap: [f32; 2]) -> [f32; 2] {
    let look = cell_look(cell);
    [0, 1].map(|axis| {
        if snap[axis] > 0.0 {
            (look[axis] * snap[axis]).floor() / snap[axis]
        } else {
            look[axis]
        }
    })
}

/// The three corners round the triangle that picture coordinates `uv` are in,
/// and how near it is to each (they add up to 1): the grid the shader blends
/// over.
pub fn corners_of(uv: [f32; 2]) -> ([[i32; 2]; 3], [f32; 3]) {
    let st = [uv[0] * CELLS_PER_REPEAT, uv[1] * CELLS_PER_REPEAT];
    let skew = [st[0] - 0.577_350_27 * st[1], 1.154_700_5 * st[1]];
    let base = [skew[0].floor(), skew[1].floor()];
    let t = [skew[0] - base[0], skew[1] - base[1]];
    let at = [base[0] as i32, base[1] as i32];
    let z = 1.0 - t[0] - t[1];
    if z > 0.0 {
        (
            [at, [at[0], at[1] + 1], [at[0] + 1, at[1]]],
            [z, t[1], t[0]],
        )
    } else {
        (
            [
                [at[0] + 1, at[1] + 1],
                [at[0] + 1, at[1]],
                [at[0], at[1] + 1],
            ],
            [-z, 1.0 - t[1], 1.0 - t[0]],
        )
    }
}

/// Where in the large picture (in repeats) a point of the world looks, twice:
/// on the flat the surface faces, a floor by east and north, a wall by along
/// the wall and up, at two sizes and turns so that the picture's own repeat
/// doesn't show. `world` and `normal` are in Bevy's axes.
pub fn wide_lookups(world: Vec3, normal: Vec3, per_metre: f32) -> [Vec2; 2] {
    let n = normal.abs();
    let flat = if n.y >= n.x && n.y >= n.z {
        Vec2::new(world.x, world.z)
    } else if n.x >= n.z {
        Vec2::new(world.z, world.y)
    } else {
        Vec2::new(world.x, world.y)
    };
    let turned = Vec2::new(0.8 * flat.x - 0.6 * flat.y, 0.6 * flat.x + 0.8 * flat.y);
    [
        flat * per_metre,
        turned * (per_metre * 0.43) + Vec2::new(0.31, 0.57),
    ]
}
