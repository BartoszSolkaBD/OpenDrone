//! Drawing the flight: the Map from its solid parts, a grid on a Test Map's
//! floor, the Quad from its collision shapes, and the FPV Camera bolted to
//! its frame ([`crate::camera`] makes its picture).
//!
//! The Simulation counts in 64-bit numbers, east, north and up from the
//! Map's origin; the picture is drawn in 32-bit numbers, in Bevy's axes:
//! east is +x, up is +y and north is −z. A Quad's body axes (forward, left,
//! up) turn into Bevy's the same way, so forward is +x, up +y and left −z.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::camera::Exposure;
use bevy::camera::visibility::NoCpuCulling;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use opendrone_maths::{Attitude, Vec3 as Vec3d};
use opendrone_physics::{DUCT_RING_SEGMENTS, PROP_DISC_THICKNESS};
use opendrone_sim::{MapShape, QuadShape};

use crate::camera::controls::CameraSettings;
use crate::camera::look::VideoLook;
use crate::camera::{EdgeDelay, QuadPoses, drawn_moment, flat_camera};
use crate::map_drawing::{MaterialLibrary, part_mesh};
use crate::set_up::{FlightPlan, repo_folder};
use crate::stepping::Flight;

/// A point in the Map (east, north, up, in metres) in Bevy's axes.
pub fn point(v: Vec3d) -> Vec3 {
    Vec3::new(v.x as f32, v.z as f32, -v.y as f32)
}

/// A size along a shape's own axes (front to back, side to side, top to
/// bottom) in Bevy's axes.
fn size(v: Vec3d) -> Vec3 {
    Vec3::new(v.x as f32, v.z as f32, v.y as f32)
}

/// An attitude (from body axes to east, north and up) as a turn in Bevy's
/// axes.
pub fn turn(attitude: Attitude) -> Quat {
    let [w, x, y, z] = attitude.quaternion();
    Quat::from_xyzw(x as f32, z as f32, -y as f32, w as f32).normalize()
}

/// The Quad as it's drawn: where the Simulation puts it, between its last
/// two ticks.
#[derive(Component)]
pub struct QuadDrawing;

/// Draws the flight in the game's window.
pub struct DrawingPlugin;

impl Plugin for DrawingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<View>()
            .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.9)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 600.0,
                ..default()
            })
            .add_systems(Startup, (draw_the_map, draw_the_quad, set_up_the_grid))
            .add_systems(
                RunFixedMainLoop,
                place_the_quad.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            )
            .add_systems(Update, draw_the_grid);
    }
}

/// The grid's lines sit on the floor, so they're drawn a little nearer the
/// camera than they are, or the floor would hide them at a low angle.
fn set_up_the_grid(mut gizmos: ResMut<GizmoConfigStore>) {
    let (config, _) = gizmos.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -0.0002;
    config.line.width = 1.5;
}

/// The Map's own parts, drawn from its `.glb`: every part with its material
/// from the library ([`MaterialLibrary`]), the sun with shadows and a sky's
/// worth of fill light. A Test Map has no `.glb`, and is drawn from its solid
/// parts instead.
fn draw_the_map_from_its_file(
    plan: &FlightPlan,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    let library = MaterialLibrary::read(&repo_folder());
    let mut by_name: BTreeMap<String, Handle<StandardMaterial>> = BTreeMap::new();
    for part in &plan.map.drawing {
        let material = by_name
            .entry(part.material.clone())
            .or_insert_with(|| materials.add(library.material(&part.material, part.colour, images)))
            .clone();
        commands.spawn((
            Mesh3d(meshes.add(part_mesh(part))),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Name::new(part.name.clone()),
        ));
    }
    info!(
        "drew {} on {} parts, {} of its {} materials with their pictures",
        plan.map.name,
        plan.map.drawing.len(),
        by_name.len().min(library.textured()),
        by_name.len()
    );
    // Sunny early afternoon: the sun about 50° up, from the south-south-east,
    // with shadows, and a bright sky's fill light (about 3 stops down in the
    // shade, as a clear day's is).
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.75, 0.82, 1.0),
        brightness: 3000.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 100_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.1,
            maximum_distance: 160.0,
            first_cascade_far_bound: 6.0,
            ..default()
        }
        .build(),
        // Light travels away from the sun: north-north-west and down.
        Transform::IDENTITY.looking_to(point(Vec3d::new(-0.38, 0.52, -0.77)).normalize(), Vec3::Y),
    ));
}

