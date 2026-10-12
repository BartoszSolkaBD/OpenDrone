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
//!   on a Test Map's floor, and the FPV Camera on the Quad's frame;
//! - [`camera`]: the FPV Camera (#66): the Quad's fisheye Lens, the Analog and
//!   Digital Video Looks, Auto-exposure, Camera Tilt and FOV in flight, and
//!   Digital's 15 ms late picture, all outside the Simulation;
//! - [`status`]: what the pilot needs to know, in the window's title;
//! - [`developer`]: commands that draw into an image instead of the window,
//!   to save the picture or measure what the camera costs a frame;
//! - [`reference`](mod@reference): the reference check, a Scenario flown through the game's
//!   stepping, which must match the headless runner's fingerprint.
//!
//! Rendering isn't pipelined and VSync is off, for the least delay from the
//! sticks to the picture (#28 §7).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0021]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md

pub mod camera;
pub mod clock;
pub mod developer;
pub mod drawing;
pub mod map_drawing;
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

use crate::camera::CameraPlugin;
use crate::drawing::DrawingPlugin;
use crate::pilot::{KeyboardPlugin, Pilot, PilotPlugin};
use crate::set_up::FlightPlan;
use crate::status::StatusPlugin;
use crate::stepping::{Flight, SteppingPlugin};

pub use crate::developer::{measure_camera_cost, screenshot_after};

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
        CameraPlugin,
        StatusPlugin,
    ))
    .insert_resource(plan)
    .insert_non_send(flight)
    .insert_resource(pilot);
    Ok(app)
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
