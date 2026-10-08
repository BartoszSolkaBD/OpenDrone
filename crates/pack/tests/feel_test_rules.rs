//! Readable checks for the Feel Test log rules CI runs on every pull request
//! (#16 §3): each compares the good fixture Quad before a change with the same
//! Quad after it. Basis: Rule (#16 §3, the Flying deep dive's rules).

mod common;

use common::{LOG, QUAD, good_fixture, line_of};
use opendrone_pack::feel_tests::{
    FeelTestReport, PacksVersion, QuadFiles, QuadVersion, RetiredQuad, check_feel_test_rules,
    compare_packs,
};

/// The fixture's Quad definition and Feel Test log, before any change.
fn before() -> (String, String) {
    let read = |file: &str| std::fs::read_to_string(good_fixture().join(file)).unwrap();
    (read(QUAD), read(LOG))
}

/// The problems a change to the Quad definition and its log brings.
fn problems(quad: &str, log: &str) -> Vec<String> {
    let (quad_before, log_before) = before();
    check_feel_test_rules(
        QUAD,
        LOG,
        QuadVersion {
            quad: Some(&quad_before),
            feel_tests: Some(&log_before),
        },
        QuadVersion {
            quad: Some(quad),
            feel_tests: Some(log),
        },
    )
    .0
    .iter()
    .map(ToString::to_string)
    .collect()
}

fn changed(old: &str, new: &str) -> String {
    let (quad, _) = before();
    assert_eq!(quad.matches(old).count(), 1, "{old:?}");
    quad.replacen(old, new, 1)
}

fn log_with(row: &str) -> String {
    before().1 + row + "\n"
}

fn at(line: usize, sentence: &str) -> String {
    format!("{QUAD} line {line}: {sentence}")
}

fn log_at(line: usize, sentence: &str) -> String {
    format!("{LOG} line {line}: {sentence}")
}

#[test]
fn an_estimate_that_moves_inside_its_range_with_a_new_log_row_passes() {
    let quad = changed("\"0.3 s⁻¹\"", "\"0.35 s⁻¹\"");
    let log = log_with(
        "| 2026-11-09 | [props] rotor_drag | 0.3 s⁻¹ → 0.35 s⁻¹ | still too floaty after a sprint |",
    );
    assert_eq!(problems(&quad, &log), Vec::<String>::new());
}

#[test]
fn an_estimate_that_moves_without_a_new_log_row_is_refused() {
    let quad = changed("\"0.3 s⁻¹\"", "\"0.35 s⁻¹\"");
    assert_eq!(
        problems(&quad, &before().1),
        [at(
            line_of(&quad, "0.35 s⁻¹"),
            "[props] rotor_drag is an Estimate that moved from 0.3 s⁻¹ to 0.35 s⁻¹, so packs/fixture/quads/ducted/feel-tests.md needs a new row for it: the date, [props] rotor_drag, 0.3 s⁻¹ → 0.35 s⁻¹, and why"
        )]
    );
}

#[test]
fn a_log_row_must_name_the_move_that_was_made() {
    let quad = changed("\"0.3 s⁻¹\"", "\"0.35 s⁻¹\"");
    let log =
        log_with("| 2026-11-09 | [props] rotor_drag | 0.3 s⁻¹ → 0.4 s⁻¹ | a typo in the log |");
    assert_eq!(
        problems(&quad, &log),
        [
            log_at(
                line_of(&log, "a typo"),
                "this row records [props] rotor_drag moving from 0.3 s⁻¹ to 0.4 s⁻¹, but this change moves it from 0.3 s⁻¹ to 0.35 s⁻¹"
            ),
            at(
                line_of(&quad, "0.35 s⁻¹"),
                "[props] rotor_drag is an Estimate that moved from 0.3 s⁻¹ to 0.35 s⁻¹, so packs/fixture/quads/ducted/feel-tests.md needs a new row for it: the date, [props] rotor_drag, 0.3 s⁻¹ → 0.35 s⁻¹, and why"
            ),
        ]
    );
}

