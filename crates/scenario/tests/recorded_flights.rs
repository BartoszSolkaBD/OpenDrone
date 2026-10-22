//! Readable checks for Physics Scenarios that play back a real flight log
//! (#47): a motor Input Track of the logged motor commands, a recorded flight
//! that "matches a recorded flight" Expectations compare the run with, the
//! felt accelerations an accelerometer reads, and the Scenarios whose data
//! is downloaded, never committed, which skip without it. Each case is a
//! committed Scenario with a change, run against the real built-in Pack.
//! Basis: Rule (#11 §2 and §3, ADR-0002) unless said.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::Packs;
use opendrone_scenario::{MOTOR_TRACK_HEADER, Repo, Report, ResultsFile, ScenarioFile, run_one};

fn repo() -> Repo {
    Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the runner lives in the repo")
}

fn packs() -> Packs {
    repo()
        .packs()
        .unwrap_or_else(|p| panic!("the Packs should read:\n{p}"))
}

fn committed(scenario: &str) -> String {
    fs::read_to_string(repo().root.join("scenarios").join(scenario)).unwrap()
}

/// The free-fall Scenario: a Whoop 65 with no drag, its motors stopped,
/// falling from rest in empty air.
fn free_fall() -> String {
    committed("physics/free-fall-is-exactly-g.toml")
}

/// The Scenario's text up to its first Expectation.
fn without_expectations(text: &str) -> &str {
    &text[..text
        .find("[[expect]]")
        .expect("every Scenario has an Expectation")]
}

/// Writes a Scenario and any CSV files beside it into a folder of their own.
fn fixture(case: &str, scenario: &str, files: &[(&str, &str)]) -> ScenarioFile {
    let folder: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("recorded-flight-fixtures")
        .join(case)
        .join("physics");
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{case}.toml"));
    fs::write(&path, scenario).unwrap();
    for (name, text) in files {
        fs::write(folder.join(name), text).unwrap();
    }
    ScenarioFile {
        relative: format!("physics/{case}"),
        path,
    }
}

fn report(file: &ScenarioFile) -> Report {
    run_one(file, &packs(), ResultsFile::Write, None)
}

fn failures(report: &Report) -> Vec<String> {
    report
        .checks
        .iter()
        .filter(|(passed, _)| !passed)
        .map(|(_, line)| line.clone())
        .collect()
}

/// The check line that starts with `start`.
fn line(report: &Report, start: &str) -> String {
    report
        .checks
        .iter()
        .map(|(_, line)| line.clone())
        .find(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("no {start:?} in {:#?}", report.checks))
}

/// The run's fingerprint, from the repeat check's line.
fn run_fingerprint(report: &Report) -> String {
    let line = line(report, "the repeat check");
    line.rsplit("run fingerprint ")
        .next()
        .unwrap()
        .trim_end_matches(')')
        .to_string()
}

/// The settled-hover Scenario, a Whoop 65 with no drag, with `inputs` in
/// place of its Timeline and the Expectation `expect`.
fn hover(inputs: &str, expect: &str) -> String {
    let text = committed("physics/settled-motors-hold-a-hover.toml");
    let head = without_expectations(&text);
    let start = head.find("[inputs]").unwrap();
    format!("{}[inputs]\n{inputs}\n\n{expect}", &head[..start])
}

const ONE_EXPECTATION: &str = "[[expect]]\nwhat  = \"height\"\nat    = \"0.2 s\"\nvalue = \"2 m ± 1 m\"\nbasis = \"observed: a check that the run happened\"\n";

#[test]
fn a_physics_scenario_plays_a_motor_input_track_exactly_as_the_same_timeline() {
    // The track gives every command at 0 s; then a row only when one
    // changes, an empty cell holding the last value.
    let timeline = hover(
        "timeline = [\n  { at = \"0 s\", motors = \"37.3%\" },\n  { at = \"0.1 s\", motors = \"40%, 37.3%, 37.3%, 30%\" },\n  { at = \"0.15 s\", motors = \"40%, 37.3%, 20%, 30%\" },\n]",
        ONE_EXPECTATION,
    );
    let track = format!(
        "# A made-up track.\n{MOTOR_TRACK_HEADER}\n0,37.3,37.3,37.3,37.3\n0.1,40,,,30\n0.15,,,20,\n0.2,,,,\n"
    );
    let played = hover("input_track = \"motors.csv\"", ONE_EXPECTATION);
    let timeline = report(&fixture("motors-timeline", &timeline, &[]));
    let played = report(&fixture(
        "motors-track",
        &played,
        &[("motors.csv", track.as_str())],
    ));
    assert!(timeline.passed(), "{:#?}", failures(&timeline));
    assert!(played.passed(), "{:#?}", failures(&played));
    assert_eq!(run_fingerprint(&played), run_fingerprint(&timeline));
}

