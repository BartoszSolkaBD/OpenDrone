//! Readable checks for the Simulation's steps, its clock, its list of Quads and
//! the scripted-motors stand-in. What the Quads do in flight is proved by the
//! Scenarios in `scenarios/`.

use opendrone_maths::Fingerprinter;
use opendrone_maths::{Attitude, DEGREE, Mat3, PilotAngles, PilotRates, Vec3};
use opendrone_sim::{
    BatteryParameters, Channel, Channels, Drag, DuctRings, EscParameters, EscState,
    FlightControllerSeam, FlightInput, MapShape, MotorCommands, MotorParameters, Mount,
    OurFlightController, PacketRate, PhysicsRate, PropDirection, PropParameters, QuadParameters,
    QuadPart, QuadSetUp, QuadShape, QuadState, Rates, RotorLayout, ScriptedMotors, SensorReadings,
    SetUp, SetUpError, SetUpProblem, Simulation, SimulationTime, StartingMotors, Tune, World,
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
            duct_offset: 0.0,
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
            grip: 0.5,
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
        gyro_range: 2000.0 * DEGREE,
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
    let start = QuadState {
        position: Vec3::new(0.0, 0.0, height),
        velocity: Vec3::ZERO,
        attitude: Attitude::BODY_IS_WORLD,
        rotation: Vec3::ZERO,
    };
    QuadSetUp {
        parameters: parameters(),
        start,
        launch_spot: start,
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

    fn power_up(&mut self, _readings: &SensorReadings) {}

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
    let inputs: Vec<(u64, FlightInput)> = inputs
        .iter()
        .map(|(tick, channels)| (*tick, FlightInput::Channels(*channels)))
        .collect();
    listen_to(physics_hz, packet_hz, &inputs, ticks)
}

/// [`listen`], with any Flight Inputs.
fn listen_to(
    physics_hz: u32,
    packet_hz: u32,
    inputs: &[(u64, FlightInput)],
    ticks: u64,
) -> Vec<(u64, u16)> {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut quad = quad_at(10.0);
    quad.packet_rate = PacketRate::from_hz(packet_hz).unwrap();
    quad.flight_controller = Box::new(Listener {
        heard: Rc::clone(&heard),
    });
    let mut sim = Simulation::new(set_up(physics_hz, 1, vec![quad])).unwrap();
    for (tick, input) in inputs {
        sim.flight_input(0, SimulationTime::from_ticks(*tick), *input);
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

#[test]
fn while_the_input_device_is_lost_the_radio_link_sends_no_frames() {
    // Basis: Rule (#37: an Input Device lost or back is a Flight Input; while
    // lost, the Radio Link sends no frames). Lost at step 40, back at step
    // 100: the frames due at 64 and 96 never leave, and the next, at 128,
    // carries the newest Channels, which arrived while it was lost.
    let inputs = [
        (0, FlightInput::Channels(rolled(992))),
        (40, FlightInput::InputDeviceLost),
        (50, FlightInput::Channels(rolled(1811))),
        (100, FlightInput::InputDeviceBack),
    ];
    let heard = listen_to(8000, 250, &inputs, 170);
    assert_eq!(heard, vec![(0, 992), (32, 992), (128, 1811), (160, 1811)]);
}

/// The Freestyle 5″'s Tune: Betaflight 2026.6.2's defaults.
fn tune() -> Tune {
    Tune::read([
        ("small_angle", "25"),
        ("rx_min_usec", "885"),
        ("rx_max_usec", "2115"),
        ("failsafe_delay", "15"),
        ("failsafe_procedure", "DROP"),
        ("failsafe_throttle", "1000"),
        ("failsafe_recovery_delay", "5"),
        ("p_roll", "45"),
        ("i_roll", "80"),
        ("d_roll", "30"),
        ("p_pitch", "47"),
        ("i_pitch", "84"),
        ("d_pitch", "34"),
        ("p_yaw", "45"),
        ("i_yaw", "80"),
        ("d_yaw", "0"),
        ("motor_output_limit", "100"),
        ("pidsum_limit", "500"),
        ("pidsum_limit_yaw", "400"),
        ("iterm_windup", "80"),
        ("pid_at_min_throttle", "ON"),
        ("min_check", "1050"),
        ("mid_rc", "1500"),
        ("deadband", "0"),
        ("yaw_deadband", "0"),
        ("yaw_control_reversed", "OFF"),
        ("airmode_start_throttle_percent", "25"),
        ("motor_pwm_protocol", "DSHOT600"),
        ("motor_idle", "550"),
        ("yaw_motors_reversed", "OFF"),
        ("mixer_type", "LEGACY"),
    ])
    .unwrap()
}

#[test]
fn reset_puts_the_quad_on_its_launch_spot_still_disarmed_with_a_full_battery_and_its_escs_starting_up()
 {
    // Basis: Rule (#37: Reset is a Flight Input that puts the Quad on the
    // Launch Spot, landed and disarmed, powered up fresh: a full battery and
    // the ESCs starting up). The whoop flies armed on half a battery, 5 m
    // up, its Arm switch on; Reset at 0.5 s.
    let floor = MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -1.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    };
    let mut quad = quad_at(5.0);
    quad.launch_spot.position = Vec3::new(1.0, 2.0, 0.010);
    quad.motors = StartingMotors::Settled;
    quad.battery = 0.5;
    quad.flight_controller = Box::new(OurFlightController::new(
        tune(),
        Rates::BETAFLIGHT_DEFAULT,
        PhysicsRate::from_hz(8000).unwrap(),
        true,
        false,
        &quad.start,
        quad.parameters.gyro_range,
    ));
    let mut set_up = set_up(8000, 1, vec![quad]);
    set_up.map = vec![floor];
    let mut sim = Simulation::new(set_up).unwrap();
    let flying = Channels {
        throttle: Channel::from_throttle(0.4),
        arm: Channel::HIGH,
        ..Channels::RESTING
    };
    sim.flight_input(0, SimulationTime::START, FlightInput::Channels(flying));
    sim.flight_input(0, SimulationTime::from_ticks(4000), FlightInput::Reset);
    for _ in 0..4000 {
        sim.step();
    }
    let before = sim.quad_output(0);
    assert!(before.flight_controller.unwrap().armed);
    assert!(before.state.position.z < 5.0 && before.battery.charge < 0.5);
    sim.step();
    let after = sim.quad_output(0);
    assert_eq!(after.state.position, Vec3::new(1.0, 2.0, 0.010));
    assert_eq!(after.state.velocity, Vec3::ZERO);
    assert_eq!(after.state.rotation, Vec3::ZERO);
    assert_eq!(after.battery.charge, 1.0);
    assert!(
        after
            .motors
            .iter()
            .all(|m| matches!(m.esc, EscState::StartingUp(_)) && m.speed == 0.0)
    );
    let record = after.flight_controller.unwrap();
    assert!(!record.armed);
    // A frame arrived on the same step, with the throttle still up and the
    // Arm switch still on: refused until the ESCs are ready and the throttle
    // is low, and then until the switch goes off and on again.
    assert_eq!(
        record.arming_blocks.names(),
        ["THROTTLE", "BOOTGRACE", "ARM_SWITCH"]
    );
}

#[test]
fn the_flight_controller_hears_whether_the_escs_have_beeped_ready() {
    // Basis: Rule (#32 §4: arming waits for the ESCs' ready beep, about
    // 1.66 s after power-up, as Bluejay's timeline in the physics gives it).
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut quad = quad_at(0.010);
    quad.motors = StartingMotors::PoweringUp;
    quad.flight_controller = Box::new(EscListener {
        heard: Rc::clone(&heard),
    });
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    for _ in 0..16_000 {
        sim.step();
    }
    let heard = heard.take();
    let first_ready = heard.iter().position(|ready| *ready).unwrap();
    assert!(heard[first_ready..].iter().all(|ready| *ready));
    let seconds = first_ready as f64 / 8000.0;
    assert!((1.66..1.67).contains(&seconds), "ready at {seconds} s");
}

/// A stand-in for the Flight Controller that writes down, each tick,
/// whether the sensor readings said the ESCs were ready.
struct EscListener {
    heard: Rc<RefCell<Vec<bool>>>,
}

impl FlightControllerSeam for EscListener {
    fn step(
        &mut self,
        _time: SimulationTime,
        readings: &SensorReadings,
        _frame: Option<&Channels>,
    ) -> MotorCommands {
        self.heard.borrow_mut().push(readings.escs_ready);
        MotorCommands::STOPPED
    }

    fn power_up(&mut self, _readings: &SensorReadings) {}

    fn write_fingerprint(&self, _f: &mut Fingerprinter) {}
}

#[test]
fn each_ticks_contacts_and_output_say_how_hard_each_prop_rubs() {
    // The whoop without its duct rings, level and nose to the north-east,
    // its motors settled: only prop 2 reaches east of the rest, 50.5 mm from
    // the centre of mass. A wall 1 mm beyond it; the Quad drifts into it.
    let mut quad = quad_at(1.0);
    quad.parameters.shape.duct_rings = None;
    quad.motors = StartingMotors::Settled;
    quad.start.velocity = Vec3::new(0.05, 0.0, 0.0);
    quad.start.attitude = Attitude::from_pilot_angles(PilotAngles {
        roll: 0.0,
        pitch: 0.0,
        heading: 45.0 * DEGREE,
    });
    quad.flight_controller = Box::new(ScriptedMotors::new(vec![(
        SimulationTime::START,
        MotorCommands::all(0.373),
    )]));
    let mut wall = set_up(8000, 1, vec![quad]);
    wall.map = vec![MapShape::Box {
        centre: Vec3::new(0.0505 + 0.001 + 0.1, 0.0, 1.0),
        size: Vec3::new(0.2, 20.0, 20.0),
        attitude: Attitude::BODY_IS_WORLD,
    }];
    let mut sim = Simulation::new(wall).unwrap();
    assert_eq!(sim.quad_output(0).prop_rubs, [0.0; 4]);
    while sim.contacts(0).is_empty() {
        sim.step();
        assert!(sim.time().ticks() < 8000, "prop 2 never reached the wall");
    }
    let contacts = sim.contacts(0);
    assert!(contacts.iter().all(|c| c.part == QuadPart::PropDisc(2)));
    assert!(contacts.iter().all(|c| c.rub > 0.0));
    let rubbed: f64 = contacts.iter().map(|c| c.rub).sum();
    assert_eq!(sim.quad_output(0).prop_rubs, [0.0, rubbed, 0.0, 0.0]);
}

#[test]
fn each_ticks_output_says_what_the_gyro_reads() {
    // Spinning nose right at 3,000 °/s, past the gyro's ±2,000 °/s, and
    // rolling right at 500 °/s, within it.
    let mut quad = quad_at(10.0);
    quad.start.rotation = PilotRates {
        roll: 500.0 * DEGREE,
        pitch: 0.0,
        yaw: 3000.0 * DEGREE,
    }
    .to_body();
    let sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    let gyro = PilotRates::from_body(sim.quad_output(0).gyro);
    assert_eq!(gyro.roll, 500.0 * DEGREE);
    assert_eq!(gyro.pitch, 0.0);
    assert_eq!(gyro.yaw, 2000.0 * DEGREE);
}

#[test]
fn the_flight_controller_reads_the_gyro_clipped_at_its_range() {
    // Basis: Rule (#26 §4): the Flight Controller sees only what a real
    // board sees, so its gyro reads 2,000 °/s of a 3,000 °/s spin. Spinning
    // nose right at 3,000 °/s and rolling right at 500 °/s, 10 m up, our
    // Flight Controller disarmed: nothing slows the spin.
    let mut quad = quad_at(10.0);
    quad.start.rotation = PilotRates {
        roll: 500.0 * DEGREE,
        pitch: 0.0,
        yaw: 3000.0 * DEGREE,
    }
    .to_body();
    quad.flight_controller = Box::new(OurFlightController::new(
        tune(),
        Rates::BETAFLIGHT_DEFAULT,
        PhysicsRate::from_hz(8000).unwrap(),
        false,
        false,
        &quad.start,
        quad.parameters.gyro_range,
    ));
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    for _ in 0..80 {
        // The Flight Controller reads the sensors at the start of each tick.
        // Spinning about two axes at once, the true rates drift a little
        // (Euler's equations), but the yaw stays far past the gyro's range.
        let rates = PilotRates::from_body(sim.quad_output(0).state.rotation);
        assert!(rates.yaw > 2900.0 * DEGREE && rates.roll < 1000.0 * DEGREE);
        sim.step();
        // Betaflight's axes, in °/s: roll right, pitch nose down and yaw
        // nose left are positive.
        let read = sim.quad_output(0).flight_controller.unwrap().gyro;
        assert!(
            (read[0] - rates.roll / DEGREE).abs() < 1e-9,
            "roll {}",
            read[0]
        );
        assert!(
            (read[1] - -rates.pitch / DEGREE).abs() < 1e-9,
            "pitch {}",
            read[1]
        );
        assert!((read[2] - -2000.0).abs() < 1e-9, "yaw {}", read[2]);
    }
}
