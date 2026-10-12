//! Runaway takeoff prevention, reimplemented from Betaflight 2026.6.2's
//! `processRx` and `subTaskPidController` (`src/main/fc/core.c`).
//!
//! A quad with a motor wired backwards or its board turned the wrong way
//! takes off in a flip and keeps going: its PIDs push one way as hard as
//! they can. Betaflight guards against that right after arming, and the
//! guard is on by default (`runaway_takeoff_prevention`). It is the same here:
//! a Quad pinned against a wall right after arming makes the same PID sums,
//! so it disarms the same way.
//!
//! - **It watches** from arming until the flight counts as stable, and not
//!   in Crash Flip.
//! - **It disarms** when any axis's PID sum stays at 600 or more (60% of the
//!   motor range) with the gyro moving (roll or pitch above 15 °/s, or yaw
//!   above 50 °/s) for more than 75 ms. Arming is then blocked (`RUNAWAY`)
//!   until the Arm switch goes off.
//! - **The flight counts as stable** once it has added up the Tune's
//!   `runaway_takeoff_deactivate_delay` (500 ms by default; half that with
//!   the throttle at 75% or more) of moments, checked on each Radio Link
//!   frame, in which every PID sum is below 100 and the throttle is at
//!   `runaway_takeoff_deactivate_throttle_percent` (20% by default) with a
//!   stick moved at least 15%, or at twice that throttle (up to 75%) with
//!   the sticks anywhere. Then the guard is off until the next power-up
//!   (Reset), however often the Quad is disarmed and armed.
//!
//! The PID sums are the last loop's, unclamped; the sticks are as the frame
//! before this one left them, and the throttle this frame's, as Betaflight
//! has them when it checks. Betaflight reads its filtered gyro; until the
//! filters arrive (#49) this reads the gyro as the sensor reads it.

use opendrone_maths::Fingerprinter;

use crate::tune::Tune;

/// A PID sum at or above this, on any axis, counts as running away
/// (`RUNAWAY_TAKEOFF_PIDSUM_THRESHOLD`).
const PIDSUM_THRESHOLD: f64 = 600.0;
/// How long it must run away before the disarm, in µs: more than this
/// (`RUNAWAY_TAKEOFF_ACTIVATE_DELAY`).
const ACTIVATE_DELAY_US: u64 = 75_000;
/// Roll and pitch must turn faster than this, in whole °/s, and yaw faster
/// than the next, for the Quad to count as moving
/// (`RUNAWAY_TAKEOFF_GYRO_LIMIT_RP`, `_YAW`).
const GYRO_LIMIT_ROLL_PITCH: u64 = 15;
const GYRO_LIMIT_YAW: u64 = 50;
/// A stable flight's sticks: one moved at least this far, in percent
/// (`RUNAWAY_TAKEOFF_DEACTIVATE_STICK_PERCENT`).
const STICK_PERCENT: f64 = 15.0;
/// A stable flight's PID sums: each below this
/// (`RUNAWAY_TAKEOFF_DEACTIVATE_PIDSUM_LIMIT`).
const STABLE_PIDSUM: f64 = 100.0;
/// At this throttle, in percent, or more, the stable flight it needs is
/// halved (`RUNAWAY_TAKEOFF_HIGH_THROTTLE_PERCENT`).
const HIGH_THROTTLE_PERCENT: i64 = 75;

/// Where the Flight Controller stands when the guard looks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Moment {
    pub armed: bool,
    /// In Crash Flip.
    pub crash_flip: bool,
    /// The loop's moment, in µs since power-up.
    pub now_us: u64,
}

/// Runaway takeoff prevention's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Runaway {
    /// The flight has counted as stable since power-up: the guard is off
    /// (`runawayTakeoffCheckDisabled`).
    pub done: bool,
    /// When the current stretch of stable flight started, in µs
    /// (`runawayTakeoffDeactivateUs`).
    stable_since_us: Option<u64>,
    /// The stable flight before it since arming, in µs
    /// (`runawayTakeoffAccumulatedUs`).
    stable_us: u64,
    /// When running away disarms, in µs (`runawayTakeoffTriggerUs`).
    trigger_us: Option<u64>,
}

