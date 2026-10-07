//! Readable checks on the Calibration maths (#19 §3).
//!
//! Basis: Rule, from #19 §3, unless a check says it's Observed from a
//! recorded trace. Values are SDL's axis units (−32768…32767) and µs.

mod common;

use std::time::Duration;

use common::{Trace, channels, profile, replay, secs};
use opendrone_input::calibration::{
    ChannelOrder, EndsSeen, RadioMove, RadioRest, centre_and_deadband, gamepad_stick,
    radio_calibration, rests_after_flicks,
};
use opendrone_input::channels::{CENTRE_US, HIGH_US, LOW_US, stick_deflection, stick_us};
use opendrone_input::profile::{RadioChannels, RadioStick, Setup};
use opendrone_input::{Inputs, PilotCopy};

/// The Pocket's axes with sticks centred and the throttle down, as EdgeTX
/// 2.10 sends them (#18): centre is SDL's +15.
const REST: [i16; 8] = [15, 15, -32768, 15, -32768, -32768, -32768, -32768];

fn frames(moves: &[[i16; 8]]) -> Vec<Vec<i16>> {
    moves.iter().map(|m| m.to_vec()).collect()
}

/// Runs step 2 over the frames, returning the moves seen in order.
fn order(rest: [i16; 8], moves: &[[i16; 8]]) -> (Vec<RadioMove>, Option<RadioChannels>) {
    let mut order = ChannelOrder::new(&RadioRest(rest.to_vec()));
    let mut seen = Vec::new();
    for frame in frames(moves) {
        seen.extend(order.see(&frame));
    }
    (seen, order.channels())
}

#[test]
fn a_radio_calibration_finds_aetr_from_throttle_up_yaw_right_pitch_up_roll_right() {
    let mut f = REST;
    let mut moves = Vec::new();
    f[2] = 32767; // throttle up, CH3
    moves.push(f);
    f[3] = 32767; // yaw right, CH4
    moves.push(f);
    f[3] = 15;
    f[1] = 32767; // pitch up, CH2
    moves.push(f);
    f[1] = 15;
    f[0] = 32767; // roll right, CH1
    moves.push(f);
    let (seen, channels) = order(REST, &moves);
    assert_eq!(seen, RadioMove::IN_ORDER);
    assert_eq!(channels, Some(RadioChannels::aetr()));
}

#[test]
fn a_radio_calibration_catches_reta_and_a_reversed_channel() {
    // Official EdgeTX builds on fresh settings send RETA: yaw on CH1,
    // pitch on CH2, throttle on CH3, roll on CH4. Here pitch also runs
    // reversed.
    let rest = [15, 15, -32768, 15, 0, 0, 0, 0];
    let mut f = rest;
    let mut moves = Vec::new();
    f[2] = 32767; // throttle up
    moves.push(f);
    f[0] = 32767; // yaw right
    moves.push(f);
    f[1] = -32768; // pitch up, reversed
    moves.push(f);
    f[3] = 32767; // roll right
    moves.push(f);
    let (_, channels) = order(rest, &moves);
    assert_eq!(
        channels,
        Some(RadioChannels {
            yaw: RadioStick::on(1),
            pitch: RadioStick {
                channel: 2,
                reverse: true
            },
            throttle: RadioStick::on(3),
            roll: RadioStick::on(4),
        })
    );
}

#[test]
fn a_radio_calibration_moves_on_only_once_a_stick_has_moved_half_way_from_centre_to_an_end() {
    let throttle = |value| {
        let mut f = REST;
        f[2] = value;
        f
    };
    // 16384 is half of SDL's 32768 from centre to an end.
    assert_eq!(order(REST, &[throttle(-32768 + 16383)]).0, []);
    assert_eq!(
        order(REST, &[throttle(-32768 + 16384)]).0,
        [RadioMove::ThrottleUp]
    );
}

#[test]
fn the_pocket_calibrated_from_its_own_recording_lands_full_stick_at_988_and_2012_and_centre_at_1500()
 {
    // Basis: Observed. Step 1 is the Pocket's values when it was plugged in
    // (sticks centred, throttle down); step 3 is every stick move in the
    // recording, which reached each stick's ends.
    let trace = Trace::read("pocket-rf-off-18.csv");
    let rest = RadioRest(trace.start.axes.clone());
    let mut ends = EndsSeen::new();
    let mut axes = trace.start.axes.clone();
    for row in trace.rows.iter().filter(|r| r.kind == "axis") {
        axes[row.index] = i16::try_from(row.value).unwrap();
        ends.see(&axes);
    }
    let calibration = radio_calibration(&rest, &RadioChannels::aetr(), &ends).unwrap();
    let pocket = profile("opendrone/radiomaster-pocket");
    let Setup::Radio(mut setup) = pocket.setup.clone() else {
        panic!("a Radio")
    };
    setup.calibration = calibration;
    let mut inputs = Inputs::new(common::built_in_profiles());
    inputs.set_copy(
        trace.device.model(),
        Some(PilotCopy {
            setup: Setup::Radio(setup),
            calibrated: true,
            measured_report_rate: Default::default(),
        }),
        Duration::ZERO,
    );
    let events = replay(&mut inputs, trace.batches());
    let all = channels(&events);
    let first = all[0].1;
    assert_eq!(
        (first.roll, first.pitch, first.yaw),
        (CENTRE_US, CENTRE_US, CENTRE_US)
    );
    assert_eq!(first.throttle, LOW_US);
    let reaches = |pick: fn(&opendrone_input::Channels) -> f64| {
        let values: Vec<f64> = all.iter().map(|(_, c)| pick(c)).collect();
        let low = values.iter().copied().fold(f64::MAX, f64::min);
        let high = values.iter().copied().fold(f64::MIN, f64::max);
        (low, high)
    };
    assert_eq!(reaches(|c| c.roll), (LOW_US, HIGH_US));
    assert_eq!(reaches(|c| c.pitch), (LOW_US, HIGH_US));
    assert_eq!(reaches(|c| c.yaw), (LOW_US, HIGH_US));
    assert_eq!(reaches(|c| c.throttle), (LOW_US, HIGH_US));
}

