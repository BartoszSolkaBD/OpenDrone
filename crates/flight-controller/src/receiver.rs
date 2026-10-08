//! What the Flight Controller makes of each Radio Link frame: Betaflight
//! 2026.6.2's `updateRcCommands` and the throttle curve's lookup table
//! (`src/main/fc/rc.c`), reimplemented from their behaviour.
//!
//! Betaflight works the sticks in floating point but the throttle in whole
//! microseconds and whole steps; both are kept, because they decide the
//! exact throttle the mixer sees.

use crate::Channels;
use crate::rates::{Axis, Rates};
use crate::tune::Tune;

/// The bottom and top of Betaflight's command range, in µs
/// (`PWM_RANGE_MIN`, `PWM_RANGE_MAX`).
pub(crate) const RANGE_MIN: i64 = 1000;
pub(crate) const RANGE_MAX: i64 = 2000;

/// How many points the throttle curve's table holds
/// (`THROTTLE_LOOKUP_LENGTH`).
const LOOKUP_LENGTH: usize = 12;

/// One frame's Channels as Betaflight reads them, in µs (`rcData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RcData {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub throttle: f64,
    pub aux: [f64; 3],
}

/// How many Channels the Flight Controller reads: four sticks and AUX1–3.
pub(crate) const CHANNEL_COUNT: usize = 7;
/// Betaflight's place for the throttle among them (`THROTTLE`).
pub(crate) const THROTTLE: usize = 3;
/// How many of them are sticks (`NON_AUX_CHANNEL_COUNT`).
pub(crate) const STICK_COUNT: usize = 4;

impl RcData {
    /// What Betaflight holds before its first frame (`rxInit`): every Channel
    /// at `mid_rc`, the throttle at `rx_min_usec`, and AUX1 just below the
    /// Arm range's start, so Arm is off (1675 µs, one 25 µs step under
    /// 1700 µs).
    pub fn at_power_up(tune: &Tune) -> RcData {
        let mid = f64::from(tune.mid_rc);
        RcData {
            roll: mid,
            pitch: mid,
            yaw: mid,
            throttle: f64::from(tune.rx_min_usec),
            aux: [1675.0, mid, mid],
        }
    }

    /// Every Channel in Betaflight's order: roll, pitch, yaw, throttle, then
    /// AUX1–3.
    pub fn values(&self) -> [f64; CHANNEL_COUNT] {
        [
            self.roll,
            self.pitch,
            self.yaw,
            self.throttle,
            self.aux[0],
            self.aux[1],
            self.aux[2],
        ]
    }

    /// The Channels from their values in Betaflight's order.
    pub fn from_values(values: [f64; CHANNEL_COUNT]) -> RcData {
        RcData {
            roll: values[0],
            pitch: values[1],
            yaw: values[2],
            throttle: values[3],
            aux: [values[4], values[5], values[6]],
        }
    }

    pub fn from_channels(channels: &Channels) -> RcData {
        RcData {
            roll: channels.roll.micros(),
            pitch: channels.pitch.micros(),
            yaw: channels.yaw.micros(),
            throttle: channels.throttle.micros(),
            aux: [
                channels.arm.micros(),
                channels.flight_mode.micros(),
                channels.crash_flip.micros(),
            ],
        }
    }

    /// The throttle as Betaflight's whole-number checks read it: the float
    /// cut to a whole number of µs, as C does when it passes a float to an
    /// `int`.
    pub fn throttle_whole(&self) -> i64 {
        self.throttle.trunc() as i64
    }

    /// True when the throttle counts as low: below `min_check`
    /// (`calculateThrottleStatus`).
    pub fn throttle_low(&self, tune: &Tune) -> bool {
        self.throttle < f64::from(tune.min_check)
    }

    /// The throttle in whole percent above `min_check`, from 0 to 100
    /// (`calculateThrottlePercent`), as Airmode's start reads it.
    pub fn throttle_percent(&self, tune: &Tune) -> i64 {
        let channel = self.throttle_whole().clamp(RANGE_MIN, RANGE_MAX);
        let min_check = i64::from(tune.min_check);
        // C's integer division cuts towards zero, as Rust's does. A
        // `min_check` at or above 2000 µs leaves no throttle above it.
        (((channel - min_check) * 100) / (RANGE_MAX - min_check).max(1)).clamp(0, 100)
    }
}

