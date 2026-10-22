//! Readable checks for the screens: what each holds, how a pilot moves
//! between them, and how a Radio's and a Gamepad's sticks move the cursor.
//! Every name is the plain sentence it proves. Basis: Rule, from the Screens
//! and navigation deep dive (docs/context/screens.md), the resolution of #23
//! (variant A) and the resolution of #19 §2 and §7. Times in milliseconds on
//! the computer's clock; Channels in µs, as a receiver hands them over (988
//! low, 1500 centre, 2012 high); Gamepad sticks in SDL's units (±32767).

use std::time::Duration;

use opendrone::screens::menu_input::{MenuInput, PadMenu, RadioMenu};
use opendrone::screens::model::{
    COUNTDOWN_SECONDS, Catalogue, DeviceFacts, Effect, Experience, Facts, ItemId, MapChoice, Menus,
    Pick, QuadChoice, Screen, quad_is_in_the_air,
};
use opendrone::settings_file::{FlyingDevice, PilotSettings, Preset, TextSize};
use opendrone_input::DeviceId;
use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::Kind;
use opendrone_input::raw::PadState;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ---------------------------------------------------------------- Radio

/// A Radio's sticks all at centre.
const CENTRE: f64 = 1500.0;
const FULL_HIGH: f64 = 2012.0;
const FULL_LOW: f64 = 988.0;

#[test]
fn pushing_the_right_stick_up_or_down_moves_the_cursor_up_or_down() {
    // Basis: Rule (#23: the right stick moves). Pitch reads high when the
    // stick is pushed up.
    let mut radio = RadioMenu::new();
    assert_eq!(radio.read(ms(0), CENTRE, CENTRE, CENTRE), vec![]);
    assert_eq!(
        radio.read(ms(20), CENTRE, FULL_HIGH, CENTRE),
        vec![MenuInput::Up]
    );
    assert_eq!(radio.read(ms(40), CENTRE, CENTRE, CENTRE), vec![]);
    assert_eq!(
        radio.read(ms(60), CENTRE, FULL_LOW, CENTRE),
        vec![MenuInput::Down]
    );
}

#[test]
fn pushing_the_right_stick_right_or_left_changes_a_setting_right_or_left() {
    // Basis: Rule (#23: every setting is one stop, changed with left and right).
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(20), FULL_HIGH, CENTRE, CENTRE),
        vec![MenuInput::Right]
    );
    radio.read(ms(40), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(60), FULL_LOW, CENTRE, CENTRE),
        vec![MenuInput::Left]
    );
}

#[test]
fn a_flick_of_yaw_right_selects_and_a_flick_left_goes_back() {
    // Basis: Rule (#23, #19 §7).
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(20), CENTRE, CENTRE, FULL_HIGH),
        vec![MenuInput::Select]
    );
    radio.read(ms(40), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(60), CENTRE, CENTRE, FULL_LOW),
        vec![MenuInput::Back]
    );
}

#[test]
fn a_flick_selects_once_however_long_yaw_is_held() {
    // Basis: Rule: a flick is one press. Yaw must come back near centre
    // before it can flick again.
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(20), CENTRE, CENTRE, FULL_HIGH),
        vec![MenuInput::Select]
    );
    for t in 1..60 {
        assert_eq!(
            radio.read(ms(20 + 20 * t), CENTRE, CENTRE, FULL_HIGH),
            vec![]
        );
    }
    // Back to centre, then another flick.
    assert_eq!(radio.read(ms(1300), CENTRE, CENTRE, CENTRE), vec![]);
    assert_eq!(
        radio.read(ms(1320), CENTRE, CENTRE, FULL_HIGH),
        vec![MenuInput::Select]
    );
}

#[test]
fn a_small_stick_movement_does_nothing() {
    // Basis: Rule: a quarter of full travel (±128 µs) is under the push
    // point, so a resting stick's flicker never moves the cursor.
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(20), 1500.0 + 120.0, 1500.0 - 120.0, 1500.0 + 150.0),
        vec![]
    );
}

#[test]
fn a_held_push_repeats_after_half_a_second_and_then_every_150_ms() {
    // Basis: Rule (menu_input: REPEAT_AFTER 500 ms, REPEAT_EVERY 150 ms).
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    let mut moves_at = Vec::new();
    for t in (20..=1000).step_by(10) {
        if !radio.read(ms(t), CENTRE, FULL_LOW, CENTRE).is_empty() {
            moves_at.push(t);
        }
    }
    assert_eq!(moves_at, vec![20, 520, 670, 820, 970]);
}

