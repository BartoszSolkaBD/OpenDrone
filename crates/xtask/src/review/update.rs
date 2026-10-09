//! What the review workflow posts: the Review Report comment, the PR's labels,
//! and the Red Flag gate's and the Review check's commit statuses.
//!
//! The Review Report's sections from Red Flags down are worked out by
//! `review-report`, in the same job, from main's code. Text from the pull
//! request in them is already escaped ([`super::markdown`]). Even so, they are
//! checked against the PR's latest commit, kept short, kept from passing for
//! this comment's marker or for a Verdict heading, and their Areas checked
//! against main's CODEOWNERS. The Verdict section is worked out here, from the
//! comments.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::markdown::code;
use super::verdict::{self, Comment, Issue, PullRequest, State};

/// The hidden first line that marks the Review Report comment.
pub const MARKER: &str = "<!-- opendrone-review-report -->";
/// The account CI posts as. Only its comments can be the Review Report.
pub const CI_ACCOUNT: &str = "github-actions[bot]";
/// The label for a PR that waits for the maintainer.
pub const NEEDS_MAINTAINER: &str = "needs-maintainer";
/// The label for a PR whose review rounds ran out, until a triage comment
/// moves what still blocks into its own issue.
pub const NEEDS_TRIAGE: &str = "needs-triage";
/// The label for a merged PR that skipped a check.
pub const SKIPPED_A_CHECK: &str = "skipped-a-check";
/// Area labels start with this, such as `area: Physics`.
pub const AREA_PREFIX: &str = "area: ";
/// The most Report text posted; a comment holds 65,536 characters.
const MOST_REPORT: usize = 60_000;

/// A label to make sure exists, and to add.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub colour: &'static str,
    pub description: String,
}

impl Label {
    pub fn needs_maintainer() -> Label {
        Label {
            name: NEEDS_MAINTAINER.to_string(),
            colour: "B60205",
            description: "Waits for the maintainer, such as for a Red Flag".to_string(),
        }
    }

    pub fn needs_triage() -> Label {
        Label {
            name: NEEDS_TRIAGE.to_string(),
            colour: "FBCA04",
            description: "Needs sorting; on a PR, five review rounds failed and what still blocks \
                          needs its own issue"
                .to_string(),
        }
    }

    pub fn skipped_a_check() -> Label {
        Label {
            name: SKIPPED_A_CHECK.to_string(),
            colour: "D93F0B",
            description: "Merged before every required check passed; Review Reports name it until \
                          this label is removed"
                .to_string(),
        }
    }

    pub fn area(area: &str) -> Label {
        Label {
            name: format!("{AREA_PREFIX}{area}"),
            colour: "C5DEF5",
            description: format!("Touches the {area} Area (.github/CODEOWNERS)"),
        }
    }

    pub fn json(&self) -> Value {
        json!({ "name": self.name, "color": self.colour, "description": self.description })
    }
}

/// The Report's sections from Red Flags down, once checked.
#[derive(Clone, Debug)]
pub enum Report {
    Ready {
        markdown: String,
        waits_for_maintainer: bool,
        areas: Vec<String>,
    },
    /// Not there, or not for the PR's latest commit: why, in plain words.
    Missing(String),
}

impl Report {
    /// Reads `review-report`'s folder (`report.md`, `review.json` and the
    /// `pr.json` naming the PR and commit it is for), if it is for this PR's
    /// latest commit.
    pub fn read(folder: Option<&Path>, pr: &PullRequest) -> Report {
        let head = verdict::short(&pr.head_sha);
        let missing = || {
            Report::Missing(format!(
                "The rest of this report couldn't be worked out for `{head}`. The Review \
                 workflow's run says why, and the next push or comment tries again."
            ))
        };
        let Some(folder) = folder else {
            return missing();
        };
        let read_json = |name: &str| -> Option<Value> {
            serde_json::from_str(&fs::read_to_string(folder.join(name)).ok()?).ok()
        };
        let (Some(about), Some(review), Ok(markdown)) = (
            read_json("pr.json"),
            read_json("review.json"),
            fs::read_to_string(folder.join("report.md")),
        ) else {
            return missing();
        };
        let number = about.get("number").and_then(Value::as_u64);
        let sha = about.get("head_sha").and_then(Value::as_str);
        if number != Some(pr.number) || sha != Some(pr.head_sha.as_str()) {
            return missing();
        }
        Report::Ready {
            markdown: sanitise(&markdown),
            waits_for_maintainer: review
                .get("waits_for_maintainer")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            areas: review
                .get("areas")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        }
    }
}

