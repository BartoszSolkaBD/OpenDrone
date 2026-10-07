//! What the privileged workflow posts: the Review Report comment, the PR's
//! labels and the Review check's commit status.
//!
//! The Review Report's sections from Red Flags down come from the unprivileged
//! workflow's download, which a pull request could shape, so they are only
//! ever text: checked against the PR's latest commit, kept short, stripped of
//! anything that could pass for this comment's marker, and their Areas checked
//! against main's CODEOWNERS. The Verdict section is worked out here, from the
//! comments.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::verdict::{self, Comment, PullRequest, ReviewCheck};

/// The hidden first line that marks the Review Report comment.
pub const MARKER: &str = "<!-- opendrone-review-report -->";
/// The account CI posts as. Only its comments can be the Review Report.
pub const CI_ACCOUNT: &str = "github-actions[bot]";
/// The label for a PR that waits for the maintainer.
pub const NEEDS_MAINTAINER: &str = "needs-maintainer";
/// The label for a merged PR that skipped a check.
pub const SKIPPED_A_CHECK: &str = "skipped-a-check";
/// Area labels start with this, such as `area: Physics`.
pub const AREA_PREFIX: &str = "area: ";
/// The most Report text taken from the download; a comment holds 65,536.
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
            description: "Waits for the maintainer: a Red Flag, or three failed review rounds"
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

/// The Report sections from the unprivileged workflow, once checked.
#[derive(Clone, Debug)]
pub enum Report {
    Ready {
        markdown: String,
        waits_for_maintainer: bool,
        areas: Vec<String>,
    },
    /// Not there, or not for the PR's latest commit: why, in plain words.
    NotYet(String),
}

impl Report {
    /// Reads the download folder (`report.md`, `review.json`, `pr.json`), if
    /// it is for this PR's latest commit.
    pub fn read(folder: Option<&Path>, pr: &PullRequest) -> Report {
        let head = verdict::short(&pr.head_sha);
        let Some(folder) = folder else {
            return Report::NotYet(format!(
                "The rest of this report appears when the Review Report workflow has finished \
                 for `{head}`."
            ));
        };
        let read_json = |name: &str| -> Option<Value> {
            serde_json::from_str(&fs::read_to_string(folder.join(name)).ok()?).ok()
        };
        let (Some(about), Some(review), Ok(markdown)) = (
            read_json("pr.json"),
            read_json("review.json"),
            fs::read_to_string(folder.join("report.md")),
        ) else {
            return Report::NotYet(format!(
                "The Review Report workflow didn't finish its report for `{head}`; its run on \
                 the PR's Checks tab says why."
            ));
        };
        let number = about.get("number").and_then(Value::as_u64);
        let sha = about.get("head_sha").and_then(Value::as_str);
        if number != Some(pr.number) || sha != Some(pr.head_sha.as_str()) {
            return Report::NotYet(format!(
                "The rest of this report appears when the Review Report workflow has finished \
                 for `{head}`."
            ));
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

/// Report text from a download, made safe to post: no comment markers, and
/// short enough for one comment.
fn sanitise(markdown: &str) -> String {
    let mut text = markdown.replace("<!--", "&lt;!--");
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

/// Everything the privileged workflow posts.
#[derive(Clone, Debug)]
pub struct Update {
    pub check: ReviewCheck,
    pub comment: String,
    /// The Review Report comment to edit, or none yet.
    pub comment_id: Option<u64>,
    pub add: Vec<Label>,
    pub remove: Vec<String>,
}

/// Works out the update. `known_areas` are the Areas in main's CODEOWNERS:
/// only those become labels.
pub fn update(
    pr: &PullRequest,
    comments: &[Comment],
    report: &Report,
    skipped: &[SkippedMerge],
    known_areas: &[String],
) -> Update {
    let check = verdict::review_check(pr, comments);

    let mut comment = String::new();
    let _ = writeln!(
        comment,
        "{MARKER}\n## Review Report\n\n### Verdict\n\n{}\n",
        check.report
    );
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
        Report::NotYet(why) => {
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
         arrives, the rest after every push. It covers commit `{}`; {guide}.</sub>",
        verdict::short(&pr.head_sha)
    );

    let comment_id = comments
        .iter()
        .find(|c| c.author == CI_ACCOUNT && c.body.starts_with(MARKER))
        .map(|c| c.id);

    let mut add = Vec::new();
    let mut remove = Vec::new();
    let waits = matches!(
        report,
        Report::Ready {
            waits_for_maintainer: true,
            ..
        }
    ) || check.failed_rounds >= verdict::ROUNDS;
    if waits && !pr.labels.iter().any(|l| l == NEEDS_MAINTAINER) {
        add.push(Label::needs_maintainer());
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
        check,
        comment,
        comment_id,
        add,
        remove,
    }
}
