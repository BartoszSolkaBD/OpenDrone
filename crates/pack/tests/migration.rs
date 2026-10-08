//! Readable checks for format migration (#60): a named step rewrites a file,
//! adding its item with the value that keeps today's behaviour, keeping
//! every comment and the layout, and bumping `format`; `cargo xtask migrate`
//! runs it over every Pack file at once or not at all; and the Pack readers
//! upgrade an older file in memory with the same step.
//!
//! The steps here are synthetic: each pretends an item the files hold today
//! arrived in a step from the format before the newest ([`PREVIOUS`], which
//! is 0 while format 1 is the first). So the step's output can be compared
//! with the committed file, byte for byte, and the checks keep working once
//! real steps exist. Basis: Rule, from ADR-0002 and ADR-0011. The end-to-end
//! checks over the repo's Scenarios and Packs, with their Results, are in the
//! Scenario runner's `tests/migration.rs`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Fixture, repo};
use opendrone_pack::document::{Document, FORMAT};
use opendrone_pack::migration::toml_edit::DocumentMut;
use opendrone_pack::migration::{
    Family, FileKind, MigrationFile, PACK_STEPS, Step, add_after, add_to_inline_table, apply,
    migrate, pack_files, toml_files, upgrade, upgraded_with,
};
use opendrone_pack::{
    Problems, TEST_MAPS_FOLDER, check_quad, read_map_file, read_map_file_with_steps,
    read_quad_file, read_tune, test_map, test_map_ids,
};

/// The format before the newest: the one the synthetic steps upgrade.
const PREVIOUS: i64 = FORMAT - 1;

/// A synthetic Pack step: Quad definitions gain `[motors] restart_tries`,
/// written after `start_wait`.
const RESTART_TRIES: Step = Step {
    name: "add-restart-tries",
    family: Family::Packs,
    from: PREVIOUS,
    says: "Quad definitions gain `[motors] restart_tries = 3`: Bluejay tries three times to restart a stalled motor, as every Quad did before the item existed",
    rewrite: add_restart_tries,
};

fn add_restart_tries(kind: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    match kind {
        FileKind::Quad => add_after(doc, "motors", "start_wait", "restart_tries", 3),
        _ => Ok(()),
    }
}

/// The same step, from format 0 to 1, for the checks on small files written
/// out in full.
const RESTART_TRIES_FROM_0: Step = Step {
    from: 0,
    ..RESTART_TRIES
};

const RESTART_TRIES_LINE: &str = "restart_tries       = 3\n";

/// A synthetic Pack step for Maps: `[world] air_density` arrives, at sea
/// level.
const ADD_AIR_DENSITY: Step = Step {
    name: "add-air-density",
    family: Family::Packs,
    from: PREVIOUS,
    says: "Maps gain `[world] air_density = \"1.225 kg/m³\"`: every Map had sea-level air before the item existed",
    rewrite: add_air_density,
};

fn add_air_density(kind: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    match kind {
        FileKind::Map => add_after(doc, "world", "gravity", "air_density", "1.225 kg/m³"),
        _ => Ok(()),
    }
}

/// A committed file as it was in the format before: `format` one lower, and
/// without `line` when it is given.
fn before(text: &str, line: Option<&str>) -> String {
    assert_eq!(format_of(text), FORMAT);
    let text = with_format(text, PREVIOUS);
    match line {
        Some(line) => {
            assert_eq!(text.matches(line).count(), 1, "{line:?}");
            text.replacen(line, "", 1)
        }
        None => text,
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

/// The file with another number on its `format` line, spaced as it was.
fn with_format(text: &str, format: i64) -> String {
    let line = text
        .lines()
        .find(|line| line.starts_with("format "))
        .expect("every file has its format line");
    let (key, _) = line.split_once('=').unwrap();
    text.replacen(line, &format!("{key}= {format}"), 1)
}

fn built_in_quads() -> Vec<(String, String)> {
    let quads = repo().join("packs/opendrone/quads");
    let mut found: Vec<(String, String)> = fs::read_dir(&quads)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let label = format!(
                "packs/opendrone/quads/{}/quad.toml",
                entry.file_name().to_string_lossy()
            );
            (
                label,
                fs::read_to_string(entry.path().join("quad.toml")).unwrap(),
            )
        })
        .collect();
    found.sort();
    assert!(
        found.len() >= 2,
        "expected the Whoop 65 and the Freestyle 5″"
    );
    found
}

