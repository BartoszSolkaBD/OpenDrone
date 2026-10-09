//! Rates: the pilot's curves that turn stick movement into rotation speed and
//! throttle, copied field for field from a Betaflight 2026.6 rate profile
//! (`controlRateConfig_t` in `src/main/fc/controlrate_profile.h`, the profile
//! name aside), as Betaflight's CLI stores them.
//!
//! The curves follow Betaflight 2026.6.2's `applyBetaflightRates`,
//! `applyRaceFlightRates`, `applyKissRates`, `applyActualRates` and
//! `applyQuickRates` (`src/main/fc/rc.c`), reimplemented from their maths
//! (the Betaflight research §2), then clamped to the axis's rate limit
//! (`processRcCommand`).

use opendrone_maths::Fingerprinter;

/// The active Rates: every field of a Betaflight 2026.6 rate profile, as the
/// CLI stores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rates {
    /// `rates_type`.
    pub rates_type: RatesType,
    pub roll: AxisRates,
    pub pitch: AxisRates,
    pub yaw: AxisRates,
    /// `roll_rate_limit`, `pitch_rate_limit` and `yaw_rate_limit`, in °/s,
    /// from 200 to 1998.
    pub rate_limit: [u16; 3],
    /// `thr_mid`: where on the stick the hover point sits, from 0 to 100.
    pub thr_mid: u8,
    /// `thr_hover`: the throttle at the hover point, from 0 to 100.
    pub thr_hover: u8,
    /// `thr_expo`, from 0 to 100.
    pub thr_expo: u8,
    /// `throttle_limit_type`.
    pub throttle_limit_type: ThrottleLimitType,
    /// `throttle_limit_percent`, from 25 to 100. Betaflight keeps it while the
    /// limit is off, unused; then it is 100, Betaflight's default.
    pub throttle_limit_percent: u8,
    /// `quickrates_rc_expo`.
    pub quickrates_rc_expo: bool,
}

/// One axis's three stored numbers, such as `roll_rc_rate`, `roll_srate` and
/// `roll_expo`. What each means depends on the Rates type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisRates {
    pub rc_rate: u8,
    pub srate: u8,
    pub expo: u8,
}

/// Betaflight's five Rates types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatesType {
    Betaflight,
    Raceflight,
    Kiss,
    Actual,
    Quick,
}

/// `throttle_limit_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrottleLimitType {
    Off,
    /// Scales the whole throttle range down to the limit.
    Scale,
    /// Cuts the throttle off at the limit.
    Clip,
}

/// One of the three rotation axes, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Roll,
    Pitch,
    Yaw,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::Roll, Axis::Pitch, Axis::Yaw];

    pub(crate) fn index(self) -> usize {
        match self {
            Axis::Roll => 0,
            Axis::Pitch => 1,
            Axis::Yaw => 2,
        }
    }
}

/// The highest rotation speed any Rates type may ask for, in °/s
/// (`SETPOINT_RATE_LIMIT_MAX`).
const SETPOINT_RATE_LIMIT: f64 = 1998.0;

impl Rates {
    /// Betaflight 2026.6's default rate profile: Actual, 70 °/s at centre, 670
    /// °/s at full stick, no expo, rate limits 1998 °/s, a straight throttle
    /// line and no throttle limit (`controlrate_profile.c`).
    pub const BETAFLIGHT_DEFAULT: Rates = Rates {
        rates_type: RatesType::Actual,
        roll: AxisRates {
            rc_rate: 7,
            srate: 67,
            expo: 0,
        },
        pitch: AxisRates {
            rc_rate: 7,
            srate: 67,
            expo: 0,
        },
        yaw: AxisRates {
            rc_rate: 7,
            srate: 67,
            expo: 0,
        },
        rate_limit: [1998; 3],
        thr_mid: 50,
        thr_hover: 50,
        thr_expo: 0,
        throttle_limit_type: ThrottleLimitType::Off,
        throttle_limit_percent: 100,
        quickrates_rc_expo: false,
    };

    /// One axis's three stored numbers.
    pub fn axis(&self, axis: Axis) -> AxisRates {
        match axis {
            Axis::Roll => self.roll,
            Axis::Pitch => self.pitch,
            Axis::Yaw => self.yaw,
        }
    }

    /// The rotation speed, in °/s, these Rates ask for on one axis at a stick
    /// deflection from −1 to +1, within the axis's rate limit: Betaflight's
    /// raw setpoint, before any smoothing.
    pub fn setpoint(&self, axis: Axis, deflection: f64) -> f64 {
        let limit = f64::from(self.rate_limit[axis.index()]);
        self.curve(axis, deflection).clamp(-limit, limit)
    }

    /// The rotation speed these Rates ask for on one axis at full stick, in
    /// °/s, before the rate limit: Betaflight's `maxRcRate`, from which
    /// yaw spin recovery's AUTO threshold is worked out (`initRcProcessing`).
    pub fn max_rate(&self, axis: Axis) -> f64 {
        self.curve(axis, 1.0)
    }

    /// The Rates type's curve at a stick deflection from −1 to +1.
    fn curve(&self, axis: Axis, deflection: f64) -> f64 {
        let numbers = self.axis(axis);
        let a = deflection.abs();
        match self.rates_type {
            RatesType::Betaflight => betaflight(numbers, deflection, a),
            RatesType::Raceflight => raceflight(numbers, deflection, a),
            RatesType::Kiss => kiss(numbers, deflection, a),
            RatesType::Actual => actual(numbers, deflection, a),
            RatesType::Quick => quick(numbers, deflection, a, self.quickrates_rc_expo),
        }
    }

