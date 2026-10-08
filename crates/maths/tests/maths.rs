//! Readable checks for the shared number types: the directions every crate
//! uses, exact turning, and fingerprints that are the same on every computer.

use core::f64::consts::PI;

use opendrone_maths::functions::{cbrt, cos, max, min};
use opendrone_maths::{
    Attitude, DEGREE, Fingerprint, Fingerprinter, Mat3, PilotAngles, PilotRates, Vec3,
};

const FORWARD: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const LEFT: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const UP: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const EAST: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const NORTH: Vec3 = Vec3::new(0.0, 1.0, 0.0);

fn attitude(roll_degrees: f64, pitch_degrees: f64, heading_degrees: f64) -> Attitude {
    Attitude::from_pilot_angles(PilotAngles {
        roll: roll_degrees * DEGREE,
        pitch: pitch_degrees * DEGREE,
        heading: heading_degrees * DEGREE,
    })
}

fn assert_close(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 1e-12,
        "expected {expected:?}, got {actual:?}"
    );
}

fn assert_angle(actual: f64, expected_degrees: f64) {
    assert!(
        (actual / DEGREE - expected_degrees).abs() < 1e-9,
        "expected {expected_degrees}°, got {}°",
        actual / DEGREE
    );
}

#[test]
fn level_with_heading_0_points_the_nose_north_and_the_top_up() {
    let level = attitude(0.0, 0.0, 0.0);
    assert_close(level.body_to_world(FORWARD), NORTH);
    assert_close(level.body_to_world(LEFT), -EAST);
    assert_close(level.body_to_world(UP), UP);
}

#[test]
fn heading_90_points_the_nose_east() {
    assert_close(attitude(0.0, 0.0, 90.0).body_to_world(FORWARD), EAST);
}

#[test]
fn rolling_right_lowers_the_right_side() {
    let right = -LEFT;
    let rolled = attitude(30.0, 0.0, 0.0).body_to_world(right);
    assert!(
        rolled.z < 0.0,
        "the right side should point down, got {rolled:?}"
    );
}

#[test]
fn pitching_nose_up_raises_the_nose() {
    let nose = attitude(0.0, 30.0, 0.0).body_to_world(FORWARD);
    assert_close(nose, Vec3::new(0.0, cos(30.0 * DEGREE), 0.5));
}

#[test]
fn roll_pitch_and_heading_survive_a_round_trip_through_an_attitude() {
    for (roll, pitch, heading) in [
        (0.0, 0.0, 0.0),
        (10.0, -20.0, 30.0),
        (-170.0, 80.0, 359.0),
        (90.0, 30.0, 45.0),
        (179.0, -5.0, 180.0),
    ] {
        let angles = attitude(roll, pitch, heading).pilot_angles();
        assert_angle(angles.roll, roll);
        assert_angle(angles.pitch, pitch);
        assert_angle(angles.heading, heading);
    }
}

#[test]
fn heading_reads_from_0_up_to_a_full_turn() {
    assert_angle(attitude(0.0, 0.0, 270.0).pilot_angles().heading, 270.0);
    assert_angle(attitude(0.0, 0.0, -90.0).pilot_angles().heading, 270.0);
}

#[test]
fn a_steady_rotation_turns_by_exactly_rate_times_time() {
    // 2000 °/s about the up axis, in 8 kHz steps, for 0.045 s: 90°.
    let rate = 2000.0 * DEGREE;
    let step = 1.0 / 8000.0;
    let mut turning = attitude(0.0, 0.0, 0.0);
    for _ in 0..360 {
        turning = turning.turned_by(UP * (rate * step));
    }
    // Turning about up (anticlockwise seen from above) takes the nose from
    // north to west, which is heading 270°.
    assert_angle(turning.pilot_angles().heading, 270.0);
}

#[test]
fn spinning_about_the_roll_axis_keeps_the_nose_pointing_the_same_way() {
    let start = attitude(0.0, 30.0, 45.0);
    let mut spinning = start;
    for _ in 0..1000 {
        spinning = spinning.turned_by(FORWARD * (PI / 500.0));
    }
    assert_close(
        spinning.body_to_world(FORWARD),
        start.body_to_world(FORWARD),
    );
}

#[test]
fn turning_by_nothing_changes_nothing() {
    let start = attitude(12.0, 34.0, 56.0);
    assert_eq!(start.turned_by(Vec3::ZERO), start);
}

#[test]
fn rolling_right_pitching_up_and_yawing_right_are_positive_rates() {
    // Rolling right turns about the nose; pitching up turns about the
    // right-hand side; yawing right turns about the bottom.
    let rates = PilotRates::from_body(Vec3::new(1.0, -2.0, -3.0));
    assert_eq!(
        rates,
        PilotRates {
            roll: 1.0,
            pitch: 2.0,
            yaw: 3.0
        }
    );
    assert_eq!(rates.to_body(), Vec3::new(1.0, -2.0, -3.0));
}

