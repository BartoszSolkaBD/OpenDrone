//! Real grass (#338): a Map's ground tagged `od_scatter` grows thousands of
//! clumps of blades, each a small mesh, so grass has depth where the ground's
//! picture alone is flat.
//!
//! - **Growing.** The game places the clumps when the Map is drawn, on the
//!   tagged part's upward-facing triangles, from a fixed seed and the part's
//!   name: the same Map always grows the same grass ([`grow`]). A clump has
//!   6 to 12 blades, each two segments and a tip (three triangles), leaning
//!   outward and curving, of its own height, colour and width.
//! - **Drawing.** The clumps are put together into one mesh for each 8 m
//!   square of ground (a cell), drawn with one material: one draw for a cell,
//!   not one entity for a clump. A cell further from the camera than the draw
//!   distance isn't drawn at all.
//! - **Thinning.** Each clump has a rank from 0 to 1, and the shader draws
//!   only the clumps whose rank is under [`keep`] at their distance: all of
//!   them near, fewer further away, none at the draw distance. A clump about
//!   to go shrinks into the ground first, so there is no edge where the grass
//!   stops: the ground's picture takes over.
//! - **Light.** A clump takes the light of the ground it stands on: on a Map
//!   with baked light, every corner of the clump reads the atlas where its
//!   root is (the atlas can't hold the blades themselves); on a Map without,
//!   the sun and sky light it as the ground, with its shadows falling on it.
//!   Its own blades get darker towards the root. Grass casts no shadows.
//! - **Wind.** The shader sways the blades a little, more at the tip, in
//!   slow gusts that roll across the field.
//! - **Not solid.** Grass is never a collider: a Quad flies through it.
//!
//! The numbers the shader thins and sways with are in `grass.wesl`, the same
//! as here: keep the two in step. Nothing here touches the Simulation.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::VisibilityRange;
use bevy::light::NotShadowCaster;
use bevy::mesh::{
    Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology,
    VertexAttributeValues, VertexFormat,
};
use bevy::pbr::{
    ExtendedMaterial, Lightmap, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use opendrone_pack::{DrawnPart, Scatter, ScatterKind};

/// Where the shader is, as Bevy's embedded assets name it.
const SHADER: &str = "embedded://opendrone/grass.wesl";

/// The seed every Map's grass grows from, with each part's name.
pub const GRASS_SEED: u64 = 0x6772_6173_735f_3333;

/// The side of the square of ground whose clumps are drawn together, in
/// metres.
pub const CELL: f32 = 8.0;

/// Up to this distance from the camera, in metres, every clump is drawn.
pub const FULL_UNTIL: f32 = 15.0;

/// From this distance the last clumps start to go, and at
/// [`DRAW_DISTANCE`] none is left.
pub const TAPER_FROM: f32 = 42.0;

/// No clump is drawn further than this from the camera, in metres.
pub const DRAW_DISTANCE: f32 = 60.0;

/// The fewest and most blades a clump has.
pub const BLADES: (u32, u32) = (6, 12);

/// How much of the ground a triangle must face up to grow grass: the cosine
/// of the steepest slope, 50°. A wall or a slab's side grows none.
pub const STEEPEST: f32 = 0.64;

/// The share of clumps drawn at a distance from the camera, in metres: 1 up
/// to [`FULL_UNTIL`], then falling as one over the distance (about the same
/// number of clumps in each step of distance on the screen), to none at
/// [`DRAW_DISTANCE`]. A clump whose rank is under it is drawn.
pub fn keep(distance: f32) -> f32 {
    let share = (FULL_UNTIL / distance.max(1e-3)).min(1.0);
    share * (1.0 - smoothstep(TAPER_FROM, DRAW_DISTANCE, distance))
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One clump of grass, as the game grows it: in the Map's axes (east, north,
/// up), in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct Clump {
    /// Where its root stands on the ground.
    pub at: [f32; 3],
    /// Its tallest blade's height.
    pub height: f32,
    /// How many blades it has.
    pub blades: u32,
    /// The compass turn of its first blade, in radians.
    pub turn: f32,
    /// Its colour, as linear red, green and blue.
    pub colour: [f32; 3],
    /// From 0 to 1: it is drawn where [`keep`] is above this.
    pub rank: f32,
    /// Its own number, for the rest of its look.
    pub seed: u64,
    /// Where its root is in the Map's light atlas, if the ground has one.
    pub light_uv: Option<[f32; 2]>,
}

/// A stream of numbers from a seed (SplitMix64): the same seed gives the
/// same numbers on every computer.
pub struct Seeded(u64);

impl Seeded {
    pub fn new(seed: u64) -> Seeded {
        Seeded(seed)
    }

    pub fn number(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number from 0 up to 1.
    pub fn unit(&mut self) -> f32 {
        (self.number() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

/// The seed of a part's grass: [`GRASS_SEED`] and the part's name (FNV-1a).
fn part_seed(name: &str) -> u64 {
    name.bytes()
        .fold(GRASS_SEED ^ 0xCBF2_9CE4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
        })
}

/// The colours a clump of grass is mixed from, linear: fresh green, olive
/// and dry straw.
const GRASS_COLOURS: [[f32; 3]; 3] = [
    [0.040, 0.125, 0.012],
    [0.070, 0.115, 0.020],
    [0.150, 0.125, 0.045],
];

/// The clumps a part grows, in a fixed order: for each upward-facing
/// triangle, as many as its area and the density make, each at a random
/// place on it.
pub fn grow(part: &DrawnPart, scatter: &Scatter) -> Vec<Clump> {
    let mut random = Seeded::new(part_seed(&part.name));
    let mut clumps = Vec::new();
    for triangle in &part.triangles {
        let [a, b, c] = triangle.map(|i| Vec3::from(part.corners[i as usize]));
        let face = (b - a).cross(c - a);
        let area = face.length() / 2.0;
        if area <= 0.0 || face.z / face.length() < STEEPEST {
            continue;
        }
        let count = (area * scatter.density + random.unit()).floor() as u32;
        for _ in 0..count {
            // A uniform place on the triangle.
            let (r1, r2) = (random.unit().sqrt(), random.unit());
            let (wa, wb, wc) = (1.0 - r1, r1 * (1.0 - r2), r1 * r2);
            let at = a * wa + b * wb + c * wc;
            let light_uv = (part.light_uvs.len() == part.corners.len()).then(|| {
                let [la, lb, lc] = triangle.map(|i| Vec2::from(part.light_uvs[i as usize]));
                (la * wa + lb * wb + lc * wc).to_array()
            });
            let (dry, olive) = (random.unit(), random.unit());
            let mix = |i: usize| {
                let fresh =
                    GRASS_COLOURS[0][i] + (GRASS_COLOURS[1][i] - GRASS_COLOURS[0][i]) * olive;
                // One clump in six or so is drier.
                let straw = (dry - 0.82).max(0.0) / 0.18;
                fresh + (GRASS_COLOURS[2][i] - fresh) * straw
            };
            clumps.push(Clump {
                at: at.to_array(),
                height: random.between(scatter.height[0], scatter.height[1]),
                blades: BLADES.0 + (random.unit() * (BLADES.1 - BLADES.0 + 1) as f32) as u32,
                turn: random.unit() * TAU,
                colour: [mix(0), mix(1), mix(2)],
                rank: random.unit(),
                seed: random.number(),
                light_uv,
            });
        }
    }
    clumps
}

/// The cell a point (east, north) falls in.
pub fn cell_of(at: [f32; 3]) -> (i32, i32) {
    ((at[0] / CELL).floor() as i32, (at[1] / CELL).floor() as i32)
}

/// The clumps a camera at `eye` (east, north, up) would draw: those whose
/// rank is under [`keep`] at their distance, whether in view or not.
pub fn drawn_from(clumps: &[Clump], eye: [f32; 3]) -> usize {
    let eye = Vec3::from(eye);
    clumps
        .iter()
        .filter(|c| c.rank < keep(eye.distance(Vec3::from(c.at))))
        .count()
}

/// A clump's root and rank on every corner of its blades.
pub const ATTRIBUTE_ROOT: MeshVertexAttribute =
    MeshVertexAttribute::new("Grass_Root", 338_001, VertexFormat::Float32x4);

/// A corner's colour (red, green, blue) and how far up its blade it is (0 at
/// the root, 1 at the tip), as whole numbers that stand for 0 to 1.
pub const ATTRIBUTE_LOOK: MeshVertexAttribute =
    MeshVertexAttribute::new("Grass_Look", 338_002, VertexFormat::Unorm8x4);

/// Bevy's axes from the Map's: east is +x, up +y, north −z.
fn bevy(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y)
}

/// A mesh of clumps: five corners and three triangles for every blade, in
/// Bevy's axes, with each corner's clump root and rank ([`ATTRIBUTE_ROOT`]),
/// colour and height up the blade ([`ATTRIBUTE_LOOK`]), and the root's place
/// in the light atlas (`UV_1`) if the clumps have one.
pub fn clumps_mesh(clumps: &[&Clump]) -> Mesh {
    let blades: usize = clumps.iter().map(|c| c.blades as usize).sum();
    let mut positions = Vec::with_capacity(blades * 5);
    let mut roots = Vec::with_capacity(blades * 5);
    let mut looks = Vec::with_capacity(blades * 5);
    let mut light_uvs = Vec::new();
    let mut indices: Vec<u32> = Vec::with_capacity(blades * 9);
    let unorm = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    for clump in clumps {
        let mut random = Seeded::new(clump.seed);
        let root = Vec3::from(clump.at);
        let root_bevy = bevy(root);
        for blade in 0..clump.blades {
            let turn = clump.turn + blade as f32 * 2.399_963 + random.between(-0.4, 0.4);
            let out = Vec3::new(turn.cos(), turn.sin(), 0.0);
            let side = Vec3::new(-out.y, out.x, 0.0);
            // A blade starts a little away from the middle, leans outward
            // and curves over towards its tip.
            let foot = root + out * random.between(0.0, 0.09) * (clump.height / 0.3).min(2.0);
            let height = clump.height * random.between(0.55, 1.0);
            let bend = random.between(0.15, 0.6);
            let width = random.between(0.010, 0.020) * (clump.height / 0.3).clamp(0.6, 2.0);
            let twist = side * random.between(-0.3, 0.3) * width;
            let shade = random.between(0.85, 1.15);
            let along = |t: f32| foot + Vec3::Z * (height * t) + out * (height * bend * t * t);
            let first = positions.len() as u32;
            for (t, half) in [(0.0, width), (0.5, width * 0.7), (1.0, 0.0)] {
                let middle = along(t);
                let tip = half == 0.0;
                let corners: &[Vec3] = if tip {
                    &[middle + twist]
                } else {
                    &[middle - side * half, middle + side * half]
                };
                for corner in corners {
                    positions.push(bevy(*corner).to_array());
                    roots.push([root_bevy.x, root_bevy.y, root_bevy.z, clump.rank]);
                    // Darker at the root, where the clump shades itself.
                    let light = (0.35 + 0.65 * t) * shade;
                    looks.push([
                        unorm(clump.colour[0] * light * 4.0),
                        unorm(clump.colour[1] * light * 4.0),
                        unorm(clump.colour[2] * light * 4.0),
                        unorm(t),
                    ]);
                    if let Some(uv) = clump.light_uv {
                        light_uvs.push(uv);
                    }
                }
            }
            // Corners: 0 1 at the foot, 2 3 halfway, 4 the tip.
            indices.extend([0, 1, 2, 1, 3, 2, 2, 3, 4].map(|i| first + i));
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(ATTRIBUTE_ROOT, roots);
    mesh.insert_attribute(ATTRIBUTE_LOOK, VertexAttributeValues::Unorm8x4(looks));
    if !light_uvs.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, light_uvs);
    }
    mesh.insert_indices(match u16::try_from(indices.len()) {
        Ok(_) if indices.iter().all(|i| *i <= u32::from(u16::MAX)) => {
            Indices::U16(indices.into_iter().map(|i| i as u16).collect())
        }
        _ => Indices::U32(indices),
    });
    mesh
}

/// How much brighter a clump's colour is stored than it is drawn: the look
/// attribute holds 0 to 1 in 255 steps, and grass is dark, so it is stored
/// four times over and the material's colour takes it back down.
pub const COLOUR_SCALE: f32 = 4.0;

/// The material grass is drawn with: the standard one, with the grass
/// shader's corners (which thin, sway and light the clumps).
pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassShader>;

/// What the grass shader is told: the wind's direction (east, then up-north
/// in Bevy's axes as x and z), its strength in metres of sway at a tip and
/// how fast its gusts come.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GrassShader {
    #[uniform(100)]
    pub wind: Vec4,
}

impl Default for GrassShader {
    fn default() -> GrassShader {
        // From the west-south-west, 6 cm at a tip, a gust every 3 s or so.
        GrassShader {
            wind: Vec4::new(0.92, -0.38, 0.06, 2.1),
        }
    }
}

impl MaterialExtension for GrassShader {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let mut attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        if layout.0.contains(Mesh::ATTRIBUTE_UV_1) {
            attributes.push(Mesh::ATTRIBUTE_UV_1.at_shader_location(3));
        }
        attributes.push(ATTRIBUTE_ROOT.at_shader_location(8));
        attributes.push(ATTRIBUTE_LOOK.at_shader_location(9));
        descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
        // The shader hands the standard fragment a colour for each corner.
        descriptor.vertex.shader_defs.push("VERTEX_COLORS".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("VERTEX_COLORS".into());
        }
        Ok(())
    }
}

/// Puts the grass shader in the game.
pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "grass.wesl");
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default());
    }
}

