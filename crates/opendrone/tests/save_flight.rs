//! Readable checks for Save Flight (#73): the Action that writes the flight
//! so far as a Scenario and an Input Track. The Rule behind them is
//! ADR-0001: the same starting state and the same Flight Inputs give the same
//! flight, bit for bit, so a saved flight flown again by the headless Scenario
//! runner gives the fingerprint it had in the game. Times in milliseconds on
//! both clocks; Channels as CRSF steps.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use opendrone::pilot::{FlightWorld, Pilot};
use opendrone::save::flight_files;
use opendrone::set_up::FlightPlan;
use opendrone::settings::Settings;
use opendrone::stepping::Flight;
use opendrone_input::controls::{PadAxis, PadButton};
use opendrone_input::profile::Action;
use opendrone_input::{Batch, DeviceInfo, DeviceState, InputDeviceProfile, Raw, SdlId};
use opendrone_pack::input_device::{ActionSource, Setup};
use opendrone_scenario::{read_scenario, run};
use opendrone_sim::{
    Channel, Channels, FlightInput, InputDeviceFacts, PacketRate, Rates, ReportRate, SimulationTime,
};

const DUALSENSE: SdlId = SdlId(2);

/// A DualSense over USB with its motion sensors on: it reports at rest.
fn dualsense() -> Raw {
    Raw::Added {
        device: DUALSENSE,
        info: DeviceInfo {
            name: "DualSense Wireless Controller".to_string(),
            usb_vendor: 0x054C,
            usb_product: 0x0CE6,
            sdl_gamepad: true,
            connection: Some(opendrone_input::profile::Connection::Usb),
            heartbeat: true,
        },
        state: DeviceState::new(6, 0, true),
    }
}

/// The built-in profiles, with Save Flight bound to the DualSense's touchpad
/// (no profile binds it by default).
fn profiles_with_save_flight_on_the_touchpad() -> Vec<InputDeviceProfile> {
    common::packs()
        .input_devices()
        .into_iter()
        .cloned()
        .map(|mut profile| {
            if let Setup::Gamepad(pad) = &mut profile.setup {
                pad.actions.insert(
                    Action::SaveFlight,
                    ActionSource::Button(PadButton::Touchpad),
                );
            }
            profile
        })
        .collect()
}

