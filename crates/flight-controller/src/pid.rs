//! The PID loop in rate mode, reimplemented from Betaflight 2026.6.2's
//! `pidController` (`src/main/flight/pid.c`), its gain scaling and filters
//! (`pidInitConfig` and `pidInitFilters`, `pid_init.c`, and `pid.h`), and
//! `dynLpfDTermUpdate`; the Betaflight research §3 and §5.
//!
//! Each axis compares the setpoint (after RC smoothing) with the filtered
//! gyro (in °/s) and corrects the error with Betaflight's own scaling, so a
//! pilot's "P 45" means here what it means on their quad:
//!
//! - P = 0.032029 × P gain × error. Yaw's P then passes through its own
//!   low-pass (`yaw_lowpass_hz`, a PT1).
//! - I grows by 0.244381 × I gain × the loop's step × error each loop (yaw's
//!   I gain counts 2.5 times), within ± `iterm_windup`% of the PID-sum limit.
//! - D = 0.000529 × D gain × how fast the gyro reading falls, per second:
//!   on the measurement, so a stick move doesn't kick it. The gyro D reads
//!   passes through the D term's two low-passes first (`dterm_lpf1_*`,
//!   which may follow the throttle, and `dterm_lpf2_*`).
//! - F = 0.013754 × F gain ÷ 100 × feedforward ([`crate::feedforward`]).
//! - The sum is P + I + D + F.
//!
//! During yaw spin recovery the yaw setpoint is 0, I is emptied on every
//! axis, and roll and pitch's P, D and F are 0, so only yaw's P and D work,
//! braking the spin.
//!
//! Not here yet: anti-gravity, I-term relax, TPA and Dynamic D (#50).

use opendrone_maths::Fingerprinter;

use crate::filter::Lowpass;
use crate::gyro_filter::{Dynamic, nyquist};
use crate::tune::Tune;

/// Betaflight's gain scaling (`PTERM_SCALE`, `ITERM_SCALE`, `DTERM_SCALE`,
/// `FEEDFORWARD_SCALE`).
const P_SCALE: f64 = 0.032029;
const I_SCALE: f64 = 0.244381;
const D_SCALE: f64 = 0.000529;
const F_SCALE: f64 = 0.013754;
/// Yaw's I gain counts this many times (unless integrated yaw is on, which
/// OpenDrone doesn't simulate).
const YAW_I_FACTOR: f64 = 2.5;

/// One axis's terms after a loop, on Betaflight's scale: 1000 is the whole
/// motor range.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Terms {
    pub p: f64,
    pub i: f64,
    pub d: f64,
    pub f: f64,
    pub sum: f64,
}

/// The PID loop's memory.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pid {
    kp: [f64; 3],
    ki: [f64; 3],
    kd: [f64; 3],
    kf: [f64; 3],
    iterm_limit: [f64; 3],
    /// The loop's step, in seconds, and how many loops a second.
    dt: f64,
    frequency: f64,
    /// Each axis's I term, carried from loop to loop.
    iterm: [f64; 3],
    /// The D term's two low-passes on each axis, the first's dynamic range
    /// when it is dynamic, and yaw P's low-pass.
    dterm_lowpass_1: [Lowpass; 3],
    dterm_lowpass_2: [Lowpass; 3],
    dterm_dynamic: Option<Dynamic>,
    yaw_lowpass: Lowpass,
    /// Whether the D term's and yaw P's low-passes have settled on their
    /// first input.
    settled: bool,
    /// The filtered gyro reading D compared with, from the loop before.
    previous_gyro: [f64; 3],
    pub terms: [Terms; 3],
}