#[test]
fn an_attitudes_matrix_turns_directions_the_same_way() {
    let turned = attitude(20.0, -10.0, 135.0);
    let v = Vec3::new(0.3, -0.4, 0.5);
    assert_close(turned.to_matrix() * v, turned.body_to_world(v));
    assert_close(turned.world_to_body(turned.body_to_world(v)), v);
}

#[test]
fn a_matrix_times_its_inverse_changes_nothing() {
    let m = Mat3::from_rows(
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(0.5, 3.0, -1.0),
        Vec3::new(0.0, 0.25, 4.0),
    );
    let product = m * m.inverse().expect("this matrix has an inverse");
    for (row, identity) in product.rows.iter().zip(Mat3::IDENTITY.rows) {
        assert_close(*row, identity);
    }
}

#[test]
fn a_matrix_with_no_inverse_says_so() {
    assert_eq!(Mat3::diagonal(Vec3::new(1.0, 0.0, 1.0)).inverse(), None);
}

#[test]
fn the_same_numbers_give_the_same_fingerprint_and_one_bit_changes_it() {
    let fingerprint = |numbers: &[f64]| {
        let mut f = Fingerprinter::new();
        f.write_f64s(numbers);
        f.finish()
    };
    assert_eq!(fingerprint(&[9.81, 1.0]), fingerprint(&[9.81, 1.0]));
    assert_ne!(
        fingerprint(&[9.81]),
        fingerprint(&[f64::from_bits(9.81_f64.to_bits() + 1)])
    );
    assert_ne!(fingerprint(&[1.0, 2.0]), fingerprint(&[2.0, 1.0]));
}

#[test]
fn minus_zero_and_every_not_a_number_are_fingerprinted_the_same_on_every_computer() {
    let fingerprint = |n: f64| {
        let mut f = Fingerprinter::new();
        f.write_f64(n);
        f.finish()
    };
    assert_eq!(fingerprint(-0.0), fingerprint(0.0));
    assert_eq!(fingerprint(f64::NAN), fingerprint(-f64::NAN));
    assert_eq!(
        fingerprint(f64::NAN),
        fingerprint(f64::from_bits(0x7ff0_0000_0000_0001))
    );
}

#[test]
fn a_fingerprint_is_the_same_on_every_computer() {
    // 64-bit FNV-1a over the bytes of 9.81, least significant first. CI runs
    // this on ARM and x86, so a platform difference fails here.
    let mut f = Fingerprinter::new();
    f.write_f64(9.81);
    assert_eq!(f.finish(), Fingerprint(0x565b_7331_2a88_8880));
    assert_eq!(f.finish().to_string(), "565b73312a888880");
    assert_eq!(
        Fingerprinter::new().finish().to_string(),
        "cbf29ce484222325"
    );
}

#[test]
fn the_cube_root_undoes_cubing_whichever_side_of_zero() {
    assert_eq!(cbrt(8.0), 2.0);
    assert_eq!(cbrt(-27.0), -3.0);
    let x = 1.112_372_435_695_794_5_f64;
    assert!((cbrt(x * x) * cbrt(x) - x).abs() < 1e-15);
}

#[test]
fn min_and_max_pick_the_same_zero_on_every_computer() {
    // Given +0 and -0, std's f64::min and f64::max may return either, and ARM
    // and x86 differ. Ours return the first.
    assert_eq!(min(0.0, -0.0).to_bits(), 0.0_f64.to_bits());
    assert_eq!(min(-0.0, 0.0).to_bits(), (-0.0_f64).to_bits());
    assert_eq!(max(0.0, -0.0).to_bits(), 0.0_f64.to_bits());
    assert_eq!(max(-0.0, 0.0).to_bits(), (-0.0_f64).to_bits());
}

#[test]
fn min_and_max_pick_the_smaller_and_larger_and_pass_on_not_a_number() {
    assert_eq!(min(1.0, 2.0), 1.0);
    assert_eq!(min(2.0, -3.0), -3.0);
    assert_eq!(max(1.0, 2.0), 2.0);
    assert_eq!(max(2.0, -3.0), 2.0);
    assert!(min(f64::NAN, 1.0).is_nan() && min(1.0, f64::NAN).is_nan());
    assert!(max(f64::NAN, 1.0).is_nan() && max(1.0, f64::NAN).is_nan());
}

fn assert_same_attitude(actual: Attitude, expected: Attitude, within: f64) {
    for axis in [FORWARD, LEFT, UP] {
        let (a, e) = (actual.body_to_world(axis), expected.body_to_world(axis));
        assert!(
            (a - e).length() < within,
            "{axis:?} points {a:?}, not {e:?}"
        );
    }
}

/// How far apart two attitudes are: the largest distance between where they
/// point the same body direction.
fn apart(one: Attitude, other: Attitude) -> f64 {
    [FORWARD, LEFT, UP]
        .into_iter()
        .map(|axis| (one.body_to_world(axis) - other.body_to_world(axis)).length())
        .fold(0.0, max)
}

