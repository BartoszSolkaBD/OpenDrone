//! Readable checks on turning values into Channels: where full stick lands,
//! the deadband, full resolution, throttle styles and stick modes.
//!
//! Basis: Rule, from #19 §3–§5, ADR-0017 and Betaflight 2026.6's deadband
//! (`fapplyDeadband`, `fc/rc.c`), unless a check says otherwise. Values are
//! SDL's axis units (−32768…32767) and µs.

mod common;

use opendrone_input::channels::{
    CENTRE_US, HIGH_US, LOW_US, channels, share_of_travel, stick_deflection, stick_us, throttle_us,
};
use opendrone_input::controls::{PadAxis, Position, Stick, Trigger};
use opendrone_input::profile::{
    Ends, GamepadChannels, GamepadThrottle, PadStick, Setup, StickCalibration, StickMode,
    ThrottleStyle, ThrottleZero,
};
use opendrone_input::raw::{DeviceState, PadState};
use opendrone_input::switches::VirtualSwitches;

#[test]
fn full_stick_lands_at_988_and_2012_us_and_centre_at_1500_us() {
    let full = StickCalibration::full();
    assert_eq!(stick_us(stick_deflection(-32768.0, &full, true)), LOW_US);
    assert_eq!(stick_us(stick_deflection(0.0, &full, true)), CENTRE_US);
    assert_eq!(stick_us(stick_deflection(32767.0, &full, true)), HIGH_US);
    let throttle = Ends::full_stick();
    assert_eq!(throttle_us(share_of_travel(-32768.0, &throttle)), LOW_US);
    assert_eq!(throttle_us(share_of_travel(32767.0, &throttle)), HIGH_US);
}

#[test]
fn a_calibrated_centre_off_the_middle_still_reads_1500_us() {
    // A Gamepad stick resting 10 % right: each side keeps its own span.
    let calibration = StickCalibration {
        min: -32768.0,
        centre: 3277.0,
        max: 32767.0,
        deadband: 0.0,
    };
    assert_eq!(
        stick_us(stick_deflection(3277.0, &calibration, true)),
        CENTRE_US
    );
    assert_eq!(
        stick_us(stick_deflection(32767.0, &calibration, true)),
        HIGH_US
    );
    assert_eq!(
        stick_us(stick_deflection(-32768.0, &calibration, true)),
        LOW_US
    );
}

/// Betaflight 2026.6's own deadband (`fapplyDeadband` in `common/maths.c`,
/// then divided by `500 - deadband` in `fc/rc.c`), on a stick from −500 to
/// 500.
fn betaflight(stick: f64, deadband: f64) -> f64 {
    let after = if stick.abs() < deadband {
        0.0
    } else if stick >= 0.0 {
        stick - deadband
    } else {
        stick + deadband
    };
    after / (500.0 - deadband)
}

#[test]
fn the_deadband_works_like_betaflights_and_full_stick_stays_full() {
    // A 5 % deadband on a stick that runs ±500 like Betaflight's rcCommand.
    let calibration = StickCalibration {
        min: -500.0,
        centre: 0.0,
        max: 500.0,
        deadband: 25.0,
    };
    for stick in [
        -500.0, -300.0, -25.0, -24.0, 0.0, 10.0, 25.0, 26.0, 250.0, 500.0,
    ] {
        let ours = stick_deflection(stick, &calibration, true);
        assert!(
            (ours - betaflight(stick, 25.0)).abs() < 1e-12,
            "at {stick}: {ours} against Betaflight's {}",
            betaflight(stick, 25.0)
        );
    }
    assert_eq!(stick_deflection(500.0, &calibration, true), 1.0);
    assert_eq!(stick_deflection(20.0, &calibration, true), 0.0);
}

fn pad(axes: [i16; 6]) -> DeviceState {
    let mut state = DeviceState::new(6, 13, true);
    let mut pad = PadState::at_rest();
    pad.axes = axes;
    state.pad = Some(pad);
    state
}

fn dualsense_with(channels_layout: GamepadChannels, deadband: f64) -> Setup {
    let profile = common::profile("opendrone/dualsense");
    let Setup::Gamepad(mut setup) = profile.setup else {
        panic!("a Gamepad")
    };
    setup.channels = channels_layout;
    for stick in Stick::ALL {
        setup.calibration.stick_mut(stick).deadband = deadband;
    }
    Setup::Gamepad(setup)
}

#[test]
fn the_deadband_acts_on_roll_pitch_and_yaw_but_never_the_throttle() {
    let setup = dualsense_with(
        GamepadChannels::in_mode(StickMode::Mode2, ThrottleStyle::FullTravel),
        1000.0,
    );
    // Every stick 500 units off centre, inside the deadband.
    let state = pad([500, -500, 500, -500, 0, 0]);
    let c = channels(&setup, &state, &VirtualSwitches::new());
    assert_eq!((c.roll, c.pitch, c.yaw), (CENTRE_US, CENTRE_US, CENTRE_US));
    assert!(
        c.throttle > CENTRE_US,
        "the throttle still moves: {}",
        c.throttle
    );
}

#[test]
fn every_one_of_the_pockets_2049_stick_steps_stays_its_own_channel_value() {
    // Basis: Observed (#18): the Pocket's sticks sit on SDL's grid of 32
    // units, offset by 15 at centre, from −32768 to 32767.
    let full = StickCalibration::full();
    let mut values: Vec<f64> = (0..2049)
        .map(|step| (-32768 + 32 * step).min(32767))
        .map(|v| stick_us(stick_deflection(f64::from(v), &full, true)))
        .collect();
    values.dedup();
    assert_eq!(values.len(), 2049);
    assert!(
        values.windows(2).all(|w| w[1] - w[0] > 0.45),
        "about 0.5 µs a step"
    );
}

