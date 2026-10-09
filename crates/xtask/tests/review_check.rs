//! Readable checks for `cargo xtask review-update` and `cargo xtask
//! merge-check`: the Review check, the Review Report comment and its labels,
//! and naming merges that skipped a check (#15 §3, ADR-0010).
//!
//! Each check writes GitHub's records of a pull request and its comments, as
//! the review workflow fetches them, and runs the real command on them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const HEAD: &str = "1111111111111111111111111111111111111111";
const OLDER: &str = "2222222222222222222222222222222222222222";
/// A later commit, on a branch stacked on this PR's.
const LATER: &str = "3333333333333333333333333333333333333333";
const MAINTAINER: &str = "BartoszSolkaBD";

// The Review check.

#[test]
fn a_pass_verdict_on_the_latest_commit_passes_the_review_check() {
    let update = Pr::new("pass-on-head")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    update.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
    update.says("**Review check: passes.**");
}

#[test]
fn with_no_verdict_yet_the_review_check_waits() {
    let update = Pr::new("no-verdict")
        .comment(MAINTAINER, "Starting a Reviewer now.")
        .update();
    update.check_is("pending", "Waiting for a Reviewer's Verdict on 1111111");
    update.says("No Reviewer's Verdict covers the latest commit, `1111111`, yet.");
}

#[test]
fn a_pass_verdict_on_an_older_commit_waits_for_a_fresh_verdict() {
    let update = Pr::new("pass-on-older")
        .comment(MAINTAINER, &verdict(OLDER, "pass"))
        .update();
    update.check_is(
        "pending",
        "Waiting for a fresh Verdict on 1111111; the newest covers 2222222",
    );
    update.says("every new commit needs a fresh Verdict");
}

#[test]
fn changes_needed_on_the_latest_commit_fails_the_review_check() {
    let update = Pr::new("changes-needed")
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .update();
    update.check_is(
        "failure",
        "The Reviewer asked for changes on 1111111 (round 1 of 5)",
    );
}

#[test]
fn only_the_newest_verdict_counts() {
    let failed_after_a_pass = Pr::new("pass-then-changes")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .update();
    failed_after_a_pass.check_is(
        "failure",
        "The Reviewer asked for changes on 1111111 (round 1 of 5)",
    );
    let passed_after_a_fail = Pr::new("changes-then-pass")
        .comment(MAINTAINER, &verdict(OLDER, "changes needed"))
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    passed_after_a_fail.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
    passed_after_a_fail.says("Failed review rounds: 1 of 5.");
}

// Review rounds, and triage after the fifth failed one (#120).

#[test]
fn a_pass_in_the_fourth_or_the_fifth_review_round_passes_the_review_check() {
    for failed in [3, 4] {
        let update = Pr::new(&format!("pass-in-round-{}", failed + 1))
            .failed_rounds(failed, OLDER)
            .comment(MAINTAINER, &verdict(HEAD, "pass"))
            .update();
        update.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
        update.says(&format!("Failed review rounds: {failed} of 5."));
        assert!(update.add.is_empty(), "{:?}", update.add);
    }
}

#[test]
fn a_fifth_failed_review_round_fails_the_review_check_and_labels_the_pr_needs_triage() {
    let update = Pr::new("five-rounds")
        .failed_rounds(4, OLDER)
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .update();
    update.check_is("failure", &out_of_rounds(5));
    update.says("Failed review rounds: 5 of 5.");
    update.says("this PR gets the `needs-triage` label");
    update.says("first line is `Triaged to #<issue>`");
    update.adds_label("needs-triage");
    update.does_not_add_label("needs-maintainer");
    let already = Pr::new("five-rounds-labelled")
        .label("needs-triage")
        .failed_rounds(5, HEAD)
        .update();
    assert!(already.add.is_empty(), "{:?}", already.add);
    assert!(already.remove.is_empty(), "{:?}", already.remove);
}

#[test]
fn after_five_failed_review_rounds_a_pass_alone_still_fails_the_review_check() {
    let update = Pr::new("five-rounds-then-pass")
        .failed_rounds(5, OLDER)
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    update.check_is("failure", &out_of_rounds(5));
    update.adds_label("needs-triage");
}

