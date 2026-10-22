//! Readable checks for how the pilot's Input Devices fly the Quad, fed the
//! input thread's batches as the game reads them, with no SDL: a Pocket and
//! a DualSense over USB, as the built-in Pack's profiles describe them.
//! Each frame reads its batch, then runs the 1 kHz ticks the time calls for,
//! as Bevy does. Basis: Rule (docs/context/input.md and ADR-0003). Times in
//! milliseconds on both clocks; Channels as CRSF steps.

mod common;

use std::time::Duration;

use bevy::input::ButtonInput;
use bevy::prelude::*;
use opendrone::camera::controls::{Repeater, camera_inputs};
use opendrone::pilot::{FlightWorld, Pilot};
use opendrone::set_up::FlightPlan;
use opendrone::stepping::Flight;
use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::{Action, Connection};
use opendrone_input::{Batch, DeviceInfo, DeviceState, Lost, Raw, SdlId};
use opendrone_pack::input_device::controls::Position;
use opendrone_pack::input_device::{ActionSource, Setup};
use opendrone_sim::{Channel, EscState, FlightInput, QuadOutput};

const POCKET: SdlId = SdlId(1);
const DUALSENSE: SdlId = SdlId(2);

/// A Radiomaster Pocket over USB, RF off: eight axis channels and the rest
/// as buttons, all at rest.
fn pocket() -> Raw {
    Raw::Added {
        device: POCKET,
        info: DeviceInfo {
            name: "EdgeTX Radiomaster Pocket Joystick".to_string(),
            usb_vendor: 0x1209,
            usb_product: 0x4F54,
            sdl_gamepad: true,
            connection: Some(Connection::Usb),
            heartbeat: false,
        },
        state: DeviceState::new(8, 24, false),
    }
}

/// A DualSense over USB with its motion sensors on: it reports at rest.
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

/// The game, one frame at a time.
struct Game {
    plan: FlightPlan,
    flight: Flight,
    pilot: Pilot,
    game_clock: Time<Virtual>,
    ticks: u64,
    /// The computer's clock at the last frame; the game's clock stands still
    /// while paused.
    real: Duration,
}

impl Game {
    fn new() -> Game {
        let profiles = common::packs()
            .input_devices()
            .into_iter()
            .cloned()
            .collect();
        Game::with_profiles(profiles)
    }

    fn with_profiles(profiles: Vec<opendrone_input::InputDeviceProfile>) -> Game {
        let plan = common::plan("whoop-65");
        Game {
            flight: Flight::new(plan.simulation(None).unwrap()).unwrap(),
            pilot: Pilot::without_input_thread(&plan, profiles),
            plan,
            game_clock: Time::default(),
            ticks: 0,
            real: Duration::ZERO,
        }
    }

    /// A frame at `ms`: the batch the input thread read then, if any, then
    /// the ticks up to it (none while paused).
    fn frame(&mut self, ms: u64, events: Vec<Raw>) {
        let at = Duration::from_millis(ms);
        if !self.game_clock.is_paused() {
            self.game_clock.advance_by(at - self.real);
        }
        self.real = at;
        let batches = if events.is_empty() {
            Vec::new()
        } else {
            vec![Batch { at, events }]
        };
        let mut world = FlightWorld {
            flight: &mut self.flight,
            plan: &self.plan,
            fixed_clock: Duration::from_millis(self.ticks),
            game_clock: &mut self.game_clock,
        };
        self.pilot.read(batches, at, &mut world);
        while Duration::from_millis(self.ticks + 1) <= self.game_clock.elapsed() {
            self.flight.tick();
            self.ticks += 1;
        }
    }

    /// Frames every `every` ms after `from` up to `to`, each with `events`.
    fn frames(&mut self, from: u64, to: u64, every: u64, events: &[Raw]) {
        let mut ms = from + every;
        while ms <= to {
            self.frame(ms, events.to_vec());
            ms += every;
        }
    }

    fn output(&self) -> QuadOutput {
        self.flight.simulation().quad_output(0)
    }

    fn steps(&self) -> u64 {
        self.flight.simulation().time().ticks()
    }
}

