//! The Legacy mixer with Airmode, motor idle and the motor output limit,
//! reimplemented from Betaflight 2026.6.2's `mixTable`,
//! `applyMixerAdjustment`, `applyMixToMotors` and `applyThrottleLimit`
//! (`src/main/flight/mixer.c`), with the DShot endpoints of
//! `dshotInitEndpoints` (`src/main/drivers/dshot.c`); the Betaflight research
//! §4.
//!
//! The mixer spreads the PID sums over the four motors of an X frame, adds
//! the throttle, and keeps every motor between idle and the output limit:
//!
//! 1. Each PID sum is held within its PID-sum limit and divided by 1000, so
//!    1000 is the whole motor range. Props-in (`yaw_motors_reversed` off)
//!    turns yaw round.
//! 2. Each motor's mix is roll, pitch and yaw times its place on the frame.
//! 3. Airmode, always on: if the mix spans more than the whole range, every
//!    motor's mix shrinks to fit; then the throttle moves up or down just
//!    enough that no motor clips. So a Quad keeps full control at zero
//!    throttle.
//! 4. Each motor gets idle plus its share of the range above idle: DShot
//!    48 + `motor_idle` × 1999 up to 2047 less the output limit's cut, sent
//!    as the nearest whole DShot value. A disarmed Flight Controller sends 0,
//!    DShot's "stop".
//!
//! Not here yet: throttle boost, anti-gravity's and TPA's reading of the
//! throttle (#50), thrust linearisation, battery-sag compensation and
//! dynamic idle (later), and Crash Flip (#54).

use crate::constrain;
use crate::rates::{Rates, ThrottleLimitType};
use crate::tune::Tune;

/// Betaflight's QuadX table, in its motor order (rear right, front right,
/// rear left, front left): each motor's share of roll, pitch and yaw
/// (`mixerQuadX`).
const QUAD_X: [[f64; 3]; 4] = [
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [1.0, -1.0, -1.0],
];

/// DShot's lowest and highest throttle values (`DSHOT_MIN_THROTTLE`,
/// `DSHOT_MAX_THROTTLE`); 0 is "stop".
pub const DSHOT_LOWEST: u16 = 48;
pub const DSHOT_HIGHEST: u16 = 2047;
const DSHOT_RANGE: f64 = (DSHOT_HIGHEST - DSHOT_LOWEST) as f64;
/// Betaflight scales PID sums down by this before mixing
/// (`PID_MIXER_SCALING`).
const PID_MIXER_SCALING: f64 = 1000.0;

/// What the mixer gives for one loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mixed {
    /// The throttle after the throttle limit, from 0 to 1, before Airmode
    /// moves it: what Betaflight's Blackbox logs as the mixer's throttle.
    pub throttle: f64,
    /// Each motor's DShot value.
    pub dshot: [u16; 4],
}

/// Mixes one loop's PID sums and throttle command into four DShot values.
pub(crate) fn mix(
    tune: &Tune,
    rates: &Rates,
    pid_sums: [f64; 3],
    throttle_command: i64,
    armed: bool,
) -> Mixed {
    // The motors' endpoints, in DShot values.
    let idle = f64::from(tune.motor_idle) * 0.0001;
    let limit = f64::from(tune.motor_output_limit) / 100.0;
    let output_low = f64::from(DSHOT_LOWEST) + idle * DSHOT_RANGE;
    let output_high = f64::from(DSHOT_HIGHEST) - DSHOT_RANGE * (1.0 - limit);

    // The throttle, from 0 to 1, then the pilot's throttle limit.
    let mut throttle = ((throttle_command - 1000) as f64 / 1000.0).clamp(0.0, 1.0);
    if rates.throttle_limit_type != ThrottleLimitType::Off && rates.throttle_limit_percent < 100 {
        let factor = f64::from(rates.throttle_limit_percent) / 100.0;
        throttle = match rates.throttle_limit_type {
            ThrottleLimitType::Scale => throttle * factor,
            ThrottleLimitType::Clip => opendrone_maths::functions::min(throttle, factor),
            ThrottleLimitType::Off => throttle,
        };
    }
    let mixer_throttle = throttle;

    // 1. The PID sums, held within their limits.
    let roll_pitch = f64::from(tune.pidsum_limit);
    let yaw_limit = f64::from(tune.pidsum_limit_yaw);
    let roll = pid_sums[0].clamp(-roll_pitch, roll_pitch) / PID_MIXER_SCALING;
    let pitch = pid_sums[1].clamp(-roll_pitch, roll_pitch) / PID_MIXER_SCALING;
    let mut yaw = pid_sums[2].clamp(-yaw_limit, yaw_limit) / PID_MIXER_SCALING;
    if !tune.yaw_motors_reversed {
        yaw = -yaw;
    }

    // 2. Each motor's mix, and the mix's lowest and highest (never above 0
    // or below 0 at the start, as Betaflight starts them).
    let mut motor_mix = [0.0; 4];
    let mut lowest = 0.0;
    let mut highest = 0.0;
    for (mix, place) in motor_mix.iter_mut().zip(QUAD_X) {
        *mix = roll * place[0] + pitch * place[1] + yaw * place[2];
        if *mix > highest {
            highest = *mix;
        } else if *mix < lowest {
            lowest = *mix;
        }
    }

    // 3. Airmode: shrink a mix wider than the range, then move the throttle
    // so that no motor clips.
    let range = highest - lowest;
    let normalise = if range > 1.0 { 1.0 / range } else { 1.0 };
    for mix in &mut motor_mix {
        *mix *= normalise;
    }
    throttle = constrain(throttle, -lowest * normalise, 1.0 - highest * normalise);

    // 4. Idle plus each motor's share of the range, as whole DShot values.
    let mut dshot = [0; 4];
    if armed {
        for (value, mix) in dshot.iter_mut().zip(motor_mix) {
            let output = constrain(
                output_low + (output_high - output_low) * (mix + throttle),
                output_low,
                output_high,
            );
            // `lrintf`: to the nearest whole number, halves to the even one.
            *value = output.round_ties_even() as u16;
        }
    }
    Mixed {
        throttle: mixer_throttle,
        dshot,
    }
}