/// Betaflight's whole-number `constrain`: `low` below it, `high` above it,
/// and never a panic, even when `low` is above `high`.
fn constrain_whole(value: i64, low: i64, high: i64) -> i64 {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

/// The throttle curve's table: 12 points from Betaflight's `thr_mid`,
/// `thr_hover` and `thr_expo`, in whole µs from 1000 to 2000
/// (`initRcProcessing`). Two quadratic Bézier segments meet at the hover
/// point; with mid 0.50, hover 0.50 and expo 0 they make a straight line.
pub(crate) fn throttle_lookup(rates: &Rates) -> [i64; LOOKUP_LENGTH] {
    let mid = f64::from(rates.thr_mid) / 100.0;
    let expo = f64::from(rates.thr_expo) / 100.0;
    let hover = f64::from(rates.thr_hover) / 100.0;
    let cp1x = mid * 0.5;
    let cp1y = hover * 0.5 * (1.0 + expo);
    let cp2x = (1.0 + mid) * 0.5;
    let cp2y = 1.0 + (hover - 1.0) * 0.5 * (1.0 + expo);
    let mut table = [0; LOOKUP_LENGTH];
    for (i, point) in table.iter_mut().enumerate() {
        let x = i as f64 / (LOOKUP_LENGTH - 1) as f64;
        let y = if x <= mid {
            bezier(x, [0.0, cp1x, mid], [0.0, cp1y, hover])
        } else {
            bezier(x, [mid, cp2x, 1.0], [hover, cp2y, 1.0])
        };
        // `lrintf`: to the nearest whole number, halves to the even one.
        *point = (RANGE_MIN as f64 + y * (RANGE_MAX - RANGE_MIN) as f64).round_ties_even() as i64;
    }
    table
}

/// A quadratic Bézier's height where its across-ness is `x`: solve for the
/// curve's parameter, preferring the first root when it lies on the curve,
/// as Betaflight's `quadraticBezier` does.
fn bezier(x: f64, px: [f64; 3], py: [f64; 3]) -> f64 {
    let a = px[0] - 2.0 * px[1] + px[2];
    let b = 2.0 * px[1] - 2.0 * px[0];
    let c = px[0] - x;
    let mut t = 0.0;
    if a.abs() < 1e-6 {
        if b.abs() > 1e-6 {
            t = -c / b;
        }
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let root = disc.sqrt();
            let t1 = (-b + root) / (2.0 * a);
            let t2 = (-b - root) / (2.0 * a);
            t = if (0.0..=1.0).contains(&t1) { t1 } else { t2 };
        }
    }
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    u * u * py[0] + 2.0 * u * t * py[1] + t * t * py[2]
}

/// The sticks and throttle as commands (`rcCommand`): roll, pitch and yaw
/// from −500 to 500 after the deadband (yaw turned round, so stick right is
/// negative), and the throttle in whole µs from 1000 to 2000 through the
/// throttle curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RcCommand {
    pub sticks: [f64; 3],
    pub throttle: i64,
}

impl RcCommand {
    pub fn new(rc: &RcData, tune: &Tune, lookup: &[i64; LOOKUP_LENGTH]) -> RcCommand {
        let mid = f64::from(tune.mid_rc);
        let stick = |value: f64, deadband: u8| {
            apply_deadband((value - mid).clamp(-500.0, 500.0), f64::from(deadband))
        };
        let yaw_direction = if tune.yaw_control_reversed { 1.0 } else { -1.0 };
        let sticks = [
            stick(rc.roll, tune.deadband),
            stick(rc.pitch, tune.deadband),
            yaw_direction * stick(rc.yaw, tune.yaw_deadband),
        ];
        let min_check = i64::from(tune.min_check);
        let throttle = constrain_whole(rc.throttle_whole(), min_check, RANGE_MAX);
        let throttle = (throttle - min_check) * RANGE_MIN / (RANGE_MAX - min_check).max(1);
        RcCommand {
            sticks,
            throttle: lookup_throttle(throttle, lookup),
        }
    }

    /// Each axis's stick deflection, from −1 to +1, past the deadband.
    pub fn deflection(&self, tune: &Tune, axis: Axis) -> f64 {
        let divider = match axis {
            Axis::Roll | Axis::Pitch => 500.0 - f64::from(tune.deadband),
            Axis::Yaw => 500.0 - f64::from(tune.yaw_deadband),
        };
        self.sticks[axis.index()] / divider
    }
}

/// Betaflight's `fapplyDeadband`: zero inside the deadband, and the rest
/// moved in by it, so full stick stays full stick.
fn apply_deadband(value: f64, deadband: f64) -> f64 {
    if value.abs() < deadband {
        0.0
    } else if value >= 0.0 {
        value - deadband
    } else {
        value + deadband
    }
}

/// Spreads 0–1000 over the table's 11 steps and joins the two points either
/// side in whole numbers (`rcLookupThrottle`, `scaleRange`).
fn lookup_throttle(throttle: i64, lookup: &[i64; LOOKUP_LENGTH]) -> i64 {
    let steps = (LOOKUP_LENGTH - 1) as i64;
    let range = RANGE_MAX - RANGE_MIN;
    let scaled = throttle * steps;
    let index = scaled / range;
    let rest = scaled % range;
    if index >= steps {
        return lookup[LOOKUP_LENGTH - 1];
    }
    if index < 0 {
        return lookup[0];
    }
    let (from, to) = (lookup[index as usize], lookup[index as usize + 1]);
    (to - from) * rest / range + from
}
