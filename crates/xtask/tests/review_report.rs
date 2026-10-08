//! Readable checks for `cargo xtask review-report`: the Review Report's Red
//! Flags, What moved and Areas touched, and the Red Flag gate (#15 §5 and
//! §10, #11 §4).
//!
//! Each check copies the fixture repo in `tests/review/repo/` twice, as the
//! base and the head of a pull request, changes the head with files from
//! `tests/review/changes/`, and runs the real command on the two copies. The
//! real `.github/CODEOWNERS` goes into both, so the Areas are the real ones.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

// The locked Expectations: Source and Rule.

#[test]
fn a_loosened_tolerance_on_a_rule_expectation_waits_for_the_maintainer() {
    let review = PullRequest::new("rule-loosened")
        .change(FREE_FALL, "free-fall-rule-loosened.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "\"vertical speed at 1 s\" went from `-9.81 m/s ± 0.00001 m/s` to \
         `-9.81 m/s ± 0.001 m/s`, a loosened tolerance",
    );
    review.has_label_needs("needs-maintainer");
}

#[test]
fn even_a_tighter_tolerance_on_a_rule_expectation_waits_for_the_maintainer() {
    let review = PullRequest::new("rule-tightened")
        .change(FREE_FALL, "free-fall-rule-tightened.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "\"height at 1 s\" went from `-4.905 m ± 0.001 m` to `-4.905 m ± 0.5 mm`.",
    );
}

#[test]
fn turning_a_rule_expectation_into_an_observed_one_waits_for_the_maintainer() {
    let review = PullRequest::new("rule-to-observed")
        .change(FREE_FALL, "free-fall-rule-to-observed.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "\"vertical speed at 1 s\" has an Observed Basis instead of Rule",
    );
}

#[test]
fn removing_a_rule_expectation_waits_for_the_maintainer() {
    let review = PullRequest::new("rule-removed")
        .change(FREE_FALL, "free-fall-rule-removed.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "\"height at 1 s\" (`-4.905 m ± 0.001 m`) was removed",
    );
}

#[test]
fn a_new_citation_on_a_source_expectation_waits_for_the_maintainer() {
    let review = PullRequest::new("source-changed")
        .change(FREE_FALL, "free-fall-source-changed.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Source Expectation changed",
        "\"vertical acceleration, mean over 0 s to 1 s\" has a new basis line",
    );
}

#[test]
fn reordering_and_respacing_expectations_and_adding_a_rule_one_raises_no_red_flag() {
    let review = PullRequest::new("reordered")
        .change(FREE_FALL, "free-fall-reordered.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.has_no_red_flags();
}

#[test]
fn moving_a_scenario_to_another_folder_unchanged_raises_no_red_flag() {
    let review = PullRequest::new("scenario-moved")
        .rename(FREE_FALL, "scenarios/flight/free-fall.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.has_no_red_flags();
}

#[test]
fn moving_a_scenario_and_loosening_a_rule_expectation_in_it_waits_for_the_maintainer() {
    let review = PullRequest::new("scenario-moved-and-loosened")
        .delete(FREE_FALL)
        .change(
            "scenarios/flight/free-fall.toml",
            "free-fall-rule-loosened.toml",
        )
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "`scenarios/flight/free-fall.toml`, moved from `scenarios/physics/free-fall.toml`: \
         \"vertical speed at 1 s\" went from `-9.81 m/s ± 0.00001 m/s` to \
         `-9.81 m/s ± 0.001 m/s`, a loosened tolerance",
    );
}

#[test]
fn renaming_a_scenario_and_its_file_and_loosening_a_rule_expectation_waits_for_the_maintainer() {
    // The Reviewer's case on #92: a new file, a new name, and "height at 1 s"
    // loosened from ± 0.001 m to ± 0.5 m.
    let review = PullRequest::new("scenario-renamed-and-loosened")
        .delete(FREE_FALL)
        .change(
            "scenarios/physics/free-fall-renamed.toml",
            "free-fall-renamed-and-loosened.toml",
        )
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "`scenarios/physics/free-fall-renamed.toml`, moved from \
         `scenarios/physics/free-fall.toml`: \"height at 1 s\" went from `-4.905 m ± 0.001 m` \
         to `-4.905 m ± 0.5 m`, a loosened tolerance",
    );
    assert!(
        !review.report.contains("A Scenario deleted"),
        "every Expectation was found again, so the Scenario only moved:\n{}",
        review.report
    );
}

#[test]
fn a_new_setup_under_rule_and_source_expectations_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("setup-changed")
        .change(FREE_FALL, "free-fall-setup-changed.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A Scenario's setup changed under its Source or Rule Expectations",
        "`scenarios/physics/free-fall.toml`: `inputs.timeline`, `start.random_seed` changed, so \
         its Source and Rule Expectations now check a different flight.",
    );
}

