//! The Quad's rotors turning in the picture (#319), from each motor's
//! simulated speed. Drawing only: nothing here enters the Simulation.
//!
//! Each rotor is drawn two ways at once, cross-faded by how fast it turns:
//!
//! - **blades**, a few flat blades that turn the way the Quad definition's
//!   props turn, by the angle the motor's speed adds each drawn frame;
//! - a **blurred disc**, a faint translucent disc, a little denser toward the
//!   blade tips, which is what a real propeller shows at flight speeds.
//!
//! A drawn frame samples the turning, so a rotor that turns a good share of
//! the gap between two blades in one frame looks like it turns slowly, or
//! backwards (the wagon-wheel, or stroboscope, effect). So the blades are
//! drawn only while a blade moves a small share of that gap in one frame, and
//! fade out before the share gets large: by the time the speed could be
//! misread, only the disc is left, and a disc looks the same at every angle.
//! The share is measured against the frame's own length, so a slower picture
//! fades its blades out at a lower speed.

use std::f64::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::stepping::Flight;

/// While a blade moves this share of the gap between two blades or less in
/// one drawn frame, the rotor is drawn as blades alone: the turning reads
/// true.
pub const BLADES_ALONE_BELOW: f64 = 0.05;

/// Once a blade moves this share of the gap between two blades in one drawn
/// frame, the rotor is drawn as the blurred disc alone. A quarter is half of
/// what the eye can still follow (at half, a wheel can be taken to turn
/// either way), which leaves room for the fade.
pub const DISC_ALONE_FROM: f64 = 0.25;

/// How far each blade is turned about its own length, in degrees: a real
/// blade is pitched to bite the air, and so shows when it is seen edge-on.
const BLADE_PITCH: f32 = 20.0;

/// How dense the blades are while they are drawn alone, 0 to 1.
const BLADE_OPACITY: f32 = 0.9;

/// How dense the disc is where it is densest, by the blade tips, 0 to 1.
const DISC_OPACITY: f32 = 0.3;

/// The shortest frame the fade is judged by, in seconds: a 240 Hz picture.
const SHORTEST_FRAME: f64 = 1.0 / 240.0;

/// The longest frame the fade is judged by, in seconds: a stall isn't a
/// slower picture.
const LONGEST_FRAME: f64 = 1.0 / 15.0;

/// How much of the gap between two blades a blade moves in one drawn frame:
/// 1 is a whole gap. `speed` is the rotor's, in rad/s (either way round),
/// `frame_time` is the drawn frame's length in seconds.
pub fn gaps_per_frame(speed: f64, frame_time: f64, blades: u32) -> f64 {
    speed.abs() * frame_time * f64::from(blades.max(1)) / TAU
}