#[test]
fn a_stick_already_pushed_when_the_menu_opens_does_nothing_until_it_is_let_go() {
    // Basis: Rule: opening the Pause Menu in a turn never selects what is
    // under the cursor. The yaw is held to the right when the first reading
    // comes.
    let mut radio = RadioMenu::new();
    assert_eq!(radio.read(ms(0), CENTRE, FULL_HIGH, FULL_HIGH), vec![]);
    for t in (20..=2000).step_by(20) {
        assert_eq!(
            radio.read(ms(t), CENTRE, FULL_HIGH, FULL_HIGH),
            vec![],
            "at {t} ms"
        );
    }
    radio.read(ms(2100), CENTRE, CENTRE, CENTRE);
    assert_eq!(
        radio.read(ms(2120), CENTRE, FULL_HIGH, CENTRE),
        vec![MenuInput::Up]
    );
}

// -------------------------------------------------------------- Gamepad

fn pad() -> PadState {
    PadState::at_rest()
}

#[test]
fn the_d_pad_moves_the_cursor_on_a_gamepad() {
    // Basis: Rule (#23: D-pad or left stick).
    let mut menu = PadMenu::new();
    menu.read(ms(0), &pad());
    let mut down = pad();
    down.buttons[PadButton::DpadDown.index()] = true;
    assert_eq!(menu.read(ms(20), &down), vec![MenuInput::Down]);
    assert_eq!(menu.read(ms(40), &pad()), vec![]);
    let mut left = pad();
    left.buttons[PadButton::DpadLeft.index()] = true;
    assert_eq!(menu.read(ms(60), &left), vec![MenuInput::Left]);
}

#[test]
fn the_left_stick_moves_the_cursor_on_a_gamepad() {
    // Basis: Rule (#23). SDL reads a stick pushed down as positive.
    let mut menu = PadMenu::new();
    menu.read(ms(0), &pad());
    let mut up = pad();
    up.axes[PadAxis::LeftY.index()] = -30000;
    assert_eq!(menu.read(ms(20), &up), vec![MenuInput::Up]);
    assert_eq!(menu.read(ms(40), &pad()), vec![]);
    let mut right = pad();
    right.axes[PadAxis::LeftX.index()] = 30000;
    assert_eq!(menu.read(ms(60), &right), vec![MenuInput::Right]);
}

#[test]
fn a_gamepad_rest_flicker_does_not_move_the_cursor() {
    // Basis: Observed (#18, #27): the DualSense's sticks rest 3 to 10 % off
    // centre. 10 % of travel is 3277.
    let mut menu = PadMenu::new();
    menu.read(ms(0), &pad());
    let mut resting = pad();
    resting.axes[PadAxis::LeftX.index()] = 3300;
    resting.axes[PadAxis::LeftY.index()] = -3300;
    assert_eq!(menu.read(ms(20), &resting), vec![]);
}

#[test]
fn cross_selects_circle_goes_back_and_l1_and_r1_switch_tabs() {
    // Basis: Rule (#23): ✕ selects, ○ goes back, L1 and R1 switch tabs.
    let mut menu = PadMenu::new();
    menu.read(ms(0), &pad());
    for (button, wanted) in [
        (PadButton::South, MenuInput::Select),
        (PadButton::East, MenuInput::Back),
        (PadButton::LeftShoulder, MenuInput::PreviousTab),
        (PadButton::RightShoulder, MenuInput::NextTab),
    ] {
        let mut pressed = pad();
        pressed.buttons[button.index()] = true;
        assert_eq!(menu.read(ms(20), &pressed), vec![wanted]);
        // Held: not a second press.
        assert_eq!(menu.read(ms(40), &pressed), vec![]);
        menu.read(ms(60), &pad());
    }
}

#[test]
fn a_button_held_when_the_menu_opens_is_not_a_press() {
    // Basis: Rule: opening a menu with ✕ held selects nothing.
    let mut menu = PadMenu::new();
    let mut held = pad();
    held.buttons[PadButton::South.index()] = true;
    assert_eq!(menu.read(ms(0), &held), vec![]);
    assert_eq!(menu.read(ms(20), &held), vec![]);
}

// ----------------------------------------------------------------- Menus

fn catalogue() -> Catalogue {
    Catalogue {
        maps: vec![
            MapChoice {
                id: "opendrone/bando".to_string(),
                name: "Bando".to_string(),
                description: "A tower".to_string(),
            },
            MapChoice {
                id: "opendrone/skate-park".to_string(),
                name: "Skate Park".to_string(),
                description: "Concrete".to_string(),
            },
        ],
        quads: vec![
            QuadChoice {
                id: "opendrone/freestyle-5".to_string(),
                name: "Freestyle 5″".to_string(),
                description: "A typical 5″ quad".to_string(),
                spec_line: "6S".to_string(),
                picture: None,
            },
            QuadChoice {
                id: "opendrone/whoop-65".to_string(),
                name: "Whoop 65".to_string(),
                description: "A whoop".to_string(),
                spec_line: "1S".to_string(),
                picture: None,
            },
        ],
    }
}

