//! Readable checks for Scenarios that play back an Input Track: a CSV file of
//! recorded Flight Inputs beside the Scenario, with a row only when something
//! happens (#11 §2, #56), and the device facts that come with it. Each broken
//! case is a committed Scenario or Input Track with one change, run against
//! the real built-in Pack. Basis: Rule (#11 §2, ADR-0002) unless said.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::Packs;
use opendrone_scenario::{INPUT_TRACK_HEADER, Repo, Report, ResultsFile, ScenarioFile, run_one};

fn repo() -> Repo {
    Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the runner lives in the repo")
}

fn packs() -> Packs {
    repo()
        .packs()
        .unwrap_or_else(|p| panic!("the Packs should read:\n{p}"))
}

/// The DualSense lock Scenario, which plays `dualsense-at-rest.csv`.
fn locked() -> String {
    fs::read_to_string(
        repo()
            .root
            .join("scenarios/recorded/dualsense-locks-to-its-report-beat-at-250-hz.toml"),
    )
    .unwrap()
}

/// A short Input Track: the sticks at rest, a roll, and heartbeats.
fn track() -> String {
    format!(
        "# A made-up track.\n{INPUT_TRACK_HEADER}\n0,1500,1500,988,1500,988,988,988,\n0.004,1600,,,,,,,\n0.008,,,,,,,,\n0.012,1500,,,,,,,\n"
    )
}

/// Writes a Scenario, and an Input Track named `track.csv` beside it if
/// given, into a folder of their own, as `recorded/<case>.toml` would be.
fn fixture(case: &str, scenario: &str, track: Option<&str>) -> ScenarioFile {
    let folder: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("track-fixtures")
        .join(case)
        .join("recorded");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{case}.toml"));
    let scenario = scenario.replace("dualsense-at-rest.csv", "track.csv");
    fs::write(&path, scenario).unwrap();
    if let Some(track) = track {
        fs::write(folder.join("track.csv"), track).unwrap();
    }
    ScenarioFile {
        relative: format!("recorded/{case}"),
        path,
    }
}

