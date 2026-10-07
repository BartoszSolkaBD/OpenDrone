//! Turning a device's values into Channels: channel mapping, Calibration,
//! the deadband, throttle styles and switches.
//!
//! Channels are in microseconds, as a receiver hands them to the Flight
//! Controller. Calibrated full stick lands at 988 and 2012 µs and centre at
//! 1500 µs, exactly where ELRS puts ±100 % and centre (#19 §3). They keep the
//! device's full resolution: nothing is rounded here, so each of the
//! Pocket's 2049 stick steps and the DualSense's 256 stays its own value. The
//! Radio Link rounds them to the receiver's steps.

use crate::controls::Position;
use crate::profile::{
    Ends, FlightMode, FlightModeSwitch, GamepadSetup, GamepadThrottle, OnOffSwitch, PadStick,
    RadioSetup, RadioStick, Setup, StickCalibration, Switches, ThrottleZero,
};
use crate::raw::DeviceState;
use crate::switches::VirtualSwitches;

/// Full stick low, and the low end of a switch Channel: −100 % on ELRS.
pub const LOW_US: f64 = 988.0;
/// Centre stick, and a switch Channel's middle.
pub const CENTRE_US: f64 = 1500.0;
/// Full stick high, and the high end of a switch Channel: +100 % on ELRS.
pub const HIGH_US: f64 = 2012.0;
/// From centre to full stick.
const HALF_TRAVEL_US: f64 = 512.0;

/// One Input Device's Channels, in microseconds.
///
/// Arm, Flight Mode and Crash Flip are switch Channels with fixed meanings
/// (ADR-0017): AUX1 high is armed; AUX2 low is Acro, middle Horizon, high
/// Angle; AUX3 high is Crash Flip. A switch with no source is `None`, and the
/// game decides it: Auto-arm, or the Flight Mode setting and the Preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channels {
    pub roll: f64,
    pub pitch: f64,
    pub throttle: f64,
    pub yaw: f64,
    /// AUX1.
    pub arm: Option<f64>,
    /// AUX2.
    pub flight_mode: Option<f64>,
    /// AUX3.
    pub crash_flip: Option<f64>,
}

impl Channels {
    /// Sticks centred, throttle low and no switch sources.
    pub fn at_rest() -> Channels {
        Channels {
            roll: CENTRE_US,
            pitch: CENTRE_US,
            throttle: LOW_US,
            yaw: CENTRE_US,
            arm: None,
            flight_mode: None,
            crash_flip: None,
        }
    }

    /// Whether AUX1 says armed.
    pub fn armed(&self) -> Option<bool> {
        self.arm.map(|us| us > CENTRE_US)
    }

    /// The Flight Mode AUX2 says.
    pub fn mode(&self) -> Option<FlightMode> {
        self.flight_mode.map(|us| {
            if us < (LOW_US + CENTRE_US) / 2.0 {
                FlightMode::Acro
            } else if us > (CENTRE_US + HIGH_US) / 2.0 {
                FlightMode::Angle
            } else {
                FlightMode::Horizon
            }
        })
    }

    /// Whether AUX3 says Crash Flip.
    pub fn crash_flipping(&self) -> Option<bool> {
        self.crash_flip.map(|us| us > CENTRE_US)
    }
}

/// How far a centring stick is from centre, from −1 (full low) to +1 (full
/// high), in SDL's direction, after its Calibration and, when `deadband` is
/// true, its deadband.
///
/// The deadband works like Betaflight's (`fapplyDeadband`, `rc.c`): inside
/// it the stick reads centre; outside, the deadband is taken off and the
/// rest stretched, so full stick stays full. Each side of the centre has its
/// own span, because a Gamepad's centre is rarely in the middle. Ends that
/// stop short of SDL's ±100 % are stretched to full stick.
pub fn stick_deflection(value: f64, calibration: &StickCalibration, deadband: bool) -> f64 {
    let band = if deadband { calibration.deadband } else { 0.0 };
    let offset = value - calibration.centre;
    if offset.abs() <= band {
        return 0.0;
    }
    if offset > 0.0 {
        let span = calibration.max - calibration.centre - band;
        if span <= 0.0 {
            return 1.0;
        }
        ((offset - band) / span).min(1.0)
    } else {
        let span = calibration.centre - calibration.min - band;
        if span <= 0.0 {
            return -1.0;
        }
        ((offset + band) / span).max(-1.0)
    }
}

/// How far along its travel a throttle stick or trigger is, from 0 to 1.
pub fn share_of_travel(value: f64, ends: &Ends) -> f64 {
    let span = ends.max - ends.min;
    if span == 0.0 {
        return 0.0;
    }
    ((value - ends.min) / span).clamp(0.0, 1.0)
}

/// The Channel for a stick deflection from −1 to +1.
pub fn stick_us(deflection: f64) -> f64 {
    CENTRE_US + HALF_TRAVEL_US * deflection
}

/// The Channel for a throttle share from 0 to 1.
pub fn throttle_us(share: f64) -> f64 {
    LOW_US + 2.0 * HALF_TRAVEL_US * share
}

/// A switch Channel that is on or off.
fn on_off_us(on: bool) -> f64 {
    if on { HIGH_US } else { LOW_US }
}