#[test]
fn a_triaged_to_comment_from_the_maintainer_s_account_after_five_failed_rounds_passes_the_review_check()
 {
    let update = Pr::new("triaged")
        .label("needs-triage")
        .failed_rounds(4, OLDER)
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .comment(
            MAINTAINER,
            "Triaged to #200\n\nThe two blocking problems left are in #200.",
        )
        .issue(200, "open")
        .update();
    update.check_is(
        "success",
        "Triaged to #200 after 5 failed review rounds: passes on 1111111",
    );
    update.says("**Review check: passes.**");
    update.says(
        "Failed review rounds: 5 of 5. What still blocked is triaged to \
         [#200](https://github.com/BartoszSolkaBD/OpenDrone/issues/200).",
    );
    assert_eq!(update.remove, ["needs-triage"]);
    assert!(update.add.is_empty(), "{:?}", update.add);
}

#[test]
fn a_triage_passes_only_the_commit_the_fifth_verdict_reviewed_and_a_later_commit_needs_a_fresh_verdict()
 {
    let later_commit = Pr::new("triaged-older")
        .failed_rounds(5, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .issue(200, "open")
        .update();
    later_commit.check_is(
        "pending",
        "Waiting for a fresh Verdict on 1111111; the triage covers 2222222",
    );
    later_commit.says("every new commit needs a fresh Verdict");
    later_commit.says("What still blocked is triaged to [#200]");
    later_commit.does_not_add_label("needs-triage");

    let fresh_pass = Pr::new("triaged-then-pass")
        .failed_rounds(5, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .issue(200, "open")
        .update();
    fresh_pass.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
    fresh_pass.says("What still blocked is triaged to [#200]");
}

#[test]
fn a_failed_round_after_the_triage_needs_a_triage_of_its_own() {
    let failed_again = Pr::new("failed-after-triage")
        .failed_rounds(5, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .issue(200, "open")
        .update();
    failed_again.check_is("failure", &out_of_rounds(6));
    failed_again.says("Failed review rounds: 6 of 5.");
    failed_again.adds_label("needs-triage");

    let triaged_again = Pr::new("triaged-again")
        .failed_rounds(5, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .comment(MAINTAINER, "Triaged to #201")
        .issue(200, "open")
        .issue(201, "open")
        .update();
    triaged_again.check_is(
        "success",
        "Triaged to #201 after 6 failed review rounds: passes on 1111111",
    );
}

#[test]
fn a_triaged_to_comment_before_the_fifth_failed_round_changes_nothing() {
    let in_round_four = Pr::new("triage-too-early")
        .failed_rounds(3, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .issue(200, "open")
        .update();
    in_round_four.check_is(
        "failure",
        "The Reviewer asked for changes on 1111111 (round 4 of 5)",
    );
    in_round_four.says("Failed review rounds: 4 of 5.");
    assert!(
        !in_round_four.comment.contains("triaged to"),
        "{}",
        in_round_four.comment
    );
    in_round_four.does_not_add_label("needs-triage");

    let then_a_fifth = Pr::new("triage-before-the-fifth")
        .failed_rounds(4, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, &verdict(HEAD, "changes needed"))
        .issue(200, "open")
        .update();
    then_a_fifth.check_is("failure", &out_of_rounds(5));
    then_a_fifth.adds_label("needs-triage");
}

#[test]
fn a_triaged_to_comment_from_any_other_account_changes_nothing() {
    for (i, author) in ["someone-else", "github-actions[bot]"].iter().enumerate() {
        let update = Pr::new(&format!("triage-from-stranger-{i}"))
            .failed_rounds(5, HEAD)
            .comment(author, "Triaged to #200")
            .issue(200, "open")
            .update();
        update.check_is("failure", &out_of_rounds(5));
        update.adds_label("needs-triage");
    }
}

#[test]
fn a_triaged_to_comment_naming_a_closed_or_missing_issue_changes_nothing() {
    let closed = Pr::new("triage-closed-issue")
        .failed_rounds(5, HEAD)
        .comment(MAINTAINER, "Triaged to #200")
        .issue(200, "closed")
        .update();
    closed.check_is("failure", &out_of_rounds(5));
    closed.says("`Triaged to #200` doesn't count: #200 isn't an open issue in this repo.");
    closed.adds_label("needs-triage");

    let missing = Pr::new("triage-missing-issue")
        .failed_rounds(5, HEAD)
        .comment(MAINTAINER, "Triaged to #200")
        .update();
    missing.check_is("failure", &out_of_rounds(5));
    missing.says("`Triaged to #200` doesn't count: #200 isn't an open issue in this repo.");

    // GitHub's issue records hold pull requests too, and an issue moved to
    // another repo answers with that repo's record.
    let a_pull_request = Pr::new("triage-names-a-pr")
        .failed_rounds(5, HEAD)
        .comment(MAINTAINER, "Triaged to #201")
        .issue_record(json!({
            "number": 201,
            "state": "open",
            "html_url": "https://github.com/BartoszSolkaBD/OpenDrone/pull/201",
            "pull_request": {
                "url": "https://api.github.com/repos/BartoszSolkaBD/OpenDrone/pulls/201",
            },
        }))
        .update();
    a_pull_request.check_is("failure", &out_of_rounds(5));
    let moved = Pr::new("triage-names-a-moved-issue")
        .failed_rounds(5, HEAD)
        .comment(MAINTAINER, "Triaged to #202")
        .issue_record(json!({
            "number": 7,
            "state": "open",
            "html_url": "https://github.com/someone/elsewhere/issues/7",
        }))
        .update();
    moved.check_is("failure", &out_of_rounds(5));
}

#[test]
fn a_comment_is_a_triage_only_in_the_exact_format() {
    let not_triages = [
        "Triaged to #200, mostly",
        "triaged to #200",
        "Triaged to 200",
        "Triaged to #",
        "Triaged to #0200",
        "Triaged to #200 #201",
        " Triaged to #200",
        "Some words first.\nTriaged to #200",
    ];
    for (i, body) in not_triages.iter().enumerate() {
        let update = Pr::new(&format!("not-a-triage-{i}"))
            .failed_rounds(5, HEAD)
            .comment(MAINTAINER, body)
            .issue(200, "open")
            .update();
        update.check_is("failure", &out_of_rounds(5));
    }
    let with_trailing_space_and_windows_lines = Pr::new("triage-crlf")
        .failed_rounds(5, HEAD)
        .comment(
            MAINTAINER,
            "\r\nTriaged to #200  \r\n\r\nWhat still blocks: the tolerance.\r\n",
        )
        .issue(200, "open")
        .update();
    with_trailing_space_and_windows_lines.check_is(
        "success",
        "Triaged to #200 after 5 failed review rounds: passes on 1111111",
    );
}

#[test]
fn the_workflow_looks_up_only_the_issues_a_triage_comment_could_pass_with() {
    let too_few_rounds = Pr::new("lookups-too-few")
        .failed_rounds(4, OLDER)
        .comment(MAINTAINER, "Triaged to #200")
        .triage_issues();
    assert_eq!(too_few_rounds, Vec::<String>::new());

    let lookups = Pr::new("lookups")
        .failed_rounds(4, OLDER)
        .comment(MAINTAINER, "Triaged to #100")
        .failed_rounds(1, OLDER)
        .comment("someone-else", "Triaged to #101")
        .comment(MAINTAINER, "Triaged to #200")
        .comment(MAINTAINER, "Triaged to #201")
        .comment(MAINTAINER, "Triaged to #200")
        .triage_issues();
    assert_eq!(lookups, ["200", "201"], "newest first, each once");
}

// Who can pass, and what counts as a Verdict.

#[test]
fn a_verdict_from_any_other_account_is_not_a_verdict() {
    let update = Pr::new("stranger-verdict")
        .comment("someone-else", &verdict(HEAD, "pass"))
        .comment("github-actions[bot]", &verdict(HEAD, "pass"))
        .update();
    update.check_is("pending", "Waiting for a Reviewer's Verdict on 1111111");
}

#[test]
fn a_comment_is_a_verdict_only_in_the_exact_format() {
    let not_verdicts = [
        // The last line must be exactly the Verdict.
        format!("Reviewed commit {HEAD}\n\nLooks fine.\n\nVerdict: pass, mostly"),
        format!("Reviewed commit {HEAD}\n\nVerdict: pass\n\nOne more thing."),
        format!("Reviewed commit {HEAD}\n\nverdict: PASS"),
        // The first line must name the full commit.
        "Reviewed commit 1111111\n\nVerdict: pass".to_string(),
        format!("I reviewed commit {HEAD}\n\nVerdict: pass"),
        format!("Some words first.\nReviewed commit {HEAD}\n\nVerdict: pass"),
    ];
    for (i, body) in not_verdicts.iter().enumerate() {
        let update = Pr::new(&format!("not-a-verdict-{i}"))
            .comment(MAINTAINER, body)
            .update();
        update.check_is("pending", "Waiting for a Reviewer's Verdict on 1111111");
    }
    let with_trailing_space_and_windows_lines = Pr::new("verdict-crlf")
        .comment(
            MAINTAINER,
            &format!("Reviewed commit {HEAD}\r\n\r\n| table |\r\n\r\nVerdict: pass  \r\n"),
        )
        .update();
    with_trailing_space_and_windows_lines
        .check_is("success", "The Reviewer's Verdict on 1111111 says pass");
}

#[test]
fn a_pr_opened_by_someone_else_fails_the_review_check_even_with_a_pass() {
    let update = Pr::new("stranger-pr")
        .author("someone-else")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    update.check_is(
        "failure",
        "Only a PR opened by the maintainer's account or Dependabot, from a branch here, can pass",
    );
    update.says("This PR was opened by `someone-else`, from a branch in this repo.");
}

#[test]
fn a_pr_from_a_fork_fails_the_review_check_even_when_the_maintainer_opened_it() {
    let update = Pr::new("fork-pr")
        .opened_from_a_fork()
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    update.check_is(
        "failure",
        "Only a PR opened by the maintainer's account or Dependabot, from a branch here, can pass",
    );
    update.says("from a fork");
}

#[test]
fn a_dependabot_pr_from_a_branch_in_this_repo_can_pass() {
    let update = Pr::new("dependabot")
        .author("dependabot[bot]")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .update();
    update.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
}

// The Review Report comment and its labels.

#[test]
fn the_review_report_is_one_comment_found_by_its_marker_and_posted_by_ci() {
    let first = Pr::new("first-report").update();
    assert_eq!(
        first.comment_id, "",
        "no Report yet, so a new comment is made"
    );
    assert!(
        first
            .comment
            .starts_with("<!-- opendrone-review-report -->\n## Review Report")
    );

    let later = Pr::new("later-report")
        .comment_with_id(7, MAINTAINER, "<!-- opendrone-review-report -->\nnot CI's")
        .comment_with_id(
            8,
            "github-actions[bot]",
            "<!-- opendrone-review-report -->\nold",
        )
        .update();
    assert_eq!(later.comment_id, "8", "CI's own Report comment is edited");
}

#[test]
fn the_report_s_sections_are_the_ones_worked_out_for_the_latest_commit() {
    let update = Pr::new("report-ready")
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &["Physics"])
        .update();
    update.says("### Verdict");
    update.says("### Red Flags\n\nNone.");
    update.says("It covers commit `1111111`");
}

#[test]
fn the_red_flag_gate_passes_when_no_red_flag_waits_for_the_maintainer() {
    let update = Pr::new("gate-passes")
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[])
        .update();
    update.gate_is("success", "No Red Flag waits for the maintainer");
}

#[test]
fn a_red_flag_that_waits_fails_the_gate_and_adds_the_needs_maintainer_label() {
    let update = Pr::new("needs-maintainer")
        .report(HEAD, "### Red Flags\n", true, &[])
        .update();
    update.gate_is(
        "failure",
        "A Red Flag waits for the maintainer: see the Review Report",
    );
    update.adds_label("needs-maintainer");
    let already = Pr::new("needs-maintainer-already")
        .label("needs-maintainer")
        .report(HEAD, "### Red Flags\n", true, &[])
        .update();
    assert!(already.add.is_empty(), "{:?}", already.add);
}

#[test]
fn red_flags_worked_out_for_an_older_commit_are_not_shown_and_the_gate_reports_an_error() {
    let update = Pr::new("report-stale")
        .report(OLDER, "### Red Flags\n\nNone.\n", false, &["Physics"])
        .update();
    update.says("_The rest of this report couldn't be worked out for `1111111`.");
    assert!(
        !update.comment.contains("### Red Flags"),
        "{}",
        update.comment
    );
    assert!(
        update.add.is_empty() && update.remove.is_empty(),
        "no labels from Red Flags for another commit"
    );
    update.gate_is(
        "error",
        "The Red Flags couldn't be worked out: see the Review workflow's run",
    );
}

#[test]
fn with_no_red_flags_worked_out_the_gate_reports_an_error_and_never_passes() {
    let update = Pr::new("report-missing").update();
    update.gate_is(
        "error",
        "The Red Flags couldn't be worked out: see the Review workflow's run",
    );
}

#[test]
fn a_pr_into_another_branch_sets_neither_status_and_the_report_says_why() {
    let update = Pr::new("into-another-branch")
        .into_branch("ticket/41-thrust-stand")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[])
        .update();
    assert!(update.statuses.is_empty(), "{:?}", update.statuses);
    update.says(
        "**Not judged:** this PR merges into `ticket/41-thrust-stand`, not `main`, so CI sets \
         neither the Red Flag gate nor the Review check on its commits.",
    );
}

#[test]
fn both_checks_fail_while_another_open_pr_into_main_holds_the_same_commit() {
    let update = Pr::new("shared-head")
        .sharing_with(78, "open", "main", HEAD)
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[])
        .update();
    update.gate_is(
        "failure",
        "Other open PRs into main share this commit (#78): keep one",
    );
    update.check_is(
        "failure",
        "Other open PRs into main share this commit (#78): keep one",
    );
    update.says(
        "**Both checks fail:** other open PRs into `main` have this same latest commit (#78).",
    );
    update.says("The PR left with this commit is then judged again.");
}

#[test]
fn a_closed_pr_or_one_into_another_branch_with_the_same_commit_changes_nothing() {
    let update = Pr::new("shared-head-elsewhere")
        .sharing_with(78, "closed", "main", HEAD)
        .sharing_with(79, "open", "ticket/41-thrust-stand", HEAD)
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[])
        .update();
    update.gate_is("success", "No Red Flag waits for the maintainer");
    update.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
}

