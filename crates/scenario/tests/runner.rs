//! Readable checks for the Scenario runner: the committed Scenarios pass, and
//! broken Scenarios are refused with their file, line and a plain sentence.
//! Each broken case is the free-fall Scenario with one change, run against
//! the real built-in Pack.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::Packs;
use opendrone_scenario::agreement::{Computer, compare};
use opendrone_scenario::{
    Repo, Report, ResultsFile, ScenarioFile, fingerprints_text, read_scenario, run, run_one,
};

fn repo() -> Repo {
    Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the runner lives in the repo")
}

fn packs() -> Packs {
    repo()
        .packs()
        .unwrap_or_else(|p| panic!("the Packs should read:\n{p}"))
}

fn free_fall() -> String {
    fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-fall-is-exactly-g.toml"),
    )
    .unwrap()
}

/// The free-fall Scenario with `old` replaced by `new`, written to a fixture
/// folder.
fn changed(case: &str, old: &str, new: &str) -> ScenarioFile {
    let text = free_fall();
    assert!(text.contains(old), "the free-fall Scenario has no {old:?}");
    fixture(case, &text.replacen(old, new, 1))
}

fn fixture(case: &str, text: &str) -> ScenarioFile {
    let folder: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scenario-fixtures");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{case}.toml"));
    fs::write(&path, text).unwrap();
    let _ = fs::remove_file(folder.join(format!("{case}.results.toml")));
    ScenarioFile {
        relative: case.to_string(),
        path,
    }
}

fn report(file: &ScenarioFile, results: ResultsFile) -> Report {
    run_one(file, &packs(), results, None)
}

fn failures(report: &Report) -> Vec<String> {
    report
        .checks
        .iter()
        .filter(|(passed, _)| !passed)
        .map(|(_, line)| line.clone())
        .collect()
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines().position(|line| line.contains(needle)).unwrap() + 1
}

#[test]
fn the_committed_scenarios_pass_and_their_results_are_up_to_date() {
    let repo = repo();
    let files = repo.scenario_files().unwrap();
    assert!(files.len() >= 2, "expected the two physics Scenarios");
    for file in &files {
        let report = run_one(file, &packs(), ResultsFile::Check, None);
        assert!(
            report.passed(),
            "{}: {:#?}",
            file.label(),
            failures(&report)
        );
    }
    assert_eq!(repo.orphaned_results().unwrap(), Vec::<String>::new());
}

#[test]
fn a_failed_expectation_fails_and_says_what_was_measured() {
    let file = changed(
        "failed-expectation",
        "value = \"-9.81 m/s ± 0.00001 m/s\"",
        "value = \"-9 m/s ± 0.1 m/s\"",
    );
    let line = line_of(&free_fall(), "what  = \"vertical speed\"") - 1;
    assert_eq!(
        failures(&report(&file, ResultsFile::Write)),
        [format!(
            "vertical speed at 1 s: measured -9.81 m/s, expected -9 m/s ± 0.1 m/s (line {line}, Rule)"
        )]
    );
}