/// The Channel AUX2 carries for a Flight Mode.
pub fn flight_mode_us(mode: FlightMode) -> f64 {
    match mode {
        FlightMode::Acro => LOW_US,
        FlightMode::Horizon => CENTRE_US,
        FlightMode::Angle => HIGH_US,
    }
}

/// One device's Channels, from its current values, its profile's setup and
/// its Virtual Switches.
pub fn channels(setup: &Setup, state: &DeviceState, virtuals: &VirtualSwitches) -> Channels {
    let (roll, pitch, throttle, yaw) = match setup {
        Setup::Radio(radio) => radio_sticks(radio, state),
        Setup::Gamepad(pad) => gamepad_sticks(pad, state),
    };
    let switches = setup.switches();
    Channels {
        roll,
        pitch,
        throttle,
        yaw,
        arm: on_off(switches.arm.as_ref(), state, virtuals.arm()).map(on_off_us),
        flight_mode: flight_mode(switches, state, virtuals).map(flight_mode_us),
        crash_flip: on_off(switches.crash_flip.as_ref(), state, virtuals.crash_flip())
            .map(on_off_us),
    }
}

/// Where a Radio channel sits: a third of an axis channel's travel, or a
/// button channel pressed.
pub fn channel_position(state: &DeviceState, channel: u8) -> Option<Position> {
    if channel <= crate::controls::RADIO_AXIS_CHANNELS {
        state.channel_axis(channel).map(Position::of_axis)
    } else {
        state.channel_button(channel).map(|down| {
            if down {
                Position::Pressed
            } else {
                Position::Low
            }
        })
    }
}

fn on_off(switch: Option<&OnOffSwitch>, state: &DeviceState, virtual_on: bool) -> Option<bool> {
    match switch? {
        OnOffSwitch::Channel { channel, on } => {
            Some(channel_position(state, *channel) == Some(*on))
        }
        OnOffSwitch::Virtual { .. } => Some(virtual_on),
    }
}

fn flight_mode(
    switches: &Switches,
    state: &DeviceState,
    virtuals: &VirtualSwitches,
) -> Option<FlightMode> {
    match switches.flight_mode.as_ref()? {
        FlightModeSwitch::Channel {
            channel,
            low,
            middle,
            high,
        } => match channel_position(state, *channel)? {
            Position::Low => Some(*low),
            Position::Middle => Some(*middle),
            Position::High | Position::Pressed => Some(*high),
        },
        FlightModeSwitch::Steps { modes, .. } => modes
            .get(virtuals.flight_mode_step())
            .or(modes.first())
            .copied(),
    }
}

/// A Radio's sticks: roll, pitch, throttle and yaw. A channel the device
/// doesn't send reads centre, or no throttle.
fn radio_sticks(radio: &RadioSetup, state: &DeviceState) -> (f64, f64, f64, f64) {
    let calibration = &radio.calibration;
    let stick = |s: RadioStick, cal: &StickCalibration| {
        let Some(value) = state.channel_axis(s.channel) else {
            return CENTRE_US;
        };
        let d = stick_deflection(f64::from(value), cal, true);
        stick_us(if s.reverse { -d } else { d })
    };
    let throttle = {
        let s = radio.channels.throttle;
        match state.channel_axis(s.channel) {
            Some(value) => {
                let share = share_of_travel(f64::from(value), &calibration.throttle);
                throttle_us(if s.reverse { 1.0 - share } else { share })
            }
            None => LOW_US,
        }
    };
    (
        stick(radio.channels.roll, &calibration.roll),
        stick(radio.channels.pitch, &calibration.pitch),
        throttle,
        stick(radio.channels.yaw, &calibration.yaw),
    )
}

/// A Gamepad's sticks. Up and right read high. A device SDL can't read as a
/// gamepad reads centred sticks and no throttle.
fn gamepad_sticks(pad: &GamepadSetup, state: &DeviceState) -> (f64, f64, f64, f64) {
    let Some(values) = state.pad.as_ref() else {
        return (CENTRE_US, CENTRE_US, LOW_US, CENTRE_US);
    };
    let calibration = &pad.calibration;
    // How far a stick is pushed, up and right high, before any reverse.
    let deflection = |stick: crate::controls::Stick, deadband: bool| {
        let value = f64::from(values.axis(stick.pad_axis()));
        let d = stick_deflection(value, calibration.stick(stick), deadband);
        if stick.is_vertical() { -d } else { d }
    };
    let stick = |s: PadStick| {
        let d = deflection(s.stick, true);
        stick_us(if s.reverse { -d } else { d })
    };
    // The deadband acts on roll, pitch and yaw only, never the throttle.
    let throttle = match pad.channels.throttle {
        GamepadThrottle::Stick {
            stick,
            zero: ThrottleZero::AtRest,
        } => throttle_us(deflection(stick, false).max(0.0)),
        GamepadThrottle::Stick {
            stick,
            zero: ThrottleZero::AtBottom,
        } => throttle_us((deflection(stick, false) + 1.0) / 2.0),
        GamepadThrottle::Trigger(trigger) => {
            let value = f64::from(values.axis(trigger.pad_axis()));
            throttle_us(share_of_travel(value, calibration.trigger(trigger)))
        }
    };
    (
        stick(pad.channels.roll),
        stick(pad.channels.pitch),
        throttle,
        stick(pad.channels.yaw),
    )
}
