//! `cargo xtask migrate`: the format migration tool (#60, ADR-0002,
//! ADR-0011).
//!
//! - `migrate` lists every step, newest last, with the files it rewrites.
//! - `migrate <step>` runs one named step over every Scenario, or over every
//!   Pack file, Test Quad and Test Map: each file in the format the step upgrades gets
//!   the step's item, with the value that keeps today's behaviour, and its
//!   `format` line bumped, keeping every comment and the layout. A file
//!   already in the newer format is left alone, so running it twice changes
//!   nothing. If any file can't take the step, nothing is written.
//!
//! The steps live with the files they rewrite: `SCENARIO_STEPS` in
//! `opendrone-scenario` and `PACK_STEPS` in `opendrone_pack::migration`,
//! where the Pack reader runs the same steps in memory over a pilot's older
//! Pack. This only finds the step by name, and reports.

use std::process::ExitCode;

use opendrone_pack::document::FORMAT;
use opendrone_pack::migration::{Family, PACK_STEPS, Step, migrate};
use opendrone_scenario::{Repo, SCENARIO_FORMAT, SCENARIO_STEPS};

const USAGE: &str = "Usage: cargo xtask migrate [<step>]";

pub fn run(args: &[String]) -> ExitCode {
    let steps: Vec<&Step> = SCENARIO_STEPS.iter().chain(PACK_STEPS).collect();
    match args {
        [] => {
            list(&steps);
            ExitCode::SUCCESS
        }
        [name] if !name.starts_with('-') => match steps.iter().find(|step| step.name == name) {
            Some(step) => run_step(step),
            None => {
                eprintln!("There's no format migration step called \"{name}\".");
                list(&steps);
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn list(steps: &[&Step]) {
    println!(
        "Scenarios are format {SCENARIO_FORMAT}, and Pack files, Test Quads and Test Maps are format {FORMAT}."
    );
    if steps.is_empty() {
        println!(
            "There are no format migration steps yet. A pull request that adds a starting-state item, or changes a Pack file's format, adds one: see docs/format-migration.md."
        );
        return;
    }
    println!("The steps, each run with `cargo xtask migrate <step>`:");
    for step in steps {
        println!(
            "- {}: {}, format {} to {}. {}",
            step.name,
            step.family.words(),
            step.from,
            step.from + 1,
            step.says
        );
    }
}

fn run_step(step: &Step) -> ExitCode {
    let repo = match std::env::current_dir()
        .ok()
        .and_then(|here| Repo::around(&here))
    {
        Some(repo) => repo,
        None => {
            eprintln!(
                "Run this inside the OpenDrone repo: no folder here or above holds both scenarios/ and packs/."
            );
            return ExitCode::from(2);
        }
    };
    let migration = repo
        .files_to_migrate(step.family)
        .and_then(|files| migrate(step, &files));
    let migration = match migration {
        Ok(migration) => migration,
        Err(problems) => {
            println!(
                "`{}` can't run, so nothing was written. Its problems:",
                step.name
            );
            for problem in &problems.0 {
                println!("- {problem}");
            }
            return ExitCode::FAILURE;
        }
    };
    if let Err(problems) = migration.write() {
        println!("Some files couldn't be written:");
        for problem in &problems.0 {
            println!("- {problem}");
        }
        return ExitCode::FAILURE;
    }
    println!(
        "`{}`, format {} to {}: {}",
        step.name,
        step.from,
        step.from + 1,
        step.says
    );
    println!(
        "Rewrote {} file(s); {} already format {}, left alone.",
        migration.rewritten.len(),
        migration.already.len(),
        step.from + 1
    );
    for (file, _) in &migration.rewritten {
        println!("- {}", file.label);
    }
    println!(
        "{}",
        match step.family {
            Family::Scenarios =>
                "Next: `cargo scenarios check`. The step keeps today's behaviour, so every Results file must stay the same.",
            Family::Packs =>
                "Next: `cargo xtask packs` and `cargo scenarios check`. The step keeps today's behaviour, so every Pack must pass and every Results file must stay the same.",
        }
    );
    ExitCode::SUCCESS
}