#[test]
fn a_stretch_can_be_measured_by_its_mean_lowest_highest_or_final_value() {
    // Falling from rest, the vertical speed after step n of 8000 is
    // -9.81 m/s² × n × 0.125 ms: its mean over the second is -9.81 m/s² ×
    // 0.125 ms × 8001 / 2 = -4.9056 m/s, and the last is -9.81 m/s.
    let stretch = |statistic: &str, value: &str| {
        format!(
            "\n[[expect]]\nwhat = \"vertical speed\"\nover = \"0 s to 1 s\"\n{statistic} = \"{value}\"\nbasis = \"rule: speed = g × t\"\n"
        )
    };
    let text = free_fall()
        + &stretch("mean", "-4.9056 m/s ± 0.0001 m/s")
        + &stretch("lowest", "-9.81 m/s ± 0.00001 m/s")
        + &stretch("highest", "-0.00122625 m/s ± 0.0000001 m/s")
        + &stretch("final", "-9.81 m/s ± 0.00001 m/s");
    let report = report(&fixture("statistics", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
    let lines: Vec<&str> = report
        .checks
        .iter()
        .map(|(_, line)| line.as_str())
        .collect();
    for expected in [
        "vertical speed, mean over 0 s to 1 s: measured -4.91 m/s",
        "vertical speed, lowest over 0 s to 1 s: measured -9.81 m/s",
        "vertical speed, highest over 0 s to 1 s: measured -0.00123 m/s",
        "vertical speed, final over 0 s to 1 s: measured -9.81 m/s",
    ] {
        assert!(
            lines.iter().any(|line| line.starts_with(expected)),
            "no {expected:?} in {lines:#?}"
        );
    }
}

#[test]
fn a_starting_state_that_leaves_an_item_out_is_refused_naming_it() {
    // ADR-0002: no hidden defaults.
    let file = changed("no-seed", "random_seed       = 1\n", "");
    assert_eq!(
        failures(&report(&file, ResultsFile::Write)),
        ["scenarios/no-seed.toml line 7: [start] is missing `random_seed`"]
    );
}

#[test]
fn every_problem_in_a_scenario_is_listed_at_once() {
    let text = free_fall()
        .replacen(
            "battery           = \"100%\"",
            "battery           = \"110%\"",
            1,
        )
        .replacen(
            "flight_mode       = \"Acro\"",
            "flight_mode       = \"Sport\"",
            1,
        );
    let file = fixture("two-problems", &text);
    let battery = line_of(&text, "battery ");
    let mode = line_of(&text, "flight_mode ");
    assert_eq!(
        failures(&report(&file, ResultsFile::Write)),
        [
            format!(
                "scenarios/two-problems.toml line {battery}: the battery's charge must be from 0% to 100%"
            ),
            format!(
                "scenarios/two-problems.toml line {mode}: `flight_mode` must be one of \"Acro\", \"Angle\", \"Horizon\", not \"Sport\""
            ),
        ]
    );
}

#[test]
fn an_unknown_measurement_is_refused_listing_what_the_runner_measures() {
    let file = changed(
        "unknown-measure",
        "what  = \"vertical speed\"",
        "what  = \"sink rate\"",
    );
    let line = line_of(&free_fall(), "what  = \"vertical speed\"");
    assert_eq!(
        failures(&report(&file, ResultsFile::Write)),
        [format!(
            "scenarios/unknown-measure.toml line {line}: the runner can't measure \"sink rate\" yet; it measures height, distance east, distance north, vertical speed, horizontal speed, speed, vertical acceleration, roll rate, pitch rate, yaw rate, roll, pitch, heading, motor N speed (N from 1 to 4, in Betaflight's motor order), motor N thrust, motor N torque, motor N current, motor N drive, total thrust, battery voltage, battery current, battery charge used, battery sag"
        )]
    );
}

#[test]
fn an_expectation_needs_a_tolerance_and_a_basis() {
    let text = free_fall()
        .replacen(
            "value = \"-9.81 m/s ± 0.00001 m/s\"",
            "value = \"-9.81 m/s\"",
            1,
        )
        .replacen(
            "basis = \"rule: speed = g × t",
            "basis = \"because: speed = g × t",
            1,
        );
    let file = fixture("no-tolerance-no-basis", &text);
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].ends_with(
        "a basis starts with \"source:\" (a cited outside reference), \"rule:\" (worked out from physics, with the working shown) or \"observed:\" (what the Simulation did when the Expectation was written), followed by the citation or the working"
    ));
    assert!(found[1].ends_with(
        "\"-9.81 m/s\" needs a tolerance: add \"± amount\" or \"± percent\", or write \"between X and Y\""
    ));
}

#[test]
fn an_expectation_measuring_the_wrong_kind_of_thing_is_refused() {
    let file = changed(
        "wrong-kind",
        "value = \"-9.81 m/s ± 0.00001 m/s\"",
        "value = \"-9.81 m ± 0.00001 m\"",
    );
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].ends_with("vertical speed is a speed, such as \"5 m/s\""),
        "{found:?}"
    );
}

#[test]
fn a_moment_between_two_physics_steps_is_refused() {
    let file = changed("between-steps", "at    = \"1 s\"", "at    = \"1.00001 s\"");
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].ends_with(
            "\"1.00001 s\" isn't a whole number of physics steps from the start: at 8000 Hz one step is 0.000125 s"
        ),
        "{found:?}"
    );
}

#[test]
fn radians_and_decimal_commas_are_refused_in_a_scenario_too() {
    let text = free_fall()
        .replacen(
            "rotation          = \"0 °/s\"",
            "rotation          = \"0 rad/s\"",
            1,
        )
        .replacen("at    = \"1 s\"", "at    = \"0,5 s\"", 1);
    let found = failures(&report(
        &fixture("radians-commas", &text),
        ResultsFile::Write,
    ));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].contains("is in radians, which OpenDrone files never use"));
    assert!(found[1].contains("numbers take a decimal point, so write \"0.5 s\""));
}

#[test]
fn a_motor_command_above_100_percent_is_refused() {
    let file = changed(
        "motors-over",
        "motors = \"0%\"",
        "motors = \"40%, 40%, 101%, 40%\"",
    );
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].ends_with("a motor command must be from 0% to 100%"),
        "{found:?}"
    );
}

#[test]
fn a_decimal_comma_in_the_motor_commands_is_refused_with_the_fix() {
    let file = changed("motors-comma", "motors = \"0%\"", "motors = \"0,0%\"");
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].ends_with(
            "\"0,0%\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"0.0%\""
        ),
        "{found:?}"
    );
}

