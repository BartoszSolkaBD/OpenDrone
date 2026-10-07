//! Readable checks that feed the recorded device traces from #18, #27 and
//! #30 (committed, trimmed, in `tests/traces/`) through the built-in Pack's
//! Input Device profiles, exactly as the game does: the input thread's
//! batches into `Inputs`, Channels out.
//!
//! Basis: Observed. Each trace is the maintainer's own recording on the dev
//! Mac (SDL 3.4.18, 2026-10-04); the expected values come from what the
//! trace says the device did at that moment, in µs.

mod common;

use std::time::Duration;

use common::{DEVICE, Trace, channels, channels_at, profile, replay, secs};
use opendrone_input::calibration::{
    EndsSeen, centre_and_deadband, gamepad_stick, rests_after_flicks,
};
use opendrone_input::channels::{CENTRE_US, HIGH_US, LOW_US};
use opendrone_input::controls::{PadAxis, PadButton, Stick, Trigger};
use opendrone_input::profile::{Action, FlightMode, GamepadThrottle, Setup, ThrottleZero};
use opendrone_input::transmitting::{Transmitting, still_transmitting};
use opendrone_input::{Batch, DeviceId, InputEvent, Inputs, Lost, PilotCopy, Raw};

fn pocket() -> (Trace, Inputs, Vec<InputEvent>) {
    let trace = Trace::read("pocket-rf-off-18.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    let events = replay(&mut inputs, trace.batches());
    (trace, inputs, events)
}

#[test]
fn the_pocket_is_found_as_a_radio_and_matched_to_its_own_profile() {
    let (_, _, events) = pocket();
    let InputEvent::Found {
        name,
        kind,
        profile,
        ..
    } = &events[0]
    else {
        panic!("the first event is the Pocket being found: {:?}", events[0]);
    };
    assert_eq!(name, "EdgeTX Radiomaster Pocket Joystick");
    assert_eq!(*kind, opendrone_input::Kind::Radio);
    assert_eq!(profile.as_deref(), Some("opendrone/radiomaster-pocket"));
}

#[test]
fn the_pockets_ch1_to_ch4_move_roll_pitch_throttle_and_yaw_and_nothing_else() {
    let trace = Trace::read("pocket-rf-off-18.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    let mut checked = 0;
    for batch in trace.batches() {
        let before = inputs.devices().first().map(|d| d.channels());
        let moved: Vec<u8> = batch
            .events
            .iter()
            .filter_map(|raw| match raw {
                Raw::Axis { axis, .. } => Some(*axis),
                _ => None,
            })
            .collect();
        inputs.take(batch);
        let (Some(before), [axis]) = (before, moved.as_slice()) else {
            continue;
        };
        let after = inputs.devices()[0].channels();
        let changed = [
            before.roll != after.roll,
            before.pitch != after.pitch,
            before.throttle != after.throttle,
            before.yaw != after.yaw,
        ];
        let expected = match axis {
            0 => [true, false, false, false],
            1 => [false, true, false, false],
            2 => [false, false, true, false],
            3 => [false, false, false, true],
            _ => [false; 4],
        };
        assert_eq!(
            changed, expected,
            "axis {axis} moved, and only its stick should follow"
        );
        checked += 1;
    }
    assert!(
        checked > 1000,
        "only {checked} single-axis moves were checked"
    );
}

#[test]
fn the_pockets_throttle_rests_at_988_us_and_its_sticks_at_centre() {
    let (_, _, events) = pocket();
    let first = channels(&events)[0].1;
    assert_eq!(first.throttle, LOW_US);
    // EdgeTX 2.10 puts centre 15 SDL units above SDL's 0 (#18): 0.23 µs at
    // the Pack's starting values, "trust the radio".
    for stick in [first.roll, first.pitch, first.yaw] {
        assert!(
            (stick - CENTRE_US).abs() < 0.25,
            "a resting stick read {stick} µs"
        );
    }
}

#[test]
fn the_pockets_switches_arm_on_ch5_pick_the_flight_mode_on_ch6_and_crash_flip_on_ch7() {
    let (_, _, events) = pocket();
    let at = |s: f64| channels_at(&events, secs(s));
    // Before any switch moves: CH5–CH7 low.
    assert_eq!(at(20.0).armed(), Some(false));
    assert_eq!(at(20.0).mode(), Some(FlightMode::Acro));
    assert_eq!(at(20.0).crash_flipping(), Some(false));
    // CH5 high at 20.114 s, low again at 20.896 s.
    assert_eq!(at(20.2).arm, Some(HIGH_US));
    assert_eq!(at(21.0).arm, Some(LOW_US));
    // CH7 high at 29.800 s, back through the middle at 31.351 s.
    assert_eq!(at(29.9).crash_flip, Some(HIGH_US));
    assert_eq!(at(31.36).crash_flip, Some(LOW_US));
    // CH6 high at 30.289 s, middle at 31.067 s, low at 31.817 s.
    assert_eq!(at(30.3).mode(), Some(FlightMode::Angle));
    assert_eq!(at(30.3).flight_mode, Some(HIGH_US));
    assert_eq!(at(31.1).mode(), Some(FlightMode::Horizon));
    assert_eq!(at(31.1).flight_mode, Some(CENTRE_US));
    assert_eq!(at(31.9).mode(), Some(FlightMode::Acro));
}

#[test]
fn the_pockets_se_switch_on_ch9_holds_reset_while_pressed() {
    let trace = Trace::read("pocket-rf-off-18.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    let mut presses = Vec::new();
    let mut was_held = false;
    for batch in trace.batches() {
        let at = batch.at;
        inputs.take(batch);
        let held = inputs.devices()[0].actions_held() == [Action::Reset];
        if held && !was_held {
            presses.push(at.as_millis());
        }
        was_held = held;
    }
    // SE pressed 23.4 s in, and four times between 35.0 and 36.2 s.
    assert_eq!(presses, [23_422, 34_986, 35_507, 35_765, 35_995]);
}

#[test]
fn values_that_arrive_with_the_pockets_unplug_are_thrown_away() {
    let (trace, inputs, events) = pocket();
    let unplugged = trace.rows.last().unwrap();
    assert_eq!(unplugged.kind, "removed");
    // In the same poll as the removal, SDL sent CH6 and CH7 back to centre
    // (#18). CH6 is the Flight Mode switch: that would have flipped Acro to
    // Horizon as the Pocket went.
    let same_instant: Vec<_> = trace
        .rows
        .iter()
        .filter(|r| r.at == unplugged.at && r.kind == "axis")
        .map(|r| (r.index, r.value))
        .collect();
    assert_eq!(same_instant, [(5, 15), (6, 15)]);
    assert!(
        events.contains(&InputEvent::Lost {
            device: DeviceId(0),
            at: unplugged.at,
            why: Lost::Unplugged,
        }),
        "the unplug is handed over as lost"
    );
    assert!(
        channels(&events).iter().all(|(at, _)| *at != unplugged.at),
        "no Channels arrive with the unplug"
    );
    assert_eq!(
        inputs.devices()[0].channels().mode(),
        Some(FlightMode::Acro)
    );

    // Had they been kept, the Flight Mode would have jumped.
    let mut kept = Inputs::new(common::built_in_profiles());
    let mut batches = trace.batches();
    let last = batches.pop().unwrap();
    let values: Vec<Raw> = last.events.into_iter().filter(|e| e.is_value()).collect();
    batches.push(Batch {
        at: last.at,
        events: values,
    });
    replay(&mut kept, batches);
    assert_eq!(
        kept.devices()[0].channels().mode(),
        Some(FlightMode::Horizon)
    );
}

#[test]
fn a_resting_pocket_that_sends_nothing_is_never_lost() {
    let (trace, _, events) = pocket();
    // The hands-off step: the Pocket sent nothing at all for over five
    // seconds, and the game checked for silence every 10 ms throughout.
    let longest_silence = trace
        .rows
        .windows(2)
        .map(|pair| pair[1].at - pair[0].at)
        .max()
        .unwrap();
    assert!(
        longest_silence > Duration::from_secs(5),
        "{longest_silence:?}"
    );
    let lost: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, InputEvent::Lost { .. }))
        .collect();
    assert_eq!(lost.len(), 1, "lost only once, when unplugged: {lost:?}");
}

/// The DualSense calibrated from the #27 hands-off moments after fast
/// circles: each stick's centre and deadband from where it came to rest,
/// its ends from the circles.
fn dualsense_calibrated() -> PilotCopy {
    let let_go = Trace::read("dualsense-let-go-27.csv");
    let pack = profile("opendrone/dualsense");
    let Setup::Gamepad(mut setup) = pack.setup.clone() else {
        panic!("the DualSense is a Gamepad");
    };
    for stick in Stick::ALL {
        let values = let_go.axis(stick.pad_axis().index());
        let rests = rests_after_flicks(&values, let_go.end()).expect("the stick was flicked");
        let (centre, deadband) = centre_and_deadband(&rests).unwrap();
        let mut ends = EndsSeen::new();
        for (_, value) in &values {
            ends.see(&[*value]);
        }
        *setup.calibration.stick_mut(stick) = gamepad_stick(centre, deadband, ends.of(0));
    }
    PilotCopy {
        setup: Setup::Gamepad(setup),
        calibrated: true,
        measured_report_rate: Default::default(),
    }
}

fn dualsense_at_rest(copy: Option<PilotCopy>) -> Vec<InputEvent> {
    let trace = Trace::read("dualsense-at-rest-27.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    if let Some(copy) = copy {
        inputs.set_copy(trace.device.model(), Some(copy), Duration::ZERO);
    }
    replay(&mut inputs, trace.batches())
}

#[test]
fn the_dualsenses_right_stick_flickers_one_step_at_rest_in_the_recording() {
    let trace = Trace::read("dualsense-at-rest-27.csv");
    let right_x: Vec<i16> = trace
        .axis(PadAxis::RightX.index())
        .iter()
        .map(|v| v.1)
        .collect();
    let mut seen = right_x.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        [2955, 3212],
        "one 8-bit step (257), 9–10 % right of centre"
    );
    assert!(
        right_x.len() > 150,
        "it flickered {} times in 6 s",
        right_x.len()
    );
}