fn dualsense() -> DeviceFacts {
    DeviceFacts {
        name: "DualSense".to_string(),
        profile_name: "DualSense".to_string(),
        kind: Kind::Gamepad,
        arm_bound: true,
    }
}

fn pocket() -> DeviceFacts {
    DeviceFacts {
        name: "EdgeTX Radiomaster Pocket Joystick".to_string(),
        profile_name: "Radiomaster Pocket".to_string(),
        kind: Kind::Radio,
        arm_bound: true,
    }
}

/// What the game knows, for a screen: these devices, nothing flying yet, the
/// Quad on the ground.
struct World {
    catalogue: Catalogue,
    devices: Vec<DeviceFacts>,
    in_the_air: bool,
}

impl World {
    fn with(devices: Vec<DeviceFacts>) -> World {
        World {
            catalogue: catalogue(),
            devices,
            in_the_air: false,
        }
    }

    fn facts(&self) -> Facts<'_> {
        Facts {
            devices: &self.devices,
            catalogue: &self.catalogue,
            flying_with: None,
            in_the_air: self.in_the_air,
        }
    }
}

fn returning() -> PilotSettings {
    PilotSettings {
        first_launch_done: true,
        ..PilotSettings::default()
    }
}

/// Presses `inputs` in order at 0 ms, from the keyboard.
fn press_all(menus: &mut Menus, world: &World, inputs: &[MenuInput]) -> Vec<Effect> {
    let mut effects = Vec::new();
    for input in inputs {
        effects.extend(menus.press(*input, None, &world.facts(), ms(0)));
    }
    effects
}

fn labels(menus: &Menus, world: &World) -> Vec<String> {
    menus
        .items(&world.facts())
        .into_iter()
        .map(|i| i.label)
        .collect()
}

fn cursor_label(menus: &Menus, world: &World) -> String {
    let page = menus.page(ms(0), &world.facts());
    page.items[page.cursor].label.clone()
}

#[test]
fn a_new_install_opens_on_first_launch_and_a_returning_pilot_on_the_hub() {
    let world = World::with(vec![]);
    assert_eq!(
        Menus::new(PilotSettings::default()).screen(),
        Screen::FirstLaunch
    );
    let hub = Menus::new(returning());
    assert_eq!(hub.screen(), Screen::Hub);
    // FLY is selected on arrival.
    assert_eq!(cursor_label(&hub, &world), "FLY");
}

#[test]
fn a_returning_pilot_reaches_the_launch_spot_in_one_press() {
    // Basis: Rule (screens.md: one press from the Hub).
    let world = World::with(vec![dualsense()]);
    let mut hub = Menus::new(returning());
    let effects = press_all(&mut hub, &world, &[MenuInput::Select]);
    assert_eq!(effects, vec![Effect::Load { by: None }]);
    assert_eq!(hub.screen(), Screen::Loading);
}

#[test]
fn the_device_that_pressed_fly_is_the_one_that_flies() {
    // Basis: Rule (#19 §2: the Flying Input Device is the one that pressed FLY).
    let world = World::with(vec![dualsense()]);
    let mut hub = Menus::new(returning());
    let effects = hub.press(MenuInput::Select, Some(DeviceId(7)), &world.facts(), ms(0));
    assert_eq!(
        effects,
        vec![Effect::Load {
            by: Some(DeviceId(7))
        }]
    );
}

#[test]
fn first_launch_has_text_size_and_reduce_motion_first_then_the_question() {
    let world = World::with(vec![]);
    let menus = Menus::new(PilotSettings::default());
    assert_eq!(
        labels(&menus, &world),
        vec![
            "Text size",
            "Reduce motion",
            "No, never",
            "A little: in a sim, or line of sight",
            "Yes, I fly real quads in Acro",
            "Skip"
        ]
    );
    let page = menus.page(ms(0), &world.facts());
    assert!(page.lines[0].starts_with("Flashing images."));
    assert!(page.lines[1].contains("Have you flown FPV before?"));
}

#[test]
fn each_answer_to_have_you_flown_fpv_picks_its_preset() {
    // Basis: Rule (#23: No, never is Beginner; A little is Intermediate; Yes
    // is Pro; Skip is Beginner).
    let world = World::with(vec![]);
    for (answer, preset) in [
        (Experience::NeverFlown, Preset::Beginner),
        (Experience::ALittle, Preset::Intermediate),
        (Experience::Yes, Preset::Pro),
        (Experience::Skip, Preset::Beginner),
    ] {
        let mut menus = Menus::new(PilotSettings::default());
        menus.point_at(ItemId::Answer(answer), &world.facts());
        let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
        assert_eq!(effects, vec![Effect::SettingsChanged]);
        assert_eq!(menus.settings().preset, preset, "{answer:?}");
        assert_eq!(menus.screen(), Screen::InputDeviceStep);
    }
}

