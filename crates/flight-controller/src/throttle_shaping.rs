//! What Betaflight's mixer works out from its throttle besides the motors:
//! anti-gravity's throttle speed and the TPA factor, which the PID loop reads
//! the next loop, and throttle boost, which pushes the throttle itself.
//! Reimplemented from Betaflight 2026.6.2's `mixTable`
//! (`src/main/flight/mixer.c`), `pidUpdateAntiGravityThrottleFilter`,
//! `pidUpdateTpaFactor` and `getTpaFactorClassic` (`src/main/flight/pid.c`)
//! and their set-up in `pidInitConfig` and `pidInitFilters`
//! (`src/main/flight/pid_init.c`); the Betaflight research §1 and §3.
//!
//! Each loop, after the PID loop, the mixer takes its throttle, from 0 to 1,
//! after the pilot's throttle limit, and:
//!
//! 1. **Anti-gravity** takes how fast the throttle moved since the loop
//!    before, per second, weights it by (1 − throttle)², and when the
//!    throttle rose, by half of (1 − throttle) again, so it works most at low
//!    throttle and most on a punch from there. A second-order low-pass
//!    (PT2) at `anti_gravity_cutoff_hz` smooths it. The next loop's PID loop
//!    multiplies it by `anti_gravity_gain`.
//! 2. **TPA** works out its factor. Above `tpa_breakpoint` it falls in a
//!    straight line, from 1 at the breakpoint to 1 − `tpa_rate`% at full
//!    throttle. Below `tpa_low_breakpoint` low-throttle TPA cuts instead,
//!    from nothing at that breakpoint to `tpa_low_rate`% at zero throttle,
//!    but only until the throttle first reaches the breakpoint after
//!    power-up, unless `tpa_low_always` is ON. A breakpoint is held within
//!    0–99% of the throttle, and low-throttle TPA's within 1% and the other.
//! 3. **Throttle boost** adds `throttle_boost` ÷ 10 times the throttle's
//!    distance from a first-order low-pass (PT1) of it at
//!    `throttle_boost_cutoff`, held within 0 to 1: a fast throttle move goes
//!    further for a moment. Betaflight's Blackbox logs the boosted throttle as
//!    the mixer's, and the motors get it, but anti-gravity, TPA and the
//!    dynamic low-passes read the throttle before the boost.
//!
//! Anti-gravity is a Betaflight feature (`ANTI_GRAVITY`), on by default and
//! on in every Tune OpenDrone has: a gain of 0 is how a Tune turns it off.
//!
//! In Crash Flip, Betaflight's mixer stops before any of this, so it all
//! holds. (Betaflight's PID loop then multiplies its stored anti-gravity by
//! the gain again every loop, as the mixer no longer replaces it; here it
//! holds instead, so a Quad leaving Crash Flip doesn't get a boost that has
//! grown past any number.)
//!
//! A fresh Flight Controller starts as if its first loop's throttle had held
//! since power-up: no throttle speed, the throttle boost's low-pass on it,
//! and the TPA factor worked out from it, with low-throttle TPA already over
//! if it is past its breakpoint.

use opendrone_maths::{Fingerprinter, functions};

use crate::constrain;
use crate::filter::{Lowpass, LowpassType, pt1_gain};
use crate::gyro_filter::nyquist;
use crate::pid::Shaping;
use crate::tune::Tune;

/// The bottom of Betaflight's command range, in µs (`PWM_RANGE_MIN`).
const RANGE_MIN: f64 = 1000.0;

/// The mixer's throttle work for the PID loop, and throttle boost.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThrottleShaping {
    frequency: f64,
    /// Anti-gravity: its gain, the throttle the loop before, its low-pass,
    /// and the throttle's smoothed speed (before the gain).
    anti_gravity_gain: f64,
    previous_throttle: f64,
    anti_gravity_lowpass: Lowpass,
    throttle_speed: f64,
    /// TPA: its breakpoints as throttles from 0 to 1, how fast each cuts, and
    /// whether low-throttle TPA keeps working.
    tpa_breakpoint: f64,
    tpa_multiplier: f64,
    tpa_low_breakpoint: f64,
    tpa_low_multiplier: f64,
    tpa_low_always: bool,
    /// Low-throttle TPA is over until the next power-up
    /// (`isTpaLowFaded`).
    tpa_low_over: bool,
    /// The TPA factor the next loop's PID loop uses.
    tpa_factor: f64,
    /// Throttle boost: how strongly (0: off) and its low-pass.
    boost: f64,
    boost_lowpass: Lowpass,
    /// Whether it has settled on its first throttle.
    settled: bool,
}

