//! Readable checks for the house-rule Clippy settings (ADR-0001): every core
//! crate carries the same ones, they apply to the core crates only, and Clippy
//! really flags each rule.
//!
//! Clippy quietly ignores a rule whose name it can't find, so the last two
//! checks run Clippy on a crate that breaks every rule
//! (`house_rules/breaks_every_rule.rs`), once with the core's settings and
//! once without. It uses the real parry3d, the version the workspace pins, so
//! the rules naming parry3d's functions are proved against the real paths.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CORE: [&str; 5] = ["maths", "physics", "flight-controller", "sim", "test-pilot"];
const NOT_CORE: [&str; 7] = [
    "input",
    "pack",
    "scenario",
    "blackbox",
    "sound",
    "opendrone",
    "xtask",
];

#[test]
fn every_core_crate_carries_the_same_house_rule_clippy_settings() {
    let reference = read(&crates().join("maths/clippy.toml"));
    for name in CORE {
        let settings = read(&crates().join(name).join("clippy.toml"));
        assert_eq!(
            settings, reference,
            "crates/{name}/clippy.toml differs from crates/maths/clippy.toml, \
             but the five core crates share one set of house rules"
        );
    }
}

#[test]
fn the_house_rules_apply_to_the_core_crates_only() {
    let mut folders: Vec<PathBuf> = NOT_CORE.iter().map(|name| crates().join(name)).collect();
    folders.push(repo());
    for folder in folders {
        for file in ["clippy.toml", ".clippy.toml"] {
            let path = folder.join(file);
            if path.exists() {
                assert!(
                    !read(&path).contains("disallowed-"),
                    "{} bans methods or types, but the house rules apply to the core crates only",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn the_house_rules_ban_the_parry3d_functions_that_would_run_its_allowed_operating_system_maths() {
    // walls.toml lets parry3d keep its calls to the operating system's
    // `acos` (a mesh's pseudo-normals) and `log2` (rebalancing a tree of
    // moving shapes) only because the Simulation never runs them. These are
    // every public way into them in parry3d 0.31.1; the check below proves
    // Clippy flags each one.
    let settings: toml::Table = read(&crates().join("maths/clippy.toml"))
        .parse()
        .expect("clippy.toml is valid TOML");
    let entries = settings["disallowed-methods"]
        .as_array()
        .expect("a list of rules");
    for path in [
        "parry3d_f64::shape::TriMesh::set_flags",
        "parry3d_f64::shape::TriMesh::with_flags",
        "parry3d_f64::shape::TriMesh::update_vertices",
        "parry3d_f64::shape::TriMesh::set_vertices",
        "parry3d_f64::shape::SharedShape::trimesh_with_flags",
        "parry3d_f64::shape::TriMesh::connected_component_meshes",
        "parry3d_f64::shape::TriMeshConnectedComponents::to_meshes",
        "parry3d_f64::shape::TriMesh::append",
        "parry3d_f64::transformation::volume_mesh",
        "parry3d_f64::partitioning::Bvh::optimize_incremental",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry["path"].as_str() == Some(path))
            .unwrap_or_else(|| panic!("the house rules don't ban {path}"));
        let reason = entry["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains("operating system's") && reason.contains("(ADR-0001)"),
            "the rule banning {path} needs a reason naming the operating system's maths and \
             ADR-0001, not {reason:?}"
        );
    }
}

#[test]
fn clippy_flags_every_house_rule_broken_in_a_core_crate() {
    let report = clippy_on_the_rule_breaker("core-crate", true);
    let missed: Vec<String> = house_rules()
        .into_iter()
        .filter(|rule| !report.contains(rule))
        .collect();
    assert!(
        missed.is_empty(),
        "Clippy didn't flag these house rules in a core crate, so a path in clippy.toml may be \
         wrong or breaks_every_rule.rs doesn't break it yet:\n  {}\n\nClippy said:\n{report}",
        missed.join("\n  ")
    );
}

#[test]
fn an_edge_crate_may_use_std_maths_hash_maps_and_the_clock() {
    let report = clippy_on_the_rule_breaker("edge-crate", false);
    assert!(
        !report.contains("disallowed"),
        "Clippy applied a house rule outside the core:\n{report}"
    );
}

/// Every rule in the core's `clippy.toml`, as Clippy words it when it flags
/// one: "disallowed method `f64::sin`", "disallowed type `std::fs::File`".
fn house_rules() -> Vec<String> {
    let settings: toml::Table = read(&crates().join("maths/clippy.toml"))
        .parse()
        .expect("clippy.toml is valid TOML");
    let mut rules = Vec::new();
    for (key, kind) in [
        ("disallowed-methods", "method"),
        ("disallowed-types", "type"),
    ] {
        let entries = settings[key].as_array().expect("a list of rules");
        for entry in entries {
            let path = entry["path"].as_str().expect("each rule has a path");
            rules.push(format!("disallowed {kind} `{path}`"));
        }
    }
    assert!(rules.len() > 10, "the house rules look empty: {rules:?}");
    rules
}

/// Runs Clippy on a scratch crate holding `breaks_every_rule.rs`, with or
/// without the core's `clippy.toml` beside it, and returns what Clippy said.
/// The crate uses parry3d as the workspace does, with the workspace's
/// `Cargo.lock`, so nothing is downloaded.
fn clippy_on_the_rule_breaker(case: &str, with_house_rules: bool) -> String {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("house-rules");
    let folder = scratch.join(case);
    match fs::remove_dir_all(&folder) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            panic!("can't clear {}: {error}", folder.display())
        }
        _ => {}
    }
    fs::create_dir_all(&folder).expect("can make the scratch folder");
    let workspace: toml::Table = read(&repo().join("Cargo.toml"))
        .parse()
        .expect("the workspace's Cargo.toml is valid TOML");
    let parry = &workspace["workspace"]["dependencies"]["parry3d-f64"];
    // Each case's crate has its own name: the two share a target folder,
    // where two crates of one name would share Clippy's saved findings.
    let manifest = format!(
        "[package]\nname = \"rule-breaker-{case}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\nparry3d-f64 = {parry}\n\n\
         [workspace]\n"
    );
    fs::write(folder.join("Cargo.toml"), manifest).expect("can write Cargo.toml");
    fs::copy(repo().join("Cargo.lock"), folder.join("Cargo.lock")).expect("can copy Cargo.lock");
    fs::write(
        folder.join("lib.rs"),
        include_str!("house_rules/breaks_every_rule.rs"),
    )
    .expect("can write lib.rs");
    if with_house_rules {
        fs::copy(
            crates().join("maths/clippy.toml"),
            folder.join("clippy.toml"),
        )
        .expect("can copy clippy.toml");
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["clippy", "--quiet", "--offline", "--manifest-path"])
        .arg(folder.join("Cargo.toml"))
        .current_dir(&folder)
        // Both cases share one target folder, so parry3d is checked once.
        .env("CARGO_TARGET_DIR", scratch.join("target"))
        .env_remove("CLIPPY_CONF_DIR")
        .output()
        .expect("cargo clippy runs");
    let report = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "Clippy couldn't run:\n{report}");
    report
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crates() -> PathBuf {
    repo().join("crates")
}

/// A file's text, with Windows line endings read as plain ones.
fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("can't read {}: {error}", path.display()))
        .replace("\r\n", "\n")
}
