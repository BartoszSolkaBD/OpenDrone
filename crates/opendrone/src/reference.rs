//! The reference check: a Scenario flown through the game's own stepping
//! must give the same fingerprint as the headless Scenario runner (ADR-0003,
//! "Checks in CI"). The game build compiles the core together with Bevy, so
//! this is what shows that Bevy's being there changes nothing in a flight.
//!
//! The Scenario's Simulation and Flight Inputs are set up exactly as the
//! runner sets them up ([`opendrone_scenario::simulation`]). Then Bevy's
//! fixed timestep steps it, 8 steps a tick, as in the game, through frames of
//! uneven lengths, so the ticks arrive in bursts of different sizes. The
//! fingerprint after every step goes into the run's fingerprint, which must
//! equal the `run` fingerprint in the Scenario's Results file, written by
//! the headless runner, which CI checks on every computer.

use std::fs;
use std::path::Path;
use std::time::Duration;

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use opendrone_maths::Fingerprint;
use opendrone_pack::{Packs, Problems};
use opendrone_scenario::{Repo, Scenario, ScenarioFile, read_scenario, simulation};

use crate::stepping::{Flight, SteppingPlugin};

/// The lengths of the frames the check runs through, in milliseconds, over
/// and over: from shorter than one tick to many ticks at once.
pub const FRAME_LENGTHS_MS: [u64; 5] = [3, 17, 8, 41, 1];

/// One Scenario flown through the game's stepping.
#[derive(Clone, Debug)]
pub struct ReferenceFlight {
    /// The Scenario, as the runner labels it.
    pub label: String,
    /// How many Simulation steps it took, and how many each tick ran.
    pub steps: u64,
    pub steps_per_tick: u64,
    /// How many frames Bevy ran.
    pub frames: u64,
    /// The run's fingerprint through the game's stepping.
    pub game: Fingerprint,
    /// The run's fingerprint in its Results file, from the headless runner.
    pub runner: Fingerprint,
}

impl ReferenceFlight {
    pub fn agrees(&self) -> bool {
        self.game == self.runner
    }
}

/// A Scenario read from the repo, with its Input Track if it plays one, and
/// the Packs (and Test Quads) it names its Quad and Map in.
pub struct LoadedScenario {
    pub file: ScenarioFile,
    pub label: String,
    pub scenario: Scenario,
    pub packs: Packs,
}

/// Reads the Scenario at `path` (inside the repo's `scenarios/` folder), its
/// Input Track and the Packs, as the headless runner does.
pub fn load(repo: &Path, path: &Path) -> Result<LoadedScenario, String> {
    let repo = Repo {
        root: repo.to_path_buf(),
    };
    let file = repo
        .scenario_file(path)
        .ok_or_else(|| format!("{} isn't a Scenario in scenarios/", path.display()))?;
    let label = file.label();
    let text = fs::read_to_string(&file.path).map_err(|e| format!("{label}: {e}"))?;
    let mut scenario = read_scenario(&label, &text).map_err(|p| p.to_string())?;
    if let Some(name) = scenario.input_track().map(str::to_string) {
        let track = fs::read_to_string(file.path.with_file_name(&name))
            .map_err(|e| format!("{label}: its Input Track {name} can't be read: {e}"))?;
        scenario
            .read_input_track(&name, &track)
            .map_err(|p: Problems| p.to_string())?;
    }
    let packs = repo.packs().map_err(|p| p.to_string())?;
    Ok(LoadedScenario {
        file,
        label,
        scenario,
        packs,
    })
}

/// Flies the Scenario at `path` (inside the repo's `scenarios/` folder)
/// through the game's stepping.
pub fn fly(repo: &Path, path: &Path) -> Result<ReferenceFlight, String> {
    let LoadedScenario {
        file,
        label,
        scenario,
        packs,
    } = load(repo, path)?;
    let (sim, inputs) = simulation(&scenario, &packs).map_err(|p| p.to_string())?;
    let flight = Flight::scripted(sim, inputs)?;
    let runner = runner_fingerprint(&file.results_path())?;

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin, SteppingPlugin))
        .insert_non_send(flight);
    let mut frames: u64 = 0;
    while !app.world().non_send::<Flight>().finished() {
        let length = FRAME_LENGTHS_MS[(frames % FRAME_LENGTHS_MS.len() as u64) as usize];
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            length,
        )));
        app.update();
        frames += 1;
    }
    let flight = app.world().non_send::<Flight>();
    Ok(ReferenceFlight {
        label,
        steps: flight.simulation().time().ticks(),
        steps_per_tick: flight.steps_per_tick(),
        frames,
        game: flight
            .run_fingerprint()
            .expect("a scripted flight keeps its fingerprints"),
        runner,
    })
}

/// The `run` fingerprint in a Results file.
fn runner_fingerprint(results: &Path) -> Result<Fingerprint, String> {
    let text = fs::read_to_string(results)
        .map_err(|e| format!("{} can't be read: {e}", results.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", results.display()))?;
    table
        .get("fingerprints")
        .and_then(|f| f.get("run"))
        .and_then(|run| run.as_str())
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        .map(Fingerprint)
        .ok_or_else(|| format!("{} has no [fingerprints] run", results.display()))
}