#[test]
fn a_log_row_and_the_quad_are_compared_as_numbers_with_their_units() {
    let quad = changed(
        "\"35 ms\", confidence = \"Estimate\", range = \"20–50 ms\", source = \"guess\" }\nslow_down",
        "\"40 ms\", confidence = \"Estimate\", range = \"20–50 ms\", source = \"guess\" }\nslow_down",
    );
    let log = log_with("| 2026-11-09 | [motors] spin_up | 0.035 s → 0.04 s | punches felt soft |");
    assert_eq!(problems(&quad, &log), Vec::<String>::new());
}

#[test]
fn an_invented_earlier_row_doesnt_widen_a_relative_range() {
    // Reviewer's first case: inertia moved 20×, with an invented row ahead of
    // the real one that would make the move look small.
    let quad = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 1400, pitch 1800, yaw 2800 g·cm²",
    );
    let log = log_with(
        "| 2026-11-09 | [frame] inertia | roll 700, pitch 900, yaw 1400 g·cm² → roll 70, pitch 90, yaw 140 g·cm² | made up |\n| 2026-11-09 | [frame] inertia | roll 70, pitch 90, yaw 140 g·cm² → roll 1400, pitch 1800, yaw 2800 g·cm² | the real move |",
    );
    assert_eq!(
        problems(&quad, &log),
        [
            log_at(
                line_of(&log, "made up"),
                "this row records [frame] inertia moving from roll 700, pitch 900, yaw 1400 g·cm² to roll 70, pitch 90, yaw 140 g·cm², but this change moves it from roll 70, pitch 90, yaw 140 g·cm² to roll 1400, pitch 1800, yaw 2800 g·cm²"
            ),
            log_at(
                line_of(&log, "the real move"),
                "this change already has a row for [frame] inertia; one move, one row"
            ),
            at(
                line_of(&quad, "roll 1400"),
                "[frame] inertia moved to roll 1400, pitch 1800, yaw 2800 g·cm², outside its range, ×0.5–×2, measured from roll 70, pitch 90, yaw 140 g·cm², the value it started at"
            ),
        ]
    );
}

#[test]
fn a_row_for_a_move_that_didnt_happen_is_refused() {
    // Reviewer's second case: nothing in the Quad changes, but a row claims a
    // move, which would become the start for later moves.
    let (quad, _) = before();
    let log = log_with(
        "| 2026-11-09 | [frame] inertia | roll 7, pitch 9, yaw 14 g·cm² → roll 70, pitch 90, yaw 140 g·cm² | never happened |",
    );
    assert_eq!(
        problems(&quad, &log),
        [log_at(
            line_of(&log, "never happened"),
            "this row records [frame] inertia moving from roll 7, pitch 9, yaw 14 g·cm² to roll 70, pitch 90, yaw 140 g·cm², but this change doesn't move it"
        )]
    );
}

#[test]
fn a_row_whose_value_cant_be_read_is_refused() {
    // Reviewer's third case: an unreadable old value can't excuse a 100× move.
    let quad = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 7000, pitch 9000, yaw 14000 g·cm²",
    );
    let log = log_with(
        "| 2026-11-09 | [frame] inertia | n/a → roll 7000, pitch 9000, yaw 14000 g·cm² | unknown before |",
    );
    let found = problems(&quad, &log);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with(&log_at(
            line_of(&log, "unknown before"),
            "the old value, \"n/a\", doesn't read as [frame] inertia:"
        )),
        "{found:?}"
    );
}

#[test]
fn a_curve_that_changes_its_number_of_points_needs_a_new_source() {
    let old = "4.35 V at 100%, 3.92 V at 50%, 3.30 V at 0%";
    let new = "4.35 V at 100%, 4.10 V at 75%, 3.92 V at 50%, 3.30 V at 0%";
    let quad = changed(old, new);
    let log = log_with(&format!(
        "| 2026-11-09 | [battery] voltage_curve | {old} → {new} | more detail |"
    ));
    assert_eq!(
        problems(&quad, &log),
        [at(
            line_of(&quad, "4.10 V at 75%"),
            "[battery] voltage_curve: it now has 4 points and had 3, which changes what's known about it, so it needs a new source"
        )]
    );
}

#[test]
fn an_estimate_that_moves_outside_its_range_is_refused_even_with_a_log_row() {
    let quad = changed("\"0.3 s⁻¹\"", "\"0.7 s⁻¹\"");
    let log =
        log_with("| 2026-11-09 | [props] rotor_drag | 0.3 s⁻¹ → 0.7 s⁻¹ | wanted more carve |");
    assert_eq!(
        problems(&quad, &log),
        [at(
            line_of(&quad, "0.7 s⁻¹"),
            "[props] rotor_drag: \"0.7 s⁻¹\" is outside its range, 0.1–0.6 s⁻¹"
        )]
    );
}