#[test]
fn a_new_install_that_skips_setup_reaches_fly_in_three_presses() {
    // Basis: Rule (screens.md: answer the question, Set up later, FLY). The
    // cursor starts on the first answer, so a press is a Select.
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(PilotSettings::default());
    // 1: the answer (No, never).
    press_all(&mut menus, &world, &[MenuInput::Select]);
    // 2: Set up later, one move down from Set up.
    press_all(&mut menus, &world, &[MenuInput::Down, MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Hub);
    assert!(menus.settings().first_launch_done);
    // 3: FLY.
    let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(effects, vec![Effect::Load { by: None }]);
}

#[test]
fn the_input_device_step_offers_set_up_first_and_set_up_later_one_move_away() {
    // Basis: Rule (#19 §7: the selected button is Set up).
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(PilotSettings::default());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(labels(&menus, &world), vec!["Set up", "Set up later"]);
    assert_eq!(cursor_label(&menus, &world), "Set up");
    let page = menus.page(ms(0), &world.facts());
    assert_eq!(page.lines, vec!["Connected: DualSense"]);
}

#[test]
fn with_no_input_device_the_step_says_plug_in_your_radio_or_gamepad() {
    // Basis: Rule (#19 §7).
    let world = World::with(vec![]);
    let mut menus = Menus::new(PilotSettings::default());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    let page = menus.page(ms(0), &world.facts());
    assert_eq!(page.lines, vec!["Plug in your Radio or Gamepad"]);
    assert_eq!(labels(&menus, &world), vec!["Continue without"]);
    let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(effects, vec![Effect::SettingsChanged]);
    assert_eq!(menus.screen(), Screen::Hub);
}

#[test]
fn set_up_goes_on_to_the_hub_and_says_setup_is_not_in_this_version() {
    // Setup is another ticket (#76): the button must not pretend.
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(PilotSettings::default());
    press_all(&mut menus, &world, &[MenuInput::Select, MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Hub);
    let page = menus.page(ms(0), &world.facts());
    assert!(page.notices[0].contains("arrives in a later version"));
}

#[test]
fn text_size_and_reduce_motion_are_one_stop_each_changed_with_left_and_right() {
    // Basis: Rule (screens.md: each setting is one stop).
    let world = World::with(vec![]);
    let mut menus = Menus::new(PilotSettings::default());
    menus.point_at(ItemId::TextSize, &world.facts());
    assert_eq!(
        press_all(&mut menus, &world, &[MenuInput::Right]),
        vec![Effect::SettingsChanged]
    );
    assert_eq!(menus.settings().text_size, TextSize::Large);
    press_all(&mut menus, &world, &[MenuInput::Right]);
    assert_eq!(menus.settings().text_size, TextSize::Larger);
    press_all(&mut menus, &world, &[MenuInput::Left, MenuInput::Left]);
    assert_eq!(menus.settings().text_size, TextSize::Normal);
    menus.point_at(ItemId::ReduceMotion, &world.facts());
    press_all(&mut menus, &world, &[MenuInput::Right]);
    assert!(menus.settings().reduce_motion);
}

#[test]
fn the_hub_has_fly_chips_for_map_quad_and_preset_settings_and_quit() {
    let world = World::with(vec![dualsense()]);
    let hub = Menus::new(returning());
    assert_eq!(
        labels(&hub, &world),
        vec!["FLY", "Map", "Quad", "Preset", "Settings", "Quit"]
    );
    let items = hub.items(&world.facts());
    assert_eq!(items[1].value.as_deref(), Some("Skate Park"));
    assert_eq!(items[2].value.as_deref(), Some("Whoop 65"));
    assert_eq!(items[3].value.as_deref(), Some("Beginner"));
}

#[test]
fn the_map_quad_and_preset_chips_change_in_place_and_are_saved() {
    // Basis: Rule (#23: each changeable in place). Ids are what is saved.
    let world = World::with(vec![dualsense()]);
    let mut hub = Menus::new(returning());
    hub.point_at(ItemId::MapChip, &world.facts());
    assert_eq!(
        press_all(&mut hub, &world, &[MenuInput::Left]),
        vec![Effect::SettingsChanged]
    );
    assert_eq!(hub.settings().map, "opendrone/bando");
    hub.point_at(ItemId::QuadChip, &world.facts());
    press_all(&mut hub, &world, &[MenuInput::Left]);
    assert_eq!(hub.settings().quad, "opendrone/freestyle-5");
    hub.point_at(ItemId::PresetChip, &world.facts());
    press_all(&mut hub, &world, &[MenuInput::Right]);
    assert_eq!(hub.settings().preset, Preset::Intermediate);
    assert_eq!(hub.screen(), Screen::Hub);
}

#[test]
fn the_hub_warns_with_no_input_device_and_never_blocks_fly() {
    // Basis: Rule (screens.md: a Pre-flight Warning never blocks flying).
    let world = World::with(vec![]);
    let mut hub = Menus::new(returning());
    let page = hub.page(ms(0), &world.facts());
    assert_eq!(
        page.warnings,
        vec!["No Input Device connected. Plug in your Radio or Gamepad."]
    );
    assert_eq!(
        press_all(&mut hub, &world, &[MenuInput::Select]),
        vec![Effect::Load { by: None }]
    );
}

#[test]
fn the_hub_warns_when_arm_is_not_bound_unless_auto_arm_is_on() {
    // Basis: Rule (#19 §7, #23: Auto-arm off and Arm not bound).
    let mut unbound = dualsense();
    unbound.arm_bound = false;
    let world = World::with(vec![unbound]);
    let mut hub = Menus::new(returning());
    assert_eq!(
        hub.page(ms(0), &world.facts()).warnings,
        vec!["Arm isn't bound on your DualSense. Turn on Auto-arm in Settings."]
    );
    // Turn Auto-arm on in Settings.
    hub.point_at(ItemId::OpenSettings, &world.facts());
    press_all(&mut hub, &world, &[MenuInput::Select]);
    hub.point_at(ItemId::AutoArm, &world.facts());
    press_all(&mut hub, &world, &[MenuInput::Right, MenuInput::Back]);
    assert_eq!(hub.screen(), Screen::Hub);
    assert!(hub.page(ms(0), &world.facts()).warnings.is_empty());
}

#[test]
fn a_device_with_arm_bound_gives_no_warning() {
    let world = World::with(vec![dualsense()]);
    let hub = Menus::new(returning());
    assert!(hub.page(ms(0), &world.facts()).warnings.is_empty());
}

#[test]
fn the_hub_says_which_device_it_flies_with() {
    // Basis: Rule (#19 §2: "Flying with: …").
    let world = World::with(vec![dualsense()]);
    let hub = Menus::new(returning());
    let facts = Facts {
        flying_with: Some("DualSense"),
        ..world.facts()
    };
    assert_eq!(
        hub.page(ms(0), &facts).flying_with.as_deref(),
        Some("Flying with: DualSense")
    );
}

#[test]
fn a_notice_shows_for_six_seconds_and_then_goes() {
    let world = World::with(vec![dualsense()]);
    let mut hub = Menus::new(returning());
    hub.notice("DualSense connected", ms(1000));
    assert_eq!(
        hub.page(ms(2000), &world.facts()).notices,
        vec!["DualSense connected"]
    );
    hub.tick(ms(7001));
    assert!(hub.page(ms(7001), &world.facts()).notices.is_empty());
}

#[test]
fn the_loading_screen_shows_map_quad_and_preset_and_a_not_bound_arm() {
    // Basis: Rule (#23 §4: the key reminder shows "⚠ not bound" for Arm).
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(returning());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    menus.set_arm_bound(Some(false));
    menus.set_progress(0.4);
    let loading = menus.page(ms(0), &world.facts()).loading.unwrap();
    assert_eq!(loading.map, "Skate Park");
    assert_eq!(loading.quad, "Whoop 65");
    assert_eq!(loading.preset, "Beginner");
    assert_eq!(loading.progress, 0.4);
    let arm = &loading.keys[0];
    assert_eq!(
        (arm.action.as_str(), arm.key.as_str()),
        ("Arm", "not bound")
    );
    assert!(arm.warning);
    let others: Vec<(&str, &str)> = loading.keys[1..]
        .iter()
        .map(|k| (k.action.as_str(), k.key.as_str()))
        .collect();
    assert_eq!(
        others,
        vec![
            ("Reset", "R"),
            ("Pause", "Esc"),
            ("Camera Tilt", "Up and Down"),
            ("FOV", "Right and Left")
        ]
    );
    // Bound: no warning.
    menus.set_arm_bound(Some(true));
    let loading = menus.page(ms(0), &world.facts()).loading.unwrap();
    assert!(!loading.keys[0].warning);
}

#[test]
fn nothing_a_pilot_presses_gets_past_the_loading_screen() {
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(returning());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    let effects = press_all(
        &mut menus,
        &world,
        &[MenuInput::Select, MenuInput::Back, MenuInput::Down],
    );
    assert_eq!(effects, vec![]);
    assert_eq!(menus.screen(), Screen::Loading);
    menus.finish_loading();
    assert_eq!(menus.screen(), Screen::Flying);
}

/// A Hub pilot who pressed FLY and finished loading: in Free Flight.
fn flying() -> (Menus, World) {
    let world = World::with(vec![dualsense()]);
    let mut menus = Menus::new(returning());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    menus.finish_loading();
    (menus, world)
}

#[test]
fn pause_in_free_flight_opens_the_pause_menu_and_freezes_the_simulation() {
    // Basis: Rule (screens.md: single-player Pause freezes the simulation).
    let (mut menus, world) = flying();
    assert!(!menus.frozen());
    let effects = menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    assert_eq!(effects, vec![Effect::Freeze]);
    assert_eq!(menus.screen(), Screen::PauseMenu);
    assert!(menus.frozen());
    assert_eq!(
        labels(&menus, &world),
        vec![
            "Resume",
            "Reset",
            "Map",
            "Quad",
            "Settings",
            "Back to the Hub",
            "Quit"
        ]
    );
    assert_eq!(cursor_label(&menus, &world), "Resume");
}

#[test]
fn resume_counts_three_two_one_when_the_quad_was_in_the_air() {
    // Basis: Rule (#23 §6: Resume counts down 3-2-1, but only if the Quad
    // was in the air).
    let (mut menus, mut world) = flying();
    world.in_the_air = true;
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    let effects = menus.press(MenuInput::Select, None, &world.facts(), ms(10_000));
    assert_eq!(effects, vec![], "still frozen while it counts");
    assert!(menus.frozen());
    let count = |menus: &Menus, now| menus.page(ms(now), &world.facts()).countdown;
    assert_eq!(count(&menus, 10_000), Some(3));
    assert_eq!(count(&menus, 11_100), Some(2));
    assert_eq!(count(&menus, 12_100), Some(1));
    assert_eq!(menus.tick(ms(12_900)), vec![]);
    assert_eq!(
        menus.tick(ms(10_000 + u64::from(COUNTDOWN_SECONDS) * 1000)),
        vec![Effect::GoOn]
    );
    assert!(!menus.frozen());
    assert_eq!(count(&menus, 13_100), None);
}

#[test]
fn resume_goes_on_at_once_when_the_quad_was_on_the_ground() {
    // Basis: Rule (#23 §6).
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    let effects = menus.press(MenuInput::Select, None, &world.facts(), ms(500));
    assert_eq!(effects, vec![Effect::GoOn]);
    assert_eq!(menus.screen(), Screen::Flying);
    assert!(!menus.frozen());
}

#[test]
fn pause_again_closes_the_menu_and_pause_during_the_count_opens_it_again() {
    let (mut menus, mut world) = flying();
    world.in_the_air = true;
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    // The Pause Action in the Pause Menu is Resume: a count, in the air.
    menus.press(MenuInput::Pause, None, &world.facts(), ms(100));
    assert_eq!(menus.screen(), Screen::Flying);
    assert!(menus.frozen());
    // Pause during the count goes back to the menu, still frozen.
    let effects = menus.press(MenuInput::Pause, None, &world.facts(), ms(1100));
    assert_eq!(effects, vec![]);
    assert_eq!(menus.screen(), Screen::PauseMenu);
    assert!(menus.frozen());
}

#[test]
fn back_in_the_pause_menu_resumes() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    let effects = menus.press(MenuInput::Back, None, &world.facts(), ms(100));
    assert_eq!(effects, vec![Effect::GoOn]);
    assert_eq!(menus.screen(), Screen::Flying);
}

#[test]
fn reset_from_the_pause_menu_puts_the_quad_on_its_launch_spot_and_flies_on() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::Reset, &world.facts());
    let effects = menus.press(MenuInput::Select, None, &world.facts(), ms(100));
    assert_eq!(effects, vec![Effect::Reset]);
    assert_eq!(menus.screen(), Screen::Flying);
    assert!(!menus.frozen());
}

