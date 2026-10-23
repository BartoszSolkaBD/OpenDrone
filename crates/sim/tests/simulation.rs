//! Readable checks for the Simulation's steps, its clock, its list of Quads and
//! the scripted-motors stand-in. What the Quads do in flight is proved by the
//! Scenarios in `scenarios/`.

use opendrone_maths::{Attitude, DEGREE, Mat3, PilotAngles, PilotRates, Vec3};
use opendrone_maths::{Fingerprint, Fingerprinter, functions};
use opendrone_sim::{
    BatteryParameters, Channel, Channels, Drag, DuctRings, EscParameters, EscState,
    FlightControllerSeam, FlightInput, GroundAndCeiling, InputDeviceFacts, MapShape, MotorCommands,
    MotorParameters, Mount, OurFlightController, PacketRate, PhysicsRate, PropDirection,
    PropParameters, PropWash, QuadParameters, QuadPart, QuadSetUp, QuadShape, QuadState, Rates,
    ReportRate, RotorLayout, ScriptedMotors, SensorReadings, SetUp, SetUpError, SetUpProblem,
    Simulation, SimulationTime, StartingMotors, Tune, World, sensor_readings,
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
        prop_wash: PropWash::NONE,
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
        ground_and_ceiling: GroundAndCeiling {
            ground_effect_body: 2.0,
            ceiling_effect_asymmetry: 1.0,
        },
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
        camera_housing: None,
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
        input_device: None,
        input_smoothing: false,
        endless_battery: false,
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
fn endless_battery_keeps_the_charge_full_while_the_voltage_still_sags() {
    let run = |endless: bool| {
        let mut quad = quad_at(10.0);
        quad.motors = StartingMotors::Settled;
        quad.endless_battery = endless;
        let mut quad = quad;
        quad.flight_controller = Box::new(ScriptedMotors::new(vec![(
            SimulationTime::START,
            MotorCommands::all(1.0),
        )]));
        let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
        let mut sag = 0.0_f64;
        for _ in 0..4000 {
            sim.step();
            sag = functions::max(sag, sim.quad_output(0).battery.sag);
        }
        (sim.quad_output(0).battery, sag)
    };
    let (flat, _) = run(false);
    let (endless, sag) = run(true);
    assert!(flat.charge < 1.0);
    assert!((endless.charge - 1.0).abs() < 1e-12);
    // What the pack gave is still counted, and the voltage still sags.
    assert!((endless.charge_used - flat.charge_used).abs() < 0.01 * flat.charge_used);
    assert!(sag > 0.1);
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

/// A copy has a log of its own, holding what the original had heard, so that
/// nothing a step writes in one shows in the other (`clone_box`'s rule).
impl Clone for Listener {
    fn clone(&self) -> Self {
        Listener {
            heard: Rc::new(RefCell::new(self.heard.borrow().clone())),
        }
    }
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

    fn clone_box(&self) -> Box<dyn FlightControllerSeam> {
        Box::new(self.clone())
    }
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

#[test]
fn restoring_a_copy_adds_nothing_to_the_log_of_the_flight_controller_it_replaced() {
    // Basis: Rule (`FlightControllerSeam::clone_box`: a copy shares nothing
    // with the original that a step could change). The Simulation's copy
    // holds a clone of the Flight Controller; put back after more frames, it
    // goes on writing its own log, and the log the test watches stops at what
    // the original heard.
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut quad = quad_at(10.0);
    quad.flight_controller = Box::new(Listener {
        heard: Rc::clone(&heard),
    });
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    sim.flight_input(
        0,
        SimulationTime::from_ticks(0),
        FlightInput::Channels(rolled(992)),
    );
    for _ in 0..64 {
        sim.step();
    }
    let copy = sim.snapshot();
    for _ in 0..64 {
        sim.step();
    }
    let heard_by_the_original = heard.borrow().len();
    assert_eq!(heard_by_the_original, 4, "frames at steps 0, 32, 64 and 96");

    sim.restore(&copy);
    for _ in 0..64 {
        sim.step();
    }
    assert_eq!(heard.borrow().len(), heard_by_the_original);
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
        ("yaw_spin_recovery", "AUTO"),
        ("yaw_spin_threshold", "1950"),
        ("runaway_takeoff_prevention", "ON"),
        ("runaway_takeoff_deactivate_delay", "500"),
        ("runaway_takeoff_deactivate_throttle_percent", "20"),
        ("current_meter", "ADC"),
        ("vbat_min_cell_voltage", "330"),
        ("vbat_max_cell_voltage", "430"),
        ("vbat_warning_cell_voltage", "350"),
        ("vbat_full_cell_voltage", "410"),
        ("force_battery_cell_count", "0"),
        ("vbat_hysteresis", "1"),
        ("vbat_detect_cell_voltage", "300"),
        ("use_vbat_alerts", "ON"),
        ("vbat_display_lpf_period", "30"),
        ("ibat_lpf_period", "10"),
        ("vbat_duration_for_warning", "0"),
        ("vbat_duration_for_critical", "0"),
        ("rx_min_usec", "885"),
        ("rx_max_usec", "2115"),
        ("failsafe_delay", "15"),
        ("failsafe_procedure", "DROP"),
        ("failsafe_throttle", "1000"),
        ("failsafe_recovery_delay", "5"),
        ("p_roll", "45"),
        ("i_roll", "80"),
        ("d_roll", "30"),
        ("d_max_roll", "40"),
        ("f_roll", "120"),
        ("p_pitch", "47"),
        ("i_pitch", "84"),
        ("d_pitch", "34"),
        ("d_max_pitch", "46"),
        ("f_pitch", "125"),
        ("p_yaw", "45"),
        ("i_yaw", "80"),
        ("d_yaw", "0"),
        ("d_max_yaw", "0"),
        ("f_yaw", "120"),
        ("feedforward_transition", "0"),
        ("feedforward_averaging", "2_POINT"),
        ("feedforward_smooth_factor", "65"),
        ("feedforward_jitter_factor", "7"),
        ("feedforward_boost", "15"),
        ("feedforward_max_rate_limit", "90"),
        ("d_max_gain", "0"),
        ("d_max_advance", "35"),
        ("iterm_relax", "RP"),
        ("iterm_relax_type", "SETPOINT"),
        ("iterm_relax_cutoff", "15"),
        ("anti_gravity_gain", "80"),
        ("tpa_mode", "D"),
        ("tpa_rate", "65"),
        ("tpa_breakpoint", "1350"),
        ("throttle_boost", "5"),
        ("motor_output_limit", "100"),
        ("angle_p_gain", "50"),
        ("angle_limit", "60"),
        ("horizon_level_strength", "75"),
        ("pidsum_limit", "500"),
        ("pidsum_limit_yaw", "400"),
        ("iterm_windup", "80"),
        ("pid_at_min_throttle", "ON"),
        ("feedforward_yaw_hold_gain", "15"),
        ("feedforward_yaw_hold_time", "100"),
        ("anti_gravity_cutoff_hz", "5"),
        ("anti_gravity_p_gain", "100"),
        ("tpa_low_rate", "20"),
        ("tpa_low_breakpoint", "1050"),
        ("tpa_low_always", "OFF"),
        ("throttle_boost_cutoff", "15"),
        ("angle_feedforward", "50"),
        ("angle_feedforward_smoothing_ms", "80"),
        ("angle_earth_ref", "100"),
        ("horizon_limit_sticks", "75"),
        ("horizon_limit_degrees", "135"),
        ("horizon_ignore_sticks", "OFF"),
        ("horizon_delay_ms", "500"),
        ("level_race_mode", "OFF"),
        ("gyro_lpf1_type", "PT1"),
        ("gyro_lpf1_static_hz", "250"),
        ("gyro_lpf1_dyn_min_hz", "250"),
        ("gyro_lpf1_dyn_max_hz", "500"),
        ("gyro_lpf2_type", "PT1"),
        ("gyro_lpf2_static_hz", "500"),
        ("dterm_lpf1_type", "PT1"),
        ("dterm_lpf1_static_hz", "75"),
        ("dterm_lpf1_dyn_min_hz", "75"),
        ("dterm_lpf1_dyn_max_hz", "150"),
        ("dterm_lpf2_type", "PT1"),
        ("dterm_lpf2_static_hz", "150"),
        ("gyro_lpf1_dyn_expo", "5"),
        ("dterm_lpf1_dyn_expo", "5"),
        ("yaw_lowpass_hz", "100"),
        ("min_check", "1050"),
        ("mid_rc", "1500"),
        ("deadband", "0"),
        ("yaw_deadband", "0"),
        ("rc_smoothing", "ON"),
        ("rc_smoothing_auto_factor", "30"),
        ("rc_smoothing_auto_factor_throttle", "30"),
        ("rc_smoothing_setpoint_cutoff", "0"),
        ("rc_smoothing_throttle_cutoff", "0"),
        ("yaw_control_reversed", "OFF"),
        ("airmode_start_throttle_percent", "25"),
        ("motor_pwm_protocol", "DSHOT600"),
        ("dshot_bidir", "OFF"),
        ("motor_idle", "550"),
        ("motor_poles", "14"),
        ("yaw_motors_reversed", "OFF"),
        ("mixer_type", "LEGACY"),
        ("crashflip_motor_percent", "0"),
        ("crashflip_rate", "0"),
        ("crashflip_auto_rearm", "OFF"),
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
        PacketRate::from_hz(250).unwrap(),
        true,
        false,
        &sensor_readings(&quad.start, false, quad.parameters.gyro_range),
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

/// A copy has a log of its own, holding what the original had heard, so that
/// nothing a step writes in one shows in the other (`clone_box`'s rule).
impl Clone for EscListener {
    fn clone(&self) -> Self {
        EscListener {
            heard: Rc::new(RefCell::new(self.heard.borrow().clone())),
        }
    }
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

    fn clone_box(&self) -> Box<dyn FlightControllerSeam> {
        Box::new(self.clone())
    }
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
        PacketRate::from_hz(250).unwrap(),
        false,
        false,
        &sensor_readings(&quad.start, false, quad.parameters.gyro_range),
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

/// A whoop with the Whoop 65's Prop Wash numbers, level and sinking straight
/// down at 5 m/s, in the middle of the band where a rotor sinks into its own
/// air, its motors settled and held there.
fn whoop_sinking_through_its_own_air(east: f64) -> QuadSetUp {
    let mut quad = quad_at(100.0);
    quad.parameters.prop_wash = PropWash {
        strength: 0.2,
        flicker: 15.0,
    };
    quad.start.position.x = east;
    quad.start.velocity = Vec3::new(0.0, 0.0, -5.0);
    quad.motors = StartingMotors::Settled;
    quad.flight_controller = Box::new(ScriptedMotors::new(vec![(
        SimulationTime::START,
        MotorCommands::all(0.355),
    )]));
    quad
}

#[test]
fn each_quad_flickers_its_own_way_and_the_seed_decides_how() {
    // Each Quad's Prop Wash flicker is seeded from the Simulation's seed in
    // turn, so two Quads flying the same way are shaken differently, and the
    // same set-up shakes them the same way again.
    let flown = |seed| {
        let quads = vec![
            whoop_sinking_through_its_own_air(0.0),
            whoop_sinking_through_its_own_air(5.0),
        ];
        let mut sim = Simulation::new(set_up(8000, seed, quads)).unwrap();
        for _ in 0..800 {
            sim.step();
        }
        [sim.quad_state(0).rotation, sim.quad_state(1).rotation]
    };
    let [first, second] = flown(1);
    assert!(first != Vec3::ZERO && second != Vec3::ZERO);
    assert_ne!(first, second);
    assert_eq!(flown(1), [first, second]);
    assert_ne!(flown(2), [first, second]);
}

#[test]
fn reset_carries_each_rotors_prop_wash_flicker_on() {
    // Basis: Rule (Reset puts the Quad back on its Launch Spot, but the
    // flicker belongs to the air, not the Quad, so it goes on where it was:
    // flying with a Reset at 0.5 s and flying without one give the same
    // flicker at every step).
    let flown = |reset: bool| {
        let mut sim = Simulation::new(set_up(
            8000,
            1,
            vec![whoop_sinking_through_its_own_air(0.0)],
        ))
        .unwrap();
        if reset {
            sim.flight_input(0, SimulationTime::from_ticks(4000), FlightInput::Reset);
        }
        let start = sim.quad_flicker(0);
        let mut after_reset = Vec::new();
        for tick in 0..6000 {
            sim.step();
            if tick >= 4000 && tick % 500 == 0 {
                after_reset.push(sim.quad_flicker(0));
            }
        }
        (start, after_reset, sim.quad_state(0).position.z)
    };
    let (start, with_reset, height_with_reset) = flown(true);
    let (_, without, height_without) = flown(false);
    // The Reset happened: it put the Quad back on its Launch Spot, 100 m up,
    // at rest, so 0.25 s later it has barely sunk, where the other Quad,
    // sinking at 5 m/s all the while, is 3.7 m down.
    assert!(height_with_reset > 99.0, "{height_with_reset}");
    assert!(height_without < 97.0, "{height_without}");
    assert_eq!(with_reset.len(), 4);
    assert_eq!(with_reset, without);
    // Not the flicker starting over: after 0.5 s it is somewhere else.
    for values in with_reset {
        assert_ne!(values, start);
    }
}

/// A stand-in for the Flight Controller that writes down every Radio Link
/// frame it is handed, whole, with its step.
struct FrameListener {
    heard: Rc<RefCell<Vec<(u64, Channels)>>>,
}

/// A copy has a log of its own, holding what the original had heard, so that
/// nothing a step writes in one shows in the other (`clone_box`'s rule).
impl Clone for FrameListener {
    fn clone(&self) -> Self {
        FrameListener {
            heard: Rc::new(RefCell::new(self.heard.borrow().clone())),
        }
    }
}

impl FlightControllerSeam for FrameListener {
    fn step(
        &mut self,
        time: SimulationTime,
        _readings: &SensorReadings,
        frame: Option<&Channels>,
    ) -> MotorCommands {
        if let Some(channels) = frame {
            self.heard.borrow_mut().push((time.ticks(), *channels));
        }
        MotorCommands::STOPPED
    }

    fn power_up(&mut self, _readings: &SensorReadings) {}

    fn write_fingerprint(&self, _f: &mut Fingerprinter) {}

    fn clone_box(&self) -> Box<dyn FlightControllerSeam> {
        Box::new(self.clone())
    }
}

/// Runs a Quad set up by `set` (10 m up, Radio Link at 250 Hz) for `ticks`
/// steps at 8 kHz with these Flight Inputs, giving back every frame its
/// Flight Controller heard, and the Simulation.
fn frames_heard(
    set: impl FnOnce(&mut QuadSetUp),
    inputs: &[(u64, FlightInput)],
    ticks: u64,
) -> (Vec<(u64, Channels)>, Simulation) {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut quad = quad_at(10.0);
    quad.flight_controller = Box::new(FrameListener {
        heard: Rc::clone(&heard),
    });
    set(&mut quad);
    let mut sim = Simulation::new(set_up(8000, 1, vec![quad])).unwrap();
    for (tick, input) in inputs {
        sim.flight_input(0, SimulationTime::from_ticks(*tick), *input);
    }
    for _ in 0..ticks {
        sim.step();
    }
    (heard.take(), sim)
}

/// The sticks at rest, then at 8 ms roll and yaw full right, pitch full back
/// and the throttle full up.
fn stick_jump() -> [(u64, FlightInput); 2] {
    let full = Channels {
        roll: Channel::HIGH,
        pitch: Channel::LOW,
        yaw: Channel::HIGH,
        throttle: Channel::HIGH,
        ..Channels::RESTING
    };
    [
        (0, FlightInput::Channels(Channels::RESTING)),
        (64, FlightInput::Channels(full)),
    ]
}

/// The frame at `tick`.
fn frame_at(frames: &[(u64, Channels)], tick: u64) -> Channels {
    frames
        .iter()
        .find(|(t, _)| *t == tick)
        .map(|(_, channels)| *channels)
        .unwrap_or_else(|| panic!("no frame at step {tick}"))
}

#[test]
fn input_smoothing_takes_roll_pitch_and_yaw_1_minus_e_to_the_minus_2_pi_15_hz_t_of_the_way_and_leaves_the_throttle()
 {
    // Basis: Rule (#21: Input smoothing is one fixed gentle 15 Hz low-pass on
    // roll, pitch and yaw before the Radio Link; the throttle is untouched).
    // A first-order low-pass at 15 Hz has covered 1 − e^(−2π × 15 Hz × t) of
    // a jump t seconds later; each frame carries that, rounded to the
    // receiver's nearest step. The jump arrives at step 64, with a frame.
    let (frames, _) = frames_heard(|q| q.input_smoothing = true, &stick_jump(), 16_000);
    assert_eq!(frames.len(), 500);
    for (tick, channels) in &frames {
        let t = tick.saturating_sub(64) as f64 / 8000.0;
        let covered = -functions::exp_m1(-2.0 * core::f64::consts::PI * 15.0 * t);
        let expected = |from: f64, to: f64| (from + (to - from) * covered).round() as u16;
        assert_eq!(channels.roll.step(), expected(992.0, 1811.0), "step {tick}");
        assert_eq!(channels.pitch.step(), expected(992.0, 172.0), "step {tick}");
        assert_eq!(channels.yaw.step(), expected(992.0, 1811.0), "step {tick}");
        let throttle = if *tick >= 64 { 1811 } else { 172 };
        assert_eq!(channels.throttle.step(), throttle, "step {tick}");
    }
    // The frame at the jump still carries the sticks at rest. The one 12 ms
    // on, at step 160, carries 1 − e^(−2π × 15 Hz × 12 ms) = 67.7% of the
    // way: step 992 + 0.677 × 819 = 1547.
    assert_eq!(frame_at(&frames, 64).roll, Channel::CENTRE);
    assert_eq!(frame_at(&frames, 160).roll.step(), 1547);
    // After 1 s the sticks have arrived.
    let (_, last) = frames.last().unwrap();
    assert_eq!(
        (last.roll, last.pitch, last.yaw),
        (Channel::HIGH, Channel::LOW, Channel::HIGH)
    );
}

#[test]
fn without_input_smoothing_the_sticks_reach_the_next_frame_as_they_came() {
    // Basis: Rule (#21: Input smoothing is an Assist, off unless chosen).
    let (frames, _) = frames_heard(|_| {}, &stick_jump(), 200);
    let jumped = frame_at(&frames, 64);
    assert_eq!((jumped.roll, jumped.pitch), (Channel::HIGH, Channel::LOW));
}

#[test]
fn input_smoothing_starts_where_the_pilot_holds_the_sticks() {
    // Basis: Rule (#21): the first Channels are taken as they are, not
    // ramped to from centre.
    let held = Channels {
        roll: Channel::HIGH,
        ..Channels::RESTING
    };
    let (frames, _) = frames_heard(
        |q| q.input_smoothing = true,
        &[(0, FlightInput::Channels(held))],
        100,
    );
    assert!(!frames.is_empty());
    assert!(frames.iter().all(|(_, f)| f.roll == Channel::HIGH));
}

#[test]
fn the_input_devices_facts_come_with_the_set_up_and_lock_the_radio_link_to_its_beat() {
    // Basis: Rule (ADR-0020, #27). A DualSense's facts: 250 Hz, reports at
    // rest. Reports every 4 ms from step 5 to step 3 973: each frame leaves
    // 6 steps after one, the output says the link is locked, and 1 s after
    // the last report the device counts as lost.
    let inputs: Vec<(u64, FlightInput)> = (0..125u16)
        .map(|k| {
            (
                5 + 32 * u64::from(k),
                FlightInput::Channels(rolled(992 + k)),
            )
        })
        .collect();
    let (frames, mut sim) = frames_heard(
        |q| {
            q.input_device = Some(InputDeviceFacts {
                report_rate: ReportRate::from_hz(250),
                reports_at_rest: true,
            });
        },
        &inputs,
        4_000,
    );
    for (k, (tick, channels)) in frames.iter().enumerate().take(125) {
        assert_eq!(*tick, 11 + 32 * k as u64);
        assert_eq!(channels.roll.step(), 992 + k as u16);
    }
    let output = sim.quad_output(0).radio_link;
    assert!(output.locked && !output.lost);
    assert_eq!(output.last_frame.map(|c| c.roll.step()), Some(992 + 124));
    while sim.time().ticks() < 3_973 + 8_000 {
        sim.step();
        assert!(!sim.quad_output(0).radio_link.lost);
    }
    sim.step();
    assert!(sim.quad_output(0).radio_link.lost);
}

/// A whoop on our Flight Controller, armed and flying 1 m over a floor, with
/// Prop Wash, Input smoothing and a device that reports at rest: every part
/// of a Quad's state that a copy has to carry is doing something.
fn eventful_quad(height: f64) -> QuadSetUp {
    let mut quad = quad_at(height);
    quad.parameters.prop_wash = PropWash {
        strength: 0.2,
        flicker: 15.0,
    };
    quad.launch_spot.position = Vec3::new(0.0, 0.0, 0.010);
    quad.motors = StartingMotors::Settled;
    quad.input_smoothing = true;
    quad.input_device = Some(InputDeviceFacts {
        report_rate: ReportRate::from_hz(250),
        reports_at_rest: true,
    });
    quad.flight_controller = Box::new(OurFlightController::new(
        tune(),
        Rates::BETAFLIGHT_DEFAULT,
        PhysicsRate::from_hz(8000).unwrap(),
        PacketRate::from_hz(250).unwrap(),
        true,
        false,
        &sensor_readings(&quad.start, false, quad.parameters.gyro_range),
    ));
    quad
}

fn over_a_floor(seed: u64, quads: Vec<QuadSetUp>) -> SetUp {
    let mut set_up = set_up(8000, seed, quads);
    set_up.map = vec![MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -1.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    }];
    set_up
}

/// What a pilot does with one Quad over 6 000 steps (0.75 s): the device
/// reports every 32 steps with the sticks wandering (`phase` makes each
/// pilot's wander different), it is lost at step 3 000 and back at 3 400, and
/// Reset comes at 4 000.
fn pilot(phase: u64) -> Vec<(u64, FlightInput)> {
    let mut inputs: Vec<(u64, FlightInput)> = (0..6_000 / 32)
        .map(|k| {
            let channels = Channels {
                roll: Channel::from_stick(0.04 * ((k + phase) % 17) as f64 - 0.3),
                pitch: Channel::from_stick(0.2 - 0.02 * ((k * 3 + phase) % 11) as f64),
                throttle: Channel::from_throttle(0.35 + 0.01 * ((k + phase) % 9) as f64),
                arm: Channel::HIGH,
                ..Channels::RESTING
            };
            (32 * k, FlightInput::Channels(channels))
        })
        .collect();
    inputs.push((3_000, FlightInput::InputDeviceLost));
    inputs.push((3_400, FlightInput::InputDeviceBack));
    inputs.push((4_000, FlightInput::Reset));
    inputs.sort_by_key(|(tick, _)| *tick);
    inputs
}

/// Steps `sim` from its present step to `until`, giving each Quad's pilot's
/// Flight Inputs as their steps come, and writing down the Simulation's
/// fingerprint after every step.
fn fly(sim: &mut Simulation, pilots: &[Vec<(u64, FlightInput)>], until: u64) -> Vec<Fingerprint> {
    let mut fingerprints = Vec::new();
    while sim.time().ticks() < until {
        let now = sim.time().ticks();
        for (quad, inputs) in pilots.iter().enumerate() {
            for (_, input) in inputs.iter().filter(|(tick, _)| *tick == now) {
                sim.flight_input(quad, sim.time(), *input);
            }
        }
        sim.step();
        fingerprints.push(sim.fingerprint());
    }
    fingerprints
}

#[test]
fn restoring_a_copy_and_replaying_the_same_flight_inputs_gives_the_same_fingerprint_at_every_step()
{
    // Basis: Rule (#58: a restored copy, given the same Flight Inputs, is in
    // bit-identical states). A copy at step 2 500, mid-flight on a whoop with
    // a live Flight Controller, Input smoothing, Prop Wash and a Radio Link
    // locked to its device; then the device is lost and comes back, and
    // Reset follows, so the copy must carry everything those touch.
    let pilots = [pilot(0)];
    let mut sim = Simulation::new(over_a_floor(5, vec![eventful_quad(1.0)])).unwrap();
    fly(&mut sim, &pilots, 2_500);
    let copy = sim.snapshot();
    assert_eq!(copy.time(), SimulationTime::from_ticks(2_500));
    let copied_at = sim.fingerprint();
    let first = fly(&mut sim, &pilots, 6_000);
    let ended = sim.quad_output(0);
    assert_ne!(sim.fingerprint(), copied_at);

    sim.restore(&copy);
    assert_eq!(sim.time(), copy.time());
    assert_eq!(sim.fingerprint(), copied_at);
    let again = fly(&mut sim, &pilots, 6_000);
    assert_eq!(again, first);
    assert_eq!(sim.quad_output(0), ended);

    // The same copy can be put back again, and still gives the same flight.
    sim.restore(&copy);
    assert_eq!(fly(&mut sim, &pilots, 6_000), first);
}

#[test]
fn a_copy_fits_any_simulation_built_from_the_same_set_up_and_carries_the_whole_state() {
    // Basis: Rule (#58). A Simulation that has only just been built, with a
    // copy put into it, flies on as the original did: nothing the copy needs
    // lives outside it, the seed and the random generators included.
    let pilots = [pilot(3)];
    let mut sim = Simulation::new(over_a_floor(9, vec![eventful_quad(1.0)])).unwrap();
    fly(&mut sim, &pilots, 3_100);
    let copy = sim.snapshot();
    let copied_at = sim.fingerprint();
    let first = fly(&mut sim, &pilots, 5_000);

    let mut other = Simulation::new(over_a_floor(9, vec![eventful_quad(1.0)])).unwrap();
    other.restore(&copy);
    assert_eq!(other.fingerprint(), copied_at);
    assert_eq!(fly(&mut other, &pilots, 5_000), first);

    let mut other_seed = Simulation::new(over_a_floor(10, vec![eventful_quad(1.0)])).unwrap();
    other_seed.restore(&copy);
    assert_eq!(other_seed.fingerprint(), copied_at);
    assert_eq!(fly(&mut other_seed, &pilots, 5_000), first);
}

#[test]
fn taking_a_copy_changes_nothing() {
    // Basis: Rule (#58).
    let pilots = [pilot(1)];
    let run = |copying: bool| {
        let mut sim = Simulation::new(over_a_floor(2, vec![eventful_quad(1.0)])).unwrap();
        let mut fingerprints = Vec::new();
        for until in (1..=40).map(|k| k * 100) {
            fingerprints.extend(fly(&mut sim, &pilots, until));
            if copying {
                let _ = sim.snapshot();
            }
        }
        fingerprints
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn flight_inputs_still_waiting_when_the_copy_is_taken_wait_again_when_it_is_restored() {
    // Basis: Rule (#58: the copy is the whole state, and a Flight Input given
    // for a later step is part of it). Roll is given for step 100; the copy
    // comes at step 50.
    let roll = FlightInput::Channels(Channels {
        roll: Channel::HIGH,
        ..Channels::RESTING
    });
    let mut sim = Simulation::new(over_a_floor(1, vec![eventful_quad(1.0)])).unwrap();
    sim.flight_input(0, SimulationTime::from_ticks(100), roll);
    for _ in 0..50 {
        sim.step();
    }
    let copy = sim.snapshot();
    for _ in 0..450 {
        sim.step();
    }
    let ended = sim.fingerprint();

    sim.restore(&copy);
    for _ in 0..450 {
        sim.step();
    }
    assert_eq!(sim.fingerprint(), ended);

    // Without the waiting input the flight is another one, so the copy
    // really held it.
    let mut without = Simulation::new(over_a_floor(1, vec![eventful_quad(1.0)])).unwrap();
    for _ in 0..500 {
        without.step();
    }
    assert_ne!(without.fingerprint(), ended);
}

/// SplitMix64's step: each Quad's flicker is seeded from the Simulation's
/// seed in turn, so the seed that gives a Quad flying alone the second Quad's
/// flicker is the Simulation's seed moved on by one step.
const SEED_STEP: u64 = 0x9e37_79b9_7f4a_7c15;

#[test]
fn two_quads_in_one_simulation_each_fly_exactly_as_they_would_alone() {
    // Basis: Rule (#58, and #37: Quads are stepped in a fixed order and don't
    // touch each other). Two pilots fly two different whoops (one 1 m up, one
    // 2 m) with different sticks, each through the whole of `pilot`: lost and
    // back, and Reset. Each Quad's whole state, its Flight Controller's
    // memory included, matches the same Quad alone at every step. The second
    // Quad alone gets the seed that gives it the flicker it has in the pair.
    let seed = 77_u64;
    let pilots = [pilot(0), pilot(5)];
    let quads = vec![eventful_quad(1.0), eventful_quad(2.0)];
    let mut pair = Simulation::new(over_a_floor(seed, quads)).unwrap();
    let mut first = Simulation::new(over_a_floor(seed, vec![eventful_quad(1.0)])).unwrap();
    let mut second = Simulation::new(over_a_floor(
        seed.wrapping_add(SEED_STEP),
        vec![eventful_quad(2.0)],
    ))
    .unwrap();
    assert_eq!(pair.quad_count(), 2);
    for tick in 0..6_000_u64 {
        fly(&mut pair, &pilots, tick + 1);
        fly(&mut first, &pilots[..1], tick + 1);
        fly(&mut second, &pilots[1..], tick + 1);
        assert_eq!(
            pair.quad_fingerprint(0),
            first.quad_fingerprint(0),
            "first Quad, step {tick}"
        );
        assert_eq!(
            pair.quad_fingerprint(1),
            second.quad_fingerprint(0),
            "second Quad, step {tick}"
        );
    }
    assert_eq!(pair.quad_output(0), first.quad_output(0));
    assert_eq!(pair.quad_output(1), second.quad_output(0));
    // The two flew differently, or the check would prove nothing.
    assert_ne!(pair.quad_fingerprint(0), pair.quad_fingerprint(1));
}

#[test]
fn a_copy_of_two_quads_brings_back_both() {
    // Basis: Rule (#58).
    let pilots = [pilot(0), pilot(5)];
    let quads = vec![eventful_quad(1.0), eventful_quad(2.0)];
    let mut sim = Simulation::new(over_a_floor(4, quads)).unwrap();
    fly(&mut sim, &pilots, 2_000);
    let copy = sim.snapshot();
    let first = fly(&mut sim, &pilots, 5_000);
    sim.restore(&copy);
    assert_eq!(fly(&mut sim, &pilots, 5_000), first);
}
