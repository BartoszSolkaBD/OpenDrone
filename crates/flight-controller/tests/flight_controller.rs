//! Readable checks for the Flight Controller's edges: how Channels are
//! numbered, how a Tune is read, and what a fresh Flight Controller sends
//! and blocks. How it flies, arms and runs Failsafe is proved by the
//! Scenarios in `scenarios/flight-controller/`.

use opendrone_flight_controller::{
    Channel, Channels, FailsafeProcedure, FlightController, Rates, SensorReadings, Tune,
};
use opendrone_maths::{Attitude, Vec3};

/// The Freestyle 5″'s Tune: Betaflight 2026.6.2's defaults.
const DEFAULTS: &[(&str, &str)] = &[
    ("small_angle", "25"),
    ("yaw_spin_recovery", "AUTO"),
    ("yaw_spin_threshold", "1950"),
    ("runaway_takeoff_prevention", "ON"),
    ("runaway_takeoff_deactivate_delay", "500"),
    ("runaway_takeoff_deactivate_throttle_percent", "20"),
    ("rx_min_usec", "885"),
    ("rx_max_usec", "2115"),
    ("failsafe_delay", "15"),
    ("failsafe_procedure", "DROP"),
    ("failsafe_throttle", "1000"),
    ("failsafe_recovery_delay", "5"),
    ("p_roll", "45"),
    ("i_roll", "80"),
    ("d_roll", "30"),
    ("f_roll", "120"),
    ("p_pitch", "47"),
    ("i_pitch", "84"),
    ("d_pitch", "34"),
    ("f_pitch", "125"),
    ("p_yaw", "45"),
    ("i_yaw", "80"),
    ("d_yaw", "0"),
    ("f_yaw", "120"),
    ("feedforward_transition", "0"),
    ("feedforward_averaging", "2_POINT"),
    ("feedforward_smooth_factor", "65"),
    ("feedforward_jitter_factor", "7"),
    ("feedforward_boost", "15"),
    ("feedforward_max_rate_limit", "90"),
    ("motor_output_limit", "100"),
    ("pidsum_limit", "500"),
    ("pidsum_limit_yaw", "400"),
    ("iterm_windup", "80"),
    ("pid_at_min_throttle", "ON"),
    ("feedforward_yaw_hold_gain", "15"),
    ("feedforward_yaw_hold_time", "100"),
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
    ("motor_idle", "550"),
    ("yaw_motors_reversed", "OFF"),
    ("mixer_type", "LEGACY"),
    ("crashflip_motor_percent", "0"),
    ("crashflip_rate", "0"),
    ("crashflip_auto_rearm", "OFF"),
];

fn tune() -> Tune {
    Tune::read(DEFAULTS.iter().copied()).unwrap()
}

