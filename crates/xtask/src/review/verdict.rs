//! Verdicts and the Review check (#15 §3, ADR-0010).
//!
//! A Verdict is a PR comment from the maintainer's account (agents work under
//! it) whose first line is `Reviewed commit <full SHA>` and whose last line is
//! exactly `Verdict: pass` or `Verdict: changes needed`. The Review check
//! passes only when:
//!
//! - the PR was opened by the maintainer's account or by Dependabot, from a
//!   branch in this repo;
//! - fewer than 3 review rounds (Verdicts of changes needed) have failed;
//! - the newest Verdict covers the PR's latest commit, and says pass.

use serde_json::Value;

/// The maintainer's GitHub account. Agents work under it in Phase 1, so it
/// posts the Verdicts, and only its Verdicts count.
pub const MAINTAINER: &str = "BartoszSolkaBD";
/// Dependabot's account, whose PRs may pass the Review check too.
pub const DEPENDABOT: &str = "dependabot[bot]";
/// After this many failed review rounds, the PR waits for the maintainer.
pub const ROUNDS: usize = 3;

/// What the PR's API record says, as far as the Review check needs it.
#[derive(Clone, Debug)]
pub struct PullRequest {
    pub number: u64,
    pub author: String,
    /// The PR's latest commit, in full.
    pub head_sha: String,
    /// Whether its branch is in this repo rather than a fork.
    pub from_this_repo: bool,
    pub labels: Vec<String>,
    /// The repo's web address, such as `https://github.com/owner/repo`.
    pub repo_url: String,
    /// The branch the PR merges into.
    pub base_ref: String,
    /// The repo's default branch, main.
    pub default_branch: String,
}

impl PullRequest {
    /// Whether the PR merges into the default branch: only those PRs are
    /// judged, because a commit status belongs to a commit, whatever PR it
    /// is in.
    pub fn merges_into_default_branch(&self) -> bool {
        !self.default_branch.is_empty() && self.base_ref == self.default_branch
    }
}

impl PullRequest {
    /// Reads GitHub's pull request record (`GET /repos/{owner}/{repo}/pulls/{n}`).
    pub fn from_api(pr: &Value) -> Result<PullRequest, String> {
        let text = |pointer: &str| {
            pr.pointer(pointer)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let head_sha = text("/head/sha").ok_or("the pull request has no head commit")?;
        if head_sha.len() != 40 || !head_sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("\"{head_sha}\" isn't a full commit SHA"));
        }
        let base_repo = text("/base/repo/full_name");
        let head_repo = text("/head/repo/full_name");
        Ok(PullRequest {
            number: pr
                .get("number")
                .and_then(Value::as_u64)
                .ok_or("the pull request has no number")?,
            author: text("/user/login").unwrap_or_default(),
            head_sha,
            from_this_repo: base_repo.is_some() && head_repo == base_repo,
            labels: pr
                .get("labels")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|label| label.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect(),
            repo_url: text("/base/repo/html_url").unwrap_or_default(),
            base_ref: text("/base/ref").unwrap_or_default(),
            default_branch: text("/base/repo/default_branch").unwrap_or_default(),
        })
    }
}

/// One PR comment.
#[derive(Clone, Debug)]
pub struct Comment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub url: String,
}

impl Comment {
    /// Reads GitHub's comment records (`GET /repos/{owner}/{repo}/issues/{n}/comments`),
    /// in the order GitHub lists them: oldest first. Several pages, one after
    /// another, are read as one list.
    pub fn list_from_api(text: &str) -> Result<Vec<Comment>, String> {
        let mut comments = Vec::new();
        for page in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
            let page = page.map_err(|error| format!("the comments aren't JSON: {error}"))?;
            let entries = match page {
                Value::Array(entries) => entries,
                other => vec![other],
            };
            for entry in entries {
                comments.push(Comment {
                    id: entry.get("id").and_then(Value::as_u64).unwrap_or_default(),
                    author: entry
                        .pointer("/user/login")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    body: entry
                        .get("body")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    url: entry
                        .get("html_url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                });
            }
        }
        Ok(comments)
    }
}

/// One Verdict: the commit it reviewed, and whether it says pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub commit: String,
    pub pass: bool,
    pub url: String,
}

/// The Verdict in a comment's text, if it is one: the first line names the
/// full commit, and the last line is exactly `Verdict: pass` or
/// `Verdict: changes needed`.
pub fn read_verdict(body: &str) -> Option<(String, bool)> {
    let mut lines = body
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty());
    let commit = lines.next()?.strip_prefix("Reviewed commit ")?;
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pass = match lines.next_back()? {
        "Verdict: pass" => true,
        "Verdict: changes needed" => false,
        _ => return None,
    };
    Some((commit.to_ascii_lowercase(), pass))
}