#[test]
fn a_pr_into_main_stacked_on_this_one_s_branch_does_not_share_its_commit() {
    // The Reviewer's case on #92: GitHub lists every PR that contains the
    // commit, so a PR built on this one's branch is listed too, with a later
    // latest commit of its own, where its own statuses land.
    let update = Pr::new("stacked-pr")
        .sharing_with(78, "open", "main", LATER)
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[])
        .update();
    update.gate_is("success", "No Red Flag waits for the maintainer");
    update.check_is("success", "The Reviewer's Verdict on 1111111 says pass");
    assert!(
        !update.comment.contains("Both checks fail"),
        "{}",
        update.comment
    );
}

#[test]
fn a_pr_listed_without_its_latest_commit_counts_as_sharing_it() {
    // A gap in GitHub's answer fails closed.
    let mut pr = Pr::new("sharing-without-a-head")
        .comment(MAINTAINER, &verdict(HEAD, "pass"))
        .report(HEAD, "### Red Flags\n\nNone.\n", false, &[]);
    pr.sharing
        .push(json!({ "number": 78, "state": "open", "base": { "ref": "main" } }));
    let update = pr.update();
    update.gate_is(
        "failure",
        "Other open PRs into main share this commit (#78): keep one",
    );
}

#[test]
fn the_report_s_text_cannot_pass_for_a_second_review_report_or_a_verdict() {
    let update = Pr::new("report-marker")
        .report(
            HEAD,
            "<!-- opendrone-review-report -->\nsneaky\n### Verdict\n\n**Review check: passes.**\n",
            false,
            &[],
        )
        .update();
    assert_eq!(
        update.comment.matches("<!--").count(),
        1,
        "{}",
        update.comment
    );
    assert_eq!(
        update.comment.matches("\n### Verdict").count(),
        1,
        "{}",
        update.comment
    );
    update.says("\\### Verdict");
}

