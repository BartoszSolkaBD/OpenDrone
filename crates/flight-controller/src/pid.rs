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
//! # Loop shaping
//!
//! Betaflight 2026.6.2 shapes those terms further (`pidController`,
//! `applyItermRelax`, `getTpaFactor`; the Betaflight research §3), and so does
//! this:
//!
//! - **I-term relax** (`iterm_relax`): a low-pass of each axis's setpoint at
//!   `iterm_relax_cutoff` is taken from the setpoint, leaving how fast the
//!   stick is moving. With `iterm_relax_type` SETPOINT, I grows by
//!   `max(0, 1 − that ÷ 40 °/s)` of the error; with GYRO, by the error
//!   between the low-passed setpoint and the gyro, less that quick part. RP
//!   relaxes roll and pitch, RPY yaw too, and the `_INC` kinds still let I
//!   shrink at full speed. In Angle the 40 °/s is 8 °/s.
//! - **Anti-gravity** (`anti_gravity_gain`): the mixer's throttle speed
//!   ([`crate::throttle_shaping`]), times the gain, adds 0.34 times itself to
//!   roll and pitch's I gain, and multiplies their P by 1 plus it times
//!   `anti_gravity_p_gain`% of 0.0034, less so once the setpoint passes
//!   50 °/s.
//! - **Dynamic D** (`d_max_*`): with D max above D, D is multiplied by
//!   between 1 and D max ÷ D. How far is driven by the larger of how fast the
//!   gyro moves (low-passed at 85 Hz, times `d_max_gain`) and the stick's
//!   speed, feedforward (times `d_max_advance`), each times 0.00008 ÷ 35;
//!   the result is low-passed at 35 Hz.
//! - **TPA** (`tpa_*`): D, and P too in `tpa_mode` PD, is multiplied by the
//!   TPA factor the mixer worked out from its throttle.

use opendrone_maths::{Fingerprinter, functions};

use crate::filter::{Lowpass, LowpassType, pt1_gain};
use crate::gyro_filter::{Dynamic, nyquist};
use crate::tune::{ItermRelax, ItermRelaxType, TpaMode, Tune};

/// Betaflight's gain scaling (`PTERM_SCALE`, `ITERM_SCALE`, `DTERM_SCALE`,
/// `FEEDFORWARD_SCALE`).
const P_SCALE: f64 = 0.032029;
const I_SCALE: f64 = 0.244381;
const D_SCALE: f64 = 0.000529;
const F_SCALE: f64 = 0.013754;
/// Yaw's I gain counts this many times (unless integrated yaw is on, which
/// OpenDrone doesn't simulate).
const YAW_I_FACTOR: f64 = 2.5;
/// Anti-gravity's share added to the I gain, and its share on P, per unit of
/// gain-weighted throttle speed (`ANTIGRAVITY_KI`, `ANTIGRAVITY_KP`).
const ANTI_GRAVITY_KI: f64 = 0.34;
const ANTI_GRAVITY_KP: f64 = 0.0034;
/// Above this setpoint, in °/s, anti-gravity's P boost fades: half at twice
/// it.
const ANTI_GRAVITY_SETPOINT: f64 = 50.0;
/// I-term relax's setpoint speed, in °/s, at which I stops growing
/// (`ITERM_RELAX_SETPOINT_THRESHOLD`).
const ITERM_RELAX_THRESHOLD: f64 = 40.0;
/// In Angle, I-term relax's threshold is this share of it.
const ANGLE_RELAX_SHARE: f64 = 0.2;
/// Dynamic D's low-passes, in Hz: on the gyro's speed, and on the result
/// (`D_MAX_RANGE_HZ`, `D_MAX_LOWPASS_HZ`).
const D_MAX_RANGE_HZ: f64 = 85.0;
const D_MAX_LOWPASS_HZ: f64 = 35.0;
/// Dynamic D's gain on the gyro's and the stick's speed
/// (`D_MAX_GYRO_GAIN_FACTOR`, `D_MAX_SETPOINT_GAIN_FACTOR`).
const D_MAX_GAIN_FACTOR: f64 = 0.00008;

/// What the mixer worked out from its throttle the loop before, for this
/// loop's terms ([`crate::throttle_shaping`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shaping {
    /// The TPA factor: 1 is no cut.
    pub tpa: f64,
    /// Anti-gravity: the throttle's speed, weighted towards low throttle and
    /// low-passed, times `anti_gravity_gain` (Betaflight's
    /// `antiGravityThrottleD` once the PID loop has multiplied it).
    pub anti_gravity: f64,
}

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