#[test]
fn the_dualsenses_rest_flicker_is_removed_by_its_calibrated_deadband() {
    let copy = dualsense_calibrated();
    let events = dualsense_at_rest(Some(copy));
    let all = channels(&events);
    // Roll, pitch and yaw hold exactly centre for the whole six seconds,
    // through 181 flickers of the right stick.
    for (_, c) in &all {
        assert_eq!((c.roll, c.pitch, c.yaw), (CENTRE_US, CENTRE_US, CENTRE_US));
    }
    // The deadband never acts on the throttle (#19 §3). The left stick
    // flickers once, one step above its rest: with zero at rest that's 8 µs
    // of throttle, far below Betaflight's min_check (1050 µs), where the
    // Flight Controller still reads no throttle.
    assert!(all.len() <= 3, "{} Channels changes", all.len());
    for (_, c) in &all {
        assert!(c.throttle < 1050.0, "{} µs", c.throttle);
    }
}

#[test]
fn uncalibrated_the_dualsenses_rest_flicker_reaches_the_roll_channel() {
    // The Pack's starting values: centre 0 %, deadband 5 %. The right stick
    // rests 9–10 % off centre, past the deadband, so roll reads about 22–26 µs
    // right and flickers (#27's "uncalibrated" row).
    let events = dualsense_at_rest(None);
    let mut rolls: Vec<f64> = channels(&events).iter().map(|(_, c)| c.roll).collect();
    assert!(rolls.len() > 150, "roll changed {} times", rolls.len());
    rolls.sort_by(f64::total_cmp);
    rolls.dedup();
    assert_eq!(rolls.len(), 2);
    assert!(
        rolls.iter().all(|r| (1521.0..1527.0).contains(r)),
        "{rolls:?}"
    );
}

