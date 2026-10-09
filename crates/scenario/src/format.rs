//! The Scenario file format's number, the steps between formats, and which
//! of the repo's files a step rewrites (ADR-0002, #60).
//!
//! When a new starting-state item arrives, such as a new Assist, the pull
//! request that brings it teaches [`crate::read_scenario`] the item, adds a
//! [`Step`] to [`SCENARIO_STEPS`] that writes it with the value that keeps
//! today's behaviour, and runs `cargo xtask migrate <step>` over every
//! Scenario. Every Results file must stay the same. The steps themselves are
//! `opendrone_pack::migration`'s, shared with the Pack files.

use std::fs;

use opendrone_pack::document::Document;
use opendrone_pack::migration::toml_edit::{DocumentMut, InlineTable, Item, Value};
use opendrone_pack::migration::{
    Family, FileKind, MigrationFile, Step, add_after_in, pack_files, toml_files,
};
use opendrone_pack::units::{Dimension, parse_quantity};
use opendrone_pack::{Problems, TEST_MAPS_FOLDER};

use crate::{RESULTS_ENDING, Repo, walk};

/// The steps that bring an older Scenario up to date, one format each, in
/// order: the first upgrades format 1, the next format 2, and so on.
/// Scenarios are never upgraded in memory: an older one is refused, naming
/// the steps to run.
pub const SCENARIO_STEPS: &[Step] = &[Step {
    name: "add-crash-flip-switch",
    family: Family::Scenarios,
    from: 1,
    says: "a pilot's Timeline and each case of a Flight Controller Scenario set the Crash Flip switch on AUX3, written \"off\": before #54 the runner always sent it low",
    rewrite: add_crash_flip_switch,
}];

/// `add-crash-flip-switch`: `crash_flip = "off"` after `arm` in the
/// Timeline's moment at 0 s that sets the Arm switch, and in every
/// `[[case]]`. A Physics or Thrust Stand Scenario's scripted motors have no
/// switches, and an Input Track carries AUX3 already.
fn add_crash_flip_switch(_: FileKind, doc: &mut DocumentMut) -> Result<(), String> {
    let timeline = doc
        .get_mut("inputs")
        .and_then(Item::as_table_mut)
        .and_then(|inputs| inputs.get_mut("timeline"))
        .and_then(Item::as_array_mut);
    if let Some(timeline) = timeline
        && let Some(start) = timeline
            .iter_mut()
            .filter_map(Value::as_inline_table_mut)
            .find(|moment| moment.contains_key("arm") && at_the_start(moment))
    {
        add_after_in_inline(start, "arm", "crash_flip", "off")?;
    }
    if let Some(cases) = doc.get_mut("case").and_then(Item::as_array_of_tables_mut) {
        for case in cases.iter_mut() {
            add_after_in(case, "[[case]]", "arm", "crash_flip", "off")?;
        }
    }
    Ok(())
}

/// Whether a Timeline moment is at 0 s.
fn at_the_start(moment: &InlineTable) -> bool {
    moment
        .get("at")
        .and_then(Value::as_str)
        .and_then(|at| parse_quantity(at).ok())
        .and_then(|at| at.as_a(Dimension::TIME).ok())
        == Some(0.0)
}

