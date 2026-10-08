//! Readable checks for ground and ceiling effect (#44), beyond what the
//! Physics Scenarios show. Basis: Source for the formulas (Sanchez-Cuevas,
//! Heredia & Ollero 2017, eq. 4; Hsiao & Chirarattananon 2019, eqs. 6, 7 and
//! 10), Rule for the rest.

mod common;

use common::{STEP, WORLD, whoop};
use opendrone_maths::{Attitude, DEGREE, Fingerprinter, PilotAngles, Vec3};
use opendrone_physics::{
    MapCollision, MapShape, MotorCommands, Mount, QuadBody, QuadParameters, QuadStart, QuadState,
    SetUpProblem, StartingMotors,
};

/// The Whoop 65's props: 17.5 mm across the radius.
const R: f64 = 0.0175;
/// How far its props' plane sits above its centre of mass.
const ROTOR_HEIGHT: f64 = 0.008;

/// A level sheet of ground, 40 m square, its top at `height`.
fn floor_at(height: f64) -> MapShape {
    MapShape::Box {
        centre: Vec3::new(0.0, 0.0, height - 1.0),
        size: Vec3::new(40.0, 40.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    }
}

fn map(shapes: &[MapShape]) -> MapCollision {
    MapCollision::new(shapes).unwrap()
}

/// Still, with its props `props` up, at `attitude`.
fn at(props: f64, attitude: Attitude) -> QuadState {
    QuadState {
        position: Vec3::new(0.0, 0.0, props - ROTOR_HEIGHT),
        velocity: Vec3::ZERO,
        attitude,
        rotation: Vec3::ZERO,
    }
}

/// A whoop whose motors settle as in open air, wherever it starts.
fn settled_in_open_air(parameters: QuadParameters, state: QuadState, mount: Mount) -> QuadBody {
    let start = QuadStart {
        state,
        motors: StartingMotors::Settled,
        battery: 1.0,
        mount,
    };
    QuadBody::new(parameters, start, &WORLD).unwrap()
}

/// One step on `map`, then each motor's thrust.
fn thrusts_after_a_step(quad: &mut QuadBody, map: &MapCollision) -> [f64; 4] {
    quad.step(&WORLD, map, &MotorCommands::all(0.373), STEP);
    quad.motors().map(|motor| motor.thrust)
}

/// Sanchez-Cuevas et al.'s eq. (4) for the Whoop 65 at height `z`.
fn eq_4(z: f64) -> f64 {
    let (b, k_b) = (0.066, 2.0);
    let d = b / core::f64::consts::SQRT_2;
    let cubed = |x: f64| x * x * x;
    1.0 / (1.0
        - (R / (4.0 * z)) * (R / (4.0 * z))
        - R * R * z / cubed((d * d + 4.0 * z * z).sqrt())
        - R * R / 2.0 * z / cubed((2.0 * d * d + 4.0 * z * z).sqrt())
        - 2.0 * R * R * z / cubed((b * b + 4.0 * z * z).sqrt()) * k_b)
}

#[test]
fn over_a_floor_each_rotor_gives_sanchez_cuevas_eq_4_times_its_open_air_thrust_at_the_same_speed() {
    // Two whoops at the same speeds, one over a floor, one in open air: the
    // floor multiplies each rotor's thrust by eq. (4)'s gain.
    for z in [0.6 * R, R, 2.0 * R, 4.0 * R, 8.0 * R] {
        let state = at(1.0 + z, Attitude::BODY_IS_WORLD);
        let mut over_the_floor = settled_in_open_air(whoop(), state, Mount::Free);
        let mut in_open_air = over_the_floor.clone();
        let near = thrusts_after_a_step(&mut over_the_floor, &map(&[floor_at(1.0)]));
        let far = thrusts_after_a_step(&mut in_open_air, &MapCollision::default());
        for (near, far) in near.iter().zip(far) {
            let gain = near / far;
            assert!(
                (gain - eq_4(z)).abs() < 1e-9,
                "at {} R: {gain} against {}",
                z / R,
                eq_4(z)
            );
        }
    }
}

#[test]
fn under_a_ceiling_each_rotor_gives_gamma_to_the_two_thirds_times_its_open_air_thrust() {
    for z in [0.6 * R, R, 2.0 * R, 5.0 * R] {
        let ceiling = MapShape::Box {
            centre: Vec3::new(0.0, 0.0, 4.0),
            size: Vec3::new(40.0, 40.0, 2.0),
            attitude: Attitude::BODY_IS_WORLD,
        };
        let state = at(3.0 - z, Attitude::BODY_IS_WORLD);
        let mut under = settled_in_open_air(whoop(), state, Mount::Free);
        let mut in_open_air = under.clone();
        let near = thrusts_after_a_step(&mut under, &map(&[ceiling]));
        let far = thrusts_after_a_step(&mut in_open_air, &MapCollision::default());
        let height = z / R;
        let gamma = 0.5 + 0.5 * (1.0 + 1.0 / (8.0 * height * height)).sqrt();
        let expected = opendrone_maths::functions::cbrt(gamma * gamma);
        for (near, far) in near.iter().zip(far) {
            assert!((near / far - expected).abs() < 1e-9);
        }
    }
}

#[test]
fn tilted_a_rotor_looks_along_its_own_axis_so_the_higher_rotors_gain_less() {
    // Pitched 30° nose up over a floor, the front rotors sit higher than the
    // rear ones and look farther along their tilted axes.
    let attitude = Attitude::from_pilot_angles(PilotAngles {
        roll: 0.0,
        pitch: 30.0 * DEGREE,
        heading: 0.0,
    });
    let state = at(1.0 + 3.0 * R, attitude);
    let mut tilted = settled_in_open_air(whoop(), state, Mount::Free);
    let mut in_open_air = tilted.clone();
    let near = thrusts_after_a_step(&mut tilted, &map(&[floor_at(1.0)]));
    let far = thrusts_after_a_step(&mut in_open_air, &MapCollision::default());
    let gains: Vec<f64> = near.iter().zip(far).map(|(near, far)| near / far).collect();
    assert!(gains.iter().all(|gain| *gain > 1.0), "{gains:?}");
    // Motor 2 (front right) against motor 1 (rear right).
    assert!(gains[1] < gains[0], "{gains:?}");
}

#[test]
fn on_its_side_over_a_floor_a_rotor_blows_past_it_and_gets_no_cushion() {
    let attitude = Attitude::from_pilot_angles(PilotAngles {
        roll: 90.0 * DEGREE,
        pitch: 0.0,
        heading: 0.0,
    });
    let state = at(1.0 + 0.05, attitude);
    let mut on_its_side = settled_in_open_air(whoop(), state, Mount::Free);
    let mut in_open_air = on_its_side.clone();
    let near = thrusts_after_a_step(&mut on_its_side, &map(&[floor_at(1.0)]));
    let far = thrusts_after_a_step(&mut in_open_air, &MapCollision::default());
    assert_eq!(near, far);
}

#[test]
fn on_its_back_over_a_floor_a_rotor_draws_its_air_from_the_floor_and_is_pulled_to_it() {
    // Upside down, each rotor's intake faces the floor: the ceiling effect,
    // γ^⅔, at one rotor radius. Its hub is then 8 mm below the centre.
    let attitude = Attitude::from_pilot_angles(PilotAngles {
        roll: 180.0 * DEGREE,
        pitch: 0.0,
        heading: 0.0,
    });
    let state = QuadState {
        position: Vec3::new(0.0, 0.0, 1.0 + R + ROTOR_HEIGHT),
        ..at(0.0, attitude)
    };
    let mut on_its_back = settled_in_open_air(whoop(), state, Mount::Free);
    let mut in_open_air = on_its_back.clone();
    let commands = MotorCommands::all(0.373);
    on_its_back.step(&WORLD, &map(&[floor_at(1.0)]), &commands, STEP);
    in_open_air.step(&WORLD, &MapCollision::default(), &commands, STEP);
    let gamma: f64 = 0.5 + 0.5 * (1.0_f64 + 1.0 / 8.0).sqrt();
    let expected = opendrone_maths::functions::cbrt(gamma * gamma);
    for (near, far) in on_its_back.motors().iter().zip(in_open_air.motors()) {
        assert!((near.thrust / far.thrust - expected).abs() < 1e-9);
    }
}

#[test]
fn half_over_a_ledge_only_the_rotors_over_it_gain_and_the_nose_tips_away() {
    // Nose east, the platform to the west (behind): the rear pair, motors 1
    // and 3, are over it.
    let platform = MapShape::Box {
        centre: Vec3::new(-10.0, 0.0, 0.5),
        size: Vec3::new(20.0, 40.0, 1.0),
        attitude: Attitude::BODY_IS_WORLD,
    };
    let state = at(1.0 + 2.0 * R, Attitude::BODY_IS_WORLD);
    let mut quad = settled_in_open_air(whoop(), state, Mount::Free);
    let thrusts = thrusts_after_a_step(&mut quad, &map(&[platform]));
    assert!(thrusts[0] > thrusts[1] && thrusts[2] > thrusts[3]);
    assert_eq!(thrusts[0], thrusts[2]);
    assert_eq!(thrusts[1], thrusts[3]);
    // Nose down: about the body's left axis, positive.
    assert!(quad.state().rotation.y > 0.0);
    assert_eq!(quad.state().rotation.x, 0.0);
}

#[test]
fn on_the_thrust_stand_over_a_floor_the_rotors_give_their_thrust_stand_thrust() {
    let state = at(1.0 + R, Attitude::BODY_IS_WORLD);
    let mut on_the_stand = settled_in_open_air(whoop(), state, Mount::ThrustStand);
    let mut away = on_the_stand.clone();
    let near = thrusts_after_a_step(&mut on_the_stand, &map(&[floor_at(1.0)]));
    let far = thrusts_after_a_step(&mut away, &MapCollision::default());
    assert_eq!(near, far);
}

#[test]
fn with_the_floor_beyond_ten_rotor_radii_a_hovering_whoop_steps_bit_for_bit_as_in_open_air() {
    let state = at(1.0 + 10.01 * R, Attitude::BODY_IS_WORLD);
    let mut over_the_floor = settled_in_open_air(whoop(), state, Mount::Free);
    let mut in_open_air = over_the_floor.clone();
    let floor = map(&[floor_at(1.0)]);
    for _ in 0..800 {
        over_the_floor.step(&WORLD, &floor, &MotorCommands::all(0.373), STEP);
        in_open_air.step(
            &WORLD,
            &MapCollision::default(),
            &MotorCommands::all(0.373),
            STEP,
        );
    }
    let fingerprint = |quad: &QuadBody| {
        let mut f = Fingerprinter::new();
        quad.write_fingerprint(&mut f);
        f.finish()
    };
    assert_eq!(fingerprint(&over_the_floor), fingerprint(&in_open_air));
}

#[test]
fn settled_on_a_map_the_motors_hold_the_quad_with_the_floors_gain() {
    // QuadBody::new_on_map settles the motors with the floor's gain: four
    // rotors' thrust carries the weight, at eq. (4)'s gain over the open-air
    // speed's thrust.
    let state = at(1.0 + 2.0 * R, Attitude::BODY_IS_WORLD);
    let floor = map(&[floor_at(1.0)]);
    let start = QuadStart {
        state,
        motors: StartingMotors::Settled,
        battery: 1.0,
        mount: Mount::Free,
    };
    let on_the_map = QuadBody::new_on_map(whoop(), start, &WORLD, &floor).unwrap();
    let in_open_air = QuadBody::new(whoop(), start, &WORLD).unwrap();
    let weight = whoop().mass * WORLD.gravity;
    let total: f64 = on_the_map.motors().iter().map(|motor| motor.thrust).sum();
    assert!((total - weight).abs() < 1e-12);
    let slower = on_the_map.motors()[0].speed / in_open_air.motors()[0].speed;
    assert!((slower * slower * eq_4(2.0 * R) - 1.0).abs() < 1e-12);
}

#[test]
fn a_ground_effect_body_term_too_big_for_the_rotors_layout_is_refused() {
    let mut parameters = whoop();
    parameters.ground_and_ceiling.ground_effect_body = 40.0;
    let state = at(1.0, Attitude::BODY_IS_WORLD);
    let start = QuadStart {
        state,
        motors: StartingMotors::Stopped,
        battery: 1.0,
        mount: Mount::Free,
    };
    assert_eq!(
        QuadBody::new(parameters, start, &WORLD).unwrap_err(),
        SetUpProblem::GroundOrCeilingEffectCantWork
    );
}
