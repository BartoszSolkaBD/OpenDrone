//! Readable checks on telling devices apart, matching them to profiles, the
//! pilot's copy, and devices found, lost and back.
//!
//! Basis: Rule, from #19 §1–§2 and the Input deep dive. Device names are the
//! ones SDL gave in #18, and EdgeTX's USB strings from #19's research.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{built_in_profiles, profile};
use opendrone_input::profile::{Connection, Setup};
use opendrone_input::raw::DeviceState;
use opendrone_input::{
    Batch, DeviceId, DeviceInfo, InputEvent, Inputs, Kind, Lost, PilotCopy, Raw, SdlId,
    find_profile, kind_of,
};

fn device(name: &str, vendor: u16, product: u16, gamepad: bool) -> DeviceInfo {
    DeviceInfo {
        name: name.to_string(),
        usb_vendor: vendor,
        usb_product: product,
        sdl_gamepad: gamepad,
        connection: Some(Connection::Usb),
        heartbeat: gamepad,
    }
}

fn pocket(name: &str) -> DeviceInfo {
    device(name, 0x1209, 0x4F54, false)
}

fn profile_for(info: &DeviceInfo) -> String {
    find_profile(info, &built_in_profiles())
        .expect("a profile fits")
        .id
        .clone()
}

#[test]
fn every_edgetx_radio_is_a_radio_even_where_sdl_reads_it_as_a_gamepad() {
    // On Linux SDL builds a gamepad mapping for the Pocket by itself (#19).
    let on_linux = device("EdgeTX Radiomaster Pocket Joystick", 0x1209, 0x4F54, true);
    assert_eq!(kind_of(&on_linux), Kind::Radio);
}

#[test]
fn a_device_sdl_reads_as_a_gamepad_is_a_gamepad_and_anything_else_a_radio() {
    let xbox = device("Xbox Wireless Controller", 0x045E, 0x0B13, true);
    let stick = device("Some Flight Stick", 0x1234, 0x5678, false);
    assert_eq!(kind_of(&xbox), Kind::Gamepad);
    assert_eq!(kind_of(&stick), Kind::Radio);
}

#[test]
fn the_pocket_is_matched_by_its_ids_and_its_product_name_anywhere_ignoring_case() {
    for name in [
        "EdgeTX Radiomaster Pocket Joystick", // as SDL named it in #18
        "OpenTX Radiomaster Pocket Joystick", // EdgeTX's manufacturer string
        "OpenTX RadioMaster Pocket Joystick", // EdgeTX after 2.12.4
        "Radiomaster Pocket Joystick",
    ] {
        assert_eq!(
            profile_for(&pocket(name)),
            "opendrone/radiomaster-pocket",
            "{name}"
        );
    }
}

#[test]
fn the_dualsense_is_matched_by_its_ids_and_name() {
    let dualsense = device("DualSense Wireless Controller", 0x054C, 0x0CE6, true);
    assert_eq!(profile_for(&dualsense), "opendrone/dualsense");
}

#[test]
fn another_edgetx_radio_falls_back_to_any_radio() {
    assert_eq!(
        profile_for(&pocket("OpenTX Jumper T-Pro Joystick")),
        "opendrone/any-radio"
    );
}

#[test]
fn the_same_name_with_other_ids_isnt_the_pocket() {
    let impostor = device("Radiomaster Pocket Joystick", 0x1234, 0x4F54, false);
    assert_eq!(profile_for(&impostor), "opendrone/any-radio");
}

#[test]
fn an_unknown_gamepad_falls_back_to_any_gamepad_and_an_unknown_joystick_to_any_radio() {
    let xbox = device("Xbox Wireless Controller", 0x045E, 0x0B13, true);
    let stick = device("Some Flight Stick", 0x1234, 0x5678, false);
    assert_eq!(profile_for(&xbox), "opendrone/any-gamepad");
    assert_eq!(profile_for(&stick), "opendrone/any-radio");
}

#[test]
fn the_fallbacks_carry_no_report_rate_for_setup_to_measure() {
    for id in ["opendrone/any-radio", "opendrone/any-gamepad"] {
        assert!(profile(id).facts.report_rate.is_empty(), "{id}");
    }
    assert_eq!(
        profile("opendrone/radiomaster-pocket").report_rate(Some(Connection::Usb)),
        Some(1000.0)
    );
    assert_eq!(
        profile("opendrone/dualsense").report_rate(Some(Connection::Usb)),
        Some(250.0)
    );
    assert_eq!(
        profile("opendrone/dualsense").report_rate(Some(Connection::Bluetooth)),
        None
    );
}