#[test]
fn a_relative_range_is_measured_from_the_value_the_estimate_started_at() {
    // The inertia started at roll 70 (its first row's old value) and may move
    // ×0.5–×2 from there. Doubling it once is inside; doubling it again isn't,
    // though it is only ×2 from the value before this change.
    let first = "| 2026-11-09 | [frame] inertia | roll 70, pitch 90, yaw 140 g·cm² → roll 140, pitch 180, yaw 280 g·cm² | flips too snappy |";
    let (quad_before, log_before) = before();
    let quad_middle = quad_before.replace(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 140, pitch 180, yaw 280 g·cm²",
    );
    let log_middle = log_before + first + "\n";
    let quad_after = quad_before.replace(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 280, pitch 360, yaw 560 g·cm²",
    );
    let log_after = log_middle.clone()
        + "| 2026-11-16 | [frame] inertia | roll 140, pitch 180, yaw 280 g·cm² → roll 280, pitch 360, yaw 560 g·cm² | still too snappy |\n";
    let found = check_feel_test_rules(
        QUAD,
        LOG,
        QuadVersion {
            quad: Some(&quad_middle),
            feel_tests: Some(&log_middle),
        },
        QuadVersion {
            quad: Some(&quad_after),
            feel_tests: Some(&log_after),
        },
    );
    assert_eq!(
        found.to_string(),
        at(
            line_of(&quad_after, "roll 280"),
            "[frame] inertia moved to roll 280, pitch 360, yaw 560 g·cm², outside its range, ×0.5–×2, measured from roll 70, pitch 90, yaw 140 g·cm², the value it started at"
        )
    );
}

#[test]
fn a_locked_number_that_changes_without_a_new_source_is_refused() {
    let quad = changed("\"23.0 g\"", "\"24.0 g\"");
    assert_eq!(
        problems(&quad, &before().1),
        [at(
            line_of(&quad, "24.0 g"),
            "[frame] dry_mass is Manufacturer, so it's locked: it changed from 23.0 g to 24.0 g without a new source; name a new source, or update its line in [sources]"
        )]
    );
}

#[test]
fn a_locked_number_may_change_with_a_new_source_key_or_a_new_source_line() {
    let new_key = changed(
        "dry_mass     = { value = \"23.0 g\", confidence = \"Manufacturer\", source = \"maker\" }",
        "dry_mass     = { value = \"24.0 g\", confidence = \"Measured\", source = \"guess\" }",
    );
    // A new Confidence comes with a new source here, so it passes.
    assert_eq!(problems(&new_key, &before().1), Vec::<String>::new());
    let new_line = changed("\"23.0 g\"", "\"24.0 g\"").replace(
        "maker      = \"the maker's page\"",
        "maker      = \"the maker's page, version 2 (read 2026-11-09)\"",
    );
    assert_eq!(problems(&new_line, &before().1), Vec::<String>::new());
}

#[test]
fn an_estimates_range_changes_only_with_a_new_source() {
    let quad = changed("range = \"0.1–0.6 s⁻¹\"", "range = \"0.1–1 s⁻¹\"");
    assert_eq!(
        problems(&quad, &before().1),
        [at(
            line_of(&quad, "0.1–1 s⁻¹"),
            "[props] rotor_drag: its range changed from 0.1–0.6 s⁻¹ to 0.1–1 s⁻¹, which changes what's known about it, so it needs a new source too"
        )]
    );
}

#[test]
fn an_estimate_that_moves_with_a_new_range_and_source_still_needs_its_log_row() {
    // Re-sourcing a range is allowed, but the move itself is still logged.
    let quad = changed(
        "\"0.3 s⁻¹\", confidence = \"Estimate\", range = \"0.1–0.6 s⁻¹\", source = \"guess\"",
        "\"0.8 s⁻¹\", confidence = \"Estimate\", range = \"0.1–1 s⁻¹\", source = \"worked-out\"",
    );
    assert_eq!(
        problems(&quad, &before().1),
        [at(
            line_of(&quad, "0.8 s⁻¹"),
            "[props] rotor_drag is an Estimate that moved from 0.3 s⁻¹ to 0.8 s⁻¹, so packs/fixture/quads/ducted/feel-tests.md needs a new row for it: the date, [props] rotor_drag, 0.3 s⁻¹ → 0.8 s⁻¹, and why"
        )]
    );
}