#[test]
fn stopped_motors_are_only_for_scenarios_that_script_their_motors() {
    // Where the Flight Controller runs, a landed "fresh" start is Reset, whose
    // ESCs start up first; "stopped" says they are already ready.
    let file = changed(
        "flight-stopped",
        "kind              = \"physics\"",
        "kind              = \"flight\"",
    );
    let found = failures(&report(&file, ResultsFile::Write));
    assert!(
        found.iter().any(|line| line.ends_with(
            "motors \"stopped\" (at rest, with the ESCs already ready) and \"powering up\" (with the ESCs just powered) are only for Physics and Thrust Stand Scenarios, which script their motors; a Flight Scenario that starts landed with a \"fresh\" Flight Controller is Reset, and the arming and power-up ticket (#52) names how its motors start"
        )),
        "{found:#?}"
    );
}

#[test]
fn flight_and_flight_controller_scenarios_wait_for_the_flight_controller() {
    for kind in ["flight", "flight controller"] {
        let file = changed(
            "flight-kind",
            "kind              = \"physics\"",
            &format!("kind              = \"{kind}\""),
        );
        let found = failures(&report(&file, ResultsFile::Write));
        assert!(
            found[0].ends_with(
                "only Physics and Thrust Stand Scenarios can run so far: Flight and Flight Controller Scenarios arrive with the Flight Controller (#48)"
            ),
            "{found:#?}"
        );
    }
}

#[test]
fn a_quad_on_the_thrust_stand_is_held_still_so_it_starts_still_and_never_settled() {
    let text = free_fall()
        .replacen(
            "kind              = \"physics\"",
            "kind              = \"thrust stand\"",
            1,
        )
        .replacen(
            "speed             = \"0 m/s\"",
            "speed             = \"1 m/s east, 0 m/s north, 0 m/s up\"",
            1,
        )
        .replacen(
            "rotation          = \"0 °/s\"",
            "rotation          = \"roll 10 °/s, pitch 0 °/s, yaw 0 °/s\"",
            1,
        )
        .replacen(
            "motors            = \"stopped\"",
            "motors            = \"settled\"",
            1,
        );
    let found = failures(&report(&fixture("held-still", &text), ResultsFile::Write));
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(found[0].ends_with(
        "a Quad on the thrust stand is held still, so there is no motion for \"settled\" motors to hold; start them \"stopped\" (ESCs ready) or \"powering up\" (ESCs just powered)"
    ));
    assert!(
        found[1].ends_with("a Quad on the thrust stand is held still: its `speed` must be zero")
    );
    assert!(
        found[2].ends_with("a Quad on the thrust stand is held still: its `rotation` must be zero")
    );
}

#[test]
fn on_the_thrust_stand_the_quad_stays_where_it_starts() {
    // Basis: Rule. The free-fall Scenario on the thrust stand: nothing moves.
    let text = free_fall()
        .replacen(
            "kind              = \"physics\"",
            "kind              = \"thrust stand\"",
            1,
        )
        .replacen(
            "value = \"-9.81 m/s ± 0.00001 m/s\"",
            "value = \"0 m/s ± 0 m/s\"",
            1,
        );
    let report = report(&fixture("on-the-stand", &text), ResultsFile::Write);
    let found = failures(&report);
    assert!(
        found.iter().any(|line| line
            .starts_with("vertical acceleration, lowest over 0 s to 1 s: measured 0 m/s²")),
        "{found:#?}"
    );
    assert!(
        !found
            .iter()
            .any(|line| line.starts_with("vertical speed at 1 s")),
        "{found:#?}"
    );
}