#[test]
fn picking_another_map_from_the_pause_menu_loads_it_and_saves_its_id() {
    // Basis: Rule (#23: picking a new Map or Quad puts the Quad on its Launch
    // Spot, via the loading screen; ids are what is saved).
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::OpenMapPicker, &world.facts());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Picker(Pick::Map));
    // The cursor starts on the Map in use (the Skate Park, second).
    assert_eq!(cursor_label(&menus, &world), "Skate Park");
    press_all(&mut menus, &world, &[MenuInput::Up]);
    let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(
        effects,
        vec![Effect::SettingsChanged, Effect::Load { by: None }]
    );
    assert_eq!(menus.settings().map, "opendrone/bando");
    assert_eq!(menus.screen(), Screen::Loading);
}

#[test]
fn picking_another_quad_from_the_pause_menu_loads_it_and_saves_its_id() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::OpenQuadPicker, &world.facts());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Picker(Pick::Quad));
    assert_eq!(cursor_label(&menus, &world), "Whoop 65");
    press_all(&mut menus, &world, &[MenuInput::Up]);
    let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(
        effects,
        vec![Effect::SettingsChanged, Effect::Load { by: None }]
    );
    assert_eq!(menus.settings().quad, "opendrone/freestyle-5");
}

#[test]
fn back_in_a_picker_returns_to_the_pause_menu_without_loading() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::OpenQuadPicker, &world.facts());
    press_all(&mut menus, &world, &[MenuInput::Select, MenuInput::Back]);
    assert_eq!(menus.screen(), Screen::PauseMenu);
}

