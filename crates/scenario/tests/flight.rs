//! Readable checks for the Flight and Flight Controller kinds of Scenario:
//! pilots' Timelines, tables of cases, mirrored sticks, and what each kind
//! may say. Each broken case is a committed Scenario with one change, run
//! against the real built-in Pack. Basis: Rule (#11 §2, ADR-0002) unless
//! said.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::Packs;
use opendrone_scenario::{Repo, Report, ResultsFile, ScenarioFile, read_scenario, run_one};

fn repo() -> Repo {
    Repo::around(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the runner lives in the repo")
}

fn packs() -> Packs {
    repo()
        .packs()
        .unwrap_or_else(|p| panic!("the Packs should read:\n{p}"))
}

fn scenario(relative: &str) -> String {
    fs::read_to_string(repo().root.join(format!("scenarios/{relative}.toml"))).unwrap()
}

/// The full-right-roll tracer.
fn tracer() -> String {
    scenario("flight-controller/full-right-roll-reaches-the-max-rate")
}

/// A Flight Controller Scenario fed a Timeline.
fn arming() -> String {
    scenario("flight-controller/arming")
}

/// A Flight Controller Scenario fed a table of cases.
fn cases() -> String {
    scenario("flight-controller/motor-idle-and-output-limit")
}

fn fixture(case: &str, text: &str) -> ScenarioFile {
    let folder: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flight-fixtures");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{case}.toml"));
    fs::write(&path, text).unwrap();
    ScenarioFile {
        relative: case.to_string(),
        path,
    }
}

fn changed(case: &str, text: &str, old: &str, new: &str) -> ScenarioFile {
    assert!(text.contains(old), "no {old:?} to change");
    fixture(case, &text.replacen(old, new, 1))
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

fn line_of(text: &str, needle: &str) -> usize {
    text.lines().position(|line| line.contains(needle)).unwrap() + 1
}

#[test]
fn a_pilots_timeline_starts_by_setting_every_stick_and_the_arm_switch() {
    // ADR-0002: the run starts from values the file states.
    let text = tracer();
    let old = "{ at = \"0 s\",   roll = \"0%\", pitch = \"0%\", yaw = \"0%\", throttle = \"30%\", arm = \"on\" }";
    let file = changed(
        "no-arm-at-0",
        &text,
        old,
        "{ at = \"0 s\",   roll = \"0%\", pitch = \"0%\", throttle = \"30%\" }",
    );
    assert_eq!(
        failures(&report(&file)),
        [format!(
            "scenarios/no-arm-at-0.toml line {}: the Timeline starts with a moment at 0 s that sets every stick and the Arm switch (ADR-0002); it doesn't set `yaw`, `arm`",
            line_of(&text, old)
        )]
    );
}

#[test]
fn a_ramp_moves_a_stick_in_a_straight_line_from_where_it_was_last_set() {
    // Basis: Source. Roll ramps from 0% at 0 s to 100% at 0.1 s. The frame
    // that leaves at 0.048 s carries 48%, the nearest CRSF step to 1500 + 512
    // × 0.48 µs being 1385, which Betaflight reads as 1746.31 µs: a
    // deflection of 0.49262, for which Actual 70/670/0 asks 70 × 0.49262 +
    // 600 × 0.49262² = 180.09 °/s. The loop just before 0.05 s still has that
    // frame.
    let text = arming().replacen(
        "  # Throttle low, level: arms.\n",
        "  { at = \"0.1 s\", roll = \"ramp to 100%\" },\n",
        1,
    ) + "\n[[expect]]\nwhat = \"roll setpoint\"\nat = \"0.05 s\"\nvalue = \"180.09 °/s ± 0.01 °/s\"\nbasis = \"source: the ramp's frame at 0.048 s\"\n"
        + "\n[[expect]]\nwhat = \"roll setpoint\"\nat = \"0.15 s\"\nvalue = \"670 °/s ± 0.01 °/s\"\nbasis = \"source: the ramp ends at full stick\"\n";
    let report = report(&fixture("ramp", &text));
    assert!(report.passed(), "{:#?}", failures(&report));
}

#[test]
fn a_ramp_needs_an_earlier_value_to_ramp_from() {
    let text = arming();
    let old = "{ at = \"0.1 s\", arm = \"on\" }";
    let file = changed(
        "ramp-from-nothing",
        &text,
        old,
        "{ at = \"0.1 s\", arm = \"on\" },\n  { at = \"0.2 s\", throttle = \"ramp to 10%\" }",
    );
    // The throttle is set at 0 s, so this ramp reads; a ramp at 0 s doesn't.
    let text = fs::read_to_string(&file.path).unwrap();
    assert!(read_scenario(&file.label(), &text).is_ok());
    let file = changed(
        "ramp-at-0",
        &text,
        "throttle = \"0%\", arm = \"off\" }",
        "throttle = \"ramp to 10%\", arm = \"off\" }",
    );
    let found = failures(&report(&file));
    assert!(
        found.iter().any(|line| line.ends_with(
            "`throttle` ramps to its value from where an earlier moment set it, but no earlier moment sets `throttle`"
        )),
        "{found:#?}"
    );
}

#[test]
fn a_quad_whose_tune_doesnt_spell_out_every_flight_controller_setting_cant_fly() {
    // ADR-0015: a Tune spells out every setting the Flight Controller reads.
    // The Whoop 65's Tune gets its settings from the importer (#53).
    let text = tracer();
    let file = changed(
        "whoop-tune",
        &text,
        "\"opendrone/freestyle-5\"",
        "\"opendrone/whoop-65\"",
    );
    let found = failures(&report(&file));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with(&format!(
            "scenarios/whoop-tune.toml line {}: the Quad \"opendrone/whoop-65\" can't fly with our Flight Controller yet: its Tune doesn't spell out `small_angle`, `p_roll`, ",
            line_of(&text, "quad              =")
        )),
        "{found:#?}"
    );
    assert!(found[0].ends_with("`mixer_type` (ADR-0015)"), "{found:#?}");
}