const STILL: SensorReadings = SensorReadings {
    gyro: Vec3::ZERO,
    attitude: Attitude::BODY_IS_WORLD,
    escs_ready: true,
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
fn settings_the_flight_controller_doesnt_know_are_left_alone() {
    let lines = DEFAULTS
        .iter()
        .copied()
        .chain([("motor_poles", "14"), ("vbat_max_cell_voltage", "435")]);
    assert_eq!(Tune::read(lines).unwrap(), tune());
    assert!(Tune::check("vbat_max_cell_voltage", "anything").is_ok());
}

#[test]
fn settings_not_simulated_yet_are_optional_but_checked_as_betaflight_checks_them() {
    // Basis: Rule (ADR-0015), with the ranges and words of Betaflight
    // 2026.6.2's src/main/cli/settings.c.
    let names: Vec<&str> = Tune::not_simulated_yet()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    for name in [
        "d_max_roll",
        "iterm_relax",
        "anti_gravity_gain",
        "tpa_low_rate",
    ] {
        assert!(names.contains(&name), "{name}");
    }
    // The filters, RC smoothing and feedforward are simulated now (#49).
    for name in ["f_roll", "yaw_lowpass_hz", "rc_smoothing"] {
        assert!(!names.contains(&name), "{name}");
        assert!(Tune::settings().contains(&name), "{name}");
    }
    // Set off, they read; the Flight Controller flies as now either way.
    let off = DEFAULTS.iter().copied().chain([
        ("anti_gravity_gain", "0"),
        ("tpa_low_rate", "0"),
        ("iterm_relax", "OFF"),
    ]);
    assert_eq!(Tune::read(off).unwrap(), tune());
    // A value Betaflight wouldn't take is refused.
    let wrong = DEFAULTS
        .iter()
        .copied()
        .chain([("anti_gravity_gain", "251"), ("iterm_relax", "SOME")]);
    let problems = Tune::read(wrong).unwrap_err();
    assert!(problems.missing.is_empty());
    assert_eq!(
        problems.wrong,
        [
            (
                "iterm_relax",
                "`iterm_relax` must be one of OFF, RP, RPY, RP_INC, RPY_INC, not \"SOME\""
                    .to_string()
            ),
            (
                "anti_gravity_gain",
                "`anti_gravity_gain` is a whole number from 0 to 250, not \"251\"".to_string()
            ),
        ]
    );
    assert!(Tune::check("dyn_notch_count", "8").is_err());
    assert!(Tune::check("dyn_notch_count", "0").is_ok());
}

#[test]
fn a_fresh_flight_controller_sends_dshot_stop_until_it_is_armed() {
    // Basis: Source (Betaflight's `DSHOT_CMD_MOTOR_STOP`, 0, while
    // disarmed).
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &STILL);
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
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, true, &STILL);
    let armed = Channels {
        arm: Channel::HIGH,
        ..Channels::RESTING
    };
    let motors = fc.step(&STILL, Some(&armed));
    assert!(fc.armed());
    assert_eq!(motors[0].dshot, 158);
}

#[test]
fn a_tune_set_to_auto_land_or_gps_rescue_flies_drop_and_says_it_isnt_simulated_yet() {
    // Basis: Rule (#21: DROP is the only procedure; LAND and GPS Rescue fly
    // DROP with a "not simulated yet" note). Betaflight 2026.6.2's words are
    // AUTO-LAND, DROP and GPS-RESCUE (`lookupTableFailsafe`).
    assert!(tune().not_simulated_values().is_empty());
    for (word, procedure) in [
        ("AUTO-LAND", FailsafeProcedure::AutoLand),
        ("GPS-RESCUE", FailsafeProcedure::GpsRescue),
    ] {
        let lines = DEFAULTS.iter().map(|(name, value)| match *name {
            "failsafe_procedure" => (*name, word),
            _ => (*name, *value),
        });
        let read = Tune::read(lines).unwrap();
        assert_eq!(read.failsafe_procedure, procedure);
        assert_eq!(
            read.not_simulated_values(),
            [(
                "failsafe_procedure",
                format!(
                    "`failsafe_procedure` is {word}, which isn't simulated yet: the Quad flies DROP, disarming as Failsafe's stage 2 starts"
                )
            )]
        );
    }
    let lines = DEFAULTS.iter().map(|(name, value)| match *name {
        "failsafe_procedure" => (*name, "LAND"),
        _ => (*name, *value),
    });
    assert_eq!(
        Tune::read(lines).unwrap_err().wrong,
        [(
            "failsafe_procedure",
            "`failsafe_procedure` must be one of AUTO-LAND, DROP, GPS-RESCUE, not \"LAND\""
                .to_string()
        )]
    );
}

