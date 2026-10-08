//! Readable checks for the two Pack checks CI runs: `cargo xtask packs` and
//! `cargo xtask feel-tests --base HEAD^1`. Each runs the real command, the way
//! CI does, on the repo or on a scratch copy of the pack crate's good fixture
//! Pack. The feel-tests checks commit a change in a scratch git repository,
//! so `HEAD^1` is the base, as on a pull request's merge commit.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const QUAD: &str = "packs/fixture/quads/ducted/quad.toml";
const LOG: &str = "packs/fixture/quads/ducted/feel-tests.md";

/// Runs `cargo xtask <args>` in `folder`, and returns whether it passed and
/// what it printed.
fn xtask(folder: &Path, args: &[&str]) -> (bool, String) {
    // Git's own settings, as on a fresh CI runner: it quotes any path that
    // isn't plain ASCII unless asked otherwise.
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(folder)
        .env("GIT_CONFIG_GLOBAL", folder.join("no-global-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("xtask runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), text)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A scratch folder holding the pack crate's good fixture Pack under `packs/`.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(case: &str) -> Scratch {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("pack-checks")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        copy(
            &repo_root().join("crates/pack/tests/fixtures/good/packs"),
            &root.join("packs"),
        );
        Scratch { root }
    }

    fn change(&self, file: &str, old: &str, new: &str) {
        let path = self.root.join(file);
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(old).count(), 1, "{file}: {old:?}");
        fs::write(path, text.replacen(old, new, 1)).unwrap();
    }

    fn append(&self, file: &str, line: &str) {
        let path = self.root.join(file);
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, text + line + "\n").unwrap();
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Scratch",
                "-c",
                "user.email=scratch@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_GLOBAL", self.root.join("no-global-gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Makes the scratch folder a git repository with the fixture committed
    /// as the base.
    fn with_base(self) -> Scratch {
        self.git(&["init", "--quiet", "--initial-branch=main"]);
        self.commit("base");
        self
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--message", message]);
    }
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

#[test]
fn every_pack_in_the_repo_passes_the_pack_checker() {
    let (passed, text) = xtask(&repo_root(), &["packs"]);
    assert!(passed, "{text}");
    assert!(text.contains("opendrone/whoop-65"), "{text}");
    assert!(text.contains("opendrone/freestyle-5"), "{text}");
    assert!(text.contains("test/whoop-65-no-drag"), "{text}");
}

#[test]
fn a_problem_in_any_pack_blocks_naming_its_file_and_line() {
    let scratch = Scratch::new("broken-pack");
    scratch.change(QUAD, "\"23.0 g\"", "\"23,0 g\"");
    let (passed, text) = xtask(&scratch.root, &["packs"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("- packs/fixture/quads/ducted/quad.toml line 20: \"23,0 g\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"23.0 g\""),
        "{text}"
    );
}

#[test]
fn an_estimate_moved_without_a_feel_test_log_row_blocks() {
    let scratch = Scratch::new("feel-tests-no-row").with_base();
    scratch.change(QUAD, "\"0.3 s⁻¹\"", "\"0.35 s⁻¹\"");
    scratch.commit("move rotor drag");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    let line = fs::read_to_string(scratch.root.join(QUAD))
        .unwrap()
        .lines()
        .position(|line| line.contains("0.35 s⁻¹"))
        .unwrap()
        + 1;
    assert!(
        text.contains(&format!("packs/fixture/quads/ducted/quad.toml line {line}: [props] rotor_drag is an Estimate that moved from 0.3 s⁻¹ to 0.35 s⁻¹, so packs/fixture/quads/ducted/feel-tests.md needs a new row for it")),
        "{text}"
    );
}

#[test]
fn an_estimate_moved_with_its_feel_test_log_row_passes() {
    let scratch = Scratch::new("feel-tests-with-row").with_base();
    scratch.change(QUAD, "\"0.3 s⁻¹\"", "\"0.35 s⁻¹\"");
    scratch.append(
        LOG,
        "| 2026-11-09 | [props] rotor_drag | 0.3 s⁻¹ → 0.35 s⁻¹ | still too floaty after a sprint |",
    );
    scratch.commit("move rotor drag, with its row");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
}

#[test]
fn a_locked_number_changed_without_a_new_source_blocks() {
    let scratch = Scratch::new("feel-tests-locked").with_base();
    scratch.change(QUAD, "\"23.0 g\"", "\"24.0 g\"");
    scratch.commit("heavier");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("[frame] dry_mass is Manufacturer, so it's locked"),
        "{text}"
    );
}

/// The fixture's inertia moved 20×, against its range of ×0.5–×2.
const INERTIA: &str = "roll 70, pitch 90, yaw 140 g·cm²";
const INERTIA_20X: &str = "roll 1400, pitch 1800, yaw 2800 g·cm²";

#[test]
fn moving_a_packs_folder_keeps_its_quads_ids_so_the_rules_still_compare_them() {
    // Reviewer's round-2 case: the Pack's folder renamed with a 20× move.
    let scratch = Scratch::new("feel-tests-pack-folder").with_base();
    fs::rename(
        scratch.root.join("packs/fixture"),
        scratch.root.join("packs/built-in"),
    )
    .unwrap();
    scratch.change(
        "packs/built-in/quads/ducted/quad.toml",
        INERTIA,
        INERTIA_20X,
    );
    scratch.commit("rename the Pack's folder and move the inertia");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("packs/built-in/quads/ducted/quad.toml line 22: [frame] inertia is an Estimate that moved"),
        "{text}"
    );
}

