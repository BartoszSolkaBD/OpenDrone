//! Readable checks for the Feel Test log rules CI runs on every pull request
//! (#16 §3): each compares the good fixture Quad before a change with the same
//! Quad after it. Basis: Rule (#16 §3, the Flying deep dive's rules).

mod common;

use common::{LOG, QUAD, good_fixture, line_of};
use opendrone_pack::feel_tests::{QuadVersion, check_feel_test_rules};

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
    assert_eq!(problems(&quad, &log).len(), 1);
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
            feel_tests: None,
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
