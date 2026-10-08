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
fn a_switch_on_in_two_positions_is_refused_because_a_profiles_switch_is_on_in_one() {
    // Basis: Rule (#19 §5: an on/off switch Channel is on in one position).
    assert_eq!(
        switches_from_aux("aux 0 0 0 1300 2100 0 0").unwrap_err(),
        [
            "Arm on AUX1 (CH5) is on at middle and high: a profile's Arm switch is on in one position."
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
