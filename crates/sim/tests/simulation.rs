//! Readable checks for the Simulation's steps, its clock, its list of Quads and
//! the scripted-motors stand-in. What the Quads do in flight is proved by the
//! Scenarios in `scenarios/`.

use opendrone_maths::{Attitude, Mat3, Vec3};
use opendrone_sim::{
    Drag, DuctRings, FlightControllerSeam, MapShape, MotorCommands, PhysicsRate, QuadParameters,
    QuadPart, QuadSetUp, QuadShape, QuadState, ScriptedMotors, SetUp, SetUpError, SetUpProblem,
    Simulation, SimulationTime, World,
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
        shape: whoop_shape(),
    }
}

/// The Whoop 65's collision shape, as its Quad definition gives it.
fn whoop_shape() -> QuadShape {
    QuadShape {
        body: Vec3::new(0.035, 0.030, 0.020),
        pack: Vec3::new(0.064, 0.010, 0.006),
        pack_height: -0.006,
        diagonal: 0.066,
        rotor_height: 0.008,
        prop_diameter: 0.035,
        duct_rings: Some(DuctRings {
            inside_diameter: 0.037,
            wall: 0.0015,
            height: 0.014,
        }),
        bounce: 0.3,
        friction: 0.5,
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
        map: Vec::new(),
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
        SimulationTime::from_ticks(1),
        MotorCommands::all(0.25),
    )]));
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    sim.step();
    assert_eq!(sim.motor_commands(0), MotorCommands::STOPPED);
    sim.step();
    assert_eq!(sim.motor_commands(0), MotorCommands::all(0.25));
}

#[test]
fn a_set_up_with_a_quad_the_physics_cant_move_is_refused_naming_the_quad() {
    let mut massless = quad_at(0.0);
    massless.parameters.mass = 0.0;
    let error = Simulation::new(set_up(8000, 1, vec![quad_at(0.0), massless]))
        .err()
        .expect("a Quad without mass can't be set up");
    assert_eq!(
        error,
        SetUpError::Quad {
            quad: 1,
            problem: SetUpProblem::MassNotAboveZero
        }
    );
}

#[test]
fn each_ticks_output_names_where_the_map_pushed_the_quad() {
    // The whoop rests on a floor whose top is at 0 m: its body box, 20 mm
    // tall round its centre, touches it at its four bottom corners.
    let floor = MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -1.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    };
    let mut landed = set_up(8000, 1, vec![quad_at(0.010), quad_at(5.0)]);
    landed.map = vec![floor];
    let mut sim = Simulation::new(landed).unwrap();
    assert!(sim.contacts(0).is_empty(), "nothing has happened yet");
    sim.step();
    let contacts = sim.contacts(0);
    assert_eq!(contacts.len(), 4, "{contacts:?}");
    for contact in contacts {
        assert_eq!(contact.part, QuadPart::Body);
        assert_eq!(contact.shape, 0);
        assert_eq!(contact.normal, Vec3::new(0.0, 0.0, 1.0));
        assert!(contact.push > 0.0);
    }
    // The Quad 5 m up touches nothing.
    assert!(sim.contacts(1).is_empty());
}
