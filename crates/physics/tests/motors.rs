//! Readable checks for the motors' forces on the Quad, its ESCs and its
//! battery, beyond what the Thrust Stand and Physics Scenarios show. Basis:
//! Rule, unless a check says otherwise.

mod common;

use common::{STEP, WORLD, start, whoop};
use opendrone_maths::PilotRates;
use opendrone_physics::{
    EscState, MotorCommand, MotorCommands, Mount, PropDirection, QuadBody, SpinDirection,
    StartUpStep, StartingMotors,
};

fn run(quad: &mut QuadBody, commands: MotorCommands, seconds: f64) {
    for _ in 0..(seconds / STEP).round() as u64 {
        quad.step(&WORLD, &commands, STEP);
    }
}

fn command(throttle: f64, direction: SpinDirection) -> MotorCommand {
    MotorCommand {
        throttle,
        direction,
    }
}

#[test]
fn the_thrust_stand_holds_the_quad_still_while_its_motors_run() {
    let mut quad = QuadBody::new(
        whoop(),
        start(StartingMotors::Stopped, Mount::ThrustStand),
        &WORLD,
    )
    .unwrap();
    let before = *quad.state();
    run(&mut quad, MotorCommands::all(1.0), 1.0);
    assert_eq!(*quad.state(), before);
    let thrust: f64 = quad.motors().iter().map(|m| m.thrust).sum();
    assert!(
        thrust > 3.0 * 0.0312 * 9.81,
        "full drive should make several times the weight, not {thrust} N"
    );
}

#[test]
fn a_motor_spinning_backwards_pushes_the_other_way_with_the_quads_reverse_thrust_share() {
    // Thrust grows with the speed squared; spinning backwards, it points the
    // other way and is the Quad definition's reverse share of forward's: 50%.
    let thrust_per_speed_squared = |direction| {
        let mut quad = QuadBody::new(
            whoop(),
            start(StartingMotors::Stopped, Mount::ThrustStand),
            &WORLD,
        )
        .unwrap();
        run(&mut quad, MotorCommands([command(0.5, direction); 4]), 1.0);
        let motor = quad.motors()[0];
        motor.thrust / (motor.speed * motor.speed)
    };
    let forward = thrust_per_speed_squared(SpinDirection::Normal);
    let backward = thrust_per_speed_squared(SpinDirection::Reversed);
    assert!(forward > 0.0);
    assert!(((backward / forward) - -0.5).abs() < 1e-12);
}

#[test]
fn faster_anticlockwise_props_yaw_the_quad_right() {
    // With the props in (Betaflight's default), motors 2 and 3 turn
    // anticlockwise seen from above. Their drag twists the frame the other
    // way, clockwise: the nose turns right. Being opposite each other, they
    // neither roll nor pitch it.
    let mut quad =
        QuadBody::new(whoop(), start(StartingMotors::Stopped, Mount::Free), &WORLD).unwrap();
    let faster = command(0.6, SpinDirection::Normal);
    let slower = command(0.4, SpinDirection::Normal);
    run(
        &mut quad,
        MotorCommands([slower, faster, faster, slower]),
        0.5,
    );
    let rates = PilotRates::from_body(quad.state().rotation);
    assert!(
        rates.yaw > 0.1,
        "it should yaw right, not at {} rad/s",
        rates.yaw
    );
    assert!(rates.roll.abs() < 1e-9 && rates.pitch.abs() < 1e-9);

    // Props out, every prop turns the other way, and so does the Quad.
    let mut parameters = whoop();
    parameters.rotors.direction = PropDirection::PropsOut;
    let mut quad = QuadBody::new(
        parameters,
        start(StartingMotors::Stopped, Mount::Free),
        &WORLD,
    )
    .unwrap();
    run(
        &mut quad,
        MotorCommands([slower, faster, faster, slower]),
        0.5,
    );
    assert!(PilotRates::from_body(quad.state().rotation).yaw < -0.1);
}