/// Report text made safe to post as part of the comment: nothing that could
/// pass for the comment's hidden marker, no heading that could pass for the
/// Verdict section, and short enough for one comment.
fn sanitise(markdown: &str) -> String {
    let mut text: String = markdown
        .replace("<!--", "&lt;!--")
        .lines()
        .map(|line| {
            let heading = line.trim_start().starts_with('#');
            if heading && line.to_lowercase().contains("verdict") {
                format!("\\{}", line.trim_start())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.len() > MOST_REPORT {
        let mut cut = MOST_REPORT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n\n…the rest is cut off: it was too long for one comment.\n");
    }
    text
}

/// A merged PR that skipped a check, from GitHub's search.
#[derive(Clone, Debug)]
pub struct SkippedMerge {
    pub number: u64,
    pub title: String,
    pub url: String,
}

impl SkippedMerge {
    /// Reads GitHub's search results (`GET /search/issues`).
    pub fn list_from_api(search: &Value) -> Vec<SkippedMerge> {
        search
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                Some(SkippedMerge {
                    number: item.get("number")?.as_u64()?,
                    title: item.get("title")?.as_str()?.to_string(),
                    url: item.get("html_url")?.as_str()?.to_string(),
                })
            })
            .collect()
    }
}

/// A PR that may share the head commit, from GitHub's list of the commit's
/// PRs (`GET /repos/{owner}/{repo}/commits/{sha}/pulls`). That list holds
/// every PR that *contains* the commit, such as a PR stacked on this one's
/// branch, so only one whose latest commit is this same commit shares it.
#[derive(Clone, Debug)]
pub struct SharingPr {
    pub number: u64,
    pub open: bool,
    pub base_ref: String,
    /// Its latest commit, or none if GitHub's record leaves it out.
    pub head_sha: Option<String>,
}

impl SharingPr {
    pub fn list_from_api(pulls: &Value) -> Vec<SharingPr> {
        pulls
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|pr| {
                Some(SharingPr {
                    number: pr.get("number")?.as_u64()?,
                    open: pr.get("state").and_then(Value::as_str) == Some("open"),
                    base_ref: pr.pointer("/base/ref")?.as_str()?.to_string(),
                    head_sha: pr
                        .pointer("/head/sha")
                        .and_then(Value::as_str)
                        .map(str::to_ascii_lowercase),
                })
            })
            .collect()
    }

    /// Whether this other PR shares `pr`'s latest commit, so the commit's
    /// statuses would have to carry both: it is open, it merges into the
    /// default branch too, and its latest commit is the same. A record with no
    /// latest commit counts as sharing, so a gap in GitHub's answer fails
    /// closed.
    pub fn shares_with(&self, pr: &PullRequest) -> bool {
        self.open
            && self.number != pr.number
            && self.base_ref == pr.default_branch
            && self
                .head_sha
                .as_deref()
                .is_none_or(|sha| sha == pr.head_sha.to_ascii_lowercase())
    }
}

/// A commit status to set on the PR's latest commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub context: &'static str,
    pub state: State,
    pub description: String,
}

/// The names of the two statuses.
pub const GATE: &str = "Red Flag gate";
pub const REVIEW_CHECK: &str = "Review check";

/// Everything the review workflow posts.
#[derive(Clone, Debug)]
pub struct Update {
    /// The commit statuses to set: the Red Flag gate and the Review check,
    /// or none for a PR that doesn't merge into the default branch.
    pub statuses: Vec<Status>,
    pub comment: String,
    /// The Review Report comment to edit, or none yet.
    pub comment_id: Option<u64>,
    pub add: Vec<Label>,
    pub remove: Vec<String>,
}

