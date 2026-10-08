//! The Scenario runner: headless, used in CI.
//!
//! An edge crate ([ADR-0003]): it reads Scenarios and Input Tracks, and writes
//! Results and `.bbl` files. It uses the Simulation, the Flight Controller, the
//! Test Pilot, `opendrone-pack` and `opendrone-blackbox`.
//!
//! It runs all four kinds: Flight Scenarios fly our Flight Controller and
//! the physics together, the pilot's sticks entering as Flight Inputs;
//! Physics and Thrust Stand Scenarios script the motors in its place (on the
//! thrust stand the Quad is held still); Flight Controller Scenarios run the
//! Flight Controller alone, fed a Timeline of sticks and sensor readings or a
//! table of cases. For each Scenario in `scenarios/` it:
//!
//! 1. reads the file with the shared unit list, refusing any starting state
//!    that leaves an item out (ADR-0002) and naming the file and line of every
//!    problem ([`read_scenario`]);
//! 2. builds the Simulation (or the Flight Controller alone) from the
//!    starting state and the Packs, turns the sticks in percent into
//!    whole-number Channels, and steps it at the physics rate up to the last
//!    moment the file mentions,
//!    measuring every Expectation at its moment or over its stretch, plus the
//!    automatic "no broken numbers" check ([`run()`]); an Expectation that
//!    compares with another run gets that run too, the same Scenario with a
//!    starting-state item or two changed;
//! 3. runs it a second time, and checks the whole state's fingerprint agrees
//!    after every step (the repeat check);
//! 4. writes, or checks, `<name>.results.toml` beside the Scenario
//!    ([`results_text`]), and can write the fingerprint after every step for
//!    the agreement check across computers ([`agreement`]).
//!
//! The command is `cargo scenarios` (see `main.rs`). How to read a Scenario
//! and its Results: `docs/verification/reading-a-scenario.md`.
//!
//! Every Scenario starts with `format = N` ([`SCENARIO_FORMAT`]). A new
//! starting-state item comes with a step in [`SCENARIO_STEPS`] that
//! `cargo xtask migrate` runs over every Scenario ([`Repo::files_to_migrate`]).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

pub mod agreement;
mod format;
mod measure;
mod rates;
mod read;
mod results;
mod run;

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::{Packs, Problems};

pub use format::{SCENARIO_FORMAT, SCENARIO_STEPS};
pub use measure::{Axis, Event, Measure, Sample, Term};
pub use opendrone_sim::StartingMotors;
pub use rates::{AxisRates, Rates, RatesType, ThrottleLimitType};
pub use read::{
    Assists, Basis, BasisKind, Case, Compared, Comparison, Expectation, Expecting, FlightMode,
    InputDevice, Inputs, Kind, Named, OtherRun, PilotChanges, PilotEntry, Scenario, Start,
    StartingFlightController, Statistic, Stick, When, read_scenario,
};
pub use results::{fingerprints_text, read_fingerprints, results_text};
pub use run::{Measured, Outcome, Received, run};

/// The ending of every Results file, beside its Scenario.
pub const RESULTS_ENDING: &str = ".results.toml";

/// The repo's folders the runner reads.
#[derive(Clone, Debug)]
pub struct Repo {
    pub root: PathBuf,
}

/// One Scenario file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScenarioFile {
    /// Its path inside `scenarios/`, with `/` between folders and without
    /// `.toml`, such as `physics/free-fall-is-exactly-g`.
    pub relative: String,
    pub path: PathBuf,
}

impl ScenarioFile {
    /// How reports and problems name it, such as
    /// `scenarios/physics/free-fall-is-exactly-g.toml`.
    pub fn label(&self) -> String {
        format!("scenarios/{}.toml", self.relative)
    }

    /// Where its Results file goes: beside it, as `<name>.results.toml`.
    pub fn results_path(&self) -> PathBuf {
        self.path.with_file_name(format!(
            "{}{RESULTS_ENDING}",
            self.relative.rsplit('/').next().unwrap_or(&self.relative)
        ))
    }
}

