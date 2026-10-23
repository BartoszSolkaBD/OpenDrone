//! The FPV Camera (#66): the pilot's only view, seen through the Quad's
//! fisheye Lens in an Analog or a Digital Video Look.
//!
//! - [`lens`]: where the picture sits in the window, the Lens's fisheye curve,
//!   and how wide the flat render underneath must reach (ADR-0009).
//! - [`look`]: the two Video Looks and each camera's limits: Dynamic Range,
//!   lines, sharpness, the grade, and how Auto-exposure adapts.
//! - [`exposure`]: Auto-exposure's limits as a compensation curve for Bevy's
//!   own, and the mask that meters the flat render by how much of the picture
//!   each part becomes.
//! - [`pose`]: where the Quad was, so Digital's picture can be drawn 15 ms
//!   late. The delay is in the picture only.
//! - [`controls`]: Camera Tilt and FOV, 1° per press and repeating, kept for
//!   each Quad, and the brief readout.
//! - [`params`] and `render`: what the GPU is told, and the camera's two
//!   steps: a mip chain of the flat render and one merged pass that warps it
//!   into the Lens and makes the Video Look. Its shaders are WESL.
//!
//! Camera settings never enter the Simulation, so none of this can change a
//! flight, and a Scenario never sees it.

pub mod controls;
pub mod exposure;
pub mod lens;
pub mod look;
pub mod params;
pub mod pose;
pub mod render;

use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoCpuCulling;
use bevy::camera::{Exposure, RenderTarget};
use bevy::image::ImageSampler;
use bevy::post_process::auto_exposure::{
    AutoExposure, AutoExposureCompensationCurve, AutoExposurePlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::ManualTextureViews;
use bevy::render::view::Tonemapping;
use bevy::window::PrimaryWindow;

use crate::drawing::{View, WorldDrawing, point, turn};
use crate::pilot::Pilot;
use crate::screens::model::InputMode;
use crate::set_up::FlightPlan;
use crate::stepping::{Flight, TICK_HZ, step_the_flight};

use self::controls::{CameraSettings, READOUT_FOR, Repeater, camera_inputs};
use self::exposure::{METERING_RANGE, compensation_points, metering_mask};
use self::lens::{Lens, Picture};
use self::look::{VideoLook, camera_limits};
use self::params::FrameInputs;
use self::pose::{Pose, PoseHistory};
use self::render::{FpvDisplay, FpvFrame, FpvRenderPlugin, SOURCE_VIEW, SourceTexture};

/// The nearest distance the flat render draws, in metres, measured along the
/// camera's axis. Its near plane is a flat rectangle, as wide and tall as the
/// flat render reaches, so at the picture's corners it reaches nearer a wall
/// than straight ahead does, by 2.6 mm at the Whoop 65's default Camera Tilt
/// and FOV. A wall the Quad is pressed against has to stay beyond all of it,
/// or the picture shows through the wall: the Whoop 65's camera sits behind
/// its front duct rings, which touch the wall first in a level hit, and its
/// camera housing, a collision part behind the lens, touches first in a
/// nose-dive. The checks in `tests/camera.rs` take the near plane's four
/// corners.
pub const NEAR: f32 = 0.001;

/// How far the flat render draws, in metres.
const FAR: f32 = 3000.0;

/// The camera that draws the Map once, into the flat render.
#[derive(Component)]
pub struct FpvCamera;

/// What the picture is drawn into, in pixels: the window, or the picture a
/// developer command saves.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub size: UVec2,
    /// A developer command set it, so the window doesn't.
    pub fixed: bool,
}

/// The brief readout after a Camera Tilt or FOV change, until the sim's
/// notices arrive with the OSD ticket and show it at the top of the picture.
#[derive(Resource, Clone, Debug, Default)]
pub struct CameraReadout {
    current: Option<(String, Duration)>,
}

impl CameraReadout {
    /// The readout's text while it shows, at a moment of the computer's time.
    pub fn text(&self, now: Duration) -> Option<&str> {
        self.current
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(text, _)| text.as_str())
    }
}

/// Where the Quad was after each tick, which Digital's late picture is drawn
/// from.
#[derive(Resource, Default)]
pub struct QuadPoses(pub PoseHistory);

/// How much longer than its usual 15 ms Digital's picture is late, in
/// seconds, near the edge of the Video Signal's range: the hook the Video
/// Signal ticket (#67) sets. It is at most 20 ms and zero until then.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EdgeDelay(pub f64);

/// The camera's pieces: its render, its settings and their keys, and the
/// history behind Digital's delay.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((AutoExposurePlugin, FpvRenderPlugin))
            .init_resource::<CameraSettings>()
            .init_resource::<CameraReadout>()
            .init_resource::<QuadPoses>()
            .init_resource::<EdgeDelay>()
            .init_resource::<Output>()
            .insert_resource(Repeating(Repeater::new()))
            .add_systems(Startup, spawn_the_display_camera)
            .add_systems(FixedUpdate, record_the_pose.after(step_the_flight))
            .add_systems(
                Update,
                (read_the_camera_keys, track_the_output, set_up_the_camera).chain(),
            );
    }
}

