//! Readable checks on Virtual Switches and keyboard keys (#19 §5,
//! ADR-0017). Basis: Rule.

mod common;

use std::time::Duration;

use common::{built_in_profiles, profile};
use opendrone_input::channels::{HIGH_US, LOW_US};
use opendrone_input::controls::{Key, PadButton};
use opendrone_input::profile::{
    FlightMode, FlightModeSwitch, OnOffSwitch, Press, PressStyle, Setup,
};
use opendrone_input::raw::DeviceState;
use opendrone_input::{
    Batch, Channels, DeviceId, DeviceInfo, InputEvent, Inputs, PilotCopy, Raw, SdlId,
};

const PAD: SdlId = SdlId(3);

fn dualsense() -> DeviceInfo {
    DeviceInfo {
        name: "DualSense Wireless Controller".into(),
        usb_vendor: 0x054C,
        usb_product: 0x0CE6,
        sdl_gamepad: true,
        connection: None,
        heartbeat: false,
    }
}

fn plugged_in(copy: Option<PilotCopy>) -> Inputs {
    let mut inputs = Inputs::new(built_in_profiles());
    if let Some(copy) = copy {
        inputs.set_copy(dualsense().model(), Some(copy), Duration::ZERO);
    }
    inputs.take(Batch {
        at: Duration::ZERO,
        events: vec![Raw::Added {
            device: PAD,
            info: dualsense(),
            state: DeviceState::new(6, 13, true),
        }],
    });
    inputs
}

fn button(inputs: &mut Inputs, button: PadButton, down: bool, at_ms: u64) -> Option<Channels> {
    let events = inputs.take(Batch {
        at: Duration::from_millis(at_ms),
        events: vec![Raw::PadButton {
            device: PAD,
            button,
            down,
        }],
    });
    last_channels(&events)
}

fn last_channels(events: &[InputEvent]) -> Option<Channels> {
    events.iter().rev().find_map(|e| match e {
        InputEvent::Channels { channels, .. } => Some(*channels),
        _ => None,
    })
}

fn now(inputs: &Inputs) -> Channels {
    inputs.devices()[0].channels()
}

#[test]
fn the_arm_button_toggles_on_each_press() {
    let mut inputs = plugged_in(None);
    assert_eq!(now(&inputs).arm, Some(LOW_US));
    button(&mut inputs, PadButton::RightShoulder, true, 10);
    assert_eq!(now(&inputs).arm, Some(HIGH_US));
    button(&mut inputs, PadButton::RightShoulder, false, 20);
    assert_eq!(now(&inputs).arm, Some(HIGH_US), "letting go keeps it armed");
    button(&mut inputs, PadButton::RightShoulder, true, 30);
    assert_eq!(now(&inputs).arm, Some(LOW_US), "the next press disarms");
}

#[test]
fn the_arm_toggle_turns_itself_off_whenever_the_game_reports_a_disarm() {
    let mut inputs = plugged_in(None);
    button(&mut inputs, PadButton::RightShoulder, true, 10);
    button(&mut inputs, PadButton::RightShoulder, false, 20);
    let events = inputs.disarmed(Duration::from_millis(500));
    assert_eq!(last_channels(&events).unwrap().arm, Some(LOW_US));
    // A real off-then-on reaches the Flight Controller on the next press.
    assert_eq!(
        button(&mut inputs, PadButton::RightShoulder, true, 600)
            .unwrap()
            .arm,
        Some(HIGH_US)
    );
    // A disarm while already off changes nothing.
    button(&mut inputs, PadButton::RightShoulder, false, 610);
    inputs.disarmed(Duration::from_millis(700));
    assert!(inputs.disarmed(Duration::from_millis(800)).is_empty());
}

#[test]
fn crash_flip_is_on_only_while_held() {
    let mut inputs = plugged_in(None);
    assert_eq!(now(&inputs).crash_flip, Some(LOW_US));
    button(&mut inputs, PadButton::LeftShoulder, true, 10);
    assert_eq!(now(&inputs).crash_flip, Some(HIGH_US));
    button(&mut inputs, PadButton::LeftShoulder, false, 20);
    assert_eq!(now(&inputs).crash_flip, Some(LOW_US));
}

#[test]
fn the_flight_mode_is_unbound_on_a_gamepad_until_a_button_is_given() {
    // The Flight Mode setting and the Preset decide it then.
    assert_eq!(now(&plugged_in(None)).flight_mode, None);
}

fn with_switches(change: impl FnOnce(&mut opendrone_input::profile::Switches)) -> PilotCopy {
    let mut copy = PilotCopy::of(&profile("opendrone/dualsense"));
    let Setup::Gamepad(setup) = &mut copy.setup else {
        panic!("a Gamepad")
    };
    change(&mut setup.switches);
    copy
}

#[test]
fn a_flight_mode_button_steps_through_its_modes_on_each_press() {
    let copy = with_switches(|s| {
        s.flight_mode = Some(FlightModeSwitch::Steps {
            source: Press::Button(PadButton::North),
            modes: vec![FlightMode::Acro, FlightMode::Horizon, FlightMode::Angle],
        });
    });
    let mut inputs = plugged_in(Some(copy));
    let mut seen = vec![now(&inputs).mode()];
    for i in 0..3 {
        button(&mut inputs, PadButton::North, true, 10 + 20 * i);
        seen.push(now(&inputs).mode());
        button(&mut inputs, PadButton::North, false, 20 + 20 * i);
    }
    assert_eq!(
        seen,
        [
            Some(FlightMode::Acro),
            Some(FlightMode::Horizon),
            Some(FlightMode::Angle),
            Some(FlightMode::Acro)
        ]
    );
}

