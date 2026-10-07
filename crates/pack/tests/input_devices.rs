//! Readable checks for Input Device profiles, `input-devices/<id>.toml`
//! (#19 §8): the built-in Pack carries the four alpha profiles, and the
//! checker refuses a broken one with its file, line and a plain sentence.
//!
//! Basis: Source for the built-in profiles (#19 §8, the #18 recordings);
//! Rule for the checker (#19 §1–§6, ADR-0017, ADR-0011).

mod common;

use common::{Fixture, line_of, repo};
use opendrone_pack::input_device::controls::{PadButton, Position, Stick};
use opendrone_pack::input_device::{
    Action, ActionSource, Connection, FlightMode, FlightModeSwitch, GamepadThrottle, Kind, Match,
    OnOffSwitch, Press, PressStyle, RadioChannels, Setup, ThrottleZero,
};
use opendrone_pack::{InputDeviceProfile, Packs, read_input_device_file};

fn built_in(id: &str) -> InputDeviceProfile {
    Packs::open(&repo().join("packs"), "packs")
        .unwrap()
        .input_device(id)
        .unwrap_or_else(|p| panic!("{id} should pass the Pack checker:\n{p}"))
}

#[test]
fn the_built_in_pack_carries_the_pocket_dualsense_any_radio_and_any_gamepad_profiles() {
    let packs = Packs::open(&repo().join("packs"), "packs").unwrap();
    assert_eq!(packs.problems().to_string(), "");
    let ids: Vec<&str> = packs
        .input_devices()
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "opendrone/any-gamepad",
            "opendrone/any-radio",
            "opendrone/dualsense",
            "opendrone/radiomaster-pocket"
        ]
    );
}

#[test]
fn the_pocket_profile_holds_19s_layout() {
    let pocket = built_in("opendrone/radiomaster-pocket");
    assert_eq!(pocket.facts.name, "Radiomaster Pocket");
    assert_eq!(
        pocket.facts.matches,
        Match::Device {
            usb_vendor: 0x1209,
            usb_product: 0x4F54,
            product_name: "Radiomaster Pocket Joystick".into()
        }
    );
    assert_eq!(pocket.report_rate(Some(Connection::Usb)), Some(1000.0));
    assert!(!pocket.reports_at_rest(Some(Connection::Usb)));
    let Setup::Radio(setup) = &pocket.setup else {
        panic!("the Pocket is a Radio")
    };
    assert_eq!(setup.channels, RadioChannels::aetr());
    assert_eq!(
        setup.switches.arm,
        Some(OnOffSwitch::Channel {
            channel: 5,
            on: Position::High
        })
    );
    assert_eq!(
        setup.switches.flight_mode,
        Some(FlightModeSwitch::Channel {
            channel: 6,
            low: FlightMode::Acro,
            middle: FlightMode::Horizon,
            high: FlightMode::Angle
        })
    );
    assert_eq!(
        setup.switches.crash_flip,
        Some(OnOffSwitch::Channel {
            channel: 7,
            on: Position::High
        })
    );
    assert_eq!(
        setup.actions.get(&Action::Reset),
        Some(&ActionSource::Channel {
            channel: 9,
            on: Position::Pressed
        })
    );
    assert_eq!(
        setup.actions.get(&Action::Pause),
        None,
        "setup offers a spare switch"
    );
    // "Trust the radio": −100 %, 0 %, +100 %, no deadband.
    assert_eq!(setup.calibration.roll.min, -32768.0);
    assert_eq!(setup.calibration.roll.centre, 0.0);
    assert_eq!(setup.calibration.roll.max, 32767.0);
    assert_eq!(setup.calibration.roll.deadband, 0.0);
}

#[test]
fn any_radio_has_the_pockets_layout_but_no_report_rate_or_actions() {
    let any = built_in("opendrone/any-radio");
    let pocket = built_in("opendrone/radiomaster-pocket");
    assert_eq!(any.facts.matches, Match::Any);
    assert!(any.facts.report_rate.is_empty());
    let (Setup::Radio(any), Setup::Radio(pocket)) = (any.setup, pocket.setup) else {
        panic!("both are Radios")
    };
    assert_eq!(any.channels, pocket.channels);
    assert_eq!(any.switches, pocket.switches);
    assert_eq!(any.calibration, pocket.calibration);
    assert!(any.actions.is_empty());
}