#[test]
fn an_expectation_can_compare_with_the_same_run_at_another_physics_rate() {
    // Basis: Rule. Falling for 1 s in fixed steps of length h lands
    // ½·g·t·h lower than ½·g·t²: 0.613 mm at 8 kHz and 1.226 mm at 4 kHz,
    // so this run is 0.613 mm higher than the 4 kHz one.
    let text = free_fall()
        + "\n[[expect]]\nwhat = \"height\"\nat = \"1 s\"\nagainst = { physics_rate = \"4 kHz\" }\ncompare = \"difference\"\nvalue = \"0.613125 mm ± 0.000001 mm\"\nbasis = \"rule: ½ × 9.81 m/s² × 1 s × (1/4000 − 1/8000) s\"\n"
        + "\n[[expect]]\nwhat = \"vertical speed\"\nover = \"0 s to 1 s\"\nmean = \"100% ± 0.000001%\"\nagainst = { battery = \"50%\" }\ncompare = \"ratio\"\nbasis = \"rule: the battery changes nothing with the motors stopped\"\n";
    let report = report(&fixture("compared", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
    let lines: Vec<&String> = report.checks.iter().map(|(_, line)| line).collect();
    assert!(lines.iter().any(|line| line.starts_with(
        "height at 1 s, minus the same run at 4 kHz: measured 0.613 mm, expected 0.613125 mm ± 0.000001 mm"
    )));
    assert!(lines.iter().any(|line| line.starts_with(
        "vertical speed, mean over 0 s to 1 s, as a share of the same run with the battery at 50%: measured 100%"
    )));
}

#[test]
fn a_comparison_needs_both_its_other_run_and_how_to_compare() {
    let expect = |extra: &str| {
        format!(
            "\n[[expect]]\nwhat = \"height\"\nat = \"1 s\"\n{extra}\nvalue = \"0 m ± 1 m\"\nbasis = \"rule: x\"\n"
        )
    };
    let text = free_fall()
        + &expect("compare = \"difference\"")
        + &expect("against = { physics_rate = \"4 kHz\" }")
        + &expect("against = { wind = \"5 m/s\" }\ncompare = \"difference\"")
        + &expect("against = { physics_rate = \"4 kHz\" }\ncompare = \"sum\"");
    let found = failures(&report(&fixture("comparisons", &text), ResultsFile::Write));
    let ends = [
        "`compare` needs `against`: the other run to compare with, such as `against = { physics_rate = \"4 kHz\" }`",
        "an Expectation `against` another run needs `compare`: `compare` says how this run's value meets the other's: \"difference\" (this run's minus the other's) or \"ratio\" (this run's as a share of the other's)",
        "`wind` isn't something OpenDrone reads in [expect[8].against]; it reads `physics_rate`, `battery`",
        "`against` names what the other run changes: `physics_rate`, `battery`, or both, written as in [start]",
        "`compare` says how this run's value meets the other's: \"difference\" (this run's minus the other's) or \"ratio\" (this run's as a share of the other's), not \"sum\"",
    ];
    assert_eq!(found.len(), ends.len(), "{found:#?}");
    for (found, end) in found.iter().zip(ends) {
        assert!(found.ends_with(end), "{found}");
    }
}

#[test]
fn a_compared_moment_must_be_a_whole_step_in_the_other_run_and_angles_cant_be_compared_yet() {
    let text = free_fall()
        + "\n[[expect]]\nwhat = \"height\"\nat = \"0.000125 s\"\nagainst = { physics_rate = \"4 kHz\" }\ncompare = \"difference\"\nvalue = \"0 m ± 1 m\"\nbasis = \"rule: x\"\n"
        + "\n[[expect]]\nwhat = \"pitch\"\nat = \"1 s\"\nagainst = { physics_rate = \"4 kHz\" }\ncompare = \"difference\"\nvalue = \"0° ± 1°\"\nbasis = \"rule: x\"\n";
    let found = failures(&report(
        &fixture("compared-badly", &text),
        ResultsFile::Write,
    ));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].ends_with(
        "this moment isn't a whole number of physics steps at 4000 Hz, the rate of the run it's compared with: there one step is 0.00025 s"
    ), "{found:#?}");
    assert!(found[1].ends_with(
        "pitch can't be compared with another run yet, because angles wrap round; compare a rate or a position instead"
    ), "{found:#?}");
}

#[test]
fn a_quad_that_cant_be_found_is_reported_at_the_line_that_names_it() {
    let file = changed("no-such-quad", "test/whoop-65-no-drag", "test/whoop-99");
    let line = line_of(&free_fall(), "quad   ");
    let found = failures(&report(&file, ResultsFile::Write));
    assert_eq!(
        found[0],
        format!("scenarios/no-such-quad.toml line {line}: can't use the Quad \"test/whoop-99\":")
    );
    assert_eq!(
        found[1],
        "test/whoop-99: there's no Test Quad here: scenarios/test-quads/whoop-99.toml doesn't exist"
    );
}

#[test]
fn an_angle_is_compared_the_short_way_round() {
    let tumble = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-tumble-keeps-its-spin.toml"),
    )
    .unwrap();
    // 45° and 405° are the same heading.
    let file = fixture(
        "angle-wraps",
        &tumble.replacen("value = \"45° ± 0.001°\"", "value = \"405° ± 0.001°\"", 1),
    );
    let report = report(&file, ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

/// The slow turn through north, with its Expectations replaced by
/// `expectations` and its start changed by `start`.
fn slow_turn(start: &[(&str, &str)], expectations: &[(&str, &str, &str)]) -> String {
    let text = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/slow-turn-through-north.toml"),
    )
    .unwrap();
    let mut text = text[..text.find("[[expect]]").unwrap()].to_string();
    for (old, new) in start {
        assert!(text.contains(old), "no {old:?}");
        text = text.replacen(old, new, 1);
    }
    for (what, statistic, value) in expectations {
        text += &format!(
            "[[expect]]\nwhat = \"{what}\"\nover = \"0 s to 1 s\"\n{statistic} = \"{value}\"\nbasis = \"rule: 2 °/s for 1 s\"\n\n"
        );
    }
    text
}