#[test]
fn a_new_setup_in_a_scenario_with_only_observed_expectations_raises_no_red_flag() {
    let review = PullRequest::new("observed-setup-changed")
        .write(
            "scenarios/feel/whoop-hover.toml",
            "format = 1\nname = \"The Whoop 65 hovers at the signed-off throttle\"\n\n\
             [start]\nrandom_seed = 2\n\n[[expect]]\nwhat = \"height\"\nat = \"5 s\"\n\
             value = \"2 m ± 0.1 m\"\nbasis = \"observed: the maintainer's Feel Test sign-off\"\n",
        )
        .review();
    review.has_no_red_flags();
}

#[test]
fn the_same_moment_spelled_another_way_raises_no_red_flag() {
    let review = PullRequest::new("moments-respelled")
        .change(FREE_FALL, "free-fall-respelled.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.has_no_red_flags();
}

// Observed Expectations and deleted Scenarios.

#[test]
fn an_updated_observed_expectation_is_listed_with_its_one_line_reason() {
    let review = PullRequest::new("observed-updated")
        .change(FREE_FALL, "free-fall-observed-updated.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.lists(
        "An Observed Expectation updated",
        "\"horizontal speed at 0.5 s\" went from `0 m/s ± 0.001 m/s` to \
         `0.0002 m/s ± 0.001 m/s`, and has a new basis line. Reason given: \"the stopped props \
         now leave a little sideways drift\".",
    );
}

#[test]
fn a_loosened_tolerance_on_an_observed_expectation_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("observed-loosened")
        .change(FREE_FALL, "free-fall-observed-loosened.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A tolerance loosened on an Observed Expectation",
        "went from `0 m/s ± 0.001 m/s` to `0 m/s ± 0.01 m/s`, a loosened tolerance",
    );
}

#[test]
fn the_same_observed_tolerance_written_in_other_units_is_not_loosened() {
    let review = PullRequest::new("observed-same-width")
        .change(FREE_FALL, "free-fall-observed-same-width.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.lists(
        "An Observed Expectation updated",
        "went from `0 m/s ± 0.001 m/s` to `0 m/s ± 1 mm/s`. Its basis line is unchanged, so \
         it gives no new reason.",
    );
    assert!(!review.report.contains("loosened"), "{}", review.report);
}

#[test]
fn removing_an_observed_expectation_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("observed-removed")
        .change(FREE_FALL, "free-fall-observed-removed.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "An Observed Expectation removed",
        "\"horizontal speed at 0.5 s\" (`0 m/s ± 0.001 m/s`) was removed",
    );
}

#[test]
fn a_deleted_scenario_with_only_observed_expectations_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("observed-scenario-deleted")
        .delete("scenarios/feel/whoop-hover.toml")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A Scenario deleted",
        "`scenarios/feel/whoop-hover.toml` (\"The Whoop 65 hovers at the signed-off throttle\"). \
         It must be replaced, or the ticket must ask for it.",
    );
}

#[test]
fn deleting_a_scenario_with_rule_and_source_expectations_waits_for_the_maintainer() {
    let review = PullRequest::new("scenario-deleted")
        .delete(FREE_FALL)
        .delete("scenarios/physics/free-fall.results.toml")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "A Rule Expectation changed",
        "`scenarios/physics/free-fall.toml`: \"height at 1 s\" (`-4.905 m ± 0.001 m`) was \
         removed",
    );
    review.waits_for_the_maintainer(
        "A Source Expectation changed",
        "\"vertical acceleration, mean over 0 s to 1 s\" (`-9.81 m/s² ± 0.01 m/s²`) was removed",
    );
    review.reviewer_decides("A Scenario deleted", "`scenarios/physics/free-fall.toml`");
}

#[test]
fn a_scenario_split_into_two_files_has_each_new_file_s_setup_compared() {
    // The Reviewer's case on #92: the first new file keeps the setup; the
    // second starts 100 m up at 1 kHz and holds "height at 1 s", unchanged.
    let review = PullRequest::new("scenario-split")
        .delete(FREE_FALL)
        .change(
            "scenarios/physics/free-fall-speed.toml",
            "free-fall-split-speed.toml",
        )
        .change(
            "scenarios/physics/free-fall-height.toml",
            "free-fall-split-height.toml",
        )
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A Scenario's setup changed under its Source or Rule Expectations",
        "`scenarios/physics/free-fall-height.toml`, moved from \
         `scenarios/physics/free-fall.toml`: `start.position`, `start.physics_rate` changed, so \
         its Source and Rule Expectations now check a different flight.",
    );
    assert!(
        !review
            .report
            .contains("`scenarios/physics/free-fall-speed.toml`, moved from"),
        "the first new file keeps the setup:\n{}",
        review.report
    );
    assert!(
        !review.report.contains("A Scenario deleted"),
        "every Expectation was found again:\n{}",
        review.report
    );
}

