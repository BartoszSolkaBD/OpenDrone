//! Readable checks for the Simulation's steps, its clock, its list of Quads and
//! the scripted-motors stand-in. What the Quads do in flight is proved by the
//! Scenarios in `scenarios/`.

use opendrone_maths::{Attitude, Mat3, Vec3};
use opendrone_sim::{
    Drag, FlightControllerSeam, MotorCommands, PhysicsRate, QuadParameters, QuadSetUp, QuadState,
    ScriptedMotors, SetUp, SetUpProblem, Simulation, SimulationTime, World,
};

const WORLD: World = World {
    gravity: 9.81,
    air_density: 1.225,
};

fn parameters() -> QuadParameters {
    QuadParameters {
        mass: 0.0312,
        inertia: Mat3::diagonal(Vec3::new(7.0e-6, 9.0e-6, 14.0e-6)),
        drag: Drag {
            body_area: Vec3::ZERO,
            rotor: 0.0,
            duct_ram: 0.0,
        },
    }
}

fn quad_at(height: f64) -> QuadSetUp {
    QuadSetUp {
        parameters: parameters(),
        start: QuadState {
            position: Vec3::new(0.0, 0.0, height),
            velocity: Vec3::ZERO,
            attitude: Attitude::BODY_IS_WORLD,
            rotation: Vec3::ZERO,
        },
        flight_controller: Box::new(ScriptedMotors::new(Vec::new())),
    }
}

fn set_up(hz: u32, random_seed: u64, quads: Vec<QuadSetUp>) -> SetUp {
    SetUp {
        physics_rate: PhysicsRate::from_hz(hz).unwrap(),
        world: WORLD,
        random_seed,
        quads,
    }
}

#[test]
fn simulation_time_counts_whole_steps_at_the_set_ups_physics_rate() {
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad_at(0.0)])).unwrap();
    for _ in 0..8000 {
        sim.step();
    }
    assert_eq!(sim.time(), SimulationTime::from_ticks(8000));
    assert_eq!(sim.time().seconds(sim.physics_rate()), 1.0);

    let mut slower = Simulation::new(set_up(4000, 1, vec![quad_at(0.0)])).unwrap();
    for _ in 0..4000 {
        slower.step();
    }
    assert_eq!(slower.time().seconds(slower.physics_rate()), 1.0);
}

#[test]
fn a_physics_rate_of_zero_is_refused() {
    assert_eq!(PhysicsRate::from_hz(0), None);
}

#[test]
fn every_quad_in_the_list_is_stepped_and_keeps_its_own_state() {
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad_at(10.0), quad_at(20.0)])).unwrap();
    for _ in 0..800 {
        sim.step();
    }
    assert_eq!(sim.quad_count(), 2);
    // Both fell for the same time, so both fall equally fast, but each from
    // its own height.
    assert!(sim.quad_state(0).velocity.z < 0.0);
    assert_eq!(sim.quad_state(0).velocity, sim.quad_state(1).velocity);
    assert!(sim.quad_state(0).position.z < 10.0);
    assert!(sim.quad_state(1).position.z > 19.0);
}

#[test]
fn the_same_set_up_gives_the_same_fingerprint_at_every_step() {
    let mut one = Simulation::new(set_up(8000, 7, vec![quad_at(1.0)])).unwrap();
    let mut two = Simulation::new(set_up(8000, 7, vec![quad_at(1.0)])).unwrap();
    for _ in 0..100 {
        assert_eq!(one.fingerprint(), two.fingerprint());
        one.step();
        two.step();
    }
}

#[test]
fn the_fingerprint_covers_the_seed_the_order_of_quads_and_the_time() {
    let fingerprint = |seed, heights: [f64; 2]| {
        Simulation::new(set_up(8000, seed, heights.map(quad_at).into()))
            .unwrap()
            .fingerprint()
    };
    assert_ne!(fingerprint(1, [1.0, 2.0]), fingerprint(2, [1.0, 2.0]));
    assert_ne!(fingerprint(1, [1.0, 2.0]), fingerprint(1, [2.0, 1.0]));

    let mut sim = Simulation::new(set_up(8000, 1, vec![quad_at(1.0)])).unwrap();
    let before = sim.fingerprint();
    sim.step();
    assert_ne!(sim.fingerprint(), before);
}

#[test]
fn scripted_motors_hold_each_command_until_the_next() {
    let at = SimulationTime::from_ticks;
    let mut motors = ScriptedMotors::new(vec![
        (at(10), MotorCommands::all(0.5)),
        (at(2), MotorCommands::all(0.25)),
    ]);
    let throttle = |motors: &mut ScriptedMotors, tick| motors.step(at(tick)).0[0].throttle;
    assert_eq!(throttle(&mut motors, 0), 0.0);
    assert_eq!(throttle(&mut motors, 1), 0.0);
    assert_eq!(throttle(&mut motors, 2), 0.25);
    assert_eq!(throttle(&mut motors, 9), 0.25);
    assert_eq!(throttle(&mut motors, 10), 0.5);
    assert_eq!(throttle(&mut motors, 5000), 0.5);
}

#[test]
fn the_simulation_reports_the_motor_commands_the_seam_gave() {
    let mut quad = quad_at(0.0);
    quad.flight_controller = Box::new(ScriptedMotors::new(vec![(
        SimulationTime::START,
        MotorCommands::all(0.0),
    )]));
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    sim.step();
    assert_eq!(sim.motor_commands(0), MotorCommands::STOPPED);
}

#[test]
fn a_set_up_with_a_quad_the_physics_cant_move_is_refused_naming_the_quad() {
    let mut massless = quad_at(0.0);
    massless.parameters.mass = 0.0;
    let error = Simulation::new(set_up(8000, 1, vec![quad_at(0.0), massless]))
        .err()
        .expect("a Quad without mass can't be set up");
    assert_eq!(error.quad, 1);
    assert_eq!(error.problem, SetUpProblem::MassNotAboveZero);
}