#[test]
fn a_tune_set_to_auto_land_disarms_as_drop_does_when_the_link_goes() {
    // Basis: Rule (#21: AUTO-LAND flies DROP) and Source (failsafe.c: DROP
    // disarms once more than failsafe_delay, 1.5 s, has passed since the last
    // good frame, at an 11 ms check).
    for word in ["DROP", "AUTO-LAND", "GPS-RESCUE"] {
        let lines = DEFAULTS.iter().map(|(name, value)| match *name {
            "failsafe_procedure" => (*name, word),
            _ => (*name, *value),
        });
        let tune = Tune::read(lines).unwrap();
        let mut fc =
            FlightController::new(tune, Rates::BETAFLIGHT_DEFAULT, 8000, 250, true, &STILL);
        // Frames for 1 s (the last at 0.996 s), then none.
        frames(&mut fc, arm(true), STILL, 8000);
        for _ in 0..(8000 * 3 / 2 - 100) {
            fc.step(&STILL, None);
        }
        assert!(fc.armed(), "{word}: still armed at 2.4875 s");
        for _ in 0..200 {
            fc.step(&STILL, None);
        }
        // Dropped at the check at 2.497 s.
        assert!(!fc.armed(), "{word}: dropped by 2.5125 s");
        assert!(fc.debug().arming_blocks.failsafe);
    }
}

/// Sticks centred, throttle low, the Arm switch as given.
fn arm(on: bool) -> Channels {
    Channels {
        arm: Channel::from_switch(on),
        ..Channels::RESTING
    }
}

/// Runs `loops` loops with these Channels in a frame every 32 loops (250 Hz
/// at 8 kHz), the first in the first loop.
fn frames(fc: &mut FlightController, channels: Channels, readings: SensorReadings, loops: u32) {
    for k in 0..loops {
        let frame = (k % 32 == 0).then_some(&channels);
        fc.step(&readings, frame);
    }
}

#[test]
fn a_fresh_flight_controller_names_bootgrace_until_the_escs_are_ready() {
    // Basis: Rule (#32 §4: BOOTGRACE holds until the ESCs' ready beep, in
    // place of Betaflight's 5 s) and Source (Betaflight 2026.6.2's
    // `updateArmingStatus`: BOOTGRACE clears once its wait is over, and an
    // Arm switch on while any flag stands raises ARM_SWITCH, which clears
    // only with the switch off).
    let powering_up = SensorReadings {
        escs_ready: false,
        ..STILL
    };
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &STILL);
    assert_eq!(fc.debug().arming_blocks.names(), ["BOOTGRACE"]);
    // The Arm switch already on at power-up: refused.
    frames(&mut fc, arm(true), powering_up, 320);
    assert!(!fc.armed());
    assert_eq!(
        fc.debug().arming_blocks.names(),
        ["BOOTGRACE", "ARM_SWITCH"]
    );
    // The ESCs are ready: BOOTGRACE clears, but the switch must go off first.
    frames(&mut fc, arm(true), STILL, 320);
    assert!(!fc.armed());
    assert_eq!(fc.debug().arming_blocks.names(), ["ARM_SWITCH"]);
    frames(&mut fc, arm(false), STILL, 320);
    assert!(fc.debug().arming_blocks.names().is_empty());
    frames(&mut fc, arm(true), STILL, 320);
    assert!(fc.armed());
}

#[test]
fn a_fresh_flight_controller_counts_the_link_as_settled_and_watches_it_at_once() {
    // Basis: Rule (#21: power-up skips Betaflight's own waits, the link
    // settled) and Source (`rxFrameCheck`: no frame for 150 ms is RXLOSS).
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &STILL);
    frames(&mut fc, arm(false), STILL, 32);
    let record = *fc.debug();
    assert!(record.arming_blocks.names().is_empty(), "{record:?}");
    assert!(record.failsafe.signal && record.failsafe.link_up);
    // No frame at all: 150 ms on, RXLOSS. 1201 loops are 150.125 ms.
    let mut silent =
        FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &STILL);
    for _ in 0..1201 {
        silent.step(&STILL, None);
    }
    assert!(!silent.debug().arming_blocks.rx_loss);
    assert!(silent.debug().failsafe.signal);
    silent.step(&STILL, None);
    // The Channels are worked out again, so the arming checks run too, and
    // BOOTGRACE clears: the ESCs are ready.
    assert_eq!(silent.debug().arming_blocks.names(), ["RXLOSS"]);
    assert!(!silent.debug().failsafe.signal);
}