#[test]
fn a_pilots_copy_rewrites_only_the_setup_and_a_report_rate_the_pack_lacks() {
    let pack = profile("opendrone/any-radio");
    let mut copy = PilotCopy::of(&pack);
    let Setup::Radio(setup) = &mut copy.setup else {
        panic!("a Radio")
    };
    setup.channels.roll.reverse = true;
    copy.measured_report_rate = BTreeMap::from([(Connection::Usb, 500.0)]);
    let flown = copy.laid_over(&pack);
    assert_eq!(flown.setup, copy.setup);
    assert_eq!(flown.facts.name, pack.facts.name);
    assert_eq!(flown.facts.matches, pack.facts.matches);
    assert_eq!(flown.report_rate(Some(Connection::Usb)), Some(500.0));

    // Where the Pack has a Report Rate, it wins: the device's facts always
    // come from the current Pack.
    let pocket = profile("opendrone/radiomaster-pocket");
    let mut copy = PilotCopy::of(&pocket);
    copy.measured_report_rate = BTreeMap::from([(Connection::Usb, 250.0)]);
    assert_eq!(
        copy.laid_over(&pocket).report_rate(Some(Connection::Usb)),
        Some(1000.0)
    );
}

#[test]
fn a_pilots_copy_can_turn_a_radio_into_a_gamepad() {
    let pack = profile("opendrone/any-radio");
    let copy = PilotCopy {
        setup: profile("opendrone/any-gamepad").setup,
        calibrated: false,
        measured_report_rate: BTreeMap::new(),
    };
    assert_eq!(copy.laid_over(&pack).kind(), Kind::Gamepad);
}

fn added(sdl: u32, info: &DeviceInfo, at: f64) -> Batch {
    Batch {
        at: Duration::from_secs_f64(at),
        events: vec![Raw::Added {
            device: SdlId(sdl),
            info: info.clone(),
            state: DeviceState::new(8, 24, false),
        }],
    }
}

fn removed(sdl: u32, at: f64) -> Batch {
    Batch {
        at: Duration::from_secs_f64(at),
        events: vec![Raw::Removed { device: SdlId(sdl) }],
    }
}

#[test]
fn a_device_unplugged_and_plugged_back_in_comes_back_under_its_own_number() {
    let mut inputs = Inputs::new(built_in_profiles());
    let info = pocket("EdgeTX Radiomaster Pocket Joystick");
    let found = inputs.take(added(1, &info, 1.0));
    assert!(matches!(
        found[0],
        InputEvent::Found {
            device: DeviceId(0),
            ..
        }
    ));
    assert_eq!(
        inputs.take(removed(1, 2.0)),
        [InputEvent::Lost {
            device: DeviceId(0),
            at: Duration::from_secs(2),
            why: Lost::Unplugged
        }]
    );
    assert_eq!(inputs.devices()[0].lost(), Some(Lost::Unplugged));
    // SDL gives it a new number when it comes back; ours stays.
    let back = inputs.take(added(7, &info, 3.0));
    assert_eq!(
        back[0],
        InputEvent::Back {
            device: DeviceId(0),
            at: Duration::from_secs(3)
        }
    );
    assert_eq!(inputs.devices().len(), 1);
    assert_eq!(inputs.devices()[0].lost(), None);
}

#[test]
fn a_second_device_is_found_under_a_number_of_its_own() {
    let mut inputs = Inputs::new(built_in_profiles());
    inputs.take(added(1, &pocket("EdgeTX Radiomaster Pocket Joystick"), 1.0));
    let second = inputs.take(added(
        2,
        &device("DualSense Wireless Controller", 0x054C, 0x0CE6, true),
        1.5,
    ));
    assert!(matches!(
        &second[0],
        InputEvent::Found { device: DeviceId(1), kind: Kind::Gamepad, profile: Some(p), .. } if p == "opendrone/dualsense"
    ));
}

#[test]
fn reset_to_the_packs_starting_profile_drops_the_pilots_copy() {
    let mut inputs = Inputs::new(built_in_profiles());
    let info = pocket("EdgeTX Radiomaster Pocket Joystick");
    inputs.take(added(1, &info, 1.0));
    let pack = profile("opendrone/radiomaster-pocket");
    let mut copy = PilotCopy::of(&pack);
    copy.calibrated = true;
    let Setup::Radio(setup) = &mut copy.setup else {
        panic!("a Radio")
    };
    setup.channels.roll.reverse = true;
    inputs.set_copy(info.model(), Some(copy), Duration::from_secs(2));
    assert!(inputs.devices()[0].calibrated());
    assert_ne!(inputs.devices()[0].profile().unwrap().setup, pack.setup);
    inputs.set_copy(info.model(), None, Duration::from_secs(3));
    assert!(!inputs.devices()[0].calibrated());
    assert_eq!(inputs.devices()[0].profile().unwrap().setup, pack.setup);
}
