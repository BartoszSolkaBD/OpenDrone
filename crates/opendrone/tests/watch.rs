//! Readable checks for watching a Scenario in the game (#73): a Flight or
//! Thrust Stand Scenario opened by the developer command and stepped by the
//! game's own stepping flies the flight the headless runner measures, and
//! nobody's sticks, Reset or Save Flight change it, while Pause stops it.
//! Basis: Rule (ADR-0001, ADR-0003): the same starting state and Flight
//! Inputs give the same flight, bit for bit. Times in milliseconds on both
//! clocks.

mod common;

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use opendrone::pilot::{FlightWorld, Pilot};
use opendrone::set_up::repo_folder;
use opendrone::stepping::Flight;
use opendrone::watch::open_scenario;
use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::Connection;
use opendrone_input::{Batch, DeviceInfo, DeviceState, Raw, SdlId};
use opendrone_maths::Fingerprint;

const DUALSENSE: SdlId = SdlId(2);

/// The `run` fingerprint in a Scenario's Results file, which the headless
/// runner wrote.
fn runner_fingerprint(scenario: &Path) -> Fingerprint {
    let results = scenario.with_file_name(format!(
        "{}.results.toml",
        scenario.file_stem().unwrap().to_string_lossy()
    ));
    let text = std::fs::read_to_string(&results).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    let hex = table["fingerprints"]["run"].as_str().unwrap();
    Fingerprint(u64::from_str_radix(hex, 16).unwrap())
}

fn scenario(path: &str) -> std::path::PathBuf {
    repo_folder().join("scenarios").join(path)
}

#[test]
fn a_flight_scenario_watched_in_the_game_flies_the_flight_the_runner_measures() {
    let path = scenario("flight-controller/arming-waits-for-the-escs-after-reset.toml");
    let watched = open_scenario(&repo_folder(), &path).unwrap();
    assert_eq!(watched.plan.quad.id, "opendrone/freestyle-5");
    assert_eq!(watched.plan.map.id, "test/flat-floor");
    assert!(
        watched.watching.name.contains("Arming waits"),
        "{:?}",
        watched.watching
    );
    let mut flight = watched.flight;
    while !flight.finished() {
        flight.tick();
    }
    assert_eq!(flight.run_fingerprint(), Some(runner_fingerprint(&path)));
}

#[test]
fn a_thrust_stand_scenario_that_scripts_its_motors_can_be_watched() {
    let path = scenario("quads/whoop-65/motor-step.toml");
    let watched = open_scenario(&repo_folder(), &path).unwrap();
    let mut flight = watched.flight;
    while !flight.finished() {
        flight.tick();
    }
    assert_eq!(flight.run_fingerprint(), Some(runner_fingerprint(&path)));
}

#[test]
fn a_thrust_stand_scenario_that_flies_our_flight_controller_can_be_watched() {
    let path = scenario("quads/freestyle-5/a-reversed-motor-gives-the-quads-reverse-share.toml");
    let watched = open_scenario(&repo_folder(), &path).unwrap();
    let mut flight = watched.flight;
    while !flight.finished() {
        flight.tick();
    }
    assert_eq!(flight.run_fingerprint(), Some(runner_fingerprint(&path)));
}

#[test]
fn only_flight_and_thrust_stand_scenarios_can_be_watched() {
    for (path, kind) in [
        ("flight-controller/arming.toml", "Flight Controller"),
        ("physics/free-fall-is-exactly-g.toml", "Physics"),
    ] {
        let Err(problem) = open_scenario(&repo_folder(), &scenario(path)) else {
            panic!("{kind} Scenario {path} opened");
        };
        assert!(
            problem.contains("only those can be watched"),
            "{kind}: {problem}"
        );
    }
}

#[test]
fn a_file_outside_scenarios_is_refused_in_plain_words() {
    let Err(problem) = open_scenario(&repo_folder(), Path::new("Cargo.toml")) else {
        panic!("Cargo.toml opened");
    };
    assert!(
        problem.contains("isn't a Scenario in scenarios/"),
        "{problem}"
    );
}

