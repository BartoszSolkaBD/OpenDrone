//! Readable checks for the `aux` paste (#19 §5, ADR-0017, #53): the Arm,
//! Flight Mode and Crash Flip switches read from pasted `aux` lines, each
//! switch position tested at 988, 1500 and 2012 µs as Betaflight does. They
//! read the maintainer's real exports in `docs/research/quad-settings/`. That
//! the Meteor's lines give exactly the Pocket's starting layout is checked
//! in `opendrone-pack`, which holds Input Device profiles. Basis: Rule (#19
//! §5) unless said.

use opendrone_flight_controller::cli::AuxPaste;
use opendrone_flight_controller::cli::aux_paste::{AuxFlightModeSwitch, AuxSwitch, FlightMode};

const METEOR: &str = include_str!("../../../docs/research/quad-settings/meteor65-pro.diff-all.txt");
const CETUS: &str = include_str!("../../../docs/research/quad-settings/cetus-x.diff-all.txt");

use FlightMode::{Acro, Angle, Horizon};

const HIGH: [bool; 3] = [false, false, true];

fn paste(text: &str) -> AuxPaste {
    AuxPaste::read(text).unwrap_or_else(|refusal| panic!("should read:\n{refusal}"))
}

fn refused(text: &str) -> String {
    AuxPaste::read(text)
        .err()
        .unwrap_or_else(|| panic!("should be refused"))
        .to_string()
}

/// The positions a mode on AUX1 is on at, for one pasted range.
fn on_at(from: u16, to: u16) -> [bool; 3] {
    paste(&format!("aux 0 0 0 {from} {to} 0 0"))
        .arm
        .map_or([false; 3], |arm| arm.on)
}

#[test]
fn the_meteors_lines_give_arm_on_aux1_high_and_crash_flip_on_aux3_high() {
    // Basis: Source (the Meteor's diff all: ARM on AUX1 1700–2100, ANGLE on
    // AUX2 1700–2100, HORIZON on AUX2 1300–1700, FLIP OVER AFTER CRASH on
    // AUX3 1700–2100).
    let meteor = paste(METEOR);
    assert_eq!(meteor.arm, Some(AuxSwitch { aux: 1, on: HIGH }));
    assert_eq!(
        meteor.flight_mode,
        Some(AuxFlightModeSwitch {
            aux: 2,
            modes: [Acro, Horizon, Angle]
        })
    );
    assert_eq!(meteor.crash_flip, Some(AuxSwitch { aux: 3, on: HIGH }));
    assert!(meteor.ignored.is_empty());
}

#[test]
fn the_cetus_x_has_angle_at_the_bottom_and_its_beeper_is_ignored() {
    // Basis: Source (the Cetus X's diff all: ANGLE on AUX2 900–1300, a
    // factory setup for beginners, and BEEPER on AUX4).
    let cetus = paste(CETUS);
    assert_eq!(
        cetus.flight_mode.map(|f| f.modes),
        Some([Angle, Horizon, Acro])
    );
    assert_eq!(cetus.arm.map(|a| a.channel()), Some(5));
    assert_eq!(cetus.crash_flip.map(|c| c.channel()), Some(7));
    assert_eq!(cetus.ignored.len(), 1);
    assert_eq!(cetus.ignored[0].mode, "BEEPER");
    assert_eq!(cetus.ignored[0].text, "aux 3 13 3 1700 2100 0 0");
}

#[test]
fn auxn_is_the_radios_ch_n_plus_4() {
    let arm = paste("aux 0 0 3 1700 2100 0 0").arm.unwrap();
    assert_eq!((arm.aux, arm.channel()), (4, 8));
    assert_eq!(paste("aux 0 0 0 1700 2100 0 0").arm.unwrap().channel(), 5);
}

