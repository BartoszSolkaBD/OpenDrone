//! The game, and the only crate that uses Bevy ([ADR-0021]: Bevy 0.20.0).
//!
//! It uses every other OpenDrone crate except `xtask`. It turns the computer's
//! clock into Simulation Time and hands the Simulation Flight Inputs. It is one
//! of the two crates that may use `unsafe` ([ADR-0003]).
//!
//! What the game is made of, so far:
//!
//! - [`stepping`]: the Simulation stepped in Bevy's fixed timestep at 1 kHz,
//!   8 steps a tick in one system, with catch-up stepping;
//! - [`clock`]: the computer's clock turned into Simulation Time, standing
//!   still while paused;
//! - [`pilot`]: the Input Devices' Channels as Flight Inputs, the Flying
//!   Input Device, and the Actions Reset and Pause; in a menu, the Input
//!   Devices move the cursor instead;
//! - [`screens`] (#68): First Launch, the Hub, the loading screen, the Pause
//!   Menu and a small Settings page, on Bevy's UI widgets, usable with a
//!   Radio alone;
//! - [`settings_file`] (#68): the one settings file, saved the moment a
//!   setting changes, with the pilot's settings in it;
//! - [`set_up`]: the flight set up from the Packs and the [`settings`]
//!   (the pilot's own, from the settings file);
//! - [`sound`](mod@sound): the Quad's sound On the Quad, on the sound's own
//!   thread, silent when there's no sound device (#74);
//! - [`drawing`]: the Map and the Quad drawn from their solid parts, a grid
//!   on a Test Map's floor, and the FPV Camera on the Quad's frame;
//! - [`rotors`] (#319): the Quad's rotors turning in the picture, from each
//!   motor's speed, as blades that fade into a blurred disc as they speed up;
//! - [`sky`] (#315): the sky behind a baked Map, made from its `[light]`;
//! - [`camera`]: the FPV Camera (#66): the Quad's fisheye Lens, the Analog and
//!   Digital Video Looks, Auto-exposure, Camera Tilt and FOV in flight, and
//!   Digital's 15 ms late picture, all outside the Simulation;
//! - [`status`]: what the pilot needs to know, in the window's title;
//! - [`developer`]: commands that draw into an image instead of the window,
//!   to save the picture or measure what the camera costs a frame;
//! - [`map_views`] (#291): a developer command that draws a Map as the game
//!   does, from any camera, into PNG files, for an agent to look at;
//! - [`reference`](mod@reference): the reference check, a Scenario flown through the game's
//!   stepping, which must match the headless runner's fingerprint;
//! - [`watch`]: a developer command that opens a Flight or Thrust Stand
//!   Scenario in the FPV view, with pause (#73);
//! - [`save`]: the Save Flight Action (#73), which writes the flight as a
//!   Scenario and an Input Track the headless runner flies again to the same
//!   fingerprint.
//!
//! Rendering isn't pipelined and VSync is off, for the least delay from the
//! sticks to the picture (#28 §7).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0021]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md

pub mod camera;
pub mod clock;
pub mod decals;
pub mod developer;
pub mod drawing;
pub mod grass;
pub mod light_atlas;
pub mod map_drawing;
pub mod map_views;
pub mod pilot;
pub mod reference;
pub mod repeat_breaking;
pub mod rotors;
pub mod save;
pub mod screens;
pub mod set_up;
pub mod settings;
pub mod settings_file;
pub mod sky;
pub mod sound;
pub mod status;
pub mod stepping;
pub mod surface_look;
pub mod watch;

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::window::PresentMode;

use crate::camera::CameraPlugin;
use crate::drawing::DrawingPlugin;
use crate::pilot::{KeyboardPlugin, Pilot, PilotPlugin};
use crate::rotors::RotorsPlugin;
use crate::screens::{Screens, ScreensPlugin, ViewPlugin};
use crate::set_up::FlightPlan;
use crate::sound::SoundPlugin;
use crate::status::StatusPlugin;
use crate::stepping::{Flight, SteppingPlugin};
use crate::watch::WatchedScenario;

pub use crate::developer::{measure_camera_cost, screenshot_after};
pub use crate::map_views::render_map_views;

/// The game in its window: the flight, the pilot's Input Devices and the
/// keyboard, drawn with no pipelining and no VSync, and the Quad's sound.
pub fn game(plan: FlightPlan, pilot: Pilot) -> Result<App, String> {
    let flight = Flight::new(plan.simulation(None)?)?;
    Ok(window_game(plan, pilot, flight))
}

/// The game in its window with its screens (#68): First Launch on a new
/// install, then the Hub, the loading screen and the Pause Menu. `plan` is the
/// flight the settings start from, made behind the screens until FLY makes
/// it afresh.
pub fn screens_game(plan: FlightPlan, pilot: Pilot, screens: Screens) -> Result<App, String> {
    let flight = Flight::new(plan.simulation(None)?)?;
    let mut app = window_game(plan, pilot.with_screens(), flight);
    app.add_plugins((ScreensPlugin, ViewPlugin))
        .insert_resource(screens);
    Ok(app)
}

/// The game in its window, watching a Scenario (#73): its flight stepped by
/// the game, with nobody flying it. Pause works as in any flight.
pub fn watching_game(watched: WatchedScenario, pilot: Pilot) -> App {
    let WatchedScenario {
        plan,
        flight,
        watching,
    } = watched;
    let mut app = window_game(plan, pilot.watching(), flight);
    app.insert_resource(watching);
    app
}

/// The game's window, systems and resources, flying `flight`.
fn window_game(plan: FlightPlan, pilot: Pilot, flight: Flight) -> App {
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
        RotorsPlugin,
        CameraPlugin,
        StatusPlugin,
        SoundPlugin,
    ))
    .insert_resource(plan)
    .insert_non_send(flight)
    .insert_resource(pilot);
    app
}

/// How long a headless run lasts, in the computer's time.
#[derive(Resource)]
struct HeadlessFor(Duration);

/// The game with no window and no picture: the flight, the Input Devices
/// and the Quad's sound, for `seconds` of the computer's time. CI's smoke
/// run uses it on machines with no screen and no sound device, where the
/// sound runs silent.
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
        SoundPlugin,
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
