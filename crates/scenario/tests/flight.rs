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
    // A copy of the built-in Pack whose Whoop 65 Tune has lost two lines.
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flight-fixtures/short-tune");
    let _ = fs::remove_dir_all(&scratch);
    copy(&repo().root.join("packs"), &scratch.join("packs"));
    let tune = scratch.join("packs/opendrone/quads/whoop-65/tune.txt");
    let short: String = fs::read_to_string(&tune)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("set p_roll ") && !line.starts_with("set mixer_type "))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(&tune, short).unwrap();
    let packs = Packs::open(&scratch.join("packs"), "packs")
        .unwrap()
        .with_test_quads(
            &repo().scenarios_folder().join("test-quads"),
            "scenarios/test-quads",
        );
    let text = tracer();
    let file = changed(
        "whoop-tune",
        &text,
        "\"opendrone/freestyle-5\"",
        "\"opendrone/whoop-65\"",
    );
    let found = failures(&run_one(&file, &packs, ResultsFile::Write, None));
    assert_eq!(
        found,
        [format!(
            "scenarios/whoop-tune.toml line {}: the Quad \"opendrone/whoop-65\" can't fly with our Flight Controller yet: its Tune doesn't spell out `p_roll`, `mixer_type` (ADR-0015)",
            line_of(&text, "quad              =")
        )]
    );
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
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
fn angle_horizon_input_smoothing_and_endless_battery_wait_for_their_tickets() {
    let text = tracer()
        .replacen(
            "flight_mode       = \"Acro\"",
            "flight_mode       = \"Angle\"",
            1,
        )
        .replacen("input_smoothing = \"off\"", "input_smoothing = \"on\"", 1)
        .replacen("endless_battery = \"off\"", "endless_battery = \"on\"", 1);
    let found = failures(&report(&fixture("angle-assists", &text)));
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(found[0].ends_with(
        "the Flight Controller flies Acro so far: Angle and Horizon arrive with their ticket (#51)"
    ));
    assert!(found[1].ends_with(
        "Input smoothing doesn't run yet, so `input_smoothing` must be \"off\": Input smoothing arrives with the Radio Link ticket (#56)"
    ));
    assert!(found[2].ends_with(
        "Endless Battery doesn't run yet, so `endless_battery` must be \"off\": Endless Battery arrives with its ticket (#57)"
    ));
}

#[test]
fn auto_arm_flies_in_a_flight_scenario_but_not_with_the_flight_controller_alone() {
    // Auto-arm is the Simulation's Assist, in front of the Flight
    // Controller (ADR-0003).
    let text = arming().replacen("auto_arm = \"off\"", "auto_arm = \"on\"", 1);
    let found = failures(&report(&fixture("fc-auto-arm", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "Auto-arm is an Assist of the Simulation, in front of the Flight Controller, and a Flight Controller Scenario runs the Flight Controller alone, so `auto_arm` must be \"off\""
    ));
    let auto_arm = scenario("flight-controller/auto-arm");
    assert!(read_scenario("auto-arm", &auto_arm).is_ok());
}

#[test]
fn with_auto_arm_on_no_arm_switch_is_bound_so_arm_stays_off() {
    let text = scenario("flight-controller/auto-arm");
    let old = "{ at = \"2 s\",    throttle = \"20%\" },";
    let file = changed(
        "auto-arm-switch",
        &text,
        old,
        "{ at = \"2 s\",    throttle = \"20%\", arm = \"on\" },",
    );
    assert_eq!(
        failures(&report(&file)),
        [format!(
            "scenarios/auto-arm-switch.toml line {}: with Auto-arm on, no Arm switch is bound (it would take over), so `arm` stays \"off\": Auto-arm turns Arm on itself",
            line_of(&text, old)
        )]
    );
}

/// A landed Flight Scenario that starts as Reset leaves the Quad.
fn landed() -> String {
    scenario("flight-controller/arm-switch-on-at-power-up")
}

#[test]
fn a_landed_flight_scenario_starts_as_reset_leaves_the_quad_disarmed_and_still() {
    // A landed start with a "fresh" Flight Controller is exactly Reset: its
    // ESCs power up first, and it is disarmed and still (#32 §4).
    let text = landed()
        .replacen("armed             = false", "armed             = true", 1)
        .replacen(
            "speed             = \"0 m/s\"",
            "speed             = \"1 m/s east, 0 m/s north, 0 m/s up\"",
            1,
        );
    let found = failures(&report(&fixture("landed-moving", &text)));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].ends_with(
        "a Flight Scenario whose motors start \"powering up\" starts as Reset leaves the Quad, disarmed: `armed` must be false"
    ));
    assert!(found[1].ends_with(
        "a Flight Scenario whose motors start \"powering up\" starts as Reset leaves the Quad, still: its `speed` must be zero"
    ));
}

