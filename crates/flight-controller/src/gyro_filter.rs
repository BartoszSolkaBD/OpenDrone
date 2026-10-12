//! The gyro's low-pass filters and the dynamic low-passes' throttle:
//! Betaflight 2026.6.2's `gyroInitFilters` (`src/main/sensors/gyro_init.c`),
//! its gyro filtering (`gyroUpdate` in `src/main/sensors/gyro.c` and
//! `gyro_filter_impl.c`), `dynLpfGyroUpdate` and the mixer's
//! `updateDynLpfCutoffs` (`src/main/flight/mixer.c`), reimplemented from
//! their behaviour; the Betaflight research §5.
//!
//! The gyro reading goes through low-pass 2 (`gyro_lpf2_*`), which
//! Betaflight runs as each sample arrives, then low-pass 1 (`gyro_lpf1_*`).
//! Every Flight Controller part that reads the gyro reads it filtered: the
//! PID loop, yaw spin recovery, runaway takeoff prevention and Crash Flip.
//! The simulated gyro is clean, so the filters only add their delay, which
//! is what matters for Prop Wash ([ADR-0005]). The notch and RPM filters
//! wait for gyro noise (#21).
//!
//! # Dynamic low-passes
//!
//! With `gyro_lpf1_dyn_min_hz` above 0, low-pass 1 is dynamic: its cutoff
//! follows the mixer's throttle from `gyro_lpf1_dyn_min_hz` at zero throttle
//! to `gyro_lpf1_dyn_max_hz` at full, along the curve
//! `gyro_lpf1_dyn_expo` sets. The D term's first low-pass does the same. At
//! the end of a loop the mixer's throttle, rounded to whole percent, moves
//! both cutoffs, when it has changed and at least 5 ms have passed since they
//! last moved; the next loop filters with them.
//!
//! [ADR-0005]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0005-prop-wash-from-physics.md

use opendrone_maths::{Fingerprinter, functions};

use crate::filter::{Lowpass, LowpassType, dynamic_cutoff};
use crate::tune::Tune;

/// The dynamic low-passes move at most this often, in µs
/// (`DYN_LPF_THROTTLE_UPDATE_DELAY_US`).
const DYNAMIC_EVERY_US: u64 = 5000;
/// The throttle is rounded to this many steps first
/// (`DYN_LPF_THROTTLE_STEPS`).
const THROTTLE_STEPS: f64 = 100.0;

/// The nyquist limit a BIQUAD must stay under, in Hz: 95% of half the loop
/// rate, cut to whole Hz, as Betaflight works it out.
pub(crate) fn nyquist(loop_hz: u32) -> f64 {
    (f64::from(loop_hz) / 2.0 * 0.95).trunc()
}

/// One dynamic low-pass's range, from the Tune.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dynamic {
    pub kind: LowpassType,
    pub lowest: u16,
    pub highest: u16,
    pub expo: u8,
}

impl Dynamic {
    /// The cutoff at a throttle from 0 to 1, in Hz; a BIQUAD's is held
    /// under the nyquist limit (`0.475 ÷ the loop's step`).
    pub fn cutoff(&self, throttle: f64, dt: f64) -> f64 {
        let cutoff = dynamic_cutoff(throttle, self.lowest, self.highest, self.expo);
        if self.kind == LowpassType::Biquad {
            functions::min(cutoff, 0.475 / dt)
        } else {
            cutoff
        }
    }
}

/// The gyro's two low-passes on each axis.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GyroFilters {
    lowpass_1: [Lowpass; 3],
    lowpass_2: [Lowpass; 3],
    /// Low-pass 1's dynamic range, when it is dynamic.
    dynamic: Option<Dynamic>,
    dt: f64,
    settled: bool,
}

