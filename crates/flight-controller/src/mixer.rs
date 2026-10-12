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
//! During yaw spin recovery the yaw PID sum may use the whole motor range
//! (`PIDSUM_LIMIT_MAX`, 1000). Betaflight also holds the throttle at 50% then
//! when Airmode is off; here Airmode is always on, so it never does.
//!
//! In Crash Flip the mixer is bypassed: [`crash_flip`] drives the motors
//! straight from the sticks.
//!
//! Not here yet: throttle boost, anti-gravity's and TPA's reading of the
//! throttle (#50), thrust linearisation, battery-sag compensation and
//! dynamic idle (later).

use opendrone_maths::functions;

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
/// The highest PID-sum limit, which yaw gets during yaw spin recovery
/// (`PIDSUM_LIMIT_MAX`).
const PIDSUM_LIMIT_MAX: f64 = 1000.0;
/// In Crash Flip, a stick moved less than this far drives nothing
/// (`CRASHFLIP_STICK_DEADBAND`).
const CRASH_FLIP_STICK_DEADBAND: f64 = 0.15;
/// In Crash Flip, a motor driven at less than this share of its range is
/// stopped (`CRASHFLIP_MOTOR_DEADBAND`).
const CRASH_FLIP_MOTOR_DEADBAND: f64 = 0.02;

/// What the mixer gives for one loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mixed {
    /// The throttle after the throttle limit, from 0 to 1, before Airmode
    /// moves it: what Betaflight's Blackbox logs as the mixer's throttle.
    pub throttle: f64,
    /// Each motor's DShot value.
    pub dshot: [u16; 4],
}

/// The motors' endpoints, in DShot values: idle, and the top of the range
/// after the output limit (`motorOutputLow`, `motorOutputHigh`).
fn endpoints(tune: &Tune) -> (f64, f64) {
    let idle = f64::from(tune.motor_idle) * 0.0001;
    let limit = f64::from(tune.motor_output_limit) / 100.0;
    let output_low = f64::from(DSHOT_LOWEST) + idle * DSHOT_RANGE;
    let output_high = f64::from(DSHOT_HIGHEST) - DSHOT_RANGE * (1.0 - limit);
    (output_low, output_high)
}

/// Mixes one loop's PID sums and throttle command into four DShot values.
/// `yaw_spin` is true during yaw spin recovery.
pub(crate) fn mix(
    tune: &Tune,
    rates: &Rates,
    pid_sums: [f64; 3],
    throttle_command: i64,
    armed: bool,
    yaw_spin: bool,
) -> Mixed {
    let (output_low, output_high) = endpoints(tune);

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
    let yaw_limit = if yaw_spin {
        PIDSUM_LIMIT_MAX
    } else {
        f64::from(tune.pidsum_limit_yaw)
    };
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

/// What Crash Flip reads besides the Tune: the sticks and the sensors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CrashFlipInputs {
    /// Each stick's deflection, from −1 to +1, past the deadband, in
    /// Betaflight's directions (roll right, pitch forward positive; yaw
    /// right negative).
    pub deflection: [f64; 3],
    /// The gyro, in Betaflight's axes and °/s.
    pub gyro: [f64; 3],
    /// The cosine of the tilt: 1 upright, 0 on edge, −1 upside down
    /// (`getCosTiltAngle`).
    pub cos_tilt: f64,
}