/// How much of the rotor is drawn as blades, from 1 (blades alone, the rotor
/// at rest or turning slowly) to 0 (the blurred disc alone, at flight
/// speeds): 1 up to [`BLADES_ALONE_BELOW`] of a gap in a frame, 0 from
/// [`DISC_ALONE_FROM`], and a smooth step between.
pub fn blade_share(speed: f64, frame_time: f64, blades: u32) -> f64 {
    let gaps = gaps_per_frame(speed, frame_time, blades);
    let t = ((gaps - BLADES_ALONE_BELOW) / (DISC_ALONE_FROM - BLADES_ALONE_BELOW)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The angle a rotor is at after turning for `time` seconds at `speed`
/// (rad/s, positive the normal way), from `angle`, in radians counted
/// anticlockwise seen from above, and kept between 0 and a whole turn so it
/// never loses its fine digits. `turning` is the way the prop turns when its
/// motor turns the normal way: +1 anticlockwise, -1 clockwise
/// ([`opendrone_physics::RotorLayout::turning`]).
pub fn turned(angle: f64, turning: f64, speed: f64, time: f64) -> f64 {
    (angle + turning * speed * time).rem_euclid(TAU)
}

/// How opaque the blurred disc is at `radius`, a share of the prop's radius
/// from 0 (the hub) to 1 (the blade tips), as a share of [`DISC_OPACITY`]:
/// thin inside, densest toward the tips where the blades move fastest, and
/// nothing beyond them.
pub fn disc_density(radius: f64) -> f64 {
    if radius >= 1.0 {
        return 0.0;
    }
    let inside = 0.35;
    let ramp = ((radius - 0.15) / 0.65).clamp(0.0, 1.0);
    // A little softer at the very edge, so it isn't a hard ring.
    let edge = ((1.0 - radius) / 0.08).clamp(0.0, 1.0);
    (inside + (1.0 - inside) * ramp) * edge
}

/// One rotor, drawn as turning blades and a blurred disc.
#[derive(Component, Clone, Debug)]
pub struct Rotor {
    /// Which motor drives it, in Betaflight's motor order.
    pub motor: usize,
    /// +1 when the prop turns anticlockwise seen from above with its motor
    /// turning the normal way, -1 clockwise.
    pub turning: f64,
    /// How many blades the prop has.
    pub blades: u32,
    /// Where the blades are, in radians anticlockwise seen from above.
    pub angle: f64,
    /// How much of it is drawn as blades now, 0 to 1.
    pub shown: f64,
    blades_part: Entity,
    disc_part: Entity,
    blade_material: Handle<StandardMaterial>,
    disc_material: Handle<StandardMaterial>,
}

impl Rotor {
    /// Turns the rotor for one drawn frame: `time` seconds of the game's
    /// clock (none while it's paused) at `speed`, judged for a picture whose
    /// frames are `frame_time` seconds apart. Returns the blades' share.
    pub fn step(&mut self, speed: f64, time: f64, frame_time: f64) -> f64 {
        self.angle = turned(self.angle, self.turning, speed, time);
        self.shown = blade_share(speed, frame_time, self.blades);
        self.shown
    }
}

/// A rotor's picture, as the developer's `--render-views` asks for it, in
/// place of the Simulation's motors.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct RotorsShown {
    /// Every motor turns at this speed (rad/s) instead of its own.
    pub speed: Option<f64>,
    /// Frames are judged to be this long (seconds) instead of the drawn
    /// frame's own length, so a picture saved offline is judged as the
    /// pilot's is.
    pub frame_time: Option<f64>,
}

/// Draws the rotors turning, once the Quad is drawn.
pub struct RotorsPlugin;

impl Plugin for RotorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RotorsShown>()
            .add_systems(Update, spin_the_rotors);
    }
}

/// What the parts of a Quad's rotors are drawn from, made once.
pub struct RotorParts {
    blades: Handle<Mesh>,
    disc: Handle<Mesh>,
    disc_picture: Handle<Image>,
    blade_count: u32,
}

impl RotorParts {
    /// The meshes and the disc's picture for a prop of `diameter` metres,
    /// with `blades` blades and a disc `thickness` metres thick.
    pub fn new(
        diameter: f64,
        blades: u32,
        thickness: f64,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
    ) -> RotorParts {
        let radius = (diameter / 2.0) as f32;
        RotorParts {
            blades: meshes.add(blades_mesh(radius, blades)),
            disc: meshes.add(Cylinder::new(radius, thickness as f32)),
            disc_picture: images.add(disc_picture()),
            blade_count: blades,
        }
    }

    /// Spawns one rotor, centred where its parent puts it: turning with its
    /// `motor`, the way `turning` says.
    pub fn spawn(
        &self,
        parent: &mut ChildSpawnerCommands,
        materials: &mut Assets<StandardMaterial>,
        motor: usize,
        turning: f64,
        at: Vec3,
    ) {
        let blade_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.16, 0.16, 0.18, BLADE_OPACITY),
            perceptual_roughness: 0.5,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let disc_material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.0),
            base_color_texture: Some(self.disc_picture.clone()),
            perceptual_roughness: 0.7,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
        let mut rotor = parent.spawn((Transform::from_translation(at), Visibility::default()));
        let (mut blades_part, mut disc_part) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
        rotor.with_children(|parts| {
            // At rest the blades show and the disc doesn't.
            blades_part = parts
                .spawn((
                    Mesh3d(self.blades.clone()),
                    MeshMaterial3d(blade_material.clone()),
                    Transform::IDENTITY,
                ))
                .id();
            disc_part = parts
                .spawn((
                    Mesh3d(self.disc.clone()),
                    MeshMaterial3d(disc_material.clone()),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id();
        });
        rotor.insert(Rotor {
            motor,
            turning,
            blades: self.blade_count,
            angle: 0.0,
            shown: 1.0,
            blades_part,
            disc_part,
            blade_material,
            disc_material,
        });
    }
}

