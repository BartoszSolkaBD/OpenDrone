//! The agreement check: every computer must give the same fingerprint after
//! every step of every Scenario (ADR-0001). On a mismatch it names the
//! Scenario and the first step where the computers split.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::results::read_fingerprints;

/// The ending of every per-step fingerprint file.
pub const FINGERPRINTS_ENDING: &str = ".fingerprints";

/// One computer's fingerprint files, by Scenario (its path inside
/// `scenarios/`, without `.toml`).
#[derive(Clone, Debug)]
pub struct Computer {
    pub name: String,
    pub files: BTreeMap<String, String>,
}

impl Computer {
    /// Reads every fingerprint file in a folder and the folders inside it.
    /// The computer is named after the folder, without a `fingerprints-`
    /// prefix.
    pub fn read(folder: &Path) -> Result<Computer, String> {
        let folder_name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| folder.display().to_string());
        let name = folder_name
            .strip_prefix("fingerprints-")
            .unwrap_or(&folder_name)
            .to_string();
        let mut files = BTreeMap::new();
        read_folder(folder, "", &mut files).map_err(|error| {
            format!(
                "{name}: can't read its fingerprints in {}: {error}; did its Scenario run finish?",
                folder.display()
            )
        })?;
        Ok(Computer { name, files })
    }
}

fn read_folder(
    folder: &Path,
    prefix: &str,
    files: &mut BTreeMap<String, String>,
) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(folder)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            read_folder(&path, &format!("{prefix}{name}/"), files)?;
        } else if let Some(scenario) = name.strip_suffix(FINGERPRINTS_ENDING) {
            files.insert(format!("{prefix}{scenario}"), fs::read_to_string(&path)?);
        }
    }
    Ok(())
}

/// What the agreement check found: one line per Scenario, and whether every
/// computer agreed on every one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agreement {
    pub lines: Vec<String>,
    pub agreed: bool,
}

/// Compares the computers' fingerprints, Scenario by Scenario and step by
/// step.
pub fn compare(computers: &[Computer]) -> Agreement {
    let mut lines = Vec::new();
    let mut agreed = true;
    if computers.len() < 2 {
        return Agreement {
            lines: vec![
                "the agreement check needs the fingerprints of at least two computers".into(),
            ],
            agreed: false,
        };
    }
    for computer in computers {
        if computer.files.is_empty() {
            agreed = false;
            lines.push(format!(
                "{}: no fingerprints at all; did its Scenario run finish?",
                computer.name
            ));
        }
    }
    let scenarios: BTreeSet<&String> = computers.iter().flat_map(|c| c.files.keys()).collect();
    for scenario in scenarios {
        match compare_one(scenario, computers) {
            Ok(line) => lines.push(line),
            Err(line) => {
                agreed = false;
                lines.push(line);
            }
        }
    }
    Agreement { lines, agreed }
}

fn compare_one(scenario: &str, computers: &[Computer]) -> Result<String, String> {
    let mut runs = Vec::new();
    for computer in computers {
        let Some(text) = computer.files.get(scenario) else {
            return Err(format!(
                "{scenario}: {} has no fingerprints for it; did its run of this Scenario finish?",
                computer.name
            ));
        };
        let Some((rate, steps)) = read_fingerprints(text) else {
            return Err(format!(
                "{scenario}: {}'s fingerprint file can't be read",
                computer.name
            ));
        };
        runs.push((computer.name.as_str(), rate, steps));
    }
    let rate = runs[0].1;
    if runs.iter().any(|(_, r, _)| *r != rate) {
        return Err(format!(
            "{scenario}: the computers ran at different physics rates"
        ));
    }
    let longest = runs.iter().map(|(_, _, s)| s.len()).max().unwrap_or(0);
    for step in 0..longest {
        let first = runs[0].2.get(step);
        if runs.iter().all(|(_, _, steps)| steps.get(step) == first) {
            continue;
        }
        let gave: Vec<String> = runs
            .iter()
            .map(|(name, _, steps)| match steps.get(step) {
                Some(fingerprint) => format!("{name} {fingerprint}"),
                None => format!("{name} had stopped"),
            })
            .collect();
        return Err(format!(
            "{scenario}: the computers split at step {step} ({} s): {}",
            step as f64 / f64::from(rate),
            gave.join(", ")
        ));
    }
    Ok(format!(
        "{scenario}: all {} computers agree at the start and after each of its {} steps",
        runs.len(),
        longest.saturating_sub(1)
    ))
}