#[test]
fn renaming_a_quads_folder_must_keep_every_number() {
    // Reviewer's round-2 case: the Quad's folder renamed with a 20× move.
    let scratch = Scratch::new("feel-tests-quad-folder").with_base();
    fs::rename(
        scratch.root.join("packs/fixture/quads/ducted"),
        scratch.root.join("packs/fixture/quads/ducted-pro"),
    )
    .unwrap();
    scratch.change(
        "packs/fixture/quads/ducted-pro/quad.toml",
        INERTIA,
        INERTIA_20X,
    );
    scratch.commit("rename the Quad's folder and move the inertia");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("packs/fixture/quads/ducted-pro/quad.toml: this change adds the Quad fixture/ducted-pro and removes fixture/ducted, so it reads as a rename or a move, which must keep every setting as it was"),
        "{text}"
    );
}

#[test]
fn renaming_a_quads_folder_alone_passes() {
    let scratch = Scratch::new("feel-tests-pure-rename").with_base();
    fs::rename(
        scratch.root.join("packs/fixture/quads/ducted"),
        scratch.root.join("packs/fixture/quads/ducted-pro"),
    )
    .unwrap();
    scratch.commit("rename the Quad's folder");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
}

#[test]
fn a_number_that_passes_on_a_new_source_is_listed_for_the_reviewer() {
    let scratch = Scratch::new("feel-tests-listed").with_base();
    scratch.change(QUAD, "\"23.0 g\"", "\"24.0 g\"");
    scratch.change(
        QUAD,
        "maker      = \"the maker's page\"",
        "maker      = \"the maker's page, read again\"",
    );
    scratch.commit("heavier, with a new source");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    assert!(
        text.contains("These passed on their source alone (changed with a new source, re-sourced, taken out, or added); the Reviewer judges whether each source is real:"),
        "{text}"
    );
    assert!(
        text.contains("[frame] dry_mass changed (23.0 g → 24.0 g) with a new source"),
        "{text}"
    );
}

#[test]
fn an_estimate_moved_without_a_row_blocks_in_a_pack_folder_with_a_non_ascii_name() {
    // Reviewer's round-3 case: git quotes a path that isn't plain ASCII, so
    // the base read as having no Quads, every Quad as new, and nothing was
    // compared.
    let scratch = Scratch::new("feel-tests-non-ascii-pack-folder").with_base();
    fs::rename(
        scratch.root.join("packs/fixture"),
        scratch.root.join("packs/fixture-łódź"),
    )
    .unwrap();
    scratch.commit("step 1: rename the Pack's folder, and nothing else");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    assert!(
        text.contains("The Feel Test log rules hold for the 1 Quad(s) compared with their version at HEAD^1 (fixture/ducted)"),
        "{text}"
    );
    scratch.change(
        "packs/fixture-łódź/quads/ducted/quad.toml",
        INERTIA,
        INERTIA_20X,
    );
    scratch.commit("step 2: move the inertia 20×, with no row");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("- packs/fixture-łódź/quads/ducted/quad.toml line 22: [frame] inertia is an Estimate that moved from roll 70, pitch 90, yaw 140 g·cm² to roll 1400, pitch 1800, yaw 2800 g·cm²"),
        "{text}"
    );
}

#[test]
fn a_new_source_row_that_keeps_its_value_is_listed_because_it_moves_where_the_range_is_measured_from()
 {
    // Reviewer's round-3 case: three changes take the inertia ×4 against a
    // range of ×0.5–×2. Each passes, as the rules allow, so the re-sourcing in
    // the middle one must be listed for the Reviewer.
    const DOUBLED: &str = "roll 140, pitch 180, yaw 280 g·cm²";
    const DOUBLED_AGAIN: &str = "roll 280, pitch 360, yaw 560 g·cm²";
    let scratch = Scratch::new("feel-tests-re-sourced-in-place").with_base();
    scratch.change(QUAD, INERTIA, DOUBLED);
    scratch.append(
        LOG,
        &format!("| 2026-11-02 | [frame] inertia | {INERTIA} → {DOUBLED} | step 1 |"),
    );
    scratch.commit("step 1: ×2, with its row");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");

    scratch.change(
        QUAD,
        "guess      = \"a guess\"",
        "guess      = \"a guess; re-read\"",
    );
    scratch.append(
        LOG,
        &format!("| 2026-11-03 | [frame] inertia | {DOUBLED} → {DOUBLED} | New source: re-read |"),
    );
    scratch.commit("step 2: a new source, and a \"New source\" row that keeps the value");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    assert!(
        text.contains(&format!("- {LOG} line 7: [frame] inertia was re-sourced at {DOUBLED} (Estimate, range ×0.5–×2, from the source guess: \"a guess; re-read\"), so its range is measured from there from now on")),
        "{text}"
    );

    scratch.change(QUAD, DOUBLED, DOUBLED_AGAIN);
    scratch.append(
        LOG,
        &format!("| 2026-11-04 | [frame] inertia | {DOUBLED} → {DOUBLED_AGAIN} | step 3 |"),
    );
    scratch.commit("step 3: ×2 again, with its row");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
}

