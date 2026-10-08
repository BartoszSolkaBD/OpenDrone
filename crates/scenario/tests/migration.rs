//! Readable checks for format migration over the repo's own files (#60): the
//! step `cargo xtask migrate` runs over every Scenario, or every Pack file,
//! adds its item with the value that keeps today's behaviour, keeps every
//! comment and the layout, bumps `format`, and leaves every Result unchanged.
//!
//! Each check runs a synthetic step on a scratch copy of the repo, whose
//! files are written in the format before the newest (a pretend format 0
//! while format 1 is the first), before an item they hold today existed. The
//! step must give back the committed files byte for byte, and every Scenario
//! must still match its committed Results. Written against the newest
//! format, the checks keep working once real steps exist. Basis: Rule, from ADR-0002 (a new item is
//! written with the value that keeps today's behaviour, so no Result moves)
//! and ADR-0011 (the same steps upgrade the Pack files).

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::TEST_MAPS_FOLDER;
use opendrone_pack::document::{Document, FORMAT};
use opendrone_pack::migration::toml_edit::DocumentMut;
use opendrone_pack::migration::{
    Family, FileKind, PACK_STEPS, Step, add_after, add_to_inline_table, migrate,
};
use opendrone_scenario::{Repo, ResultsFile, SCENARIO_FORMAT, SCENARIO_STEPS, run_one};

/// A synthetic Scenario step: a new Assist, Auto-arm, written off.
const ADD_AUTO_ARM: Step = Step {
    name: "add-auto-arm",
    family: Family::Scenarios,
    from: SCENARIO_FORMAT - 1,
    says: "Scenarios gain the Auto-arm Assist, off: before it existed, no Scenario armed by itself",
    rewrite: add_auto_arm,
};

fn add_auto_arm(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    add_to_inline_table(doc, "start", "assists", "auto_arm", "off")
}

const AUTO_ARM: &str = ", auto_arm = \"off\" }";

/// A synthetic Pack step: Quad definitions gain `[motors] restart_tries`.
const ADD_RESTART_TRIES: Step = Step {
    name: "add-restart-tries",
    family: Family::Packs,
    from: FORMAT - 1,
    says: "Quad definitions gain `[motors] restart_tries = 3`: Bluejay tries three times to restart a stalled motor, as every Quad did before the item existed",
    rewrite: add_restart_tries,
};

fn add_restart_tries(kind: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    match kind {
        FileKind::Quad => add_after(doc, "motors", "start_wait", "restart_tries", 3),
        _ => Ok(()),
    }
}

const RESTART_TRIES: &str = "restart_tries       = 3\n";

fn repo() -> Repo {
    Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the runner lives in the repo")
}