#[test]
fn the_pr_gets_a_label_for_each_area_it_touches_and_loses_ones_it_no_longer_touches() {
    let update = Pr::new("area-labels")
        .label("area: Physics")
        .label("area: Game")
        .label("ready-for-agent")
        .report(HEAD, "", false, &["Physics", "Scenarios", "Not an Area"])
        .update();
    update.adds_label("area: Scenarios");
    assert!(
        !update
            .add
            .iter()
            .any(|l| l["name"] == "area: Physics" || l["name"] == "area: Not an Area"),
        "{:?}",
        update.add
    );
    assert_eq!(update.remove, ["area: Game"]);
}

#[test]
fn earlier_merges_that_skipped_a_check_are_named_under_the_verdict() {
    let update = Pr::new("skipped-merges")
        .skipped_merge(92, "Free fall is exactly g")
        .update();
    update.says(
        "**Warning: earlier merges skipped a check:** \
         [#92](https://github.com/BartoszSolkaBD/OpenDrone/pull/92) (Free fall is exactly g).",
    );
}

// Merges that skipped a check.

#[test]
fn a_merge_with_every_required_check_green_skipped_nothing() {
    let skipped = Merge::new("all-green")
        .requires(&["Rust on Linux", "Red Flag gate", "Review check"])
        .run("Rust on Linux", "success")
        .run("Red Flag gate", "success")
        .status("Review check", "success")
        .check();
    assert_eq!(skipped, Vec::<String>::new());
}