#[test]
fn a_flight_controller_scenario_leaves_out_the_map_place_motors_and_battery() {
    let text = cases();
    let file = changed(
        "with-a-map",
        &text,
        "attitude          =",
        "map               = \"test/empty-air\"\nbattery           = \"100%\"\nattitude          =",
    );
    let found = failures(&report(&file));
    assert_eq!(found.len(), 2, "{found:#?}");
    for (found, key) in found.iter().zip(["map", "battery"]) {
        assert!(found.ends_with(&format!(
            "a Flight Controller Scenario runs the Flight Controller alone, with no Map, place, motors or battery, so its [start] has no `{key}`"
        )), "{found}");
    }
}

#[test]
fn a_flight_controller_scenario_measures_only_what_the_flight_controller_does() {
    let text = arming()
        + "\n[[expect]]\nwhat = \"height\"\nat = \"1 s\"\nvalue = \"0 m ± 1 m\"\nbasis = \"rule: x\"\n";
    let found = failures(&report(&fixture("fc-height", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "a Flight Controller Scenario runs the Flight Controller alone, so it measures only what the Flight Controller does, not height"
    ));
}

#[test]
fn a_scenario_with_scripted_motors_cant_measure_the_flight_controller() {
    let text = fs::read_to_string(
        repo()
            .root
            .join("scenarios/physics/free-fall-is-exactly-g.toml"),
    )
    .unwrap()
        + "\n[[expect]]\nwhat = \"roll setpoint\"\nat = \"1 s\"\nvalue = \"0 °/s ± 1 °/s\"\nbasis = \"rule: x\"\n";
    let found = failures(&report(&fixture("physics-setpoint", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "roll setpoint is our Flight Controller's, but a Physics Scenario's motors are scripted"
    ));
}

#[test]
fn what_a_flight_controller_loop_did_cant_be_measured_before_the_first_loop() {
    let text = tracer()
        + "\n[[expect]]\nwhat = \"motor 1 DShot\"\nat = \"0 s\"\nvalue = \"0 ± 1\"\nbasis = \"rule: x\"\n";
    let found = failures(&report(&fixture("dshot-at-0", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "motor 1 DShot is what a Flight Controller loop did, and the first loop runs during the first step, so it can't be measured at 0 s"
    ));
}

#[test]
fn angle_horizon_and_the_assists_wait_for_their_tickets() {
    let text = tracer()
        .replacen(
            "flight_mode       = \"Acro\"",
            "flight_mode       = \"Angle\"",
            1,
        )
        .replacen("auto_arm = \"off\"", "auto_arm = \"on\"", 1);
    let found = failures(&report(&fixture("angle-auto-arm", &text)));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].ends_with(
        "the Flight Controller flies Acro so far: Angle and Horizon arrive with their ticket (#51)"
    ));
    assert!(found[1].ends_with(
        "Assists don't run yet, so `auto_arm` must be \"off\": Auto-arm arrives with the arming ticket (#52)"
    ));
}

#[test]
fn mirrored_sticks_need_a_flight_that_starts_as_its_own_mirror_image() {
    let mirrored = "\n[[expect]]\nwhat = \"roll rate\"\nat = \"1.2 s\"\nvalue = \"-100% ± 1%\"\nagainst = { sticks = \"mirrored\" }\ncompare = \"ratio\"\nbasis = \"rule: x\"\n";
    let text = tracer().replacen(
        "attitude          = \"level, heading 0°\"",
        "attitude          = \"roll 10°, pitch 0°, heading 0°\"",
        1,
    ) + mirrored;
    let found = failures(&report(&fixture("mirror-rolled", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "mirrored sticks give a mirrored flight only from a start that is its own mirror image: no roll, no roll or yaw rotation, and no speed sideways to the heading"
    ));

    let text = arming() + mirrored.replacen("roll rate", "roll setpoint", 1).as_str();
    let found = failures(&report(&fixture("mirror-fc", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "only a Flight Scenario's sticks can be mirrored, where the pilot flies the Quad"
    ));
}

#[test]
fn a_case_sets_every_stick_and_the_arm_switch_and_never_ramps() {
    let text = cases();
    let file = changed(
        "case-without-arm",
        &text,
        "throttle = \"0%\"\narm      = \"on\"\n",
        "throttle = \"ramp to 10%\"\n",
    );
    let found = failures(&report(&file));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].ends_with(
        "a case sets every stick and the Arm switch (ADR-0002); this one doesn't set `arm`"
    ));
    assert!(
        found[1].ends_with("a case is one loop, so `throttle` can't ramp: give the value itself")
    );
}

#[test]
fn a_table_of_cases_is_only_for_the_flight_controller_alone() {
    let text = tracer() + "\n[[case]]\nroll = \"0%\"\n";
    let found = failures(&report(&fixture("tracer-cases", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "a table of [[case]]s feeds only a Flight Controller Scenario, which runs the Flight Controller alone"
    ));
}

#[test]
fn a_results_file_names_each_case_and_says_the_flight_controller_ran_alone() {
    let report = report(&fixture("cases-results", &cases()));
    assert!(report.passed(), "{:#?}", failures(&report));
    let results = fs::read_to_string(
        Path::new(env!("CARGO_TARGET_TMPDIR")).join("flight-fixtures/cases-results.results.toml"),
    )
    .unwrap();
    for line in [
        "what     = \"motor 1 DShot with arm on\"",
        "what     = \"motor 1 DShot with throttle 50%, arm on\"",
        "map  = \"none\"   # the Flight Controller runs alone, with no Map",
        "\"case 3\" = ",
    ] {
        assert!(results.contains(line), "no {line:?} in\n{results}");
    }
}