impl ThrottleShaping {
    pub fn new(tune: &Tune, loop_hz: u32) -> ThrottleShaping {
        let frequency = f64::from(loop_hz.max(1));
        let dt = 1.0 / frequency;
        let tpa_breakpoint = constrain(
            (f64::from(tune.tpa_breakpoint) - RANGE_MIN) / 1000.0,
            0.0,
            0.99,
        );
        // Betaflight takes it that the low breakpoint is never above the
        // other.
        let tpa_low_breakpoint = functions::min(
            constrain(
                (f64::from(tune.tpa_low_breakpoint) - RANGE_MIN) / 1000.0,
                0.01,
                1.0,
            ),
            tpa_breakpoint,
        );
        ThrottleShaping {
            frequency,
            anti_gravity_gain: f64::from(tune.anti_gravity_gain),
            previous_throttle: 0.0,
            anti_gravity_lowpass: Lowpass::new(
                LowpassType::Pt2,
                f64::from(tune.anti_gravity_cutoff_hz),
                dt,
                nyquist(loop_hz),
            ),
            throttle_speed: 0.0,
            tpa_breakpoint,
            tpa_multiplier: (f64::from(tune.tpa_rate) / 100.0) / (1.0 - tpa_breakpoint),
            tpa_low_breakpoint,
            tpa_low_multiplier: f64::from(tune.tpa_low_rate) / (100.0 * tpa_low_breakpoint),
            tpa_low_always: tune.tpa_low_always,
            tpa_low_over: false,
            tpa_factor: 1.0,
            boost: f64::from(tune.throttle_boost) * 0.1,
            boost_lowpass: Lowpass::pt(1, pt1_gain(f64::from(tune.throttle_boost_cutoff), dt)),
            settled: false,
        }
    }

    /// Before the first loop's PID loop: settles on the mixer's throttle,
    /// from 0 to 1, as if it had held since power-up. Later calls do nothing.
    pub fn settle(&mut self, throttle: f64) {
        if self.settled {
            return;
        }
        self.settled = true;
        self.previous_throttle = throttle;
        self.boost_lowpass.settle(throttle);
        self.tpa_factor = self.tpa(throttle);
    }

    /// What the PID loop reads: the TPA factor and anti-gravity, times its
    /// gain.
    pub fn shaping(&self) -> Shaping {
        Shaping {
            tpa: self.tpa_factor,
            anti_gravity: self.throttle_speed * self.anti_gravity_gain,
        }
    }

    /// The mixer's work on its throttle, from 0 to 1, after the throttle
    /// limit: anti-gravity's throttle speed and the TPA factor for the next
    /// loop; then gives the throttle after throttle boost.
    pub fn mixer(&mut self, throttle: f64) -> f64 {
        self.settle(throttle);
        // Anti-gravity (`pidUpdateAntiGravityThrottleFilter`).
        let below = 1.0 - throttle;
        let mut speed = (throttle - self.previous_throttle).abs() * self.frequency;
        speed *= below * below;
        if throttle > self.previous_throttle {
            speed *= below * 0.5;
        }
        self.previous_throttle = throttle;
        self.throttle_speed = self.anti_gravity_lowpass.apply(speed);
        // TPA (`pidUpdateTpaFactor`).
        self.tpa_factor = self.tpa(constrain(throttle, 0.0, 1.0));
        // Throttle boost.
        if self.boost > 0.0 {
            let quick = throttle - self.boost_lowpass.apply(throttle);
            constrain(throttle + self.boost * quick, 0.0, 1.0)
        } else {
            throttle
        }
    }

    /// The TPA factor at a throttle from 0 to 1 (`getTpaFactorClassic`):
    /// low-throttle TPA's latch moves on as the throttle passes its
    /// breakpoint.
    fn tpa(&mut self, throttle: f64) -> f64 {
        let past_low = throttle >= self.tpa_low_breakpoint || self.tpa_low_breakpoint <= 0.01;
        let cut = if past_low || self.tpa_low_over {
            if !self.tpa_low_always {
                self.tpa_low_over = true;
            }
            self.tpa_multiplier * functions::max(throttle - self.tpa_breakpoint, 0.0)
        } else {
            self.tpa_low_multiplier * (self.tpa_low_breakpoint - throttle)
        };
        1.0 - cut
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64s(&[self.previous_throttle, self.throttle_speed, self.tpa_factor]);
        self.anti_gravity_lowpass.write_fingerprint(f);
        self.boost_lowpass.write_fingerprint(f);
        f.write_u64(u64::from(self.tpa_low_over));
        f.write_u64(u64::from(self.settled));
    }
}