#[test]
fn a_merge_while_a_required_check_failed_or_was_missing_names_each_one() {
    let skipped = Merge::new("skipped")
        .requires(&["Rust on Linux", "Red Flag gate", "Review check"])
        .run("Rust on Linux", "success")
        .run("Red Flag gate", "failure")
        .check();
    assert_eq!(
        skipped,
        ["Red Flag gate: failure", "Review check: never reported"]
    );
}

#[test]
fn a_check_that_passed_when_run_again_counts_as_passed() {
    let skipped = Merge::new("rerun")
        .requires(&["Rust on Windows"])
        .run_with_id(1, "Rust on Windows", "failure")
        .run_with_id(2, "Rust on Windows", "success")
        .check();
    assert_eq!(skipped, Vec::<String>::new());
}

#[test]
fn a_check_run_and_a_status_with_the_same_name_must_both_have_passed() {
    let status_failed = Merge::new("run-passed-status-failed")
        .requires(&["Review check"])
        .run("Review check", "success")
        .status("Review check", "failure")
        .check();
    assert_eq!(status_failed, ["Review check: failure"]);
    let run_failed = Merge::new("status-passed-run-failed")
        .requires(&["Red Flag gate"])
        .run("Red Flag gate", "failure")
        .status("Red Flag gate", "success")
        .check();
    assert_eq!(run_failed, ["Red Flag gate: failure"]);
}