/// The grass of a Map, in cells, ready to draw.
pub struct GrownGrass {
    /// Every clump, part by part.
    pub clumps: usize,
    /// One mesh for each cell, in the cells' order, with the cell.
    pub cells: Vec<((ScatterKind, i32, i32), Mesh)>,
}

/// Grows every tagged part's grass and puts it into cells, each kind of
/// plant in its own cells.
pub fn grow_the_map(parts: &[DrawnPart]) -> GrownGrass {
    let mut by_cell: BTreeMap<(ScatterKind, i32, i32), Vec<Clump>> = BTreeMap::new();
    let mut count = 0;
    for part in parts {
        let Some(scatter) = &part.scatter else {
            continue;
        };
        for clump in grow(part, scatter) {
            let (x, y) = cell_of(clump.at);
            by_cell.entry((scatter.kind, x, y)).or_default().push(clump);
            count += 1;
        }
    }
    GrownGrass {
        clumps: count,
        cells: by_cell
            .into_iter()
            .map(|(cell, clumps)| {
                let refs: Vec<&Clump> = clumps.iter().collect();
                (cell, clumps_mesh(&refs))
            })
            .collect(),
    }
}

/// Draws a Map's grass: one entity a cell, with no shadow, not drawn beyond
/// the draw distance, lit by the atlas if the Map has one (with its exposure).
pub fn draw_the_grass(
    world: &mut World,
    parts: Vec<DrawnPart>,
    atlas: Option<(Handle<Image>, f32)>,
    marker: impl Bundle + Clone,
) {
    let grown = grow_the_map(&parts);
    if grown.cells.is_empty() {
        return;
    }
    let material = GrassMaterial {
        base: StandardMaterial {
            base_color: Color::linear_rgb(
                1.0 / COLOUR_SCALE,
                1.0 / COLOUR_SCALE,
                1.0 / COLOUR_SCALE,
            ),
            perceptual_roughness: 0.85,
            reflectance: 0.25,
            cull_mode: None,
            lightmap_exposure: atlas.as_ref().map_or(1.0, |(_, exposure)| *exposure),
            ..default()
        },
        extension: GrassShader::default(),
    };
    let material = world.resource_mut::<Assets<GrassMaterial>>().add(material);
    // A cell's middle is at most half its diagonal from any clump in it.
    let reach = DRAW_DISTANCE + CELL * std::f32::consts::FRAC_1_SQRT_2 + 1.0;
    let cells = grown.cells.len();
    for (_, mesh) in grown.cells {
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let mut cell = world.spawn((
            marker.clone(),
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: reach..reach,
                use_aabb: true,
            },
            Name::new("grass"),
        ));
        if let Some((image, _)) = &atlas {
            cell.insert(Lightmap {
                image: image.clone(),
                ..default()
            });
        }
    }
    info!(
        "grew {} clumps of grass in {} cells of {} m",
        grown.clumps, cells, CELL
    );
}