#[test]
fn a_changed_test_quad_under_a_scenario_with_rule_expectations_is_for_the_reviewer_to_decide() {
    // free-fall.toml doesn't change, but the Test Quad it flies does. A
    // Scenario with only Observed Expectations flying it raises nothing.
    let pull_request = PullRequest::new("test-quad-changed")
        .on_main(
            "scenarios/physics/drift-with-no-drag.toml",
            "format = 1\nname = \"No drift with no drag\"\n\n[start]\n\
             quad = \"test/whoop-65-no-drag\"\n\n[[expect]]\nwhat = \"east speed\"\n\
             at = \"1 s\"\nvalue = \"0 m/s ± 0.01 m/s\"\n\
             basis = \"observed: what the Simulation did when this was written\"\n",
        )
        .change(TEST_QUAD, "whoop-65-no-drag-with-rotor-drag.toml");
    // Read from two folders, and from two git commits, as CI reads them.
    for review in [pull_request.review(), pull_request.review_as_commits()] {
        assert!(review.gate_passed, "{}", review.output);
        review.reviewer_decides(
            "A Scenario's setup changed under its Source or Rule Expectations",
            "`scenarios/physics/free-fall.toml`: the Test Quad `test/whoop-65-no-drag` changed, \
             so its Source and Rule Expectations now check a different flight.",
        );
        assert!(
            !review.report.contains("drift-with-no-drag"),
            "an Observed-only Scenario raises nothing:\n{}",
            review.report
        );
    }
}

// ADRs, Bevy and wgpu.

#[test]
fn an_edited_existing_adr_waits_for_the_maintainer() {
    let review = PullRequest::new("adr-edited")
        .change(ADR, "adr-edited.md.txt")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer("An existing ADR edited", ADR);
}

#[test]
fn a_deleted_adr_waits_for_the_maintainer() {
    let review = PullRequest::new("adr-deleted").delete(ADR).review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer("An existing ADR deleted or renamed", ADR);
}

#[test]
fn a_new_adr_is_listed_only() {
    let review = PullRequest::new("adr-new")
        .write("docs/adr/0002-a-new-decision.md", "# A new decision\n")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.lists("A new ADR", "`docs/adr/0002-a-new-decision.md`");
}

#[test]
fn bevy_moving_to_a_new_0_n_with_its_wgpu_major_waits_for_the_maintainer() {
    let review = PullRequest::new("bevy-0.21")
        .change("Cargo.lock", "bevy-0.21.lock")
        .review();
    assert!(!review.gate_passed, "{}", review.output);
    review.waits_for_the_maintainer(
        "Bevy moves to a new version",
        "`bevy` goes from 0.20.0-rc.2 to 0.21.0",
    );
    review.waits_for_the_maintainer(
        "wgpu moves to a new version",
        "`wgpu` goes from 27.0.1 to 28.0.0",
    );
}

#[test]
fn bevy_s_release_candidate_becoming_the_release_raises_no_red_flag() {
    let review = PullRequest::new("bevy-0.20-final")
        .change("Cargo.lock", "bevy-0.20-final.lock")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    assert!(
        !review.report.contains("moves to a new version"),
        "{}",
        review.report
    );
}

#[test]
fn a_bevy_patch_release_raises_no_red_flag() {
    let review = PullRequest::new("bevy-0.20-patch")
        .change("Cargo.lock", "bevy-0.20-patch.lock")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    assert!(
        !review.report.contains("moves to a new version"),
        "{}",
        review.report
    );
}

// Flags for the Reviewer, and listed ones.

#[test]
fn allowing_a_house_rule_lint_in_a_core_crate_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("house-rule-exception")
        .change(
            "crates/physics/src/lib.rs",
            "physics-allows-std-maths.rs.txt",
        )
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::disallowed_methods`.",
    );
}

#[test]
fn a_char_literal_holding_a_quote_cannot_hide_a_house_rule_exception() {
    // The Reviewer's case on #92: `'"'` once looked like the start of a string.
    let review = PullRequest::new("char-literal-hides-allow")
        .change(
            "crates/physics/src/lib.rs",
            "physics-char-literal-hides-allow.rs.txt",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::disallowed_methods`.",
    );
}

#[test]
fn a_char_literal_holding_a_quote_cannot_hide_unsafe_code() {
    let review = PullRequest::new("char-literal-hides-unsafe")
        .change(
            "crates/sim/src/lib.rs",
            "sim-unsafe-after-char-literal.rs.txt",
        )
        .review();
    review.reviewer_decides(
        "New `unsafe` code",
        "`crates/sim/src/lib.rs` adds `let _q = '\"'; let first = unsafe { *x.as_ptr() };`.",
    );
}

#[test]
fn raw_strings_byte_strings_block_comments_and_lifetimes_are_not_code() {
    let review = PullRequest::new("words-not-code")
        .change(
            "crates/sim/src/lib.rs",
            "sim-unsafe-in-raw-strings-and-comments.rs.txt",
        )
        .review();
    assert!(
        !review.report.contains("New `unsafe` code"),
        "{}",
        review.report
    );
    assert!(
        !review.report.contains("A house-rule exception"),
        "{}",
        review.report
    );
}