/// The Tune with one setting changed.
fn tune_with(name: &str, value: &str) -> Tune {
    let lines = DEFAULTS
        .iter()
        .map(|(n, v)| if *n == name { (*n, value) } else { (*n, *v) });
    Tune::read(lines).unwrap()
}

#[test]
fn yaw_spin_recovery_on_auto_starts_200_degrees_a_second_past_the_max_yaw_rate() {
    // Basis: Source (Betaflight 2026.6.2's `initYawSpinRecovery`,
    // src/main/sensors/gyro.c: AUTO adds a quarter of the max yaw rate, or
    // 200 °/s if that's more). Betaflight's default Rates reach 670 °/s, a
    // quarter of which is 167, so 870 °/s (#26 §4).
    let fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, true, &STILL);
    assert_eq!(fc.yaw_spin_threshold(), Some(870));
}

#[test]
fn yaw_spin_recovery_on_auto_adds_a_quarter_of_a_fast_max_yaw_rate_up_to_1950() {
    // Basis: Source (`initYawSpinRecovery`: the threshold is held within
    // 500–1950 °/s, YAW_SPIN_RECOVERY_THRESHOLD_MIN and _MAX). Actual Rates
    // of 1000 °/s on yaw add 250: 1250 °/s. 1800 °/s would give 2250, held at
    // 1950.
    let mut rates = Rates::BETAFLIGHT_DEFAULT;
    rates.yaw.srate = 100;
    let fc = FlightController::new(tune(), rates.clone(), 8000, 250, true, &STILL);
    assert_eq!(fc.yaw_spin_threshold(), Some(1250));
    rates.yaw.srate = 180;
    let fc = FlightController::new(tune(), rates, 8000, 250, true, &STILL);
    assert_eq!(fc.yaw_spin_threshold(), Some(1950));
}

#[test]
fn yaw_spin_recovery_on_starts_at_the_tunes_threshold_and_off_never_starts() {
    // Basis: Source (`initYawSpinRecovery`: ON takes yaw_spin_threshold; OFF
    // turns it off).
    let on = FlightController::new(
        tune_with("yaw_spin_recovery", "ON"),
        Rates::BETAFLIGHT_DEFAULT,
        8000,
        250,
        true,
        &STILL,
    );
    assert_eq!(on.yaw_spin_threshold(), Some(1950));
    let off = FlightController::new(
        tune_with("yaw_spin_recovery", "OFF"),
        Rates::BETAFLIGHT_DEFAULT,
        8000,
        250,
        true,
        &STILL,
    );
    assert_eq!(off.yaw_spin_threshold(), None);
}

#[test]
fn a_fresh_flight_controller_has_detected_the_packet_rate_and_tuned_rc_smoothing_to_it() {
    // Basis: Rule (#37: power-up leaves the link-rate estimate settled) and
    // Source (Betaflight 2026.6.2's `rcSmoothingSetFilterCutoffs`: the
    // automatic cutoff is the rate × 1.5 ÷ (1 + 30 ÷ 10), never below 15 Hz,
    // kept in whole Hz: 93.75 Hz is 93 Hz at 250 Hz, 18.75 Hz is 18 Hz at
    // 50 Hz).
    for (packet_rate, cutoff) in [(50, 18), (250, 93), (500, 187), (1000, 375)] {
        let fc = FlightController::new(
            tune(),
            Rates::BETAFLIGHT_DEFAULT,
            8000,
            packet_rate,
            false,
            &STILL,
        );
        assert_eq!(fc.debug().detected_packet_rate, f64::from(packet_rate));
        assert_eq!(fc.debug().rc_smoothing_cutoffs, Some([cutoff, cutoff]));
    }
    // With RC smoothing off it has no cutoffs.
    let off = FlightController::new(
        tune_with("rc_smoothing", "OFF"),
        Rates::BETAFLIGHT_DEFAULT,
        8000,
        250,
        false,
        &STILL,
    );
    assert_eq!(off.debug().rc_smoothing_cutoffs, None);
}