/// Works out the update. `known_areas` are the Areas in main's CODEOWNERS:
/// only those become labels. `sharing` are the PRs GitHub lists for the head
/// commit, this one included or not. `issues` are GitHub's records of the
/// issues its triage comments name.
///
/// A commit status belongs to a commit, not to a PR, so:
/// - only a PR into the default branch gets the two statuses; one into any
///   other branch is reported on but never sets them;
/// - if another open PR into the default branch has the same latest commit,
///   both statuses fail, since one commit can't carry two PRs' results. A PR
///   stacked on this one's branch has a later commit of its own, so it
///   doesn't count.
pub fn update(
    pr: &PullRequest,
    comments: &[Comment],
    report: &Report,
    skipped: &[SkippedMerge],
    known_areas: &[String],
    sharing: &[SharingPr],
    issues: &[Issue],
) -> Update {
    let check = verdict::review_check(pr, comments, issues);
    let gate = match report {
        Report::Ready {
            waits_for_maintainer: true,
            ..
        } => (
            State::Failure,
            "A Red Flag waits for the maintainer: see the Review Report".to_string(),
        ),
        Report::Ready { .. } => (
            State::Success,
            "No Red Flag waits for the maintainer".to_string(),
        ),
        Report::Missing(_) => (
            State::Error,
            "The Red Flags couldn't be worked out: see the Review workflow's run".to_string(),
        ),
    };

    let others: Vec<String> = sharing
        .iter()
        .filter(|p| p.shares_with(pr))
        .map(|p| format!("#{}", p.number))
        .collect();
    let (statuses, note) = if !pr.merges_into_default_branch() {
        (
            Vec::new(),
            Some(format!(
                "**Not judged:** this PR merges into {}, not {}, so CI sets neither the Red Flag \
                 gate nor the Review check on its commits. Only PRs into {} are judged; the Red \
                 Flags below are against {}.",
                code(&pr.base_ref),
                code(&pr.default_branch),
                code(&pr.default_branch),
                code(&pr.base_ref)
            )),
        )
    } else if !others.is_empty() {
        let description = format!(
            "Other open PRs into {} share this commit ({}): keep one",
            pr.default_branch,
            others.join(", ")
        );
        let description: String = description.chars().take(140).collect();
        (
            vec![
                Status {
                    context: GATE,
                    state: State::Failure,
                    description: description.clone(),
                },
                Status {
                    context: REVIEW_CHECK,
                    state: State::Failure,
                    description,
                },
            ],
            Some(format!(
                "**Both checks fail:** other open PRs into {} have this same latest commit ({}). \
                 A commit's checks can carry only one PR's results, so close the others or push \
                 a new commit to them. The PR left with this commit is then judged again.",
                code(&pr.default_branch),
                others.join(", ")
            )),
        )
    } else {
        (
            vec![
                Status {
                    context: GATE,
                    state: gate.0,
                    description: gate.1,
                },
                Status {
                    context: REVIEW_CHECK,
                    state: check.state,
                    description: check.description.clone(),
                },
            ],
            None,
        )
    };

    let mut comment = String::new();
    let _ = writeln!(
        comment,
        "{MARKER}\n## Review Report\n\n### Verdict\n\n{}\n",
        check.report
    );
    if let Some(note) = note {
        let _ = writeln!(comment, "{note}\n");
    }
    if !skipped.is_empty() {
        let merges: Vec<String> = skipped
            .iter()
            .map(|m| {
                format!(
                    "[#{}]({}) ({})",
                    m.number,
                    m.url,
                    m.title.replace(['[', ']', '<', '>'], "")
                )
            })
            .collect();
        let _ = writeln!(
            comment,
            "**Warning: earlier merges skipped a check:** {}. Each keeps its `{SKIPPED_A_CHECK}` \
             label until the maintainer takes it off.\n",
            merges.join(", ")
        );
    }
    match report {
        Report::Ready { markdown, .. } => {
            let _ = writeln!(comment, "{}", markdown.trim_end());
        }
        Report::Missing(why) => {
            let _ = writeln!(comment, "_{why}_");
        }
    }
    let guide = if pr.repo_url.is_empty() {
        "docs/review-report.md".to_string()
    } else {
        format!(
            "[how to read it]({}/blob/main/docs/review-report.md)",
            pr.repo_url
        )
    };
    let _ = writeln!(
        comment,
        "\n---\n<sub>CI keeps this comment up to date: the Verdict part whenever a comment \
         arrives, the rest too, and after every push. It covers commit `{}`; {guide}.</sub>",
        verdict::short(&pr.head_sha)
    );

    let comment_id = comments
        .iter()
        .find(|c| c.author == CI_ACCOUNT && c.body.starts_with(MARKER))
        .map(|c| c.id);

    let mut add = Vec::new();
    let mut remove = Vec::new();
    let has = |name: &str| pr.labels.iter().any(|l| l == name);
    let waits = matches!(
        report,
        Report::Ready {
            waits_for_maintainer: true,
            ..
        }
    );
    if waits && !has(NEEDS_MAINTAINER) {
        add.push(Label::needs_maintainer());
    }
    // Five failed rounds wait for triage, not for the maintainer; the label
    // comes off once a triage comment counts.
    if check.waits_for_triage() && !has(NEEDS_TRIAGE) {
        add.push(Label::needs_triage());
    }
    if check.triaged_to.is_some() && has(NEEDS_TRIAGE) {
        remove.push(NEEDS_TRIAGE.to_string());
    }
    if let Report::Ready { areas, .. } = report {
        let wanted: Vec<&String> = areas.iter().filter(|a| known_areas.contains(a)).collect();
        for area in &wanted {
            let label = Label::area(area);
            if !pr.labels.contains(&label.name) {
                add.push(label);
            }
        }
        for label in &pr.labels {
            if let Some(area) = label.strip_prefix(AREA_PREFIX)
                && !wanted.iter().any(|w| w.as_str() == area)
            {
                remove.push(label.clone());
            }
        }
    }
    Update {
        statuses,
        comment,
        comment_id,
        add,
        remove,
    }
}