#[test]
fn a_broken_motor_input_track_is_refused_naming_its_file_and_line() {
    let played = hover("input_track = \"motors.csv\"", ONE_EXPECTATION);
    let refused = |case: &str, track: &str| {
        failures(&report(&fixture(case, &played, &[("motors.csv", track)])))
    };
    let file = "scenarios/physics/motors.csv";
    assert_eq!(
        refused("motors-header", "time (s),motor 1,motor 2\n0,1,1\n"),
        [format!(
            "{file} line 1: a motor Input Track's first line after its comments is its header, \"{MOTOR_TRACK_HEADER}\", not \"time (s),motor 1,motor 2\""
        )]
    );
    assert_eq!(
        refused(
            "motors-first-row",
            &format!("{MOTOR_TRACK_HEADER}\n0.1,30,30,30,30\n")
        ),
        [format!(
            "{file} line 2: a motor Input Track's first row is at 0 s and gives all four motor commands"
        )]
    );
    assert_eq!(
        refused(
            "motors-above-100",
            &format!("{MOTOR_TRACK_HEADER}\n0,30,30,30,30\n0.1,130,,,\n")
        ),
        [format!(
            "{file} line 3: \"130\" isn't a motor command OpenDrone can read: write a percent from 0 to 100, such as 23.04"
        )]
    );
    assert_eq!(
        refused(
            "motors-between-steps",
            &format!("{MOTOR_TRACK_HEADER}\n0,30,30,30,30\n0.00001,40,,,\n")
        ),
        [format!(
            "{file} line 3: \"0.00001\" isn't a time OpenDrone can use here: write seconds from the Scenario's start, such as 0.002, each a whole number of physics steps (at 8000 Hz one step is 0.000125 s)"
        )]
    );
}

#[test]
fn a_motor_input_track_comes_from_no_input_device() {
    let played = hover(
        "input_track  = \"motors.csv\"\ninput_device = { report_rate = \"250 Hz\", reports_at_rest = true }",
        ONE_EXPECTATION,
    );
    let track = format!("{MOTOR_TRACK_HEADER}\n0,37.3,37.3,37.3,37.3\n");
    let report = report(&fixture(
        "motors-device",
        &played,
        &[("motors.csv", track.as_str())],
    ));
    let failures = failures(&report);
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert!(
        failures[0].ends_with("a Physics or Thrust Stand Scenario's Input Track holds recorded motor commands, which no Input Device sent, so it has no `input_device`"),
        "{failures:#?}"
    );
}

#[test]
fn matching_a_recorded_flight_measures_the_mean_and_root_mean_square_gap_at_the_recorded_moments() {
    // Falling from rest, the vertical speed is -9.81 m/s² × t. Against a
    // recording that says 0 m/s at 0.25, 0.5, 0.75 and 1 s, the gaps are
    // -9.81 m/s² × those times: their mean is -9.81 × 0.625 = -6.13125 m/s,
    // and their root mean square 9.81 × √0.46875 = 6.71645 m/s. The row at
    // 0 s is the stretch's start, which a stretch leaves out, and an empty
    // cell records nothing.
    let recorded = "# Made up.\ntime (s),vertical speed (m/s),height (m)\n0,100,0\n0.25,0,\n0.5,0,\n0.75,0,\n1,0,\n";
    let text = format!(
        "{}recorded_flight = \"recorded.csv\"\n{}\n[[expect]]\nwhat     = \"vertical speed\"\nover     = \"0 s to 1 s\"\nmean_gap = \"-6.13125 m/s ± 0.0001 m/s\"\nbasis    = \"rule: the mean of -9.81 m/s² × 0.25, 0.5, 0.75 and 1 s\"\n\n[[expect]]\nwhat    = \"vertical speed\"\nover    = \"0 s to 1 s\"\nrms_gap = \"6.71645 m/s ± 0.0001 m/s\"\nbasis   = \"rule: their root mean square\"\n",
        free_fall_head(),
        free_fall_rest()
    );
    let report = report(&fixture("gaps", &text, &[("recorded.csv", recorded)]));
    assert!(report.passed(), "{:#?}", failures(&report));
    assert!(
        line(
            &report,
            "vertical speed, mean gap from the recorded flight over 0 s to 1 s: measured -6.13 m/s"
        )
        .contains("(line")
    );
    assert!(line(&report, "vertical speed, root-mean-square gap from the recorded flight over 0 s to 1 s: measured 6.72 m/s").contains("(line"));
}