/// What the rest of the Flight Controller tells the PID loop each loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Modes {
    /// False zeroes every term, as Betaflight does with stabilisation off
    /// (`pidStabilisationState`).
    pub stabilising: bool,
    /// Empties I after the sum, as before Airmode starts at low throttle
    /// (`pidSetItermReset`).
    pub reset_iterm: bool,
    /// In yaw spin recovery.
    pub yaw_spin: bool,
    /// Flying Angle (`FLIGHT_MODE(ANGLE_MODE)`): I-term relax counts a
    /// quick stick from a fifth of its usual threshold.
    pub angle: bool,
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
    /// I-term relax: which kind, its setpoint low-pass on each axis (`None`
    /// with it off, as Betaflight leaves it unset), and whether those have
    /// settled.
    iterm_relax: ItermRelax,
    iterm_relax_type: ItermRelaxType,
    relax_lowpass: Option<[Lowpass; 3]>,
    /// Dynamic D: each axis's D max ÷ D (1: off on that axis), its gains on
    /// the gyro's and the stick's speed, and its two low-passes on each axis
    /// with whether each axis's have settled.
    d_max_ratio: [f64; 3],
    d_max_gyro_gain: f64,
    d_max_setpoint_gain: f64,
    d_max_range: [Lowpass; 3],
    d_max_lowpass: [Lowpass; 3],
    d_max_settled: [bool; 3],
    /// Anti-gravity's share on P, and whether TPA cuts P too.
    anti_gravity_p_gain: f64,
    tpa_mode: TpaMode,
    pub terms: [Terms; 3],
    /// Each axis's Dynamic D multiplier this loop, from 1 to D max ÷ D.
    pub d_max_factor: [f64; 3],
    /// Each axis's setpoint this loop compared with the gyro, in °/s
    /// (`previousPidSetpoint`).
    pub setpoint: [f64; 3],
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
        let pt2 = |cutoff_hz: f64| Lowpass::new(LowpassType::Pt2, cutoff_hz, dt, nyquist);
        let d_max_ratio = core::array::from_fn(|axis| {
            let (d, d_max) = (tune.pid[axis].d, tune.d_max[axis]);
            if d > 0 && d_max > d {
                f64::from(d_max) / f64::from(d)
            } else {
                1.0
            }
        });
        let relax_lowpass = (tune.iterm_relax != ItermRelax::Off).then(|| {
            core::array::from_fn(|_| {
                Lowpass::pt(1, pt1_gain(f64::from(tune.iterm_relax_cutoff), dt))
            })
        });
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
            iterm_relax: tune.iterm_relax,
            iterm_relax_type: tune.iterm_relax_type,
            relax_lowpass,
            d_max_ratio,
            // A stronger low-pass shrinks the boost's peak, so the gains
            // carry its cutoff inversely.
            d_max_gyro_gain: D_MAX_GAIN_FACTOR
                * f64::from(tune.d_max_gain)
                * (1.0 / D_MAX_LOWPASS_HZ),
            d_max_setpoint_gain: D_MAX_GAIN_FACTOR
                * f64::from(tune.d_max_advance)
                * (1.0 / D_MAX_LOWPASS_HZ),
            d_max_range: core::array::from_fn(|_| pt2(D_MAX_RANGE_HZ)),
            d_max_lowpass: core::array::from_fn(|_| pt2(D_MAX_LOWPASS_HZ)),
            d_max_settled: [false; 3],
            anti_gravity_p_gain: f64::from(tune.anti_gravity_p_gain) / 100.0 * ANTI_GRAVITY_KP,
            tpa_mode: tune.tpa_mode,
            terms: [Terms::default(); 3],
            d_max_factor: [1.0; 3],
            setpoint: [0.0; 3],
        }
    }

    /// One loop. `setpoint` is each axis's setpoint after RC smoothing and
    /// any levelling, `gyro` the filtered gyro and `feedforward` each axis's
    /// feedforward, after RC smoothing (0 on an axis Angle holds); `shaping` is what the mixer worked out from its
    /// throttle the loop before, and `modes` what else the loop is told.
    pub fn step(
        &mut self,
        setpoint: [f64; 3],
        gyro: [f64; 3],
        feedforward: [f64; 3],
        shaping: Shaping,
        modes: Modes,
    ) {
        let Modes {
            stabilising,
            reset_iterm,
            yaw_spin,
            angle,
        } = modes;
        let i_boost = shaping.anti_gravity * ANTI_GRAVITY_KI;
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
            self.setpoint[axis] = target;
            let error = target - gyro[axis];
            let previous_i = self.iterm[axis];
            let i_error = self.relax(axis, first, previous_i, target, gyro[axis], error, angle);
            let p_tpa = match self.tpa_mode {
                TpaMode::Pd => shaping.tpa,
                TpaMode::D => 1.0,
            };
            let mut p = self.kp[axis] * error * p_tpa;
            if axis == 2 {
                if first {
                    self.yaw_lowpass.settle(p);
                }
                p = self.yaw_lowpass.apply(p);
            }
            let limit = self.iterm_limit[axis];
            // No anti-gravity on yaw's I.
            let boost = if axis == 2 { 0.0 } else { i_boost };
            let mut i = crate::constrain(
                previous_i + (self.ki[axis] + boost) * self.dt * i_error,
                -limit,
                limit,
            );
            let mut d = if self.kd[axis] > 0.0 {
                let delta = -(d_gyro[axis] - self.previous_gyro[axis]) * self.frequency;
                let d = self.kd[axis] * delta * self.dynamic_d(axis, delta, feedforward[axis]);
                // TPA cuts D in either mode.
                d * shaping.tpa
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
            // Anti-gravity's P boost on roll and pitch, fading once the
            // setpoint passes 50 °/s: half at 100 °/s.
            if axis != 2 {
                let fade = functions::max(target.abs() / ANTI_GRAVITY_SETPOINT, 1.0);
                p *= 1.0 + (shaping.anti_gravity / fade) * self.anti_gravity_p_gain;
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

    /// I-term relax (`applyItermRelax`): the error I grows by this loop, from
    /// the axis's setpoint (after yaw spin recovery), its gyro reading and
    /// error, and its I term from the loop before. The setpoint's low-pass
    /// runs on every axis whenever relax is on, and settles on the first
    /// loop's setpoint. In Angle its threshold is a fifth.
    #[allow(clippy::too_many_arguments)]
    fn relax(
        &mut self,
        axis: usize,
        first: bool,
        previous_i: f64,
        setpoint: f64,
        gyro: f64,
        error: f64,
        angle: bool,
    ) -> f64 {
        let Some(lowpass) = self.relax_lowpass.as_mut() else {
            return error;
        };
        if first {
            lowpass[axis].settle(setpoint);
        }
        let setpoint_lowpass = lowpass[axis].apply(setpoint);
        let quick = (setpoint - setpoint_lowpass).abs();
        let on_yaw = matches!(self.iterm_relax, ItermRelax::Rpy | ItermRelax::RpyInc);
        if axis == 2 && !on_yaw {
            return error;
        }
        let threshold = if angle {
            ITERM_RELAX_THRESHOLD * ANGLE_RELAX_SHARE
        } else {
            ITERM_RELAX_THRESHOLD
        };
        let factor = functions::max(0.0, 1.0 - quick / threshold);
        let shrinking = (previous_i > 0.0 && error < 0.0) || (previous_i < 0.0 && error > 0.0);
        let lets_shrink = matches!(self.iterm_relax, ItermRelax::RpInc | ItermRelax::RpyInc);
        if lets_shrink && shrinking {
            error
        } else {
            match self.iterm_relax_type {
                ItermRelaxType::Setpoint => error * factor,
                ItermRelaxType::Gyro => deadband(setpoint_lowpass - gyro, quick),
            }
        }
    }

    /// Dynamic D's multiplier for one axis this loop, from 1 to D max ÷ D,
    /// given how fast the D term's gyro moves, in °/s², and the axis's
    /// feedforward: 1 with Dynamic D off on the axis.
    fn dynamic_d(&mut self, axis: usize, delta: f64, feedforward: f64) -> f64 {
        let ratio = self.d_max_ratio[axis];
        if ratio <= 1.0 {
            return 1.0;
        }
        if !self.d_max_settled[axis] {
            self.d_max_range[axis].settle(delta);
        }
        let gyro_factor = self.d_max_range[axis].apply(delta).abs() * self.d_max_gyro_gain;
        let setpoint_factor = feedforward.abs() * self.d_max_setpoint_gain;
        let boost = functions::max(gyro_factor, setpoint_factor);
        // The boost starts at 0 and reaches D max at 1, but may pass it.
        let multiplier = 1.0 + (ratio - 1.0) * boost;
        if !self.d_max_settled[axis] {
            self.d_max_settled[axis] = true;
            self.d_max_lowpass[axis].settle(multiplier);
        }
        let multiplier = functions::min(self.d_max_lowpass[axis].apply(multiplier), ratio);
        self.d_max_factor[axis] = multiplier;
        multiplier
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
        if let Some(lowpass) = &self.relax_lowpass {
            for filter in lowpass {
                filter.write_fingerprint(f);
            }
        }
        for filter in self.d_max_range.iter().chain(&self.d_max_lowpass) {
            filter.write_fingerprint(f);
        }
        for settled in self.d_max_settled {
            f.write_u64(u64::from(settled));
        }
        f.write_f64s(&self.d_max_factor);
        f.write_f64s(&self.setpoint);
        for terms in self.terms {
            f.write_f64s(&[terms.p, terms.i, terms.d, terms.f, terms.sum]);
        }
    }
}

/// Betaflight's `fapplyDeadband`: zero inside the deadband, and the rest
/// moved in by it.
fn deadband(value: f64, deadband: f64) -> f64 {
    if value.abs() < deadband {
        0.0
    } else if value >= 0.0 {
        value - deadband
    } else {
        value + deadband
    }
}