#[test]
fn a_heading_crossing_north_has_the_right_lowest_highest_mean_and_final_value() {
    // From 359.5° at 2 °/s for 1 s: from just after -0.5° up to 1.5°.
    let text = slow_turn(
        &[],
        &[
            ("heading", "lowest", "-0.5° ± 0.001°"),
            ("heading", "highest", "1.5° ± 0.001°"),
            ("heading", "mean", "0.5° ± 0.001°"),
            ("heading", "final", "1.5° ± 0.001°"),
        ],
    );
    let report = report(&fixture("heading-across-north", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn a_heading_that_crossed_north_doesnt_pass_for_staying_at_north() {
    // The heading reached 1.5°, so "highest 0° ± 1°" must fail.
    let text = slow_turn(&[], &[("heading", "highest", "0° ± 1°")]);
    let found = failures(&report(
        &fixture("highest-past-north", &text),
        ResultsFile::Write,
    ));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("heading, highest over 0 s to 1 s: measured 1.50°"),
        "{found:?}"
    );
}

#[test]
fn a_roll_crossing_upside_down_has_the_right_lowest_highest_mean_and_final_value() {
    // From 179.5° at 2 °/s for 1 s: through 180° (upside down) to 181.5°,
    // which reads as -178.5°.
    let text = slow_turn(
        &[
            (
                "attitude          = \"level, heading 359.5°\"",
                "attitude          = \"roll 179.5°, pitch 0°, heading 0°\"",
            ),
            (
                "rotation          = \"roll 0 °/s, pitch 0 °/s, yaw 2 °/s\"",
                "rotation          = \"roll 2 °/s, pitch 0 °/s, yaw 0 °/s\"",
            ),
        ],
        &[
            ("roll", "lowest", "179.5° ± 0.001°"),
            ("roll", "highest", "-178.5° ± 0.001°"),
            ("roll", "mean", "180.5° ± 0.001°"),
            ("roll", "final", "181.5° ± 0.001°"),
        ],
    );
    let report = report(
        &fixture("roll-across-upside-down", &text),
        ResultsFile::Write,
    );
    assert!(report.passed(), "{:#?}", failures(&report));
}

/// What a stretch's lowest, highest or mean measures when the angle jumped.
fn none(what: &str, statistic: &str) -> String {
    "none: the {} jumped, or reached half a turn from the expected value, during the stretch, as it does in flips and when the nose passes straight up or down, so its {} has no single answer; check it at moments, over a shorter stretch, or check its rate"
        .replacen("{}", what, 1)
        .replacen("{}", statistic, 1)
}

/// The free tumble (2000 °/s of roll for 1.125 s, six and a quarter rolls),
/// with its Expectations replaced by `expectations` over the whole run.
fn tumble_over_the_run(expectations: &[(&str, &str)]) -> String {
    let text = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-tumble-keeps-its-spin.toml"),
    )
    .unwrap();
    let mut text = text[..text.find("[[expect]]").unwrap()].to_string();
    for (statistic, value) in expectations {
        text += &format!(
            "[[expect]]\nwhat = \"roll\"\nover = \"0 s to 1.125 s\"\n{statistic} = \"{value}\"\nbasis = \"rule: a check of the runner\"\n\n"
        );
    }
    text
}

#[test]
fn a_roll_mean_lowest_or_highest_over_whole_rolls_fails_whatever_is_expected() {
    // The roll sweeps round six times, so it goes more than half a turn from
    // any expected value: its mean, lowest and highest have no single answer.
    for expected in ["0° ± 8°", "90° ± 8°", "123° ± 8°", "-150° ± 8°"] {
        let text = tumble_over_the_run(&[
            ("mean", expected),
            ("lowest", expected),
            ("highest", expected),
        ]);
        let found = failures(&report(&fixture("roll-sweeps", &text), ResultsFile::Write));
        assert_eq!(found.len(), 3, "{expected}: {found:#?}");
        for (line, statistic) in found.iter().zip(["mean", "lowest", "highest"]) {
            assert!(
                line.starts_with(&format!(
                    "roll, {statistic} over 0 s to 1.125 s: measured {}",
                    none("roll", statistic)
                )),
                "{expected}: {line}"
            );
        }
    }
}

/// One pitch flip, nose up first, in 1 s at 360 °/s, starting level with the
/// nose at `heading`, with `expectations` over the whole flip.
fn pitch_flip(heading: &str, expectations: &[(&str, &str, &str)]) -> String {
    turning(
        &format!("level, heading {heading}"),
        "roll 0 °/s, pitch 360 °/s, yaw 0 °/s",
        "1 s",
        expectations,
    )
}

/// The free tumble's set-up with another start `attitude` and `rotation`,
/// and `expectations` over 0 s to `until`.
fn turning(
    attitude: &str,
    rotation: &str,
    until: &str,
    expectations: &[(&str, &str, &str)],
) -> String {
    let text = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-tumble-keeps-its-spin.toml"),
    )
    .unwrap();
    let mut text = text[..text.find("[[expect]]").unwrap()]
        .replacen(
            "attitude          = \"roll 0°, pitch 30°, heading 45°\"",
            &format!("attitude          = \"{attitude}\""),
            1,
        )
        .replacen(
            "rotation          = \"roll 2000 °/s, pitch 0 °/s, yaw 0 °/s\"",
            &format!("rotation          = \"{rotation}\""),
            1,
        );
    assert!(text.contains(attitude) && text.contains(rotation));
    for (what, statistic, value) in expectations {
        let when = if *statistic == "at" {
            format!("at = \"{until}\"\nvalue")
        } else {
            format!("over = \"0 s to {until}\"\n{statistic}")
        };
        text += &format!(
            "[[expect]]\nwhat = \"{what}\"\n{when} = \"{value}\"\nbasis = \"rule: a check of the runner\"\n\n"
        );
    }
    text
}