impl GyroFilters {
    pub fn new(tune: &Tune, loop_hz: u32) -> GyroFilters {
        let dt = 1.0 / f64::from(loop_hz.max(1));
        let nyquist = nyquist(loop_hz);
        let dynamic = (tune.gyro_lpf1_dyn_min_hz > 0).then_some(Dynamic {
            kind: tune.gyro_lpf1_type,
            lowest: tune.gyro_lpf1_dyn_min_hz,
            highest: tune.gyro_lpf1_dyn_max_hz,
            expo: tune.gyro_lpf1_dyn_expo,
        });
        let first_cutoff = dynamic.map_or(tune.gyro_lpf1_static_hz, |d| d.lowest);
        GyroFilters {
            lowpass_1: core::array::from_fn(|_| {
                Lowpass::new(tune.gyro_lpf1_type, f64::from(first_cutoff), dt, nyquist)
            }),
            lowpass_2: core::array::from_fn(|_| {
                Lowpass::new(
                    tune.gyro_lpf2_type,
                    f64::from(tune.gyro_lpf2_static_hz),
                    dt,
                    nyquist,
                )
            }),
            dynamic,
            dt,
            settled: false,
        }
    }

    /// One loop's gyro reading, in °/s, filtered. The first settles every
    /// filter on it.
    pub fn apply(&mut self, gyro: [f64; 3]) -> [f64; 3] {
        if !self.settled {
            self.settled = true;
            for (axis, value) in gyro.iter().enumerate() {
                self.lowpass_1[axis].settle(*value);
                self.lowpass_2[axis].settle(*value);
            }
        }
        let mut filtered = [0.0; 3];
        for (axis, out) in filtered.iter_mut().enumerate() {
            let sample = self.lowpass_2[axis].apply(gyro[axis]);
            *out = self.lowpass_1[axis].apply(sample);
        }
        filtered
    }

    /// Moves low-pass 1's cutoff to the throttle, if it is dynamic
    /// (`dynLpfGyroUpdate`).
    fn follow(&mut self, throttle: f64) {
        if let Some(dynamic) = self.dynamic {
            let cutoff = dynamic.cutoff(throttle, self.dt);
            for filter in &mut self.lowpass_1 {
                filter.set_cutoff(dynamic.kind, cutoff, self.dt);
            }
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for filter in self.lowpass_1.iter().chain(&self.lowpass_2) {
            filter.write_fingerprint(f);
        }
        f.write_u64(u64::from(self.settled));
    }
}

/// When the dynamic low-passes last moved, and to which throttle
/// (`updateDynLpfCutoffs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DynamicThrottle {
    /// When they last moved, in µs; `None` at power-up, which counts as long
    /// ago, so a fresh Flight Controller moves them at once.
    moved_at_us: Option<u64>,
    /// The throttle they moved to, in whole percent; `None` before the first.
    percent: Option<i64>,
}

impl DynamicThrottle {
    pub fn new() -> DynamicThrottle {
        DynamicThrottle {
            moved_at_us: None,
            percent: None,
        }
    }

    /// The end of a loop: the mixer's throttle, from 0 to 1, moves the gyro's
    /// and the D term's dynamic low-passes when it has changed by a whole
    /// percent and 5 ms have passed since they last moved. Gives the
    /// throttle they moved to, if they did.
    pub fn after_mixing(
        &mut self,
        now_us: u64,
        throttle: f64,
        gyro: &mut GyroFilters,
    ) -> Option<f64> {
        let due = self
            .moved_at_us
            .is_none_or(|at| now_us.saturating_sub(at) >= DYNAMIC_EVERY_US);
        if !due {
            return None;
        }
        // `lrintf`: to the nearest whole number, halves to the even one.
        let percent = (throttle * THROTTLE_STEPS).round_ties_even() as i64;
        if self.percent == Some(percent) {
            return None;
        }
        self.percent = Some(percent);
        self.moved_at_us = Some(now_us);
        let quantized = percent as f64 / THROTTLE_STEPS;
        gyro.follow(quantized);
        Some(quantized)
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for value in [self.moved_at_us, self.percent.map(|p| p as u64)] {
            match value {
                None => f.write_u64(0),
                Some(v) => {
                    f.write_u64(1);
                    f.write_u64(v);
                }
            }
        }
    }
}