/// The free-fall Scenario's lines before `[start]`, and from `[start]` to
/// its first Expectation, so a top-level key can go between.
fn free_fall_head() -> String {
    let text = free_fall();
    text[..text.find("[start]").unwrap()].to_string()
}

fn free_fall_rest() -> String {
    let text = free_fall();
    without_expectations(&text)[text.find("[start]").unwrap()..].to_string()
}

#[test]
fn a_gap_needs_a_recorded_flight_with_a_column_for_its_measure_in_its_stretch() {
    let gap = |what: &str, over: &str| {
        format!(
            "[[expect]]\nwhat     = \"{what}\"\nover     = \"{over}\"\nmean_gap = \"0 m/s ± 1 m/s\"\nbasis    = \"observed: a check\"\n"
        )
    };
    // No recorded flight named.
    let none = format!(
        "{}{}",
        without_expectations(&free_fall()),
        gap("vertical speed", "0 s to 1 s")
    );
    let failures_none = failures(&report(&fixture("gap-without-recording", &none, &[])));
    assert!(
        failures_none[0].ends_with("`mean_gap` compares this run with a recorded flight, so the Scenario names one: `recorded_flight = \"<file>.csv\"`, a CSV file beside it"),
        "{failures_none:#?}"
    );
    // A recorded flight without that measure, and one with nothing in the
    // stretch.
    let recorded = "time (s),height (m)\n0.5,0\n";
    let text = format!(
        "{}recorded_flight = \"recorded.csv\"\n{}\n{}\n{}",
        free_fall_head(),
        free_fall_rest(),
        gap("vertical speed", "0 s to 1 s"),
        gap("height", "0.6 s to 1 s").replace("m/s", "m"),
    );
    let failures = failures(&report(&fixture(
        "gap-without-column",
        &text,
        &[("recorded.csv", recorded)],
    )));
    assert_eq!(failures.len(), 2, "{failures:#?}");
    assert!(
        failures[0].ends_with("vertical speed has no column in the recorded flight recorded.csv, so this run can't be compared with it"),
        "{failures:#?}"
    );
    assert!(
        failures[1].ends_with("the recorded flight recorded.csv has no height in this stretch, so there's nothing to compare with"),
        "{failures:#?}"
    );
}

#[test]
fn a_recorded_flight_names_its_columns_as_what_does_with_their_units() {
    let text = format!(
        "{}recorded_flight = \"recorded.csv\"\n{}\n[[expect]]\nwhat     = \"vertical speed\"\nover     = \"0 s to 1 s\"\nmean_gap = \"0 m/s ± 100 m/s\"\nbasis    = \"observed: a check\"\n",
        free_fall_head(),
        free_fall_rest()
    );
    let refused = |case: &str, recorded: &str| {
        failures(&report(&fixture(
            case,
            &text,
            &[("recorded.csv", recorded)],
        )))
    };
    let file = "scenarios/physics/recorded.csv";
    assert_eq!(
        refused(
            "recorded-wrong-unit",
            "time (s),vertical speed (m)\n0.5,0\n"
        ),
        [format!(
            "{file} line 1: \"vertical speed (m)\": vertical speed is a speed, such as \"5 m/s\""
        )]
    );
    assert_eq!(
        refused("recorded-angle", "time (s),roll (°)\n0.5,0\n"),
        [format!(
            "{file} line 1: \"roll\" is an angle, which wraps round, so a run can't be compared with a recording of it yet; record its rate instead"
        )]
    );
    assert_eq!(
        refused("recorded-unknown", "time (s),wobble (m/s)\n0.5,0\n"),
        [format!(
            "{file} line 1: \"wobble\" isn't something the runner measures: each column after the time is something the Scenario measures, named as `what` names it, with its unit in brackets, such as \"roll rate (°/s)\""
        )]
    );
}