#[test]
fn a_confidence_changes_only_with_a_new_source() {
    let quad = changed(
        "\"0.3 s⁻¹\", confidence = \"Estimate\", range = \"0.1–0.6 s⁻¹\"",
        "\"0.3 s⁻¹\", confidence = \"Measured\"",
    );
    assert_eq!(
        problems(&quad, &before().1),
        [at(
            line_of(&quad, "\"0.3 s⁻¹\", confidence = \"Measured\""),
            "[props] rotor_drag: its Confidence changed from Estimate to Measured, which changes what's known about it, so it needs a new source too"
        )]
    );
}

#[test]
fn the_feel_test_log_only_grows() {
    let (quad, log) = before();
    let log = log.replace("carved too wide after a sprint", "carved a bit wide");
    assert_eq!(
        problems(&quad, &log),
        [
            "packs/fixture/quads/ducted/feel-tests.md: the Feel Test log only grows: a row from before this change was changed or removed; put it back and add a new row instead"
        ]
    );
}

#[test]
fn counts_choices_camera_defaults_and_the_sound_block_move_freely() {
    let quad = changed("blades             = 3", "blades             = 4")
        .replace("fov         = \"160°\"", "fov         = \"150°\"")
        .replace(
            "hit_level             = \"0.5\"",
            "hit_level             = \"0.7\"",
        );
    assert_eq!(problems(&quad, &before().1), Vec::<String>::new());
}

#[test]
fn a_new_quad_has_nothing_to_compare() {
    let (quad, log) = before();
    let found = check_feel_test_rules(
        QUAD,
        LOG,
        QuadVersion::default(),
        QuadVersion {
            quad: Some(&quad),
            feel_tests: Some(&log),
        },
    );
    assert!(found.is_empty());
}

#[test]
fn a_version_an_older_checker_passed_is_still_compared_number_by_number() {
    // Before #40 a Quad file held only a few sections; whatever of it still
    // reads is compared.
    let older = "format = 1\nname = \"Ducted\"\n\n[frame]\ndry_mass = { value = \"23.0 g\", confidence = \"Manufacturer\", source = \"maker\" }\n\n[sources]\nmaker = \"the maker's page\"\n";
    let (quad, log) = before();
    let quad = quad.replacen("\"23.0 g\"", "\"24.0 g\"", 1);
    let found = check_feel_test_rules(
        QUAD,
        LOG,
        QuadVersion {
            quad: Some(older),
            feel_tests: Some(&log),
        },
        QuadVersion {
            quad: Some(&quad),
            feel_tests: Some(&log),
        },
    );
    assert_eq!(
        found.to_string(),
        at(
            line_of(&quad, "24.0 g"),
            "[frame] dry_mass is Manufacturer, so it's locked: it changed from 23.0 g to 24.0 g without a new source; name a new source, or update its line in [sources]"
        )
    );
}

/// The full report of a change: problems and the numbers that passed on a
/// new source.
fn report(quad: &str, log: &str) -> FeelTestReport {
    let (quad_before, log_before) = before();
    compare_packs(
        &holding(&[files("fixture/ducted", &quad_before, &log_before)]),
        &holding(&[files("fixture/ducted", quad, log)]),
    )
}

/// A version of the Packs holding these Quads, and retiring none.
fn holding(quads: &[QuadFiles]) -> PacksVersion {
    PacksVersion {
        quads: quads.to_vec(),
        retired: Vec::new(),
    }
}

/// The fixture Pack's line retiring its Quad, `fixture/ducted`.
fn retiring_the_fixture_quad() -> RetiredQuad {
    RetiredQuad {
        id: "fixture/ducted".to_string(),
        manifest_file: "packs/fixture/pack.toml".to_string(),
        line: 14,
        why: "Replaced by a 75 mm whoop with the same ducts.".to_string(),
    }
}