#[test]
fn a_raw_c_string_cannot_hide_a_house_rule_exception() {
    // The Reviewer's case on #92: in `cr#"\"#` the backslash is text, so the
    // string ends right after it.
    let review = PullRequest::new("raw-c-string-hides-allow")
        .change(
            "crates/physics/src/lib.rs",
            "physics-raw-c-string-hides-allow.rs.txt",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::disallowed_methods`.",
    );
}

#[test]
fn no_string_prefix_or_suffix_rust_accepts_can_hide_unsafe_code_later_on_its_line() {
    // Each line compiles with Rust 1.99, and each literal ends before
    // `unsafe`, whatever its prefix, or a suffix before it, does to a
    // backslash.
    let lines = [
        // C strings and raw C strings.
        r#"let _p = c"\""; let first = unsafe { *x.as_ptr() };"#,
        r##"let _p = cr#"\"#; let first = unsafe { *x.as_ptr() }; let _q = "";"##,
        r#"let _p = cr"\"; let first = unsafe { *x.as_ptr() }; let _q = "";"#,
        // Byte strings, raw byte strings and byte chars.
        r#"let _p = b"\""; let first = unsafe { *x.as_ptr() };"#,
        r##"let _p = br#"\"#; let first = unsafe { *x.as_ptr() }; let _q = "";"##,
        r#"let _p = b'"'; let first = unsafe { *x.as_ptr() };"#,
        // A suffix right after a literal, which a macro's input allows: this
        // `r` is a suffix, not a raw string's prefix, so `\"` is an escape.
        r#"ignore!("a"r"\" "); let first = unsafe { *x.as_ptr() }; ignore!("");"#,
        r#"ignore!('a'r"\" "); let first = unsafe { *x.as_ptr() }; ignore!("");"#,
        r#"ignore!(r"a"r"\" "); let first = unsafe { *x.as_ptr() }; ignore!("");"#,
        r#"ignore!(c"a"r"\" "); let first = unsafe { *x.as_ptr() }; ignore!("");"#,
    ];
    for (i, line) in lines.iter().enumerate() {
        let review = PullRequest::new(&format!("literal-hides-unsafe-{i}"))
            .write(
                "crates/sim/src/lib.rs",
                &format!("{SIM_START}    {line}\n    first\n}}\n"),
            )
            .review();
        review.reviewer_decides(
            "New `unsafe` code",
            &format!("`crates/sim/src/lib.rs` adds `{line}`."),
        );
    }
}

#[test]
fn a_first_line_rust_skips_cannot_hide_unsafe_code_on_the_lines_after_it() {
    // Rust skips a first line that starts with `#!` (a shebang), quote and
    // all, so the next line is code.
    let review = PullRequest::new("shebang-hides-unsafe")
        .write(
            "crates/sim/src/lib.rs",
            "#!/usr/bin/env run \"\n\
             pub fn first(x: &[u8]) -> u8 { unsafe { *x.as_ptr() } }\n\
             // \"\n",
        )
        .review();
    review.reviewer_decides(
        "New `unsafe` code",
        "`crates/sim/src/lib.rs` adds `pub fn first(x: &[u8]) -> u8 { unsafe { *x.as_ptr() } }`.",
    );
}

#[test]
fn an_inner_attribute_after_an_invisible_mark_is_code_not_a_skipped_first_line() {
    // Rust reads U+200E, the left-to-right mark, as a space, so `#!` and the
    // mark before `[allow(…)]` make an inner attribute, not a shebang.
    let review = PullRequest::new("inner-attribute-after-a-mark")
        .write(
            "crates/physics/src/lib.rs",
            "#!\u{200E}[allow(clippy::disallowed_methods)]\n//! A fixture core crate.\n\n\
             pub fn sine(x: f64) -> f64 {\n    x.sin()\n}\n",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::disallowed_methods`.",
    );
}

#[test]
fn c_strings_raw_c_strings_literal_suffixes_and_a_skipped_first_line_are_not_code() {
    let review = PullRequest::new("c-strings-not-code")
        .change("crates/sim/src/lib.rs", "sim-unsafe-in-c-strings.rs.txt")
        .review();
    assert!(
        !review.report.contains("New `unsafe` code"),
        "{}",
        review.report
    );
    assert!(
        !review.report.contains("A house-rule exception"),
        "{}",
        review.report
    );
}

#[test]
fn a_house_rule_lint_allowed_over_several_lines_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("house-rule-split")
        .change(
            "crates/physics/src/lib.rs",
            "physics-allows-split-over-lines.rs.txt",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::disallowed_methods`.",
    );
}

#[test]
fn allowing_the_lint_group_that_holds_the_house_rules_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("house-rule-group")
        .change(
            "crates/physics/src/lib.rs",
            "physics-allows-style-group.rs.txt",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/src/lib.rs` now allows `clippy::style`.",
    );
}

#[test]
fn changing_a_core_crate_s_lint_settings_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("core-lint-settings")
        .write(
            "crates/physics/Cargo.toml",
            "[package]\nname = \"opendrone-physics\"\n\n\
             [lints.clippy]\ndisallowed_methods = \"allow\"\n",
        )
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/Cargo.toml` changes the crate's lint settings.",
    );
    review.reviewer_decides(
        "New `unsafe` code",
        "`crates/physics/Cargo.toml` adds no `[lints] workspace = true` any more, so `unsafe` \
         isn't forbidden there.",
    );
}

