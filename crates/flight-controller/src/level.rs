//! Angle and Horizon, the self-levelling Flight Modes, reimplemented from
//! Betaflight 2026.6.2's `pidLevel`, `calcHorizonLevelStrength` and the
//! levelling part of `pidController` (`src/main/flight/pid.c`), their set-up
//! in `pidInitFilters` and `pidInitConfig` (`pid_init.c`), and the mode
//! logic in `processRxModes` (`src/main/fc/core.c`).
//!
//! # Angle
//!
//! Roll and pitch's sticks ask for a tilt instead of a rotation speed: each
//! axis's setpoint (after RC smoothing), as a share of its Rates' max rate,
//! times `angle_limit`, so full stick asks for `angle_limit` degrees and half
//! stick for whatever share of the max rate the Rates give at half stick.
//! The setpoint the PID loop works to is then
//!
//! - how far the Quad's true attitude is from that tilt, in degrees, times
//!   `angle_p_gain` ÷ 10 (°/s for each degree off),
//! - plus feedforward: the axis's feedforward, as the same share of the max
//!   rate, times `angle_limit` and `angle_feedforward`%, through a
//!   third-order low-pass with a time constant of
//!   `angle_feedforward_smoothing_ms`,
//! - plus, with `angle_earth_ref`, a share of the yaw stick's rate, so a yaw
//!   while tilted turns about the world's vertical: on roll, minus the yaw
//!   rate times the sine of the pitch the sticks ask for; on pitch, plus the
//!   yaw rate times the sine of the roll they ask for. Each axis reads the
//!   other's tilt and the yaw rate as Betaflight's loop leaves them: roll
//!   pitch's from the loop before, pitch roll's from this loop, and both the
//!   yaw rate from the loop before. The yaw rate itself is cut by the cosine
//!   of the larger tilt asked for, times `angle_earth_ref`%,
//!
//! all through a third-order low-pass at 50 Hz. Yaw stays in Acro. On the
//! levelled axes the PID loop's own feedforward is 0, and so is the stick
//! speed Dynamic D reads; I-term relax counts a quick stick from 8 °/s
//! instead of 40.
//!
//! # Horizon
//!
//! Horizon works out Angle's setpoint the same way, then blends it with the
//! Acro one by a strength from 0 to 1:
//!
//! - 1 level, falling in a straight line to 0 at a tilt (the larger of the
//!   roll and pitch angles) of `horizon_limit_degrees`;
//! - times 1 with the sticks centred, falling to 0 at a roll or pitch stick
//!   (the larger) of `horizon_limit_sticks` hundredths, unless
//!   `horizon_ignore_sticks` is on; the sticks are each frame's, through RC
//!   smoothing's own low-pass on them while Horizon flies;
//! - times `horizon_level_strength`%, at most 1;
//! - and, with `horizon_delay_ms`, never more than its own first-order
//!   low-pass with that time constant, so a strength that falls comes back
//!   slowly.
//!
//! The yaw rate's cut for `angle_earth_ref` is scaled by the strength too.
//! The PID loop's feedforward, Dynamic D and I-term relax work as in Acro.
//!
//! # Picking the Flight Mode
//!
//! As Betaflight's `processRxModes` does with each frame: Angle when its
//! switch is on in the Modes table, or while Failsafe's stage 2 runs;
//! otherwise Horizon when its switch is on; otherwise Acro. With
//! `level_race_mode` on, Angle and Horizon level only roll, and pitch flies
//! as in Acro, with no earth reference.
//!
//! # What differs from a real board
//!
//! Betaflight levels against its own estimate of the attitude, worked out
//! 100 to about 800 times a second from the gyro and accelerometer and kept
//! in tenths of a degree. OpenDrone's Flight Controller levels against the
//! true attitude, which the Simulation hands it as a sensor reading every
//! loop (#21). Betaflight's `sin_approx` and `cos_approx` are the exact sine
//! and cosine here, which differ by about a millionth.
//!
//! # A fresh Flight Controller
//!
//! A Flight Controller that levels from its first frame on starts with each
//! low-pass settled on its first input, as if it had flown that Flight Mode
//! since power-up. A switch into Angle or Horizon later finds the low-passes
//! as Betaflight leaves them: as the last levelling left them, or at 0 if it
//! never levelled, as Betaflight's start at power-up. So Horizon's strength
//! then rises over `horizon_delay_ms`.

use core::f64::consts::PI;

use opendrone_maths::{Attitude, DEGREE, Fingerprinter, Vec3, functions};

use crate::FlightMode;
use crate::filter::{Lowpass, pt1_gain, pt3_gain};
use crate::tune::Tune;

/// The cutoff of the low-pass on the levelling's rate, in Hz
/// (`ATTITUDE_CUTOFF_HZ`).
const ATTITUDE_CUTOFF_HZ: f64 = 50.0;