#[test]
fn faster_right_motors_roll_the_quad_left_and_faster_front_motors_pitch_it_up() {
    // Motors 1 and 2 are on the right; more thrust there lifts the right side.
    // Motors 2 and 4 are at the front; more thrust there lifts the nose.
    let roll_and_pitch = |commands: [f64; 4]| {
        let mut quad =
            QuadBody::new(whoop(), start(StartingMotors::Stopped, Mount::Free), &WORLD).unwrap();
        run(
            &mut quad,
            MotorCommands(commands.map(|t| command(t, SpinDirection::Normal))),
            0.3,
        );
        let rates = PilotRates::from_body(quad.state().rotation);
        (rates.roll, rates.pitch)
    };
    let (roll, _) = roll_and_pitch([0.6, 0.6, 0.4, 0.4]);
    assert!(roll < -0.1, "it should roll left, not at {roll} rad/s");
    let (_, pitch) = roll_and_pitch([0.4, 0.6, 0.4, 0.6]);
    assert!(pitch > 0.1, "it should pitch up, not at {pitch} rad/s");
}

#[test]
fn settled_motors_carry_a_level_quads_weight_from_the_first_step() {
    let mut quad =
        QuadBody::new(whoop(), start(StartingMotors::Settled, Mount::Free), &WORLD).unwrap();
    let thrust: f64 = quad.motors().iter().map(|m| m.thrust).sum();
    assert!((thrust - 0.0312 * 9.81).abs() < 1e-12);
    assert!(quad.motors().iter().all(|m| m.esc == EscState::Running));
    quad.step(&WORLD, &MotorCommands::all(0.0), STEP);
    // One step on, the motors have barely started to slow: the Quad has
    // hardly begun to fall.
    assert!(quad.state().velocity.z.abs() < 1e-4);
    assert_eq!(quad.state().velocity.x, 0.0);
}

#[test]
fn powered_up_escs_play_their_tones_and_ignore_every_command_until_ready() {
    // Source: Bluejay v0.21.0 (see the esc module): ready about 1.66 s after
    // power-up, with the throttle held at zero.
    let mut quad = QuadBody::new(
        whoop(),
        start(StartingMotors::PoweringUp, Mount::ThrustStand),
        &WORLD,
    )
    .unwrap();
    assert_eq!(
        quad.motors()[0].esc,
        EscState::StartingUp(StartUpStep::PowerOn)
    );
    // Full throttle during the melody is ignored, and forgotten by the time
    // the ESC counts zero throttle.
    run(&mut quad, MotorCommands::STOPPED, 0.5);
    assert_eq!(
        quad.motors()[0].esc,
        EscState::StartingUp(StartUpStep::Melody)
    );
    run(&mut quad, MotorCommands::all(1.0), 0.5);
    assert_eq!(quad.motors()[0].speed, 0.0);
    run(&mut quad, MotorCommands::STOPPED, 0.65);
    assert!(matches!(quad.motors()[0].esc, EscState::StartingUp(_)));
    run(&mut quad, MotorCommands::STOPPED, 0.02);
    assert_eq!(quad.motors()[0].esc, EscState::Ready);
}

#[test]
fn a_flat_pack_keeps_fading_past_empty_with_no_cutoff() {
    let battery = whoop().battery;
    let empty = battery.resting_cell_voltage(0.0);
    assert_eq!(empty, 3.30);
    let past = battery.resting_cell_voltage(-0.1);
    assert!(past < empty && past > 0.0);
    assert!(battery.resting_cell_voltage(-0.2) < past);
    // Points between the curve's are on the straight line between them.
    assert!((battery.resting_cell_voltage(0.65) - (3.92 + 4.15) / 2.0).abs() < 1e-12);
    assert_eq!(battery.resting_cell_voltage(1.0), 4.35);
}

#[test]
fn a_motor_with_its_esc_off_spins_down_freely_and_stops_without_turning_back() {
    // Ready ESCs switch off every motor that isn't commanded: a settled motor
    // told 0% is braked, then let go below Bluejay's minimum speed, and the
    // prop's drag and the motor's losses bring it to rest.
    let mut quad = QuadBody::new(
        whoop(),
        start(StartingMotors::Settled, Mount::ThrustStand),
        &WORLD,
    )
    .unwrap();
    run(&mut quad, MotorCommands::STOPPED, 1.0);
    let motor = quad.motors()[0];
    assert_eq!(motor.esc, EscState::Ready);
    assert!(motor.speed >= 0.0 && motor.speed < 30.0);
    assert_eq!(motor.current, 0.0);
}
