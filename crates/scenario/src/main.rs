//! `cargo scenarios`: the Scenario runner's command. See the library's docs
//! and `docs/verification/reading-a-scenario.md`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use opendrone_scenario::agreement::{self, Computer};
use opendrone_scenario::{Repo, Report, ResultsFile, ScenarioFile, run_one};

const USAGE: &str = "\
Usage: cargo scenarios <command>

Commands:
  run [<scenario.toml>...]
      Run every Scenario in scenarios/ (or the ones named), check each
      Expectation, and write <name>.results.toml beside each one.
  check [--fingerprints <folder>]
      What CI runs: run every Scenario twice (the repeat check), check each
      Expectation, and check every Results file is up to date, changing
      nothing. With --fingerprints, write the fingerprint after every step of
      every Scenario into <folder>, for the agreement check.
  agree <folder> <folder> [<folder>...]
      The agreement check: compare the fingerprints that `check
      --fingerprints` wrote on several computers, and name the Scenario and
      the first step where they split.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.split_first() {
        Some((command, rest)) if command == "run" => run(rest),
        Some((command, rest)) if command == "check" => check(rest),
        Some((command, rest)) if command == "agree" => agree(rest),
        _ => Err(USAGE.to_string()),
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn repo() -> Result<Repo, String> {
    let here = std::env::current_dir().map_err(|e| format!("can't tell where this is: {e}"))?;
    Repo::around(&here).ok_or_else(|| {
        "Run this inside the OpenDrone repo: no folder here or above holds both scenarios/ and packs/."
            .to_string()
    })
}

fn run(args: &[String]) -> Result<bool, String> {
    let repo = repo()?;
    let files = if args.is_empty() {
        all_scenarios(&repo)?
    } else {
        args.iter()
            .map(|arg| {
                repo.scenario_file(Path::new(arg))
                    .ok_or_else(|| format!("{arg} isn't a Scenario file inside scenarios/"))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    run_all(&repo, &files, ResultsFile::Write, None)
}

fn check(args: &[String]) -> Result<bool, String> {
    let fingerprints = match args {
        [] => None,
        [flag, folder] if flag == "--fingerprints" => Some(PathBuf::from(folder)),
        _ => return Err(USAGE.to_string()),
    };
    let repo = repo()?;
    let files = all_scenarios(&repo)?;
    let mut passed = run_all(&repo, &files, ResultsFile::Check, fingerprints.as_deref())?;
    let orphans = repo
        .orphaned_results()
        .map_err(|e| format!("can't read scenarios/: {e}"))?;
    for orphan in orphans {
        passed = false;
        println!("FAILED  {orphan} has no Scenario beside it: delete it, or put its Scenario back");
    }
    Ok(passed)
}

fn all_scenarios(repo: &Repo) -> Result<Vec<ScenarioFile>, String> {
    let files = repo
        .scenario_files()
        .map_err(|e| format!("can't read scenarios/: {e}"))?;
    if files.is_empty() {
        return Err("There are no Scenarios in scenarios/.".to_string());
    }
    Ok(files)
}

fn run_all(
    repo: &Repo,
    files: &[ScenarioFile],
    results: ResultsFile,
    fingerprints: Option<&Path>,
) -> Result<bool, String> {
    let packs = repo.packs().map_err(|problems| {
        format!("The Packs can't be read, so no Scenario can run:\n{problems}")
    })?;
    let mut failed = 0;
    for file in files {
        let report = run_one(file, &packs, results, fingerprints);
        print_report(&report);
        if !report.passed() {
            failed += 1;
        }
    }
    let count = files.len();
    let scenarios = if count == 1 { "Scenario" } else { "Scenarios" };
    if failed == 0 {
        println!("All {count} {scenarios} passed.");
    } else {
        println!("{failed} of {count} {scenarios} failed.");
    }
    Ok(failed == 0)
}

fn print_report(report: &Report) {
    match &report.name {
        Some(name) => println!("{}: \"{name}\"", report.label),
        None => println!("{}", report.label),
    }
    for (passed, line) in &report.checks {
        let mark = if *passed { "passed" } else { "FAILED" };
        println!("  {mark}  {line}");
    }
    println!();
}

fn agree(args: &[String]) -> Result<bool, String> {
    if args.len() < 2 {
        return Err(USAGE.to_string());
    }
    let mut computers = Vec::new();
    let mut readable = true;
    for folder in args {
        match Computer::read(Path::new(folder)) {
            Ok(computer) => computers.push(computer),
            Err(message) => {
                readable = false;
                println!("FAILED  {message}");
            }
        }
    }
    let names: Vec<&str> = computers.iter().map(|c| c.name.as_str()).collect();
    println!("Comparing the fingerprints from {}.", names.join(", "));
    let found = agreement::compare(&computers);
    for line in &found.lines {
        println!("  {line}");
    }
    if !readable {
        println!(
            "Some computers' fingerprints are missing, so the agreement check can't pass; see why above."
        );
    } else if found.agreed {
        println!("Every computer agrees on every Scenario, at every step.");
    } else {
        println!("The computers don't agree: the Simulation isn't the same everywhere (ADR-0001).");
    }
    Ok(readable && found.agreed)
}
