//! The four motor voices and the frame hum, made with no sound device.
//!
//! Each check's Basis is a Rule from docs/context/sound.md or
//! docs/research/quad-sound.md, which the check names.

mod common;

use common::*;

/// The share of a line's power, in dB, against the bands either side of it:
/// how far it stands out.
fn stands_out(samples: &[f32], hz: f32, half_width: f32, away: f32) -> f32 {
    let line = band_power(samples, hz - half_width, hz + half_width);
    let below = band_power(
        samples,
        hz * (1.0 - away) - half_width,
        hz * (1.0 - away) + half_width,
    );
    let above = band_power(
        samples,
        hz * (1.0 + away) - half_width,
        hz * (1.0 + away) + half_width,
    );
    10.0 * (line / below.max(above)).log10()
}

#[test]
fn a_hovering_whoop_sings_loudest_at_its_blade_pass_pitch_blade_count_times_its_rotation_rate() {
    // Rule: the blade-pass line dominates a real whoop's sound
    // (quad-sound.md §2). At 22,700 RPM, 3 blades pass 1,135 times a second.
    let mut sound = offline(whoop());
    let made = sound.play(whoop_hover(), 1.0);
    let heard = between(&made, 0.3, 1.0);
    let blade_pass = 22_700.0 / 60.0 * 3.0;
    let margin = stands_out(heard, blade_pass, 12.0, 0.1);
    assert!(
        margin > 15.0,
        "the blade-pass line at {blade_pass:.0} Hz stands only {margin:.1} dB above 10% either side"
    );
}

#[test]
fn each_motor_has_its_own_voice_so_four_speeds_make_four_lines() {
    // Rule: each motor has its own voice (sound.md, "The Quad's sound").
    let quad = whoop();
    let mut frame = whoop_hover();
    let lines = [1000.0, 1100.0, 1200.0, 1300.0];
    for (motor, hz) in frame.motors.iter_mut().zip(lines) {
        motor.speed = speed_for_blade_pass(&quad, hz);
    }
    let mut sound = offline(quad);
    let made = sound.play(frame, 1.2);
    let heard = between(&made, 0.4, 1.2);
    for hz in lines {
        let margin = stands_out(heard, hz, 8.0, 0.045);
        assert!(
            margin > 6.0,
            "the line at {hz} Hz stands only {margin:.1} dB above the gaps beside it"
        );
    }
}

#[test]
fn the_pitch_follows_the_motors_so_a_punch_out_doubles_the_blade_pass_pitch() {
    // Rule: the sound is made from each motor's simulated speed (ADR-0023).
    // From hover at 22,700 RPM to full throttle at 45,000 RPM, the
    // blade-pass line moves from 1,135 Hz to 2,250 Hz.
    let mut sound = offline(whoop());
    sound.play(whoop_hover(), 0.3);
    let made = sound.play(whoop_full(), 1.0);
    let heard = between(&made, 0.3, 1.0);
    let full = 45_000.0 / 60.0 * 3.0;
    let margin = stands_out(heard, full, 20.0, 0.1);
    assert!(
        margin > 10.0,
        "at full throttle the line at {full:.0} Hz stands only {margin:.1} dB out"
    );
}

#[test]
fn the_speed_wobble_widens_each_motors_tone() {
    // Rule: a small sound-side wobble in each motor's speed widens its tone
    // (sound.md); a real whoop's blade-pass line is a wide cluster
    // (quad-sound.md §2). Measured: the share of the line's power within
    // ±4 Hz of its centre, out of ±5%.
    let narrowness = |wobble: f32| {
        let mut quad = whoop();
        quad.block.speed_wobble = wobble;
        let mut sound = offline(quad);
        let made = sound.play(whoop_hover(), 1.0);
        let heard = between(&made, 0.3, 1.0);
        let centre = 22_700.0 / 60.0 * 3.0;
        band_power(heard, centre - 4.0, centre + 4.0)
            / band_power(heard, centre * 0.95, centre * 1.05)
    };
    let steady = narrowness(0.0);
    let wobbling = narrowness(0.015);
    assert!(
        wobbling < steady * 0.5,
        "with a 1.5% wobble, {wobbling:.3} of the line sits at its centre; with none, {steady:.3}"
    );
}

#[test]
fn a_whoop_with_its_motors_stopped_sitting_still_is_silent() {
    // Rule: nothing is heard that the Simulation didn't do (sound.md).
    let mut sound = offline(whoop());
    let made = sound.play(all_four(0.0, 0.0, 0.0), 0.5);
    assert!(peak(&made) < 1e-6, "the peak is {}", peak(&made));
}

#[test]
fn the_5_inch_frame_hums_at_its_props_once_a_turn_rate() {
    // Rule: On the Quad has a frame hum that follows the motors (sound.md).
    // Hovering at 9,400 RPM, the props turn 157 times a second.
    let shaft_band = |hum_level: f32| {
        let mut quad = freestyle_5();
        quad.block.hum_level = hum_level;
        let mut sound = offline(quad);
        let made = sound.play(
            all_four(9_400.0 / 60.0 * std::f32::consts::TAU, 5.0, 1.6),
            1.0,
        );
        band_power(between(&made, 0.3, 1.0), 150.0, 164.0)
    };
    let quiet = shaft_band(0.0);
    let humming = shaft_band(0.2);
    let louder = 10.0 * (humming / quiet).log10();
    assert!(
        louder > 5.0,
        "the 5″'s hum adds only {louder:.1} dB at its once-a-turn rate"
    );
}