// One file

#[test]
fn a_step_adds_its_item_lined_up_with_its_neighbours_and_keeps_every_comment() {
    let before = "\
# A comment at the top of the file.
format = 0   # the format line keeps its comment

[motors]   # a comment on the section
spin_up             = \"35 ms\"
start_wait          = \"0.1 s\"   # a comment after a value

# A comment above the next item stays with it.
startup_power_limit = \"1.96%\"

[battery]
cells = 1
";
    assert_eq!(
        apply(&RESTART_TRIES_FROM_0, FileKind::Quad, before).unwrap(),
        "\
# A comment at the top of the file.
format = 1   # the format line keeps its comment

[motors]   # a comment on the section
spin_up             = \"35 ms\"
start_wait          = \"0.1 s\"   # a comment after a value
restart_tries       = 3

# A comment above the next item stays with it.
startup_power_limit = \"1.96%\"

[battery]
cells = 1
"
    );
}

#[test]
fn every_built_in_quad_comes_back_from_the_step_exactly_as_committed() {
    for (label, committed) in built_in_quads() {
        let old = before(&committed, Some(RESTART_TRIES_LINE));
        assert!(!old.contains("restart_tries"));
        assert_eq!(
            apply(&RESTART_TRIES, FileKind::Quad, &old).unwrap(),
            committed,
            "{label}"
        );
    }
}

#[test]
fn a_kind_of_file_the_step_doesnt_change_gets_only_its_format_bumped() {
    for (file, kind) in [
        ("packs/opendrone/pack.toml", FileKind::Manifest),
        (
            "scenarios/test-quads/whoop-65-no-drag.toml",
            FileKind::TestQuad,
        ),
        ("crates/pack/test-maps/empty-air.toml", FileKind::Map),
    ] {
        let committed = fs::read_to_string(repo().join(file)).unwrap();
        assert_eq!(
            apply(&RESTART_TRIES, kind, &before(&committed, None)).unwrap(),
            committed,
            "{file}"
        );
    }
}