#[test]
fn an_accelerometer_feels_nothing_in_free_fall_and_1_g_up_in_a_still_hover() {
    // Basis: Rule. Falling freely, only gravity acts, which an accelerometer
    // doesn't feel; hovering still on settled motors, it feels the thrust
    // carrying the weight: 9.81 m/s² along the up axis.
    let felt = |what: &str, statistic: &str, value: &str| {
        format!(
            "\n[[expect]]\nwhat = \"{what}\"\nover = \"0 s to 0.5 s\"\n{statistic} = \"{value}\"\nbasis = \"rule: as the test says\"\n"
        )
    };
    let falling = format!(
        "{}{}{}{}",
        without_expectations(&free_fall()),
        felt("felt acceleration up", "lowest", "0 m/s² ± 0.000001 m/s²"),
        felt("felt acceleration up", "highest", "0 m/s² ± 0.000001 m/s²"),
        felt(
            "felt acceleration forward",
            "highest",
            "0 m/s² ± 0.000001 m/s²"
        ),
    );
    let report_falling = report(&fixture("felt-in-free-fall", &falling, &[]));
    assert!(report_falling.passed(), "{:#?}", failures(&report_falling));
    let hovering = hover(
        "timeline = [\n  { at = \"0 s\", motors = \"37.3%\" },\n]",
        &format!(
            "{}{}",
            "\n[[expect]]\nwhat = \"felt acceleration up\"\nat = \"0.000125 s\"\nvalue = \"9.81 m/s² ± 0.001 m/s²\"\nbasis = \"rule: settled motors carry the weight at the first step\"\n",
            felt(
                "felt acceleration left",
                "highest",
                "0 m/s² ± 0.000001 m/s²"
            ),
        ),
    );
    let report_hovering = report(&fixture("felt-in-a-hover", &hovering, &[]));
    assert!(
        report_hovering.passed(),
        "{:#?}",
        failures(&report_hovering)
    );
}

#[test]
fn a_scenario_whose_downloaded_data_is_missing_is_skipped_and_blocks_nothing() {
    // Data with no licence is downloaded when it's needed, never committed
    // (docs/data-policy.md): without its files the Scenario is skipped, and
    // passes.
    let text = format!(
        "{}downloaded = \"NeuroBEM\"\n{}",
        free_fall_head(),
        hover("input_track = \"neurobem-motors.csv\"", ONE_EXPECTATION)
            .split_once("[start]")
            .map(|(_, rest)| format!("[start]{rest}"))
            .unwrap()
    );
    let report = report(&fixture("downloaded-missing", &text, &[]));
    assert!(report.passed(), "{:#?}", failures(&report));
    assert!(report.skipped);
    assert!(
        line(&report, "skipped: its NeuroBEM files aren't here")
            .ends_with("so it doesn't block; `cargo xtask download neurobem` downloads them"),
        "{:#?}",
        report.checks
    );
    // With its files there, it runs as any other Scenario.
    let track = format!("{MOTOR_TRACK_HEADER}\n0,37.3,37.3,37.3,37.3\n");
    let report = run_one(
        &fixture(
            "downloaded-here",
            &text,
            &[("neurobem-motors.csv", track.as_str())],
        ),
        &packs(),
        ResultsFile::Write,
        None,
    );
    assert!(report.passed(), "{:#?}", failures(&report));
    assert!(!report.skipped);
}

#[test]
fn only_the_data_sets_opendrone_downloads_can_be_named() {
    let text = format!(
        "{}downloaded = \"Some Dataset\"\n{}",
        free_fall_head(),
        free_fall_rest() + ONE_EXPECTATION
    );
    let failures = failures(&report(&fixture("downloaded-unknown", &text, &[])));
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert!(
        failures[0].ends_with("`downloaded` names the outside data set a Scenario's files are downloaded from, because they may not be committed (docs/data-policy.md): \"NeuroBEM\", not \"Some Dataset\""),
        "{failures:#?}"
    );
}