fn files(id: &str, quad: &str, log: &str) -> QuadFiles {
    let folder = format!(
        "packs/{}/quads/{}",
        id.split('/').next().unwrap(),
        id.split('/').nth(1).unwrap()
    );
    QuadFiles {
        id: id.to_string(),
        quad_file: format!("{folder}/quad.toml"),
        log_file: format!("{folder}/feel-tests.md"),
        quad: quad.to_string(),
        feel_tests: Some(log.to_string()),
    }
}

fn sentences(problems: &opendrone_pack::Problems) -> Vec<String> {
    problems.0.iter().map(ToString::to_string).collect()
}

#[test]
fn a_curve_whose_points_move_sideways_needs_a_new_source() {
    // Reviewer's round-2 case: the same voltages at other charge levels change
    // the curve as much as moving the voltages, but no range covers them.
    let old = "4.35 V at 100%, 3.92 V at 50%, 3.30 V at 0%";
    let new = "4.35 V at 100%, 3.92 V at 5%, 3.30 V at 0%";
    let quad = changed(old, new);
    let log = log_with(&format!(
        "| 2026-11-09 | [battery] voltage_curve | {old} → {new} | sideways |"
    ));
    assert_eq!(
        problems(&quad, &log),
        [at(
            line_of(&quad, "3.92 V at 5%"),
            "[battery] voltage_curve: the places its points hold at moved, which changes what's known about it, so it needs a new source"
        )]
    );
}

#[test]
fn a_value_whose_condition_changes_needs_a_new_source() {
    // Reviewer's round-2 case: "0.3 A at 4 V" to "0.3 A at 0.05 V" keeps the
    // current, but the Simulation receives the voltage too.
    let quad = changed("\"0.3 A at 4 V\"", "\"0.3 A at 0.05 V\"");
    let log = log_with(
        "| 2026-11-09 | [motors] no_load_current | 0.3 A at 4 V → 0.3 A at 0.05 V | measured lower |",
    );
    assert_eq!(
        problems(&quad, &log),
        [at(
            line_of(&quad, "0.05 V"),
            "[motors] no_load_current: the condition it holds at changed, which changes what's known about it, so it needs a new source"
        )]
    );
}

#[test]
fn an_estimate_re_sourced_with_a_new_source_row_starts_its_range_afresh() {
    // Rotor inertia, ×0.5–×2, moved ×4 with a new source and a "New source"
    // row: it passes, and CI lists both the move and where its range is
    // measured from now, for the Reviewer.
    let quad = changed(
        "\"0.25 g·cm²\", confidence = \"Estimate\", range = \"×0.5–×2\", source = \"guess\"",
        "\"1 g·cm²\", confidence = \"Estimate\", range = \"×0.5–×2\", source = \"worked-out\"",
    );
    let log = log_with(
        "| 2026-11-09 | [props] rotor_inertia | 0.25 g·cm² → 1 g·cm² | New source: weighed the props and bells |",
    );
    let found = report(&quad, &log);
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    assert_eq!(
        found.passed_on_a_new_source,
        [
            format!(
                "{LOG} line {}: [props] rotor_inertia was re-sourced at 1 g·cm² (Estimate, range ×0.5–×2, from the source worked-out: \"worked out from the maker's numbers\"), so its range is measured from there from now on",
                line_of(&log, "weighed the props")
            ),
            format!(
                "{QUAD} line {}: [props] rotor_inertia changed (0.25 g·cm² → 1 g·cm²) with a new source",
                line_of(&quad, "1 g·cm²")
            ),
        ]
    );
    // The next change is measured from 1 g·cm², the value the new source
    // gave: ×1.5 from there passes, though it is ×6 from where it started.
    let later_quad = quad.replacen("\"1 g·cm²\"", "\"1.5 g·cm²\"", 1);
    let later_log = log.clone()
        + "| 2026-11-16 | [props] rotor_inertia | 1 g·cm² → 1.5 g·cm² | spins up a touch slowly |\n";
    let later = check_feel_test_rules(
        QUAD,
        LOG,
        QuadVersion {
            quad: Some(&quad),
            feel_tests: Some(&log),
        },
        QuadVersion {
            quad: Some(&later_quad),
            feel_tests: Some(&later_log),
        },
    );
    assert_eq!(sentences(&later), Vec::<String>::new());
}

