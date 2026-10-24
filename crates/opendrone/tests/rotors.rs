//! Readable checks for the Quad's rotors turning in the picture (#319): the
//! angle they turn through, the way they turn, and the fade from blades to a
//! blurred disc that keeps the picture free of the stroboscope effect (a
//! wheel that seems to turn slowly or backwards). None of it enters the
//! Simulation, so none of it is a Scenario. Speeds in rad/s, times in
//! seconds, angles in radians. How the rotors look is judged by eye, from the
//! `--render-views` pictures in the PR, never here.

mod common;

use std::f64::consts::TAU;

use opendrone::map_views::ViewRequest;
use opendrone::rotors::{
    BLADES_ALONE_BELOW, DISC_ALONE_FROM, blade_share, disc_density, gaps_per_frame, turned,
};
use opendrone::set_up::FlightPlan;
use opendrone::settings::Settings;
use opendrone::sound::top_speed_and_current;
use opendrone_physics::{PropDirection, RotorLayout};

/// The frame lengths a picture can have: 240, 144, 120, 60, 45, 30 and 15 Hz.
const FRAME_TIMES: [f64; 7] = [
    1.0 / 240.0,
    1.0 / 144.0,
    1.0 / 120.0,
    1.0 / 60.0,
    1.0 / 45.0,
    1.0 / 30.0,
    1.0 / 15.0,
];

#[test]
fn a_rotor_at_rest_stands_still_and_shows_its_blades() {
    // Basis: Rule (#319): at rest the rotors stand still.
    for blades in 2..=6 {
        for frame_time in FRAME_TIMES {
            assert_eq!(blade_share(0.0, frame_time, blades), 1.0);
        }
    }
    assert_eq!(turned(1.25, 1.0, 0.0, 1.0 / 60.0), 1.25);
}

#[test]
fn a_rotor_turns_by_its_speed_times_the_time_the_way_its_prop_turns() {
    // Basis: Rule (#319): the angle advances with the motor's speed each
    // drawn frame. 10 rad/s for 0.01 s is 0.1 rad, anticlockwise (+) or
    // clockwise (-) seen from above, as the prop turns.
    assert!((turned(0.5, 1.0, 10.0, 0.01) - 0.6).abs() < 1e-12);
    assert!((turned(0.5, -1.0, 10.0, 0.01) - 0.4).abs() < 1e-12);
    // A motor spinning backwards turns its prop the other way.
    assert!((turned(0.5, 1.0, -10.0, 0.01) - 0.4).abs() < 1e-12);
}

#[test]
fn each_motors_rotor_turns_the_way_the_quad_definitions_props_turn() {
    // Basis: Source (Betaflight 2026.6 mixer, `yaw_motors_reversed`): with
    // the props in (the default) the front right motor (2) and rear left
    // motor (3) turn anticlockwise seen from above and the rear right (1)
    // and front left (4) clockwise; props out turn every one the other way.
    // The drawing turns each rotor by `RotorLayout::turning`.
    for (direction, anticlockwise) in [
        (PropDirection::PropsIn, [false, true, true, false]),
        (PropDirection::PropsOut, [true, false, false, true]),
    ] {
        let layout = RotorLayout {
            diagonal: 0.2,
            rotor_height: 0.0,
            direction,
        };
        for (motor, turning) in layout.turning().into_iter().enumerate() {
            let angle = turned(1.0, turning, 100.0, 1.0 / 1000.0);
            assert_eq!(
                angle > 1.0,
                anticlockwise[motor],
                "{direction:?}, motor {}: from 1.0 rad to {angle}",
                motor + 1
            );
        }
    }
}

#[test]
fn the_angle_stays_within_one_turn_however_long_the_rotor_spins() {
    // Basis: Rule (#319): the angle wraps, so a long flight never loses its
    // fine digits. An hour of 60 Hz frames at 2,500 rad/s, both ways round.
    for turning in [1.0, -1.0] {
        let mut angle = 0.0;
        for _ in 0..216_000 {
            angle = turned(angle, turning, 2500.0, 1.0 / 60.0);
            assert!((0.0..TAU).contains(&angle), "{angle}");
        }
    }
}

#[test]
fn both_quads_show_a_blurred_disc_alone_at_a_hover_and_above() {
    // Basis: Rule (#319): at a hover the rotors show as a blurred disc. A
    // hover is well above a fifth of a motor's top speed on a Quad that can
    // lift more than itself, which both can, so a fifth is a safe floor for
    // it. In a 60 Hz picture.
    let packs = common::packs();
    for quad in ["freestyle-5", "whoop-65"] {
        let plan =
            FlightPlan::from_packs(&packs, Settings::defaults(quad)).expect("the Quad flies");
        let (top, _) = top_speed_and_current(&plan.quad, plan.map.world.air_density);
        let blades = plan.quad.props.blades;
        for share in [0.2, 0.35, 0.5, 0.75, 1.0] {
            assert_eq!(
                blade_share(share * top, 1.0 / 60.0, blades),
                0.0,
                "{quad} at {share} of its top speed, {top:.0} rad/s, {blades} blades"
            );
        }
    }
}

