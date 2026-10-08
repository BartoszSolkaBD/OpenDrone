//! Input smoothing: the Assist that smooths the pilot's roll, pitch and yaw
//! before the Radio Link, for an Input Device whose sticks feel steppy, such
//! as a Gamepad's 8-bit sticks (#21, #27).
//!
//! It is one fixed, gentle low-pass: a first-order filter at 15 Hz
//! ([`INPUT_SMOOTHING_HZ`]) on roll, pitch and yaw, which delays a stick move
//! by about 10.6 ms (1 ÷ (2π × 15 Hz)). The throttle and the switches pass
//! untouched. It works on the newest Channels as time passes, one physics
//! step at a time, as a filter on the stick itself would: after a stick
//! jumps, the smoothed stick has covered 1 − e^(−2π × 15 Hz × t) of the jump
//! t seconds later, exactly, at every step. A Radio Link frame carries the
//! smoothed sticks as they stand when it leaves, rounded to the nearest
//! whole-number step a receiver outputs.
//!
//! It is separate from Betaflight's own RC smoothing in the Flight
//! Controller, which is always on.

use opendrone_flight_controller::{Channel, Channels};
use opendrone_maths::{Fingerprinter, functions};

use crate::PhysicsRate;

/// Input smoothing's corner frequency: gentle, about 10 ms of delay (#21).
pub const INPUT_SMOOTHING_HZ: f64 = 15.0;

/// Input smoothing for one Quad's pilot.
#[derive(Clone, Debug, PartialEq)]
pub struct InputSmoothing {
    /// The share of the way to the newest sticks one physics step closes:
    /// 1 − e^(−2π × 15 Hz ÷ physics rate).
    gain: f64,
    /// Roll, pitch and yaw as smoothed so far, in the receiver's steps, not
    /// rounded; `None` until the first Channels arrive.
    smoothed: Option<[f64; 3]>,
    /// The newest roll, pitch and yaw, in the receiver's steps.
    newest: [f64; 3],
}

impl InputSmoothing {
    pub fn new(physics_rate: PhysicsRate) -> InputSmoothing {
        let step = -2.0 * core::f64::consts::PI * INPUT_SMOOTHING_HZ * physics_rate.step_length();
        InputSmoothing {
            gain: -functions::exp_m1(step),
            smoothed: None,
            newest: [0.0; 3],
        }
    }

    /// New Channels arrived. The first ones are taken as they are, so the
    /// sticks start where the pilot holds them.
    pub fn hear(&mut self, channels: &Channels) {
        self.newest = sticks(channels);
        if self.smoothed.is_none() {
            self.smoothed = Some(self.newest);
        }
    }

    /// A frame's Channels with roll, pitch and yaw smoothed, as they stand
    /// now; the throttle and the switches as they came.
    pub fn smooth(&self, channels: Channels) -> Channels {
        let Some([roll, pitch, yaw]) = self.smoothed else {
            return channels;
        };
        Channels {
            roll: nearest(roll, channels.roll),
            pitch: nearest(pitch, channels.pitch),
            yaw: nearest(yaw, channels.yaw),
            ..channels
        }
    }

    /// One physics step passes: the smoothed sticks close
    /// [`INPUT_SMOOTHING_HZ`]'s share of the way to the newest.
    pub fn step(&mut self) {
        if let Some(smoothed) = &mut self.smoothed {
            for (smoothed, newest) in smoothed.iter_mut().zip(self.newest) {
                *smoothed += self.gain * (newest - *smoothed);
            }
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        match &self.smoothed {
            None => f.write_u64(0),
            Some(smoothed) => {
                f.write_u64(1);
                f.write_f64s(smoothed);
            }
        }
        f.write_f64s(&self.newest);
    }
}

fn sticks(channels: &Channels) -> [f64; 3] {
    [channels.roll, channels.pitch, channels.yaw].map(|c| f64::from(c.step()))
}

/// The nearest whole step to a smoothed value. A smoothed value always lies
/// between two steps the device sent, so it is always a step a receiver can
/// output; `fallback` is never needed.
fn nearest(value: f64, fallback: Channel) -> Channel {
    let step = value.round().clamp(0.0, 2047.0);
    Channel::from_step(step as u16).unwrap_or(fallback)
}
