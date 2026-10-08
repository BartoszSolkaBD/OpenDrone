//! The Scenario file format's number, the steps between formats, and which
//! of the repo's files a step rewrites (ADR-0002, #60).
//!
//! When a new starting-state item arrives, such as a new Assist, the pull
//! request that brings it teaches [`crate::read_scenario`] the item, adds a
//! [`Step`] to [`SCENARIO_STEPS`] that writes it with the value that keeps
//! today's behaviour, and runs `cargo xtask migrate <step>` over every
//! Scenario. Every Results file must stay the same. The steps themselves are
//! `opendrone_pack::migration`'s, shared with the Pack files.

use opendrone_pack::migration::{Family, FileKind, MigrationFile, Step, pack_files, toml_files};
use opendrone_pack::{Problems, TEST_MAPS_FOLDER};

use crate::Repo;

/// The steps that bring an older Scenario up to date, one format each, in
/// order: the first upgrades format 1, the next format 2, and so on.
/// Scenarios are never upgraded in memory: an older one is refused, naming
/// the steps to run.
pub const SCENARIO_STEPS: &[Step] = &[];

/// The newest Scenario format. Format 1 is the first, and each step in
/// [`SCENARIO_STEPS`] adds one. Pack files are numbered apart
/// (`opendrone_pack::document::FORMAT`).
pub const SCENARIO_FORMAT: i64 = 1 + SCENARIO_STEPS.len() as i64;

impl Repo {
    /// Every file a step of `family` rewrites: every Scenario in
    /// `scenarios/`; or every Pack file in `packs/`, every Test Quad in
    /// `scenarios/test-quads/` and every built-in Test Map in
    /// `crates/pack/test-maps/`. Results files are never migrated: the
    /// runner writes them.
    pub fn files_to_migrate(&self, family: Family) -> Result<Vec<MigrationFile>, Problems> {
        match family {
            Family::Scenarios => {
                let files = self.scenario_files().map_err(|error| {
                    Problems::of_file("scenarios", format!("can't be read: {error}"))
                })?;
                Ok(files
                    .into_iter()
                    .map(|file| MigrationFile {
                        label: file.label(),
                        path: file.path,
                        kind: FileKind::Scenario,
                    })
                    .collect())
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