    /// Feeds every field into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(match self.rates_type {
            RatesType::Betaflight => 0,
            RatesType::Raceflight => 1,
            RatesType::Kiss => 2,
            RatesType::Actual => 3,
            RatesType::Quick => 4,
        });
        for axis in [self.roll, self.pitch, self.yaw] {
            for n in [axis.rc_rate, axis.srate, axis.expo] {
                f.write_u64(u64::from(n));
            }
        }
        for limit in self.rate_limit {
            f.write_u64(u64::from(limit));
        }
        for n in [
            self.thr_mid,
            self.thr_hover,
            self.thr_expo,
            self.throttle_limit_percent,
        ] {
            f.write_u64(u64::from(n));
        }
        f.write_u64(match self.throttle_limit_type {
            ThrottleLimitType::Off => 0,
            ThrottleLimitType::Scale => 1,
            ThrottleLimitType::Clip => 2,
        });
        f.write_u64(u64::from(self.quickrates_rc_expo));
    }
}

fn cube(x: f64) -> f64 {
    x * x * x
}

/// "Betaflight": the stick is bent by expo (a cubic), scaled by 200 °/s per
/// unit of RC rate (each unit above 2 adds another 14.54), and steepened
/// towards full stick by the super rate.
fn betaflight(numbers: AxisRates, x: f64, a: f64) -> f64 {
    let mut stick = x;
    if numbers.expo > 0 {
        let expo = f64::from(numbers.expo) / 100.0;
        stick = x * cube(a) * expo + x * (1.0 - expo);
    }
    let mut rc_rate = f64::from(numbers.rc_rate) / 100.0;
    if rc_rate > 2.0 {
        rc_rate += 14.54 * (rc_rate - 2.0);
    }
    let mut rate = 200.0 * rc_rate * stick;
    if numbers.srate > 0 {
        let super_factor = 1.0 / (1.0 - a * f64::from(numbers.srate) / 100.0).clamp(0.01, 1.0);
        rate *= super_factor;
    }
    rate
}

/// "Raceflight": 10 °/s per unit of rate, the stick bent by expo (a square
/// term), then raised by acro+ towards full stick.
fn raceflight(numbers: AxisRates, x: f64, a: f64) -> f64 {
    let stick = (1.0 + 0.01 * f64::from(numbers.expo) * (x * x - 1.0)) * x;
    let rate = 10.0 * f64::from(numbers.rc_rate) * stick;
    rate * (1.0 + a * f64::from(numbers.srate) * 0.01)
}

/// "KISS": a cubic curve by RC curve, 2000 °/s per unit of RC rate ÷ 1000,
/// steepened by rate towards full stick, and held within ±1998 °/s.
fn kiss(numbers: AxisRates, x: f64, a: f64) -> f64 {
    let curve = f64::from(numbers.expo) / 100.0;
    let use_rates = 1.0 / (1.0 - a * f64::from(numbers.srate) / 100.0).clamp(0.01, 1.0);
    let stick = (cube(x) * curve + x * (1.0 - curve)) * (f64::from(numbers.rc_rate) / 1000.0);
    (2000.0 * use_rates * stick).clamp(-SETPOINT_RATE_LIMIT, SETPOINT_RATE_LIMIT)
}

/// "Actual": the centre sensitivity at the centre, rising to the max rate at
/// full stick; expo (a fifth-power term) moves where the curve bends.
fn actual(numbers: AxisRates, x: f64, a: f64) -> f64 {
    let expo = f64::from(numbers.expo) / 100.0;
    let bent = a * (cube(x) * x * x * expo + x * (1.0 - expo));
    let centre = f64::from(numbers.rc_rate) * 10.0;
    let stick_movement =
        opendrone_maths::functions::max(0.0, f64::from(numbers.srate) * 10.0 - centre);
    x * centre + stick_movement * bent
}

/// "Quick": twice the RC rate at the centre and the max rate at full stick,
/// joined by a super factor; expo bends the curve, on the stick's size or,
/// with `quickrates_rc_expo`, on the stick itself. Held within ±1998 °/s.
fn quick(numbers: AxisRates, x: f64, a: f64, rc_expo: bool) -> f64 {
    // Betaflight keeps both as whole numbers.
    let rc_rate = u32::from(numbers.rc_rate) * 2;
    let max_dps = (u32::from(numbers.srate) * 10).max(rc_rate);
    let expo = f64::from(numbers.expo) / 100.0;
    let ratio = f64::from(max_dps) / f64::from(rc_rate);
    let super_config = (ratio - 1.0) / ratio;
    let rate = if rc_expo {
        let curve = cube(x) * expo + x * (1.0 - expo);
        let super_factor = 1.0 / (1.0 - a * super_config).clamp(0.01, 1.0);
        curve * f64::from(rc_rate) * super_factor
    } else {
        let curve = cube(a) * expo + a * (1.0 - expo);
        let super_factor = 1.0 / (1.0 - curve * super_config).clamp(0.01, 1.0);
        x * f64::from(rc_rate) * super_factor
    };
    rate.clamp(-SETPOINT_RATE_LIMIT, SETPOINT_RATE_LIMIT)
}