impl Pid {
    /// A fresh loop: I empty, and D's memory starting from the current gyro
    /// reading, so the first loop gets no artificial kick; its low-passes
    /// settle on their first input.
    pub fn new(tune: &Tune, loop_hz: u32, gyro: [f64; 3]) -> Pid {
        let mut kp = [0.0; 3];
        let mut ki = [0.0; 3];
        let mut kd = [0.0; 3];
        let mut kf = [0.0; 3];
        for (axis, gains) in tune.pid.iter().enumerate() {
            kp[axis] = P_SCALE * f64::from(gains.p);
            ki[axis] = I_SCALE * f64::from(gains.i);
            kd[axis] = D_SCALE * f64::from(gains.d);
            kf[axis] = F_SCALE * (f64::from(gains.f) * 0.01);
        }
        ki[2] *= YAW_I_FACTOR;
        let windup = 0.01 * f64::from(tune.iterm_windup);
        let iterm_limit = [
            windup * f64::from(tune.pidsum_limit),
            windup * f64::from(tune.pidsum_limit),
            windup * f64::from(tune.pidsum_limit_yaw),
        ];
        let frequency = f64::from(loop_hz.max(1));
        let dt = 1.0 / frequency;
        let nyquist = nyquist(loop_hz);
        let dterm_dynamic = (tune.dterm_lpf1_dyn_min_hz > 0).then_some(Dynamic {
            kind: tune.dterm_lpf1_type,
            lowest: tune.dterm_lpf1_dyn_min_hz,
            highest: tune.dterm_lpf1_dyn_max_hz,
            expo: tune.dterm_lpf1_dyn_expo,
        });
        let first_cutoff = dterm_dynamic.map_or(tune.dterm_lpf1_static_hz, |d| d.lowest);
        Pid {
            kp,
            ki,
            kd,
            kf,
            iterm_limit,
            dt,
            frequency,
            iterm: [0.0; 3],
            dterm_lowpass_1: core::array::from_fn(|_| {
                Lowpass::new(tune.dterm_lpf1_type, f64::from(first_cutoff), dt, nyquist)
            }),
            dterm_lowpass_2: core::array::from_fn(|_| {
                Lowpass::new(
                    tune.dterm_lpf2_type,
                    f64::from(tune.dterm_lpf2_static_hz),
                    dt,
                    nyquist,
                )
            }),
            dterm_dynamic,
            yaw_lowpass: Lowpass::new(
                crate::filter::LowpassType::Pt1,
                f64::from(tune.yaw_lowpass_hz),
                dt,
                nyquist,
            ),
            settled: false,
            previous_gyro: gyro,
            terms: [Terms::default(); 3],
        }
    }

    /// One loop. `setpoint` is each axis's setpoint after RC smoothing,
    /// `gyro` the filtered gyro and `feedforward` each axis's feedforward,
    /// after RC smoothing. `stabilising` false zeroes every term, as
    /// Betaflight does with stabilisation off; `reset_iterm` empties I after
    /// the sum, as before Airmode starts at low throttle; `yaw_spin` is true
    /// during yaw spin recovery.
    pub fn step(
        &mut self,
        setpoint: [f64; 3],
        gyro: [f64; 3],
        feedforward: [f64; 3],
        stabilising: bool,
        reset_iterm: bool,
        yaw_spin: bool,
    ) {
        // The D term's low-passes run first, on every axis
        // (`gyroRateDterm`).
        let first = !self.settled;
        let mut d_gyro = gyro;
        for (axis, value) in d_gyro.iter_mut().enumerate() {
            if first {
                self.dterm_lowpass_1[axis].settle(*value);
                self.dterm_lowpass_2[axis].settle(*value);
            }
            *value = self.dterm_lowpass_1[axis].apply(*value);
            *value = self.dterm_lowpass_2[axis].apply(*value);
        }
        for axis in 0..3 {
            let target = if axis == 2 && yaw_spin {
                0.0
            } else {
                setpoint[axis]
            };
            let error = target - gyro[axis];
            let mut p = self.kp[axis] * error;
            if axis == 2 {
                if first {
                    self.yaw_lowpass.settle(p);
                }
                p = self.yaw_lowpass.apply(p);
            }
            let limit = self.iterm_limit[axis];
            let mut i = (self.iterm[axis] + self.ki[axis] * self.dt * error).clamp(-limit, limit);
            let mut d = if self.kd[axis] > 0.0 {
                self.kd[axis] * -(d_gyro[axis] - self.previous_gyro[axis]) * self.frequency
            } else {
                0.0
            };
            self.previous_gyro[axis] = d_gyro[axis];
            let mut f = self.kf[axis] * feedforward[axis];
            if yaw_spin {
                i = 0.0;
                if axis < 2 {
                    p = 0.0;
                    d = 0.0;
                    f = 0.0;
                }
            }
            self.iterm[axis] = i;
            self.terms[axis] = Terms {
                p,
                i,
                d,
                f,
                sum: p + i + d + f,
            };
        }
        self.settled = true;
        if !stabilising {
            self.iterm = [0.0; 3];
            self.terms = [Terms::default(); 3];
        } else if reset_iterm {
            self.iterm = [0.0; 3];
        }
    }

    /// Moves the D term's first low-pass to the throttle, from 0 to 1, if it
    /// is dynamic (`dynLpfDTermUpdate`).
    pub fn follow(&mut self, throttle: f64) {
        if let Some(dynamic) = self.dterm_dynamic {
            let cutoff = dynamic.cutoff(throttle, self.dt);
            for filter in &mut self.dterm_lowpass_1 {
                filter.set_cutoff(dynamic.kind, cutoff, self.dt);
            }
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64s(&self.iterm);
        for filter in self
            .dterm_lowpass_1
            .iter()
            .chain(&self.dterm_lowpass_2)
            .chain([&self.yaw_lowpass])
        {
            filter.write_fingerprint(f);
        }
        f.write_u64(u64::from(self.settled));
        f.write_f64s(&self.previous_gyro);
        for terms in self.terms {
            f.write_f64s(&[terms.p, terms.i, terms.d, terms.f, terms.sum]);
        }
    }
}