#[test]
fn a_dotted_clippy_toml_in_a_core_crate_is_a_house_rule_exception_and_a_repo_rules_change() {
    // Clippy reads `.clippy.toml` instead of the `clippy.toml` beside it, so
    // even an empty one turns the house rules off (the Reviewer's case on #92).
    let review = PullRequest::new("core-dotted-clippy-toml")
        .write("crates/physics/.clippy.toml", "")
        .review();
    review.reviewer_decides(
        "A house-rule exception in a core crate",
        "`crates/physics/.clippy.toml`. Clippy reads a `.clippy.toml` instead of the \
         `clippy.toml` beside it, which holds the house rules, so even an empty one turns them \
         off.",
    );
    review.reviewer_decides(
        "A change to the Repo rules",
        "`crates/physics/.clippy.toml`",
    );
}

#[test]
fn a_dotted_clippy_toml_in_an_edge_crate_is_a_repo_rules_change() {
    let review = PullRequest::new("edge-dotted-clippy-toml")
        .write("crates/input/.clippy.toml", "msrv = \"1.99\"\n")
        .review();
    review.reviewer_decides("A change to the Repo rules", "`crates/input/.clippy.toml`");
    assert!(
        !review.report.contains("A house-rule exception"),
        "input isn't a core crate:\n{}",
        review.report
    );
}

#[test]
fn an_edge_crate_dropping_the_workspace_s_lints_is_new_unsafe_code_for_the_reviewer() {
    let review = PullRequest::new("edge-crate-drops-lints")
        .write(
            "crates/pack/Cargo.toml",
            "[package]\nname = \"opendrone-pack\"\n",
        )
        .review();
    review.reviewer_decides(
        "New `unsafe` code",
        "`crates/pack/Cargo.toml` adds no `[lints] workspace = true` any more",
    );
    assert!(
        !review.report.contains("A house-rule exception"),
        "pack isn't a core crate:\n{}",
        review.report
    );
}

#[test]
fn the_same_lint_allowed_in_an_edge_crate_is_no_house_rule_exception() {
    let review = PullRequest::new("edge-crate-allows-std-maths")
        .change("crates/input/src/lib.rs", "physics-allows-std-maths.rs.txt")
        .review();
    assert!(
        !review.report.contains("A house-rule exception"),
        "{}",
        review.report
    );
}

#[test]
fn new_unsafe_code_is_for_the_reviewer_to_decide() {
    let review = PullRequest::new("unsafe-added")
        .change("crates/sim/src/lib.rs", "sim-with-unsafe.rs.txt")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "New `unsafe` code",
        "`crates/sim/src/lib.rs` adds `let first = unsafe { *x.as_ptr() };`.",
    );
}

#[test]
fn the_word_unsafe_in_a_comment_a_message_or_a_longer_name_is_not_unsafe_code() {
    let review = PullRequest::new("unsafe-in-words")
        .change("crates/sim/src/lib.rs", "sim-unsafe-in-words.rs.txt")
        .review();
    assert!(
        !review.report.contains("New `unsafe` code"),
        "{}",
        review.report
    );
}

#[test]
fn allowing_unsafe_code_in_a_crate_is_for_the_reviewer_to_decide() {
    let in_the_manifest = PullRequest::new("unsafe-allowed-in-manifest")
        .write(
            "crates/sim/Cargo.toml",
            "[package]\nname = \"opendrone-sim\"\n\n[lints.rust]\nunsafe_code = \"allow\"\n",
        )
        .review();
    in_the_manifest.reviewer_decides(
        "New `unsafe` code",
        "`crates/sim/Cargo.toml` adds its own `unsafe_code` setting, `\"allow\"`.",
    );
    let in_the_code = PullRequest::new("unsafe-allowed-in-code")
        .write(
            "crates/sim/src/lib.rs",
            "#![allow(unsafe_code)]\n//! A crate.\n",
        )
        .review();
    in_the_code.reviewer_decides(
        "New `unsafe` code",
        "`crates/sim/src/lib.rs` adds an allowance of the `unsafe_code` lint.",
    );
}

#[test]
fn a_change_to_ci_workflows_says_the_pr_s_own_statuses_can_t_be_trusted() {
    let review = PullRequest::new("workflow-changed")
        .write(
            ".github/workflows/extra.yml",
            "name: Extra\non: pull_request\npermissions:\n  statuses: write\n",
        )
        .review();
    review.reviewer_decides(
        "A change to CI workflows",
        "`.github/workflows/extra.yml`. A workflow can set any commit status, so for this PR the \
         Red Flag gate's and the Review check's own results can't be trusted.",
    );
}

#[test]
fn a_file_name_cannot_hide_behind_invisible_formatting_characters() {
    let review = PullRequest::new("bidi-file-name")
        .write("docs/notes-\u{202E}dm.txt", "Notes.\n")
        .review();
    assert!(!review.report.contains('\u{202E}'), "{}", review.report);
    review.says("`docs/notes-\\u{202E}dm.txt`");
}

#[test]
fn the_areas_come_from_main_s_codeowners_when_ci_gives_it() {
    let review = PullRequest::new("main-codeowners")
        .write("README.md", "# OpenDrone\n")
        .review_with_codeowners("# Front page\n/README.md @BartoszSolkaBD\n");
    assert_eq!(review.json["areas"], serde_json::json!(["Front page"]));
    review.says("- **Front page**: `README.md`");
}