/// Runs a fresh Flight Controller, settled at 250 Hz, for `loops` loops,
/// with a frame at each loop `frame_at` picks; gives the detected Packet
/// Rate and the setpoints' RC smoothing cutoff after each frame.
fn detected_after_each_frame(loops: u32, frame_at: impl Fn(u32) -> bool) -> Vec<(f64, u16)> {
    let mut fc = FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &STILL);
    let channels = arm(false);
    let mut seen = Vec::new();
    for k in 0..loops {
        let frame = frame_at(k).then_some(&channels);
        fc.step(&STILL, frame);
        if frame.is_some() {
            let record = fc.debug();
            seen.push((
                record.detected_packet_rate,
                record.rc_smoothing_cutoffs.unwrap()[0],
            ));
        }
    }
    seen
}

#[test]
fn frames_twice_as_often_snap_the_detected_packet_rate_after_three_and_retune_three_frames_later() {
    // Basis: Source (Betaflight 2026.6.2's `updateRcRefreshRate` and
    // `shouldUpdateSmoothing`). Settled at 250 Hz, frames come every 2 ms
    // (500 Hz), the first at 0 ms. The first's gap, 0 ms, is held at 0.8 ms
    // and doesn't count. The next three are each more than 20% faster, on
    // the same side: the third snaps the detected Packet Rate to 500 Hz.
    // Three more frames at 500 Hz count, and the third retunes RC smoothing:
    // 500 × 1.5 ÷ 4 = 187.5 Hz, kept as 187 Hz.
    let seen = detected_after_each_frame(16 * 8, |k| k % 16 == 0);
    assert_eq!(
        seen,
        [
            (250.0, 93),
            (250.0, 93),
            (250.0, 93),
            (500.0, 93),
            (500.0, 93),
            (500.0, 93),
            (500.0, 187),
            (500.0, 187),
        ]
    );
}

#[test]
fn frames_close_to_the_detected_packet_rate_move_it_a_tenth_of_the_way() {
    // Basis: Source (`shouldUpdateSmoothing`: a rate within 20% moves the
    // smoothed rate 10% of the way). Frames every 3.5 ms (285.714 Hz, 14%
    // faster than 250 Hz), the first at 0 ms, which doesn't count.
    let seen = detected_after_each_frame(28 * 4, |k| k % 28 == 0);
    let rate = 1e6 / 3500.0;
    let first = 250.0 + 0.1 * (rate - 250.0);
    let second = first + 0.1 * (rate - first);
    let third = second + 0.1 * (rate - second);
    assert_eq!(seen[0].0, 250.0);
    assert!((seen[1].0 - first).abs() < 1e-9, "{seen:?}");
    assert!((seen[2].0 - second).abs() < 1e-9, "{seen:?}");
    assert!((seen[3].0 - third).abs() < 1e-9, "{seen:?}");
    // The third that counted retunes RC smoothing: 259.7 Hz × 0.375 =
    // 97.4 Hz, kept as 97 Hz.
    assert_eq!(
        seen.iter().map(|(_, cutoff)| *cutoff).collect::<Vec<_>>(),
        [93, 93, 93, 97]
    );
}

#[test]
fn frames_off_on_alternate_sides_are_jitter_and_leave_the_detected_packet_rate_alone() {
    // Basis: Source (`shouldUpdateSmoothing`: an outlier on the other side
    // starts the count again, as often happens at 1 kHz). Settled at 250 Hz,
    // gaps of 1 ms and 7 ms by turns: 1,000 Hz and 143 Hz, each more than
    // 20% off, never three in a row on one side.
    let at = [0, 8, 64, 72, 128, 136, 192, 200, 256, 264];
    let seen = detected_after_each_frame(300, |k| at.contains(&k));
    assert_eq!(seen.len(), at.len());
    assert!(
        seen.iter()
            .all(|(rate, cutoff)| *rate == 250.0 && *cutoff == 93)
    );
}