#[test]
fn a_new_source_row_needs_a_real_new_source() {
    // Without a new source, a "New source" row can't move the start.
    let quad = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 140, pitch 180, yaw 280 g·cm²",
    );
    let log = log_with(
        "| 2026-11-09 | [frame] inertia | roll 70, pitch 90, yaw 140 g·cm² → roll 140, pitch 180, yaw 280 g·cm² | New source: trust me |",
    );
    assert_eq!(
        problems(&quad, &log),
        [log_at(
            line_of(&log, "trust me"),
            "this row says [frame] inertia has a new source, but this change doesn't change its source; a \"New source\" row records a re-sourcing, which moves where its range is measured from"
        )]
    );
}

#[test]
fn an_estimate_moved_with_a_new_source_says_so_in_its_row() {
    let quad = changed(
        "\"0.25 g·cm²\", confidence = \"Estimate\", range = \"×0.5–×2\", source = \"guess\"",
        "\"1 g·cm²\", confidence = \"Estimate\", range = \"×0.5–×2\", source = \"worked-out\"",
    );
    let log =
        log_with("| 2026-11-09 | [props] rotor_inertia | 0.25 g·cm² → 1 g·cm² | weighed them |");
    assert_eq!(
        problems(&quad, &log),
        [log_at(
            line_of(&log, "weighed them"),
            "[props] rotor_inertia moved with a new source, so its row's why starts with \"New source\", which records where its range is measured from now"
        )]
    );
}

#[test]
fn every_number_that_passes_only_on_a_new_source_is_listed() {
    let quad = changed("\"23.0 g\"", "\"24.0 g\"").replace(
        "maker      = \"the maker's page\"",
        "maker      = \"the maker's page, read again\"",
    );
    let found = report(&quad, &before().1);
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    let listed: Vec<&str> = found
        .passed_on_a_new_source
        .iter()
        .map(|line| line.split(": ").nth(1).unwrap())
        .collect();
    assert!(
        listed.contains(&"[frame] dry_mass changed (23.0 g → 24.0 g) with a new source"),
        "{listed:?}"
    );
}

#[test]
fn quads_are_paired_by_id_so_a_pure_rename_passes() {
    let (quad, log) = before();
    let found = compare_packs(
        &holding(&[files("fixture/ducted", &quad, &log)]),
        &holding(&[files("fixture/ducted-pro", &quad, &log)]),
    );
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    assert_eq!(
        found.taken_out,
        ["fixture/ducted, renamed or moved to fixture/ducted-pro with every setting as it was"]
    );
}

/// The removed Quad's problem when a change takes it out without saying so.
const TAKEN_OUT: &str = "packs/fixture/quads/ducted/quad.toml: this change takes out the Quad fixture/ducted without saying so. Put it back, or retire it: add \"quads/ducted\" = \"<why>\" under [retired] in its Pack's pack.toml. To take a whole Pack out, retire its Quads in one change and take the Pack out in the next";

#[test]
fn a_quad_taken_out_without_saying_so_is_refused() {
    // Reviewer's case on #99: a Quad that left the comparison (its folder
    // hidden, linked or deleted) was never named, so a later change could
    // bring it back as new with any numbers.
    let (quad, log) = before();
    let found = compare_packs(
        &holding(&[files("fixture/ducted", &quad, &log)]),
        &holding(&[]),
    );
    assert_eq!(sentences(&found.problems), [TAKEN_OUT]);
    assert_eq!(found.taken_out, Vec::<String>::new());
}

#[test]
fn a_quad_its_pack_retires_is_named_with_why_for_the_reviewer() {
    let (quad, log) = before();
    let found = compare_packs(
        &holding(&[files("fixture/ducted", &quad, &log)]),
        &PacksVersion {
            quads: Vec::new(),
            retired: vec![retiring_the_fixture_quad()],
        },
    );
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    assert_eq!(
        found.taken_out,
        [
            "fixture/ducted, retired by packs/fixture/pack.toml line 14: \"Replaced by a 75 mm whoop with the same ducts.\""
        ]
    );
}