#[test]
fn a_change_to_ci_or_agents_md_is_a_repo_rules_change_for_the_reviewer_to_decide() {
    let review = PullRequest::new("repo-rules")
        .write(".github/workflows/ci.yml", "name: CI\n")
        .write("AGENTS.md", "# AGENTS.md\n")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.reviewer_decides(
        "A change to the Repo rules",
        "`.github/workflows/ci.yml`, `AGENTS.md`. The ticket must ask for it",
    );
}

#[test]
fn a_new_outside_library_is_listed_with_its_licence() {
    let review = PullRequest::new("new-library")
        .change("Cargo.lock", "new-library.lock")
        .review_with_metadata("new-library-metadata.json");
    assert!(review.gate_passed, "{}", review.output);
    review.lists(
        "A new outside library",
        "`smallvec` 1.15.1 (licence: MIT OR Apache-2.0).",
    );
    review.says("| smallvec | 1.15.1 | MIT OR Apache-2.0 |");
    assert!(
        !review.report.contains("`libm`") && !review.report.contains("`opendrone-sim`"),
        "a new version of a library, or a new OpenDrone crate, isn't a new outside library:\n{}",
        review.report
    );
}

#[test]
fn text_from_a_pull_request_cannot_fake_the_report_s_structure_or_mention_anyone() {
    let review = PullRequest::new("hostile-text")
        .change(FREE_FALL, "free-fall-observed-hostile-reason.toml")
        .review();
    review.lists("An Observed Expectation updated", "Reason given:");
    assert!(!review.report.contains("<!--"), "{}", review.report);
    assert!(!review.report.contains("<pre>"), "{}", review.report);
    assert!(
        review
            .report
            .lines()
            .all(|line| !line.contains("Verdict") || !line.starts_with('#')),
        "a reason can't start a line, so it can't make a Verdict heading:\n{}",
        review.report
    );
    review.says("&lt;\\!-- opendrone-review-report --&gt; \\#\\#\\# Verdict");
    review.says("\\[click\\](https://example.com) @\u{200B}someone");
}

#[test]
fn a_new_glossary_term_is_listed_only() {
    let review = PullRequest::new("new-term")
        .change("docs/context/flying.md", "flying-new-term.md.txt")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.lists(
        "A new glossary term",
        "**Rates**, in `docs/context/flying.md`.",
    );
}

// What moved.

#[test]
fn what_moved_lists_every_moved_expectation_biggest_move_first_with_before_and_after() {
    let review = PullRequest::new("moved")
        .change(FREE_FALL_RESULTS, "free-fall-moved.results.toml")
        .change(FREE_TUMBLE_RESULTS, "free-tumble-moved.results.toml")
        .review();
    let rows = review.table_rows("### What moved");
    assert_eq!(
        rows,
        [
            "| Free fall is exactly g | no broken numbers: every number in the state is a real \
             number after every step | none broken | after step 4000, vertical speed is not a \
             number | changed |",
            "| Free fall is exactly g | horizontal speed at 0.5 s | 0 m/s | 0.0002 m/s | from \
             zero |",
            "| Free fall is exactly g | height at 1 s | -4.91 m | -5.40 m | −10.0% |",
            "| A free tumble keeps its spin | roll at 1.125 s | 90.0° | 91.0° | +1.1% |",
            "| Free fall is exactly g | vertical speed at 1 s | -9.81 m/s | -9.83 m/s | −0.20% |",
        ],
        "{}",
        review.report
    );
    assert!(
        !review.report.contains("vertical acceleration, mean over"),
        "an Expectation whose value didn't move isn't listed:\n{}",
        review.report
    );
}

#[test]
fn a_move_too_small_for_3_significant_figures_shows_in_the_fingerprints() {
    let review = PullRequest::new("fingerprint-only")
        .change(
            FREE_TUMBLE_RESULTS,
            "free-tumble-fingerprint-only.results.toml",
        )
        .review();
    review.says("No measured value moved.");
    review.says(
        "- A free tumble keeps its spin: the flight's fingerprint moved, first seen at the \
         1.125 s checkpoint while every measured value stayed the same to 3 significant figures",
    );
}

#[test]
fn a_new_scenario_shows_as_new_in_what_moved() {
    let review = PullRequest::new("new-scenario")
        .copy(
            "scenarios/physics/free-fall.toml",
            "scenarios/physics/free-fall-again.toml",
        )
        .copy(
            FREE_FALL_RESULTS,
            "scenarios/physics/free-fall-again.results.toml",
        )
        .review();
    review.says("- Free fall is exactly g: a new Scenario, with 5 Expectations");
}

#[test]
fn nothing_moves_when_no_results_file_changed() {
    let review = PullRequest::new("nothing-moved")
        .write("README.md", "# OpenDrone\n")
        .review();
    review.says("Nothing moved: no Results file changed");
}

// Areas, and the sections that aren't measured yet.