/// Crash Flip's motor commands, reimplemented from Betaflight 2026.6.2's
/// `applyCrashFlipModeToMotors` (`src/main/flight/mixer.c`): open loop, the
/// PID sums and the throttle ignored.
///
/// - Pitch forward drives the front pair, pitch back the rear pair, roll
///   right the right pair and roll left the left pair. A stick within 30° of
///   a diagonal drives the one motor at that corner. Yaw, when it's moved
///   further than roll and pitch, drives a diagonal pair.
/// - Power is the stick's deflection (roll and pitch together, as one
///   length), once it is past 15%: linear, with no expo.
/// - A motor whose mix would push the wrong way gets `crashflip_motor_percent`
///   of it backwards, or stays stopped at 0. A motor driven at less than 2% of
///   its range is stopped.
/// - With `crashflip_rate` above 0, power fades as the Quad turns. With the
///   roll or pitch rate: full up to half of `crashflip_rate` (in tens of
///   °/s), nothing at it. With the tilt: full until the cosine of the tilt
///   has changed by 0.5 since Crash Flip first drove the motors
///   (`tilt_at_start`), nothing once it has changed by 1, which from flat is
///   a quarter turn.
pub(crate) fn crash_flip(
    tune: &Tune,
    inputs: CrashFlipInputs,
    tilt_at_start: &mut Option<f64>,
) -> [u16; 4] {
    let (output_low, output_high) = endpoints(tune);
    let [roll, pitch, yaw] = inputs.deflection;
    let (roll_abs, pitch_abs, yaw_abs) = (roll.abs(), pitch.abs(), yaw.abs());
    let mut sign_pitch = if pitch < 0.0 { 1.0 } else { -1.0 };
    let mut sign_roll = if roll < 0.0 { 1.0 } else { -1.0 };
    let mut sign_yaw =
        (if yaw < 0.0 { 1.0 } else { -1.0 }) * (if tune.yaw_motors_reversed { 1.0 } else { -1.0 });

    let mut length = (pitch_abs * pitch_abs + roll_abs * roll_abs).sqrt();
    if yaw_abs > functions::max(pitch_abs, roll_abs) {
        // Yaw leads: roll and pitch drive nothing.
        length = yaw_abs;
        sign_roll = 0.0;
        sign_pitch = 0.0;
    } else {
        sign_yaw = 0.0;
    }
    // Off a diagonal by more than 30°, only roll or only pitch drives.
    let cos_phi = if length > 0.0 {
        (pitch_abs + roll_abs) / (core::f64::consts::SQRT_2 * length)
    } else {
        0.0
    };
    let cos_30 = 3.0_f64.sqrt() / 2.0;
    if cos_phi < cos_30 {
        if roll_abs > pitch_abs {
            sign_pitch = 0.0;
        } else {
            sign_roll = 0.0;
        }
    }

    let mut power = if length > CRASH_FLIP_STICK_DEADBAND {
        length
    } else {
        0.0
    };
    let rate_limit = f64::from(tune.crashflip_rate) * 10.0;
    if rate_limit > 0.0 {
        let half = 0.5;
        let tilt = inputs.cos_tilt;
        let start = *tilt_at_start.get_or_insert(tilt);
        let change_needed = functions::max(1.0 - (tilt - start).abs(), 0.0);
        let attitude_fade = if change_needed > half {
            1.0
        } else {
            change_needed / half
        };
        let gyro_rate = functions::max(inputs.gyro[0].abs(), inputs.gyro[1].abs());
        let rate_change = functions::min(gyro_rate / rate_limit, 1.0);
        let rate_fade = if rate_change < half {
            1.0
        } else {
            (1.0 - rate_change) / half
        };
        power *= attitude_fade * rate_fade;
    }

    let mut dshot = [0; 4];
    for (value, place) in dshot.iter_mut().zip(QUAD_X) {
        let mut mix = sign_pitch * place[1] + sign_roll * place[0] + sign_yaw * place[2];
        if mix < 0.0 {
            mix = if tune.crashflip_motor_percent > 0 {
                -mix * f64::from(tune.crashflip_motor_percent) / 100.0
            } else {
                0.0
            };
        }
        let share = functions::min(1.0, power * mix);
        *value = if share < CRASH_FLIP_MOTOR_DEADBAND {
            0
        } else {
            // `lrintf`: to the nearest whole number, halves to the even one.
            (output_low + share * (output_high - output_low)).round_ties_even() as u16
        };
    }
    dshot
}
