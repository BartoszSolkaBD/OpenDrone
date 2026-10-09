//! Readable checks for `.github/scripts/review.sh`, which the Review workflow
//! runs (#77, #102): when it sets the Red Flag gate and the Review check, and
//! which other pull requests it has judged again.
//!
//! Each check runs the real script with `DRY_RUN=1`, so it prints each change
//! it would make on GitHub instead of making it. A stand-in for `gh` answers
//! from files these checks write, as GitHub's API would, and the pull request
//! lives in real git repositories in a scratch folder.
//!
//! The script runs under Linux's bash in CI, but these checks skip Windows,
//! where `bash` may start Windows' own Linux shell instead of Git Bash.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const REPO: &str = "BartoszSolkaBD/OpenDrone";
const HEAD: &str = "1111111111111111111111111111111111111111";
const OLDER: &str = "2222222222222222222222222222222222222222";
/// A later commit, on a branch stacked on the PR's.
const LATER: &str = "3333333333333333333333333333333333333333";

// When the two statuses are set.

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_run_sets_both_statuses_to_pending_first_then_to_their_results() {
    let scratch = Scratch::new("pending-first");
    let head = scratch.pull_request_branch();
    scratch.answer(
        &format!("repos/{REPO}/pulls/77"),
        &pull_request(77, &head, "main"),
    );
    scratch.answer(
        &format!("repos/{REPO}/issues/77/comments?per_page=100"),
        &json!([]),
    );
    scratch.answer(
        &format!("repos/{REPO}/commits/{head}/pulls?per_page=100"),
        &json!([listed(77, "open", "main", &head)]),
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(run.passed, "{}", run.output);
    let short = &head[..7];
    assert_eq!(
        run.statuses(),
        [
            format!("Red Flag gate on {head}: pending (Working out the Red Flags)"),
            format!("Review check on {head}: pending (Working out the Review check)"),
            format!("Red Flag gate on {head}: success (No Red Flag waits for the maintainer)"),
            format!(
                "Review check on {head}: pending (Waiting for a Reviewer's Verdict on {short})"
            ),
        ],
        "{}",
        run.output
    );
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_run_whose_fetch_of_the_pr_s_commit_fails_leaves_both_statuses_at_error_never_an_older_result()
{
    // The Reviewer's case on #92: the fetch fails (here, because the commit
    // isn't there), after an earlier run had set both statuses to success.
    let scratch = Scratch::new("stops-early");
    scratch.pull_request_branch();
    scratch.answer(
        &format!("repos/{REPO}/pulls/77"),
        &pull_request(77, HEAD, "main"),
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(!run.passed, "{}", run.output);
    let failed = "error (The review workflow failed: see its run)";
    assert_eq!(
        run.statuses(),
        [
            format!("Red Flag gate on {HEAD}: pending (Working out the Red Flags)"),
            format!("Review check on {HEAD}: pending (Working out the Review check)"),
            format!("Red Flag gate on {HEAD}: {failed}"),
            format!("Review check on {HEAD}: {failed}"),
        ],
        "{}",
        run.output
    );
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pr_event_s_run_that_cannot_read_the_pr_from_github_marks_the_commit_it_named_with_error() {
    // GitHub's record of the PR doesn't come; the event named the PR's latest
    // commit and its base branch, main.
    let scratch = Scratch::new("pr-unreadable");
    scratch.fails(
        &format!("repos/{REPO}/pulls/77"),
        "gh: Server Error (HTTP 502)",
    );
    let run = scratch.run(
        "update",
        &[
            ("PR_NUMBER", "77"),
            ("EVENT_HEAD_SHA", HEAD),
            ("EVENT_BASE_REF", "main"),
            ("DEFAULT_BRANCH", "main"),
        ],
    );
    assert!(!run.passed, "{}", run.output);
    let failed = "error (The review workflow failed: see its run)";
    assert_eq!(
        run.statuses(),
        [
            format!("Red Flag gate on {HEAD}: {failed}"),
            format!("Review check on {HEAD}: {failed}"),
        ],
        "{}",
        run.output
    );
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_comment_s_run_that_cannot_read_the_pr_names_no_commit_so_only_its_failed_run_shows() {
    let scratch = Scratch::new("comment-pr-unreadable");
    scratch.fails(
        &format!("repos/{REPO}/pulls/77"),
        "gh: Server Error (HTTP 502)",
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77"), ("DEFAULT_BRANCH", "main")]);
    assert!(!run.passed, "{}", run.output);
    assert!(run.statuses().is_empty(), "{}", run.output);
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_pr_into_another_branch_never_sets_a_status_not_even_pending() {
    let scratch = Scratch::new("into-another-branch");
    let head = scratch.pull_request_branch();
    scratch.answer(
        &format!("repos/{REPO}/pulls/77"),
        &pull_request(77, &head, "ticket/41-thrust-stand"),
    );
    scratch.answer(
        &format!("repos/{REPO}/issues/77/comments?per_page=100"),
        &json!([]),
    );
    scratch.answer(
        &format!("repos/{REPO}/commits/{head}/pulls?per_page=100"),
        &json!([listed(77, "open", "ticket/41-thrust-stand", &head)]),
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(run.passed, "{}", run.output);
    assert!(run.statuses().is_empty(), "{}", run.output);
    assert!(
        run.stdout
            .contains("#77 merges into ticket/41-thrust-stand, not main, so it sets no statuses."),
        "{}",
        run.output
    );
}

// Triage after five failed review rounds (#120).

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn after_five_failed_rounds_the_issue_a_triage_comment_names_is_looked_up_and_an_open_one_passes() {
    let scratch = Scratch::new("triage-open-issue");
    let head = scratch.five_failed_rounds_and("Triaged to #200");
    scratch.answer(
        &format!("repos/{REPO}/issues/200"),
        &json!({
            "number": 200,
            "state": "open",
            "html_url": format!("https://github.com/{REPO}/issues/200"),
        }),
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(run.passed, "{}", run.output);
    let short = &head[..7];
    assert_eq!(
        run.statuses().last(),
        Some(&format!(
            "Review check on {head}: success (Triaged to #200 after 5 failed review rounds: \
             passes on {short})"
        )),
        "{}",
        run.output
    );
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn a_triage_comment_naming_an_issue_github_does_not_have_counts_for_nothing() {
    let scratch = Scratch::new("triage-missing-issue");
    let head = scratch.five_failed_rounds_and("Triaged to #200");
    scratch.fails(
        &format!("repos/{REPO}/issues/200"),
        "gh: Not Found (HTTP 404)",
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(run.passed, "{}", run.output);
    assert_eq!(
        run.statuses().last(),
        Some(&format!(
            "Review check on {head}: failure (5 review rounds failed: waiting for a triage \
             comment naming an open issue)"
        )),
        "{}",
        run.output
    );
    assert!(
        run.stdout
            .contains("Added the label `needs-triage` to #77."),
        "{}",
        run.output
    );
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn when_github_cannot_say_whether_the_triage_issue_is_open_both_statuses_show_an_error() {
    let scratch = Scratch::new("triage-issue-error");
    let head = scratch.five_failed_rounds_and("Triaged to #200");
    scratch.fails(
        &format!("repos/{REPO}/issues/200"),
        "gh: Server Error (HTTP 502)",
    );
    let run = scratch.run("update", &[("PR_NUMBER", "77")]);
    assert!(!run.passed, "{}", run.output);
    assert!(
        run.output
            .contains("GitHub couldn't say whether issue #200 is open."),
        "{}",
        run.output
    );
    let failed = "error (The review workflow failed: see its run)";
    assert_eq!(
        run.statuses(),
        [
            format!("Red Flag gate on {head}: pending (Working out the Red Flags)"),
            format!("Review check on {head}: pending (Working out the Review check)"),
            format!("Red Flag gate on {head}: {failed}"),
            format!("Review check on {head}: {failed}"),
        ],
        "{}",
        run.output
    );
}

// Which other PRs are judged again.

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn when_a_pr_closes_the_open_prs_into_main_with_its_latest_commit_are_judged_again() {
    let scratch = Scratch::new("closed");
    scratch.answer(
        &format!("repos/{REPO}/commits/{HEAD}/pulls?per_page=100"),
        &json!([
            // The PR that closed.
            listed(77, "closed", "main", HEAD),
            // Its latest commit is the same: judged again.
            listed(78, "open", "main", HEAD),
            // Stacked on the closed PR's branch: its latest commit is its own.
            listed(79, "open", "main", LATER),
            // Into another branch, so never judged.
            listed(80, "open", "ticket/41-thrust-stand", HEAD),
            listed(81, "closed", "main", HEAD),
        ]),
    );
    let run = scratch.run(
        "others",
        &[
            ("PR_NUMBER", "77"),
            ("DEFAULT_BRANCH", "main"),
            ("HEAD_SHA", HEAD),
        ],
    );
    assert!(run.passed, "{}", run.output);
    assert_eq!(run.outputs()["prs"], "[78]", "{}", run.output);
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn after_a_push_the_open_prs_into_main_whose_latest_commit_was_the_old_one_are_judged_again() {
    let scratch = Scratch::new("pushed");
    // GitHub lists the pushed PR for its old commit too, with its new one.
    scratch.answer(
        &format!("repos/{REPO}/commits/{OLDER}/pulls?per_page=100"),
        &json!([
            listed(77, "open", "main", HEAD),
            listed(78, "open", "main", OLDER),
        ]),
    );
    scratch.answer(
        &format!("repos/{REPO}/commits/{HEAD}/pulls?per_page=100"),
        &json!([listed(77, "open", "main", HEAD)]),
    );
    let run = scratch.run(
        "others",
        &[
            ("PR_NUMBER", "77"),
            ("DEFAULT_BRANCH", "main"),
            ("HEAD_SHA", HEAD),
            ("BEFORE_SHA", OLDER),
        ],
    );
    assert!(run.passed, "{}", run.output);
    assert_eq!(run.outputs()["prs"], "[78]", "{}", run.output);
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn with_no_other_pr_on_the_commit_nobody_is_judged_again() {
    // The old commit is gone after a force-push, so GitHub has no PRs for it.
    let scratch = Scratch::new("nobody");
    scratch.answer(
        &format!("repos/{REPO}/commits/{HEAD}/pulls?per_page=100"),
        &json!([listed(77, "open", "main", HEAD)]),
    );
    scratch.fails(
        &format!("repos/{REPO}/commits/{OLDER}/pulls?per_page=100"),
        &format!("gh: No commit found for SHA: {OLDER} (HTTP 422)"),
    );
    let run = scratch.run(
        "others",
        &[
            ("PR_NUMBER", "77"),
            ("DEFAULT_BRANCH", "main"),
            ("HEAD_SHA", HEAD),
            ("BEFORE_SHA", OLDER),
        ],
    );
    assert!(run.passed, "{}", run.output);
    assert_eq!(run.outputs()["prs"], "[]", "{}", run.output);
}

#[test]
#[cfg_attr(windows, ignore = "bash may not be Git Bash on Windows")]
fn when_github_cannot_say_which_prs_have_the_commit_the_job_fails_and_shows() {
    // Any error but "no such commit" fails the job, so it never passes
    // quietly while a PR stays stuck with the "keep one" failure.
    let scratch = Scratch::new("github-down");
    scratch.fails(
        &format!("repos/{REPO}/commits/{HEAD}/pulls?per_page=100"),
        "gh: Server Error (HTTP 502)",
    );
    let run = scratch.run(
        "others",
        &[
            ("PR_NUMBER", "77"),
            ("DEFAULT_BRANCH", "main"),
            ("HEAD_SHA", HEAD),
        ],
    );
    assert!(!run.passed, "{}", run.output);
    assert!(!run.outputs().contains_key("prs"), "{}", run.output);
    assert!(
        run.output.contains(
            "GitHub couldn't say which PRs have 1111111111111111111111111111111111111111"
        ),
        "{}",
        run.output
    );
}

/// GitHub's record of a pull request (`GET /repos/{owner}/{repo}/pulls/{n}`).
fn pull_request(number: u64, head: &str, base: &str) -> Value {
    json!({
        "number": number,
        "state": "open",
        "user": { "login": "BartoszSolkaBD" },
        "head": { "sha": head, "repo": { "full_name": REPO } },
        "base": {
            "ref": base,
            "repo": {
                "full_name": REPO,
                "html_url": format!("https://github.com/{REPO}"),
                "default_branch": "main",
            },
        },
        "labels": [],
    })
}

/// A comment from the maintainer's account, as GitHub lists it
/// (`GET …/issues/{n}/comments`).
fn maintainer_comment(id: u64, body: &str) -> Value {
    json!({
        "id": id,
        "user": { "login": "BartoszSolkaBD" },
        "body": body,
        "html_url": format!("https://github.com/{REPO}/pull/77#issuecomment-{id}"),
    })
}

/// A PR as GitHub lists it for a commit (`GET …/commits/{sha}/pulls`).
fn listed(number: u64, state: &str, base: &str, head: &str) -> Value {
    json!({ "number": number, "state": state, "base": { "ref": base }, "head": { "sha": head } })
}

/// A stand-in for `gh`: `gh api … <endpoint>` prints the answer saved for
/// that endpoint, or the error saved for it, as `gh` prints GitHub's errors.
/// An endpoint with neither fails too.
const FAKE_GH: &str = r#"#!/usr/bin/env bash
for arg in "$@"; do
  case "$arg" in
    repos/* | search/*) endpoint="$arg" ;;
  esac
done
file="$FAKE_GH_ANSWERS/$(printf '%s' "${endpoint:-none}" | tr -c 'A-Za-z0-9.-' '_')"
if [[ -f "$file" ]]; then
  cat "$file"
elif [[ -f "$file.error" ]]; then
  cat "$file.error" >&2
  exit 1
else
  echo "gh: no answer saved for ${endpoint:-none}" >&2
  exit 1
fi
"#;

/// A scratch folder: the stand-in for `gh` with its answers, and the
/// repositories.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("review-script")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).expect("can make the scratch folder");
        fs::create_dir_all(root.join("answers")).expect("can make the answers folder");
        let gh = root.join("bin").join("gh");
        fs::write(&gh, FAKE_GH).expect("can write the stand-in for gh");
        make_runnable(&gh);
        Scratch { root }
    }

    /// Saves GitHub's answer for an endpoint.
    fn answer(&self, endpoint: &str, answer: &Value) {
        fs::write(
            self.root.join("answers").join(file_name(endpoint)),
            answer.to_string(),
        )
        .expect("can write the answer");
    }

    /// Saves the error `gh` prints for an endpoint, such as
    /// "gh: Server Error (HTTP 502)".
    fn fails(&self, endpoint: &str, error: &str) {
        fs::write(
            self.root
                .join("answers")
                .join(format!("{}.error", file_name(endpoint))),
            error,
        )
        .expect("can write the error");
    }

    /// GitHub's copy of the repository, with main and a pull request's
    /// branch on it, and main checked out beside it as the workflow does.
    /// Returns the branch's latest commit.
    fn pull_request_branch(&self) -> String {
        let origin = self.root.join("origin");
        fs::create_dir_all(origin.join(".github")).expect("can make the folder");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/CODEOWNERS"),
            origin.join(".github/CODEOWNERS"),
        )
        .expect("can copy CODEOWNERS");
        fs::write(origin.join("README.md"), "# OpenDrone\n").expect("can write");
        git(&origin, &["init", "--quiet", "--initial-branch=main"]);
        git(&origin, &["add", "--all"]);
        git(&origin, &["commit", "--quiet", "--message", "main"]);
        // Another branch a PR could merge into.
        git(&origin, &["branch", "ticket/41-thrust-stand"]);
        git(&origin, &["checkout", "--quiet", "-b", "ticket/1-words"]);
        fs::write(origin.join("README.md"), "# OpenDrone\n\nMore words.\n").expect("can write");
        git(
            &origin,
            &["commit", "--quiet", "--all", "--message", "the change"],
        );
        let head = git(&origin, &["rev-parse", "HEAD"]);
        git(&origin, &["checkout", "--quiet", "main"]);
        git(&self.root, &["clone", "--quiet", "origin", "checkout"]);
        head
    }

    /// PR #77 into main, with five Verdicts of changes needed on its latest
    /// commit and then the maintainer's comment `last`. Returns that commit.
    fn five_failed_rounds_and(&self, last: &str) -> String {
        let head = self.pull_request_branch();
        self.answer(
            &format!("repos/{REPO}/pulls/77"),
            &pull_request(77, &head, "main"),
        );
        let mut comments: Vec<Value> = (0..5)
            .map(|i| {
                maintainer_comment(
                    100 + i,
                    &format!("Reviewed commit {head}\n\nVerdict: changes needed"),
                )
            })
            .collect();
        comments.push(maintainer_comment(105, last));
        self.answer(
            &format!("repos/{REPO}/issues/77/comments?per_page=100"),
            &json!(comments),
        );
        self.answer(
            &format!("repos/{REPO}/commits/{head}/pulls?per_page=100"),
            &json!([listed(77, "open", "main", &head)]),
        );
        head
    }

    /// Runs `review.sh <command>` in the checkout, with `DRY_RUN=1`.
    fn run(&self, command: &str, env: &[(&str, &str)]) -> Run {
        let checkout = self.root.join("checkout");
        fs::create_dir_all(&checkout).expect("can make the checkout folder");
        let outputs = self.root.join("github-output.txt");
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/scripts/review.sh");
        let path: OsString = std::env::join_paths(std::iter::once(self.root.join("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .expect("a PATH");
        let mut bash = Command::new("bash");
        bash.arg(script)
            .arg(command)
            .current_dir(&checkout)
            .env("PATH", path)
            .env("DRY_RUN", "1")
            .env("REPO", REPO)
            .env("XTASK", env!("CARGO_BIN_EXE_xtask"))
            .env(
                "RUN_URL",
                "https://github.com/BartoszSolkaBD/OpenDrone/actions/runs/1",
            )
            .env("RUNNER_TEMP", self.root.join("runner-temp"))
            .env("GITHUB_OUTPUT", &outputs)
            .env("FAKE_GH_ANSWERS", self.root.join("answers"))
            .env("GIT_CONFIG_GLOBAL", self.root.join("no-global-gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("BEFORE_SHA")
            .env_remove("HEAD_SHA");
        for (key, value) in env {
            bash.env(key, value);
        }
        let output = bash.output().expect("bash runs");
        Run {
            passed: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            outputs: fs::read_to_string(outputs).unwrap_or_default(),
        }
    }
}

/// The stand-in's file name for an endpoint's answer.
fn file_name(endpoint: &str) -> String {
    endpoint
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// What a run of the script did.
struct Run {
    passed: bool,
    stdout: String,
    /// Everything it printed, for messages.
    output: String,
    /// Its step outputs.
    outputs: String,
}

impl Run {
    /// Each status it set, in order, as it says it: "Red Flag gate on <commit>:
    /// pending (Working out the Red Flags)".
    fn statuses(&self) -> Vec<String> {
        self.stdout
            .lines()
            .filter(|line| {
                line.starts_with("Red Flag gate on ") || line.starts_with("Review check on ")
            })
            .map(str::to_string)
            .collect()
    }

    fn outputs(&self) -> BTreeMap<String, String> {
        self.outputs
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }
}

/// Runs git in `folder`, with git's own settings, as on a fresh CI runner,
/// and returns what it printed.
fn git(folder: &Path, args: &[&str]) -> String {
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
        .current_dir(folder)
        .env("GIT_CONFIG_GLOBAL", folder.join("no-global-gitconfig"))
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(unix)]
fn make_runnable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("can make it runnable");
}

#[cfg(not(unix))]
fn make_runnable(_path: &Path) {}
