//! Feedforward: Betaflight 2026.6.2's `calculateFeedforward` and
//! `updateFeedforwardFilters` (`src/main/fc/rc.c`), with its settings from
//! `pidInitConfig` (`src/main/flight/pid_init.c`), reimplemented from their
//! behaviour; the Betaflight research §3 and §3.1.
//!
//! Feedforward pushes the Quad round as soon as the stick moves, before the
//! gyro sees any error. Once per Radio Link frame, each axis:
//!
//! 1. **Stick speed:** the setpoint's change since the last frame times the
//!    frame's rate (from its own gap). The receiver is ExpressLRS on CRSF,
//!    for which Betaflight turns off its handling of repeated frames
//!    (`feedforwardInterpolate`), so a repeated frame simply gives no speed.
//! 2. **Smoothing:** two first-order low-passes, one on the speed and one on
//!    its change (the stick's acceleration). Their time constant is
//!    `feedforward_smooth_factor` at a 250 Hz Packet Rate,
//!    `(1/250 s × factor) ÷ (1 − factor)`, about 7.4 ms at 65%; they are
//!    tuned to the detected Packet Rate every third frame, so the delay stays
//!    about the same at any rate.
//! 3. **Boost** (roll and pitch): the acceleration × the frame's rate ×
//!    `feedforward_boost` ÷ 1000 is added.
//! 4. **Jitter attenuation:** feedforward is scaled by `min(1, (the stick's
//!    average change over the last two frames, in µs, + 1) ÷ (1 +
//!    feedforward_jitter_factor))`, so a stick moving less than about
//!    `feedforward_jitter_factor` µs a frame gets less.
//! 5. **Max-rate limit** (roll and pitch): pushing further out, feedforward
//!    is held to `(max rate − |setpoint|) × feedforward_max_rate_limit`,
//!    the setting's number as it stands (90, not 0.9), so it fades to
//!    nothing as the stick reaches full.
//! 6. **Yaw hold** (yaw): `feedforward_yaw_hold_gain × (setpoint − the
//!    setpoint low-passed with the time constant feedforward_yaw_hold_time)`
//!    is added, so yaw keeps pushing a little after the stick stops. Below
//!    100 ms the gain is raised by `150 ÷ (time + 50)`.
//! 7. **Transition:** below a stick deflection of `feedforward_transition`
//!    hundredths, feedforward fades in proportion towards centre.
//! 8. **Averaging:** the last `feedforward_averaging` frames' feedforward is
//!    averaged.
//!
//! The PID loop's F term is `0.013754 × F gain ÷ 100 × feedforward`, after RC
//! smoothing ([`crate::rc_smoothing`]). Between frames feedforward holds.
//!
//! A fresh Flight Controller's feedforward starts settled with the sticks
//! still where the first frame has them: no speed, nothing to average.

use opendrone_maths::{Fingerprinter, functions};

use crate::constrain;
use crate::filter::{Lowpass, pt1_gain_from_delay};
use crate::rates::{Axis, Rates};
use crate::tune::Tune;

/// The Packet Rate `feedforward_smooth_factor` is set for, in Hz.
const SMOOTHING_SET_AT_HZ: f64 = 250.0;

/// What one frame gives feedforward, per axis in Betaflight's order.
pub(crate) struct FeedforwardFrame {
    /// The sticks as commands, from −500 to 500 µs after the deadband
    /// (`rcCommand`).
    pub sticks: [f64; 3],
    /// The setpoints, in °/s, before smoothing (`rawSetpoint`).
    pub setpoints: [f64; 3],
    /// How far each stick is from centre, from 0 to 1
    /// (`rcDeflectionAbs`).
    pub deflections: [f64; 3],
    /// The frame's gap, in seconds, and its rate, in Hz
    /// (`currentRxIntervalUs`, `currentRxRateHz`).
    pub gap: f64,
    pub rate_hz: f64,
}

/// Feedforward's memory.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Feedforward {
    /// The smoothing's time constant, in seconds
    /// (`feedforwardSmoothFactor`).
    smoothing_tau: f64,
    jitter_inverse: f64,
    boost: f64,
    max_rate_limit: f64,
    transition: f64,
    transition_inverse: f64,
    yaw_hold_time: f64,
    yaw_hold_gain: f64,
    /// Each axis's rate at full stick, in °/s (`maxRcRate`).
    max_rate: [f64; 3],
    /// Each axis's stick, change, setpoint and speed from the frame before
    /// (`prevRcCommand`, `prevRcCommandDeltaAbs`, `prevSetpoint`,
    /// `prevSetpointSpeed`).
    previous_stick: [f64; 3],
    previous_change: [f64; 3],
    previous_setpoint: [f64; 3],
    previous_speed: [f64; 3],
    /// The low-passes on the speed and its change (`filterSetpointSpeed`,
    /// `filterSetpointDelta`), and yaw hold's (`feedforwardYawHoldLpf`).
    speed_filter: [Lowpass; 3],
    change_filter: [Lowpass; 3],
    yaw_hold_filter: Lowpass,
    /// Each axis's last few feedforward values, for the averaging, oldest
    /// replaced first (`laggedMovingAverage`).
    recent: [Vec<f64>; 3],
    next: usize,
    settled: bool,
    /// The newest frame's feedforward, per axis (`feedforwardRaw`).
    pub raw: [f64; 3],
}