#[test]
fn the_areas_touched_come_from_the_folders_in_codeowners() {
    let review = PullRequest::new("areas")
        .change(
            "crates/physics/src/lib.rs",
            "physics-allows-std-maths.rs.txt",
        )
        .change(FREE_FALL, "free-fall-reordered.toml")
        .write("README.md", "# OpenDrone\n")
        .review();
    assert_eq!(
        review.json["areas"],
        serde_json::json!(["Physics", "Scenarios"])
    );
    review.says("- **Physics**: `crates/physics/src/lib.rs`");
    review.says("- **Scenarios**: `scenarios/physics/free-fall.toml`");
    review.says("- **Everything else (no Area)**: `README.md`");
}

#[test]
fn speed_renders_and_downloads_say_they_are_not_measured_yet() {
    let review = PullRequest::new("not-yet")
        .write("README.md", "# OpenDrone\n")
        .review();
    review.says("### Speed\n\nNot measured yet.");
    review.says("### Renders\n\nNo Map renders yet.");
    review.says("### Downloads\n\nNo Blackbox logs or builds yet.");
}

#[test]
fn a_pull_request_with_nothing_to_flag_passes_the_red_flag_gate() {
    let review = PullRequest::new("clean")
        .write("README.md", "# OpenDrone\n")
        .review();
    assert!(review.gate_passed, "{}", review.output);
    review.has_no_red_flags();
    assert_eq!(review.json["waits_for_maintainer"], false);
}

const FREE_FALL: &str = "scenarios/physics/free-fall.toml";
const FREE_FALL_RESULTS: &str = "scenarios/physics/free-fall.results.toml";
const FREE_TUMBLE_RESULTS: &str = "scenarios/physics/free-tumble.results.toml";
const ADR: &str = "docs/adr/0001-bit-exact-determinism.md";
const TEST_QUAD: &str = "scenarios/test-quads/whoop-65-no-drag.toml";
/// The start of a crate's `lib.rs`, up to a line inside a function that
/// reads the first byte of `x` into `first`.
const SIM_START: &str = "//! A fixture crate.\n\nmacro_rules! ignore {\n    ($($t:tt)*) => {};\n}\n\n\
                         /// The first byte.\npub fn first_byte(x: &[u8]) -> u8 {\n";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/review")
}

/// A pull request on the fixture repo: its base and head as two folders.
struct PullRequest {
    root: PathBuf,
    metadata: Option<PathBuf>,
    /// Main's CODEOWNERS, as CI gives it.
    codeowners: Option<PathBuf>,
}

impl PullRequest {
    fn new(name: &str) -> PullRequest {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("review-report")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        let codeowners = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/CODEOWNERS");
        for side in ["base", "head"] {
            copy_folder(&fixtures().join("repo"), &root.join(side));
            let target = root.join(side).join(".github/CODEOWNERS");
            fs::create_dir_all(target.parent().expect("a folder")).expect("can make .github");
            fs::copy(&codeowners, target).expect("can copy CODEOWNERS");
        }
        PullRequest {
            root,
            metadata: None,
            codeowners: None,
        }
    }

    fn head(&self, path: &str) -> PathBuf {
        self.root.join("head").join(path)
    }

    /// The head's `path` becomes the fixture `changes/<fixture>`.
    fn change(self, path: &str, fixture: &str) -> PullRequest {
        let text = fs::read_to_string(fixtures().join("changes").join(fixture))
            .unwrap_or_else(|_| panic!("the fixture {fixture} exists"));
        self.write(path, &text)
    }

    fn write(self, path: &str, text: &str) -> PullRequest {
        let target = self.head(path);
        fs::create_dir_all(target.parent().expect("a folder")).expect("can make the folder");
        fs::write(target, text).expect("can write the file");
        self
    }

    /// A file already on main: the same in the base and the head.
    fn on_main(self, path: &str, text: &str) -> PullRequest {
        let target = self.root.join("base").join(path);
        fs::create_dir_all(target.parent().expect("a folder")).expect("can make the folder");
        fs::write(target, text).expect("can write the file");
        self.write(path, text)
    }

    fn copy(self, from: &str, to: &str) -> PullRequest {
        let text = fs::read_to_string(self.head(from)).expect("the file exists");
        self.write(to, &text)
    }

    fn delete(self, path: &str) -> PullRequest {
        fs::remove_file(self.head(path)).expect("the file exists");
        self
    }

    fn rename(self, from: &str, to: &str) -> PullRequest {
        self.copy(from, to).delete(from)
    }

    fn review_with_codeowners(mut self, text: &str) -> Review {
        let path = self.root.join("main-CODEOWNERS");
        fs::write(&path, text).expect("can write CODEOWNERS");
        self.codeowners = Some(path);
        self.review()
    }

    fn review_with_metadata(mut self, fixture: &str) -> Review {
        self.metadata = Some(fixtures().join("changes").join(fixture));
        self.review()
    }

    /// Runs `cargo xtask review-report` on the base and the head.
    fn review(&self) -> Review {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .arg("review-report")
            .arg("--before")
            .arg(self.root.join("base"))
            .arg("--after")
            .arg(self.root.join("head"));
        self.run(command, "out")
    }

