//! Readable checks for the shared number types: the directions every crate
//! uses, exact turning, and fingerprints that are the same on every computer.

use core::f64::consts::PI;

use opendrone_maths::functions::cos;
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