/// An attitude with the nose `off` radians from straight up (or down, for
/// `sign` -1).
fn off_vertical(roll_degrees: f64, sign: f64, off: f64, heading_degrees: f64) -> Attitude {
    attitude(roll_degrees, sign * (90.0 - off / DEGREE), heading_degrees)
}

fn wrapped_degrees(angle: f64) -> f64 {
    let degrees = angle / DEGREE;
    degrees - 360.0 * (degrees / 360.0).floor()
}

#[test]
fn with_the_nose_straight_up_or_down_roll_reads_0_and_heading_carries_the_turn() {
    for (pitch, sign) in [(90.0, -1.0), (-90.0, 1.0)] {
        for (roll, heading) in [(0.0, 30.0), (30.0, 45.0), (-60.0, 200.0), (170.0, 350.0)] {
            let start = attitude(roll, pitch, heading);
            let angles = start.pilot_angles();
            assert_eq!(
                angles.roll, 0.0,
                "roll {roll}, pitch {pitch}, heading {heading}"
            );
            assert_angle(angles.pitch, pitch);
            // Nose up: heading minus roll. Nose down: heading plus roll.
            let expected = heading + sign * roll;
            let expected = expected - 360.0 * (expected / 360.0).floor();
            let read = wrapped_degrees(angles.heading);
            let apart = (read - expected + 180.0).rem_euclid(360.0) - 180.0;
            assert!(
                apart.abs() < 1e-9,
                "roll {roll}, pitch {pitch}, heading {heading}: heading reads {read}, not {expected}"
            );
            assert_same_attitude(Attitude::from_pilot_angles(angles), start, 1e-12);
        }
    }
}

#[test]
fn just_short_of_straight_up_the_angles_still_give_back_the_same_attitude() {
    // Just outside the band around straight up or down (0.00000006°, or 1e-9
    // radians), roll and heading are read the ordinary way, to 1e-12.
    for pitch in [89.9999999, -89.9999999, 89.99999, -89.999] {
        for (roll, heading) in [(0.0, 45.0), (30.0, 45.0), (-120.0, 300.0), (180.0, 10.0)] {
            let start = attitude(roll, pitch, heading);
            assert!(!start.is_straight_up_or_down(), "pitch {pitch}");
            assert_same_attitude(
                Attitude::from_pilot_angles(start.pilot_angles()),
                start,
                1e-12,
            );
        }
    }
    for sign in [1.0, -1.0] {
        for (roll, heading) in [(30.0, 45.0), (180.0, 10.0)] {
            let start = off_vertical(roll, sign, 1.001e-9, heading);
            assert!(!start.is_straight_up_or_down(), "just outside the band");
            assert_same_attitude(
                Attitude::from_pilot_angles(start.pilot_angles()),
                start,
                1e-12,
            );
        }
    }
}

#[test]
fn inside_the_band_around_straight_up_or_down_the_angles_give_back_the_attitude_to_2e_9() {
    // Within 1e-9 radians (about 0.00000006°) of straight up or down, roll
    // reads 0 though the nose is a hair off vertical. So the attitude comes
    // back off by up to twice the nose's distance from vertical: just under
    // 2e-9 (0.0000001°) at the band's edge, with a half-turn roll.
    let mut furthest: f64 = 0.0;
    for off in [1e-10, 5e-10, 0.999e-9] {
        for sign in [1.0, -1.0] {
            for roll in [-150.0, -90.0, 0.0, 30.0, 90.0, 179.0, 180.0] {
                for heading in [0.0, 45.0, 300.0] {
                    let start = off_vertical(roll, sign, off, heading);
                    assert!(start.is_straight_up_or_down(), "{off} radians off vertical");
                    let back = Attitude::from_pilot_angles(start.pilot_angles());
                    let found = apart(back, start);
                    assert!(
                        found <= 2.0 * off + 1e-15,
                        "{off} radians off, roll {roll}: {found}"
                    );
                    furthest = max(furthest, found);
                }
            }
        }
    }
    assert!(
        1.99e-9 < furthest && furthest < 2e-9,
        "the furthest was {furthest}"
    );
}

#[test]
fn a_quad_nose_straight_down_reads_the_heading_it_was_given() {
    let angles = attitude(0.0, -90.0, 30.0).pilot_angles();
    assert_eq!(angles.roll, 0.0);
    assert_angle(angles.heading, 30.0);
}

#[test]
fn any_attitude_read_as_angles_gives_back_the_same_attitude() {
    // A spread of attitudes, from a fixed sequence, with every pitch from
    // straight down to straight up.
    let mut seed: u64 = 1;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 11) as f64 / (1_u64 << 53) as f64
    };
    for _ in 0..10_000 {
        let roll = next() * 360.0 - 180.0;
        let pitch = next() * 180.0 - 90.0;
        let heading = next() * 360.0;
        let start = attitude(roll, pitch, heading);
        assert_same_attitude(
            Attitude::from_pilot_angles(start.pilot_angles()),
            start,
            1e-12,
        );
    }
}
