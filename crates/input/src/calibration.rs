//! The Calibration maths, as plain functions the setup screens feed with the
//! device's values (#19 §3). Every Input Device is calibrated, Radios
//! included.
//!
//! **A Radio**, in about 20 s. Each step moves on by itself once it sees the
//! move:
//! 1. Centre the sticks, throttle down, hands off: [`RadioRest`] captures the
//!    centres and the throttle bottom.
//! 2. Throttle up, yaw right, pitch up, roll right: [`ChannelOrder`] finds
//!    which channel carries each stick and which way it runs. It catches
//!    RETA, the default of official EdgeTX builds on fresh settings.
//! 3. Roll both sticks round their edges twice: [`EndsSeen`] captures
//!    each channel's ends.
//! 4. [`radio_calibration`] puts them together. A Radio's deadband starts at
//!    0: the Pocket shows no flicker at rest.
//!
//! **A Gamepad**, in about 15 s:
//! 1. Flick each stick in different directions and let go, 3–4 times:
//!    [`rests_after_flicks`] finds where each stick came to rest, and
//!    [`centre_and_deadband`] puts the centre in the middle of those rests,
//!    with a deadband of half their spread plus one 8-bit step. Flicking
//!    first also gets past SDL reporting a stick as 0 until it first moves
//!    more than 1.25 %.
//! 2. Roll the sticks round their edges: [`EndsSeen`] again.
//! 3. [`gamepad_stick`] puts them together.

use std::time::Duration;

use crate::controls::{AXIS_MAX, AXIS_MIN, EIGHT_BIT_STEP, RADIO_AXIS_CHANNELS};
use crate::profile::{Ends, RadioCalibration, RadioChannels, RadioStick, StickCalibration};

/// Why a Calibration can't be finished yet, as a plain sentence for the
/// setup screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unfinished(pub String);

/// A stick counts as moved once it is this far from where it rested: half way
/// from centre to an end, a quarter of full travel (#19 §3, "moves on by
/// itself once it sees the move").
const MOVED: f64 = 16384.0;

/// Radio step 1: every axis where it rests, with the sticks centred, the
/// throttle down and hands off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioRest(pub Vec<i16>);

/// The four moves of Radio step 2, in the order setup asks for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioMove {
    ThrottleUp,
    YawRight,
    PitchUp,
    RollRight,
}

impl RadioMove {
    pub const IN_ORDER: [RadioMove; 4] = [
        RadioMove::ThrottleUp,
        RadioMove::YawRight,
        RadioMove::PitchUp,
        RadioMove::RollRight,
    ];

    /// What setup asks the pilot to do.
    pub fn prompt(self) -> &'static str {
        match self {
            RadioMove::ThrottleUp => "Throttle up",
            RadioMove::YawRight => "Yaw right",
            RadioMove::PitchUp => "Pitch up",
            RadioMove::RollRight => "Roll right",
        }
    }
}

/// Radio step 2: finds the channel order and direction from four moves.
/// Feed it the axes' values after each change with [`ChannelOrder::see`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelOrder {
    rest: Vec<i16>,
    /// The stick found for each move so far, in move order.
    found: Vec<RadioStick>,
}

impl ChannelOrder {
    pub fn new(rest: &RadioRest) -> ChannelOrder {
        ChannelOrder {
            rest: rest.0.clone(),
            found: Vec::new(),
        }
    }

    /// The move setup is waiting for, or `None` when all four are found.
    pub fn waiting_for(&self) -> Option<RadioMove> {
        RadioMove::IN_ORDER.get(self.found.len()).copied()
    }

    /// Takes the axes' current values. When a channel not yet found has moved
    /// half way from centre to an end, it is the stick for the move setup is
    /// waiting for: running up means normal, down means reversed. Returns the
    /// move it just saw.
    pub fn see(&mut self, axes: &[i16]) -> Option<RadioMove> {
        let waiting = self.waiting_for()?;
        let candidates = axes
            .len()
            .min(self.rest.len())
            .min(usize::from(RADIO_AXIS_CHANNELS));
        let mut biggest: Option<(usize, f64)> = None;
        for (axis, (&value, &rest)) in axes.iter().zip(&self.rest).take(candidates).enumerate() {
            let channel = axis_channel(axis);
            if self.found.iter().any(|s| s.channel == channel) {
                continue;
            }
            let moved = f64::from(value) - f64::from(rest);
            if moved.abs() >= MOVED && biggest.is_none_or(|(_, b)| moved.abs() > b.abs()) {
                biggest = Some((axis, moved));
            }
        }
        let (axis, moved) = biggest?;
        self.found.push(RadioStick {
            channel: axis_channel(axis),
            reverse: moved < 0.0,
        });
        Some(waiting)
    }

    /// The channel order, once all four moves are found.
    pub fn channels(&self) -> Option<RadioChannels> {
        match self.found.as_slice() {
            [throttle, yaw, pitch, roll] => Some(RadioChannels {
                roll: *roll,
                pitch: *pitch,
                throttle: *throttle,
                yaw: *yaw,
            }),
            _ => None,
        }
    }
}

/// A Radio's CH1–CH8 are SDL axes 0–7.
fn axis_channel(axis: usize) -> u8 {
    u8::try_from(axis + 1).unwrap_or(u8::MAX)
}

/// The ends each axis reached: feed it the values after each change while
/// the pilot rolls the sticks round their edges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EndsSeen {
    lowest: Vec<i16>,
    highest: Vec<i16>,
}

impl EndsSeen {
    pub fn new() -> EndsSeen {
        EndsSeen::default()
    }