/// What the levelling reads each loop.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Inputs {
    pub mode: FlightMode,
    /// Each axis's setpoint after RC smoothing, in Betaflight's axes and °/s
    /// (`getSetpointRate`).
    pub setpoint: [f64; 3],
    /// Each axis's feedforward after RC smoothing (`getFeedforward`).
    pub feedforward: [f64; 3],
    /// The roll and pitch sticks' deflection from −1 to +1, as Horizon reads
    /// them (`getRcDeflection`).
    pub deflection: [f64; 2],
    /// The true attitude, as the sensors read it.
    pub attitude: Attitude,
    /// The loop works out the first frame since power-up: the low-passes
    /// settle if it levels.
    pub first_frame: bool,
}

/// What the levelling gives the PID loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Levelled {
    /// Each axis's setpoint for the PID loop, in Betaflight's axes and °/s.
    pub setpoint: [f64; 3],
    /// Each axis's feedforward for the PID loop: 0 on the axes Angle holds.
    pub feedforward: [f64; 3],
    /// Horizon's strength this loop, from 0 to 1: 0 outside Horizon.
    pub horizon_strength: f64,
}

/// Angle and Horizon's settings and memory.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Levelling {
    angle_limit: f64,
    /// `angle_p_gain` ÷ 10 (`angleGain`).
    angle_gain: f64,
    /// `angle_feedforward` ÷ 100 (`angleFeedforwardGain`).
    feedforward_gain: f64,
    /// `angle_earth_ref` ÷ 100 (`angleEarthRef`).
    earth_ref: f64,
    /// `horizon_level_strength` ÷ 100, at most 1 (`horizonGain`).
    horizon_gain: f64,
    /// 1 with `horizon_ignore_sticks` on, else 0.
    ignore_sticks: f64,
    /// `horizon_limit_sticks` ÷ 100, and the inverse.
    limit_sticks: f64,
    limit_sticks_inverse: f64,
    /// `horizon_limit_degrees`, and the inverse.
    limit_degrees: f64,
    limit_degrees_inverse: f64,
    race_mode: bool,
    /// Each axis's max rate, in °/s (`getMaxRcRate`): roll, pitch.
    max_rate: [f64; 2],
    /// Horizon's strength low-pass (`horizonSmoothingPt1`), `None` with
    /// `horizon_delay_ms` at 0, as Betaflight's code reads it (its CLI
    /// allows no less than 10 ms).
    horizon_lowpass: Option<Lowpass>,
    /// The low-passes on roll and pitch's levelling rate
    /// (`attitudeFilter`) and on their feedforward (`angleFeedforwardPt3`).
    rate_lowpass: [Lowpass; 2],
    feedforward_lowpass: [Lowpass; 2],
    /// The tilt each of roll and pitch last asked for, in degrees
    /// (`angleTarget`).
    target: [f64; 2],
    /// The yaw rate the earth reference last read (`angleYawSetpoint`).
    yaw_setpoint: f64,
}

impl Levelling {
    pub fn new(tune: &Tune, max_rate: [f64; 2], loop_hz: u32) -> Levelling {
        let dt = 1.0 / f64::from(loop_hz.max(1));
        let feedforward_hz = 1000.0 / (2.0 * PI * f64::from(tune.angle_feedforward_smoothing_ms));
        let rate_k = pt3_gain(ATTITUDE_CUTOFF_HZ, dt);
        let feedforward_k = pt3_gain(feedforward_hz, dt);
        let horizon_lowpass = (tune.horizon_delay_ms > 0).then(|| {
            let hz = 1000.0 / (2.0 * PI * f64::from(tune.horizon_delay_ms));
            Lowpass::pt(1, pt1_gain(hz, dt))
        });
        let limit_sticks = f64::from(tune.horizon_limit_sticks) / 100.0;
        let limit_degrees = f64::from(tune.horizon_limit_degrees);
        Levelling {
            angle_limit: f64::from(tune.angle_limit),
            angle_gain: f64::from(tune.angle_p_gain) / 10.0,
            feedforward_gain: f64::from(tune.angle_feedforward) / 100.0,
            earth_ref: f64::from(tune.angle_earth_ref) / 100.0,
            horizon_gain: functions::min(f64::from(tune.horizon_level_strength) / 100.0, 1.0),
            ignore_sticks: if tune.horizon_ignore_sticks { 1.0 } else { 0.0 },
            limit_sticks,
            limit_sticks_inverse: if tune.horizon_limit_sticks > 0 {
                1.0 / limit_sticks
            } else {
                1.0
            },
            limit_degrees,
            limit_degrees_inverse: if tune.horizon_limit_degrees > 0 {
                1.0 / limit_degrees
            } else {
                1.0
            },
            race_mode: tune.level_race_mode,
            max_rate,
            horizon_lowpass,
            rate_lowpass: core::array::from_fn(|_| Lowpass::pt(3, rate_k)),
            feedforward_lowpass: core::array::from_fn(|_| Lowpass::pt(3, feedforward_k)),
            target: [0.0; 2],
            yaw_setpoint: 0.0,
        }
    }