/// Every Verdict on the PR, oldest first. Only the maintainer's account posts
/// Verdicts; anyone else's comment is information, never a Verdict.
pub fn verdicts(comments: &[Comment]) -> Vec<Verdict> {
    comments
        .iter()
        .filter(|comment| comment.author == MAINTAINER)
        .filter_map(|comment| {
            let (commit, pass) = read_verdict(&comment.body)?;
            Some(Verdict {
                commit,
                pass,
                url: comment.url.clone(),
            })
        })
        .collect()
}

/// A commit status, as GitHub shows the Review check and the Red Flag gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Success,
    /// Waiting for a Verdict on the latest commit.
    Pending,
    Failure,
    /// The check couldn't be worked out.
    Error,
}

impl State {
    /// The word GitHub's commit status takes.
    pub fn word(self) -> &'static str {
        match self {
            State::Success => "success",
            State::Pending => "pending",
            State::Failure => "failure",
            State::Error => "error",
        }
    }
}

/// The Review check, worked out.
#[derive(Clone, Debug)]
pub struct ReviewCheck {
    pub state: State,
    /// The status's one-line description, at most 140 characters.
    pub description: String,
    /// The Review Report's Verdict section, in Markdown.
    pub report: String,
    /// How many review rounds have failed.
    pub failed_rounds: usize,
}

/// Works out the Review check from the PR and its comments.
pub fn review_check(pr: &PullRequest, comments: &[Comment]) -> ReviewCheck {
    let verdicts = verdicts(comments);
    let failed_rounds = verdicts.iter().filter(|v| !v.pass).count();
    let head = short(&pr.head_sha);
    let rounds = format!("Failed review rounds: {failed_rounds} of {ROUNDS}.");
    let check = |state, description: String, report: String| ReviewCheck {
        state,
        description,
        report,
        failed_rounds,
    };

    let trusted_author = pr.author == MAINTAINER || pr.author == DEPENDABOT;
    if !trusted_author || !pr.from_this_repo {
        let from = if pr.from_this_repo {
            "from a branch in this repo"
        } else {
            "from a fork"
        };
        return check(
            State::Failure,
            "Only a PR opened by the maintainer's account or Dependabot, from a branch here, can pass"
                .to_string(),
            format!(
                "**Review check: fails.** This PR was opened by `{}`, {from}. Only a PR opened by \
                 the maintainer's account or by Dependabot, from a branch in this repo, can pass \
                 the Review check (ADR-0010).",
                pr.author
            ),
        );
    }
    if failed_rounds >= ROUNDS {
        return check(
            State::Failure,
            format!("{failed_rounds} review rounds failed, so this PR waits for the maintainer"),
            format!(
                "**Review check: fails.** {failed_rounds} review rounds have failed, so this PR \
                 waits for the maintainer, and it gets the `needs-maintainer` label."
            ),
        );
    }
    let Some(newest) = verdicts.last() else {
        return check(
            State::Pending,
            format!("Waiting for a Reviewer's Verdict on {head}"),
            format!(
                "**Review check: waiting.** No Reviewer's Verdict covers the latest commit, \
                 `{head}`, yet. {rounds}"
            ),
        );
    };
    let said = if newest.pass {
        "pass"
    } else {
        "changes needed"
    };
    if newest.commit != pr.head_sha.to_ascii_lowercase() {
        return check(
            State::Pending,
            format!(
                "Waiting for a fresh Verdict on {head}; the newest covers {}",
                short(&newest.commit)
            ),
            format!(
                "**Review check: waiting.** The [newest Verdict]({}) says {said} on `{}`, but \
                 the latest commit is `{head}`, and every new commit needs a fresh Verdict. \
                 {rounds}",
                newest.url,
                short(&newest.commit)
            ),
        );
    }
    if newest.pass {
        check(
            State::Success,
            format!("The Reviewer's Verdict on {head} says pass"),
            format!(
                "**Review check: passes.** The Reviewer's [Verdict]({}) on the latest commit, \
                 `{head}`, says pass. {rounds}",
                newest.url
            ),
        )
    } else {
        check(
            State::Failure,
            format!("The Reviewer asked for changes on {head} (round {failed_rounds} of {ROUNDS})"),
            format!(
                "**Review check: fails.** The Reviewer's [Verdict]({}) on the latest commit, \
                 `{head}`, is changes needed. {rounds} After {ROUNDS}, each with a new Reviewer, \
                 the PR waits for the maintainer.",
                newest.url
            ),
        )
    }
}

/// A commit's first 7 characters, as GitHub shows it.
pub fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}