#[test]
fn the_blades_fade_out_as_the_motor_spins_up() {
    // Basis: Rule (#319): blades at low speed, a cross-fade as the speed
    // rises. A 3-blade prop in a 60 Hz picture is all blades below 5% of a
    // gap in a frame (about 9.4 rad/s), all disc from 25% (about 47 rad/s),
    // and in between partly both.
    let all_blades = BLADES_ALONE_BELOW * TAU * 60.0 / 3.0;
    let all_disc = DISC_ALONE_FROM * TAU * 60.0 / 3.0;
    assert_eq!(blade_share(all_blades * 0.99, 1.0 / 60.0, 3), 1.0);
    assert_eq!(blade_share(all_disc * 1.01, 1.0 / 60.0, 3), 0.0);
    let middle = blade_share((all_blades + all_disc) / 2.0, 1.0 / 60.0, 3);
    assert!((middle - 0.5).abs() < 1e-9, "a smooth step is even halfway");
}

#[test]
fn a_slower_picture_fades_its_blades_out_at_a_lower_speed() {
    // Basis: Rule (#319): the fade is judged against the frame's own length:
    // the same speed moves a blade further between two slower frames.
    let speed = 30.0;
    let fast = blade_share(speed, 1.0 / 240.0, 3);
    let slow = blade_share(speed, 1.0 / 30.0, 3);
    assert!(fast > slow, "{fast} against {slow}");
}

#[test]
fn no_throttle_shows_a_stroboscope_effect() {
    // Basis: Rule (#319): stepping the speed through its whole range, for
    // 2 to 6 blades and every frame length from 240 Hz to 15 Hz, the blades
    // are only ever drawn while a blade moves under a quarter of the gap
    // between two blades in a frame. A wheel can be taken to turn either way
    // from half a gap a frame, so while blades show, the nearest match is
    // the true turning; and where the blades are gone only the disc shows,
    // which looks the same at every angle. The fade never jumps or comes
    // back as the speed rises.
    for blades in 2..=6 {
        for frame_time in FRAME_TIMES {
            let mut before = blade_share(0.0, frame_time, blades);
            assert_eq!(before, 1.0);
            // Steps of 0.2% of a gap a frame, up to two whole gaps a frame
            // (what a 5-inch prop does in a 15 Hz picture at 100 rad/s).
            let step = 0.002 * TAU / (frame_time * f64::from(blades));
            let mut speed = 0.0;
            while gaps_per_frame(speed, frame_time, blades) <= 2.0 {
                speed += step;
                let share = blade_share(speed, frame_time, blades);
                let gaps = gaps_per_frame(speed, frame_time, blades);
                assert!(
                    share <= before,
                    "{blades} blades at {frame_time:.4} s: the blades came back at {speed} rad/s"
                );
                assert!(
                    before - share < 0.05,
                    "{blades} blades at {frame_time:.4} s: the fade jumped at {speed} rad/s"
                );
                if share > 0.0 {
                    assert!(
                        gaps < DISC_ALONE_FROM,
                        "{blades} blades at {frame_time:.4} s: blades at {speed} rad/s move {gaps} gaps a frame"
                    );
                    // The turning one frame on is less than half a gap, and
                    // the true way.
                    let moved = turned(0.0, 1.0, speed, frame_time);
                    let gap = TAU / f64::from(blades);
                    assert!(moved < gap / 2.0, "{moved} against a gap of {gap}");
                } else {
                    assert!(gaps >= BLADES_ALONE_BELOW);
                }
                before = share;
            }
            assert_eq!(before, 0.0, "{blades} blades at {frame_time:.4} s");
            // And the disc alone all the way to far above any motor's top
            // speed.
            for speed in [500.0, 1000.0, 2000.0, 4000.0] {
                let gaps = gaps_per_frame(speed, frame_time, blades);
                if gaps >= DISC_ALONE_FROM {
                    assert_eq!(blade_share(speed, frame_time, blades), 0.0);
                }
            }
        }
    }
}

#[test]
fn the_blurred_disc_is_faint_densest_by_the_tips_and_ends_at_them() {
    // Basis: Observed (a real propeller in a long exposure): a faint
    // translucent disc, with the tips, where the blades move fastest,
    // showing more than the hub. Nothing beyond the prop's radius.
    for step in 0..=100 {
        let density = disc_density(f64::from(step) / 100.0);
        assert!((0.0..=1.0).contains(&density), "{step}%: {density}");
    }
    assert!(disc_density(0.85) > 2.0 * disc_density(0.05));
    assert_eq!(disc_density(1.0), 0.0);
    assert_eq!(disc_density(1.5), 0.0);
}

#[test]
fn a_view_can_ask_for_the_rotors_to_turn_at_a_share_of_full_speed() {
    // Basis: Rule (#319): the developer's `rotors=` key draws the rotors at
    // a share of a motor's top speed, 0 to 1, whatever the Simulation's
    // motors do.
    let view = ViewRequest::parse("look=digital rotors=0.5").expect("reads");
    assert_eq!(view.rotors, Some(0.5));
    assert_eq!(ViewRequest::parse("look=digital").unwrap().rotors, None);
    for text in ["rotors=2", "rotors=-0.1", "rotors=fast"] {
        assert!(ViewRequest::parse(text).is_err(), "{text}");
    }
    let plan = common::plan("freestyle-5");
    let (top, _) = top_speed_and_current(&plan.quad, plan.map.world.air_density);
    let resolved = view.resolve(&plan).expect("resolves");
    assert!((resolved.rotor_speed.expect("a speed") - 0.5 * top).abs() < 1e-9);
    let none = ViewRequest::launch().resolve(&plan).expect("resolves");
    assert_eq!(none.rotor_speed, None);
}
