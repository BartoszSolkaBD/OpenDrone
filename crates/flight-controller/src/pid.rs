//! The PID loop in rate mode, reimplemented from Betaflight 2026.6.2's
//! `pidController` (`src/main/flight/pid.c`) and its gain scaling
//! (`pid_init.c`, `pid.h`); the Betaflight research §3.
//!
//! Each axis compares the setpoint with the gyro (in °/s) and corrects the
//! error with Betaflight's own scaling, so a pilot's "P 45" means here what
//! it means on their quad:
//!
//! - P = 0.032029 × P gain × error.
//! - I grows by 0.244381 × I gain × the loop's step × error each loop (yaw's
//!   I gain counts 2.5 times), within ± `iterm_windup`% of the PID-sum limit.
//! - D = 0.000529 × D gain × how fast the gyro reading falls, per second:
//!   on the measurement, so a stick move doesn't kick it.
//! - The sum is P + I + D (+ F, feedforward, which arrives with #49).
//!
//! During yaw spin recovery the yaw setpoint is 0, I is emptied on every
//! axis, and roll and pitch's P, D and F are 0, so only yaw's P and D work,
//! braking the spin.
//!
//! Not here yet: the gyro, D-term and yaw P low-pass filters, RC smoothing
//! and feedforward (#49), and anti-gravity, I-term relax, TPA and Dynamic D
//! (#50). With those missing, D works on the clean gyro straight from the
//! sensor reading.

use opendrone_maths::Fingerprinter;

use crate::tune::Tune;

/// Betaflight's gain scaling (`PTERM_SCALE`, `ITERM_SCALE`, `DTERM_SCALE`).
const P_SCALE: f64 = 0.032029;
const I_SCALE: f64 = 0.244381;
const D_SCALE: f64 = 0.000529;
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
    iterm_limit: [f64; 3],
    /// The loop's step, in seconds, and how many loops a second.
    dt: f64,
    frequency: f64,
    /// Each axis's I term, carried from loop to loop.
    iterm: [f64; 3],
    /// The gyro reading D compared with, from the loop before.
    previous_gyro: [f64; 3],
    pub terms: [Terms; 3],
}

impl Pid {
    /// A fresh loop: I empty, and D's memory starting from the current gyro
    /// reading, so the first loop gets no artificial kick.
    pub fn new(tune: &Tune, loop_hz: u32, gyro: [f64; 3]) -> Pid {
        let mut kp = [0.0; 3];
        let mut ki = [0.0; 3];
        let mut kd = [0.0; 3];
        for (axis, gains) in tune.pid.iter().enumerate() {
            kp[axis] = P_SCALE * f64::from(gains.p);
            ki[axis] = I_SCALE * f64::from(gains.i);
            kd[axis] = D_SCALE * f64::from(gains.d);
        }
        ki[2] *= YAW_I_FACTOR;
        let windup = 0.01 * f64::from(tune.iterm_windup);
        let iterm_limit = [
            windup * f64::from(tune.pidsum_limit),
            windup * f64::from(tune.pidsum_limit),
            windup * f64::from(tune.pidsum_limit_yaw),
        ];
        let frequency = f64::from(loop_hz);
        Pid {
            kp,
            ki,
            kd,
            iterm_limit,
            dt: 1.0 / frequency,
            frequency,
            iterm: [0.0; 3],
            previous_gyro: gyro,
            terms: [Terms::default(); 3],
        }
    }

    /// One loop. `stabilising` false zeroes every term, as Betaflight does
    /// with stabilisation off; `reset_iterm` empties I after the sum, as
    /// before Airmode starts at low throttle; `yaw_spin` is true during yaw
    /// spin recovery.
    pub fn step(
        &mut self,
        setpoint: [f64; 3],
        gyro: [f64; 3],
        stabilising: bool,
        reset_iterm: bool,
        yaw_spin: bool,
    ) {
        for axis in 0..3 {
            let target = if axis == 2 && yaw_spin {
                0.0
            } else {
                setpoint[axis]
            };
            let error = target - gyro[axis];
            let mut p = self.kp[axis] * error;
            let limit = self.iterm_limit[axis];
            let mut i = (self.iterm[axis] + self.ki[axis] * self.dt * error).clamp(-limit, limit);
            let mut d = if self.kd[axis] > 0.0 {
                self.kd[axis] * -(gyro[axis] - self.previous_gyro[axis]) * self.frequency
            } else {
                0.0
            };
            self.previous_gyro[axis] = gyro[axis];
            let f = 0.0;
            if yaw_spin {
                i = 0.0;
                if axis < 2 {
                    p = 0.0;
                    d = 0.0;
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
        if !stabilising {
            self.iterm = [0.0; 3];
            self.terms = [Terms::default(); 3];
        } else if reset_iterm {
            self.iterm = [0.0; 3];
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64s(&self.iterm);
        f.write_f64s(&self.previous_gyro);
        for terms in self.terms {
            f.write_f64s(&[terms.p, terms.i, terms.d, terms.f, terms.sum]);
        }
    }
}