impl Repo {
    /// The repo that holds `folder`: the nearest folder at or above it with
    /// both `scenarios/` and `packs/`.
    pub fn around(folder: &Path) -> Option<Repo> {
        folder
            .ancestors()
            .find(|f| f.join("scenarios").is_dir() && f.join("packs").is_dir())
            .map(|root| Repo {
                root: root.to_path_buf(),
            })
    }

    pub fn scenarios_folder(&self) -> PathBuf {
        self.root.join("scenarios")
    }

    /// The built-in Pack and the other Packs in `packs/`, with the Test Quads
    /// in `scenarios/test-quads/`.
    pub fn packs(&self) -> Result<Packs, Problems> {
        Ok(
            Packs::open(&self.root.join("packs"), "packs")?.with_test_quads(
                &self.scenarios_folder().join("test-quads"),
                "scenarios/test-quads",
            ),
        )
    }

    /// Every Scenario in `scenarios/`, in order of their paths. Test Quads and
    /// Results files aren't Scenarios.
    pub fn scenario_files(&self) -> std::io::Result<Vec<ScenarioFile>> {
        let mut files = Vec::new();
        walk(&self.scenarios_folder(), "", &mut |relative, path| {
            if let Some(name) = relative.strip_suffix(".toml")
                && !relative.ends_with(RESULTS_ENDING)
                && !relative.starts_with("test-quads/")
            {
                files.push(ScenarioFile {
                    relative: name.to_string(),
                    path: path.to_path_buf(),
                });
            }
        })?;
        files.sort();
        Ok(files)
    }

    /// Every Results file in `scenarios/` with no Scenario beside it.
    pub fn orphaned_results(&self) -> std::io::Result<Vec<String>> {
        let mut orphans = Vec::new();
        walk(&self.scenarios_folder(), "", &mut |relative, path| {
            if let Some(name) = relative.strip_suffix(RESULTS_ENDING)
                && !path
                    .with_file_name(format!("{}.toml", name.rsplit('/').next().unwrap_or(name)))
                    .is_file()
            {
                orphans.push(format!("scenarios/{relative}"));
            }
        })?;
        orphans.sort();
        Ok(orphans)
    }

    /// The Scenario file at `path`, which must be inside `scenarios/`.
    pub fn scenario_file(&self, path: &Path) -> Option<ScenarioFile> {
        let path = fs::canonicalize(path).ok()?;
        let folder = fs::canonicalize(self.scenarios_folder()).ok()?;
        let inside = path.strip_prefix(&folder).ok()?;
        let parts: Vec<String> = inside
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let relative = parts.join("/");
        Some(ScenarioFile {
            relative: relative.strip_suffix(".toml")?.to_string(),
            path,
        })
    }
}

/// Calls `found` with every file in `folder` and the folders inside it, in
/// name order, with its path from `folder` (`/` between folders).
pub(crate) fn walk(
    folder: &Path,
    prefix: &str,
    found: &mut dyn FnMut(&str, &Path),
) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(folder)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let relative = format!("{prefix}{name}");
        if path.is_dir() {
            walk(&path, &format!("{relative}/"), found)?;
        } else {
            found(&relative, &path);
        }
    }
    Ok(())
}

/// What happened to one Scenario: a line per check, each passed or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub label: String,
    /// The Scenario's name, once it could be read.
    pub name: Option<String>,
    pub checks: Vec<(bool, String)>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|(passed, _)| *passed)
    }

    fn check(&mut self, passed: bool, line: impl Into<String>) {
        self.checks.push((passed, line.into()));
    }
}

/// What to do with the Results file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultsFile {
    /// Write it (`cargo scenarios run`).
    Write,
    /// Check the committed one is up to date, and change nothing (CI).
    Check,
}