#[test]
fn reset_needs_a_scenario_that_starts_as_reset_leaves_the_quad() {
    // Reset puts the Quad back where the Scenario starts, its Launch Spot.
    let text = tracer();
    let old = "{ at = \"1.5 s\", roll = \"0%\" },";
    let file = changed(
        "tracer-reset",
        &text,
        old,
        "{ at = \"1.5 s\", roll = \"0%\", reset = true },",
    );
    assert_eq!(
        failures(&report(&file)),
        [format!(
            "scenarios/tracer-reset.toml line {}: Reset puts the Quad back where the Scenario starts, as its Launch Spot, so a Scenario with Reset starts as Reset leaves the Quad: landed and still, disarmed, with its motors \"powering up\"",
            line_of(&text, old)
        )]
    );
    // And a Flight Controller Scenario has no Quad to put back.
    let text = arming().replacen(
        "{ at = \"0.5 s\", arm = \"off\" },",
        "{ at = \"0.5 s\", arm = \"off\", reset = true },",
        1,
    );
    let found = failures(&report(&fixture("fc-reset", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].contains("`reset` isn't something OpenDrone reads"),
        "{found:#?}"
    );
}

#[test]
fn the_flying_input_device_is_lost_before_it_comes_back() {
    let text = scenario("flight-controller/failsafe");
    let old = "{ at = \"1 s\",   input_device = \"lost\" },";
    let file = changed(
        "back-twice",
        &text,
        old,
        "{ at = \"1 s\",   input_device = \"back\" },",
    );
    let found = failures(&report(&file));
    assert_eq!(
        found,
        [
            format!(
                "scenarios/back-twice.toml line {}: the Flying Input Device is back at 1 s, but it wasn't lost",
                line_of(&text, old)
            ),
            format!(
                "scenarios/back-twice.toml line {}: the Flying Input Device is back at 4 s, but it wasn't lost",
                line_of(&text, "input_device = \"back\"")
            ),
        ]
    );
    let file = changed(
        "lost-word",
        &text,
        old,
        "{ at = \"1 s\",   input_device = \"unplugged\" },",
    );
    let found = failures(&report(&file));
    assert!(
        found.iter().any(|line| line.ends_with(
            "`input_device` is the Flying Input Device \"lost\" (unplugged, so the Radio Link sends no frames) or \"back\", not \"unplugged\""
        )),
        "{found:#?}"
    );
}

#[test]
fn a_radio_link_drop_out_is_written_as_how_long_it_drops_out_for() {
    let text = scenario("flight-controller/radio-link-drop-outs");
    let old = "radio_link = \"drops out for 0.1 s\"";
    let file = changed("drop-out-words", &text, old, "radio_link = \"0.1 s\"");
    assert_eq!(
        failures(&report(&file)),
        [format!(
            "scenarios/drop-out-words.toml line {}: `radio_link` in a Timeline is a drop-out: the Radio Link sends no frames for a while, written like \"drops out for 0.2 s\", not \"0.1 s\"",
            line_of(&text, old)
        )]
    );
    // A drop-out at 1 s for 0.1 s is the device lost at 1 s and back at
    // 1.1 s; one that ends between two steps is refused.
    let file = changed(
        "drop-out-between-steps",
        &text,
        old,
        "radio_link = \"drops out for 0.00001 s\"",
    );
    let found = failures(&report(&file));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].contains("isn't a whole number of physics steps"),
        "{found:#?}"
    );
}

#[test]
fn something_that_happens_is_measured_by_when_it_first_does_over_a_stretch() {
    let wrong = [
        (
            "what = \"the Quad disarms\"\nat = \"1 s\"\nvalue = \"1 s ± 1 s\"",
            "\"the Quad disarms\" is something that happens: say `over` a stretch, with `first`, how long after the stretch's start it first happens, such as \"1.5 s ± 0.01 s\", or \"never\"",
        ),
        (
            "what = \"roll setpoint\"\nover = \"1 s to 2 s\"\nfirst = \"1 s ± 1 s\"",
            "`first` is for something that happens, such as \"the Quad disarms\", not roll setpoint",
        ),
        (
            "what = \"roll setpoint\"\nover = \"1 s to 2 s\"\nmean = \"never\"",
            "\"never\" is for something that happens, over a stretch with `first`, such as `what = \"the Quad disarms\"`; a quantity needs a value with a tolerance",
        ),
    ];
    for (n, (expectation, sentence)) in wrong.into_iter().enumerate() {
        let text = arming() + &format!("\n[[expect]]\n{expectation}\nbasis = \"rule: x\"\n");
        let found = failures(&report(&fixture(&format!("happens-{n}"), &text)));
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0].contains(sentence), "{found:#?}");
    }
}

#[test]
fn something_expected_never_to_happen_fails_saying_when_it_did_and_the_other_way_round() {
    // The arming Scenario's Quad disarms at 0.516 s.
    let text = arming()
        + "\n[[expect]]\nwhat = \"the Quad disarms\"\nover = \"0.5 s to 1 s\"\nfirst = \"never\"\nbasis = \"rule: x\"\n"
        + "\n[[expect]]\nwhat = \"the Quad disarms\"\nover = \"0.6 s to 1 s\"\nfirst = \"0.1 s ± 0.1 s\"\nbasis = \"rule: x\"\n";
    let found = failures(&report(&fixture("never", &text)));
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found[0].starts_with(
            "the Quad disarms, first over 0.5 s to 1 s: measured after 0.0161 s, expected never"
        ),
        "{found:#?}"
    );
    assert!(
        found[1].starts_with(
            "the Quad disarms, first over 0.6 s to 1 s: measured never, expected 0.1 s ± 0.1 s"
        ),
        "{found:#?}"
    );
}

#[test]
fn a_case_cant_see_something_happen() {
    let text = cases().replacen("what  = \"motor 1 DShot\"", "what  = \"the Quad arms\"", 1);
    let found = failures(&report(&fixture("case-event", &text)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].ends_with(
        "a case is one loop, so \"the Quad arms\", something that happens between two loops, can't be seen in one"
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