#[test]
fn a_radio_calibration_isnt_finished_until_every_stick_reached_both_ends() {
    let rest = RadioRest(REST.to_vec());
    let mut ends = EndsSeen::new();
    ends.see(&REST);
    let mut f = REST;
    f[0] = 32767;
    f[2] = 32767;
    ends.see(&f);
    let unfinished = radio_calibration(&rest, &RadioChannels::aetr(), &ends).unwrap_err();
    assert!(unfinished.0.contains("roll"), "{}", unfinished.0);
}

#[test]
fn a_gamepad_centre_is_the_middle_of_its_rests_and_its_deadband_half_their_spread_plus_one_step() {
    // The #19 example: the right stick rests 9–11 % off centre.
    let rests = [2955, 3212, 3469, 3212];
    let (centre, deadband) = centre_and_deadband(&rests).unwrap();
    assert_eq!(centre, 3212.0);
    assert_eq!(deadband, 257.0 + 257.0);
    assert_eq!(centre_and_deadband(&[]), None);
}

/// A stick flicked out to `out` and let go to `rest`, `count` times, 300 ms
/// apart, with each rest held 250 ms.
fn flicks(count: u64, out: i16, rest: i16) -> Vec<(Duration, i16)> {
    let mut values = Vec::new();
    for i in 0..count {
        let t = Duration::from_millis(300 * i);
        values.push((t, out));
        values.push((t + Duration::from_millis(20), rest));
    }
    values
}

#[test]
fn gamepad_calibration_finds_where_the_stick_rests_after_each_flick() {
    let mut values = flicks(2, 32767, 3212);
    for (t, v) in flicks(2, -32768, 2955) {
        values.push((t + secs(0.6), v));
    }
    let rests = rests_after_flicks(&values, secs(1.5)).unwrap();
    assert_eq!(rests, [3212, 3212, 2955, 2955]);
}

#[test]
fn gamepad_calibration_waits_for_three_flicks_and_let_gos() {
    let two = rests_after_flicks(&flicks(2, 32767, 3212), secs(1.0)).unwrap_err();
    assert!(two.0.contains("2 seen"), "{}", two.0);
    // A third flick still held out: not let go yet.
    let mut held = flicks(2, 32767, 3212);
    held.push((secs(0.6), 32767));
    assert!(rests_after_flicks(&held, secs(2.0)).is_err());
    assert!(rests_after_flicks(&flicks(3, 32767, 3212), secs(1.0)).is_ok());
}

#[test]
fn a_stick_sweeping_past_centre_isnt_a_rest() {
    // Values held less than 100 ms are the stick moving, not resting.
    let mut values = flicks(3, 32767, 3212);
    values.push((secs(0.95), 1000));
    values.push((secs(0.99), 32767));
    values.push((secs(1.0), 3212));
    let rests = rests_after_flicks(&values, secs(1.3)).unwrap();
    assert!(!rests.contains(&1000), "{rests:?}");
}

#[test]
fn gamepad_ends_that_stop_short_are_stretched_to_full_stick() {
    // A pad whose stick only reaches 90 % each way still gives full stick.
    let calibration = gamepad_stick(0.0, 0.0, Some((-29491, 29490)));
    assert_eq!(
        stick_us(stick_deflection(29490.0, &calibration, true)),
        HIGH_US
    );
    assert_eq!(
        stick_us(stick_deflection(-29491.0, &calibration, true)),
        LOW_US
    );
    // An end the stick never moved toward stays at SDL's limit.
    let one_sided = gamepad_stick(0.0, 0.0, Some((-100, 29490)));
    assert_eq!(one_sided.min, -32768.0);
}

#[test]
fn an_uncalibrated_device_flies_on_its_pack_profiles_starting_values() {
    let trace = Trace::read("pocket-rf-off-18.csv");
    let mut inputs = Inputs::new(common::built_in_profiles());
    replay(&mut inputs, trace.batches());
    let device = &inputs.devices()[0];
    assert!(!device.calibrated());
    assert_eq!(
        device.profile().unwrap().setup,
        profile("opendrone/radiomaster-pocket").setup
    );
}