/// A folder of its own for one test's saved files.
fn folder(test: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "opendrone-save-flight-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The game, one frame at a time, as `tests/pilot.rs` runs it.
struct Game {
    plan: FlightPlan,
    flight: Flight,
    pilot: Pilot,
    game_clock: Time<Virtual>,
    ticks: u64,
    real: Duration,
}

impl Game {
    /// A game whose flight also keeps the fingerprint of the whole run, as
    /// only the readable checks do.
    fn new(quad: &str, saves_to: PathBuf) -> Game {
        let mut game = Game::as_the_game_runs(quad, saves_to);
        game.pilot = game.pilot.checking_every_step();
        game
    }

    /// A game as it runs for a pilot: its flight keeps the Flight Inputs and
    /// nothing more.
    fn as_the_game_runs(quad: &str, saves_to: PathBuf) -> Game {
        let plan = common::plan(quad);
        Game {
            flight: Flight::new(plan.simulation(None).unwrap()).unwrap(),
            pilot: Pilot::without_input_thread(&plan, profiles_with_save_flight_on_the_touchpad())
                .saving_to(saves_to),
            plan,
            game_clock: Time::default(),
            ticks: 0,
            real: Duration::ZERO,
        }
    }

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

    /// Frames every `every` ms after `from` up to `to`, the DualSense
    /// reporting in each.
    fn reporting(&mut self, from: u64, to: u64, every: u64) {
        let mut ms = from + every;
        while ms <= to {
            self.frame(ms, vec![Raw::Report { device: DUALSENSE }]);
            ms += every;
        }
    }

    fn press(&mut self, ms: u64, button: PadButton) {
        for (at, down) in [(ms, true), (ms + 4, false)] {
            self.frame(
                at,
                vec![Raw::PadButton {
                    device: DUALSENSE,
                    button,
                    down,
                }],
            );
        }
    }

    fn stick(&mut self, ms: u64, axis: PadAxis, value: i16) {
        self.frame(
            ms,
            vec![Raw::PadAxis {
                device: DUALSENSE,
                axis,
                value,
            }],
        );
    }

    /// A pilot's flight on a DualSense: the Quad starts, arms, climbs with
    /// some roll, is Reset, arms again, and the device is unplugged and
    /// plugged back in. 10 s of it.
    fn fly(&mut self) {
        self.frame(0, vec![dualsense()]);
        self.stick(4, PadAxis::RightX, 30000);
        self.stick(8, PadAxis::RightX, 0);
        self.reporting(8, 2000, 4);
        self.press(2004, PadButton::RightShoulder);
        self.reporting(2008, 2300, 4);
        self.stick(2304, PadAxis::LeftY, -20000);
        self.reporting(2304, 3000, 4);
        self.stick(3004, PadAxis::RightX, 12000);
        self.reporting(3004, 3500, 4);
        self.stick(3504, PadAxis::RightX, 0);
        self.reporting(3504, 4000, 4);
        // Reset: back on the Launch Spot, powered up afresh.
        self.press(4004, PadButton::Back);
        self.reporting(4008, 6000, 4);
        self.press(6004, PadButton::RightShoulder);
        self.reporting(6008, 6500, 4);
        self.frame(6504, vec![Raw::Removed { device: DUALSENSE }]);
        self.reporting(6504, 7000, 4);
        self.frame(7004, vec![dualsense()]);
        self.reporting(7004, 10000, 4);
    }
}

/// What a saved flight is checked against: the files read back as the
/// headless runner reads them, and its run.
struct FlownAgain {
    scenario: opendrone_scenario::Scenario,
    outcome: opendrone_scenario::Outcome,
}

fn flown_again(files: &opendrone::save::FlightFiles, name: &str) -> FlownAgain {
    let mut scenario = read_scenario(name, &files.scenario).unwrap_or_else(|p| panic!("{p}"));
    scenario
        .read_input_track("the Input Track", &files.track)
        .unwrap_or_else(|p| panic!("{p}"));
    let outcome = run(&scenario, &common::packs()).unwrap_or_else(|p| panic!("{p}"));
    FlownAgain { scenario, outcome }
}

#[test]
fn a_saved_flight_flown_again_by_the_headless_runner_gives_the_fingerprint_it_had_in_the_game() {
    // Basis: Rule (ADR-0001). A DualSense flight with a Reset and an unplug in
    // it, 10 s, on both Quads.
    for quad in ["whoop-65", "freestyle-5"] {
        let mut game = Game::new(quad, folder("again"));
        game.fly();
        let files = flight_files(&game.plan, &game.flight, "flight.csv").unwrap();
        assert_eq!(files.steps, game.flight.simulation().time().ticks());
        let again = flown_again(&files, "the saved flight");
        assert_eq!(
            Some(again.outcome.run_fingerprint()),
            files.run_fingerprint,
            "{quad}: the runner's run fingerprint is the game's"
        );
        assert_eq!(
            again.outcome.step_fingerprints.last().copied(),
            Some(game.flight.simulation().fingerprint()),
            "{quad}: and the whole state at the last step is the same"
        );
        assert_eq!(
            files.state_fingerprint,
            game.flight.simulation().fingerprint()
        );
        assert!(
            again.outcome.first_broken_number.is_none(),
            "{quad}: {:?}",
            again.outcome.first_broken_number
        );
        assert!(
            again.outcome.expectations.iter().all(|e| e.passed),
            "{quad}: the one Expectation holds"
        );
    }
}

#[test]
fn a_game_that_fingerprints_nothing_but_the_end_still_saves_a_flight_the_runner_agrees_with() {
    // Basis: Rule (ADR-0001). The game keeps no fingerprint of its steps (that
    // would cost more than taking them), so what it says is the whole state's
    // fingerprint after the last step, and the runner's fingerprint after that
    // step is the same.
    let mut game = Game::as_the_game_runs("whoop-65", folder("plain"));
    game.fly();
    let files = flight_files(&game.plan, &game.flight, "flight.csv").unwrap();
    assert_eq!(files.run_fingerprint, None);
    let again = flown_again(&files, "the saved flight");
    assert_eq!(
        again.outcome.step_fingerprints.last().copied(),
        Some(files.state_fingerprint)
    );
    assert!(
        files
            .scenario
            .contains(&format!("was {}", files.state_fingerprint)),
        "the Scenario says the fingerprint: {}",
        files.scenario
    );
}

#[test]
fn the_saved_input_track_holds_the_reset_and_the_unplug_as_other_flight_inputs() {
    let mut game = Game::new("whoop-65", folder("other-flight-inputs"));
    game.fly();
    let files = flight_files(&game.plan, &game.flight, "flight.csv").unwrap();
    let rows: Vec<&str> = files.track.lines().collect();
    let entries = |word: &str| rows.iter().filter(|r| r.ends_with(word)).count();
    assert_eq!(entries(",reset"), 1);
    assert_eq!(entries(",input device lost"), 1);
    assert_eq!(entries(",input device back"), 1);
    // The Scenario says when the Reset was and when the Arm switch went on,
    // so Expectations can name the stretch since then.
    assert!(
        files.scenario.contains("# Reset pressed at 4"),
        "{}",
        files.scenario
    );
    assert!(files.scenario.contains("# The Arm switch went on at 2"));
}

#[test]
fn a_saved_flight_starts_landed_at_the_launch_spot_with_its_starting_state_spelled_out() {
    // Basis: Rule (ADR-0002: no hidden defaults). The Scenario is a Flight
    // Scenario that starts as Reset leaves the Quad, and every setting of the
    // flight is in it.
    let mut game = Game::new("freestyle-5", folder("start"));
    game.fly();
    let files = flight_files(&game.plan, &game.flight, "flight.csv").unwrap();
    let scenario = flown_again(&files, "the saved flight").scenario;
    let start = &scenario.start;
    assert_eq!(start.kind, opendrone_scenario::Kind::Flight);
    assert_eq!(start.quad.id, "opendrone/freestyle-5");
    assert_eq!(start.map.as_ref().unwrap().id, "test/flat-floor");
    assert_eq!(
        start.motors,
        Some(opendrone_scenario::StartingMotors::PoweringUp)
    );
    assert!(!start.armed);
    assert_eq!(start.battery, Some(1.0));
    assert_eq!(start.position, game.plan.launch_spot().position);
    assert_eq!(start.packet_rate, 250);
    assert_eq!(start.physics_rate.hz(), 8000);
    assert_eq!(start.rates, Rates::BETAFLIGHT_DEFAULT);
    // The Input Track's device is the DualSense over USB: 250 Hz, reporting
    // at rest.
    let device = match &scenario.inputs {
        opendrone_scenario::Inputs::Track(track) => track.device,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        device,
        InputDeviceFacts {
            report_rate: ReportRate::from_hz(250),
            reports_at_rest: true
        }
    );
}

#[test]
fn every_setting_of_the_flight_comes_back_from_the_saved_scenario_and_flies_the_same() {
    // Basis: Rule (ADR-0002). Settings the tracer never changes still make
    // it into the file, each Rates type among them, and the second flight still gives
    // the game's fingerprint.
    use opendrone_flight_controller::{AxisRates, RatesType, ThrottleLimitType};
    let axis = |rc_rate, srate, expo| AxisRates {
        rc_rate,
        srate,
        expo,
    };
    let rates = |rates_type, limit_type, limit| Rates {
        rates_type,
        roll: axis(100, 70, 20),
        pitch: axis(110, 72, 0),
        yaw: axis(90, 65, 5),
        rate_limit: [1998, 1500, 800],
        thr_mid: 40,
        thr_hover: 35,
        thr_expo: 10,
        throttle_limit_type: limit_type,
        throttle_limit_percent: limit,
        quickrates_rc_expo: true,
    };
    let cases = [
        rates(RatesType::Betaflight, ThrottleLimitType::Off, 100),
        rates(RatesType::Raceflight, ThrottleLimitType::Scale, 80),
        rates(RatesType::Kiss, ThrottleLimitType::Clip, 60),
        Rates {
            rate_limit: [1998; 3],
            ..rates(RatesType::Actual, ThrottleLimitType::Off, 100)
        },
        rates(RatesType::Quick, ThrottleLimitType::Scale, 25),
    ];
    let packs = common::packs();
    for (i, rates) in cases.into_iter().enumerate() {
        let mut settings = Settings::tracer("whoop-65");
        settings.rates = rates.clone();
        settings.packet_rate = PacketRate::from_hz(500).unwrap();
        settings.input_smoothing = i % 2 == 0;
        settings.endless_battery = i % 2 == 1;
        settings.auto_arm = i == 2;
        settings.random_seed = 7 + i as u64;
        let plan = FlightPlan::from_packs(&packs, settings).unwrap();
        let device = InputDeviceFacts {
            report_rate: None,
            reports_at_rest: false,
        };
        let mut flight = Flight::keeping(plan.simulation(Some(device)).unwrap(), device)
            .unwrap()
            .checking_every_step();
        let channels = |roll: f64, throttle: f64, arm: f64| Channels {
            roll: Channel::from_micros(roll),
            pitch: Channel::CENTRE,
            throttle: Channel::from_micros(throttle),
            yaw: Channel::CENTRE,
            arm: Channel::from_micros(arm),
            flight_mode: Channel::HIGH,
            crash_flip: Channel::LOW,
        };
        flight.input(
            SimulationTime::START,
            FlightInput::Channels(channels(1500.0, 988.0, 988.0)),
        );
        for tick in 1..=600_u64 {
            if tick == 250 {
                // Armed with the throttle up, which Auto-arm waits for.
                flight.input(
                    SimulationTime::from_ticks(tick * 8 + 3),
                    FlightInput::Channels(channels(1500.0, 1400.0, 2012.0)),
                );
            }
            if tick == 400 {
                flight.input(
                    SimulationTime::from_ticks(tick * 8),
                    FlightInput::Channels(channels(1731.4, 1400.0, 2012.0)),
                );
            }
            flight.tick();
        }
        let files = flight_files(&plan, &flight, "flight.csv").unwrap();
        let again = flown_again(&files, "the saved flight");
        let start = &again.scenario.start;
        assert_eq!(start.rates, rates, "case {i}: the Rates");
        assert_eq!(start.packet_rate, 500);
        assert_eq!(start.flight_mode, opendrone_scenario::FlightMode::Acro);
        assert_eq!(start.assists.input_smoothing, (i % 2 == 0));
        assert_eq!(start.assists.endless_battery, (i % 2 == 1));
        assert_eq!(start.assists.auto_arm, i == 2);
        assert_eq!(start.random_seed, 7 + i as u64);
        assert_eq!(
            Some(again.outcome.run_fingerprint()),
            files.run_fingerprint,
            "case {i}: the second flight gives the game's run fingerprint"
        );
    }
}

#[test]
fn every_receiver_step_is_written_to_the_input_track_and_read_back_as_the_same_step() {
    // Basis: Rule (ADR-0007: Channels are whole ELRS steps, 0 to 2047).
    // The Input Track writes a Channel in the µs a Calibration gives, which
    // the reader turns back into a step; no step may come back as another.
    let plan = common::plan("whoop-65");
    let device = InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    let mut flight = Flight::keeping(plan.simulation(Some(device)).unwrap(), device)
        .unwrap()
        .checking_every_step();
    let channels = |step: u16| {
        let step = Channel::from_step(step).unwrap();
        Channels {
            roll: step,
            pitch: step,
            throttle: step,
            yaw: step,
            arm: step,
            flight_mode: step,
            crash_flip: step,
        }
    };
    let mut written = Vec::new();
    for step in 0..=2047_u16 {
        let at = SimulationTime::from_ticks(u64::from(step) * 8);
        flight.input(at, FlightInput::Channels(channels(step)));
        written.push((at, FlightInput::Channels(channels(step))));
        flight.tick();
    }
    let files = flight_files(&plan, &flight, "flight.csv").unwrap();
    let scenario = flown_again(&files, "the saved flight").scenario;
    let opendrone_scenario::Inputs::Track(track) = &scenario.inputs else {
        panic!("a Scenario with an Input Track");
    };
    assert_eq!(track.inputs, written);
}

#[test]
fn inputs_still_waiting_to_arrive_are_left_out_because_they_have_changed_nothing_yet() {
    // Basis: Rule. An input stamped for a step the Simulation hasn't taken
    // can't have moved the Quad, so the saved flight ends before it, and
    // still flies again to the game's fingerprint.
    let plan = common::plan("whoop-65");
    let device = InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    let mut flight = Flight::keeping(plan.simulation(Some(device)).unwrap(), device)
        .unwrap()
        .checking_every_step();
    let rest = Channels {
        roll: Channel::CENTRE,
        pitch: Channel::CENTRE,
        throttle: Channel::LOW,
        yaw: Channel::CENTRE,
        arm: Channel::LOW,
        flight_mode: Channel::LOW,
        crash_flip: Channel::LOW,
    };
    flight.input(SimulationTime::START, FlightInput::Channels(rest));
    for _ in 0..50 {
        flight.tick();
    }
    let now = flight.simulation().time();
    flight.input(
        SimulationTime::from_ticks(now.ticks() + 5),
        FlightInput::Channels(Channels {
            roll: Channel::HIGH,
            ..rest
        }),
    );
    let files = flight_files(&plan, &flight, "flight.csv").unwrap();
    assert_eq!(files.steps, 400);
    assert_eq!(
        files.track.lines().filter(|l| !l.starts_with('#')).count(),
        2
    );
    let again = flown_again(&files, "the saved flight");
    assert_eq!(Some(again.outcome.run_fingerprint()), files.run_fingerprint);
}

#[test]
fn there_is_nothing_to_save_before_an_input_device_has_started_the_flight() {
    let plan = common::plan("whoop-65");
    let flight = Flight::new(plan.simulation(None).unwrap()).unwrap();
    let problem = flight_files(&plan, &flight, "flight.csv").unwrap_err();
    assert!(problem.contains("no flight to save"), "{problem}");
}

/// A flight on a DualSense-like device with its Flight Inputs kept up to
/// `cap`, and the Channels it sends: sticks at rest, or the Arm switch on.
fn capped_flight(plan: &FlightPlan, cap: usize) -> Flight {
    let device = InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    Flight::keeping_at_most(plan.simulation(Some(device)).unwrap(), device, cap)
        .unwrap()
        .checking_every_step()
}

fn rest_with_arm(on: bool) -> Channels {
    Channels {
        roll: Channel::CENTRE,
        pitch: Channel::CENTRE,
        throttle: Channel::LOW,
        yaw: Channel::CENTRE,
        arm: if on { Channel::HIGH } else { Channel::LOW },
        flight_mode: Channel::LOW,
        crash_flip: Channel::LOW,
    }
}

#[test]
fn after_a_very_long_flight_with_many_resets_and_arms_the_recorder_holds_no_more_than_its_span() {
    // Basis: Rule (#187). Save Flight can only write a flight from its start,
    // because Reset and arming don't start the Simulation afresh (#168), so
    // the span the recorder keeps is the flight's first MAX_KEPT_INPUTS Flight
    // Inputs, however many Resets and arms come after them.
    let plan = common::plan("whoop-65");
    let cap = 400;
    let mut flight = capped_flight(&plan, cap);
    flight.input(
        SimulationTime::START,
        FlightInput::Channels(rest_with_arm(false)),
    );
    let mut most_held = 0;
    for tick in 1..=6_000_u64 {
        let at = SimulationTime::from_ticks(tick * 8);
        let input = match tick % 10 {
            0 => FlightInput::Reset,
            _ => FlightInput::Channels(rest_with_arm(tick % 20 < 10)),
        };
        flight.input(at, input);
        flight.tick();
        most_held = most_held.max(flight.kept().unwrap().inputs.len());
    }
    assert!(
        most_held <= cap,
        "the recorder held {most_held} Flight Inputs at most, of {cap}"
    );
    assert!(flight.kept().unwrap().inputs.is_empty());
    let problem = flight_files(&plan, &flight, "flight.csv").unwrap_err();
    assert!(problem.contains("too long to save"), "{problem}");
}

#[test]
fn the_default_recorder_holds_at_most_a_million_flight_inputs_however_many_it_is_handed() {
    // Basis: Rule (#187): about 24 MB at most.
    let plan = common::plan("whoop-65");
    let device = InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    let mut flight = Flight::keeping(plan.simulation(Some(device)).unwrap(), device).unwrap();
    assert_eq!(opendrone::stepping::MAX_KEPT_INPUTS, 1_000_000);
    for _ in 0..=opendrone::stepping::MAX_KEPT_INPUTS {
        flight.input(
            SimulationTime::START,
            FlightInput::Channels(rest_with_arm(false)),
        );
    }
    assert!(flight.kept().unwrap().inputs.len() <= opendrone::stepping::MAX_KEPT_INPUTS);
}

#[test]
fn a_flight_that_fills_the_recorder_exactly_still_saves_and_flies_again_to_the_same_fingerprint() {
    // Basis: Rule (ADR-0001): inside the span, nothing is left out.
    let plan = common::plan("whoop-65");
    let cap = 100;
    let mut flight = capped_flight(&plan, cap);
    for n in 0..cap as u64 {
        flight.input(
            SimulationTime::from_ticks(n * 80),
            FlightInput::Channels(rest_with_arm(n % 2 == 1)),
        );
        for _ in 0..10 {
            flight.tick();
        }
    }
    assert_eq!(flight.kept().unwrap().inputs.len(), cap);
    let files = flight_files(&plan, &flight, "flight.csv").unwrap();
    assert_eq!(
        files.track.lines().filter(|l| !l.starts_with('#')).count(),
        cap + 1,
        "the header and a row for each Flight Input"
    );
    let again = flown_again(&files, "the saved flight");
    assert_eq!(Some(again.outcome.run_fingerprint()), files.run_fingerprint);

    // One more and it's too long, and says so instead of saving a flight
    // that starts late.
    flight.input(
        SimulationTime::from_ticks(cap as u64 * 80),
        FlightInput::Reset,
    );
    assert_eq!(flight.kept().unwrap().inputs.len(), 0);
    assert!(flight_files(&plan, &flight, "flight.csv").is_err());
}

#[test]
fn the_save_flight_action_writes_a_scenario_and_its_input_track_and_never_overwrites_one() {
    let saves = folder("action");
    let mut game = Game::new("whoop-65", saves.clone());
    game.fly();
    // The touchpad is bound to Save Flight in these profiles. Press it twice.
    game.press(10_004, PadButton::Touchpad);
    game.reporting(10_008, 10_100, 4);
    game.press(10_104, PadButton::Touchpad);
    let mut names: Vec<String> = std::fs::read_dir(&saves)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "whoop-65-saved-flight-1.csv",
            "whoop-65-saved-flight-1.toml",
            "whoop-65-saved-flight-2.csv",
            "whoop-65-saved-flight-2.toml",
        ]
    );
    // Each file pair reads back and flies to a fingerprint the game had
    // while it flew: the second is the first with 100 ms more.
    let read = |n: u32| {
        let toml =
            std::fs::read_to_string(saves.join(format!("whoop-65-saved-flight-{n}.toml"))).unwrap();
        let csv =
            std::fs::read_to_string(saves.join(format!("whoop-65-saved-flight-{n}.csv"))).unwrap();
        let mut scenario = read_scenario("saved", &toml).unwrap_or_else(|p| panic!("{p}"));
        assert_eq!(
            scenario.input_track(),
            Some(format!("whoop-65-saved-flight-{n}.csv").as_str())
        );
        scenario
            .read_input_track("saved.csv", &csv)
            .unwrap_or_else(|p| panic!("{p}"));
        (
            toml,
            run(&scenario, &common::packs()).unwrap_or_else(|p| panic!("{p}")),
        )
    };
    let (first_text, first) = read(1);
    let (_, second) = read(2);
    assert!(second.step_fingerprints.len() > first.step_fingerprints.len());
    // The file says the whole state's fingerprint the game had after its last
    // step, and the runner agrees.
    let last = first.step_fingerprints.last().unwrap();
    assert!(
        first_text.contains(&format!("was {last}")),
        "the Scenario names the game's fingerprint {last}: {first_text}"
    );
    let _ = std::fs::remove_dir_all(&saves);
}

#[test]
fn a_saved_flight_is_one_a_watched_scenario_can_open() {
    // The two developer commands meet: a flight saved from the game opens in
    // the game again, as a Scenario with its Input Track, and flown by the
    // game's stepping it ends on the fingerprint it was saved with.
    let mut game = Game::new("whoop-65", folder("watch"));
    game.fly();
    let files = flight_files(&game.plan, &game.flight, "flight.csv").unwrap();
    let scenario = flown_again(&files, "the saved flight").scenario;
    let watched =
        opendrone::watch::watch_scenario("the saved flight", &scenario, &common::packs()).unwrap();
    let mut flight = watched.flight;
    while !flight.finished() {
        flight.tick();
    }
    assert_eq!(flight.run_fingerprint(), files.run_fingerprint);
}