/// The lock Scenario on `track`, with Expectations the short track meets.
fn short(case: &str, track: &str) -> ScenarioFile {
    let text = locked();
    let start = text.find("[[expect]]").unwrap();
    let scenario = format!(
        "{}[[expect]]\nwhat    = \"reports in the frame\"\nover    = \"0 s to 0.012 s\"\nhighest = \"1 ± 0\"\nbasis   = \"rule: one report every 4 ms\"\n",
        &text[..start]
    );
    fixture(case, &scenario, Some(track))
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

#[test]
fn a_scenario_plays_an_input_track_from_a_csv_file_beside_it() {
    let found = report(&short("plays", &track()));
    assert!(found.passed(), "{:#?}", failures(&found));
    // The run lasts to the track's last row: four rows, a frame each.
    let found = report(&short(
        "plays-longer",
        &format!("{}0.016,,,,,,,,\n", track()),
    ));
    assert!(found.passed(), "{:#?}", failures(&found));
}

#[test]
fn an_input_track_that_isnt_there_is_reported_by_its_file() {
    let found = failures(&report(&fixture("missing", &locked(), None)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("scenarios/recorded/track.csv: can't be read:"),
        "{found:#?}"
    );
}

#[test]
fn an_input_track_starts_with_its_header_and_a_row_at_0_s_that_gives_every_channel() {
    let no_header = track().replace(INPUT_TRACK_HEADER, "time,roll");
    let found = failures(&report(&short("no-header", &no_header)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with("scenarios/recorded/track.csv line 2: an Input Track's first line after its comments is its header"));

    let missing = track().replace(
        "0,1500,1500,988,1500,988,988,988,",
        "0,1500,1500,,1500,988,988,988,",
    );
    let found = failures(&report(&short("first-row", &missing)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 3: an Input Track's first row is at 0 s and gives every Channel: roll, pitch, throttle, yaw, arm, flight mode and crash flip"
        ]
    );

    let late = track().replace(
        "0,1500,1500,988,1500,988,988,988,",
        "0.004,1500,1500,988,1500,988,988,988,",
    );
    let found = failures(&report(&short("first-row-late", &late)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains("line 3: an Input Track's first row is at 0 s"));
}

#[test]
fn an_input_tracks_rows_fall_on_whole_steps_in_time_order() {
    let between = track().replace("0.008,,,,,,,,", "0.00801,,,,,,,,");
    let found = failures(&report(&short("between-steps", &between)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 5: \"0.00801\" isn't a time OpenDrone can use here: write seconds from the Scenario's start, such as 0.004, each a whole number of physics steps (at 8000 Hz one step is 0.000125 s)"
        ]
    );
    let backwards = track().replace("0.008,,,,,,,,", "0.002,,,,,,,,");
    let found = failures(&report(&short("backwards", &backwards)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 5: rows go in time order, but 0.002 s comes after 0.004 s"
        ]
    );
}

#[test]
fn a_row_is_one_flight_input_with_channels_a_receiver_can_hold() {
    let both = track().replace("0.008,,,,,,,,", "0.008,1550,,,,,,,input device lost");
    let found = failures(&report(&short("both", &both)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 5: a row is one Flight Input: Channels, or an event, not both"
        ]
    );
    let beyond = track().replace("0.004,1600,", "0.004,2400,");
    let found = failures(&report(&short("beyond", &beyond)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 4: 2400 µs is beyond what a receiver's Channel holds: ELRS's 11 bits run from about 881 to 2159 µs"
        ]
    );
    let comma = track().replace("0.004,1600,", "0.004,1600;5,");
    let found = failures(&report(&short("not-a-number", &comma)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 4: \"1600;5\" isn't a Channel OpenDrone can read: write it in µs with a decimal point, such as 1503.5"
        ]
    );
    let short_row = track().replace("0.008,,,,,,,,", "0.008,,,");
    let found = failures(&report(&short("short-row", &short_row)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 5: a row has 9 cells, as the header does (the time, seven Channels and an event), not 4"
        ]
    );
}

#[test]
fn the_device_is_lost_and_back_by_turns_and_reset_needs_a_flight_that_starts_as_reset() {
    let twice = format!(
        "{}0.016,,,,,,,,input device lost\n0.02,,,,,,,,input device lost\n",
        track()
    );
    let found = failures(&report(&short("lost-twice", &twice)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 8: the Flying Input Device is lost at 0.02 s, but it was already lost: it must come back first"
        ]
    );
    let back = format!("{}0.016,,,,,,,,input device back\n", track());
    let found = failures(&report(&short("back-first", &back)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 7: the Flying Input Device is back at 0.016 s, but it wasn't lost"
        ]
    );
    let reset = format!("{}0.016,,,,,,,,reset\n", track());
    let found = failures(&report(&short("reset", &reset)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with(
        "scenarios/recorded/track.csv line 7: Reset puts the Quad back where the Scenario starts"
    ));
    let other = format!("{}0.016,,,,,,,,unplugged\n", track());
    let found = failures(&report(&short("unknown-event", &other)));
    assert_eq!(
        found,
        [
            "scenarios/recorded/track.csv line 7: `event` is empty, \"input device lost\", \"input device back\" or \"reset\", not \"unplugged\""
        ]
    );
}

#[test]
fn an_input_track_comes_with_its_devices_facts_and_a_timeline_has_none() {
    let text = locked();
    let no_facts = text.replace(
        "input_device = { report_rate = \"250 Hz\", reports_at_rest = true }\n",
        "",
    );
    let found = failures(&report(&fixture("no-facts", &no_facts, Some(&track()))));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].ends_with("[inputs] is missing `input_device`"),
        "{found:#?}"
    );

    let fraction = text.replace("report_rate = \"250 Hz\"", "report_rate = \"249.5 Hz\"");
    let found = failures(&report(&fixture("fraction", &fraction, Some(&track()))));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "a Report Rate is a whole number of reports a second, such as \"250 Hz\", or \"unknown\""
    ));

    let unknown = text.replace("report_rate = \"250 Hz\"", "report_rate = \"unknown\"");
    let start = unknown.find("[[expect]]").unwrap();
    // With no Report Rate the link runs on its own clock, and its frames at
    // 0, 4 and 8 ms catch the reports at 0, 4 and 8 ms as they arrive.
    let unknown = format!(
        "{}[[expect]]\nwhat    = \"frame age\"\nover    = \"0 s to 0.012 s\"\nhighest = \"0 ms ± 0.000001 ms\"\nbasis   = \"rule: on the link's own clock\"\n",
        &unknown[..start]
    );
    let found = report(&fixture("unknown", &unknown, Some(&track())));
    assert!(found.passed(), "{:#?}", failures(&found));

    let timeline = fs::read_to_string(repo().root.join("scenarios/flight-controller/arming.toml"))
        .unwrap()
        .replacen(
            "[inputs]\n",
            "[inputs]\ninput_device = { report_rate = \"250 Hz\", reports_at_rest = true }\n",
            1,
        );
    let found = failures(&report(&fixture("timeline-facts", &timeline, None)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "`input_device` gives the facts of the Input Device an Input Track was recorded from; a Timeline's sticks are scripted, with no device"
    ));
}

#[test]
fn an_input_track_names_a_csv_file_beside_its_scenario_and_feeds_only_a_pilots_scenario() {
    let text = locked().replace("\"dualsense-at-rest.csv\"", "\"../traces/dualsense.csv\"");
    let found = failures(&report(&fixture("elsewhere", &text, None)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "`input_track` names a CSV file beside the Scenario, such as \"dualsense-at-rest.csv\", not \"../traces/dualsense.csv\""
    ));

    let physics = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-fall-is-exactly-g.toml"),
    )
    .unwrap();
    let inputs = physics.find("[inputs]").unwrap();
    let expect = physics.find("[[expect]]").unwrap();
    let physics = format!(
        "{}[inputs]\ninput_track  = \"track.csv\"\ninput_device = {{ report_rate = \"250 Hz\", reports_at_rest = true }}\n\n{}",
        &physics[..inputs],
        &physics[expect..]
    );
    let found = failures(&report(&fixture("physics-track", &physics, Some(&track()))));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "an Input Track holds a pilot's Flight Inputs, so it feeds a Flight or Flight Controller Scenario; a Physics or Thrust Stand Scenario scripts its motors in a Timeline"
    ));
}