#[test]
fn the_input_device_whose_sticks_move_first_becomes_the_flying_input_device_and_starts_the_flight()
{
    let mut game = Game::new();
    game.frame(0, vec![pocket(), dualsense()]);
    game.frames(0, 100, 10, &[]);
    assert_eq!(game.pilot.flying_name(), None);
    assert_eq!(game.steps(), 800);
    assert_eq!(game.output().radio_link.last_frame, None);

    // Roll right on the Pocket: a fresh flight starts, flown by it.
    game.frame(
        101,
        vec![Raw::Axis {
            device: POCKET,
            axis: 0,
            value: 16384,
        }],
    );
    assert_eq!(
        game.pilot.flying_name(),
        Some("EdgeTX Radiomaster Pocket Joystick")
    );
    assert_eq!(game.steps(), 8, "the flight started afresh at the tick");
    game.frames(101, 120, 1, &[]);
    let roll = game.output().radio_link.last_frame.unwrap().roll;
    assert!(roll > Channel::CENTRE, "{roll:?}");

    // The DualSense's sticks never fly once the flight has started.
    game.frame(
        121,
        vec![Raw::PadAxis {
            device: DUALSENSE,
            axis: PadAxis::RightX,
            value: -32768,
        }],
    );
    game.frames(121, 140, 1, &[]);
    assert_eq!(game.output().radio_link.last_frame.unwrap().roll, roll);
}

/// A DualSense that started the flight with a flick of its right stick, then
/// let it go.
fn flown_by_a_dualsense() -> Game {
    let mut game = Game::new();
    game.frame(0, vec![dualsense()]);
    let flick = |value| Raw::PadAxis {
        device: DUALSENSE,
        axis: PadAxis::RightX,
        value,
    };
    game.frame(4, vec![flick(30000)]);
    game.frame(8, vec![flick(0)]);
    game
}

#[test]
fn a_resting_dualsense_keeps_its_radio_link_up_and_locked_with_its_reports_alone() {
    let mut game = flown_by_a_dualsense();
    // Two seconds of reports every 4 ms (250 Hz), none changing a thing.
    game.frames(8, 2008, 4, &[Raw::Report { device: DUALSENSE }]);
    let link = game.output().radio_link;
    assert!(!link.lost);
    assert!(link.locked, "the frames follow its 250 Hz beat (ADR-0020)");
}

#[test]
fn a_dualsense_that_stops_reporting_loses_its_radio_link_a_second_later() {
    let mut game = flown_by_a_dualsense();
    game.frames(8, 1008, 4, &[Raw::Report { device: DUALSENSE }]);
    assert!(!game.output().radio_link.lost);
    // Silent from 1.008 s.
    game.frames(1008, 1950, 4, &[]);
    assert!(!game.output().radio_link.lost, "not yet a second");
    game.frames(1950, 2100, 4, &[]);
    assert!(game.output().radio_link.lost);
    assert_eq!(game.pilot.flying_lost(), Some(Lost::Silent));
}

#[test]
fn unplugging_the_flying_input_device_loses_the_radio_link_at_once_and_plugging_it_in_brings_it_back()
 {
    let mut game = Game::new();
    game.frame(0, vec![pocket()]);
    game.frame(
        10,
        vec![Raw::Axis {
            device: POCKET,
            axis: 1,
            value: 20000,
        }],
    );
    game.frames(10, 50, 1, &[]);
    assert!(!game.output().radio_link.lost);
    game.frame(51, vec![Raw::Removed { device: POCKET }]);
    game.frames(51, 53, 1, &[]);
    assert!(game.output().radio_link.lost);
    assert_eq!(game.pilot.flying_lost(), Some(Lost::Unplugged));
    game.frame(500, vec![pocket()]);
    game.frames(500, 510, 1, &[]);
    assert!(!game.output().radio_link.lost);
}

#[test]
fn the_dualsenses_options_button_pauses_and_create_resets() {
    let mut game = flown_by_a_dualsense();
    let reports = [Raw::Report { device: DUALSENSE }];
    game.frames(8, 2000, 4, &reports);
    assert_eq!(game.output().motors[0].esc, EscState::Ready);

    // Create: Reset powers the Quad up afresh, so its ESCs start up again.
    let create = |down| Raw::PadButton {
        device: DUALSENSE,
        button: PadButton::Back,
        down,
    };
    game.frame(2004, vec![create(true)]);
    game.frames(2004, 2012, 4, &reports);
    assert!(matches!(
        game.output().motors[0].esc,
        EscState::StartingUp(_)
    ));
    game.frame(2016, vec![create(false)]);

    // Options: Pause stops stepping until it's pressed again.
    let options = |down| Raw::PadButton {
        device: DUALSENSE,
        button: PadButton::Start,
        down,
    };
    game.frame(2020, vec![options(true)]);
    let paused_at = game.steps();
    game.frame(2024, vec![options(false)]);
    game.frames(2024, 2500, 4, &reports);
    assert!(game.game_clock.is_paused());
    assert_eq!(game.steps(), paused_at);
    game.frame(2504, vec![options(true)]);
    game.frames(2504, 2600, 4, &reports);
    assert!(!game.game_clock.is_paused());
    assert!(game.steps() > paused_at);
}