fn draw_the_map(
    plan: Res<FlightPlan>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !plan.map.drawing.is_empty() {
        return draw_the_map_from_its_file(
            &plan,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
        );
    }
    let surface = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.42, 0.4),
        perceptual_roughness: 0.9,
        ..default()
    });
    for shape in &plan.map.shapes {
        let (mesh, transform) = match shape {
            MapShape::Box {
                centre,
                size: box_size,
                attitude,
            } => {
                let s = size(*box_size);
                (
                    Mesh::from(Cuboid::new(s.x, s.y, s.z)),
                    Transform::from_translation(point(*centre)).with_rotation(turn(*attitude)),
                )
            }
            MapShape::Convex { corners, triangles }
            | MapShape::TriangleMesh { corners, triangles } => {
                (triangle_mesh(corners, triangles), Transform::IDENTITY)
            }
        };
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(surface.clone()),
            transform,
        ));
    }
    // The sun.
    commands.spawn((
        DirectionalLight {
            // Direct sun, 100,000 lux, which Exposure::SUNLIGHT (ev100 15)
            // is set for: the Map's baked light arrives with its ticket.
            illuminance: 100_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(-0.4, -1.0, -0.6), Vec3::Y),
    ));
}

/// Triangles between corners, each with its own flat face.
fn triangle_mesh(corners: &[Vec3d], triangles: &[[u32; 3]]) -> Mesh {
    let positions: Vec<[f32; 3]> = triangles
        .iter()
        .flatten()
        .map(|&i| point(corners[i as usize]).to_array())
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.compute_flat_normals();
    mesh
}

/// A Test Map's floor gets a grid: a line every metre, and a brighter one
/// every 10 m, over its whole 200 m.
fn draw_the_grid(plan: Res<FlightPlan>, mut gizmos: Gizmos) {
    if !plan.map.id.starts_with("test/") {
        return;
    }
    let floor = Isometry3d::new(Vec3::new(0.0, 0.002, 0.0), Quat::from_rotation_x(FRAC_PI_2));
    gizmos.grid(
        floor,
        UVec2::splat(200),
        Vec2::splat(1.0),
        Color::srgba(0.9, 0.9, 0.9, 0.35),
    );
    gizmos.grid(
        Isometry3d::new(Vec3::new(0.0, 0.003, 0.0), floor.rotation),
        UVec2::splat(20),
        Vec2::splat(10.0),
        Color::srgb(1.0, 1.0, 1.0),
    );
}

/// The Quad from its collision shapes, as the physics builds them (the body,
/// the pack, a prop disc per motor and, on a whoop, the duct rings), with
/// the FPV Camera at the Quad's camera position and Camera Tilt.
fn draw_the_quad(
    plan: Res<FlightPlan>,
    flight: NonSend<Flight>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    view: Res<View>,
) {
    let shape = &plan.quad.parameters.shape;
    let frame = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
        perceptual_roughness: 0.6,
        ..default()
    });
    let pack = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.65, 0.1),
        ..default()
    });
    let props = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.25, 0.27, 0.3),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let (_, state) = flight.drawn_between();
    let mut quad = commands.spawn((
        QuadDrawing,
        Transform::from_translation(point(state.position)).with_rotation(turn(state.attitude)),
        Visibility::default(),
    ));
    quad.with_children(|parts| {
        let body = size(shape.body);
        parts.spawn((
            Mesh3d(meshes.add(Cuboid::new(body.x, body.y, body.z))),
            MeshMaterial3d(frame.clone()),
            Transform::IDENTITY,
        ));
        let pack_size = size(shape.pack);
        parts.spawn((
            Mesh3d(meshes.add(Cuboid::new(pack_size.x, pack_size.y, pack_size.z))),
            MeshMaterial3d(pack),
            Transform::from_translation(point(Vec3d::new(0.0, 0.0, shape.pack_height))),
        ));
        let disc = meshes.add(Cylinder::new(
            (shape.prop_diameter / 2.0) as f32,
            PROP_DISC_THICKNESS as f32,
        ));
        for centre in motor_positions(shape) {
            // Bevy's cylinders stand along their own up axis, so a disc lies
            // flat in the props' plane as it is.
            parts.spawn((
                Mesh3d(disc.clone()),
                MeshMaterial3d(props.clone()),
                Transform::from_translation(point(centre)),
            ));
            if let Some(rings) = shape.duct_rings {
                let inside = rings.inside_diameter / 2.0;
                let outside = inside + rings.wall;
                let middle = inside + rings.wall / 2.0;
                let step = std::f64::consts::TAU / f64::from(DUCT_RING_SEGMENTS);
                let length = 2.0 * outside * (step / 2.0).tan();
                let segment = meshes.add(Cuboid::new(
                    rings.wall as f32,
                    rings.height as f32,
                    length as f32,
                ));
                for k in 0..DUCT_RING_SEGMENTS {
                    let angle = step * f64::from(k);
                    let at = centre + Vec3d::new(middle * angle.cos(), middle * angle.sin(), 0.0);
                    parts.spawn((
                        Mesh3d(segment.clone()),
                        MeshMaterial3d(frame.clone()),
                        Transform::from_translation(point(at))
                            .with_rotation(Quat::from_rotation_y(angle as f32)),
                    ));
                }
            }
        }
        if *view == View::Fpv {
            // The FPV Camera: its place and tilt are set each frame from the
            // Quad's definition and the pilot's Camera Tilt.
            parts.spawn((flat_camera(), Transform::IDENTITY));
        }
    });
    if *view == View::Chase {
        commands.spawn((
            ChaseCamera,
            Camera3d::default(),
            NoCpuCulling,
            Exposure::SUNLIGHT,
            projection(0.05),
            Transform::IDENTITY,
        ));
    }
}

