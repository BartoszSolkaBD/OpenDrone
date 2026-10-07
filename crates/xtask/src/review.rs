//! The Review Report, the Red Flag gate and the Review check (#77; #15 §3,
//! §5 and §10; [ADR-0010]).
//!
//! CI runs these commands in one workflow, `.github/workflows/review.yml`,
//! built from main's code. They read a pull request only as data: its commits
//! through `git show` and `git diff`, and its comments through GitHub's API.
//! Nothing from the pull request is ever run, built or checked out.
//!
//! - `review-report` compares a pull request's base and head and writes the
//!   Review Report's sections from Red Flags to Downloads (`report.md`), with
//!   `review.json` for the labels and the Red Flag gate. It fails when a Red
//!   Flag waits for the maintainer.
//! - `new-libraries` lists the outside libraries a pull request adds, so the
//!   workflow can look up their licences.
//! - `review-update` works out the Review check from the PR and its comments,
//!   and the Review Report comment, labels and both commit statuses to post.
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
mod markdown;
mod merges;
mod report;
mod results;
mod rust;
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
use update::{Report, SharingPr, SkippedMerge};
use verdict::{Comment, PullRequest};

pub const REPORT_USAGE: &str = "\
  review-report (--base <commit> --head <commit> | --before <folder> --after <folder>)
                [--metadata <cargo-metadata.json>] [--codeowners <file>] [--out <folder>]
      Work out the Review Report's sections from Red Flags to Downloads for
      the changes between two commits (or two copies of the repo), and fail
      if a Red Flag waits for the maintainer (the Red Flag gate). The Areas
      come from --codeowners (CI gives main's), else from the base's. With
      --out, write report.md and review.json there.";

pub const NEW_LIBRARIES_USAGE: &str = "\
  new-libraries --base <commit> --head <commit>
      Print each outside library the head's Cargo.lock adds, as \"name version\".";

pub const UPDATE_USAGE: &str = "\
  review-update --pr <pr.json> --comments <comments.json> --codeowners <file>
                --out <folder> [--report <folder>] [--skipped-merges <search.json>]
                [--head-pulls <commit-pulls.json>]
      Work out the Review check, the Review Report comment, the PR's labels
      and the commit statuses to set (statuses.json) from GitHub's records,
      and write them to the --out folder.";

pub const MERGE_USAGE: &str = "\
  merge-check --branch <branch.json> --rules <rules.json>
              --check-runs <check-runs.json> --status <status.json> [--out <folder>]
      Print each required check a merged PR's last commit hadn't passed. With
      --out, if any, write the comment for that PR and its label there.";

/// `cargo xtask review-report`.
pub fn run_report(args: &[String]) -> ExitCode {
    let result = options(
        args,
        &[
            "base",
            "head",
            "before",
            "after",
            "metadata",
            "codeowners",
            "out",
        ],
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
        let codeowners = match options.get("codeowners") {
            Some(path) => Some(read_text(path)?),
            None => None,
        };
        let review = Review::of(&changes, metadata.as_ref(), codeowners.as_deref());
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

/// `cargo xtask new-libraries`.
pub fn run_new_libraries(args: &[String]) -> ExitCode {
    let result = options(args, &["base", "head"]).and_then(|options| {
        let (Some(base), Some(head)) = (options.get("base"), options.get("head")) else {
            return Err("give --base and --head".to_string());
        };
        let changes = Changes::between_commits(base, head)?;
        if !changes.touches("Cargo.lock") {
            return Ok(Vec::new());
        }
        Ok(libraries::new_libraries(
            &libraries::read_lock(&changes.base.text("Cargo.lock").unwrap_or_default()),
            &libraries::read_lock(&changes.head.text("Cargo.lock").unwrap_or_default()),
            None,
        ))
    });
    match result {
        Ok(new) => {
            for library in new {
                println!("{} {}", library.name, library.version);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("new-libraries: {error}\n\nUsage:\n{NEW_LIBRARIES_USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Every review command's usage, for `cargo xtask` with no command.
pub fn usage() -> String {
    [REPORT_USAGE, NEW_LIBRARIES_USAGE, UPDATE_USAGE, MERGE_USAGE].join("\n")
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
            "head-pulls",
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
        let sharing = match options.get("head-pulls") {
            Some(path) => SharingPr::list_from_api(&read_json(path)?),
            None => Vec::new(),
        };
        let report = Report::read(options.get("report").map(Path::new), &pr);
        let update = update::update(&pr, &comments, &report, &skipped, &known_areas, &sharing);

        let out = PathBuf::from(need("out")?);
        fs::create_dir_all(&out)
            .map_err(|error| format!("couldn't make {}: {error}", out.display()))?;
        write(
            &out.join("statuses.json"),
            &pretty(&Value::Array(
                update
                    .statuses
                    .iter()
                    .map(|s| {
                        json!({
                            "context": s.context,
                            "state": s.state.word(),
                            "description": s.description,
                        })
                    })
                    .collect(),
            )),
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
            if update.statuses.is_empty() {
                println!("Sets no statuses: the PR doesn't merge into the default branch.");
            }
            for status in &update.statuses {
                println!(
                    "{}: {} ({})",
                    status.context,
                    status.state.word(),
                    status.description
                );
            }
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
