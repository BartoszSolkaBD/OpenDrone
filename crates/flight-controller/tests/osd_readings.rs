//! Readable checks for what the Flight Controller's output gains for the OSD
//! (ADR-0022): the battery as Betaflight reads it, the beeper, Link Quality
//! and the motors' RPM. Basis: Source (Betaflight 2026.6.2's
//! `sensors/battery.c`, `sensors/voltage.c`, `sensors/current.c`,
//! `io/beeper.c`, `rx/crsf.c` and `drivers/dshot.c`) unless said.

use opendrone_flight_controller::{
    BatteryReading, BatteryState, Channel, Channels, DebugRecord, FlightController, Rates,
    SensorReadings, Tune,
};
use opendrone_maths::{Attitude, Vec3};

const FREESTYLE_5_TUNE: &str = include_str!("../../../packs/opendrone/quads/freestyle-5/tune.txt");

/// The Freestyle 5″'s Tune, with these settings changed.
fn tune(changes: &[(&str, &str)]) -> Tune {
    let lines = FREESTYLE_5_TUNE.lines().filter_map(|line| {
        let rest = line.strip_prefix("set ")?;
        let setting = rest.split('#').next()?;
        let (name, value) = setting.split_once('=')?;
        let name = name.trim();
        let value = changes
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(value.trim(), |(_, v)| *v);
        Some((name, value))
    });
    Tune::read(lines).unwrap()
}

fn readings(volts: f64, amps: f64) -> SensorReadings {
    SensorReadings {
        battery: BatteryReading {
            voltage: volts,
            current: amps,
        },
        ..SensorReadings::still(Vec3::ZERO, Attitude::BODY_IS_WORLD, true)
    }
}

/// A fresh, disarmed Flight Controller at 8 kHz on a 250 Hz link.
fn fresh(tune: Tune) -> FlightController {
    let start = readings(0.0, 0.0);
    FlightController::new(tune, Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &start)
}

/// Runs `loops` loops with these readings and a resting frame every 32
/// loops; gives the last loop's record.
fn run(fc: &mut FlightController, loops: u32, readings: SensorReadings) -> DebugRecord {
    for k in 0..loops {
        let frame = (k % 32 == 0).then_some(&Channels::RESTING);
        fc.step(&readings, frame);
    }
    *fc.debug()
}

#[test]
fn a_full_6s_pack_reads_25_19_volts_six_cells_and_ok() {
    let mut fc = fresh(tune(&[]));
    let record = run(&mut fc, 8000, readings(25.2, 0.0));
    assert_eq!(record.battery.voltage, 2519);
    assert_eq!(record.battery.cell_count, 6);
    assert_eq!(record.battery.state, BatteryState::Ok);
    assert_eq!(record.battery.average_cell_voltage(), 419);
}

#[test]
fn a_full_high_voltage_1s_pack_reads_4_34_volts_one_cell() {
    // The board's ADC reads 4.35 V as 4.34 V, below the Meteor's
    // vbat_max_cell_voltage of 435, so one cell (4.34 ÷ 4.35, rounded down,
    // plus one).
    let mut fc = fresh(tune(&[("vbat_max_cell_voltage", "435")]));
    let record = run(&mut fc, 800, readings(4.35, 0.0));
    assert_eq!(record.battery.voltage, 434);
    assert_eq!(record.battery.cell_count, 1);
}

#[test]
fn a_flight_controller_alone_finds_no_battery() {
    let mut fc = fresh(tune(&[]));
    let record = run(&mut fc, 800, readings(0.0, 0.0));
    assert_eq!(record.battery.state, BatteryState::NotPresent);
    assert_eq!(record.battery.cell_count, 0);
}

#[test]
fn a_sag_below_3_5_volts_a_cell_turns_the_battery_low_once_the_low_pass_follows() {
    // Basis: Rule: the display low-pass (vbat_display_lpf_period 30: 1 ÷ 3 s
    // = 0.333 Hz, a time constant of 0.477 s) brings 25.2 V down towards
    // 20.4 V; it reaches 6 × 3.50 − 0.01 = 20.99 V after 0.477 × ln(4.8 ÷
    // 0.59) = 1.00 s, and the state moves at the next 5 Hz check.
    let mut fc = fresh(tune(&[]));
    run(&mut fc, 800, readings(25.2, 0.0));
    let record = run(&mut fc, 7200, readings(20.4, 0.0));
    assert_eq!(record.battery.state, BatteryState::Ok, "still OK 0.9 s on");
    let record = run(&mut fc, 3200, readings(20.4, 0.0));
    assert_eq!(
        record.battery.state,
        BatteryState::Warning,
        "low by 1.3 s on"
    );
    assert_eq!(
        record.beeper.mode.map(|m| m.name()),
        Some("BAT_LOW"),
        "the beeper sounds BAT_LOW"
    );
    // Back above 21.00 V: OK again.
    let record = run(&mut fc, 40_000, readings(25.0, 0.0));
    assert_eq!(record.battery.state, BatteryState::Ok);
}