/// Options, pressed and let go in two frames at `ms` and `ms + 1`: Pause.
fn press_options(game: &mut Game, ms: u64) {
    let options = |down| Raw::PadButton {
        device: DUALSENSE,
        button: PadButton::Start,
        down,
    };
    game.frame(ms, vec![options(true)]);
    game.frame(ms + 1, vec![options(false)]);
}

/// A DualSense flight, paused with Options, resting its sticks for 600 ms of
/// pause, then flying on. `many_reports` says what the pad sends while
/// paused: a right-stick position every 4 ms with a heartbeat report between
/// (many reports), or only the last two (the latest report alone). Either
/// way the last stick position is 12000 (about 37% of full). Returns the game
/// 100 ms after it flew on.
fn paused_with_reports(many_reports: bool) -> Game {
    let mut game = flown_by_a_dualsense();
    let reports = [Raw::Report { device: DUALSENSE }];
    let stick = |value| Raw::PadAxis {
        device: DUALSENSE,
        axis: PadAxis::RightX,
        value,
    };
    game.frames(8, 2000, 4, &reports);
    press_options(&mut game, 2004);
    assert!(game.game_clock.is_paused());

    if many_reports {
        // The stick swings between the ends while the game stands still.
        for (i, ms) in (2012..2600).step_by(4).enumerate() {
            let value = if i % 2 == 0 { 30000 } else { -30000 };
            game.frame(ms, vec![stick(value)]);
            game.frame(ms + 1, reports.to_vec());
        }
    }
    game.frame(2600, vec![stick(12000)]);
    game.frame(2602, reports.to_vec());
    assert!(game.game_clock.is_paused());

    press_options(&mut game, 2604);
    assert!(!game.game_clock.is_paused());
    game.frames(2605, 2705, 4, &reports);
    game
}

#[test]
fn after_a_pause_with_many_reports_the_first_step_flies_on_the_latest_report_alone() {
    let many = paused_with_reports(true);
    let alone = paused_with_reports(false);
    assert_eq!(many.steps(), alone.steps());
    let (many_kept, alone_kept) = (many.flight.kept().unwrap(), alone.flight.kept().unwrap());
    assert_eq!(
        many_kept.inputs.len(),
        alone_kept.inputs.len(),
        "the Simulation was handed no more Flight Inputs than the latest report's"
    );
    assert_eq!(
        many_kept.state_fingerprint, alone_kept.state_fingerprint,
        "the Simulation's whole state matches after 100 ms of flying on"
    );
    assert_eq!(
        many.output().radio_link.last_frame,
        alone.output().radio_link.last_frame
    );
}

#[test]
fn unplugging_and_plugging_in_the_flying_input_device_during_a_pause_still_reaches_the_radio_link()
{
    let mut game = flown_by_a_dualsense();
    let reports = [Raw::Report { device: DUALSENSE }];
    game.frames(8, 2000, 4, &reports);
    press_options(&mut game, 2004);
    assert!(game.game_clock.is_paused());
    let before = game.flight.kept().unwrap().inputs.len();

    game.frames(2008, 2100, 4, &reports);
    game.frame(2104, vec![Raw::Removed { device: DUALSENSE }]);
    game.frames(2104, 2200, 4, &[]);
    game.frame(2204, vec![dualsense()]);
    game.frames(2204, 2300, 4, &reports);

    // Flying on: the unplug and the plug-in both arrive, in that order.
    press_options(&mut game, 2304);
    assert!(!game.game_clock.is_paused());
    let kept = game.flight.kept().unwrap();
    let arrived: Vec<FlightInput> = kept.inputs[before..].iter().map(|(_, i)| *i).collect();
    let position = |wanted| arrived.iter().position(|i| *i == wanted);
    let lost = position(FlightInput::InputDeviceLost).expect("the unplug arrives");
    let back = position(FlightInput::InputDeviceBack).expect("the plug-in arrives");
    assert!(lost < back, "{arrived:?}");
}

// ---------------------------------------------------------------------------
// A button tap shorter than a frame (#264)

