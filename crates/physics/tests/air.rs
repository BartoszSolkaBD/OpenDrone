//! Readable checks for the air and the rotors' own spin (#42), beyond what the
//! Physics Scenarios show. Basis: Rule.

mod common;

use common::{STEP, WORLD, still_at, whoop};
use opendrone_maths::{Attitude, DEGREE, PilotAngles, PilotRates, Vec3, functions};
use opendrone_physics::{
    Drag, MapCollision, MotorCommand, MotorCommands, Mount, QuadBody, QuadParameters, QuadStart,
    SpinDirection, StartingMotors,
};

fn run(quad: &mut QuadBody, commands: MotorCommands, seconds: f64) {
    let empty_air = MapCollision::default();
    for _ in 0..(seconds / STEP).round() as u64 {
        quad.step(&WORLD, &empty_air, &commands, STEP);
    }
}

/// The Whoop 65 with its own drag numbers.
fn whoop_with_its_drag() -> QuadParameters {
    let mut parameters = whoop();
    parameters.drag = Drag {
        body_area: Vec3::new(15.0e-4, 17.0e-4, 17.0e-4),
        rotor: 0.3,
        duct_ram: 1.2,
        duct_offset: 0.013,
    };
    parameters
}

fn settled(parameters: QuadParameters, attitude: Attitude, rotation: Vec3) -> QuadBody {
    let start = QuadStart {
        state: still_at(attitude, rotation),
        motors: StartingMotors::Settled,
        battery: 1.0,
        mount: Mount::Free,
    };
    QuadBody::new(parameters, start, &WORLD).unwrap()
}

fn tilted(roll: f64, pitch: f64) -> Attitude {
    Attitude::from_pilot_angles(PilotAngles {
        roll: roll * DEGREE,
        pitch: pitch * DEGREE,
        heading: 0.0,
    })
}

#[test]
fn rolling_with_every_rotor_at_one_speed_a_whoop_neither_pitches_nor_yaws() {
    // The rotors' spins cancel, so a roll turns nothing at right angles to
    // it; the air at the rotors, which the roll changes, only slows the roll.
    // So the nose-down push in "Rolling, a punch on its anticlockwise pair
    // tips a Whoop 65's nose down" comes from the faster pair's spin alone.
    let roll = 1000.0 * DEGREE;
    let mut quad = settled(
        whoop_with_its_drag(),
        Attitude::BODY_IS_WORLD,
        Vec3::new(roll, 0.0, 0.0),
    );
    run(&mut quad, MotorCommands::all(0.373), 0.05);
    let rates = PilotRates::from_body(quad.state().rotation);
    assert!(rates.pitch.abs() < 1e-9 && rates.yaw.abs() < 1e-9);
    assert!(
        rates.roll > 0.0 && rates.roll < roll,
        "the air slows the roll"
    );
}

#[test]
fn speeding_one_pair_up_turns_the_frame_so_frame_and_rotors_keep_their_spin() {
    // With no drag on the props (no power coefficient) and no air drag,
    // nothing outside twists the Quad about its up axis: what the anticlockwise
    // rotors gain in spin, the frame loses, exactly. Its yaw inertia times its
    // yaw rate, plus each rotor's inertia times its speed (anticlockwise
    // positive), stays at nothing.
    let mut parameters = whoop();
    parameters.props.power_coefficient = 0.0;
    let mut quad = settled(parameters, Attitude::BODY_IS_WORLD, Vec3::ZERO);
    let normal = |throttle| MotorCommand {
        throttle,
        direction: SpinDirection::Normal,
    };
    let rotor_inertia = quad.parameters().props.rotor_inertia;
    let yaw_inertia = quad.parameters().inertia.numbers()[8];
    let turning = quad.parameters().rotors.turning();
    for _ in 0..4 {
        run(
            &mut quad,
            MotorCommands([normal(0.2), normal(0.6), normal(0.6), normal(0.2)]),
            0.01,
        );
        let rotors: f64 = (0..4)
            .map(|k| turning[k] * rotor_inertia * quad.motors()[k].speed)
            .sum();
        let frame = yaw_inertia * quad.state().rotation.z;
        assert!(rotors > 1e-6, "the anticlockwise pair spins faster");
        assert!(
            (frame + rotors).abs() < 1e-12 * rotors,
            "frame {frame}, rotors {rotors}"
        );
    }
}

#[test]
fn settled_motors_on_a_quad_past_its_side_or_upside_down_are_still() {
    // Their thrust would only push it down.
    for (roll, pitch) in [(95.0, 0.0), (-100.0, 20.0), (180.0, 0.0), (120.0, 30.0)] {
        let quad = settled(whoop(), tilted(roll, pitch), Vec3::ZERO);
        assert!(
            quad.motors().iter().all(|m| m.speed == 0.0),
            "roll {roll}°, pitch {pitch}°"
        );
    }
}

#[test]
fn settled_motors_carry_a_tilted_quads_weight_by_the_thrusts_upward_part() {
    // Tilted 30°, the thrust's upward part is cos 30° of it, so holding the
    // height takes the weight over cos 30°: 15% more.
    let quad = settled(whoop(), tilted(30.0, 0.0), Vec3::ZERO);
    let thrust: f64 = quad.motors().iter().map(|m| m.thrust).sum();
    let weight = 0.0312 * 9.81;
    assert!((thrust * functions::cos(30.0 * DEGREE) - weight).abs() < 1e-12);
}

#[test]
fn settled_motors_tilted_past_what_full_drive_can_hold_spin_at_full_drive() {
    // Tilted 85°, holding the height would take 11.5 times the weight, more
    // than the whoop's motors give (about 3.9 times): they settle at full
    // drive.
    let quad = settled(whoop(), tilted(85.0, 0.0), Vec3::ZERO);
    let motor = quad.motors()[0];
    assert!((motor.drive - 1.0).abs() < 1e-9, "drive {}", motor.drive);
    let thrust: f64 = quad.motors().iter().map(|m| m.thrust).sum();
    assert!(thrust > 3.0 * 0.0312 * 9.81);
}
