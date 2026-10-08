//! Readable checks for "Paste from Betaflight CLI…" on a Radio's switches
//! (#19 §5, #53): pasted `aux` lines become an Input Device profile's
//! `[switches]`. How the lines are read (each position tested at 988, 1500
//! and 2012 µs, Angle beating Horizon, AND logic and links refused) is
//! checked in `opendrone-flight-controller`'s `aux_paste` checks. Basis:
//! Source (#19 §5 and the maintainer's real exports) unless said.

mod common;

use std::fs;

use common::repo;
use opendrone_pack::Packs;
use opendrone_pack::input_device::controls::Position;
use opendrone_pack::input_device::{
    FlightMode, FlightModeSwitch, OnOffSwitch, Switches, switches_from_aux,
};

fn export(name: &str) -> String {
    fs::read_to_string(repo().join("docs/research/quad-settings").join(name)).unwrap()
}

fn pocket() -> Switches {
    Packs::open(&repo().join("packs"), "packs")
        .unwrap()
        .input_device("opendrone/radiomaster-pocket")
        .unwrap_or_else(|p| panic!("the Pocket should pass the Pack checker:\n{p}"))
        .setup
        .switches()
        .clone()
}

#[test]
fn the_meteors_aux_lines_give_exactly_the_pockets_starting_layout() {
    // Basis: Source (#19 §5: "The Meteor's lines give exactly the starting
    // layout": Arm CH5 high, Flight Mode CH6 low Acro, middle Horizon, high
    // Angle, Crash Flip CH7 high).
    let pasted = switches_from_aux(&export("meteor65-pro.diff-all.txt")).unwrap();
    assert_eq!(pasted.switches, pocket());
    assert!(pasted.ignored.is_empty());
}

#[test]
fn the_cetus_xs_aux_lines_put_angle_at_the_bottom_of_ch6_and_ignore_its_beeper() {
    let pasted = switches_from_aux(&export("cetus-x.diff-all.txt")).unwrap();
    assert_eq!(
        pasted.switches.flight_mode,
        Some(FlightModeSwitch::Channel {
            channel: 6,
            low: FlightMode::Angle,
            middle: FlightMode::Horizon,
            high: FlightMode::Acro,
        })
    );
    assert_eq!(pasted.switches.arm, pocket().arm);
    assert_eq!(pasted.ignored, ["BEEPER (aux 3 13 3 1700 2100 0 0)"]);
}

#[test]
fn a_switch_on_at_middle_and_high_is_read_as_on_at_high_as_a_2_position_switch_sends() {
    // Basis: Rule. A profile's on/off switch is on in one position (#19 §8),
    // and a 2-position switch sends only its ends (988 and 2012 µs), so ARM
    // at 1500–2100 µs, a common range, arms at high.
    let pasted = switches_from_aux("aux 0 0 0 1500 2100 0 0").unwrap();
    assert_eq!(
        pasted.switches.arm,
        Some(OnOffSwitch::Channel {
            channel: 5,
            on: Position::High
        })
    );
    assert_eq!(
        pasted.notes,
        [
            "Arm on AUX1 (CH5) is on at middle and high: a profile's switch is on in one position, so OpenDrone reads it as on at high, as a 2-position switch sends."
        ]
    );
    // And low and middle as low.
    let low = switches_from_aux("aux 0 35 2 900 1700 0 0").unwrap();
    assert_eq!(
        low.switches.crash_flip,
        Some(OnOffSwitch::Channel {
            channel: 7,
            on: Position::Low
        })
    );
    // The Meteor's switches each sit in one position, so nothing is noted.
    assert!(
        switches_from_aux(&export("meteor65-pro.diff-all.txt"))
            .unwrap()
            .notes
            .is_empty()
    );
}

#[test]
fn a_switch_on_at_every_position_or_at_both_ends_is_refused() {
    // Basis: Rule (#19 §8: a profile's on/off switch is on in one position).
    assert_eq!(
        switches_from_aux("aux 0 0 0 900 2100 0 0").unwrap_err(),
        [
            "Arm on AUX1 (CH5) is on at low and middle and high: a profile's switch is on in one position, and no switch reads this as one."
        ]
    );
    assert_eq!(
        switches_from_aux("aux 0 0 0 900 1300 0 0\naux 1 0 0 1700 2100 0 0").unwrap_err(),
        [
            "Arm on AUX1 (CH5) is on at low and high: a profile's switch is on in one position, and no switch reads this as one."
        ]
    );
}

#[test]
fn aux5_and_up_are_ch9_and_up_which_a_radio_sends_as_buttons() {
    // Basis: Rule (#18: EdgeTX sends CH9–CH32 as buttons, so a switch there
    // is on when pressed).
    let high = switches_from_aux("aux 0 0 4 1700 2100 0 0").unwrap();
    assert_eq!(
        high.switches.arm,
        Some(OnOffSwitch::Channel {
            channel: 9,
            on: Position::Pressed
        })
    );
    assert_eq!(
        switches_from_aux("aux 0 0 4 900 1300 0 0").unwrap_err(),
        [
            "Arm on AUX5 is CH9, which a Radio sends as a button, so it can only be on when pressed (high), not low."
        ]
    );
    assert_eq!(
        switches_from_aux("aux 0 1 4 1700 2100 0 0").unwrap_err(),
        [
            "The Flight Mode switch on AUX5 is CH9, a button, but a Flight Mode switch needs a switch on CH1–CH8."
        ]
    );
}

#[test]
fn a_switch_the_paste_doesnt_set_is_left_without_a_source() {
    // Basis: Rule (ADR-0017: with no Flight Mode switch, the pilot's Flight
    // Mode setting drives AUX2).
    let arm_only = switches_from_aux("aux 0 0 0 1700 2100 0 0").unwrap();
    assert_eq!(arm_only.switches.flight_mode, None);
    assert_eq!(arm_only.switches.crash_flip, None);
}
