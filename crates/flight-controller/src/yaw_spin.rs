//! Yaw spin recovery, reimplemented from Betaflight 2026.6.2's
//! `initYawSpinRecovery`, `checkForYawSpin` and `handleYawSpin`
//! (`src/main/sensors/gyro.c`); the Betaflight research §4.1.
//!
//! A hit can set a Quad spinning on yaw far faster than the pilot's Rates
//! ever ask. Past a threshold, recovery starts: the PID loop drops I on every
//! axis and roll and pitch control, and brakes the yaw towards 0 °/s with
//! the whole motor range ([`crate::pid`], [`crate::mixer`]). It ends once the
//! yaw rate has stayed 100 °/s below the threshold for more than 20 ms.
//!
//! The Tune's `yaw_spin_recovery` sets the threshold:
//!
//! - `AUTO` (Betaflight's default): the Rates' max yaw rate plus a quarter of
//!   it, or plus 200 °/s if that's more, held within 500–1950 °/s. On 670 °/s
//!   Rates that's 870 °/s.
//! - `ON`: the Tune's `yaw_spin_threshold`.
//! - `OFF`: never.
//!
//! Betaflight watches its filtered gyro; until the filters arrive (#49) this
//! one watches the gyro as the sensor reads it. A hit is the only thing that
//! spins a Quad this fast: the gyro reads up to ±2000 °/s, and Betaflight's
//! check for a gyro overflowing its range is off on the gyros both alpha
//! Quads carry (`gyroHasOverflowProtection`, `gyro_init.c`).

use opendrone_maths::Fingerprinter;

use crate::rates::{Axis, Rates};
use crate::tune::{Tune, YawSpinRecovery};

/// The lowest and highest AUTO threshold, in °/s
/// (`YAW_SPIN_RECOVERY_THRESHOLD_MIN`, `_MAX`).
const LOWEST: i64 = 500;
const HIGHEST: i64 = 1950;
/// How far below the threshold the yaw rate must fall to end it, in °/s.
const BELOW: f64 = 100.0;
/// How long it must stay there, in µs: more than this.
const HOLD_US: u64 = 20_000;

/// Yaw spin recovery's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct YawSpin {
    /// The yaw rate that starts it, in whole °/s; `None` with it off.
    threshold: Option<i64>,
    /// Whether it's on now (`yawSpinDetected`).
    pub active: bool,
    /// When the yaw rate was last at or above the end's level, in µs
    /// (`yawSpinTimeUs`).
    since_us: u64,
}

impl YawSpin {
    /// At power-up, its threshold worked out from the Tune and the Rates.
    pub fn new(tune: &Tune, rates: &Rates) -> YawSpin {
        let threshold = match tune.yaw_spin_recovery {
            YawSpinRecovery::Off => None,
            YawSpinRecovery::On => Some(i64::from(tune.yaw_spin_threshold)),
            YawSpinRecovery::Auto => {
                // `(int)maxRcRate[FD_YAW]`: cut to a whole number.
                let max_yaw = rates.max_rate(Axis::Yaw).trunc() as i64;
                let allowance = (max_yaw / 4).max(200);
                Some((max_yaw + allowance).clamp(LOWEST, HIGHEST))
            }
        };
        YawSpin {
            threshold,
            active: false,
            since_us: 0,
        }
    }

    /// The threshold, in °/s, or `None` with recovery off.
    pub fn threshold(&self) -> Option<i64> {
        self.threshold
    }

    /// One loop: `yaw` is the gyro's yaw rate in °/s, `now_us` the loop's
    /// moment.
    pub fn check(&mut self, yaw: f64, now_us: u64) {
        let Some(threshold) = self.threshold else {
            return;
        };
        if self.active {
            if yaw.abs() < threshold as f64 - BELOW {
                if now_us.saturating_sub(self.since_us) > HOLD_US {
                    self.active = false;
                }
            } else {
                self.since_us = now_us;
            }
        } else if (yaw.trunc() as i64).abs() > threshold {
            // `abs((int)gyro)`: cut to a whole number first.
            self.active = true;
            self.since_us = now_us;
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.threshold.map_or(0, |t| t as u64));
        f.write_u64(u64::from(self.active));
        f.write_u64(self.since_us);
    }
}