#[test]
fn back_to_the_hub_leaves_the_flight_and_quit_quits() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::BackToHub, &world.facts());
    let effects = press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(effects, vec![Effect::ToHub]);
    assert_eq!(menus.screen(), Screen::Hub);
    assert_eq!(cursor_label(&menus, &world), "FLY");
    menus.point_at(ItemId::Quit, &world.facts());
    assert_eq!(
        press_all(&mut menus, &world, &[MenuInput::Select]),
        vec![Effect::Quit]
    );
}

#[test]
fn settings_opens_from_the_pause_menu_and_goes_back_to_it() {
    let (mut menus, world) = flying();
    menus.press(MenuInput::Pause, None, &world.facts(), ms(0));
    menus.point_at(ItemId::OpenSettings, &world.facts());
    press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Settings);
    // The Simulation stays frozen, and Pause closes the page.
    assert!(menus.frozen());
    menus.press(MenuInput::Pause, None, &world.facts(), ms(100));
    assert_eq!(menus.screen(), Screen::PauseMenu);
    assert_eq!(cursor_label(&menus, &world), "Settings");
}

#[test]
fn the_simulation_runs_only_in_free_flight_and_the_world_shows_from_the_loading_screen() {
    // Basis: Rule (screens.md: the Pause Menu freezes the simulation; #23 §4:
    // shaders compile on the loading screen, before flight).
    let menus = Menus::new(returning());
    assert!(menus.frozen());
    assert!(!menus.world_shown());
    let (flying, _) = flying();
    assert!(!flying.frozen());
    assert!(flying.world_shown());
}