/// A scratch copy of the repo's `scenarios/`, `packs/` and the built-in Test
/// Maps' files. (The runner reads the Test Maps built into the code, so the
/// copies are there only for the tool to rewrite.)
fn scratch(case: &str) -> Repo {
    let root: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("scenario-migration")
        .join(case);
    let _ = fs::remove_dir_all(&root);
    for folder in ["scenarios", "packs", TEST_MAPS_FOLDER] {
        copy(&repo().root.join(folder), &root.join(folder));
    }
    Repo { root }
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The number on a file's `format` line.
fn format_of(text: &str) -> i64 {
    Document::parse("file", text)
        .unwrap()
        .root()
        .get("format")
        .and_then(|item| item.integer())
        .expect("every file has its format line")
}

/// The file, written in `newest`, as it was in the format before: without
/// `item`, which reads `instead`.
fn before(text: &str, newest: i64, item: &str, instead: &str) -> String {
    assert_eq!(format_of(text), newest);
    let line = text
        .lines()
        .find(|line| line.starts_with("format "))
        .unwrap();
    let (key, _) = line.split_once('=').unwrap();
    text.replacen(line, &format!("{key}= {}", newest - 1), 1)
        .replacen(item, instead, 1)
}

/// Runs every Scenario in `repo` against its Packs, checking each Results
/// file is unchanged, and panics naming any that isn't.
fn every_result_is_unchanged(repo: &Repo) {
    let packs = repo.packs().unwrap();
    assert!(packs.problems().is_empty(), "{}", packs.problems());
    for file in repo.scenario_files().unwrap() {
        let report = run_one(&file, &packs, ResultsFile::Check, None);
        assert!(report.passed(), "{}: {:#?}", file.label(), report.checks);
    }
}

#[test]
fn a_step_adding_an_assist_to_every_scenario_keeps_every_comment_and_every_result() {
    let scratch = scratch("add-auto-arm");
    let files = scratch.files_to_migrate(Family::Scenarios).unwrap();
    let mut written_before = 0;
    for file in &files {
        let committed = fs::read_to_string(&file.path).unwrap();
        if committed.matches(AUTO_ARM).count() == 1 {
            fs::write(
                &file.path,
                before(&committed, SCENARIO_FORMAT, AUTO_ARM, " }"),
            )
            .unwrap();
            written_before += 1;
        }
    }
    assert!(written_before >= 3, "expected the Physics Scenarios");

    let migration = migrate(&ADD_AUTO_ARM, &files).unwrap();
    assert_eq!(migration.rewritten.len(), written_before);
    migration.write().unwrap();

    for file in &files {
        assert_eq!(
            fs::read_to_string(&file.path).unwrap(),
            fs::read_to_string(repo().root.join(&file.label)).unwrap(),
            "{} should be back exactly as committed: Auto-arm off, every comment and line in place, the newest format",
            file.label
        );
    }
    every_result_is_unchanged(&scratch);
}

#[test]
fn a_scenario_step_rewrites_only_scenarios() {
    let labels: Vec<String> = repo()
        .files_to_migrate(Family::Scenarios)
        .unwrap()
        .into_iter()
        .map(|file| {
            assert_eq!(file.kind, FileKind::Scenario);
            file.label
        })
        .collect();
    assert!(labels.contains(&"scenarios/physics/free-fall-is-exactly-g.toml".to_string()));
    for label in &labels {
        assert!(
            !label.ends_with(".results.toml") && !label.starts_with("scenarios/test-quads/"),
            "{label}: Results files are written by the runner, and Test Quads are Pack files"
        );
    }
}

#[test]
fn a_step_adding_an_item_to_every_quad_definition_leaves_every_result_unchanged() {
    let scratch = scratch("add-restart-tries");
    let files = scratch.files_to_migrate(Family::Packs).unwrap();
    for file in &files {
        let committed = fs::read_to_string(&file.path).unwrap();
        let item = if file.kind == FileKind::Quad {
            RESTART_TRIES
        } else {
            ""
        };
        fs::write(&file.path, before(&committed, FORMAT, item, "")).unwrap();
    }
    let labels: Vec<&str> = files.iter().map(|file| file.label.as_str()).collect();
    for expected in [
        "packs/opendrone/pack.toml",
        "packs/opendrone/quads/whoop-65/quad.toml",
        "scenarios/test-quads/whoop-65-no-drag.toml",
        "crates/pack/test-maps/empty-air.toml",
    ] {
        assert!(labels.contains(&expected), "{expected} in {labels:?}");
    }

    let migration = migrate(&ADD_RESTART_TRIES, &files).unwrap();
    assert_eq!(migration.rewritten.len(), files.len());
    migration.write().unwrap();

    for file in &files {
        assert_eq!(
            fs::read_to_string(&file.path).unwrap(),
            fs::read_to_string(repo().root.join(&file.label)).unwrap(),
            "{} should be back exactly as committed",
            file.label
        );
    }
    let fingerprints = |repo: &Repo| -> Vec<(String, String)> {
        let packs = repo.packs().unwrap();
        packs
            .quads()
            .into_iter()
            .chain(packs.test_quads())
            .map(|quad| (quad.id.clone(), quad.fingerprint().to_string()))
            .collect()
    };
    assert_eq!(fingerprints(&scratch), fingerprints(&repo()));
    every_result_is_unchanged(&scratch);
}

#[test]
fn the_scenario_steps_lead_one_format_at_a_time_from_format_1_to_the_newest() {
    for (i, step) in SCENARIO_STEPS.iter().enumerate() {
        assert_eq!(step.from, i as i64 + 1, "{}", step.name);
        assert_eq!(step.family, Family::Scenarios, "{}", step.name);
        assert!(opendrone_pack::is_an_id(step.name), "{}", step.name);
    }
    assert_eq!(SCENARIO_FORMAT, SCENARIO_STEPS.len() as i64 + 1);
}

#[test]
fn every_step_has_its_own_name_so_cargo_xtask_migrate_finds_one() {
    let mut names: Vec<&str> = SCENARIO_STEPS
        .iter()
        .chain(PACK_STEPS)
        .map(|step| step.name)
        .collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "{names:?}");
}

#[test]
fn every_scenario_and_pack_file_in_the_repo_is_written_in_the_newest_format() {
    let repo = repo();
    for (family, newest) in [
        (Family::Scenarios, SCENARIO_FORMAT),
        (Family::Packs, FORMAT),
    ] {
        for file in repo.files_to_migrate(family).unwrap() {
            assert_eq!(
                format_of(&fs::read_to_string(&file.path).unwrap()),
                newest,
                "{}: run its step with `cargo xtask migrate`",
                file.label
            );
        }
    }
}
