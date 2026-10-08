//! Outside libraries, read from `Cargo.lock` on both sides: the new ones with
//! their licences, and Bevy or wgpu moving to a new version series
//! ([ADR-0021]).
//!
//! [ADR-0021]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// One library in `Cargo.lock`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Locked {
    pub name: String,
    pub version: String,
    /// Where it comes from, such as crates.io. OpenDrone's own crates have none.
    pub source: Option<String>,
}

/// Every library in a `Cargo.lock`.
pub fn read_lock(text: &str) -> Vec<Locked> {
    let Ok(lock) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut libraries: Vec<Locked> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| {
            Some(Locked {
                name: package.get("name")?.as_str()?.to_string(),
                version: package.get("version")?.as_str()?.to_string(),
                source: package
                    .get("source")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect();
    libraries.sort();
    libraries
}

/// A library the head uses that the base didn't, by name.
#[derive(Clone, Debug)]
pub struct NewLibrary {
    pub name: String,
    pub version: String,
    /// Its licence, from `cargo metadata`, when that was given.
    pub licence: Option<String>,
}

/// The outside libraries in the head's `Cargo.lock` whose names the base's
/// lacks. A new version of a library already used isn't a new library.
pub fn new_libraries(
    base: &[Locked],
    head: &[Locked],
    metadata: Option<&Value>,
) -> Vec<NewLibrary> {
    let known: BTreeSet<&str> = base.iter().map(|l| l.name.as_str()).collect();
    let licences = licences(metadata);
    let mut seen = BTreeSet::new();
    head.iter()
        .filter(|l| l.source.is_some() && !known.contains(l.name.as_str()))
        .filter(|l| seen.insert((l.name.clone(), l.version.clone())))
        .map(|l| NewLibrary {
            name: l.name.clone(),
            version: l.version.clone(),
            licence: licences.get(&(l.name.clone(), l.version.clone())).cloned(),
        })
        .collect()
}

/// Each library's licence, by name and version, from `cargo metadata`.
fn licences(metadata: Option<&Value>) -> BTreeMap<(String, String), String> {
    let mut licences = BTreeMap::new();
    let packages = metadata
        .and_then(|m| m.get("packages"))
        .and_then(Value::as_array);
    for package in packages.into_iter().flatten() {
        let (Some(name), Some(version)) = (
            package.get("name").and_then(Value::as_str),
            package.get("version").and_then(Value::as_str),
        ) else {
            continue;
        };
        let licence = match (
            package.get("license").and_then(Value::as_str),
            package.get("license_file").and_then(Value::as_str),
        ) {
            (Some(licence), _) => licence.to_string(),
            (None, Some(file)) => format!("in its own file ({file})"),
            (None, None) => "none stated".to_string(),
        };
        licences.insert((name.to_string(), version.to_string()), licence);
    }
    licences
}

/// The libraries whose new version series waits for the maintainer: Bevy's
/// 0.N and, with it, wgpu's major version (ADR-0021).
pub const WATCHED: [&str; 2] = ["bevy", "wgpu"];

/// A watched library moving to a version series the base didn't use.
#[derive(Clone, Debug)]
pub struct SeriesMove {
    pub name: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
}

/// Bevy or wgpu moving to a new series: Bevy 0.20 to 0.21, or wgpu 27 to 28.
/// A patch release, or a release candidate becoming the release, stays in its
/// series, and a library the base didn't use at all is new rather than moved.
pub fn series_moves(base: &[Locked], head: &[Locked]) -> Vec<SeriesMove> {
    let mut moves = Vec::new();
    for name in WATCHED {
        let versions = |side: &[Locked]| -> Vec<String> {
            side.iter()
                .filter(|l| l.name == name)
                .map(|l| l.version.clone())
                .collect()
        };
        let (from, to) = (versions(base), versions(head));
        let series_before: BTreeSet<String> = from.iter().map(|v| series(v)).collect();
        if from.is_empty() || to.iter().all(|v| series_before.contains(&series(v))) {
            continue;
        }
        moves.push(SeriesMove {
            name: name.to_string(),
            from,
            to,
        });
    }
    moves
}

/// A version's series: `0.N` before 1.0, else the major number. Anything after
/// a `-` or `+` (a release candidate, build data) doesn't change it.
pub fn series(version: &str) -> String {
    let numbers: Vec<&str> = version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .collect();
    match numbers.as_slice() {
        ["0", minor, ..] => format!("0.{minor}"),
        [major, ..] => (*major).to_string(),
        [] => version.to_string(),
    }
}
