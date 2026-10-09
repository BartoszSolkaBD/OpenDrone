//! Verdicts, triage and the Review check (#15 §3, #120, ADR-0010).
//!
//! A Verdict is a PR comment from the maintainer's account (agents work under
//! it) whose first line is `Reviewed commit <full SHA>` and whose last line is
//! exactly `Verdict: pass` or `Verdict: changes needed`. Each Verdict of
//! changes needed is one failed review round. The Review check passes only
//! when:
//!
//! - the PR was opened by the maintainer's account or by Dependabot, from a
//!   branch in this repo;
//! - fewer than 5 review rounds have failed, or a triage comment covers the
//!   newest failed one (below);
//! - the newest Verdict covers the PR's latest commit, and says pass.
//!
//! After 5 failed rounds, what still blocks is filed as its own issue, and a
//! **triage comment** from the maintainer's account says so: its first line is
//! exactly `Triaged to #<issue>`, naming an open issue in this repo. One that
//! comes after the newest failed Verdict, once 5 have failed, counts as a pass
//! Verdict on the commit that failed Verdict reviewed. A later commit needs a
//! fresh Verdict as usual, and a later failed round needs a triage of its own.

use serde_json::Value;

/// The maintainer's GitHub account. Agents work under it in Phase 1, so it
/// posts the Verdicts, and only its Verdicts count.
pub const MAINTAINER: &str = "BartoszSolkaBD";
/// Dependabot's account, whose PRs may pass the Review check too.
pub const DEPENDABOT: &str = "dependabot[bot]";
/// A PR gets this many review rounds. Once this many have failed, the Review
/// check fails until a triage comment covers the newest failed one.
pub const ROUNDS: usize = 5;
/// The most issues the workflow looks up for triage comments.
const MOST_TRIAGE_LOOKUPS: usize = 10;

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
    /// Its comment's place among the PR's comments, oldest first.
    pub place: usize,
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
        .enumerate()
        .filter(|(_, comment)| comment.author == MAINTAINER)
        .filter_map(|(place, comment)| {
            let (commit, pass) = read_verdict(&comment.body)?;
            Some(Verdict {
                commit,
                pass,
                url: comment.url.clone(),
                place,
            })
        })
        .collect()
}

/// The issue a triage comment names, if the comment is one: its first line
/// is exactly `Triaged to #<issue>`, with the issue's number as GitHub writes
/// it. Anything may follow on later lines.
pub fn read_triage(body: &str) -> Option<u64> {
    let first = body
        .lines()
        .map(str::trim_end)
        .find(|line| !line.is_empty())?;
    let number = first.strip_prefix("Triaged to #")?;
    let digits =
        !number.is_empty() && number.len() <= 10 && number.bytes().all(|b| b.is_ascii_digit());
    if !digits || number.starts_with('0') {
        return None;
    }
    number.parse().ok()
}

/// A triage comment that could cover the newest failed Verdict.
#[derive(Clone, Debug)]
struct Triage {
    issue: u64,
    url: String,
}

/// The triage comments that could pass the Review check, oldest first: from
/// the maintainer's account, after the newest failed Verdict, once
/// [`ROUNDS`] review rounds have failed. Whether each names an open issue is
/// for GitHub to say.
fn triage_comments(comments: &[Comment]) -> Vec<Triage> {
    let verdicts = verdicts(comments);
    let failed: Vec<&Verdict> = verdicts.iter().filter(|v| !v.pass).collect();
    let Some(newest_failed) = failed.last().filter(|_| failed.len() >= ROUNDS) else {
        return Vec::new();
    };
    comments
        .iter()
        .skip(newest_failed.place + 1)
        .filter(|comment| comment.author == MAINTAINER)
        .filter_map(|comment| {
            Some(Triage {
                issue: read_triage(&comment.body)?,
                url: comment.url.clone(),
            })
        })
        .collect()
}

/// The issues the workflow looks up for the triage comments that could pass
/// the Review check: newest first, each once, at most 10.
pub fn triage_issues(comments: &[Comment]) -> Vec<u64> {
    let mut issues: Vec<u64> = Vec::new();
    for triage in triage_comments(comments).iter().rev() {
        if !issues.contains(&triage.issue) && issues.len() < MOST_TRIAGE_LOOKUPS {
            issues.push(triage.issue);
        }
    }
    issues
}

/// GitHub's record of an issue a triage comment names.
#[derive(Clone, Debug)]
pub struct Issue {
    pub number: u64,
    pub state: String,
    /// Its web address, such as `https://github.com/owner/repo/issues/120`.
    pub url: String,
}

impl Issue {
    /// Reads GitHub's issue records (`GET /repos/{owner}/{repo}/issues/{n}`),
    /// one after another. A record without a number is left out.
    pub fn list_from_api(text: &str) -> Result<Vec<Issue>, String> {
        let mut issues = Vec::new();
        for record in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
            let record = record.map_err(|error| format!("the issues aren't JSON: {error}"))?;
            let text = |key: &str| {
                record
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            if let Some(number) = record.get("number").and_then(Value::as_u64) {
                issues.push(Issue {
                    number,
                    state: text("state"),
                    url: text("html_url"),
                });
            }
        }
        Ok(issues)
    }