    /// One loop's levelling (the levelling part of `pidController`): the
    /// setpoints and feedforward the PID loop works with.
    pub fn step(&mut self, inputs: Inputs) -> Levelled {
        let Inputs {
            mode,
            setpoint,
            feedforward,
            deflection,
            attitude,
            first_frame,
        } = inputs;
        let mut levelled = Levelled {
            setpoint,
            feedforward,
            horizon_strength: 0.0,
        };
        if mode == FlightMode::Acro {
            return levelled;
        }
        let angles = betaflight_angles(attitude);
        let horizon = mode == FlightMode::Horizon;
        if horizon {
            levelled.horizon_strength = self.horizon_strength(angles, deflection, first_frame);
        }
        // Race mode levels roll only (`LEVEL_MODE_R`); otherwise roll and
        // pitch (`LEVEL_MODE_RP`).
        let axes = if self.race_mode { 1 } else { 2 };
        for axis in 0..axes {
            levelled.setpoint[axis] = self.level(
                axis,
                setpoint[axis],
                feedforward[axis],
                angles[axis],
                horizon,
                levelled.horizon_strength,
                first_frame,
            );
            // Angle holds the axis wholly: the PID loop's feedforward, and
            // the stick speed Dynamic D reads, are 0 on it.
            if !horizon {
                levelled.feedforward[axis] = 0.0;
            }
        }
        if !self.race_mode && self.earth_ref != 0.0 {
            // The yaw rate is kept for roll and pitch's next loop, then cut
            // while tilted.
            self.yaw_setpoint = setpoint[2];
            let mut tilt =
                self.earth_ref * functions::max(self.target[0].abs(), self.target[1].abs());
            if horizon {
                tilt *= levelled.horizon_strength;
            }
            levelled.setpoint[2] = setpoint[2] * functions::cos(tilt * DEGREE);
        }
        levelled
    }

    /// One axis's levelling (`pidLevel`): its setpoint for the PID loop.
    #[allow(clippy::too_many_arguments)]
    fn level(
        &mut self,
        axis: usize,
        setpoint: f64,
        feedforward: f64,
        angle: f64,
        horizon: bool,
        strength: f64,
        first_frame: bool,
    ) -> f64 {
        let max_rate_inverse = 1.0 / self.max_rate[axis];
        let mut angle_feedforward =
            self.angle_limit * feedforward * self.feedforward_gain * max_rate_inverse;
        if first_frame {
            self.feedforward_lowpass[axis].settle(angle_feedforward);
        }
        angle_feedforward = self.feedforward_lowpass[axis].apply(angle_feedforward);
        let target = crate::constrain(
            self.angle_limit * setpoint * max_rate_inverse,
            -self.angle_limit,
            self.angle_limit,
        );
        let mut rate = (target - angle) * self.angle_gain + angle_feedforward;
        // The earth reference: roll reads pitch's tilt (from the loop
        // before), pitch roll's (from this loop).
        let other = self.target[1 - axis];
        let sign = if axis == 0 { -1.0 } else { 1.0 };
        rate += self.yaw_setpoint * functions::sin(other * DEGREE) * sign * self.earth_ref;
        self.target[axis] = target;
        if first_frame {
            self.rate_lowpass[axis].settle(rate);
        }
        let rate = self.rate_lowpass[axis].apply(rate);
        if horizon {
            setpoint * (1.0 - strength) + rate * strength
        } else {
            rate
        }
    }

    /// Horizon's strength this loop, from 0 to 1
    /// (`calcHorizonLevelStrength`), from the roll and pitch angles in
    /// degrees and the roll and pitch sticks' deflection.
    fn horizon_strength(
        &mut self,
        angles: [f64; 2],
        deflection: [f64; 2],
        first_frame: bool,
    ) -> f64 {
        let inclination = functions::max(angles[0].abs(), angles[1].abs());
        let stick = functions::max(deflection[0].abs(), deflection[1].abs());
        let mut strength = functions::max(
            (self.limit_degrees - inclination) * self.limit_degrees_inverse,
            0.0,
        );
        strength *= functions::max(
            (self.limit_sticks - stick) * self.limit_sticks_inverse,
            self.ignore_sticks,
        );
        strength *= self.horizon_gain;
        if let Some(lowpass) = &mut self.horizon_lowpass {
            if first_frame {
                lowpass.settle(strength);
            }
            strength = functions::min(strength, lowpass.apply(strength));
        }
        strength
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        if let Some(lowpass) = &self.horizon_lowpass {
            lowpass.write_fingerprint(f);
        }
        for filter in self.rate_lowpass.iter().chain(&self.feedforward_lowpass) {
            filter.write_fingerprint(f);
        }
        f.write_f64s(&self.target);
        f.write_f64(self.yaw_setpoint);
    }
}

/// The roll and pitch angles Betaflight levels against, in degrees, from an
/// attitude (`imuUpdateEulerAngles`): roll right side down positive, from
/// −180° to 180°, and pitch nose down positive, from −90° to 90°, each
/// worked out from where the world's up points in the body.
pub(crate) fn betaflight_angles(attitude: Attitude) -> [f64; 2] {
    let up = attitude.world_to_body(Vec3::new(0.0, 0.0, 1.0));
    let roll = functions::atan2(up.y, up.z);
    let pitch = functions::asin(crate::constrain(-up.x, -1.0, 1.0));
    [roll / DEGREE, pitch / DEGREE]
}