#[test]
fn a_retired_quad_brought_back_is_listed_as_new_and_previously_retired() {
    // Reviewer's case on #103: a Quad brought back after it was retired has
    // nothing to compare with, so the Reviewer compares it with its numbers
    // from before it was retired, and needs to know to.
    let moved = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 1400, pitch 1800, yaw 2800 g·cm²",
    );
    let (_, log) = before();
    let found = compare_packs(
        &PacksVersion {
            quads: Vec::new(),
            retired: vec![retiring_the_fixture_quad()],
        },
        &holding(&[files("fixture/ducted", &moved, &log)]),
    );
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    assert_eq!(found.new_quads, Vec::<String>::new());
    assert_eq!(
        found.previously_retired,
        [
            "fixture/ducted, which packs/fixture/pack.toml line 14 retired before this change: \"Replaced by a 75 mm whoop with the same ducts.\""
        ]
    );
}

#[test]
fn a_rename_that_also_moves_a_number_is_refused() {
    // Reviewer's round-2 case: renaming the Quad's folder (or its Pack's id)
    // made it a new Quad, so a 20× inertia move compared with nothing.
    let (quad, log) = before();
    let moved = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 1400, pitch 1800, yaw 2800 g·cm²",
    );
    for new_id in ["fixture/ducted-pro", "renamed-pack/ducted"] {
        let found = compare_packs(
            &holding(&[files("fixture/ducted", &quad, &log)]),
            &holding(&[files(new_id, &moved, &log)]),
        );
        let folder = format!(
            "packs/{}/quads/{}",
            new_id.split('/').next().unwrap(),
            new_id.split('/').nth(1).unwrap()
        );
        assert_eq!(
            sentences(&found.problems),
            [
                format!(
                    "{folder}/quad.toml: this change adds the Quad {new_id} and removes fixture/ducted, so it reads as a rename or a move, which must keep every setting as it was; none of the removed Quads matches it. Rename or move a Quad in a change of its own, and change its numbers in another. To retire a Quad and add a different one, take it out in one change and add the new one in another"
                ),
                TAKEN_OUT.to_string(),
            ]
        );
    }
}

#[test]
fn a_new_quad_beside_the_old_ones_has_nothing_to_compare() {
    let (quad, log) = before();
    let other = changed(
        "roll 70, pitch 90, yaw 140 g·cm²",
        "roll 1400, pitch 1800, yaw 2800 g·cm²",
    );
    let found = compare_packs(
        &holding(&[files("fixture/ducted", &quad, &log)]),
        &holding(&[
            files("fixture/ducted", &quad, &log),
            files("fixture/heavy", &other, &log),
        ]),
    );
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
}

#[test]
fn taking_a_number_out_needs_a_new_source() {
    // Taking the ducts off a whoop changes its physics as much as moving
    // their numbers.
    let quad = changed(
        "[ducts]\nram_drag       = { value = \"1.2 s⁻¹\", confidence = \"Estimate\", range = \"0.6–2.4 s⁻¹\", source = \"guess\" }\nnose_up_offset = { value = \"13 mm\", confidence = \"Estimate\", range = \"9–18 mm\", source = \"guess\" }\n",
        "",
    )
    .replace(
        "duct_rings  = { value = \"37 mm inside, 1.5 mm wall, 14 mm tall\", confidence = \"Estimate\", range = \"×0.9–×1.1\", source = \"guess\" }\n",
        "",
    );
    let found = problems(&quad, &before().1);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found[0],
        format!(
            "{QUAD}: [collision] duct_rings was taken out, which changes the Quad as much as moving it, so it needs a new source: change or remove its source's line in [sources]"
        )
    );
}

#[test]
fn a_count_or_choice_the_simulation_receives_is_listed_for_the_reviewer_but_not_camera_defaults_or_sound()
 {
    // Only what the Simulation receives is listed: the blade count here, but
    // not the camera's FOV, the sound block's harmonics count or its hit
    // level.
    let quad = changed("blades             = 3", "blades             = 4")
        .replace("fov         = \"160°\"", "fov         = \"150°\"")
        .replace("harmonics             = 8", "harmonics             = 6")
        .replace(
            "hit_level             = \"0.5\"",
            "hit_level             = \"0.7\"",
        );
    let found = report(&quad, &before().1);
    assert_eq!(sentences(&found.problems), Vec::<String>::new());
    assert_eq!(
        found.changed_without_a_confidence,
        [format!(
            "{QUAD} line {}: [props] blades changed (3 → 4); it carries no Confidence",
            line_of(&quad, "blades")
        )]
    );
}