fn key_armed() -> PilotCopy {
    with_switches(|s| {
        s.arm = Some(OnOffSwitch::Virtual {
            source: Press::Key(Key::named("A").unwrap()),
            style: PressStyle::Toggle,
        });
    })
}

#[test]
fn keys_drive_virtual_switches_only_on_the_flying_input_device() {
    let mut inputs = plugged_in(Some(key_armed()));
    let a = Key::named("A").unwrap();
    // Not yet flying: the key does nothing.
    assert!(
        inputs
            .key(a.clone(), true, Duration::from_millis(10))
            .is_empty()
    );
    inputs.key(a.clone(), false, Duration::from_millis(20));
    inputs.set_flying(Some(DeviceId(0)), Duration::from_millis(30));
    let events = inputs.key(a.clone(), true, Duration::from_millis(40));
    // The key's switch travels in the Flying Input Device's Channels.
    assert!(matches!(
        events.as_slice(),
        [InputEvent::Channels { device: DeviceId(0), channels, .. }] if channels.arm == Some(HIGH_US)
    ));
}

#[test]
fn a_held_key_counts_as_let_go_when_the_window_loses_focus() {
    let copy = with_switches(|s| {
        s.crash_flip = Some(OnOffSwitch::Virtual {
            source: Press::Key(Key::named("F").unwrap()),
            style: PressStyle::Hold,
        });
    });
    let mut inputs = plugged_in(Some(copy));
    inputs.set_flying(Some(DeviceId(0)), Duration::ZERO);
    inputs.key(Key::named("F").unwrap(), true, Duration::from_millis(10));
    assert_eq!(now(&inputs).crash_flip, Some(HIGH_US));
    inputs.focus_lost(Duration::from_millis(20));
    assert_eq!(now(&inputs).crash_flip, Some(LOW_US));
}

#[test]
fn a_bound_key_replaces_the_button_as_the_switchs_one_source() {
    // Each switch Channel has one source at a time: with Arm on a key, R1
    // no longer arms.
    let mut inputs = plugged_in(Some(key_armed()));
    inputs.set_flying(Some(DeviceId(0)), Duration::ZERO);
    button(&mut inputs, PadButton::RightShoulder, true, 10);
    assert_eq!(now(&inputs).arm, Some(LOW_US));
}

#[test]
fn key_names_are_letters_digits_function_keys_and_a_few_named_keys() {
    for name in ["A", "Z", "0", "9", "F1", "F12", "Space", "LeftShift"] {
        assert!(Key::named(name).is_some(), "{name}");
    }
    for name in ["a", "F0", "F13", "F01", "Spacebar", ""] {
        assert!(Key::named(name).is_none(), "{name}");
    }
}

fn plugged_in_holding(button: PadButton) -> Inputs {
    let mut inputs = Inputs::new(built_in_profiles());
    let mut state = DeviceState::new(6, 13, true);
    state.pad.as_mut().unwrap().buttons[button.index()] = true;
    inputs.take(Batch {
        at: Duration::ZERO,
        events: vec![Raw::Added {
            device: PAD,
            info: dualsense(),
            state,
        }],
    });
    inputs
}

#[test]
fn a_button_held_while_the_device_is_plugged_in_isnt_a_press() {
    // A switch moves only when its button moves into the bound position.
    let mut inputs = plugged_in_holding(PadButton::RightShoulder);
    assert_eq!(now(&inputs).arm, Some(LOW_US));
    button(&mut inputs, PadButton::RightShoulder, false, 10);
    assert_eq!(now(&inputs).arm, Some(LOW_US));
    button(&mut inputs, PadButton::RightShoulder, true, 20);
    assert_eq!(now(&inputs).arm, Some(HIGH_US), "the first real press arms");
}

#[test]
fn a_held_crash_flip_button_is_on_from_the_moment_it_is_plugged_in() {
    let inputs = plugged_in_holding(PadButton::LeftShoulder);
    assert_eq!(now(&inputs).crash_flip, Some(HIGH_US));
}

#[test]
fn rebinding_arm_to_a_button_already_held_isnt_a_press() {
    let mut inputs = plugged_in_holding(PadButton::South);
    let copy = with_switches(|s| {
        s.arm = Some(OnOffSwitch::Virtual {
            source: Press::Button(PadButton::South),
            style: PressStyle::Toggle,
        });
    });
    inputs.set_copy(dualsense().model(), Some(copy), Duration::from_millis(5));
    assert_eq!(now(&inputs).arm, Some(LOW_US));
}

#[test]
fn a_key_already_held_when_the_flying_input_device_is_picked_isnt_a_press() {
    // The pilot holds the Arm key before pressing FLY: nothing arms until
    // the key is let go and pressed again.
    let mut inputs = plugged_in(Some(key_armed()));
    let a = Key::named("A").unwrap();
    inputs.key(a.clone(), true, Duration::from_millis(10));
    inputs.set_flying(Some(DeviceId(0)), Duration::from_millis(20));
    assert_eq!(now(&inputs).arm, Some(LOW_US));
    inputs.key(a.clone(), false, Duration::from_millis(30));
    assert_eq!(now(&inputs).arm, Some(LOW_US));
    inputs.key(a, true, Duration::from_millis(40));
    assert_eq!(now(&inputs).arm, Some(HIGH_US), "the first real press arms");
}