#[test]
fn a_check_main_does_not_require_does_not_count() {
    let skipped = Merge::new("not-required")
        .requires(&["Rust on Linux"])
        .run("Rust on Linux", "success")
        .run("Spelling", "failure")
        .check();
    assert_eq!(skipped, Vec::<String>::new());
}

#[test]
fn when_main_requires_no_checks_every_check_that_reported_counts() {
    let skipped = Merge::new("nothing-required")
        .run("Rust on Linux", "success")
        .running("Scenarios agree on every OS")
        .status("Review check", "pending")
        .check();
    assert_eq!(
        skipped,
        [
            "Scenarios agree on every OS: in progress",
            "Review check: pending"
        ]
    );
}

fn verdict(commit: &str, says: &str) -> String {
    format!(
        "Reviewed commit {commit}\n\n| # | Acceptance criterion | Status |\n|---|---|---|\n\
         | 1 | It works | met |\n\nVerdict: {says}"
    )
}

/// The Review check's description once `failed` rounds have failed and no
/// triage counts.
fn out_of_rounds(failed: usize) -> String {
    format!("{failed} review rounds failed: waiting for a triage comment naming an open issue")
}

fn scratch(kind: &str, name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(kind).join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("can make the scratch folder");
    root
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, value.to_string()).expect("can write JSON");
}

/// A pull request's records, as GitHub's API gives them.
struct Pr {
    root: PathBuf,
    pr: Value,
    comments: Vec<Value>,
    report: Option<PathBuf>,
    skipped: Vec<Value>,
    /// The PRs that hold the same head commit, as GitHub lists them.
    sharing: Vec<Value>,
    /// GitHub's records of the issues triage comments name.
    issues: Vec<Value>,
}

impl Pr {
    fn new(name: &str) -> Pr {
        Pr {
            root: scratch("review-update", name),
            pr: json!({
                "number": 77,
                "user": { "login": MAINTAINER },
                "head": { "sha": HEAD, "repo": { "full_name": "BartoszSolkaBD/OpenDrone" } },
                "base": {
                    "ref": "main",
                    "repo": {
                        "full_name": "BartoszSolkaBD/OpenDrone",
                        "html_url": "https://github.com/BartoszSolkaBD/OpenDrone",
                        "default_branch": "main",
                    },
                },
                "labels": [],
            }),
            comments: Vec::new(),
            report: None,
            skipped: Vec::new(),
            sharing: vec![json!({
                "number": 77,
                "state": "open",
                "base": { "ref": "main" },
                "head": { "sha": HEAD },
            })],
            issues: Vec::new(),
        }
    }

    fn author(mut self, login: &str) -> Pr {
        self.pr["user"]["login"] = json!(login);
        self
    }

    fn opened_from_a_fork(mut self) -> Pr {
        self.pr["head"]["repo"]["full_name"] = json!("someone/OpenDrone");
        self
    }

    fn into_branch(mut self, branch: &str) -> Pr {
        self.pr["base"]["ref"] = json!(branch);
        self.sharing[0]["base"]["ref"] = json!(branch);
        self
    }

    /// Another PR GitHub lists for the head commit, with its own latest
    /// commit: the same one, or a later one on a branch stacked on this.
    fn sharing_with(mut self, number: u64, state: &str, branch: &str, head: &str) -> Pr {
        self.sharing.push(json!({
            "number": number,
            "state": state,
            "base": { "ref": branch },
            "head": { "sha": head },
        }));
        self
    }

    fn label(mut self, name: &str) -> Pr {
        self.pr["labels"]
            .as_array_mut()
            .expect("labels")
            .push(json!({ "name": name }));
        self
    }