#[test]
fn each_switch_position_is_tested_at_988_1500_and_2012_us_as_betaflight_does() {
    // Basis: Source (2026.6.2's isRangeActive: on from the range's start up
    // to, but not at, its end, both held to 25 µs steps from 900 µs).
    assert_eq!(on_at(1700, 2100), [false, false, true]);
    assert_eq!(on_at(1500, 2100), [false, true, true]);
    assert_eq!(on_at(1525, 2100), [false, false, true]);
    assert_eq!(on_at(900, 1500), [true, false, false]);
    assert_eq!(on_at(975, 1000), [true, false, false]);
    assert_eq!(on_at(1000, 2000), [false, true, false]);
    // 1510 µs is stored as step 24, which is 1500 µs, so the middle counts.
    assert_eq!(on_at(1510, 2100), [false, true, true]);
}

#[test]
fn angle_beats_horizon_and_acro_is_wherever_neither_is_on() {
    let both = paste("aux 0 1 1 1300 2100 0 0\naux 1 2 1 900 2100 0 0");
    assert_eq!(both.flight_mode.unwrap().modes, [Horizon, Angle, Angle]);
    let horizon_only = paste("aux 0 2 1 1300 1700 0 0");
    assert_eq!(
        horizon_only.flight_mode.unwrap().modes,
        [Acro, Horizon, Acro]
    );
    // No Angle or Horizon: no Flight Mode switch, so the pilot's Flight
    // Mode setting decides (ADR-0017).
    assert_eq!(paste("aux 0 0 0 1700 2100 0 0").flight_mode, None);
}

#[test]
fn and_logic_and_linked_modes_are_refused_with_a_note() {
    assert_eq!(
        refused("aux 0 0 0 1700 2100 1 0"),
        "Line 1: ARM uses AND logic, which OpenDrone can't copy: each of Arm, Flight Mode and Crash Flip comes from one switch (ADR-0017)."
    );
    assert_eq!(
        refused("aux 0 0 0 1700 2100 0 0\naux 1 1 0 900 900 0 13"),
        "Line 2: ANGLE is linked to BEEPER, which OpenDrone can't copy: each of Arm, Flight Mode and Crash Flip comes from one switch (ADR-0017)."
    );
    // AND logic on a mode OpenDrone ignores is just ignored.
    assert!(AuxPaste::read("aux 0 0 0 1700 2100 0 0\naux 1 13 3 1700 2100 1 0").is_ok());
}

#[test]
fn other_modes_are_listed_as_ignored() {
    let pasted =
        paste("aux 0 0 0 1700 2100 0 0\naux 1 28 4 1700 2100 0 0\naux 2 36 5 1700 2100 0 0");
    let names: Vec<&str> = pasted.ignored.iter().map(|i| i.mode.as_str()).collect();
    assert_eq!(names, ["AIR MODE", "PREARM"]);
}

#[test]
fn a_switch_the_fixed_layout_cant_hold_is_refused_with_a_note() {
    assert_eq!(
        refused("aux 0 0 0 1700 2100 0 0\naux 1 0 2 1700 2100 0 0"),
        "ARM is on more than one switch (AUX1 and AUX3): OpenDrone reads each of Arm, Flight Mode and Crash Flip from one switch (ADR-0017)."
    );
    assert_eq!(
        refused("aux 0 1 1 1700 2100 0 0\naux 1 2 2 1300 1700 0 0"),
        "Angle and Horizon are on more than one switch (AUX2 and AUX3): OpenDrone reads the Flight Mode from one switch (ADR-0017)."
    );
    assert_eq!(
        refused("aux 0 0 0 1050 1400 0 0"),
        "ARM on AUX1 is on at none of 988, 1500 and 2012 µs, the positions a radio's switch sends."
    );
}

#[test]
fn a_dumps_unused_slots_are_skipped() {
    let dump = "aux 0 0 0 1700 2100 0 0\naux 1 0 0 900 900 0 0\naux 2 0 0 900 900 0 0";
    assert_eq!(paste(dump).arm, Some(AuxSwitch { aux: 1, on: HIGH }));
}

#[test]
fn a_paste_with_no_aux_lines_is_refused() {
    assert!(refused("set p_roll = 40").starts_with("There are no `aux` lines here"));
    assert!(refused("aux 0 0").starts_with("Line 1: `aux 0 0` isn't an `aux` line"));
}
