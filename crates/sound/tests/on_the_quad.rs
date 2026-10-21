//! On the Quad, the camera microphone's Listening Position, made with no
//! sound device: wind, the compressor, no clipping, and Pause.
//!
//! Each check's Basis is a Rule from docs/context/sound.md, which the check
//! names.

mod common;

use common::*;

#[test]
fn wind_on_the_microphone_grows_steeply_with_airspeed() {
    // Rule: wind on the microphone grows steeply with airspeed (sound.md,
    // "Listening Positions"). Motors stopped, so only the wind is heard.
    let wind = |airspeed: f32| {
        let mut frame = all_four(0.0, 0.0, 0.0);
        frame.airspeed = airspeed;
        let mut sound = offline(whoop());
        let made = sound.play(frame, 1.0);
        db(rms(between(&made, 0.5, 1.0)))
    };
    let (still, slow, fast) = (wind(0.0), wind(5.0), wind(10.0));
    assert!(still < -150.0, "still air gives {still:.1} dB");
    assert!(
        fast - slow > 9.0,
        "from 5 to 10 m/s the wind grows only {:.1} dB, from {slow:.1} to {fast:.1} dB",
        fast - slow
    );
}

#[test]
fn the_props_downwash_blows_on_the_microphone_too() {
    // Rule: wind on the microphone grows with the props' downwash
    // (sound.md). The same motor speeds, with and without thrust: the wind's
    // rumble, below 300 Hz, grows with it.
    let rumble = |thrust: f32| {
        let mut frame = whoop_hover();
        for motor in &mut frame.motors {
            motor.thrust = thrust;
        }
        let mut sound = offline(whoop());
        let made = sound.play(frame, 1.0);
        band_power(between(&made, 0.5, 1.0), 40.0, 300.0)
    };
    let more = 10.0 * (rumble(0.3) / rumble(0.0)).log10();
    assert!(
        more > 6.0,
        "full thrust's downwash adds only {more:.1} dB of rumble"
    );
}

#[test]
fn a_punch_out_swells_for_its_first_instant_then_the_compressor_ducks_it_within_about_20_ms() {
    // Rule: a camera-style compressor, so a punch-out swells and the first
    // instant of the swell slips through; its attack is about 20 ms
    // (sound.md). From idle to full throttle in one frame.
    let mut sound = offline(whoop());
    sound.play(all_four(600.0, 0.3, 0.0), 0.5);
    let made = sound.play(whoop_full(), 0.6);
    let first_instant = db(rms(between(&made, 0.005, 0.015)));
    let after_the_attack = db(rms(between(&made, 0.06, 0.1)));
    let settled = db(rms(between(&made, 0.3, 0.6)));
    assert!(
        first_instant - settled > 6.0,
        "the first instant is only {:.1} dB above the settled level",
        first_instant - settled
    );
    assert!(
        (after_the_attack - settled).abs() < 3.0,
        "60 ms in, the level is {:.1} dB off the settled level",
        after_the_attack - settled
    );
}

#[test]
fn no_sample_ever_passes_full_scale_on_a_5_inch_punch_out_at_35_m_s() {
    // Rule: no clipping (sound.md, "Listening Positions"). The loudest
    // case: the 5″ from idle to full throttle, flying at 35 m/s.
    let mut sound = offline(freestyle_5());
    let mut idle = all_four(300.0, 0.5, 0.2);
    idle.airspeed = 35.0;
    let mut full = freestyle_5_full();
    full.airspeed = 35.0;
    let mut made = sound.play(idle, 0.3);
    made.extend(sound.play(full, 1.0));
    assert!(peak(&made) < 1.0, "the peak is {:.3}", peak(&made));
}

#[test]
fn pause_silences_the_quad_within_about_0_1_s_and_resume_brings_it_back() {
    // Rule: on Pause the Quad goes silent within about 0.1 s; Resume brings
    // it back (sound.md, "Settings and screens").
    let mut sound = offline(whoop());
    let flying = sound.play(whoop_hover(), 0.5);
    let mut paused = whoop_hover();
    paused.paused = true;
    let silenced = sound.play(paused, 0.3);
    let resumed = sound.play(whoop_hover(), 0.3);
    let before = db(rms(between(&flying, 0.3, 0.5)));
    let at_0_1_s = db(rms(between(&silenced, 0.1, 0.12)));
    let back = db(rms(between(&resumed, 0.15, 0.3)));
    assert!(
        before - at_0_1_s > 40.0,
        "0.1 s after Pause the Quad is only {:.1} dB quieter",
        before - at_0_1_s
    );
    assert!(
        (back - before).abs() < 2.0,
        "after Resume it's {back:.1} dB, against {before:.1} dB before"
    );
}