impl Feedforward {
    /// At power-up, tuned to the detected Packet Rate.
    pub fn new(tune: &Tune, rates: &Rates, detected_hz: f64) -> Feedforward {
        let factor = 0.01 * f64::from(tune.feedforward_smooth_factor);
        let set_at = 1.0 / SMOOTHING_SET_AT_HZ;
        let transition = f64::from(tune.feedforward_transition);
        let hold_time = f64::from(tune.feedforward_yaw_hold_time);
        let mut hold_gain = f64::from(tune.feedforward_yaw_hold_gain);
        if tune.feedforward_yaw_hold_time < 100 {
            hold_gain *= 150.0 / (hold_time + 50.0);
        }
        let frames = tune.feedforward_averaging.frames();
        let mut feedforward = Feedforward {
            smoothing_tau: set_at * factor / (1.0 - factor),
            jitter_inverse: 1.0 / (1.0 + f64::from(tune.feedforward_jitter_factor)),
            boost: 0.001 * f64::from(tune.feedforward_boost),
            max_rate_limit: f64::from(tune.feedforward_max_rate_limit),
            transition: transition / 100.0,
            transition_inverse: if transition == 0.0 {
                0.0
            } else {
                100.0 / transition
            },
            yaw_hold_time: 0.001 * hold_time,
            yaw_hold_gain: hold_gain,
            max_rate: Axis::ALL.map(|axis| rates.max_rate(axis)),
            previous_stick: [0.0; 3],
            previous_change: [0.0; 3],
            previous_setpoint: [0.0; 3],
            previous_speed: [0.0; 3],
            speed_filter: core::array::from_fn(|_| Lowpass::pt(1, 1.0)),
            change_filter: core::array::from_fn(|_| Lowpass::pt(1, 1.0)),
            yaw_hold_filter: Lowpass::pt(1, 0.0),
            recent: core::array::from_fn(|_| vec![0.0; if frames > 1 { frames } else { 0 }]),
            next: 0,
            settled: false,
            raw: [0.0; 3],
        };
        feedforward.retune(detected_hz);
        feedforward
    }

    /// Tunes the smoothing to the detected Packet Rate
    /// (`updateFeedforwardFilters`).
    pub fn retune(&mut self, detected_hz: f64) {
        let k = pt1_gain_from_delay(self.smoothing_tau, 1.0 / detected_hz);
        for filter in self.speed_filter.iter_mut().chain(&mut self.change_filter) {
            filter.set_k(k);
        }
    }

    /// One frame's feedforward, every axis (`calculateFeedforward`).
    pub fn frame(&mut self, frame: &FeedforwardFrame) {
        if !self.settled {
            // As if the sticks had rested where this frame has them.
            self.settled = true;
            self.previous_stick = frame.sticks;
            self.previous_setpoint = frame.setpoints;
            self.yaw_hold_filter.settle(frame.setpoints[2]);
        }
        for axis in Axis::ALL {
            let a = axis.index();
            let change = (frame.sticks[a] - self.previous_stick[a]).abs();
            let setpoint = frame.setpoints[a];
            let setpoint_change = setpoint - self.previous_setpoint[a];
            self.previous_stick[a] = frame.sticks[a];
            self.previous_setpoint[a] = setpoint;

            // 1. The stick's speed: over CRSF, with no handling of repeats.
            let speed = setpoint_change * frame.rate_hz;

            // 4. Jitter attenuation, from this frame's change and the last.
            let jitter = functions::min(
                ((change + self.previous_change[a]) * 0.5 + 1.0) * self.jitter_inverse,
                1.0,
            );
            self.previous_change[a] = change;

            // 2. Smoothing the speed, then its change.
            let speed = self.speed_filter[a].apply(speed);
            let acceleration = speed - self.previous_speed[a];
            self.previous_speed[a] = speed;
            let acceleration = self.change_filter[a].apply(acceleration);

            let mut feedforward = speed;
            if axis == Axis::Yaw {
                feedforward *= jitter;
                // 6. Yaw hold.
                self.yaw_hold_filter
                    .set_k(pt1_gain_from_delay(self.yaw_hold_time, frame.gap));
                let held = self.yaw_hold_filter.apply(setpoint);
                feedforward += self.yaw_hold_gain * (setpoint - held);
            } else {
                // 3. Boost, then 4. jitter, then 5. the max-rate limit.
                feedforward += acceleration * frame.rate_hz * self.boost;
                feedforward *= jitter;
                if self.max_rate_limit > 0.0 && feedforward * setpoint > 0.0 {
                    let limit = (self.max_rate[a] - setpoint.abs()) * self.max_rate_limit;
                    feedforward = if limit > 0.0 {
                        constrain(feedforward, -limit, limit)
                    } else {
                        0.0
                    };
                }
            }

            // 7. Transition.
            if self.transition != 0.0 && frame.deflections[a] < self.transition {
                feedforward *= frame.deflections[a] * self.transition_inverse;
            }

            // 8. Averaging.
            let recent = &mut self.recent[a];
            if !recent.is_empty() {
                recent[self.next] = feedforward;
                feedforward = recent.iter().sum::<f64>() / recent.len() as f64;
            }
            self.raw[a] = feedforward;
        }
        let window = self.recent[0].len();
        if window > 0 {
            self.next = (self.next + 1) % window;
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for values in [
            &self.previous_stick,
            &self.previous_change,
            &self.previous_setpoint,
            &self.previous_speed,
            &self.raw,
        ] {
            f.write_f64s(values);
        }
        for filter in self
            .speed_filter
            .iter()
            .chain(&self.change_filter)
            .chain([&self.yaw_hold_filter])
        {
            filter.write_fingerprint(f);
        }
        for recent in &self.recent {
            f.write_f64s(recent);
        }
        f.write_u64(self.next as u64);
        f.write_u64(u64::from(self.settled));
    }
}