    fn comment(self, author: &str, body: &str) -> Pr {
        let id = 100 + self.comments.len() as u64;
        self.comment_with_id(id, author, body)
    }

    /// `count` Verdicts of changes needed on `commit`, one after another.
    fn failed_rounds(self, count: usize, commit: &str) -> Pr {
        (0..count).fold(self, |pr, _| {
            pr.comment(MAINTAINER, &verdict(commit, "changes needed"))
        })
    }

    /// GitHub's record of an issue in this repo, `open` or `closed`.
    fn issue(self, number: u64, state: &str) -> Pr {
        self.issue_record(json!({
            "number": number,
            "state": state,
            "html_url": format!("https://github.com/BartoszSolkaBD/OpenDrone/issues/{number}"),
        }))
    }

    /// GitHub's answer when the workflow looks up an issue.
    fn issue_record(mut self, record: Value) -> Pr {
        self.issues.push(record);
        self
    }

    fn comment_with_id(mut self, id: u64, author: &str, body: &str) -> Pr {
        self.comments.push(json!({
            "id": id,
            "user": { "login": author },
            "body": body,
            "html_url": format!("https://github.com/BartoszSolkaBD/OpenDrone/pull/77#issuecomment-{id}"),
        }));
        self
    }

    /// What `review-report` worked out, for `commit`.
    fn report(mut self, commit: &str, markdown: &str, waits: bool, areas: &[&str]) -> Pr {
        let folder = self.root.join("download");
        fs::create_dir_all(&folder).expect("can make the folder");
        write_json(
            &folder.join("pr.json"),
            &json!({ "number": 77, "head_sha": commit }),
        );
        write_json(
            &folder.join("review.json"),
            &json!({ "waits_for_maintainer": waits, "areas": areas, "red_flags": [] }),
        );
        fs::write(folder.join("report.md"), markdown).expect("can write report.md");
        self.report = Some(folder);
        self
    }

    fn skipped_merge(mut self, number: u64, title: &str) -> Pr {
        self.skipped.push(json!({
            "number": number,
            "title": title,
            "html_url": format!("https://github.com/BartoszSolkaBD/OpenDrone/pull/{number}"),
        }));
        self
    }

    /// Writes the comments as two pages, as `gh api --paginate` prints them.
    fn write_comments(&self) -> PathBuf {
        let (first, second) = self.comments.split_at(self.comments.len() / 2);
        let path = self.root.join("comments.json");
        fs::write(&path, format!("{}\n{}", json!(first), json!(second)))
            .expect("can write the comments");
        path
    }