#[test]
fn the_dualsense_profile_holds_19s_layout() {
    let dualsense = built_in("opendrone/dualsense");
    assert_eq!(dualsense.kind(), Kind::Gamepad);
    assert_eq!(dualsense.report_rate(Some(Connection::Usb)), Some(250.0));
    assert!(dualsense.reports_at_rest(Some(Connection::Usb)));
    assert_eq!(dualsense.report_rate(Some(Connection::Bluetooth)), None);
    let Setup::Gamepad(setup) = &dualsense.setup else {
        panic!("the DualSense is a Gamepad")
    };
    assert_eq!(
        setup.channels.throttle,
        GamepadThrottle::Stick {
            stick: Stick::LeftY,
            zero: ThrottleZero::AtRest
        }
    );
    assert_eq!(setup.channels.roll.stick, Stick::RightX);
    assert_eq!(
        setup.switches.arm,
        Some(OnOffSwitch::Virtual {
            source: Press::Button(PadButton::RightShoulder),
            style: PressStyle::Toggle
        })
    );
    assert_eq!(
        setup.switches.crash_flip,
        Some(OnOffSwitch::Virtual {
            source: Press::Button(PadButton::LeftShoulder),
            style: PressStyle::Hold
        })
    );
    assert_eq!(setup.switches.flight_mode, None);
    assert_eq!(
        setup.actions.get(&Action::Pause),
        Some(&ActionSource::Button(PadButton::Start))
    );
    assert_eq!(
        setup.actions.get(&Action::Reset),
        Some(&ActionSource::Button(PadButton::Back))
    );
    // Its own labels name SDL's positions.
    assert_eq!(
        dualsense
            .facts
            .label_of(opendrone_pack::input_device::controls::PadControl::Button(
                PadButton::Start
            )),
        "Options"
    );
    // 5 % of SDL's half travel.
    assert_eq!(setup.calibration.right_x.deadband, 0.05 * 32768.0);
}

#[test]
fn any_gamepad_is_the_dualsenses_layout_by_sdls_positions() {
    let (Setup::Gamepad(any), Setup::Gamepad(dualsense)) = (
        built_in("opendrone/any-gamepad").setup,
        built_in("opendrone/dualsense").setup,
    ) else {
        panic!("both are Gamepads")
    };
    assert_eq!(any, dualsense);
    assert_eq!(built_in("opendrone/any-gamepad").facts.matches, Match::Any);
}

const FILE: &str = "packs/test/input-devices/radio.toml";

/// The Pocket's profile with one change.
fn pocket_with(old: &str, new: &str) -> (String, Vec<String>) {
    let text = std::fs::read_to_string(
        repo().join("packs/opendrone/input-devices/radiomaster-pocket.toml"),
    )
    .unwrap();
    assert_eq!(text.matches(old).count(), 1, "{old:?} should be there once");
    let text = text.replacen(old, new, 1);
    (text.clone(), problems(&text))
}

/// The DualSense's profile with one change.
fn dualsense_with(old: &str, new: &str) -> (String, Vec<String>) {
    let text = std::fs::read_to_string(repo().join("packs/opendrone/input-devices/dualsense.toml"))
        .unwrap();
    assert_eq!(text.matches(old).count(), 1, "{old:?} should be there once");
    let text = text.replacen(old, new, 1);
    (text.clone(), problems(&text))
}

fn problems(text: &str) -> Vec<String> {
    match read_input_device_file("test/radio", FILE, text) {
        Ok(_) => Vec::new(),
        Err(problems) => problems.0.iter().map(ToString::to_string).collect(),
    }
}

/// Exactly one problem, on the line holding `needle`, containing `words`.
fn one_problem(text: &str, found: &[String], needle: &str, words: &str) {
    let line = line_of(text, needle);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with(&format!("{FILE} line {line}: ")),
        "expected line {line}: {}",
        found[0]
    );
    assert!(found[0].contains(words), "{}", found[0]);
}

#[test]
fn a_kind_other_than_radio_or_gamepad_is_refused() {
    let (text, found) = pocket_with("kind   = \"Radio\"", "kind   = \"Joystick\"");
    one_problem(&text, &found, "Joystick", "write \"Radio\" or \"Gamepad\"");
}

#[test]
fn a_usb_id_is_four_hex_digits() {
    let (text, found) = pocket_with("usb_vendor = \"1209\"", "usb_vendor = \"12G9\"");
    one_problem(&text, &found, "12G9", "four hex digits");
}

#[test]
fn a_switch_has_one_source_at_a_time() {
    let (text, found) = pocket_with(
        "arm         = { channel = 5, on = \"high\" }",
        "arm         = { channel = 5, key = \"A\", on = \"high\" }",
    );
    one_problem(&text, &found, "arm ", "one source at a time");
}