    /// Takes the axes' current values.
    pub fn see(&mut self, axes: &[i16]) {
        if self.lowest.len() < axes.len() {
            self.lowest.resize(axes.len(), i16::MAX);
            self.highest.resize(axes.len(), i16::MIN);
        }
        for (i, &value) in axes.iter().enumerate() {
            self.lowest[i] = self.lowest[i].min(value);
            self.highest[i] = self.highest[i].max(value);
        }
    }

    /// The lowest and highest value axis `axis` reached.
    pub fn of(&self, axis: usize) -> Option<(i16, i16)> {
        Some((*self.lowest.get(axis)?, *self.highest.get(axis)?))
    }
}

/// Puts a Radio's Calibration together: each stick's centre from where it
/// rested, its ends from the edges (or the rest, if further out), and no
/// deadband; the throttle's bottom and top likewise. Full stick then lands
/// at 988 and 2012 µs and centre at exactly 1500 µs.
pub fn radio_calibration(
    rest: &RadioRest,
    channels: &RadioChannels,
    ends: &EndsSeen,
) -> Result<RadioCalibration, Unfinished> {
    let axis_of = |stick: RadioStick, name: &str| -> Result<(f64, f64, f64), Unfinished> {
        let axis = usize::from(stick.channel - 1);
        let (Some(&rest), Some((low, high))) = (rest.0.get(axis), ends.of(axis)) else {
            return Err(Unfinished(format!(
                "the {name} stick's channel, CH{}, sent no values",
                stick.channel
            )));
        };
        Ok((
            f64::from(low.min(rest)),
            f64::from(rest),
            f64::from(high.max(rest)),
        ))
    };
    let stick = |stick: RadioStick, name: &str| -> Result<StickCalibration, Unfinished> {
        let (min, centre, max) = axis_of(stick, name)?;
        if centre - min < MOVED || max - centre < MOVED {
            return Err(Unfinished(format!(
                "the {name} stick didn't reach both of its ends: roll both sticks round their edges"
            )));
        }
        Ok(StickCalibration {
            min,
            centre,
            max,
            deadband: 0.0,
        })
    };
    let throttle = {
        let (min, _, max) = axis_of(channels.throttle, "throttle")?;
        if max - min < 2.0 * MOVED {
            return Err(Unfinished(
                "the throttle didn't travel from bottom to top: roll both sticks round their edges"
                    .to_string(),
            ));
        }
        Ends { min, max }
    };
    Ok(RadioCalibration {
        roll: stick(channels.roll, "roll")?,
        pitch: stick(channels.pitch, "pitch")?,
        yaw: stick(channels.yaw, "yaw")?,
        throttle,
    })
}

/// A Gamepad stick counts as flicked once it is this far out: half of full
/// travel.
const FLICKED: f64 = 16384.0;
/// It counts as let go once it is back within a quarter of its travel.
const LET_GO: f64 = 8192.0;
/// A value counts as a rest once the stick has held it this long: longer
/// than a stick sweeping past, shorter than a stick left alone.
pub const REST_HOLD: Duration = Duration::from_millis(100);
/// How many flicks setup asks for, at least.
pub const FLICKS_NEEDED: usize = 3;

/// Gamepad step 1: where one stick axis came to rest after each flick. Feed
/// it the axis's values with the time each arrived, and when the step ended.
/// After each flick past half travel, every value the stick then held for at
/// least [`REST_HOLD`] within a quarter of its travel is a rest, until the
/// next flick. Fails until the stick has been flicked and let go
/// [`FLICKS_NEEDED`] times.
pub fn rests_after_flicks(
    values: &[(Duration, i16)],
    end: Duration,
) -> Result<Vec<i16>, Unfinished> {
    let mut rests = Vec::new();
    let mut flicks = 0;
    let mut out = false;
    let mut let_go_since_flick = false;
    for (i, &(at, value)) in values.iter().enumerate() {
        let v = f64::from(value);
        if v.abs() >= FLICKED {
            if !out {
                flicks += 1;
            }
            out = true;
            let_go_since_flick = false;
        } else if v.abs() < LET_GO && flicks > 0 {
            out = false;
            let_go_since_flick = true;
        }
        let until = values.get(i + 1).map_or(end, |&(next, _)| next);
        if let_go_since_flick && until.saturating_sub(at) >= REST_HOLD {
            rests.push(value);
        }
    }
    let releases = flicks - usize::from(out);
    if releases < FLICKS_NEEDED || rests.is_empty() {
        return Err(Unfinished(format!(
            "flick each stick in different directions and let go, {FLICKS_NEEDED} or 4 times; {releases} seen so far"
        )));
    }
    Ok(rests)
}

/// A Gamepad stick's centre and deadband from where it came to rest: the
/// centre is the middle of the rests, and the deadband half their spread
/// plus one 8-bit step (#19 §3, #27), in SDL's units.
pub fn centre_and_deadband(rests: &[i16]) -> Option<(f64, f64)> {
    let low = f64::from(*rests.iter().min()?);
    let high = f64::from(*rests.iter().max()?);
    Some(((low + high) / 2.0, (high - low) / 2.0 + EIGHT_BIT_STEP))
}

/// Puts one Gamepad stick axis's Calibration together from its centre and
/// deadband and the ends it reached. Ends that didn't reach SDL's limits are
/// stretched to full stick; an end it never moved toward stays at SDL's
/// limit.
pub fn gamepad_stick(centre: f64, deadband: f64, ends: Option<(i16, i16)>) -> StickCalibration {
    let (low, high) = ends.map_or((AXIS_MIN, AXIS_MAX), |(low, high)| {
        (f64::from(low), f64::from(high))
    });
    let reached = |end: f64, limit: f64| {
        if (end - centre).abs() < MOVED {
            limit
        } else {
            end
        }
    };
    StickCalibration {
        min: reached(low, AXIS_MIN),
        centre,
        max: reached(high, AXIS_MAX),
        deadband,
    }
}