/// `blades` flat blades, each a tapered strip from near the hub to the tip in
/// the props' plane, evenly spread round it.
fn blades_mesh(radius: f32, blades: u32) -> Mesh {
    let (root, tip) = (0.12 * radius, radius);
    let (root_half, tip_half) = (0.05 * radius, 0.1 * radius);
    // Each blade is pitched, as a real one is, so it shows from the side too.
    let pitch = Quat::from_rotation_x(BLADE_PITCH.to_radians());
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for k in 0..blades.max(1) {
        let turn = Quat::from_rotation_y(std::f32::consts::TAU * k as f32 / blades.max(1) as f32);
        let base = positions.len() as u32;
        // Bevy's +x is along the blade, z across it.
        for corner in [
            Vec3::new(root, 0.0, -root_half),
            Vec3::new(root, 0.0, root_half),
            Vec3::new(tip, 0.0, tip_half),
            Vec3::new(tip, 0.0, -tip_half),
        ] {
            positions.push((turn * (pitch * corner)).to_array());
        }
        // Wound to face up; the material draws both sides.
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
    .with_inserted_indices(Indices::U32(indices))
}

/// The blurred disc's picture: a dark grey whose opacity is
/// [`disc_density`] (times [`DISC_OPACITY`]) across the prop's radius, turned the same way round the
/// middle, so it looks the same at every angle.
fn disc_picture() -> Image {
    const SIZE: u32 = 64;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for row in 0..SIZE {
        for column in 0..SIZE {
            let x = (f64::from(column) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
            let y = (f64::from(row) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
            let density = disc_density((x * x + y * y).sqrt());
            data.extend([
                60,
                60,
                66,
                (density * f64::from(DISC_OPACITY) * 255.0).round() as u8,
            ]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Each drawn frame: turns every rotor by its motor's speed and cross-fades
/// its blades into the disc.
#[allow(clippy::too_many_arguments)]
fn spin_the_rotors(
    flight: Option<NonSend<Flight>>,
    shown: Res<RotorsShown>,
    real: Res<Time<Real>>,
    game: Res<Time<Virtual>>,
    mut rotors: Query<(&mut Rotor, &mut Transform)>,
    mut visibilities: Query<&mut Visibility>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smoothed: Local<f64>,
) {
    if rotors.is_empty() {
        return;
    }
    // The fade is judged by how long the picture's frames really are, kept
    // from jumping about by a slow average.
    let measured = real.delta_secs_f64().clamp(SHORTEST_FRAME, LONGEST_FRAME);
    *smoothed = if *smoothed == 0.0 {
        measured
    } else {
        *smoothed + 0.1 * (measured - *smoothed)
    };
    let frame_time = shown.frame_time.unwrap_or(*smoothed);
    let time = game.delta_secs_f64();
    let motors = flight.map(|flight| flight.simulation().quad_output(0).motors);
    for (mut rotor, mut transform) in &mut rotors {
        let speed = shown
            .speed
            .or_else(|| motors.map(|motors| motors[rotor.motor].speed))
            .unwrap_or(0.0);
        let before = rotor.shown;
        let share = rotor.step(speed, time, frame_time);
        transform.rotation = Quat::from_rotation_y(rotor.angle as f32);
        if share == before {
            continue;
        }
        let (blades, disc) = (rotor.blades_part, rotor.disc_part);
        if let Ok(mut visibility) = visibilities.get_mut(blades) {
            *visibility = if share > 0.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if let Ok(mut visibility) = visibilities.get_mut(disc) {
            *visibility = if share < 1.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if let Some(mut material) = materials.get_mut(&rotor.blade_material) {
            material.base_color = material.base_color.with_alpha(BLADE_OPACITY * share as f32);
        }
        if let Some(mut material) = materials.get_mut(&rotor.disc_material) {
            material.base_color = material.base_color.with_alpha(1.0 - share as f32);
        }
    }
}
