//! What the pilot needs to know, in the window's title until the OSD and the
//! screens arrive: the Quad, the Flying Input Device or how to pick one,
//! whether the Quad is armed or what blocks arming, Failsafe and Pause, that
//! there's no sound device (which the Hub will say instead), the brief readout
//! after a Camera Tilt or FOV change, and what Save Flight said. When a
//! Scenario is being watched, its name stands in for the Input Device, and the
//! title says where it is in its run.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use opendrone_input::Lost;
use opendrone_sound::Device;

use crate::camera::CameraReadout;
use crate::pilot::Pilot;
use crate::set_up::FlightPlan;
use crate::sound::QuadSoundPlaying;
use crate::stepping::Flight;
use crate::watch::Watching;

/// Keeps the window's title up to date.
pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_the_status);
    }
}

#[allow(clippy::too_many_arguments)]
fn show_the_status(
    plan: Res<FlightPlan>,
    pilot: Res<Pilot>,
    flight: NonSend<Flight>,
    game_clock: Res<Time<Virtual>>,
    sound: Option<Res<QuadSoundPlaying>>,
    readout: Res<CameraReadout>,
    real: Res<Time<Real>>,
    watching: Option<Res<Watching>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let mut parts = vec![format!("OpenDrone: {}", plan.quad.name)];
    if let Some(watching) = &watching {
        let sim = flight.simulation();
        let seconds = sim.time().seconds(sim.physics_rate());
        parts.push(format!("watching: {}", watching.name));
        parts.push(if flight.finished() {
            format!("Scenario ended at {seconds:.3} s")
        } else {
            format!("{seconds:.1} s")
        });
    } else {
        parts.push(match (pilot.flying_name(), pilot.flying_lost()) {
            (Some(name), None) => name.to_string(),
            (Some(name), Some(Lost::Unplugged)) => format!("{name} unplugged"),
            (Some(name), Some(Lost::Silent)) => format!("{name} silent"),
            (None, _) if pilot.devices_plugged_in() > 0 => {
                "move a stick on the Input Device you want to fly with".to_string()
            }
            (None, _) => match &pilot.thread_problem {
                Some(problem) => format!("no Input Devices: {problem}"),
                None => "plug in an Input Device".to_string(),
            },
        });
    }
    let output = flight.simulation().quad_output(0);
    if let Some(record) = output.flight_controller {
        if record.armed {
            parts.push("armed".to_string());
        } else if record.arming_blocks.any() {
            parts.push(format!(
                "disarmed, arming blocked: {}",
                record.arming_blocks.names().join(" ")
            ));
        } else {
            parts.push("disarmed".to_string());
        }
        if record.failsafe.active {
            parts.push("FAILSAFE".to_string());
        }
    }
    if game_clock.is_paused() {
        parts.push("paused (Esc goes on)".to_string());
    }
    if sound.is_none_or(|s| matches!(s.device(), Device::None(_))) {
        parts.push("no sound device: running silent".to_string());
    }
    if let Some(text) = readout.text(real.elapsed()) {
        parts.push(text.to_string());
    }
    if let Some(text) = pilot.notice(&real) {
        parts.push(text.to_string());
    }
    let title = parts.join("  |  ");
    for mut window in &mut windows {
        if window.title != title {
            window.title = title.clone();
        }
    }
}