    /// Runs `cargo xtask triage-issues`, as the privileged workflow does, and
    /// returns each issue it would look up.
    fn triage_issues(&self) -> Vec<String> {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("triage-issues")
            .arg("--comments")
            .arg(self.write_comments())
            .output()
            .expect("xtask runs");
        assert!(
            output.status.success(),
            "triage-issues failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Runs `cargo xtask review-update`, as the privileged workflow does.
    fn update(&self) -> Update {
        write_json(&self.root.join("pr.json"), &self.pr);
        self.write_comments();
        // One record after another, as the workflow saves them.
        let issues: Vec<String> = self.issues.iter().map(Value::to_string).collect();
        fs::write(self.root.join("issues.json"), issues.join("\n")).expect("can write the issues");
        write_json(&self.root.join("head-pulls.json"), &json!(self.sharing));
        write_json(
            &self.root.join("skipped.json"),
            &json!({ "items": self.skipped }),
        );
        let out = self.root.join("out");
        let codeowners = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/CODEOWNERS");
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .arg("review-update")
            .arg("--pr")
            .arg(self.root.join("pr.json"))
            .arg("--comments")
            .arg(self.root.join("comments.json"))
            .arg("--skipped-merges")
            .arg(self.root.join("skipped.json"))
            .arg("--head-pulls")
            .arg(self.root.join("head-pulls.json"))
            .arg("--issues")
            .arg(self.root.join("issues.json"))
            .arg("--codeowners")
            .arg(codeowners)
            .arg("--out")
            .arg(&out);
        if let Some(report) = &self.report {
            command.arg("--report").arg(report);
        }
        let output = command.output().expect("xtask runs");
        assert!(
            output.status.success(),
            "review-update failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let read = |name: &str| fs::read_to_string(out.join(name)).expect("the file was written");
        let json = |name: &str| -> Value { serde_json::from_str(&read(name)).expect("JSON") };
        Update {
            statuses: json("statuses.json").as_array().expect("a list").clone(),
            comment: read("comment.md"),
            comment_id: read("comment-id"),
            add: json("labels-add.json").as_array().expect("a list").clone(),
            remove: json("labels-remove.json")
                .as_array()
                .expect("a list")
                .iter()
                .map(|l| l.as_str().expect("a name").to_string())
                .collect(),
        }
    }
}

/// What `review-update` would post.
struct Update {
    /// The commit statuses to set, by context.
    statuses: Vec<Value>,
    comment: String,
    comment_id: String,
    add: Vec<Value>,
    remove: Vec<String>,
}

impl Update {
    fn status(&self, context: &str) -> &Value {
        self.statuses
            .iter()
            .find(|s| s["context"] == context)
            .unwrap_or_else(|| panic!("no {context} status in {:?}", self.statuses))
    }

    fn gate_is(&self, state: &str, description: &str) {
        let gate = self.status("Red Flag gate");
        assert_eq!(gate["state"], state, "{}", self.comment);
        assert_eq!(gate["description"], description);
        assert!(description.chars().count() <= 140);
    }

    fn check_is(&self, state: &str, description: &str) {
        let status = self.status("Review check");
        assert_eq!(status["state"], state, "{}", self.comment);
        assert_eq!(status["description"], description);
        let length = description.chars().count();
        assert!(
            length <= 140,
            "GitHub cuts descriptions at 140 characters: {length}"
        );
    }

    fn says(&self, text: &str) {
        assert!(
            self.comment.contains(text),
            "expected the comment to say:\n{text}\nbut it says:\n{}",
            self.comment
        );
    }

    fn adds_label(&self, name: &str) {
        assert!(
            self.add.iter().any(|l| l["name"] == name),
            "expected the label {name} to be added: {:?}",
            self.add
        );
    }

    fn does_not_add_label(&self, name: &str) {
        assert!(
            !self.add.iter().any(|l| l["name"] == name),
            "expected no label {name} to be added: {:?}",
            self.add
        );
    }
}

/// A just-merged PR's last commit: its check runs and statuses, and what
/// main requires.
struct Merge {
    root: PathBuf,
    required: Vec<String>,
    runs: Vec<Value>,
    statuses: Vec<Value>,
}

impl Merge {
    fn new(name: &str) -> Merge {
        Merge {
            root: scratch("merge-check", name),
            required: Vec::new(),
            runs: Vec::new(),
            statuses: Vec::new(),
        }
    }

    fn requires(mut self, checks: &[&str]) -> Merge {
        self.required = checks.iter().map(|c| c.to_string()).collect();
        self
    }

    fn run(self, name: &str, conclusion: &str) -> Merge {
        let id = 10 + self.runs.len() as u64;
        self.run_with_id(id, name, conclusion)
    }

    fn run_with_id(mut self, id: u64, name: &str, conclusion: &str) -> Merge {
        self.runs.push(json!({
            "id": id, "name": name, "status": "completed", "conclusion": conclusion,
        }));
        self
    }

    fn running(mut self, name: &str) -> Merge {
        let id = 10 + self.runs.len() as u64;
        self.runs.push(json!({
            "id": id, "name": name, "status": "in_progress", "conclusion": null,
        }));
        self
    }

    fn status(mut self, context: &str, state: &str) -> Merge {
        self.statuses
            .push(json!({ "context": context, "state": state }));
        self
    }

    /// Runs `cargo xtask merge-check`, and returns each line it printed.
    fn check(&self) -> Vec<String> {
        // Required checks from classic branch protection for the first, and
        // from a ruleset for the rest, so both are read.
        let (classic, ruleset) = self.required.split_at(self.required.len().min(1));
        write_json(
            &self.root.join("branch.json"),
            &json!({ "protection": { "required_status_checks": { "contexts": classic, "checks": [] } } }),
        );
        write_json(
            &self.root.join("rules.json"),
            &json!([{
                "type": "required_status_checks",
                "parameters": { "required_status_checks":
                    ruleset.iter().map(|c| json!({ "context": c })).collect::<Vec<_>>() },
            }]),
        );
        write_json(
            &self.root.join("runs.json"),
            &json!({ "total_count": self.runs.len(), "check_runs": self.runs }),
        );
        write_json(
            &self.root.join("status.json"),
            &json!({ "statuses": self.statuses }),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("merge-check")
            .arg("--branch")
            .arg(self.root.join("branch.json"))
            .arg("--rules")
            .arg(self.root.join("rules.json"))
            .arg("--check-runs")
            .arg(self.root.join("runs.json"))
            .arg("--status")
            .arg(self.root.join("status.json"))
            .output()
            .expect("xtask runs");
        assert!(
            output.status.success(),
            "merge-check failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }
}