#[test]
fn the_current_and_mah_follow_the_physics_where_the_current_meter_reads_one() {
    // Basis: Rule: 10 A for 1 s is 10 ÷ 3.6 = 2.78 mAh, counted down to 2.
    let mut fc = fresh(tune(&[]));
    let record = run(&mut fc, 8000, readings(25.2, 10.0));
    assert_eq!(record.battery.mah_drawn, Some(2));
    // The current's 1 Hz low-pass has nearly caught up after 1 s.
    let current = record.battery.current.unwrap();
    assert!((990..=1000).contains(&current), "{current}");
    // VIRTUAL isn't simulated yet: no current.
    let mut fc = fresh(tune(&[("current_meter", "VIRTUAL")]));
    let record = run(&mut fc, 8000, readings(25.2, 10.0));
    assert_eq!(
        (record.battery.current, record.battery.mah_drawn),
        (None, None)
    );
}

#[test]
fn a_fresh_flight_controller_beeps_gyro_calibrated_three_times() {
    // Basis: Rule (#32 §3: a fresh Flight Controller counts as calibrated at
    // once) and Source (beep_gyroCalibrated: 200 ms on, 100 ms off, three
    // times).
    let mut fc = fresh(tune(&[]));
    let record = run(&mut fc, 1, readings(25.2, 0.0));
    assert_eq!(
        record.beeper.mode.map(|m| m.name()),
        Some("GYRO_CALIBRATED")
    );
    assert!(record.beeper.on);
    assert_eq!(record.beeper.beeps, 1);
    let record = run(&mut fc, 8000, readings(25.2, 0.0));
    assert_eq!(record.beeper.beeps, 3);
    assert_eq!(record.beeper.mode, None);
}

#[test]
fn the_warning_beeps_tell_a_refused_arm_once_for_each_refusal() {
    let mut fc = fresh(tune(&[]));
    run(&mut fc, 8000, readings(25.2, 0.0));
    let before = fc.debug().beeper.beeps;
    let throttle_up = Channels {
        throttle: Channel::HIGH,
        arm: Channel::HIGH,
        ..Channels::RESTING
    };
    // Three seconds: the flashes, a pause, the long beep, a pause and the
    // short beeps take 2.25 s.
    for k in 0..24_000 {
        let frame = (k % 32 == 0).then_some(&throttle_up);
        fc.step(&readings(25.2, 0.0), frame);
    }
    // THROTTLE (bit 7): four flashes then one long and three short beeps.
    let record = *fc.debug();
    assert_eq!(record.arming_blocks.names(), ["THROTTLE", "ARM_SWITCH"]);
    assert_eq!(record.beeper.beeps - before, 4 + 1 + 3);
}

#[test]
fn link_quality_falls_to_0_250_ms_after_the_last_frame_and_reports_the_rf_mode() {
    let mut fc = fresh(tune(&[]));
    let record = run(&mut fc, 8000, readings(25.2, 0.0));
    assert_eq!((record.rf_mode, record.link_quality), (7, 100));
    // The last frame at 0.996 s; none after.
    for _ in 0..(8 * 250 - 32) {
        fc.step(&readings(25.2, 0.0), None);
    }
    assert_eq!(fc.debug().link_quality, 100);
    for _ in 0..16 {
        fc.step(&readings(25.2, 0.0), None);
    }
    assert_eq!(fc.debug().link_quality, 0);
}

#[test]
fn bidirectional_dshot_brings_each_motors_rpm_back_in_hundreds_of_erpm() {
    // 20,950 RPM on 14 poles is 146,650 eRPM, sent as 1466 hundreds, read
    // back as 1466 × 100 ÷ 7 = 20,942.9, rounded to 20,943.
    let mut fc = fresh(tune(&[("dshot_bidir", "ON")]));
    let spinning = SensorReadings {
        motor_rpm: [20_950.0, 0.0, 21_000.0, 7.0],
        ..readings(25.2, 0.0)
    };
    let record = run(&mut fc, 10, spinning);
    assert_eq!(record.motor_rpm, Some([20_943, 0, 21_000, 0]));
    let mut fc = fresh(tune(&[]));
    assert_eq!(run(&mut fc, 10, spinning).motor_rpm, None);
}