/// The chase view's lens, a plain 90° top to bottom, with its nearest drawn
/// distance in metres.
fn projection(near: f32) -> Projection {
    Projection::Perspective(PerspectiveProjection {
        fov: 90f32.to_radians(),
        near,
        far: 1000.0,
        ..default()
    })
}

/// Which camera draws the picture.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// The FPV Camera bolted to the Quad's frame at its Camera Tilt, through
    /// its Lens and Video Look: what the pilot flies with.
    #[default]
    Fpv,
    /// A developer view: a camera that follows a little behind and above
    /// the Quad, level, to watch it from outside.
    Chase,
}

/// The chase view's camera.
#[derive(Component)]
pub struct ChaseCamera;

/// Where each motor sits, in body axes, in Betaflight's motor order: half the
/// diagonal out from the centre at 45°, in the props' plane, as the physics
/// places them.
fn motor_positions(shape: &QuadShape) -> [Vec3d; 4] {
    let a = shape.diagonal / 2.0 * std::f64::consts::FRAC_1_SQRT_2;
    let z = shape.rotor_height;
    [
        Vec3d::new(-a, -a, z),
        Vec3d::new(a, -a, z),
        Vec3d::new(-a, a, z),
        Vec3d::new(a, a, z),
    ]
}

/// Places the Quad where the pilot's picture shows it: between the last two
/// ticks' states, by how far the game's clock is into the next tick, and in
/// Digital a little further back, where it was when the picture left it
/// (the delay is in the picture only: the Simulation never knows).
#[allow(clippy::too_many_arguments)]
fn place_the_quad(
    flight: NonSend<Flight>,
    plan: Res<FlightPlan>,
    fixed: Res<Time<Fixed>>,
    poses: Res<QuadPoses>,
    settings: Res<CameraSettings>,
    edge_delay: Res<EdgeDelay>,
    view: Res<View>,
    mut quads: Query<&mut Transform, (With<QuadDrawing>, Without<ChaseCamera>)>,
    mut chase: Query<&mut Transform, (With<ChaseCamera>, Without<QuadDrawing>)>,
) {
    // The chase view is a developer's: it never shows the picture late.
    let look = match *view {
        View::Fpv => settings.look,
        View::Chase => VideoLook::Analog,
    };
    let shown = poses.0.shown(drawn_moment(&fixed), look, edge_delay.0);
    let (position, rotation) = match shown {
        Some(pose) => (pose.position, pose.rotation),
        None => {
            // Before the first tick there is no history: the starting state.
            let along = fixed.overstep_fraction_f64().clamp(0.0, 1.0);
            let (before, after) = flight.drawn_between();
            (
                point(before.position + (after.position - before.position) * along),
                turn(before.attitude).slerp(turn(after.attitude), along as f32),
            )
        }
    };
    for mut transform in &mut quads {
        transform.translation = position;
        transform.rotation = rotation;
    }
    // The chase view: behind the Quad's heading and above it, ten frame
    // diagonals away, looking at it.
    let distance = 10.0 * plan.quad.parameters.shape.diagonal as f32;
    let forward = rotation * Vec3::X;
    let level = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::X);
    for mut camera in &mut chase {
        *camera =
            Transform::from_translation(position - level * distance + Vec3::Y * 0.4 * distance)
                .looking_at(position, Vec3::Y);
    }
}