/// The fixture Quad's ducts: its `[ducts]` section and its duct rings.
const DUCTS: &str = "[ducts]\nram_drag       = { value = \"1.2 s⁻¹\", confidence = \"Estimate\", range = \"0.6–2.4 s⁻¹\", source = \"guess\" }\nnose_up_offset = { value = \"13 mm\", confidence = \"Estimate\", range = \"9–18 mm\", source = \"guess\" }\n\n";
const DUCT_RINGS: &str = "duct_rings  = { value = \"37 mm inside, 1.5 mm wall, 14 mm tall\", confidence = \"Estimate\", range = \"×0.9–×1.1\", source = \"guess\" }\n";

#[test]
fn a_number_added_to_a_quad_that_already_existed_is_listed_with_its_confidence_and_source() {
    // Reviewer's round-3 case: ducts added to a Quad that had none, claimed
    // as Measured from a made-up source. Taking a number out needs a new
    // source, so adding one is listed too, and the Reviewer judges the source.
    let scratch = Scratch::new("feel-tests-number-added");
    scratch.change(QUAD, DUCTS, "");
    scratch.change(QUAD, DUCT_RINGS, "");
    let scratch = scratch.with_base();
    scratch.change(
        QUAD,
        "[feel]",
        "[ducts]\nram_drag       = { value = \"5 s⁻¹\", confidence = \"Measured\", source = \"ducts\" }\nnose_up_offset = { value = \"40 mm\", confidence = \"Measured\", source = \"ducts\" }\n\n[feel]",
    );
    scratch.change(
        QUAD,
        "bounce      =",
        "duct_rings  = { value = \"130 mm inside, 2 mm wall, 30 mm tall\", confidence = \"Measured\", source = \"ducts\" }\nbounce      =",
    );
    scratch.change(QUAD, "[sources]\n", "[sources]\nducts      = \"made up\"\n");
    scratch.commit("add ducts");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    for line in [
        "line 30: [collision] duct_rings was added as 130 mm inside, 2 mm wall, 30 mm tall (Measured, from the source ducts: \"made up\")",
        "line 71: [ducts] ram_drag was added as 5 s⁻¹ (Measured, from the source ducts: \"made up\")",
        "line 72: [ducts] nose_up_offset was added as 40 mm (Measured, from the source ducts: \"made up\")",
    ] {
        assert!(text.contains(&format!("- {QUAD} {line}")), "{line}\n{text}");
    }
}

#[test]
fn removing_one_quad_and_adding_a_different_one_takes_two_changes() {
    // Reviewer's round-3 case: a change that removes a Quad reads any Quad it
    // adds as that one renamed, so a genuinely new Quad waits for a change of
    // its own.
    let add_a_different_quad = |scratch: &Scratch| {
        copy(
            &repo_root().join("crates/pack/tests/fixtures/good/packs/fixture/quads/ducted"),
            &scratch.root.join("packs/fixture/quads/ducted-75"),
        );
        scratch.change(
            "packs/fixture/quads/ducted-75/quad.toml",
            "\"66 mm\"",
            "\"75 mm\"",
        );
    };

    let scratch = Scratch::new("feel-tests-retire-and-add-at-once").with_base();
    fs::remove_dir_all(scratch.root.join("packs/fixture/quads/ducted")).unwrap();
    add_a_different_quad(&scratch);
    scratch.commit("retire the Quad and add a different one");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("- packs/fixture/quads/ducted-75/quad.toml: this change adds the Quad fixture/ducted-75 and removes fixture/ducted, so it reads as a rename or a move, which must keep every setting as it was; none of the removed Quads matches it. Rename or move a Quad in a change of its own, and change its numbers in another. To retire a Quad and add a different one, take it out in one change and add the new one in another"),
        "{text}"
    );

    let scratch = Scratch::new("feel-tests-retire-then-add").with_base();
    fs::remove_dir_all(scratch.root.join("packs/fixture/quads/ducted")).unwrap();
    scratch.commit("retire the Quad");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    add_a_different_quad(&scratch);
    scratch.commit("add a different Quad");
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(passed, "{text}");
    assert!(
        text.contains("New Quads, with no version at HEAD^1 to compare: fixture/ducted-75"),
        "{text}"
    );
}

#[test]
fn feel_tests_needs_a_base_revision_that_exists() {
    let scratch = Scratch::new("feel-tests-no-base").with_base();
    let (passed, text) = xtask(&scratch.root, &["feel-tests", "--base", "HEAD^1"]);
    assert!(!passed, "{text}");
    assert!(
        text.contains("git has no revision \"HEAD^1\" here; in CI the base is HEAD^1, which needs a checkout with fetch-depth 2"),
        "{text}"
    );
}
