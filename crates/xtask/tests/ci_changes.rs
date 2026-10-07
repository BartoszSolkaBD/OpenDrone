//! Readable checks for `.github/scripts/changes.sh`, which decides whether a
//! CI run needs the Rust checks and the security advisories (#15 §4).
//!
//! Each check makes a scratch git repository with a base commit, commits a
//! change on top of it, and runs the real script on it the way CI does: as a
//! pull request, where `HEAD^1` is the base.
//!
//! The script runs under Git Bash in CI on Windows, but these checks skip
//! Windows, where `bash` may start Windows' own Linux shell instead.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_only_changes_markdown_skips_the_rust_checks() {
    let repo = Repo::new("only-markdown");
    repo.write("README.md", "# OpenDrone, now with more words\n");
    repo.write("docs/context/new-notes.md", "Notes.\n");
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "false", "{outputs:?}");
    assert_eq!(outputs["libraries"], "false", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_renames_code_to_markdown_runs_the_rust_checks() {
    let repo = Repo::new("code-renamed-to-markdown");
    repo.rename("crates/physics/src/lib.rs", "crates/physics/src/notes.md");
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_renames_a_scenario_to_markdown_runs_the_rust_checks() {
    let repo = Repo::new("scenario-renamed-to-markdown");
    repo.rename("scenarios/free-fall.toml", "scenarios/free-fall.md");
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_only_changes_a_quads_feel_test_log_runs_the_rust_checks() {
    // The Pack checker reads each Quad's feel-tests.md, so a change to one is
    // checked even though it's Markdown.
    let repo = Repo::new("feel-test-log");
    repo.write(
        "packs/opendrone/quads/whoop-65/feel-tests.md",
        "# Feel Test log: Whoop 65\n\n| Date | Number | Old → new | Why |\n|---|---|---|---|\n| 2026-11-09 | [props] grip | 0.5 → 0.6 | wall bumps stuck too hard |\n",
    );
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_changes_a_text_file_under_docs_runs_the_rust_checks() {
    let repo = Repo::new("docs-text-file");
    repo.write(
        "docs/research/quad-settings/cetus-x.diff-all.txt",
        "set roll_rc_rate = 120\n",
    );
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_changes_cargo_lock_checks_the_security_advisories() {
    let repo = Repo::new("cargo-lock");
    repo.write("Cargo.lock", "version = 4\n# a library was added\n");
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
    assert_eq!(outputs["libraries"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pull_request_that_changes_the_licence_policy_checks_the_security_advisories() {
    let repo = Repo::new("deny-toml");
    repo.write(
        "deny.toml",
        "[advisories]\nignore = [\"RUSTSEC-0000-0000\"]\n",
    );
    let outputs = repo.commit_and_check("pull_request");
    assert_eq!(outputs["libraries"], "true", "{outputs:?}");
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_push_to_main_runs_every_rust_check_even_for_markdown() {
    let repo = Repo::new("push-to-main");
    repo.write("README.md", "# OpenDrone, now with more words\n");
    let outputs = repo.commit_and_check("push");
    assert_eq!(outputs["rust"], "true", "{outputs:?}");
}

/// A scratch git repository holding a few of the repo's kinds of files.
struct Repo {
    path: PathBuf,
}

impl Repo {
    fn new(case: &str) -> Repo {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("ci-changes")
            .join(case);
        match fs::remove_dir_all(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                panic!("can't clear {}: {error}", path.display())
            }
            _ => {}
        }
        fs::create_dir_all(&path).expect("can make the scratch folder");
        let repo = Repo { path };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo.write("README.md", "# OpenDrone\n");
        repo.write(
            "crates/physics/src/lib.rs",
            "//! Quad physics and Map collisions.\n\npub fn gravity() -> f64 {\n    -9.80665\n}\n",
        );
        repo.write(
            "scenarios/free-fall.toml",
            "name = \"free fall is exactly g\"\nbasis = \"Rule\"\n",
        );
        repo.write(
            "docs/research/quad-settings/cetus-x.diff-all.txt",
            "set roll_rc_rate = 100\n",
        );
        repo.write("Cargo.lock", "version = 4\n");
        repo.write("deny.toml", "[advisories]\nignore = []\n");
        repo.git(&["add", "--all"]);
        repo.git(&["commit", "--quiet", "--message", "base"]);
        repo
    }

    fn write(&self, file: &str, contents: &str) {
        let path = self.path.join(file);
        fs::create_dir_all(path.parent().expect("a file has a folder")).expect("can make folder");
        fs::write(path, contents).expect("can write the file");
    }

    fn rename(&self, from: &str, to: &str) {
        self.git(&["mv", from, to]);
    }

    /// Commits the change, runs changes.sh as CI would for this event, and
    /// returns its step outputs.
    fn commit_and_check(&self, event: &str) -> BTreeMap<String, String> {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--message", "the change"]);
        let outputs_file = self.path.join("github-output.txt");
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/scripts/changes.sh");
        let output = Command::new("bash")
            .arg(script)
            .current_dir(&self.path)
            .env("GITHUB_EVENT_NAME", event)
            .env("GITHUB_OUTPUT", &outputs_file)
            .output()
            .expect("bash runs");
        assert!(
            output.status.success(),
            "changes.sh failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read_to_string(&outputs_file)
            .expect("changes.sh wrote its outputs")
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
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
            .current_dir(&self.path)
            .env("GIT_CONFIG_GLOBAL", self.path.join("no-global-gitconfig"))
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
}
