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
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(folder)
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