#[test]
fn a_roll_or_heading_mean_lowest_or_highest_over_a_pitch_flip_fails_whatever_is_expected() {
    // As the nose passes straight up and then straight down, roll and heading
    // jump by half a turn, so over the flip they have no single mean, lowest
    // or highest.
    for heading in ["90°", "45°"] {
        for (what, expected) in [
            ("roll", "90° ± 1°"),
            ("roll", "-90° ± 1°"),
            ("roll", "0° ± 1°"),
            ("heading", "0° ± 1°"),
            ("heading", "180° ± 1°"),
            ("heading", "135° ± 2°"),
            ("heading", "315° ± 2°"),
        ] {
            let checks: Vec<(&str, &str, &str)> = ["mean", "lowest", "highest"]
                .iter()
                .map(|statistic| (what, *statistic, expected))
                .collect();
            let text = pitch_flip(heading, &checks);
            let found = failures(&report(&fixture("pitch-flip", &text), ResultsFile::Write));
            assert_eq!(
                found.len(),
                3,
                "from heading {heading}, {what} {expected}: {found:#?}"
            );
            for (line, statistic) in found.iter().zip(["mean", "lowest", "highest"]) {
                assert!(
                    line.starts_with(&format!(
                        "{what}, {statistic} over 0 s to 1 s: measured {}",
                        none(what, statistic)
                    )),
                    "from heading {heading}: {line}"
                );
            }
        }
    }
}

#[test]
fn a_roll_and_heading_still_have_a_final_value_after_a_pitch_flip() {
    // A whole flip ends where it started: level, nose at the start heading.
    for (heading, final_heading) in [("90°", "90° ± 0.001°"), ("45°", "45° ± 0.001°")] {
        let text = pitch_flip(
            heading,
            &[
                ("roll", "final", "0° ± 0.001°"),
                ("heading", "final", final_heading),
                ("pitch", "final", "0° ± 0.001°"),
            ],
        );
        let report = report(&fixture("pitch-flip-final", &text), ResultsFile::Write);
        assert!(
            report.passed(),
            "from heading {heading}: {:#?}",
            failures(&report)
        );
    }
}

#[test]
fn a_whole_turn_that_ends_exactly_opposite_the_expected_value_has_no_mean() {
    // A pitch flip passing just off vertical turns the heading a whole turn,
    // from 90° back to 90°, exactly opposite -90° at both ends.
    for roll in ["0.5°", "0.01°", "3°"] {
        let text = turning(
            &format!("roll {roll}, pitch 0°, heading 90°"),
            "roll 0 °/s, pitch 360 °/s, yaw 0 °/s",
            "1 s",
            &[("heading", "mean", "-90° ± 7°")],
        );
        let found = failures(&report(
            &fixture("whole-turn-mean", &text),
            ResultsFile::Write,
        ));
        assert_eq!(found.len(), 1, "roll {roll}: {found:#?}");
        assert!(
            found[0].starts_with(&format!(
                "heading, mean over 0 s to 1 s: measured {}",
                none("heading", "mean")
            )),
            "roll {roll}: {found:?}"
        );
    }
    // Five fast flips from heading 45°, ending opposite -135°.
    let text = turning(
        "roll 0.2°, pitch 0°, heading 45°",
        "roll 0 °/s, pitch 2000 °/s, yaw 0 °/s",
        "0.9 s",
        &[("heading", "mean", "-135° ± 7°")],
    );
    let found = failures(&report(
        &fixture("whole-turn-mean-fast", &text),
        ResultsFile::Write,
    ));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains(&none("heading", "mean")), "{found:?}");
}