#[test]
fn an_item_added_to_an_inline_table_goes_before_its_closing_brace() {
    let one_line = "\
format = 1
[start]
assists = { input_smoothing = \"off\", auto_arm = \"off\" }   # every Assist
";
    assert_eq!(
        apply(&WIND, FileKind::Scenario, one_line).unwrap(),
        "\
format = 2
[start]
assists = { input_smoothing = \"off\", auto_arm = \"off\", wind = \"off\" }   # every Assist
"
    );
    // TOML 1.1 lets an inline table run over several lines, with a comma
    // after the last item.
    let several_lines = "\
format = 1
[start]
assists = {
  input_smoothing = \"off\",
  auto_arm = \"off\",
}
";
    assert_eq!(
        apply(&WIND, FileKind::Scenario, several_lines).unwrap(),
        "\
format = 2
[start]
assists = {
  input_smoothing = \"off\",
  auto_arm = \"off\",
  wind = \"off\",
}
"
    );
}

#[test]
fn a_comment_beside_the_last_item_of_an_inline_table_stays_with_that_item() {
    let with_a_comma = "\
format = 1
[start]
assists = {
  input_smoothing = \"off\",
  # Auto-arm arms on the first throttle.
  auto_arm = \"off\",   # the last one
  # The end of the list.
}
";
    assert_eq!(
        apply(&WIND, FileKind::Scenario, with_a_comma).unwrap(),
        "\
format = 2
[start]
assists = {
  input_smoothing = \"off\",
  # Auto-arm arms on the first throttle.
  auto_arm = \"off\",   # the last one
  wind = \"off\",
  # The end of the list.
}
"
    );
    let without_a_comma = "\
format = 1
[start]
assists = {
  input_smoothing = \"off\",
  auto_arm = \"off\"   # the last one
}
";
    assert_eq!(
        apply(&WIND, FileKind::Scenario, without_a_comma).unwrap(),
        "\
format = 2
[start]
assists = {
  input_smoothing = \"off\",
  auto_arm = \"off\",   # the last one
  wind = \"off\"
}
"
    );
}

/// A pretend Scenario step for the inline-table checks: a new Assist, off.
const WIND: Step = Step {
    name: "add-wind-assist",
    family: Family::Scenarios,
    from: 1,
    says: "a pretend Assist",
    rewrite: add_wind,
};

fn add_wind(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    add_to_inline_table(doc, "start", "assists", "wind", "off")
}

#[test]
fn a_new_key_longer_than_its_neighbours_gets_one_space_before_its_equals_sign() {
    fn add_long(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
        add_after(doc, "board", "gyro", "accelerometer_range", "16 g")
    }
    let step = Step {
        name: "add-accelerometer-range",
        family: Family::Packs,
        from: 1,
        says: "a pretend item",
        rewrite: add_long,
    };
    assert_eq!(
        apply(
            &step,
            FileKind::Quad,
            "format = 1\n[board]\ngyro  = \"2000 °/s\"\nmag   = false\n"
        )
        .unwrap(),
        "format = 2\n[board]\ngyro  = \"2000 °/s\"\naccelerometer_range = \"16 g\"\nmag   = false\n"
    );
}

#[test]
fn a_file_with_windows_line_endings_keeps_them() {
    let before = "format = 0\r\n[motors]\r\nstart_wait = \"0.1 s\"\r\n";
    assert_eq!(
        apply(&RESTART_TRIES_FROM_0, FileKind::Quad, before).unwrap(),
        "format = 1\r\n[motors]\r\nstart_wait = \"0.1 s\"\r\nrestart_tries = 3\r\n"
    );
}

#[test]
fn a_step_refuses_another_format_another_family_and_an_item_already_there() {
    let step = RESTART_TRIES_FROM_0;
    let quad = "format = 0\n[motors]\nstart_wait = \"0.1 s\"\n";
    assert_eq!(
        apply(
            &step,
            FileKind::Quad,
            &quad.replace("format = 0", "format = 1")
        )
        .unwrap_err(),
        "this file is format 1, and `add-restart-tries` upgrades format 0 to 1"
    );
    assert_eq!(
        apply(&step, FileKind::Scenario, quad).unwrap_err(),
        "`add-restart-tries` rewrites every Pack file, Test Quad and Test Map, and this is a Scenario"
    );
    assert_eq!(
        apply(&step, FileKind::Quad, &format!("{quad}restart_tries = 3\n")).unwrap_err(),
        "[motors] already has `restart_tries`"
    );
    assert_eq!(
        apply(
            &step,
            FileKind::Quad,
            "format = 0\n[motors]\nspin_up = \"35 ms\"\n"
        )
        .unwrap_err(),
        "[motors] has no `start_wait` to add `restart_tries` after"
    );
    assert_eq!(
        apply(&step, FileKind::Quad, "format = 0\n").unwrap_err(),
        "there's no [motors]"
    );
}

// The Pack readers

#[test]
fn the_pack_reader_upgrades_an_older_quad_in_memory_with_the_same_step_as_the_tool() {
    for (label, committed) in built_in_quads() {
        let old = before(&committed, Some(RESTART_TRIES_LINE));
        let in_memory =
            upgraded_with(&label, &old, FileKind::Quad, FORMAT, &[RESTART_TRIES]).unwrap();
        let written = apply(&RESTART_TRIES, FileKind::Quad, &old).unwrap();
        assert_eq!(in_memory, written, "{label}");
        assert_eq!(
            read_quad_file(&label, &in_memory).unwrap(),
            read_quad_file(&label, &committed).unwrap(),
            "{label} reads as the very same Quad"
        );
        let tune_path = repo()
            .join(label.trim_end_matches("quad.toml"))
            .join("tune.txt");
        let tune = read_tune("tune.txt", &fs::read_to_string(tune_path).unwrap()).unwrap();
        let fingerprint = |text: &str| {
            check_quad(
                "opendrone/quad",
                None,
                &read_quad_file(&label, text).unwrap(),
                &tune,
            )
            .unwrap()
            .fingerprint()
        };
        assert_eq!(fingerprint(&in_memory), fingerprint(&committed), "{label}");
    }
}

#[test]
fn the_map_reader_upgrades_an_older_map_in_memory_too() {
    let committed =
        fs::read_to_string(repo().join(TEST_MAPS_FOLDER).join("empty-air.toml")).unwrap();
    let line = committed
        .lines()
        .find(|line| line.starts_with("air_density"))
        .unwrap();
    let old = before(&committed, Some(&format!("{line}\n")));
    assert!(!old.contains("air_density"));
    let label = "packs/example/maps/empty-air/map.toml";
    assert_eq!(
        read_map_file_with_steps("test/empty-air", label, &old, &[ADD_AIR_DENSITY]).unwrap(),
        read_map_file("test/empty-air", label, &committed).unwrap(),
        "the older map.toml, upgraded in memory, reads as the very same Map"
    );
    // Without the step, the same older file can't be read.
    assert!(read_map_file("test/empty-air", label, &old).is_err());
}

#[test]
fn every_built_in_test_map_is_a_file_the_tool_rewrites() {
    let files: Vec<MigrationFile> = toml_files(
        &repo().join(TEST_MAPS_FOLDER),
        TEST_MAPS_FOLDER,
        FileKind::Map,
    )
    .unwrap();
    let mut ids = test_map_ids();
    ids.sort();
    assert_eq!(
        files.iter().map(|f| f.label.clone()).collect::<Vec<_>>(),
        ids.iter()
            .map(|id| format!("{TEST_MAPS_FOLDER}/{}.toml", id.trim_start_matches("test/")))
            .collect::<Vec<_>>(),
        "each built-in Test Map's map.toml is a file in {TEST_MAPS_FOLDER}, so `cargo xtask migrate` reaches it"
    );
    for (id, file) in ids.iter().zip(&files) {
        let built_in = test_map(id.trim_start_matches("test/")).unwrap().unwrap();
        let from_file =
            read_map_file(id, &file.label, &fs::read_to_string(&file.path).unwrap()).unwrap();
        assert_eq!(
            (built_in.name, built_in.world),
            (from_file.name, from_file.world),
            "{id} is built from {}",
            file.label
        );
    }
}

#[test]
fn a_file_in_the_newest_format_is_read_as_it_is() {
    let text = format!("format = {FORMAT}\n[motors]\nstart_wait = \"0.1 s\"\n");
    assert_eq!(
        upgraded_with("quad.toml", &text, FileKind::Quad, FORMAT, &[RESTART_TRIES]).unwrap(),
        text
    );
}

#[test]
fn an_older_format_is_upgraded_in_memory_one_step_at_a_time() {
    fn rename_frame_mass(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
        let mass = doc.remove("frame_mass").ok_or("no `frame_mass`")?;
        doc.insert("dry_mass", mass);
        Ok(())
    }
    fn add_cells(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
        add_after(doc, "", "dry_mass", "cells", 1)
    }
    let steps = [
        Step {
            name: "rename-frame-mass",
            family: Family::Packs,
            from: 1,
            says: "`frame_mass` becomes `dry_mass`",
            rewrite: rename_frame_mass,
        },
        Step {
            name: "add-cells",
            family: Family::Packs,
            from: 2,
            says: "`cells = 1`",
            rewrite: add_cells,
        },
    ];
    assert_eq!(
        upgrade(
            "format = 1\nframe_mass = \"23 g\"\n",
            FileKind::Quad,
            1,
            3,
            &steps
        )
        .unwrap(),
        "format = 3\ndry_mass = \"23 g\"\ncells    = 1\n"
    );
    assert_eq!(
        upgrade("format = 0\n", FileKind::Quad, 0, 3, &steps).unwrap_err(),
        "this file is format 0, and this OpenDrone has no step that upgrades it"
    );
}

#[test]
fn a_pack_file_whose_step_is_missing_is_refused_at_its_format_line() {
    let problems = upgraded_with(
        "quad.toml",
        "# The Quad.\nformat = 0\n[motors]\nstart_wait = \"0.1 s\"\n",
        FileKind::Quad,
        2,
        &[RESTART_TRIES_FROM_0],
    )
    .unwrap_err();
    assert_eq!(
        problems.to_string(),
        "quad.toml line 2: this file is format 1, and this OpenDrone has no step that upgrades it"
    );
}

#[test]
fn a_file_in_an_older_format_that_isnt_upgraded_in_memory_names_the_step_that_brings_it_up_to_date()
{
    let doc = Document::parse("scenario.toml", "format = 1\nname = \"A\"\n").unwrap();
    let mut problems = Problems::new();
    doc.check_format_against(2, &[WIND], &mut problems);
    assert_eq!(
        problems.to_string(),
        "scenario.toml line 1: this file is format 1, older than the format 2 this OpenDrone reads: bring it up to date with `cargo xtask migrate add-wind-assist`"
    );
}

#[test]
fn the_pack_steps_lead_one_format_at_a_time_from_format_1_to_the_newest() {
    for (i, step) in PACK_STEPS.iter().enumerate() {
        assert_eq!(step.from, i as i64 + 1, "{}", step.name);
        assert_eq!(step.family, Family::Packs, "{}", step.name);
        assert!(opendrone_pack::is_an_id(step.name), "{}", step.name);
    }
    assert_eq!(FORMAT, PACK_STEPS.len() as i64 + 1);
}

// Every Pack file at once

/// Every Pack file the tool rewrites in a repo at `root`: its Packs, Test
/// Quads and built-in Test Maps.
fn files_of(root: &Path) -> Vec<MigrationFile> {
    let mut files = pack_files(&root.join("packs"), "packs").unwrap();
    files.extend(
        toml_files(
            &root.join("scenarios/test-quads"),
            "scenarios/test-quads",
            FileKind::TestQuad,
        )
        .unwrap(),
    );
    files.extend(
        toml_files(
            &root.join(TEST_MAPS_FOLDER),
            TEST_MAPS_FOLDER,
            FileKind::Map,
        )
        .unwrap(),
    );
    files
}

/// A scratch copy of the repo's Pack files, every one in the format before
/// the newest, and the Quads without `restart_tries`.
fn older_repo(case: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("pack-migration")
        .join(case);
    let _ = fs::remove_dir_all(&root);
    for file in files_of(&repo()) {
        let committed = fs::read_to_string(repo().join(&file.label)).unwrap();
        let line = (file.kind == FileKind::Quad).then_some(RESTART_TRIES_LINE);
        let path = root.join(&file.label);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, before(&committed, line)).unwrap();
    }
    root
}

#[test]
fn the_tool_finds_every_pack_file() {
    let fixture = Fixture::new("migration-finds-every-file");
    fixture.write("packs/fixture/maps/skate-park/map.toml", "format = 1\n");
    fixture.write("packs/fixture/input-devices/pocket.toml", "format = 1\n");
    fixture.write("packs/fixture/input-devices/notes.txt", "not a Pack file\n");
    let found = |files: Vec<MigrationFile>| -> Vec<(String, FileKind)> {
        files.into_iter().map(|f| (f.label, f.kind)).collect()
    };
    assert_eq!(
        found(pack_files(&fixture.root.join("packs"), "packs").unwrap()),
        [
            ("packs/fixture/pack.toml".to_string(), FileKind::Manifest),
            (
                "packs/fixture/quads/ducted/quad.toml".to_string(),
                FileKind::Quad
            ),
            (
                "packs/fixture/maps/skate-park/map.toml".to_string(),
                FileKind::Map
            ),
            (
                "packs/fixture/input-devices/pocket.toml".to_string(),
                FileKind::InputDevice
            ),
        ]
    );
    assert_eq!(
        found(
            toml_files(
                &fixture.root.join("test-quads"),
                "test-quads",
                FileKind::TestQuad
            )
            .unwrap()
        ),
        [(
            "test-quads/ducted-no-drag.toml".to_string(),
            FileKind::TestQuad
        )]
    );
}

#[test]
fn the_tool_brings_every_pack_file_back_exactly_as_committed() {
    let root = older_repo("every-pack-file");
    let files = files_of(&root);
    let migration = migrate(&RESTART_TRIES, &files).unwrap();
    assert_eq!(migration.rewritten.len(), files.len());
    assert!(migration.already.is_empty());
    migration.write().unwrap();
    for file in &files {
        assert_eq!(
            fs::read_to_string(&file.path).unwrap(),
            fs::read_to_string(repo().join(&file.label)).unwrap(),
            "{}",
            file.label
        );
    }
    // Run again, it finds every file done and changes nothing.
    let again = migrate(&RESTART_TRIES, &files).unwrap();
    assert!(again.rewritten.is_empty());
    assert_eq!(again.already.len(), files.len());
}

#[test]
fn the_tool_writes_nothing_when_any_file_cant_take_the_step() {
    let root = older_repo("one-file-cant");
    let files = files_of(&root);
    let manifest = root.join("packs/opendrone/pack.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, with_format(&text, PREVIOUS - 1)).unwrap();
    let problems = migrate(&RESTART_TRIES, &files).unwrap_err();
    assert_eq!(
        problems.to_string(),
        format!(
            "packs/opendrone/pack.toml line {}: this file is format {}: it needs the steps before `add-restart-tries` first, which upgrades format {PREVIOUS} to {FORMAT}",
            common::line_of(&text, "format "),
            PREVIOUS - 1
        )
    );
    // With a problem there is no migration to write: every other file is
    // still as it was.
    for file in files.iter().filter(|f| f.kind != FileKind::Manifest) {
        let text = fs::read_to_string(&file.path).unwrap();
        assert_eq!(format_of(&text), PREVIOUS, "{}", file.label);
    }
}

#[test]
fn the_tool_writes_every_file_or_none() {
    let root = older_repo("every-file-or-none");
    let files = files_of(&root);
    let migration = migrate(&RESTART_TRIES, &files).unwrap();
    // A folder stands where the manifest's new text would be written first
    // (`.pack.toml.migrating`, beside it), so that one can't be written.
    let manifest = root.join("packs/opendrone/pack.toml");
    fs::create_dir_all(manifest.with_file_name(".pack.toml.migrating")).unwrap();
    let problems = migration.write().unwrap_err().to_string();
    assert!(
        problems.starts_with("packs/opendrone/pack.toml: can't be written (")
            && problems.ends_with("), so no file was changed"),
        "{problems}"
    );
    for file in &files {
        let text = fs::read_to_string(&file.path).unwrap();
        assert_eq!(
            format_of(&text),
            PREVIOUS,
            "{} was left as it was",
            file.label
        );
    }
    // No other hidden file is left behind.
    for file in files.iter().filter(|f| f.path != manifest) {
        let name = file.path.file_name().unwrap().to_string_lossy();
        assert!(
            !file
                .path
                .with_file_name(format!(".{name}.migrating"))
                .exists(),
            "{}",
            file.label
        );
    }
}