/// Adds `key = value` to an inline table such as a Timeline moment, right
/// after `after` and spaced like it: `arm = "off" }` becomes
/// `arm = "off", crash_flip = "off" }`.
fn add_after_in_inline(
    table: &mut InlineTable,
    after: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    if table.contains_key(key) {
        return Err(format!("a Timeline moment already has `{key}`"));
    }
    let Some((anchor, anchor_item)) = table.get_key_value_mut(after) else {
        return Err(format!(
            "a Timeline moment has no `{after}` to add `{key}` after"
        ));
    };
    let key_decor = anchor.leaf_decor().clone();
    let Some(anchor_value) = anchor_item.as_value_mut() else {
        return Err(format!("`{after}` in a Timeline moment can't be read"));
    };
    let value_prefix = anchor_value
        .decor()
        .prefix()
        .and_then(|raw| raw.as_str())
        .unwrap_or(" ")
        .to_string();
    // Whatever followed the anchor's value (the space before the closing
    // brace, when it was the last) now follows the new one.
    let value_suffix = anchor_value
        .decor()
        .suffix()
        .and_then(|raw| raw.as_str())
        .unwrap_or("")
        .to_string();
    anchor_value.decor_mut().set_suffix("");
    let mut new_value = Value::from(value);
    new_value.decor_mut().set_prefix(value_prefix);
    new_value.decor_mut().set_suffix(value_suffix);
    let new_key = opendrone_pack::migration::toml_edit::Key::new(key).with_leaf_decor(key_decor);
    table.insert_formatted(&new_key, new_value);

    let order: Vec<String> = table.iter().map(|(k, _)| k.to_string()).collect();
    let place = |k: &str| {
        let lookup = if k == key { after } else { k };
        let index = order
            .iter()
            .position(|o| o == lookup)
            .unwrap_or(order.len());
        2 * index + usize::from(k == key)
    };
    table.sort_values_by(|a, _, b, _| place(a.get()).cmp(&place(b.get())));
    Ok(())
}

/// The newest Scenario format. Format 1 is the first, and each step in
/// [`SCENARIO_STEPS`] adds one. Pack files are numbered apart
/// (`opendrone_pack::document::FORMAT`).
pub const SCENARIO_FORMAT: i64 = 1 + SCENARIO_STEPS.len() as i64;

impl Repo {
    /// Every file a step of `family` rewrites:
    ///
    /// - for Scenarios, every Scenario in `scenarios/`, then every Scenario
    ///   the checks keep as a fixture under `crates/`, found by its
    ///   `[start]` table. Scenarios are never upgraded in memory, so a
    ///   fixture left in an older format would be refused;
    /// - for Pack files, every Pack file in `packs/`, every Test Quad in
    ///   `scenarios/test-quads/` and every built-in Test Map in
    ///   `crates/pack/test-maps/`. The checks' fixture Packs are left as
    ///   they are: the Pack reader upgrades them in memory.
    ///
    /// Results files are never migrated: the runner writes them.
    pub fn files_to_migrate(&self, family: Family) -> Result<Vec<MigrationFile>, Problems> {
        match family {
            Family::Scenarios => {
                let files = self.scenario_files().map_err(|error| {
                    Problems::of_file("scenarios", format!("can't be read: {error}"))
                })?;
                let mut files: Vec<MigrationFile> = files
                    .into_iter()
                    .map(|file| MigrationFile {
                        label: file.label(),
                        path: file.path,
                        kind: FileKind::Scenario,
                    })
                    .collect();
                let mut fixtures = Vec::new();
                walk(
                    &self.root.join("crates"),
                    "crates/",
                    &mut |relative, path| {
                        if relative.ends_with(".toml")
                            && !relative.ends_with(RESULTS_ENDING)
                            && fs::read_to_string(path).is_ok_and(|text| is_a_scenario(&text))
                        {
                            fixtures.push(MigrationFile {
                                path: path.to_path_buf(),
                                label: relative.to_string(),
                                kind: FileKind::Scenario,
                            });
                        }
                    },
                )
                .map_err(|error| Problems::of_file("crates", format!("can't be read: {error}")))?;
                files.extend(fixtures);
                Ok(files)
            }
            Family::Packs => {
                let mut files = pack_files(&self.root.join("packs"), "packs")?;
                files.extend(toml_files(
                    &self.scenarios_folder().join("test-quads"),
                    "scenarios/test-quads",
                    FileKind::TestQuad,
                )?);
                files.extend(toml_files(
                    &self.root.join(TEST_MAPS_FOLDER),
                    TEST_MAPS_FOLDER,
                    FileKind::Map,
                )?);
                Ok(files)
            }
        }
    }
}

/// Whether a TOML file is a Scenario: it has a `format` line and a `[start]`
/// table, which no other file OpenDrone reads has.
fn is_a_scenario(text: &str) -> bool {
    Document::parse("file", text).is_ok_and(|doc| {
        let root = doc.root();
        root.get("format").is_some() && root.get("start").is_some_and(|start| start.is_table())
    })
}