#[derive(Resource)]
struct Repeating(Repeater);

/// The Quad's pose after each tick, to draw it late in Digital.
fn record_the_pose(flight: NonSend<Flight>, fixed: Res<Time<Fixed>>, mut poses: ResMut<QuadPoses>) {
    let (before, after) = flight.drawn_between();
    let now = fixed.elapsed_secs_f64();
    let pose = |state: &opendrone_sim::QuadState| Pose {
        position: point(state.position),
        rotation: turn(state.attitude),
    };
    if flight.jumped_in_last_tick() {
        poses.0.restart(now, pose(after));
        return;
    }
    if poses.0.is_empty() {
        poses.0.push(now - 1.0 / f64::from(TICK_HZ), pose(before));
    }
    poses.0.push(now, pose(after));
}

/// The moment the picture is drawn for, in the Simulation's time: one tick
/// behind the last tick, by how far the game's clock is into the next, as the
/// Quad has always been drawn between the last two ticks.
pub fn drawn_moment(fixed: &Time<Fixed>) -> f64 {
    fixed.elapsed_secs_f64() - 1.0 / f64::from(TICK_HZ)
        + fixed.overstep_fraction_f64().clamp(0.0, 1.0) / f64::from(TICK_HZ)
}

/// The display camera, which runs the merged pass into the window. It draws
/// nothing of its own.
fn spawn_the_display_camera(mut commands: Commands, view: Res<View>) {
    if *view != View::Fpv {
        return;
    }
    commands.spawn((
        FpvDisplay,
        Camera2d,
        // The screens draw on it too, whatever it draws into.
        bevy::ui::IsDefaultUiCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Msaa::Off,
        Tonemapping::None,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
    ));
}

/// The camera that draws the Map, bolted to the Quad's frame: draws into the
/// flat render with Bevy's own Auto-exposure, which this plugin sets up each
/// frame.
pub fn flat_camera() -> impl Bundle {
    (
        FpvCamera,
        Camera3d::default(),
        // Off, and with no target yet, until the flat render exists: the
        // renderer starts a few frames after the game, and a camera whose
        // target is missing is an error.
        Camera {
            order: 0,
            is_active: false,
            ..default()
        },
        NoCpuCulling,
        // Bevy compresses the light (Reinhard) after Auto-exposure; the
        // merged pass undoes that and applies the camera's own response.
        Tonemapping::Reinhard,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        Exposure::SUNLIGHT,
        Msaa::Sample4,
        Projection::Perspective(PerspectiveProjection {
            fov: 1.5,
            near: NEAR,
            far: FAR,
            ..default()
        }),
    )
}

/// Camera Tilt and FOV from the keys and the Input Devices' buttons: a press
/// acts at once, even a key tap that ended before the frame did, and a hold
/// acts again after a second and then every 200 ms.
fn read_the_camera_keys(
    keys: Res<ButtonInput<KeyCode>>,
    pilot: Res<Pilot>,
    plan: Res<FlightPlan>,
    real: Res<Time<Real>>,
    mut repeating: ResMut<Repeating>,
    mut settings: ResMut<CameraSettings>,
    mut readout: ResMut<CameraReadout>,
) {
    // In a menu the arrows move the cursor, so the camera keeps still.
    if pilot.input_mode() != InputMode::Flight {
        return;
    }
    let (held, tapped) = camera_inputs(&keys, pilot.actions_held(), pilot.actions_tapped());
    let now = real.elapsed();
    for action in repeating.0.update_with_taps(now, &held, &tapped) {
        if let Some(changed) = settings.press(&plan.quad.id, &plan.quad.camera, action) {
            readout.current = Some((changed.text, now + READOUT_FOR));
        }
    }
}

/// How big the picture is drawn: the window, unless a developer command fixed it.
fn track_the_output(mut output: ResMut<Output>, windows: Query<&Window, With<PrimaryWindow>>) {
    if output.fixed {
        return;
    }
    if let Ok(window) = windows.single() {
        let size = UVec2::new(window.physical_width(), window.physical_height());
        if output.size != size {
            output.size = size;
        }
    }
}

/// What each frame sets on the camera that draws the Map.
type FlatCameraParts = (
    Entity,
    &'static mut Camera,
    &'static mut Transform,
    &'static mut Projection,
    Option<&'static AutoExposure>,
);

/// What the cached Auto-exposure assets were made for.
#[derive(Default, PartialEq)]
struct ExposureKey {
    look: VideoLook,
    fov_halves: i32,
    aspect: Option<lens::Aspect>,
    picture: UVec2,
}

#[derive(Default)]
struct ExposureAssets {
    key: Option<ExposureKey>,
    curve: Option<Handle<AutoExposureCompensationCurve>>,
    mask: Option<Handle<Image>>,
    curve_look: Option<VideoLook>,
}