/// Runs one Scenario twice (the repeat check), reports every Expectation and
/// the automatic check, then writes or checks its Results file. With
/// `fingerprints`, it also writes the fingerprint after every step to
/// `<fingerprints>/<relative>.fingerprints`, for the agreement check.
pub fn run_one(
    file: &ScenarioFile,
    packs: &Packs,
    results: ResultsFile,
    fingerprints: Option<&Path>,
) -> Report {
    let mut report = Report {
        label: file.label(),
        name: None,
        checks: Vec::new(),
    };
    let problems = |report: &mut Report, problems: Problems| {
        for problem in problems.0 {
            report.check(false, problem.to_string());
        }
    };
    let text = match fs::read_to_string(&file.path) {
        Ok(text) => text,
        Err(error) => {
            report.check(false, format!("{} can't be read: {error}", file.label()));
            return report;
        }
    };
    let scenario = match read_scenario(&file.label(), &text) {
        Ok(scenario) => scenario,
        Err(found) => {
            problems(&mut report, found);
            return report;
        }
    };
    report.name = Some(scenario.name.clone());
    let (first, second) = match (run(&scenario, packs), run(&scenario, packs)) {
        (Ok(first), Ok(second)) => (first, second),
        (Err(found), _) | (_, Err(found)) => {
            problems(&mut report, found);
            return report;
        }
    };

    for e in &first.expectations {
        report.check(
            e.passed,
            format!(
                "{}: measured {}, expected {} (line {}, {})",
                e.description,
                e.measured,
                e.expected,
                e.line,
                e.basis.word()
            ),
        );
    }
    match &first.first_broken_number {
        None => report.check(true, "no broken numbers"),
        Some(broken) => report.check(false, format!("a broken number appeared, {broken}")),
    }
    report.check_repeat(&first, &second);

    let fresh = results_text(&scenario, &first);
    let results_path = file.results_path();
    let results_label = format!("scenarios/{}{RESULTS_ENDING}", file.relative);
    match results {
        ResultsFile::Write => match fs::write(&results_path, &fresh) {
            Ok(()) => report.check(true, format!("wrote {results_label}")),
            Err(error) => report.check(false, format!("can't write {results_label}: {error}")),
        },
        ResultsFile::Check => {
            let committed = fs::read_to_string(&results_path).unwrap_or_default();
            if committed == fresh {
                report.check(true, format!("{results_label} is up to date"));
            } else {
                report.check(
                    false,
                    format!(
                        "{results_label} is out of date: run `cargo scenarios run` and commit it. {}",
                        first_difference(&committed, &fresh)
                    ),
                );
            }
        }
    }

    if let Some(folder) = fingerprints {
        let path = folder.join(format!(
            "{}{}",
            file.relative,
            agreement::FINGERPRINTS_ENDING
        ));
        let written = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&path, fingerprints_text(&file.relative, &first)));
        if let Err(error) = written {
            report.check(
                false,
                format!(
                    "can't write the fingerprints to {}: {error}",
                    path.display()
                ),
            );
        }
    }
    report
}

impl Report {
    fn check_repeat(&mut self, first: &Outcome, second: &Outcome) {
        let split = first
            .step_fingerprints
            .iter()
            .zip(&second.step_fingerprints)
            .position(|(a, b)| a != b);
        match split {
            None if first.step_fingerprints.len() == second.step_fingerprints.len() => self.check(
                true,
                format!(
                    "the repeat check: a second run agrees after every step (run fingerprint {})",
                    first.run_fingerprint()
                ),
            ),
            None => self.check(
                false,
                "the repeat check: the second run took a different number of steps",
            ),
            Some(step) => self.check(
                false,
                format!(
                    "the repeat check: the second run split from the first at step {step} ({} s)",
                    step as f64 / f64::from(first.physics_rate)
                ),
            ),
        }
    }
}

/// Where two texts first differ, in words, for an out-of-date Results file.
fn first_difference(committed: &str, fresh: &str) -> String {
    if committed.is_empty() {
        return "There's no committed Results file yet.".to_string();
    }
    let mut committed_lines = committed.lines();
    let mut fresh_lines = fresh.lines();
    for line in 1.. {
        match (committed_lines.next(), fresh_lines.next()) {
            (Some(a), Some(b)) if a == b => continue,
            (Some(a), Some(b)) => {
                return format!("Line {line} says {a:?}, but this run gives {b:?}.");
            }
            (Some(a), None) => {
                return format!("Line {line} says {a:?}, but this run ends before it.");
            }
            (None, Some(b)) => {
                return format!("The file ends before line {line}, which this run gives as {b:?}.");
            }
            (None, None) => break,
        }
    }
    "Only the line endings differ.".to_string()
}