#[test]
fn every_one_of_the_dualsenses_256_steps_stays_its_own_channel_value() {
    let full = StickCalibration::full();
    let mut values: Vec<f64> = (0..256)
        .map(|step| f64::from(-32768 + 257 * step))
        .map(|v| stick_us(stick_deflection(v, &full, true)))
        .collect();
    values.dedup();
    assert_eq!(values.len(), 256);
}

#[test]
fn gamepad_stick_modes_1_to_4_lay_out_the_sticks_as_rc_modes_do() {
    use Stick::{LeftX, LeftY, RightX, RightY};
    let layout = |mode| {
        let c = GamepadChannels::in_mode(mode, ThrottleStyle::ZeroAtRest);
        let GamepadThrottle::Stick { stick, .. } = c.throttle else {
            panic!("a stick throttle")
        };
        (c.roll.stick, c.pitch.stick, c.yaw.stick, stick)
    };
    // (roll, pitch, yaw, throttle)
    assert_eq!(layout(StickMode::Mode1), (RightX, LeftY, LeftX, RightY));
    assert_eq!(layout(StickMode::Mode2), (RightX, RightY, LeftX, LeftY));
    assert_eq!(layout(StickMode::Mode3), (LeftX, LeftY, RightX, RightY));
    assert_eq!(layout(StickMode::Mode4), (LeftX, RightY, RightX, LeftY));
    for mode in [
        StickMode::Mode1,
        StickMode::Mode2,
        StickMode::Mode3,
        StickMode::Mode4,
    ] {
        assert_eq!(
            GamepadChannels::in_mode(mode, ThrottleStyle::FullTravel).mode(),
            Some(mode)
        );
    }
}

#[test]
fn a_gamepad_flies_mode_2_with_zero_at_rest_throttle_by_default() {
    let Setup::Gamepad(setup) = common::profile("opendrone/dualsense").setup else {
        panic!("a Gamepad")
    };
    assert_eq!(setup.channels.mode(), Some(StickMode::Mode2));
    assert_eq!(setup.channels.throttle_style(), ThrottleStyle::ZeroAtRest);
    let Setup::Gamepad(any) = common::profile("opendrone/any-gamepad").setup else {
        panic!("a Gamepad")
    };
    assert_eq!(any.channels, setup.channels);
}

#[test]
fn the_three_gamepad_throttle_styles_read_rest_full_up_and_full_down() {
    let style = |style| {
        let setup = dualsense_with(GamepadChannels::in_mode(StickMode::Mode2, style), 0.0);
        // SDL reads a stick pushed up as negative.
        let throttle = |left_y: i16, r2: i16| {
            channels(
                &setup,
                &pad([0, left_y, 0, 0, 0, r2]),
                &VirtualSwitches::new(),
            )
            .throttle
        };
        (
            throttle(0, 0),
            throttle(-32768, 0),
            throttle(32767, 0),
            throttle(0, 32767),
        )
    };
    // (at rest, full up, full down, R2 squeezed)
    assert_eq!(
        style(ThrottleStyle::ZeroAtRest),
        (LOW_US, HIGH_US, LOW_US, LOW_US)
    );
    assert_eq!(
        style(ThrottleStyle::FullTravel),
        (CENTRE_US, HIGH_US, LOW_US, CENTRE_US)
    );
    assert_eq!(
        style(ThrottleStyle::Trigger(Trigger::Right)),
        (LOW_US, LOW_US, LOW_US, HIGH_US)
    );
}

#[test]
fn a_reversed_gamepad_stick_runs_the_other_way() {
    let mut layout = GamepadChannels::in_mode(StickMode::Mode2, ThrottleStyle::ZeroAtRest);
    layout.roll = PadStick {
        stick: Stick::RightX,
        reverse: true,
    };
    let setup = dualsense_with(layout, 0.0);
    let c = channels(
        &setup,
        &pad([0, 0, 32767, 0, 0, 0]),
        &VirtualSwitches::new(),
    );
    assert_eq!(c.roll, LOW_US);
    let _ = PadAxis::RightX;
    let _ = ThrottleZero::AtRest;
}

#[test]
fn a_radio_switch_is_read_by_thirds_of_its_travel() {
    assert_eq!(Position::of_axis(-32768), Position::Low);
    assert_eq!(Position::of_axis(-10923), Position::Low);
    assert_eq!(Position::of_axis(15), Position::Middle);
    assert_eq!(Position::of_axis(10922), Position::Middle);
    assert_eq!(Position::of_axis(10923), Position::High);
    assert_eq!(Position::of_axis(32767), Position::High);
}

#[test]
fn a_radio_that_sends_no_stick_axes_reads_centred_sticks_and_no_throttle() {
    // EdgeTX's Advanced joystick mode at its defaults sends only buttons
    // (#19): such a Radio must never read half throttle.
    let setup = common::profile("opendrone/radiomaster-pocket").setup;
    let state = DeviceState::new(0, 24, false);
    let c = channels(&setup, &state, &VirtualSwitches::new());
    assert_eq!(c.throttle, LOW_US);
    assert_eq!((c.roll, c.pitch, c.yaw), (CENTRE_US, CENTRE_US, CENTRE_US));
}