/// Each frame: the camera at the Quad's Camera Tilt, the flat render at the
/// size the picture needs, the Auto-exposure, and what the merged pass is
/// told.
#[allow(clippy::too_many_arguments)]
fn set_up_the_camera(
    plan: Res<FlightPlan>,
    settings: Res<CameraSettings>,
    world: Res<WorldDrawing>,
    output: Res<Output>,
    real: Res<Time<Real>>,
    device: Option<Res<RenderDevice>>,
    mut source: ResMut<SourceTexture>,
    mut manual: ResMut<ManualTextureViews>,
    mut frame: ResMut<FpvFrame>,
    mut cameras: Query<FlatCameraParts, With<FpvCamera>>,
    mut curves: ResMut<Assets<AutoExposureCompensationCurve>>,
    mut images: ResMut<Assets<Image>>,
    mut assets: Local<ExposureAssets>,
    mut frames: Local<u32>,
    mut commands: Commands,
) {
    if output.size.x == 0 || output.size.y == 0 {
        return;
    }
    // With no world drawn (a menu on the Hub), there is no picture to show.
    if !world.is_drawn() {
        frame.source = None;
        return;
    }
    let Some(device) = device else { return };
    let quad = settings.quad(&plan.quad.id, &plan.quad.camera);
    let look = settings.look;
    let picture = Picture::in_window(output.size, settings.aspect);
    let lens = Lens::new(quad.fov as f32, look.lens_curve());
    let reach = lens.flat_reach(&picture);
    let size = picture.size().max(UVec2::splat(16));
    source.ensure(&device, &mut manual, size);

    // The Auto-exposure's mask follows what the picture shows; its curve
    // follows the Look.
    let key = ExposureKey {
        look,
        fov_halves: (quad.fov * 2.0).round() as i32,
        aspect: Some(settings.aspect),
        picture: size,
    };
    if assets.key.as_ref() != Some(&key) {
        let (mask_size, weights) = metering_mask(&picture, &lens, reach);
        let mut mask = Image::new(
            Extent3d {
                width: mask_size.x,
                height: mask_size.y,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            weights,
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        mask.sampler = ImageSampler::linear();
        assets.mask = Some(images.add(mask));
        assets.key = Some(key);
    }
    if assets.curve_look != Some(look) {
        let points = compensation_points(&look.auto_exposure());
        match AutoExposureCompensationCurve::from_curve(
            bevy::curve::cubic_splines::LinearSpline::new(points),
        ) {
            Ok(curve) => {
                assets.curve = Some(curves.add(curve));
                assets.curve_look = Some(look);
            }
            Err(error) => warn!("Auto-exposure's compensation curve: {error}"),
        }
    }
    let limits = look.auto_exposure();
    let want = AutoExposure {
        range: METERING_RANGE.0..=METERING_RANGE.1,
        filter: 0.10..=0.90,
        speed_brighten: limits.speed_brighten,
        speed_darken: limits.speed_darken,
        exponential_transition_distance: 1.5,
        metering_mask: assets.mask.clone().unwrap_or_default(),
        compensation_curve: assets.curve.clone().unwrap_or_default(),
    };

    let tilt = quad.tilt.to_radians() as f32;
    let at = point(opendrone_maths::Vec3::new(
        plan.quad.camera.position[0],
        plan.quad.camera.position[1],
        plan.quad.camera.position[2],
    ));
    let fov = 2.0 * reach.y.atan();
    for (entity, mut camera, mut transform, mut projection, auto_exposure) in &mut cameras {
        if !camera.is_active {
            camera.is_active = true;
            commands
                .entity(entity)
                .insert(RenderTarget::TextureView(SOURCE_VIEW));
        }
        let wanted = Transform::from_translation(at).looking_to(
            Vec3::new(tilt.cos(), tilt.sin(), 0.0),
            Vec3::new(-tilt.sin(), tilt.cos(), 0.0),
        );
        transform.set_if_neq(wanted);
        if let Projection::Perspective(perspective) = projection.as_mut()
            && (perspective.fov - fov).abs() > 1e-6
        {
            perspective.fov = fov;
        }
        match auto_exposure {
            Some(current) => {
                if current.speed_brighten != want.speed_brighten
                    || current.speed_darken != want.speed_darken
                    || current.metering_mask != want.metering_mask
                    || current.compensation_curve != want.compensation_curve
                {
                    commands.entity(entity).insert(want.clone());
                }
            }
            None => {
                commands.entity(entity).insert(want.clone());
            }
        }
    }

    *frames = (*frames + 1) % 100_000;
    frame.params = FrameInputs {
        picture,
        lens,
        reach,
        source: size,
        look,
        limits: camera_limits(look, &plan.quad.camera),
        time: (real.elapsed_secs_f64() % 1000.0) as f32,
        frame: *frames,
    }
    .params();
    frame.source = source.views().cloned();
}