    /// Runs `cargo xtask review-report` the way CI does: on two git commits,
    /// the base and the head, which it reads with git alone.
    fn review_as_commits(&self) -> Review {
        let repo = self.root.join("git");
        let _ = fs::remove_dir_all(&repo);
        copy_exactly(&self.root.join("base"), &repo);
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "--quiet", "--message", "base"]);
        for entry in fs::read_dir(&repo).expect("can read the repository") {
            let entry = entry.expect("can read the entry");
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().expect("has a type").is_dir() {
                fs::remove_dir_all(entry.path()).expect("can remove the folder");
            } else {
                fs::remove_file(entry.path()).expect("can remove the file");
            }
        }
        copy_exactly(&self.root.join("head"), &repo);
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "--quiet", "--message", "head"]);
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(["review-report", "--base", "HEAD^", "--head", "HEAD"])
            .current_dir(&repo);
        isolate_git(&mut command, &repo);
        self.run(command, "out-commits")
    }

    /// Runs a `review-report` command, with `--out` in the folder `out`.
    fn run(&self, mut command: Command, out: &str) -> Review {
        let out = self.root.join(out);
        command.arg("--out").arg(&out);
        if let Some(metadata) = &self.metadata {
            command.arg("--metadata").arg(metadata);
        }
        if let Some(codeowners) = &self.codeowners {
            command.arg("--codeowners").arg(codeowners);
        }
        let output = command.output().expect("xtask runs");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "review-report broke:\n{text}"
        );
        Review {
            gate_passed: output.status.success(),
            output: text,
            report: fs::read_to_string(out.join("report.md")).expect("report.md was written"),
            json: serde_json::from_str(
                &fs::read_to_string(out.join("review.json")).expect("review.json was written"),
            )
            .expect("review.json is JSON"),
        }
    }
}

/// What `review-report` said about a pull request.
struct Review {
    gate_passed: bool,
    output: String,
    report: String,
    json: Value,
}

impl Review {
    fn flag(&self, level: &str, title: &str, detail: &str) {
        let found = self.json["red_flags"]
            .as_array()
            .expect("red_flags is a list")
            .iter()
            .any(|flag| {
                flag["level"] == level
                    && flag["title"] == title
                    && flag["detail"].as_str().is_some_and(|d| d.contains(detail))
            });
        assert!(
            found,
            "expected the {level} Red Flag \"{title}\" saying \"{detail}\", but the Report \
             says:\n{}",
            self.report
        );
        self.says(&format!("- **{title}**: "));
    }

    fn waits_for_the_maintainer(&self, title: &str, detail: &str) {
        self.flag("waits-for-maintainer", title, detail);
        assert_eq!(self.json["waits_for_maintainer"], true);
        self.says("**Waits for the maintainer.** The Red Flag gate fails");
    }

    fn reviewer_decides(&self, title: &str, detail: &str) {
        self.flag("reviewer-decides", title, detail);
        self.says("**The Reviewer decides.**");
    }

    fn lists(&self, title: &str, detail: &str) {
        self.flag("listed-only", title, detail);
        self.says("**Listed only.**");
    }

    fn has_label_needs(&self, label: &str) {
        self.says(&format!("this PR gets the `{label}` label"));
    }

    fn has_no_red_flags(&self) {
        self.says("### Red Flags\n\nNone.");
    }

    fn says(&self, text: &str) {
        assert!(
            self.report.contains(text),
            "expected the Report to say:\n{text}\nbut it says:\n{}",
            self.report
        );
    }

    /// The rows of the first table after `heading`.
    fn table_rows(&self, heading: &str) -> Vec<String> {
        let after = self
            .report
            .split_once(heading)
            .unwrap_or_else(|| panic!("no {heading} in:\n{}", self.report))
            .1;
        after
            .lines()
            .skip_while(|line| !line.starts_with('|'))
            .take_while(|line| line.starts_with('|'))
            .skip(2)
            .map(str::to_string)
            .collect()
    }
}

/// Copies a folder as it is.
fn copy_exactly(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("can make the folder");
    for entry in fs::read_dir(from).expect("can read the folder") {
        let entry = entry.expect("can read the entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("has a type").is_dir() {
            copy_exactly(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("can copy the file");
        }
    }
}

/// Git's own settings, as on a fresh CI runner, and no repository but `repo`.
fn isolate_git(command: &mut Command, repo: &Path) {
    command
        .env("GIT_CONFIG_GLOBAL", repo.join("no-global-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
}

/// Runs git in `repo`.
fn git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "user.name=Scratch",
            "-c",
            "user.email=scratch@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo);
    isolate_git(&mut command, repo);
    let output = command.output().expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Copies a folder; a fixture's `.txt` ending comes off, so `lib.rs.txt`
/// becomes `lib.rs`.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("can make the folder");
    for entry in fs::read_dir(from).expect("can read the fixture folder") {
        let entry = entry.expect("can read the entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(name.strip_suffix(".txt").unwrap_or(&name));
        if entry.file_type().expect("has a type").is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("can copy the file");
        }
    }
}
