//! Readable checks for the Flight Controller's edges: how Channels are
//! numbered, how a Tune is read, and what a fresh Flight Controller sends.
//! How it flies is proved by the Scenarios in `scenarios/flight-controller/`.

use opendrone_flight_controller::{
    Channel, Channels, FlightController, Rates, SensorReadings, Tune,
};
use opendrone_maths::{Attitude, Vec3};

/// The Freestyle 5″'s Tune: Betaflight 2026.6.2's defaults.
const DEFAULTS: &[(&str, &str)] = &[
    ("small_angle", "25"),
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
];

fn tune() -> Tune {
    Tune::read(DEFAULTS.iter().copied()).unwrap()
}

const STILL: SensorReadings = SensorReadings {
    gyro: Vec3::ZERO,
    attitude: Attitude::BODY_IS_WORLD,
};

#[test]
fn elrs_full_stick_and_centre_are_crsf_steps_172_992_and_1811() {
    // Basis: Source (ExpressLRS puts ±100% and centre at 988, 1500 and
    // 2012 µs, CRSF 172, 992 and 1811).
    assert_eq!(Channel::from_stick(-1.0), Channel::LOW);
    assert_eq!(Channel::from_stick(0.0), Channel::CENTRE);
    assert_eq!(Channel::from_stick(1.0), Channel::HIGH);
    assert_eq!(Channel::from_throttle(0.0), Channel::LOW);
    assert_eq!(Channel::from_throttle(1.0), Channel::HIGH);
    assert_eq!(Channel::from_micros(1500.0).step(), 992);
}

#[test]
fn betaflight_reads_crsf_steps_172_992_and_1811_as_988_46_1500_77_and_2012_46_us() {
    // Basis: Source (Betaflight 2026.6.2, `crsfReadRawRC`: 0.62477120195241 ×
    // step + 881).
    let close = |channel: Channel, us: f64| (channel.micros() - us).abs() < 0.005;
    assert!(close(Channel::LOW, 988.46));
    assert!(close(Channel::CENTRE, 1500.77));
    assert!(close(Channel::HIGH, 2012.46));
}

#[test]
fn a_stick_in_percent_becomes_the_nearest_whole_crsf_step() {
    // Basis: Rule. 50% is 1500 + 256 µs = 1756 µs, which is 172 + 768 ×
    // 1639 / 1024 = 1401.3 steps: step 1401. -50% is 1244 µs: 581.75, so 582.
    assert_eq!(Channel::from_stick(0.5).step(), 1401);
    assert_eq!(Channel::from_stick(-0.5).step(), 582);
    assert_eq!(Channel::from_throttle(0.5).step(), 992);
}

#[test]
fn a_tune_must_spell_out_every_setting_the_flight_controller_reads() {
    // Basis: Rule (ADR-0015).
    let partial = DEFAULTS
        .iter()
        .copied()
        .filter(|(name, _)| !matches!(*name, "p_roll" | "motor_idle"));
    let problems = Tune::read(partial).unwrap_err();
    assert_eq!(problems.missing, ["p_roll", "motor_idle"]);
    assert!(problems.wrong.is_empty());
}

#[test]
fn a_tune_value_the_flight_controller_cant_read_is_refused_with_a_sentence() {
    // Basis: Rule (the ranges and words of Betaflight 2026.6.2's
    // src/main/cli/settings.c).
    let lines = DEFAULTS.iter().map(|(name, value)| match *name {
        "small_angle" => (*name, "181"),
        "pid_at_min_throttle" => (*name, "YES"),
        "motor_pwm_protocol" => (*name, "PWM"),
        _ => (*name, *value),
    });
    let problems = Tune::read(lines).unwrap_err();
    assert_eq!(
        problems.wrong,
        [
            (
                "small_angle",
                "`small_angle` is a whole number from 0 to 180, not \"181\"".to_string()
            ),
            (
                "pid_at_min_throttle",
                "`pid_at_min_throttle` is OFF or ON, not \"YES\"".to_string()
            ),
            (
                "motor_pwm_protocol",
                "`motor_pwm_protocol` is PWM, but the simulated ESCs run Bluejay, which reads DShot: DSHOT150, DSHOT300 or DSHOT600".to_string()
            ),
        ]
    );
}

#[test]
fn settings_the_flight_controller_doesnt_read_yet_are_left_alone() {
    let lines = DEFAULTS
        .iter()
        .copied()
        .chain([("motor_poles", "14"), ("tpa_rate", "65")]);
    assert_eq!(Tune::read(lines).unwrap(), tune());
    assert!(Tune::check("tpa_rate", "anything").is_ok());
}

#[test]
fn a_fresh_flight_controller_sends_dshot_stop_until_it_is_armed() {
    // Basis: Source (Betaflight's `DSHOT_CMD_MOTOR_STOP`, 0, while
    // disarmed).
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, false, &STILL);
    let full_throttle = Channels {
        throttle: Channel::HIGH,
        ..Channels::RESTING
    };
    for _ in 0..100 {
        let motors = fc.step(&STILL, Some(&full_throttle));
        assert!(motors.iter().all(|m| m.dshot == 0));
    }
    assert!(!fc.armed());
}

#[test]
fn a_flight_controller_started_armed_idles_at_zero_throttle() {
    // Basis: Source (idle 48 + 5.5% × 1999 = 157.9, sent as 158).
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, true, &STILL);
    let armed = Channels {
        arm: Channel::HIGH,
        ..Channels::RESTING
    };
    let motors = fc.step(&STILL, Some(&armed));
    assert!(fc.armed());
    assert_eq!(motors[0].dshot, 158);
}