#[test]
fn the_dualsenses_calibration_from_its_releases_matches_19s_numbers() {
    let PilotCopy {
        setup: Setup::Gamepad(setup),
        ..
    } = dualsense_calibrated()
    else {
        panic!("a Gamepad setup")
    };
    let right_x = setup.calibration.right_x;
    // The right stick rests 9–11 % off centre, with a deadband of about 2–3 %
    // (#19 §3), in SDL's units of 1/32768.
    let percent = |v: f64| v / 32768.0 * 100.0;
    assert!(
        (9.0..11.0).contains(&percent(right_x.centre)),
        "{right_x:?}"
    );
    assert!(
        (1.0..3.0).contains(&percent(right_x.deadband)),
        "{right_x:?}"
    );
}

#[test]
fn a_dualsense_that_keeps_reporting_is_never_lost_while_it_rests() {
    let events = dualsense_at_rest(None);
    assert!(
        !events.iter().any(|e| matches!(e, InputEvent::Lost { .. })),
        "its motion sensors report 250 times a second"
    );
}

#[test]
fn a_dualsense_silent_for_one_second_is_lost_and_back_when_it_reports() {
    let trace = Trace::read("dualsense-at-rest-27.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    let batches = trace.batches();
    // The recording up to 2 s in, then nothing: as if it froze.
    let cut = batches[0].at + Duration::from_secs(2);
    let (before, after): (Vec<Batch>, Vec<Batch>) = batches.into_iter().partition(|b| b.at < cut);
    let last_heard = before.last().unwrap().at;
    replay(&mut inputs, before);
    assert!(
        inputs
            .check_silence(last_heard + Duration::from_millis(990))
            .is_empty()
    );
    assert_eq!(
        inputs.check_silence(last_heard + Duration::from_secs(1)),
        [InputEvent::Lost {
            device: DeviceId(0),
            at: last_heard + Duration::from_secs(1),
            why: Lost::Silent
        }]
    );
    // It speaks again 3.5 s later: back.
    let resumed = Batch {
        at: after.last().unwrap().at,
        events: vec![Raw::Report { device: DEVICE }],
    };
    assert_eq!(
        inputs.take(resumed.clone()),
        [InputEvent::Back {
            device: DeviceId(0),
            at: resumed.at
        }]
    );
}

fn dualsense_buttons(copy: Option<PilotCopy>) -> (Inputs, Vec<InputEvent>) {
    let trace = Trace::read("dualsense-buttons-27.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    if let Some(copy) = copy {
        inputs.set_copy(trace.device.model(), Some(copy), Duration::ZERO);
    }
    let events = replay(&mut inputs, trace.batches());
    (inputs, events)
}

#[test]
fn the_dualsenses_r1_toggles_arm_and_l1_holds_crash_flip() {
    let (mut inputs, events) = dualsense_buttons(None);
    let at = |s: f64| channels_at(&events, secs(s));
    assert_eq!(at(75.0).armed(), Some(false));
    assert_eq!(at(75.0).crash_flipping(), Some(false));
    // L1 down at 75.148 s: Crash Flip on while held.
    assert_eq!(at(75.2).crash_flipping(), Some(true));
    // R1 down at 75.628 s: armed.
    assert_eq!(at(75.7).armed(), Some(true));
    // L1 up at 76.024 s: Crash Flip off.
    assert_eq!(at(76.1).crash_flipping(), Some(false));
    // R1 up at 76.324 s: Arm stays on; it toggles.
    assert_eq!(at(76.4).armed(), Some(true));
    assert_eq!(at(84.0).armed(), Some(true));

    // The game reports a disarm (a Failsafe drop, say): Arm turns itself off.
    let disarm = inputs.disarmed(secs(84.3));
    assert_eq!(channels(&disarm)[0].1.armed(), Some(false));
    // So the next press of R1 means "arm now".
    let press = |down| Batch {
        at: secs(if down { 84.5 } else { 84.6 }),
        events: vec![Raw::PadButton {
            device: DEVICE,
            button: PadButton::RightShoulder,
            down,
        }],
    };
    assert_eq!(channels(&inputs.take(press(true)))[0].1.armed(), Some(true));
    assert!(channels(&inputs.take(press(false))).is_empty());
}

#[test]
fn the_dualsenses_options_holds_pause_and_create_holds_reset() {
    let trace = Trace::read("dualsense-buttons-27.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    let mut held = Vec::new();
    for batch in trace.batches() {
        inputs.take(batch);
        for action in inputs.devices()[0].actions_held() {
            if !held.contains(&action) {
                held.push(action);
            }
        }
    }
    // Create pressed at 81.884 s, Options at 82.332 s.
    assert_eq!(held, [Action::Reset, Action::Pause]);
}

/// The DualSense's Pack profile with another throttle style.
fn with_throttle(throttle: GamepadThrottle) -> PilotCopy {
    let pack = profile("opendrone/dualsense");
    let mut copy = PilotCopy::of(&pack);
    let Setup::Gamepad(setup) = &mut copy.setup else {
        panic!("a Gamepad")
    };
    setup.channels.throttle = throttle;
    copy
}

#[test]
fn an_r2_trigger_throttle_runs_from_988_to_2012_us_with_the_squeeze() {
    let (_, events) = dualsense_buttons(Some(with_throttle(GamepadThrottle::Trigger(
        Trigger::Right,
    ))));
    let at = |s: f64| channels_at(&events, secs(s)).throttle;
    // R2 squeezed from 76.896 s to 76.916 s, let go by 77.288 s.
    assert_eq!(at(76.8), LOW_US);
    assert_eq!(at(77.0), HIGH_US);
    assert_eq!(at(77.3), LOW_US);
    // Every step of the squeeze moves the throttle up.
    let squeeze: Vec<f64> = channels(&events)
        .iter()
        .filter(|(t, _)| (secs(76.89)..secs(76.92)).contains(t))
        .map(|(_, c)| c.throttle)
        .collect();
    assert!(squeeze.windows(2).all(|w| w[1] > w[0]), "{squeeze:?}");
}

fn let_go_throttles(zero: ThrottleZero) -> Vec<(i16, f64)> {
    let trace = Trace::read("dualsense-let-go-27.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    inputs.set_copy(
        trace.device.model(),
        Some(with_throttle(GamepadThrottle::Stick {
            stick: Stick::LeftY,
            zero,
        })),
        Duration::ZERO,
    );
    let mut pairs = Vec::new();
    for batch in trace.batches() {
        inputs.take(batch);
        let device = &inputs.devices()[0];
        let left_y = device.state().pad.unwrap().axis(PadAxis::LeftY);
        pairs.push((left_y, device.channels().throttle));
    }
    pairs
}

#[test]
fn zero_at_rest_throttle_ignores_the_stick_pulled_down() {
    // Fast circles take the left stick all round: SDL reads up as negative.
    let pairs = let_go_throttles(ThrottleZero::AtRest);
    assert!(
        pairs.iter().any(|(y, _)| *y == i16::MIN),
        "the stick reached full up"
    );
    for (left_y, throttle) in &pairs {
        if *left_y >= 0 {
            assert_eq!(*throttle, LOW_US, "pulled down or at rest is no throttle");
        }
    }
    assert!(
        pairs.iter().any(|(_, t)| *t == HIGH_US),
        "full up is full throttle"
    );
}

#[test]
fn full_travel_throttle_reads_half_at_rest_and_none_at_the_bottom() {
    let pairs = let_go_throttles(ThrottleZero::AtBottom);
    let (_, at_rest) = pairs.last().unwrap();
    assert!((at_rest - CENTRE_US).abs() < 10.0, "at rest: {at_rest} µs");
    assert!(
        pairs.iter().any(|(_, t)| *t == LOW_US),
        "full down is no throttle"
    );
    assert!(
        pairs.iter().any(|(_, t)| *t == HIGH_US),
        "full up is full throttle"
    );
}

/// The moments the Pocket's stick channels changed, through `Inputs`.
fn stick_changes(batches: Vec<Batch>) -> Vec<Duration> {
    let mut inputs = Inputs::new(common::built_in_profiles());
    let mut changes = Vec::new();
    for batch in batches {
        for event in inputs.take(batch) {
            if let InputEvent::Channels { at, .. } = event {
                changes.push(at);
            }
        }
    }
    changes
}

#[test]
fn a_pocket_with_rf_off_isnt_transmitting_in_every_recording() {
    for name in [
        "pocket-rf-off-18.csv",
        "pocket-adc-filter-off-30.csv",
        "pocket-adc-filter-on-30.csv",
    ] {
        let changes = stick_changes(Trace::read(name).batches());
        let verdict = still_transmitting(&changes);
        assert!(
            matches!(verdict, Transmitting::No { closest } if closest < Duration::from_millis(2)),
            "{name}: {verdict:?}"
        );
    }
}

/// The #30 slow circles (ADC filter Off) as EdgeTX would send them with RF
/// on at 250 Hz: one report per ELRS packet, every 4 ms, carrying the stick
/// values of that moment, reaching the input thread 0–0.66 ms after it's due
/// (the arrival spread ADR-0020 measured). Not a recording: no RF-on trace
/// exists. Basis: Rule, from EdgeTX's source (#30).
fn as_if_rf_on_at_250_hz(trace: &Trace) -> Vec<Batch> {
    let mut batches = trace.batches();
    let added = batches.remove(0);
    let first = added.at;
    let mut state = trace.start.axes.clone();
    let mut out = vec![Batch {
        at: added.at,
        events: added.events.into_iter().filter(|e| !e.is_value()).collect(),
    }];
    let mut sent = state.clone();
    let mut next = 0;
    let rows: Vec<_> = trace.rows.iter().filter(|r| r.kind == "axis").collect();
    let mut packet = 1u64;
    loop {
        let due = first + Duration::from_millis(4 * packet);
        if due > trace.end() {
            break;
        }
        while next < rows.len() && rows[next].at <= due {
            state[rows[next].index] = i16::try_from(rows[next].value).unwrap();
            next += 1;
        }
        let late = Duration::from_micros((packet * 37 % 67) * 10);
        let events: Vec<Raw> = (0..state.len())
            .filter(|&a| state[a] != sent[a])
            .map(|a| Raw::Axis {
                device: DEVICE,
                axis: u8::try_from(a).unwrap(),
                value: state[a],
            })
            .collect();
        if !events.is_empty() {
            out.push(Batch {
                at: due + late,
                events,
            });
        }
        sent.clone_from(&state);
        packet += 1;
    }
    out
}

#[test]
fn a_pocket_sending_once_per_250_hz_packet_counts_as_still_transmitting() {
    let trace = Trace::read("pocket-adc-filter-off-30.csv");
    let changes = stick_changes(as_if_rf_on_at_250_hz(&trace));
    assert!(changes.len() > 300, "{} changes", changes.len());
    let verdict = still_transmitting(&changes);
    assert!(
        matches!(verdict, Transmitting::Yes { closest } if closest > Duration::from_millis(3)),
        "{verdict:?}"
    );
}

#[test]
fn the_dualsense_is_found_as_a_gamepad_with_its_own_profile() {
    let (inputs, _) = dualsense_buttons(None);
    let device = &inputs.devices()[0];
    assert_eq!(device.kind(), opendrone_input::Kind::Gamepad);
    assert_eq!(device.profile().unwrap().id, "opendrone/dualsense");
    assert!(!device.calibrated());
}