#[test]
fn a_quad_counts_as_in_the_air_when_moving_or_armed_and_lifting_its_weight() {
    // Basis: Rule (model::quad_is_in_the_air): 0.5 m/s, or armed with thrust
    // at least half the weight. A 0.03 kg whoop weighs 0.294 N.
    let weight = 0.03 * 9.81;
    // Landed, disarmed.
    assert!(!quad_is_in_the_air(0.0, false, 0.0, weight));
    // Landed, armed and idling.
    assert!(!quad_is_in_the_air(0.0, true, 0.05 * weight, weight));
    // Hovering.
    assert!(quad_is_in_the_air(0.0, true, weight, weight));
    // Falling after a Failsafe drop, disarmed.
    assert!(quad_is_in_the_air(3.0, false, 0.0, weight));
}

#[test]
fn a_gamepad_flick_and_a_radio_flick_drive_the_same_menu_moves() {
    // Both map onto MenuInput, so one model serves every device.
    let world = World::with(vec![dualsense()]);
    let mut hub = Menus::new(returning());
    let mut radio = RadioMenu::new();
    radio.read(ms(0), CENTRE, CENTRE, CENTRE);
    for input in radio.read(ms(20), CENTRE, FULL_LOW, CENTRE) {
        hub.press(input, None, &world.facts(), ms(20));
    }
    assert_eq!(cursor_label(&hub, &world), "Map");
    let mut menu = PadMenu::new();
    menu.read(ms(0), &pad());
    let mut down = pad();
    down.buttons[PadButton::DpadDown.index()] = true;
    for input in menu.read(ms(20), &down) {
        hub.press(input, None, &world.facts(), ms(20));
    }
    assert_eq!(cursor_label(&hub, &world), "Quad");
}

// ------------------------------------------------------ Flying device

/// The value on the Settings page's Flying device stop, and its note.
fn flying_device_stop(menus: &Menus, world: &World) -> (String, String) {
    let item = menus
        .items(&world.facts())
        .into_iter()
        .find(|i| i.id == ItemId::FlyingDevice)
        .expect("the Settings page has a Flying device stop");
    (item.value.unwrap(), item.note.unwrap())
}

fn settings_page(settings: PilotSettings, world: &World) -> Menus {
    let mut menus = Menus::new(settings);
    menus.point_at(ItemId::OpenSettings, &world.facts());
    press_all(&mut menus, world, &[MenuInput::Select]);
    assert_eq!(menus.screen(), Screen::Settings);
    menus
}

