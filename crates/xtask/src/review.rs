//! The Review Report, the Red Flag gate and the Review check (#77; #15 §3,
//! §5 and §10; [ADR-0010]).
//!
//! Three commands, each run by a CI workflow:
//!
//! - `review-report` compares a pull request's base and head and writes the
//!   Review Report's sections from Red Flags to Downloads (`report.md`), with
//!   `review.json` for the labels. It fails when a Red Flag waits for the
//!   maintainer: that is the Red Flag gate. It reads the pull request's files
//!   as data and runs without write access
//!   (`.github/workflows/review-report.yml`).
//! - `review-update` works out the Review check from the PR and its comments,
//!   and the Review Report comment and labels to post. It runs in the
//!   privileged workflow, built from main (`.github/workflows/review.yml`).
//! - `merge-check` names every required check a just-merged PR hadn't passed,
//!   so later Review Reports can name that merge.
//!
//! The rules are in `docs/context/development.md`; how to read the Report is
//! in `docs/review-report.md`; what the Reviewer reads and the Verdict format
//! are in `docs/agents/reviewer.md`.
//!
//! [ADR-0010]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0010-phase-1-agent-prs-merge-automatically.md

mod areas;
mod changes;
mod flags;
mod libraries;
mod merges;
mod report;
mod results;
mod scenarios;
mod update;
mod verdict;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

use changes::Changes;
use flags::Level;
use report::Review;
use update::{Report, SkippedMerge};
use verdict::{Comment, PullRequest};

pub const REPORT_USAGE: &str = "\
  review-report (--base <commit> --head <commit> | --before <folder> --after <folder>)
                [--metadata <cargo-metadata.json>] [--out <folder>]
      Work out the Review Report's sections from Red Flags to Downloads for
      the changes between two commits (or two copies of the repo), and fail
      if a Red Flag waits for the maintainer (the Red Flag gate). With --out,
      write report.md and review.json there.";

pub const UPDATE_USAGE: &str = "\
  review-update --pr <pr.json> --comments <comments.json> --codeowners <file>
                --out <folder> [--report <folder>] [--skipped-merges <search.json>]
      Work out the Review check, the Review Report comment and the PR's
      labels from GitHub's records, and write them to the --out folder.";

pub const MERGE_USAGE: &str = "\
  merge-check --branch <branch.json> --rules <rules.json>
              --check-runs <check-runs.json> --status <status.json> [--out <folder>]
      Print each required check a merged PR's last commit hadn't passed. With
      --out, if any, write the comment for that PR and its label there.";

/// `cargo xtask review-report`.
pub fn run_report(args: &[String]) -> ExitCode {
    let result = options(
        args,
        &["base", "head", "before", "after", "metadata", "out"],
    )
    .and_then(|options| {
        let changes = match (
            options.get("base"),
            options.get("head"),
            options.get("before"),
            options.get("after"),
        ) {
            (Some(base), Some(head), None, None) => Changes::between_commits(base, head)?,
            (None, None, Some(before), Some(after)) => {
                Changes::between_folders(Path::new(before), Path::new(after))?
            }
            _ => return Err("give either --base and --head, or --before and --after".into()),
        };
        let metadata = match options.get("metadata") {
            Some(path) => Some(read_json(path)?),
            None => None,
        };
        let review = Review::of(&changes, metadata.as_ref());
        if let Some(out) = options.get("out") {
            let out = PathBuf::from(out);
            fs::create_dir_all(&out)
                .map_err(|error| format!("couldn't make {}: {error}", out.display()))?;
            write(&out.join("report.md"), &review.markdown())?;
            write(&out.join("review.json"), &pretty(&review.json()))?;
        }
        Ok(review)
    });
    let review = match result {
        Ok(review) => review,
        Err(error) => {
            eprintln!("review-report: {error}\n\nUsage:\n{REPORT_USAGE}");
            return ExitCode::from(2);
        }
    };
    for level in Level::ALL {
        let flags: Vec<_> = review.flags.iter().filter(|f| f.level == level).collect();
        if flags.is_empty() {
            continue;
        }
        println!(
            "{}:",
            match level {
                Level::WaitsForMaintainer => "Red Flags that wait for the maintainer",
                Level::ReviewerDecides => "Red Flags for the Reviewer to decide",
                Level::ListedOnly => "Red Flags listed only",
            }
        );
        for flag in flags {
            println!("  - {}: {}", flag.title, flag.detail);
        }
    }
    if review.waits_for_maintainer() {
        println!(
            "The Red Flag gate fails: a Red Flag waits for the maintainer, so this PR gets the \
             `needs-maintainer` label."
        );
        ExitCode::FAILURE
    } else {
        println!("The Red Flag gate passes: no Red Flag waits for the maintainer.");
        ExitCode::SUCCESS
    }
}