    /// Whether it is an open issue in the PR's repo. GitHub's issue records
    /// hold pull requests too, whose address has `/pull/`, and an issue moved
    /// to another repo answers with that repo's record and address.
    fn is_open_in(&self, pr: &PullRequest) -> bool {
        self.state == "open"
            && !pr.repo_url.is_empty()
            && self.url == format!("{}/issues/{}", pr.repo_url, self.number)
    }
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
    /// The issue a triage comment moved what still blocked into, once
    /// [`ROUNDS`] review rounds have failed.
    pub triaged_to: Option<u64>,
}

impl ReviewCheck {
    /// Whether [`ROUNDS`] review rounds have failed and no triage comment
    /// covers the newest one: the PR then gets the `needs-triage` label.
    pub fn waits_for_triage(&self) -> bool {
        self.failed_rounds >= ROUNDS && self.triaged_to.is_none()
    }
}

/// Works out the Review check from the PR, its comments, and GitHub's records
/// of the issues its triage comments name ([`triage_issues`]).
pub fn review_check(pr: &PullRequest, comments: &[Comment], issues: &[Issue]) -> ReviewCheck {
    let verdicts = verdicts(comments);
    let failed_rounds = verdicts.iter().filter(|v| !v.pass).count();
    let head = short(&pr.head_sha);
    let open = |issue: u64| issues.iter().any(|i| i.number == issue && i.is_open_in(pr));
    // The newest triage comment that names an open issue.
    let triage = triage_comments(comments)
        .into_iter()
        .rev()
        .find(|t| open(t.issue));
    let link = |issue: u64| {
        if pr.repo_url.is_empty() {
            format!("#{issue}")
        } else {
            format!("[#{issue}]({}/issues/{issue})", pr.repo_url)
        }
    };
    let mut rounds = format!("Failed review rounds: {failed_rounds} of {ROUNDS}.");
    if let Some(triage) = &triage {
        rounds.push_str(&format!(
            " What still blocked is triaged to {}.",
            link(triage.issue)
        ));
    }
    let check = |state, description: String, report: String| ReviewCheck {
        state,
        description,
        report,
        failed_rounds,
        triaged_to: triage.as_ref().map(|t| t.issue),
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
    if failed_rounds >= ROUNDS && triage.is_none() {
        let reviewed = verdicts
            .iter()
            .rev()
            .find(|v| !v.pass)
            .map_or("", |v| short(&v.commit));
        let mut report = format!(
            "**Review check: fails.** {failed_rounds} review rounds have failed, so this PR gets \
             the `needs-triage` label. What still blocks goes into its own issue. Then a comment \
             from the maintainer's account whose first line is `Triaged to #<issue>`, naming that \
             open issue, counts as a pass Verdict on `{reviewed}`, the commit the last failed \
             Verdict reviewed. {rounds}"
        );
        for issue in triage_issues(comments) {
            report.push_str(&format!(
                " `Triaged to #{issue}` doesn't count: #{issue} isn't an open issue in this repo."
            ));
        }
        return check(
            State::Failure,
            format!(
                "{failed_rounds} review rounds failed: waiting for a triage comment naming an open \
                 issue"
            ),
            report,
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
    let reviewed = short(&newest.commit);
    let on_head = newest.commit == pr.head_sha.to_ascii_lowercase();
    // A triage comment comes after the newest failed Verdict, so when the
    // newest Verdict fails, the triage counts as a pass on its commit.
    if let Some(triage) = triage.as_ref().filter(|_| !newest.pass) {
        return if on_head {
            check(
                State::Success,
                format!(
                    "Triaged to #{} after {failed_rounds} failed review rounds: passes on {head}",
                    triage.issue
                ),
                format!(
                    "**Review check: passes.** The [triage comment]({}) counts as a pass Verdict \
                     on the latest commit, `{head}`, which the last failed Verdict reviewed. A \
                     later commit needs a fresh Verdict. {rounds}",
                    triage.url
                ),
            )
        } else {
            check(
                State::Pending,
                format!("Waiting for a fresh Verdict on {head}; the triage covers {reviewed}"),
                format!(
                    "**Review check: waiting.** The [triage comment]({}) counts as a pass Verdict \
                     on `{reviewed}`, the commit the last failed Verdict reviewed, but the latest \
                     commit is `{head}`, and every new commit needs a fresh Verdict. {rounds}",
                    triage.url
                ),
            )
        };
    }
    let said = if newest.pass {
        "pass"
    } else {
        "changes needed"
    };
    if !on_head {
        return check(
            State::Pending,
            format!("Waiting for a fresh Verdict on {head}; the newest covers {reviewed}"),
            format!(
                "**Review check: waiting.** The [newest Verdict]({}) says {said} on `{reviewed}`, \
                 but the latest commit is `{head}`, and every new commit needs a fresh Verdict. \
                 {rounds}",
                newest.url
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
                 `{head}`, is changes needed. {rounds} After {ROUNDS} failed rounds, each with a \
                 new Reviewer, what still blocks moves to its own issue.",
                newest.url
            ),
        )
    }
}

/// A commit's first 7 characters, as GitHub shows it.
pub fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}