#[test]
fn a_scenario_that_plays_an_input_track_cant_be_compared_with_another_run_yet() {
    let text = format!(
        "{}\n[[expect]]\nwhat    = \"roll setpoint\"\nat      = \"0.004 s\"\nagainst = {{ physics_rate = \"4 kHz\" }}\ncompare = \"difference\"\nvalue   = \"0 °/s ± 1 °/s\"\nbasis   = \"rule: x\"\n",
        locked()
    );
    let found = failures(&report(&fixture("compared", &text, Some(&track()))));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "an Input Track plays back as it was recorded, at the physics rate it was recorded at, so a Scenario that plays one can't be compared with another run yet"
    ));
}

#[test]
fn what_the_radio_link_does_is_measured_only_where_the_sticks_reach_a_flight_controller() {
    let physics = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-fall-is-exactly-g.toml"),
    )
    .unwrap();
    let text = format!(
        "{physics}\n[[expect]]\nwhat    = \"reports in the frame\"\nover    = \"0 s to 0.5 s\"\nhighest = \"1 ± 0\"\nbasis   = \"rule: x\"\n"
    );
    let found = failures(&report(&fixture("physics-link", &text, None)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "reports in the frame is the Radio Link's, but a Physics Scenario's motors are scripted, so no sticks reach a Flight Controller"
    ));

    let at_start = format!(
        "{}\n[[expect]]\nwhat  = \"roll channel\"\nat    = \"0 s\"\nvalue = \"0% ± 1%\"\nbasis = \"rule: x\"\n",
        locked()
    );
    let found = failures(&report(&fixture("link-at-0", &at_start, Some(&track()))));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "roll channel is what the Radio Link did, and its first frame leaves during the first step, so it can't be measured at 0 s"
    ));
}
