//! Readable checks for how the pilot's Input Devices fly the Quad, fed the
//! input thread's batches as the game reads them, with no SDL: a Pocket and
//! a DualSense over USB, as the built-in Pack's profiles describe them.
//! Each frame reads its batch, then runs the 1 kHz ticks the time calls for,
//! as Bevy does. Basis: Rule (docs/context/input.md and ADR-0003). Times in
//! milliseconds on both clocks; Channels as CRSF steps.

mod common;

use std::time::Duration;

use bevy::prelude::*;
use opendrone::pilot::{FlightWorld, Pilot};
use opendrone::set_up::FlightPlan;
use opendrone::stepping::Flight;
use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::Connection;
use opendrone_input::{Batch, DeviceInfo, DeviceState, Lost, Raw, SdlId};
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
        let plan = common::plan("whoop-65");
        let profiles = common::packs()
            .input_devices()
            .into_iter()
            .cloned()
            .collect();
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