/// The built-in profiles with the camera's Actions bound to buttons, as a
/// pilot can rebind them: on the DualSense, Camera Tilt up is D-pad Up and
/// FOV wider is D-pad Right; on the Pocket, Camera Tilt up is the button
/// channel CH12 and FOV wider is CH13. Pause stays where the Pack puts it:
/// Options on the DualSense, CH11 on the Pocket.
fn profiles_with_the_camera_on_buttons() -> Vec<opendrone_input::InputDeviceProfile> {
    common::packs()
        .input_devices()
        .into_iter()
        .cloned()
        .map(|mut profile| {
            match &mut profile.setup {
                Setup::Gamepad(pad) => {
                    pad.actions.insert(
                        Action::CameraTiltUp,
                        ActionSource::Button(PadButton::DpadUp),
                    );
                    pad.actions
                        .insert(Action::FovWider, ActionSource::Button(PadButton::DpadRight));
                }
                Setup::Radio(radio) => {
                    for (action, channel) in [(Action::CameraTiltUp, 12), (Action::FovWider, 13)] {
                        radio.actions.insert(
                            action,
                            ActionSource::Channel {
                                channel,
                                on: Position::Pressed,
                            },
                        );
                    }
                }
            }
            profile
        })
        .collect()
}

/// A button the checks press: a DualSense button, or a Pocket button channel
/// (CH9 is button 0).
#[derive(Clone, Copy)]
enum Pressed {
    Pad(PadButton),
    Channel(u8),
}

impl Pressed {
    fn raw(self, down: bool) -> Raw {
        match self {
            Pressed::Pad(button) => Raw::PadButton {
                device: DUALSENSE,
                button,
                down,
            },
            Pressed::Channel(channel) => Raw::Button {
                device: POCKET,
                button: channel - 9,
                down,
            },
        }
    }

    /// Down and up again, both in one batch.
    fn tap(self) -> Vec<Raw> {
        vec![self.raw(true), self.raw(false)]
    }
}

const TILT_UP: [Pressed; 2] = [Pressed::Pad(PadButton::DpadUp), Pressed::Channel(12)];
const PAUSE: [Pressed; 2] = [Pressed::Pad(PadButton::Start), Pressed::Channel(11)];

/// A game with both devices plugged in and the camera's Actions on buttons,
/// with the camera's Repeater.
struct CameraGame {
    game: Game,
    repeater: Repeater,
    keys: ButtonInput<KeyCode>,
}

impl CameraGame {
    fn new() -> CameraGame {
        CameraGame {
            game: both_devices(),
            repeater: Repeater::new(),
            keys: ButtonInput::default(),
        }
    }

    /// A frame at `ms` reading `events` in one batch, then what the camera
    /// acts on, as `read_the_camera_keys` works it out.
    fn camera_frame(&mut self, ms: u64, mut events: Vec<Raw>) -> Vec<Action> {
        // The DualSense reports at rest: silent for a second, it is lost.
        events.push(Raw::Report { device: DUALSENSE });
        self.game.frame(ms, events);
        self.keys.clear();
        let (held, tapped) = camera_inputs(
            &self.keys,
            self.game.pilot.actions_held(),
            self.game.pilot.actions_tapped(),
        );
        self.repeater
            .update_with_taps(Duration::from_millis(ms), &held, &tapped)
    }
}

/// A game with a Pocket and a DualSense plugged in at 0 ms, the camera's
/// Actions on buttons.
fn both_devices() -> Game {
    let mut game = Game::with_profiles(profiles_with_the_camera_on_buttons());
    game.frame(0, vec![pocket(), dualsense()]);
    game
}

#[test]
fn a_button_pressed_and_let_go_within_one_frame_fires_its_action_once() {
    // Basis: Rule. docs/context/input.md: an Action fires once, when its
    // button moves into the bound position. A tap shorter than a frame is up
    // again when the frame looks, and still counts (#264), as a key tap does
    // (#186). Pause toggles each time it fires, so one tap pauses and a
    // second unpauses; both reports of a tap are in one batch.
    for button in PAUSE {
        let mut game = both_devices();
        game.frame(4, button.tap());
        assert!(game.game_clock.is_paused(), "the tap paused the game");
        game.frames(4, 400, 4, &[]);
        assert!(
            game.game_clock.is_paused(),
            "the frames after it do nothing"
        );
        game.frame(404, button.tap());
        assert!(!game.game_clock.is_paused(), "a second tap fires again");
    }
}

