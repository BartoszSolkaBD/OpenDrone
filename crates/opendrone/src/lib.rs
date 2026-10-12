//! The game, and the only crate that uses Bevy ([ADR-0021]: Bevy 0.20.0).
//!
//! It uses every other OpenDrone crate except `xtask`. It turns the computer's
//! clock into Simulation Time and hands the Simulation Flight Inputs. It is one
//! of the two crates that may use `unsafe` ([ADR-0003]).
//!
//! What the first flight needs, so far (#62):
//!
//! - [`stepping`]: the Simulation stepped in Bevy's fixed timestep at 1 kHz,
//!   8 steps a tick in one system, with catch-up stepping;
//! - [`clock`]: the computer's clock turned into Simulation Time, standing
//!   still while paused;
//! - [`pilot`]: the Input Devices' Channels as Flight Inputs, the Flying
//!   Input Device, and the Actions Reset and Pause;
//! - [`set_up`]: the flight set up from the built-in Pack and the
//!   [`settings`], fixed until the screens ticket;
//! - [`drawing`]: the Map and the Quad drawn from their solid parts, a grid
//!   on a Test Map's floor, and a placeholder camera on the Quad's frame;
//! - [`status`]: what the pilot needs to know, in the window's title;
//! - [`reference`](mod@reference): the reference check, a Scenario flown through the game's
//!   stepping, which must match the headless runner's fingerprint.
//!
//! Rendering isn't pipelined and VSync is off, for the least delay from the
//! sticks to the picture (#28 §7).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0021]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md

pub mod clock;
pub mod drawing;
pub mod pilot;
pub mod reference;
pub mod set_up;
pub mod settings;
pub mod status;
pub mod stepping;

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::window::PresentMode;

use crate::drawing::DrawingPlugin;
use crate::pilot::{KeyboardPlugin, Pilot, PilotPlugin};
use crate::set_up::FlightPlan;
use crate::status::StatusPlugin;
use crate::stepping::{Flight, SteppingPlugin};

/// The game in its window: the flight, the pilot's Input Devices and the
/// keyboard, drawn with no pipelining and no VSync.
pub fn game(plan: FlightPlan, pilot: Pilot) -> Result<App, String> {
    let flight = Flight::new(plan.simulation(None)?)?;
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "OpenDrone".to_string(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
            .build()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins((
        SteppingPlugin,
        PilotPlugin,
        KeyboardPlugin,
        DrawingPlugin,
        StatusPlugin,
    ))
    .insert_resource(plan)
    .insert_non_send(flight)
    .insert_resource(pilot);
    Ok(app)
}

/// A developer command: saves the camera's picture to a BMP file once the
/// game has run for `seconds`, then quits. Agents use it to look at the
/// picture on a machine they can't watch. The camera draws into a picture
/// of its own, 1280 × 720, instead of the window, so it works with the
/// screen locked too.
pub fn screenshot_after(app: &mut App, seconds: f64, path: std::path::PathBuf) {
    app.insert_resource(ScreenshotAt {
        after: Duration::from_secs_f64(seconds),
        path,
        picture: None,
        taken: false,
    })
    .add_systems(PostStartup, draw_into_a_picture)
    .add_systems(Update, take_the_screenshot);
}

#[derive(Resource)]
struct ScreenshotAt {
    after: Duration,
    path: std::path::PathBuf,
    picture: Option<Handle<Image>>,
    taken: bool,
}

fn draw_into_a_picture(
    mut at: ResMut<ScreenshotAt>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
) {
    use bevy::camera::RenderTarget;
    use bevy::render::render_resource::TextureFormat;
    let picture = images.add(Image::new_target_texture(
        1280,
        720,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(RenderTarget::Image(picture.clone().into()));
    }
    at.picture = Some(picture);
}

fn take_the_screenshot(
    mut at: ResMut<ScreenshotAt>,
    real: Res<Time<Real>>,
    mut commands: Commands,
) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    if at.taken || real.elapsed() < at.after {
        return;
    }
    let Some(picture) = at.picture.clone() else {
        return;
    };
    at.taken = true;
    let path = at.path.clone();
    commands.spawn(Screenshot::image(picture)).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            let saved = captured
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|e| e.to_string())
                .and_then(|picture| {
                    let rgb = picture.to_rgb8();
                    write_bmp(&path, rgb.width(), rgb.height(), rgb.as_raw())
                        .map_err(|e| e.to_string())
                });
            match saved {
                Ok(()) => {
                    info!("saved the picture to {}", path.display());
                    exit.write(AppExit::Success);
                }
                Err(problem) => {
                    error!("the picture can't be saved: {problem}");
                    exit.write(AppExit::error());
                }
            }
        },
    );
}

/// Writes 8-bit RGB rows, top first, as an uncompressed 24-bit BMP file.
fn write_bmp(path: &std::path::Path, width: u32, height: u32, rgb: &[u8]) -> std::io::Result<()> {
    let row = (width as usize * 3).div_ceil(4) * 4;
    let size = 54 + row * height as usize;
    let mut file = Vec::with_capacity(size);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&(size as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&(width as i32).to_le_bytes());
    file.extend_from_slice(&(height as i32).to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&24u16.to_le_bytes());
    file.extend_from_slice(&[0; 24]);
    for y in (0..height as usize).rev() {
        let start = y * width as usize * 3;
        for pixel in rgb[start..start + width as usize * 3].chunks(3) {
            file.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        file.resize(file.len() + row - width as usize * 3, 0);
    }
    std::fs::write(path, file)
}

/// How long a headless run lasts, in the computer's time.
#[derive(Resource)]
struct HeadlessFor(Duration);

/// The game with no window, no picture and no sound: the flight and the
/// Input Devices only, for `seconds` of the computer's time. CI's smoke run
/// uses it on machines with no screen and no sound device.
pub fn headless(plan: FlightPlan, pilot: Pilot, seconds: f64) -> Result<App, String> {
    let flight = Flight::new(plan.simulation(None)?)?;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        ))),
        LogPlugin::default(),
        SteppingPlugin,
        PilotPlugin,
    ))
    .insert_resource(plan)
    .insert_non_send(flight)
    .insert_resource(pilot)
    .insert_resource(HeadlessFor(Duration::from_secs_f64(seconds)))
    .add_systems(Last, end_the_headless_run);
    Ok(app)
}

fn end_the_headless_run(
    run_for: Res<HeadlessFor>,
    real: Res<Time<Real>>,
    flight: NonSend<Flight>,
    mut exit: MessageWriter<AppExit>,
) {
    if real.elapsed() < run_for.0 {
        return;
    }
    let sim = flight.simulation();
    let steps = sim.time().ticks();
    let simulated = sim.time().seconds(sim.physics_rate());
    info!(
        "headless run: {steps} Simulation steps ({simulated:.3} s of Simulation Time) in {:.3} s",
        real.elapsed().as_secs_f64()
    );
    if steps > 0 && sim.is_finite() {
        exit.write(AppExit::Success);
    } else {
        error!("the Simulation didn't step");
        exit.write(AppExit::error());
    }
}