impl Runaway {
    /// Whether it is watching now: armed, on in the Tune, not yet switched
    /// off by a stable flight, and not in Crash Flip.
    fn watching(&self, tune: &Tune, at: Moment) -> bool {
        at.armed && tune.runaway_takeoff_prevention && !self.done && !at.crash_flip
    }

    /// On arming: the stable flight and the timer start afresh. Once the
    /// guard is off, it stays off.
    pub fn arm(&mut self) {
        self.stable_since_us = None;
        self.stable_us = 0;
        self.trigger_us = None;
    }

    /// On each Radio Link frame (`processRx`): adds up stable flight, and
    /// switches the guard off once there's enough. `throttle_percent` is
    /// this frame's throttle above `min_check`; `deflection` each stick's as
    /// the frame before left it, if any; `pid_sums` the last loop's.
    pub fn frame(
        &mut self,
        tune: &Tune,
        at: Moment,
        throttle_percent: i64,
        deflection: Option<[f64; 3]>,
        pid_sums: [f64; 3],
    ) {
        if !self.watching(tune, at) {
            return;
        }
        let now_us = at.now_us;
        // Airmode is always on, so the motors always count as running.
        let low = i64::from(tune.runaway_takeoff_deactivate_throttle_percent);
        let mid = (low * 2).min(HIGH_THROTTLE_PERCENT);
        let sticks_active = deflection.is_some_and(|sticks| {
            sticks
                .iter()
                .any(|deflection| deflection.abs() * 100.0 >= STICK_PERCENT)
        });
        let stable = ((throttle_percent >= low && sticks_active) || throttle_percent >= mid)
            && pid_sums.iter().all(|sum| sum.abs() < STABLE_PIDSUM);
        if stable {
            let since = *self.stable_since_us.get_or_insert(now_us);
            let mut delay_us = u64::from(tune.runaway_takeoff_deactivate_delay);
            if throttle_percent >= HIGH_THROTTLE_PERCENT {
                delay_us /= 2;
            }
            delay_us *= 1000;
            if now_us - since + self.stable_us > delay_us {
                self.done = true;
            }
        } else {
            if let Some(since) = self.stable_since_us {
                self.stable_us += now_us - since;
            }
            self.stable_since_us = None;
        }
    }

    /// After each loop's PID sums (`subTaskPidController`): true when the
    /// Quad has run away long enough to disarm. `gyro` is in °/s.
    pub fn loop_check(
        &mut self,
        tune: &Tune,
        at: Moment,
        pid_sums: [f64; 3],
        gyro: [f64; 3],
    ) -> bool {
        let now_us = at.now_us;
        if !self.watching(tune, at) {
            self.trigger_us = None;
            return false;
        }
        // `gyroAbsRateDps`: the size of the rate, cut to whole °/s.
        let whole = |rate: f64| rate.abs().trunc() as u64;
        let over = pid_sums.iter().any(|sum| sum.abs() >= PIDSUM_THRESHOLD);
        let moving = whole(gyro[1]) > GYRO_LIMIT_ROLL_PITCH
            || whole(gyro[0]) > GYRO_LIMIT_ROLL_PITCH
            || whole(gyro[2]) > GYRO_LIMIT_YAW;
        if !(over && moving) {
            self.trigger_us = None;
            return false;
        }
        match self.trigger_us {
            None => {
                self.trigger_us = Some(now_us + ACTIVATE_DELAY_US);
                false
            }
            Some(trigger) => now_us > trigger,
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.done));
        for moment in [self.stable_since_us, self.trigger_us] {
            match moment {
                None => f.write_u64(0),
                Some(us) => {
                    f.write_u64(1);
                    f.write_u64(us);
                }
            }
        }
        f.write_u64(self.stable_us);
    }
}