#[test]
fn a_held_button_fires_once_and_not_every_frame() {
    // Basis: Rule. Held for 400 ms of 4 ms frames, Pause fired once: the
    // game is paused, and stays paused.
    for button in PAUSE {
        let mut game = both_devices();
        game.frame(4, vec![button.raw(true)]);
        assert!(game.game_clock.is_paused());
        game.frames(4, 400, 4, &[]);
        assert!(
            game.game_clock.is_paused(),
            "a held button doesn't fire again"
        );
        game.frame(404, vec![button.raw(false)]);
        assert!(game.game_clock.is_paused(), "letting go doesn't fire");
        game.frame(408, button.tap());
        assert!(!game.game_clock.is_paused());
    }
}

#[test]
fn a_camera_button_tapped_within_one_frame_acts_once_as_a_key_does() {
    // Basis: Rule. The camera's Actions act the same from a button as from a
    // key (#186, #264): once for a tap, then nothing on the frames after it.
    for button in TILT_UP {
        let mut camera = CameraGame::new();
        assert_eq!(
            camera.camera_frame(10, button.tap()),
            vec![Action::CameraTiltUp]
        );
        for ms in [20, 30, 1500] {
            assert!(camera.camera_frame(ms, vec![]).is_empty());
        }
        assert_eq!(
            camera.camera_frame(2000, button.tap()),
            vec![Action::CameraTiltUp],
            "a second tap acts again"
        );
    }
}

#[test]
fn a_held_camera_button_acts_at_once_then_repeats_after_1_s_and_every_200_ms_as_a_key_does() {
    // Basis: Source. #14 §2: "Holding a key repeats it after 1 s, every
    // 200 ms", and a button bound to the same Action does the same.
    for button in TILT_UP {
        let mut camera = CameraGame::new();
        let mut acted = Vec::new();
        let mut events = vec![button.raw(true)];
        for ms in (10..=1410).step_by(10) {
            if !camera
                .camera_frame(ms, std::mem::take(&mut events))
                .is_empty()
            {
                acted.push(ms);
            }
        }
        assert_eq!(acted, vec![10, 1010, 1210, 1410]);
    }
}

#[test]
fn a_held_camera_button_let_go_and_pressed_again_within_one_frame_acts_again_at_once() {
    // Basis: Rule. The second press is a press: it acts at once and waits a
    // second again, as a key does.
    for button in TILT_UP {
        let mut camera = CameraGame::new();
        assert_eq!(
            camera.camera_frame(10, vec![button.raw(true)]),
            vec![Action::CameraTiltUp]
        );
        assert_eq!(
            camera.camera_frame(500, vec![button.raw(false), button.raw(true)]),
            vec![Action::CameraTiltUp]
        );
        assert!(camera.camera_frame(1000, vec![]).is_empty());
        assert_eq!(
            camera.camera_frame(1500, vec![]),
            vec![Action::CameraTiltUp]
        );
    }
}

#[test]
fn a_key_tap_and_button_taps_within_one_frame_each_act_once() {
    // Basis: Rule. FOV wider from the keyboard and from a button, and Camera
    // Tilt up from a button, all tapped within the frame: each acts once.
    let mut camera = CameraGame::new();
    let right = Pressed::Pad(PadButton::DpadRight);
    let mut events = TILT_UP[0].tap();
    events.extend(right.tap());
    camera.game.frame(10, events);
    camera.keys.clear();
    camera.keys.press(KeyCode::ArrowRight);
    camera.keys.release(KeyCode::ArrowRight);
    let (held, tapped) = camera_inputs(
        &camera.keys,
        camera.game.pilot.actions_held(),
        camera.game.pilot.actions_tapped(),
    );
    assert!(held.is_empty());
    assert_eq!(tapped, vec![Action::CameraTiltUp, Action::FovWider]);
}

#[test]
fn a_switch_swept_through_a_position_is_not_a_press_of_it() {
    // Basis: Rule. Only a button's press counts between frames: a Radio's
    // axis channel that passes through its bound position within one frame
    // and ends elsewhere fires nothing, since a sweep isn't a press.
    let mut profiles = profiles_with_the_camera_on_buttons();
    for profile in &mut profiles {
        if let Setup::Radio(radio) = &mut profile.setup {
            radio.actions.insert(
                Action::FovWider,
                ActionSource::Channel {
                    channel: 8,
                    on: Position::Middle,
                },
            );
        }
    }
    let mut game = Game::with_profiles(profiles);
    game.frame(0, vec![pocket()]);
    let axis = |value| Raw::Axis {
        device: POCKET,
        axis: 7,
        value,
    };
    game.frame(5, vec![axis(-30000)]);
    game.frame(10, vec![axis(0), axis(30000)]);
    assert!(game.pilot.actions_tapped().is_empty());
}