/// A DualSense over USB with its motion sensors on.
fn dualsense() -> Raw {
    Raw::Added {
        device: DUALSENSE,
        info: DeviceInfo {
            name: "DualSense Wireless Controller".to_string(),
            usb_vendor: 0x054C,
            usb_product: 0x0CE6,
            sdl_gamepad: true,
            connection: Some(Connection::Usb),
            heartbeat: true,
        },
        state: DeviceState::new(6, 0, true),
    }
}

#[test]
fn nobody_flies_a_watched_scenario_and_reset_changes_nothing_but_pause_stops_it() {
    let path = scenario("flight-controller/arming-waits-for-the-escs-after-reset.toml");
    let watched = open_scenario(&repo_folder(), &path).unwrap();
    let plan = watched.plan;
    let mut flight = watched.flight;
    let mut pilot = Pilot::without_input_thread(
        &plan,
        common::packs()
            .input_devices()
            .into_iter()
            .cloned()
            .collect(),
    )
    .watching();
    let mut game_clock = Time::<Virtual>::default();
    let (mut ticks, mut real) = (0_u64, Duration::ZERO);
    let mut frame = |ms: u64, events: Vec<Raw>, flight: &mut Flight, pilot: &mut Pilot| {
        let at = Duration::from_millis(ms);
        if !game_clock.is_paused() {
            game_clock.advance_by(at - real);
        }
        real = at;
        let batches = if events.is_empty() {
            Vec::new()
        } else {
            vec![Batch { at, events }]
        };
        let mut world = FlightWorld {
            flight,
            plan: &plan,
            fixed_clock: Duration::from_millis(ticks),
            game_clock: &mut game_clock,
        };
        pilot.read(batches, at, &mut world);
        while Duration::from_millis(ticks + 1) <= game_clock.elapsed() {
            flight.tick();
            ticks += 1;
        }
    };
    let press = |button, down| Raw::PadButton {
        device: DUALSENSE,
        button,
        down,
    };
    frame(0, vec![dualsense()], &mut flight, &mut pilot);
    // A flick of a stick, which would start a flight if one were flown.
    frame(
        4,
        vec![Raw::PadAxis {
            device: DUALSENSE,
            axis: PadAxis::RightX,
            value: 30000,
        }],
        &mut flight,
        &mut pilot,
    );
    assert_eq!(pilot.flying_name(), None, "no Input Device flies");
    // Reset (Create) does nothing to a watched Scenario.
    frame(
        8,
        vec![press(PadButton::Back, true)],
        &mut flight,
        &mut pilot,
    );
    frame(
        12,
        vec![press(PadButton::Back, false)],
        &mut flight,
        &mut pilot,
    );
    // Pause (Options) stops it, and a second press goes on.
    frame(
        100,
        vec![press(PadButton::Start, true)],
        &mut flight,
        &mut pilot,
    );
    frame(
        104,
        vec![press(PadButton::Start, false)],
        &mut flight,
        &mut pilot,
    );
    let paused_at = flight.simulation().time();
    frame(500, vec![], &mut flight, &mut pilot);
    assert_eq!(flight.simulation().time(), paused_at, "paused");
    frame(
        504,
        vec![press(PadButton::Start, true)],
        &mut flight,
        &mut pilot,
    );
    frame(
        508,
        vec![press(PadButton::Start, false)],
        &mut flight,
        &mut pilot,
    );
    frame(600, vec![], &mut flight, &mut pilot);
    assert!(flight.simulation().time() > paused_at, "flying on");
    // Run on to the end of the Scenario: nothing the pilot did changed it.
    let mut ms = 600;
    while !flight.finished() {
        ms += 1000;
        frame(ms, vec![], &mut flight, &mut pilot);
    }
    assert_eq!(flight.run_fingerprint(), Some(runner_fingerprint(&path)));
}

#[test]
fn save_flight_in_a_watched_scenario_has_no_flight_to_save_and_says_so() {
    // Basis: Rule. Only a pilot's flight can be saved: a watched Scenario's
    // flight keeps nothing for Save Flight.
    let path = scenario("flight-controller/arming-waits-for-the-escs-after-reset.toml");
    let watched = open_scenario(&repo_folder(), &path).unwrap();
    assert!(watched.flight.kept().is_none());
    let problem =
        opendrone::save::flight_files(&watched.plan, &watched.flight, "flight.csv").unwrap_err();
    assert!(problem.contains("no flight to save"), "{problem}");
}