/// `cargo xtask review-update`.
pub fn run_update(args: &[String]) -> ExitCode {
    let result = options(
        args,
        &[
            "pr",
            "comments",
            "codeowners",
            "out",
            "report",
            "skipped-merges",
        ],
    )
    .and_then(|options| {
        let need = |key: &str| {
            options
                .get(key)
                .cloned()
                .ok_or_else(|| format!("--{key} is needed"))
        };
        let pr = PullRequest::from_api(&read_json(&need("pr")?)?)?;
        let comments = Comment::list_from_api(&read_text(&need("comments")?)?)?;
        let known_areas = areas::Areas::parse(&read_text(&need("codeowners")?)?).names();
        let skipped = match options.get("skipped-merges") {
            Some(path) => SkippedMerge::list_from_api(&read_json(path)?),
            None => Vec::new(),
        };
        let report = Report::read(options.get("report").map(Path::new), &pr);
        let update = update::update(&pr, &comments, &report, &skipped, &known_areas);

        let out = PathBuf::from(need("out")?);
        fs::create_dir_all(&out)
            .map_err(|error| format!("couldn't make {}: {error}", out.display()))?;
        write(
            &out.join("status.json"),
            &pretty(&json!({
                "state": update.check.state.word(),
                "description": update.check.description,
            })),
        )?;
        write(&out.join("comment.md"), &update.comment)?;
        write(
            &out.join("comment-id"),
            &update
                .comment_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        )?;
        write(
            &out.join("labels-add.json"),
            &pretty(&Value::Array(update.add.iter().map(|l| l.json()).collect())),
        )?;
        write(
            &out.join("labels-remove.json"),
            &pretty(&json!(update.remove)),
        )?;
        Ok(update)
    });
    match result {
        Ok(update) => {
            println!(
                "Review check: {} ({})",
                update.check.state.word(),
                update.check.description
            );
            for label in &update.add {
                println!("Adds the label `{}`", label.name);
            }
            for label in &update.remove {
                println!("Takes off the label `{label}`");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("review-update: {error}\n\nUsage:\n{UPDATE_USAGE}");
            ExitCode::from(2)
        }
    }
}

/// `cargo xtask merge-check`.
pub fn run_merge_check(args: &[String]) -> ExitCode {
    let result =
        options(args, &["branch", "rules", "check-runs", "status", "out"]).and_then(|options| {
            let need = |key: &str| {
                options
                    .get(key)
                    .cloned()
                    .ok_or_else(|| format!("--{key} is needed"))
            };
            let required = merges::required_checks(
                &read_json(&need("branch")?)?,
                &read_json(&need("rules")?)?,
            );
            let skipped = merges::skipped_checks(
                &required,
                &read_text(&need("check-runs")?)?,
                &read_json(&need("status")?)?,
            );
            if let (Some(out), false) = (options.get("out"), skipped.is_empty()) {
                let out = PathBuf::from(out);
                fs::create_dir_all(&out)
                    .map_err(|error| format!("couldn't make {}: {error}", out.display()))?;
                write(&out.join("comment.md"), &merges::comment(&skipped))?;
                write(
                    &out.join("label.json"),
                    &pretty(&update::Label::skipped_a_check().json()),
                )?;
            }
            Ok(skipped)
        });
    match result {
        Ok(skipped) => {
            for check in skipped {
                println!("{}: {}", check.check, check.state);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("merge-check: {error}\n\nUsage:\n{MERGE_USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Reads `--name value` pairs, refusing any name not in `allowed`.
fn options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut options = BTreeMap::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(name) = arg.strip_prefix("--").filter(|name| allowed.contains(name)) else {
            return Err(format!("\"{arg}\" isn't an option this command takes"));
        };
        let value = args
            .next()
            .ok_or_else(|| format!("--{name} needs a value"))?;
        options.insert(name.to_string(), value.clone());
    }
    Ok(options)
}

fn read_text(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("couldn't read {path}: {error}"))
}

fn read_json(path: &str) -> Result<Value, String> {
    serde_json::from_str(&read_text(path)?).map_err(|error| format!("{path} isn't JSON: {error}"))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("couldn't write {}: {error}", path.display()))
}

fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}
