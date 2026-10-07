//! Readable checks that `unsafe` stays forbidden everywhere except
//! `opendrone-input` and the game (#15 §4).
//!
//! The compiler enforces the ban, but only in crates that take the workspace's
//! lints. A crate that drops `[lints] workspace = true`, or a new crate that
//! never had it, would quietly allow `unsafe`, so these checks read every
//! crate's `Cargo.toml`.

use std::fs;
use std::path::{Path, PathBuf};

/// The two crates that may use `unsafe`: they talk to SDL, Bevy and the
/// operating system.
const MAY_USE_UNSAFE: [&str; 2] = ["opendrone-input", "opendrone"];

#[test]
fn the_workspace_forbids_unsafe_code() {
    let manifest = manifest(&repo().join("Cargo.toml"));
    let level = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("rust"))
        .and_then(|rust| rust.get("unsafe_code"))
        .and_then(toml::Value::as_str);
    assert_eq!(
        level,
        Some("forbid"),
        "the root Cargo.toml must set [workspace.lints.rust] unsafe_code = \"forbid\""
    );
}

#[test]
fn every_crate_except_opendrone_input_and_the_game_forbids_unsafe_code() {
    let mut checked = 0;
    for folder in crate_folders() {
        let manifest = manifest(&folder.join("Cargo.toml"));
        let name = manifest["package"]["name"]
            .as_str()
            .expect("every crate has a name");
        if MAY_USE_UNSAFE.contains(&name) {
            continue;
        }
        let inherits = manifest
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(toml::Value::as_bool);
        assert_eq!(
            inherits,
            Some(true),
            "{name} ({}) must have `[lints] workspace = true`, which forbids `unsafe`; \
             only opendrone-input and the game may use it",
            folder.join("Cargo.toml").display()
        );
        checked += 1;
    }
    assert!(
        checked >= 10,
        "expected at least ten crates that forbid `unsafe`, found {checked}"
    );
}

/// Every crate folder in `crates/`, as the workspace's `members = ["crates/*"]`
/// finds them.
fn crate_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = fs::read_dir(repo().join("crates"))
        .expect("can list crates/")
        .map(|entry| entry.expect("can read crates/").path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .collect();
    folders.sort();
    folders
}

fn manifest(path: &Path) -> toml::Table {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("can't read {}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} isn't valid TOML: {error}", path.display()))
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
