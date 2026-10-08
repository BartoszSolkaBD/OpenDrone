//! Readable checks for the Simulation's steps, its clock, its list of Quads and
//! the scripted-motors stand-in. What the Quads do in flight is proved by the
//! Scenarios in `scenarios/`.

use opendrone_maths::Fingerprinter;
use opendrone_maths::{Attitude, Mat3, Vec3};
use opendrone_sim::{
    BatteryParameters, Channel, Channels, Drag, DuctRings, EscParameters, EscState,
    FlightControllerSeam, FlightInput, MapShape, MotorCommands, MotorParameters, Mount, PacketRate,
    PhysicsRate, PropDirection, PropParameters, QuadParameters, QuadPart, QuadSetUp, QuadShape,
    QuadState, RotorLayout, ScriptedMotors, SensorReadings, SetUp, SetUpError, SetUpProblem,
    Simulation, SimulationTime, StartingMotors, World,
};
use std::cell::RefCell;
use std::rc::Rc;

const WORLD: World = World {
    gravity: 9.81,
    air_density: 1.225,
};

/// A whoop-sized Quad: the Whoop 65's numbers in SI units.
fn parameters() -> QuadParameters {
    let kv = 19500.0 * 2.0 * core::f64::consts::PI / 60.0;
    QuadParameters {
        mass: 0.0312,
        inertia: Mat3::diagonal(Vec3::new(7.0e-6, 9.0e-6, 14.0e-6)),
        drag: Drag {
            body_area: Vec3::ZERO,
            rotor: 0.0,
            duct_ram: 0.0,
        },
        rotors: RotorLayout {
            diagonal: 0.066,
            rotor_height: 0.008,
            direction: PropDirection::PropsIn,
        },
        props: PropParameters {
            diameter: 0.035,
            thrust_coefficient: 0.29,
            power_coefficient: 0.26,
            rotor_inertia: 0.25e-7,
            reverse_thrust: 0.5,
            reverse_torque: 1.0,
        },
        motors: MotorParameters {
            kv,
            poles: 12,
            winding_resistance: 0.5,
            no_load_current: 0.3,
            no_load_voltage: 4.0,
            spin_up: 0.035,
            slow_down: 0.035,
        },
        esc: EscParameters {
            start_wait: 0.1,
            startup_power_limit: 0.0196,
            restart_tries: 3,
        },
        battery: BatteryParameters {
            cells: 1,
            capacity: 0.320 * 3600.0,
            voltage_curve: vec![(1.0, 4.35), (0.5, 3.92), (0.0, 3.30)],
            resistance: 0.029,
            connector: 0.010,
            recovery: 3.3,
            slow_sag: 0.0,
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
        motors: StartingMotors::Stopped,
        battery: 1.0,
        mount: Mount::Free,
        packet_rate: PacketRate::from_hz(250).unwrap(),
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
    let throttle = |motors: &mut ScriptedMotors, tick| motors.at(at(tick)).0[0].throttle;
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

#[test]
fn each_tick_reports_every_motor_and_its_esc_and_the_battery() {
    let mut quad = quad_at(0.0);
    quad.mount = Mount::ThrustStand;
    quad.flight_controller = Box::new(ScriptedMotors::new(vec![(
        SimulationTime::START,
        MotorCommands::all(0.5),
    )]));
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    let before = sim.quad_output(0);
    assert!(before.motors.iter().all(|m| m.esc == EscState::Ready));
    assert_eq!(before.battery.charge, 1.0);
    for _ in 0..8000 {
        sim.step();
    }
    let after = sim.quad_output(0);
    assert_eq!(after.state, before.state, "the thrust stand holds it still");
    for motor in after.motors {
        assert_eq!(motor.esc, EscState::Running);
        assert!(motor.speed > 0.0 && motor.thrust > 0.0 && motor.torque > 0.0);
        assert!(motor.current > 0.0 && motor.supply_current > 0.0);
    }
    assert!(after.battery.voltage < before.battery.voltage);
    assert!(after.battery.charge_used > 0.0 && after.battery.charge < 1.0);
}

/// A stand-in for the Flight Controller that writes down the tick of every
/// Radio Link frame it is handed, and the frame's roll Channel.
struct Listener {
    heard: Rc<RefCell<Vec<(u64, u16)>>>,
}

impl FlightControllerSeam for Listener {
    fn step(
        &mut self,
        time: SimulationTime,
        _readings: &SensorReadings,
        frame: Option<&Channels>,
    ) -> MotorCommands {
        if let Some(channels) = frame {
            self.heard
                .borrow_mut()
                .push((time.ticks(), channels.roll.step()));
        }
        MotorCommands::STOPPED
    }

    fn write_fingerprint(&self, _f: &mut Fingerprinter) {}
}

/// Runs a Quad with a [`Listener`] for `ticks` steps at `physics_hz`, with
/// the Radio Link at `packet_hz`, giving it these Flight Inputs first. Gives
/// back every frame the listener heard.
fn listen(
    physics_hz: u32,
    packet_hz: u32,
    inputs: &[(u64, Channels)],
    ticks: u64,
) -> Vec<(u64, u16)> {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut quad = quad_at(10.0);
    quad.packet_rate = PacketRate::from_hz(packet_hz).unwrap();
    quad.flight_controller = Box::new(Listener {
        heard: Rc::clone(&heard),
    });
    let mut sim = Simulation::new(set_up(physics_hz, 1, vec![quad])).unwrap();
    for (tick, channels) in inputs {
        sim.flight_input(
            0,
            SimulationTime::from_ticks(*tick),
            FlightInput::Channels(*channels),
        );
    }
    for _ in 0..ticks {
        sim.step();
    }
    heard.take()
}

fn rolled(step: u16) -> Channels {
    Channels {
        roll: Channel::from_step(step).unwrap(),
        ..Channels::RESTING
    }
}

#[test]
fn the_radio_link_sends_a_frame_every_32_steps_at_250_hz_and_8_khz() {
    let heard = listen(8000, 250, &[(0, rolled(992))], 8000);
    assert_eq!(heard.len(), 250, "250 frames in one second");
    for (k, (tick, _)) in heard.iter().enumerate() {
        assert_eq!(*tick, 32 * k as u64);
    }
}

#[test]
fn the_radio_link_sends_nothing_until_the_first_channels_arrive() {
    // The Channels arrive at step 100; the first frame due after that leaves
    // at step 128.
    let heard = listen(8000, 250, &[(100, rolled(992))], 200);
    assert_eq!(heard, vec![(128, 992), (160, 992), (192, 992)]);
}

#[test]
fn channels_between_two_frames_wait_for_the_next_frame_and_the_newest_wins() {
    let inputs = [(0, rolled(992)), (5, rolled(1000)), (20, rolled(1811))];
    let heard = listen(8000, 250, &inputs, 64);
    assert_eq!(heard, vec![(0, 992), (32, 1811)]);
}

#[test]
fn a_frame_due_between_two_physics_steps_leaves_on_the_later_one() {
    // 333 Hz at 8 kHz: frame k is due 8000 k / 333 steps from the start,
    // 24.024… steps apart, so it leaves on the step that rounds that up.
    let heard = listen(8000, 333, &[(0, rolled(992))], 8000);
    assert_eq!(heard.len(), 333);
    for (k, (tick, _)) in heard.iter().enumerate() {
        assert_eq!(*tick, (k as u64 * 8000).div_ceil(333));
    }
}

#[test]
fn only_the_packet_rates_a_pilot_can_pick_are_accepted() {
    for hz in [50, 100, 150, 250, 333, 500, 1000] {
        assert!(PacketRate::from_hz(hz).is_some(), "{hz} Hz");
    }
    for hz in [0, 200, 2000] {
        assert!(PacketRate::from_hz(hz).is_none(), "{hz} Hz");
    }
}
