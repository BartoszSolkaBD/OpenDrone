//! Watching a Scenario: a developer command that opens a Flight or Thrust
//! Stand Scenario in the game, in the normal FPV view, with pause (#11 §4).
//! It has no menu entry.
//!
//! The Scenario is set up exactly as the headless runner sets it up, and
//! stepped through the game's own stepping, the same as the reference check
//! does, so what is watched is the flight the runner measures. Nobody flies
//! it: no Input Device starts a flight, Reset does nothing, and Pause stops
//! the Simulation as it does in any flight. A Test Map shows its plain grid
//! floor, as in every flight. Where the Scenario ends the Quad stays as it
//! was at its last moment.

use std::path::Path;

use bevy::prelude::*;
use opendrone_input::profile::FlightMode;
use opendrone_pack::Packs;
use opendrone_scenario::{FlightMode as ScenarioFlightMode, Kind, Scenario, simulation};
use opendrone_sim::PacketRate;

use crate::reference::{LoadedScenario, load};
use crate::set_up::FlightPlan;
use crate::settings::Settings;
use crate::stepping::Flight;

/// The Scenario being watched, for the window's title.
#[derive(Resource, Clone, Debug)]
pub struct Watching {
    /// The Scenario's name, as its file says it.
    pub name: String,
    /// How the runner labels it, such as `scenarios/physics/free-fall.toml`.
    pub label: String,
}

/// A Scenario ready to watch: what the game's set-up needs.
pub struct WatchedScenario {
    pub plan: FlightPlan,
    pub flight: Flight,
    pub watching: Watching,
}

/// Reads the Scenario at `path` (inside the repo's `scenarios/` folder) and
/// sets up its flight, to be stepped by the game.
pub fn open_scenario(repo: &Path, path: &Path) -> Result<WatchedScenario, String> {
    let LoadedScenario {
        label,
        scenario,
        packs,
        ..
    } = load(repo, path)?;
    watch_scenario(&label, &scenario, &packs)
}

/// Sets up the flight of a Scenario already read, with its Input Track if it
/// plays one, as the headless runner sets it up. `label` names it in
/// problems.
pub fn watch_scenario(
    label: &str,
    scenario: &Scenario,
    packs: &Packs,
) -> Result<WatchedScenario, String> {
    let start = &scenario.start;
    match start.kind {
        Kind::Flight | Kind::ThrustStand => {}
        Kind::FlightController | Kind::Physics => {
            return Err(format!(
                "{label} isn't a Flight or Thrust Stand Scenario, and only those can be watched"
            ));
        }
    }
    let Some(map) = &start.map else {
        return Err(format!("{label} has no Map to draw"));
    };
    let quad = packs
        .quad(&start.quad.id)
        .map_err(|p| format!("{label}: can't use the Quad \"{}\":\n{p}", start.quad.id))?;
    let map = packs
        .map(&map.id)
        .map_err(|p| format!("{label}: can't use the Map \"{}\":\n{p}", map.id))?;
    let settings = Settings {
        quad: quad.id.clone(),
        map: map.id.clone(),
        physics_rate: start.physics_rate,
        packet_rate: PacketRate::from_hz(start.packet_rate)
            .ok_or_else(|| format!("{label}: {} Hz isn't a Packet Rate", start.packet_rate))?,
        rates: start.rates.clone(),
        flight_mode: match start.flight_mode {
            ScenarioFlightMode::Acro => FlightMode::Acro,
            ScenarioFlightMode::Horizon => FlightMode::Horizon,
            ScenarioFlightMode::Angle => FlightMode::Angle,
        },
        input_smoothing: start.assists.input_smoothing,
        endless_battery: start.assists.endless_battery,
        auto_arm: start.assists.auto_arm,
        random_seed: start.random_seed,
    };
    let (sim, inputs) = simulation(scenario, packs).map_err(|p| p.to_string())?;
    let flight = Flight::scripted(sim, inputs)?;
    Ok(WatchedScenario {
        plan: FlightPlan {
            quad,
            map,
            settings,
        },
        flight,
        watching: Watching {
            name: scenario.name.clone(),
            label: label.to_string(),
        },
    })
}
