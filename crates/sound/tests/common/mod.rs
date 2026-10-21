//! The two alpha Quads' sound, and ways to measure what the graph makes.
//!
//! Their sound blocks are the built-in Pack's (#34's round 2). The game
//! reads them from the Quad definitions; these copies keep the sound's tests
//! free of every other OpenDrone crate.

#![allow(dead_code)]

use std::f32::consts::{PI, TAU};

use opendrone_sound::{MotorSound, Offline, QuadFrame, QuadSound, SoundBlock};

pub const SAMPLE_RATE: u32 = 48_000;
pub const BLOCK: u32 = 256;

/// The Whoop 65: 3-blade 35 mm props, 12-pole 0802 motors on 1S. Its top
/// speed and current come from the motor model at full charge, as the game
/// works them out.
pub fn whoop() -> QuadSound {
    QuadSound {
        blades: 3,
        poles: 12,
        prop_diameter: 0.035,
        air_density: 1.225,
        top_speed: 5_000.0,
        top_current: 3.8,
        block: SoundBlock {
            blade_pass_level: 1.0,
            harmonics: 8,
            harmonic_rolloff: 1.5,
            blade_mismatch: 0.2,
            shaft_level: 0.02,
            loudness_exponent: 2.0,
            whoosh_level: 0.9,
            whoosh_centre: 2.0,
            whoosh_q: 0.5,
            whoosh_swish: 0.5,
            motor_whine: 0.05,
            load_brightness: 0.5,
            load_loudness: 0.4,
            speed_wobble: 0.015,
            tone_roughness: 0.6,
            hiss_level: 1.7,
            hiss_corner: 2000.0,
            body_peak: 2500.0,
            body_peak_gain: 1.5,
            body_peak_q: 1.0,
            hum_level: 0.03,
            hum_first_mode: 900.0,
            hum_first_mode_q: 6.0,
            hum_second_mode: 1600.0,
            hum_second_mode_q: 8.0,
            hum_follow: 0.5,
        },
    }
}

/// The Freestyle 5″: 3-blade 5.1″ props, 14-pole 2207 motors on 6S.
pub fn freestyle_5() -> QuadSound {
    QuadSound {
        blades: 3,
        poles: 14,
        prop_diameter: 0.13,
        air_density: 1.225,
        top_speed: 3_100.0,
        top_current: 35.0,
        block: SoundBlock {
            blade_pass_level: 1.0,
            harmonics: 12,
            harmonic_rolloff: 1.3,
            blade_mismatch: 0.25,
            shaft_level: 0.08,
            loudness_exponent: 2.0,
            whoosh_level: 1.0,
            whoosh_centre: 4.0,
            whoosh_q: 0.7,
            whoosh_swish: 0.5,
            motor_whine: 0.06,
            load_brightness: 0.6,
            load_loudness: 0.5,
            speed_wobble: 0.012,
            tone_roughness: 0.5,
            hiss_level: 1.2,
            hiss_corner: 1500.0,
            body_peak: 2500.0,
            body_peak_gain: 2.0,
            body_peak_q: 1.2,
            hum_level: 0.2,
            hum_first_mode: 180.0,
            hum_first_mode_q: 4.0,
            hum_second_mode: 420.0,
            hum_second_mode_q: 6.0,
            hum_follow: 0.3,
        },
    }
}

/// The speed, in rad/s, at which a prop's blade-pass tone sits at `hz`.
pub fn speed_for_blade_pass(quad: &QuadSound, hz: f32) -> f32 {
    hz / quad.blades as f32 * TAU
}

/// All four motors alike.
pub fn all_four(speed: f32, current: f32, thrust: f32) -> QuadFrame {
    QuadFrame {
        motors: [MotorSound {
            speed,
            current,
            thrust,
        }; 4],
        airspeed: 0.0,
        paused: false,
    }
}

/// The whoop hovering: about 22,700 RPM, 1 A and 7.8 g of thrust a motor
/// (docs/research/quad-sound.md §1).
pub fn whoop_hover() -> QuadFrame {
    all_four(22_700.0 / 60.0 * TAU, 1.0, 0.0765)
}

/// The whoop at full throttle: about 45,000 RPM and 3.4 A a motor.
pub fn whoop_full() -> QuadFrame {
    all_four(45_000.0 / 60.0 * TAU, 3.4, 0.30)
}

/// The 5″ at full throttle: about 29,600 RPM and 34.6 A a motor, about
/// 1.6 kg of thrust each (T-Motor's T5147 table).
pub fn freestyle_5_full() -> QuadFrame {
    all_four(29_600.0 / 60.0 * TAU, 34.6, 15.6)
}

/// The graph with no device, at 48 kHz in 256-frame blocks.
pub fn offline(quad: QuadSound) -> Offline {
    Offline::new(quad, SAMPLE_RATE, BLOCK).expect("the graph runs offline")
}

/// The root mean square of `samples`: their loudness, as an amplitude.
pub fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

pub fn db(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-9).log10()
}

/// The largest sample, either way.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |p, s| p.max(s.abs()))
}

/// The samples from `from` to `to` seconds in.
pub fn between(samples: &[f32], from: f32, to: f32) -> &[f32] {
    let at = |t: f32| ((t * SAMPLE_RATE as f32) as usize).min(samples.len());
    &samples[at(from)..at(to)]
}

/// The power of `samples` at `hz`, through a Hann window (Goertzel).
pub fn power_at(samples: &[f32], hz: f32) -> f32 {
    let n = samples.len() as f32;
    let w = TAU * hz / SAMPLE_RATE as f32;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for (i, &x) in samples.iter().enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * PI * i as f32 / n).cos();
        let s0 = x * hann + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2) / (n * n)
}

/// The power of `samples` summed over a band from `low` to `high` Hz, in
/// 2 Hz steps.
pub fn band_power(samples: &[f32], low: f32, high: f32) -> f32 {
    let mut hz = low;
    let mut total = 0.0;
    while hz <= high {
        total += power_at(samples, hz);
        hz += 2.0;
    }
    total
}