#[test]
fn a_radio_switch_cant_name_a_gamepad_button() {
    let (text, found) = pocket_with(
        "arm         = { channel = 5, on = \"high\" }",
        "arm         = { button = \"R1\", press = \"toggle\" }",
    );
    one_problem(&text, &found, "arm ", "can't use a `button`");
}

#[test]
fn a_keyboard_key_can_drive_a_radios_switch() {
    let (_, found) = pocket_with(
        "arm         = { channel = 5, on = \"high\" }",
        "arm         = { key = \"Space\", press = \"toggle\" }",
    );
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_key_must_be_one_opendrone_knows() {
    let (text, found) = pocket_with(
        "arm         = { channel = 5, on = \"high\" }",
        "arm         = { key = \"space bar\", press = \"toggle\" }",
    );
    one_problem(&text, &found, "space bar", "isn't a key OpenDrone knows");
}

#[test]
fn two_sticks_cant_share_a_channel() {
    let (text, found) = pocket_with("pitch    = { channel = 2 }", "pitch    = { channel = 1 }");
    one_problem(&text, &found, "[channels]", "CH1 carries two sticks");
}

#[test]
fn a_switch_cant_use_a_sticks_channel() {
    let (text, found) = pocket_with(
        "crash_flip  = { channel = 7, on = \"high\" }",
        "crash_flip  = { channel = 4, on = \"high\" }",
    );
    one_problem(&text, &found, "crash_flip ", "CH4 carries a stick");
}

#[test]
fn an_action_cant_use_a_channel_a_switch_already_uses() {
    let (text, found) = pocket_with(
        "reset = { channel = 9, on = \"pressed\" }",
        "reset = { channel = 5, on = \"low\" }",
    );
    one_problem(&text, &found, "reset =", "already drives a switch");
}

#[test]
fn a_switch_position_must_fit_its_channel() {
    let (text, found) = pocket_with(
        "arm         = { channel = 5, on = \"high\" }",
        "arm         = { channel = 5, on = \"pressed\" }",
    );
    one_problem(&text, &found, "pressed", "CH1–CH8 are read by range");
    let (text, found) = pocket_with(
        "reset = { channel = 9, on = \"pressed\" }",
        "reset = { channel = 9, on = \"high\" }",
    );
    one_problem(&text, &found, "reset =", "CH9–CH32 are buttons");
}

#[test]
fn a_flight_mode_switch_needs_a_switch_on_ch1_to_ch8() {
    let (text, found) = pocket_with(
        "flight_mode = { channel = 6, low = \"Acro\", middle = \"Horizon\", high = \"Angle\" }",
        "flight_mode = { channel = 10, low = \"Acro\", middle = \"Horizon\", high = \"Angle\" }",
    );
    one_problem(&text, &found, "flight_mode =", "CH10 is a button");
}

#[test]
fn an_unknown_flight_mode_or_action_is_refused() {
    let (text, found) = pocket_with("middle = \"Horizon\"", "middle = \"Level\"");
    one_problem(&text, &found, "Level", "isn't a Flight Mode");
    let (text, found) = pocket_with("reset = { channel", "fly = { channel");
    one_problem(&text, &found, "fly =", "isn't an Action OpenDrone knows");
}

#[test]
fn a_calibration_centre_lies_between_its_ends_and_its_deadband_fits() {
    let (text, found) = pocket_with(
        "roll     = { min = \"-100 %\", centre = \"0 %\"",
        "roll     = { min = \"-100 %\", centre = \"-100 %\"",
    );
    one_problem(
        &text,
        &found,
        "centre = \"-100 %\"",
        "centre must lie between its min and max",
    );
    let (text, found) = pocket_with(
        "yaw      = { min = \"-100 %\", centre = \"0 %\", max = \"+100 %\", deadband = \"0 %\" }",
        "yaw      = { min = \"-100 %\", centre = \"0 %\", max = \"+100 %\", deadband = \"100 %\" }",
    );
    one_problem(&text, &found, "deadband = \"100 %\"", "deadband is wider");
    let (text, found) = pocket_with(
        "throttle = { min = \"-100 %\"",
        "throttle = { min = \"-120 %\"",
    );
    one_problem(&text, &found, "-120 %", "from -100 % to +100 %");
}

#[test]
fn a_gamepad_button_is_one_of_its_labels_or_sdls_positions() {
    let (text, found) = dualsense_with("button = \"R1\"", "button = \"R5\"");
    one_problem(&text, &found, "R5", "isn't a button on this Gamepad");
    let (_, found) = dualsense_with("button = \"R1\"", "button = \"right_shoulder\"");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_trigger_can_carry_the_throttle_but_never_a_switch() {
    let (_, found) = dualsense_with(
        "throttle = { stick = \"left_y\", zero = \"at rest\" }",
        "throttle = { trigger = \"R2\" }",
    );
    assert_eq!(found, Vec::<String>::new());
    let (text, found) = dualsense_with("button = \"R1\"", "button = \"R2\"");
    one_problem(
        &text,
        &found,
        "arm        = { button",
        "is a trigger, not a button",
    );
}

#[test]
fn a_gamepad_throttle_reads_zero_at_rest_or_at_bottom() {
    let (_, found) = dualsense_with("zero = \"at rest\"", "zero = \"at bottom\"");
    assert_eq!(found, Vec::<String>::new());
    let (text, found) = dualsense_with("zero = \"at rest\"", "zero = \"in the middle\"");
    one_problem(
        &text,
        &found,
        "in the middle",
        "write \"at rest\" or \"at bottom\"",
    );
}

#[test]
fn only_a_gamepad_profile_has_labels() {
    let (text, found) = pocket_with("[channels]", "[labels]\nSE = \"south\"\n\n[channels]");
    one_problem(
        &text,
        &found,
        "[labels]",
        "only a Gamepad profile has [labels]",
    );
}

#[test]
fn a_flight_mode_button_steps_through_at_least_two_modes() {
    let (_, found) = dualsense_with(
        "crash_flip = { button = \"L1\", press = \"hold\" }",
        "crash_flip = { button = \"L1\", press = \"hold\" }\nflight_mode = { button = \"Triangle\" }",
    );
    assert_eq!(found, Vec::<String>::new(), "Acro and Angle by default");
    let (text, found) = dualsense_with(
        "crash_flip = { button = \"L1\", press = \"hold\" }",
        "crash_flip = { button = \"L1\", press = \"hold\" }\nflight_mode = { button = \"Triangle\", modes = [\"Angle\"] }",
    );
    one_problem(&text, &found, "modes = ", "at least two");
}

#[test]
fn an_unknown_key_is_refused_naming_what_is_read_there() {
    let (text, found) = pocket_with("report_rate     = {", "refresh_rate    = {");
    one_problem(
        &text,
        &found,
        "refresh_rate",
        "isn't something OpenDrone reads",
    );
}

#[test]
fn a_profile_in_a_newer_format_needs_a_newer_opendrone() {
    let (text, found) = pocket_with("format = 1", "format = 2");
    one_problem(&text, &found, "format = 2", "needs a newer OpenDrone");
}

#[test]
fn every_problem_in_a_profile_is_listed_at_once() {
    let text = std::fs::read_to_string(
        repo().join("packs/opendrone/input-devices/radiomaster-pocket.toml"),
    )
    .unwrap()
    .replace(
        "kind   = \"Radio\"",
        "kind   = \"Radio\"\ncolour = \"black\"",
    )
    .replace("usb_product = \"4F54\"", "usb_product = \"4F5\"")
    .replace("middle = \"Horizon\"", "middle = \"Level\"")
    .replace("deadband = \"0 %\" }\nyaw", "deadband = \"-5 %\" }\nyaw");
    let found = problems(&text);
    assert_eq!(found.len(), 4, "{found:#?}");
}

#[test]
fn the_pack_checker_reads_input_devices_and_skips_a_broken_one() {
    let fixture = Fixture::new("input-devices");
    let pocket = std::fs::read_to_string(
        repo().join("packs/opendrone/input-devices/radiomaster-pocket.toml"),
    )
    .unwrap();
    fixture.write("packs/fixture/input-devices/pocket.toml", &pocket);
    fixture.write(
        "packs/fixture/input-devices/broken.toml",
        &pocket.replace("kind   = \"Radio\"", "kind   = \"Joystick\""),
    );
    fixture.write("packs/fixture/input-devices/notes.txt", "notes");
    fixture.write("packs/fixture/input-devices/My Radio.toml", &pocket);
    let packs = fixture.packs();
    let ids: Vec<&str> = packs
        .input_devices()
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, ["fixture/pocket"]);
    let found = fixture.problems();
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("packs/fixture/input-devices/broken.toml line 6:"))
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("notes.txt: only Input Device profiles"))
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("My Radio.toml: the file's name is the profile's id"))
    );
    assert!(packs.input_device("fixture/broken").is_err());
    let missing = packs
        .input_device("fixture/missing")
        .unwrap_err()
        .to_string();
    assert!(
        missing.contains("input-devices/missing.toml doesn't exist"),
        "{missing}"
    );
}