fn named(name: &str) -> FlyingDevice {
    FlyingDevice::Named(name.to_string())
}

#[test]
fn the_settings_page_shows_the_flying_device_after_auto_arm_and_says_what_auto_means() {
    // Basis: Rule (#271: next to Text size, Reduce motion and Auto-arm, with a
    // plain description).
    let world = World::with(vec![pocket(), dualsense()]);
    let menus = settings_page(returning(), &world);
    assert_eq!(
        labels(&menus, &world),
        vec![
            "Text size",
            "Reduce motion",
            "Auto-arm",
            "Flying device",
            "Back"
        ]
    );
    let (value, note) = flying_device_stop(&menus, &world);
    assert_eq!(value, "Auto");
    assert!(note.starts_with("Auto flies a Radio if one is plugged in, then a Gamepad."));
}

#[test]
fn the_flying_device_steps_through_auto_and_each_device_found_and_is_saved() {
    // Basis: Rule (#271: the pilot can choose one of the devices found, by
    // its profile's name; each setting is one stop).
    let world = World::with(vec![pocket(), dualsense()]);
    let mut menus = settings_page(returning(), &world);
    menus.point_at(ItemId::FlyingDevice, &world.facts());
    assert_eq!(
        press_all(&mut menus, &world, &[MenuInput::Right]),
        vec![Effect::SettingsChanged]
    );
    assert_eq!(menus.settings().flying_device, named("Radiomaster Pocket"));
    assert_eq!(flying_device_stop(&menus, &world).0, "Radiomaster Pocket");
    press_all(&mut menus, &world, &[MenuInput::Right]);
    assert_eq!(menus.settings().flying_device, named("DualSense"));
    press_all(&mut menus, &world, &[MenuInput::Right]);
    assert_eq!(menus.settings().flying_device, FlyingDevice::Auto);
    press_all(&mut menus, &world, &[MenuInput::Left]);
    assert_eq!(menus.settings().flying_device, named("DualSense"));
    // Select steps on too, as for every stop.
    press_all(&mut menus, &world, &[MenuInput::Select]);
    assert_eq!(menus.settings().flying_device, FlyingDevice::Auto);
}

#[test]
fn a_saved_device_that_is_not_plugged_in_shows_so_and_can_be_stepped_off() {
    // Basis: Rule (#271: falls back to Auto and says so).
    let world = World::with(vec![dualsense()]);
    let settings = PilotSettings {
        flying_device: named("Radiomaster Pocket"),
        ..returning()
    };
    let mut menus = settings_page(settings, &world);
    let (value, note) = flying_device_stop(&menus, &world);
    assert_eq!(value, "Radiomaster Pocket (not plugged in)");
    assert!(note.contains("isn't plugged in, so Auto picks for now"));
    menus.point_at(ItemId::FlyingDevice, &world.facts());
    // Back from the saved device is the DualSense, the last stop plugged in;
    // once the pilot has moved off it, it is no longer a stop.
    press_all(&mut menus, &world, &[MenuInput::Left]);
    assert_eq!(menus.settings().flying_device, named("DualSense"));
    press_all(&mut menus, &world, &[MenuInput::Right]);
    assert_eq!(menus.settings().flying_device, FlyingDevice::Auto);
}

#[test]
fn the_hub_warns_and_fly_says_so_when_the_saved_device_is_not_plugged_in() {
    // Basis: Rule (#271: the message is in the start notice and the Pause Menu).
    let world = World::with(vec![dualsense()]);
    let settings = PilotSettings {
        flying_device: named("Radiomaster Pocket"),
        ..returning()
    };
    let mut hub = Menus::new(settings);
    assert!(hub.warnings(&world.facts()).contains(
        &"Radiomaster Pocket is your Flying device but isn't plugged in, so Auto picks for now."
            .to_string()
    ));
    press_all(&mut hub, &world, &[MenuInput::Select]);
    let page = hub.page(ms(0), &world.facts());
    assert!(
        page.notices.contains(
            &"Radiomaster Pocket isn't plugged in, so the Flying device is Auto for this flight."
                .to_string()
        ),
        "{:?}",
        page.notices
    );
}

#[test]
fn a_flying_device_that_is_plugged_in_or_auto_gives_no_warning_and_no_notice() {
    let world = World::with(vec![pocket(), dualsense()]);
    for flying_device in [FlyingDevice::Auto, named("DualSense")] {
        let mut hub = Menus::new(PilotSettings {
            flying_device,
            ..returning()
        });
        assert!(
            hub.warnings(&world.facts())
                .iter()
                .all(|w| !w.contains("Flying device"))
        );
        press_all(&mut hub, &world, &[MenuInput::Select]);
        assert!(hub.page(ms(0), &world.facts()).notices.is_empty());
    }
}