#[test]
fn a_quad_dropped_nose_first_keeps_its_heading() {
    // Nose straight down, not turning: the heading it was given stays.
    let text = turning(
        "roll 0°, pitch -90°, heading 30°",
        "roll 0 °/s, pitch 0 °/s, yaw 0 °/s",
        "1 s",
        &[
            ("heading", "at", "30° ± 0.001°"),
            ("heading", "mean", "30° ± 0.001°"),
            ("heading", "final", "30° ± 0.001°"),
            ("roll", "final", "0° ± 0.001°"),
            ("pitch", "final", "-90° ± 0.001°"),
        ],
    );
    let report = report(&fixture("nose-first", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn with_the_nose_straight_up_roll_reads_0_and_heading_carries_the_turn() {
    // Roll 30° then nose straight up at heading 45° is the same attitude as
    // roll 0°, heading 15°: with the nose up, heading minus roll.
    let text = turning(
        "roll 30°, pitch 90°, heading 45°",
        "roll 0 °/s, pitch 0 °/s, yaw 0 °/s",
        "1 s",
        &[
            ("heading", "at", "15° ± 0.001°"),
            ("roll", "at", "0° ± 0.001°"),
            ("heading", "mean", "15° ± 0.001°"),
        ],
    );
    let report = report(&fixture("nose-up-rolled", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

/// What a stretch's lowest, highest or mean measures when the nose was
/// straight up or down for only part of it.
fn none_part_straight_up_or_down(what: &str, statistic: &str) -> String {
    "none: for part of the stretch, and not all of it, the nose was within 0.00000006° of straight up or down, where roll reads 0° and heading carries the whole turn, so the {}'s {} has no single answer; check it at moments, over a shorter stretch, or check its rate"
        .replacen("{}", what, 1)
        .replacen("{}", statistic, 1)
}

#[test]
fn a_quad_that_slowly_leaves_straight_up_has_no_single_lowest_highest_or_mean_roll_or_heading() {
    // Round 5 of #89's review: 0.00000001° off vertical, inside the band
    // where roll reads 0° and heading carries the turn (45° − 30° = 15°),
    // yawing out of it at 0.0001 °/s. After four steps the nose is out, and
    // roll reads about -81°, a step under the quarter-turn jump check. So
    // the highest roll measured 0°, and "highest roll 0° ± 0.001°" passed,
    // though the roll then read -81° to -90° for the rest of the stretch.
    let mut checks = vec![("roll", "highest", "0° ± 0.001°")];
    for (what, expected) in [("roll", "-85° ± 10°"), ("heading", "300° ± 10°")] {
        for statistic in ["lowest", "highest", "mean"] {
            checks.push((what, statistic, expected));
        }
    }
    let text = turning(
        "roll 30°, pitch 89.99999999°, heading 45°",
        "roll 0 °/s, pitch 0 °/s, yaw -0.0001 °/s",
        "0.01 s",
        &checks,
    );
    let found = failures(&report(
        &fixture("leaving-straight-up", &text),
        ResultsFile::Write,
    ));
    assert_eq!(found.len(), checks.len(), "{found:#?}");
    for (line, (what, statistic, _)) in found.iter().zip(&checks) {
        assert!(
            line.starts_with(&format!(
                "{what}, {statistic} over 0 s to 0.01 s: measured {}",
                none_part_straight_up_or_down(what, statistic)
            )),
            "{line}"
        );
    }
}

#[test]
fn a_quad_that_slowly_leaves_straight_up_still_has_a_final_roll_and_heading() {
    // The same flight. A final value and a value at a moment are single
    // readings, so they still read. By 0.01 s the nose has moved 0.000001°
    // sideways, a hundred times further than it started from vertical, and
    // heading minus roll is still the 15° it read inside the band.
    let text = turning(
        "roll 30°, pitch 89.99999999°, heading 45°",
        "roll 0 °/s, pitch 0 °/s, yaw -0.0001 °/s",
        "0.01 s",
        &[
            ("roll", "final", "-89.5° ± 0.1°"),
            ("heading", "final", "285.5° ± 0.1°"),
            ("roll", "at", "-89.5° ± 0.1°"),
            ("heading", "at", "285.5° ± 0.1°"),
            ("pitch", "mean", "90° ± 0.001°"),
        ],
    );
    let report = report(
        &fixture("leaving-straight-up-final", &text),
        ResultsFile::Write,
    );
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn a_quad_that_stays_within_the_band_around_straight_up_keeps_its_lowest_highest_and_mean() {
    // 0.00000001° off vertical and not turning: every reading is inside the
    // band, roll 0° and heading 45° − 30° = 15°.
    let text = turning(
        "roll 30°, pitch 89.99999999°, heading 45°",
        "roll 0 °/s, pitch 0 °/s, yaw 0 °/s",
        "1 s",
        &[
            ("roll", "lowest", "0° ± 0.001°"),
            ("roll", "highest", "0° ± 0.001°"),
            ("roll", "mean", "0° ± 0.001°"),
            ("heading", "lowest", "15° ± 0.001°"),
            ("heading", "highest", "15° ± 0.001°"),
            ("heading", "mean", "15° ± 0.001°"),
        ],
    );
    let report = report(&fixture("within-the-band", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn an_angle_tolerance_that_accepts_every_angle_is_refused_as_checking_nothing() {
    for expected in [
        "between 0° and 360°",
        "0° ± 180°",
        "0° ± 200°",
        "90° ± 200%",
    ] {
        let text = slow_turn(&[], &[("heading", "mean", expected)]);
        let found = failures(&report(&fixture("whole-circle", &text), ResultsFile::Write));
        assert_eq!(found.len(), 1, "{expected}: {found:#?}");
        assert!(
            found[0].contains("accepts every angle, since angles are compared the short way round, so this Expectation checks nothing; give a tolerance of less than half a turn each way"),
            "{expected}: {found:?}"
        );
    }
    // Just short of the whole circle still checks something.
    let text = slow_turn(&[], &[("heading", "mean", "0° ± 179°")]);
    let report = report(&fixture("nearly-whole-circle", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn a_roll_still_has_a_final_value_after_whole_rolls() {
    // Six and a quarter rolls end on the right side: 90°.
    let text = tumble_over_the_run(&[("final", "90° ± 0.001°")]);
    let report = report(&fixture("roll-sweeps-final", &text), ResultsFile::Write);
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn a_results_file_holds_every_measured_value_and_the_fingerprints() {
    let file = changed(
        "results",
        "name   = \"Free fall is exactly g\"",
        "name   = \"Results\"",
    );
    assert!(report(&file, ResultsFile::Write).passed());
    let results = fs::read_to_string(file.results_path()).unwrap();
    for line in [
        "scenario = \"Results\"",
        "what     = \"vertical speed at 1 s\"",
        "expected = \"-9.81 m/s ± 0.00001 m/s\"",
        "measured = \"-9.81 m/s\"",
        "what     = \"height at 1 s\"",
        "measured = \"-4.91 m\"",
        "measured = \"none broken\"",
        "[fingerprints]",
    ] {
        assert!(
            results.lines().any(|l| l == line),
            "no {line:?} in:\n{results}"
        );
    }
    for key in [
        "[fingerprints.checkpoints]",
        "quad = \"",
        "map  = \"",
        "run  = \"",
        "\"0.1 s\" = \"",
        "\"1 s\" = \"",
    ] {
        assert!(results.contains(key), "no {key:?} in:\n{results}");
    }
}

#[test]
fn an_out_of_date_results_file_is_reported_with_the_first_line_that_differs() {
    let file = changed(
        "out-of-date",
        "name   = \"Free fall is exactly g\"",
        "name   = \"Out of date\"",
    );
    assert!(report(&file, ResultsFile::Write).passed());
    let path = file.results_path();
    let results = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        results.replacen("measured = \"-4.91 m\"", "measured = \"-4.90 m\"", 1),
    )
    .unwrap();
    let found = failures(&report(&file, ResultsFile::Check));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with("scenarios/out-of-date.results.toml is out of date: run `cargo scenarios run` and commit it. Line "),
        "{found:?}"
    );
    assert!(found[0].ends_with(
        "says \"measured = \\\"-4.90 m\\\"\", but this run gives \"measured = \\\"-4.91 m\\\"\"."
    ));

    fs::remove_file(&path).unwrap();
    let found = failures(&report(&file, ResultsFile::Check));
    assert!(found[0].ends_with("There's no committed Results file yet."));
}

#[test]
fn a_scenario_runs_until_the_last_moment_it_mentions() {
    let scenario = read_scenario("free-fall.toml", &free_fall()).unwrap();
    assert_eq!(scenario.length.ticks(), 8000);
    let outcome = run(&scenario, &packs()).unwrap();
    assert_eq!(outcome.step_fingerprints.len(), 8001);
}

fn computers(names: &[&str], text: &str) -> Vec<Computer> {
    names
        .iter()
        .map(|name| Computer {
            name: name.to_string(),
            files: [("physics/free-fall".to_string(), text.to_string())].into(),
        })
        .collect()
}

fn free_fall_fingerprints() -> String {
    let scenario = read_scenario("free-fall.toml", &free_fall()).unwrap();
    fingerprints_text("physics/free-fall", &run(&scenario, &packs()).unwrap())
}

#[test]
fn computers_that_agree_at_every_step_pass_the_agreement_check() {
    let found = compare(&computers(
        &["macOS", "Windows", "Linux"],
        &free_fall_fingerprints(),
    ));
    assert!(found.agreed);
    assert_eq!(
        found.lines,
        ["physics/free-fall: all 3 computers agree at the start and after each of its 8000 steps"]
    );
}

#[test]
fn the_agreement_check_names_the_scenario_and_the_first_step_where_computers_split() {
    let text = free_fall_fingerprints();
    let mut all = computers(&["macOS", "Windows", "Linux"], &text);
    let windows = text
        .lines()
        .map(|line| match line.split_once(' ') {
            Some((tick, _)) if tick == "3" || tick == "4" => format!("{tick} 0123456789abcdef"),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    all[1]
        .files
        .insert("physics/free-fall".to_string(), windows);
    let found = compare(&all);
    assert!(!found.agreed);
    let step_3 = |line: &str| {
        line.split_once(' ')
            .filter(|(t, _)| *t == "3")
            .map(|(_, f)| f.to_string())
    };
    let mac = text.lines().find_map(step_3).unwrap();
    assert_eq!(
        found.lines,
        [format!(
            "physics/free-fall: the computers split at step 3 (0.000375 s): macOS {mac}, Windows 0123456789abcdef, Linux {mac}"
        )]
    );
}

#[test]
fn the_agreement_check_fails_when_a_computer_has_no_fingerprints() {
    let mut all = computers(&["macOS", "Windows", "Linux"], &free_fall_fingerprints());
    all[2].files.clear();
    let found = compare(&all);
    assert!(!found.agreed);
    assert_eq!(
        found.lines,
        [
            "Linux: no fingerprints at all; did its Scenario run finish?",
            "physics/free-fall: Linux has no fingerprints for it; did its run of this Scenario finish?",
        ]
    );
}
