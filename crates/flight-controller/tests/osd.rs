//! Readable checks for Betaflight's OSD logic, worked out beside the
//! Simulation (ADR-0022, #31): fed Flight Controller loops, it hands out the
//! screen. The Scenarios in `scenarios/osd/` check it against real flights;
//! these check its edges with loops written by hand. Basis: Source
//! (Betaflight 2026.6.2's `osd/osd.c`, `osd/osd_elements.c` and
//! `osd/osd_warnings.c`) unless said.

use opendrone_flight_controller::osd::{Osd, OsdElement, OsdLayout, Page, Warning};
use opendrone_flight_controller::{
    ArmingBlocks, BatteryReadings, BatteryState, DebugRecord, FlightController, FlightMode, Rates,
    SensorReadings, Tune,
};
use opendrone_maths::{Attitude, Vec3};
use std::sync::OnceLock;

const FREESTYLE_5_TUNE: &str = include_str!("../../../packs/opendrone/quads/freestyle-5/tune.txt");

/// The Freestyle 5″'s Tune, read from its `set` lines.
fn tune() -> Tune {
    let lines = FREESTYLE_5_TUNE.lines().filter_map(|line| {
        let rest = line.strip_prefix("set ")?;
        let setting = rest.split('#').next()?;
        let (name, value) = setting.split_once('=')?;
        Some((name.trim(), value.trim()))
    });
    Tune::read(lines).unwrap()
}

/// A fresh Flight Controller's record, disarmed, with a full 6S pack
/// (25.20 V) found.
fn record() -> DebugRecord {
    static RECORD: OnceLock<DebugRecord> = OnceLock::new();
    *RECORD.get_or_init(fresh_record)
}

fn fresh_record() -> DebugRecord {
    let still = SensorReadings::still(Vec3::ZERO, Attitude::BODY_IS_WORLD, true);
    let mut record =
        *FlightController::new(tune(), Rates::BETAFLIGHT_DEFAULT, 8000, 250, false, &still).debug();
    record.arming_blocks = ArmingBlocks::default();
    record.battery = BatteryReadings {
        voltage: 2520,
        cell_count: 6,
        state: BatteryState::Ok,
        current: Some(0),
        mah_drawn: Some(0),
    };
    record
}

/// The OSD with every element shown, for a Quad named "Freestyle 5″".
fn osd() -> Osd {
    let mut layout = OsdLayout::opendrone_default();
    layout.show_everything();
    Osd::new(layout, "Freestyle 5″", &tune())
}

/// Feeds the OSD a loop every 125 µs from `from` up to and including `to`,
/// each from `at`.
fn run(osd: &mut Osd, from: u64, to: u64, at: impl Fn(u64) -> DebugRecord) {
    let mut now = from;
    while now <= to {
        let mut record = at(now);
        record.since_power_up_us = now;
        osd.see(&record);
        now += 125;
    }
}

#[test]
fn it_redraws_12_times_a_second_on_the_flight_controllers_clock_and_holds_between() {
    let mut osd = osd();
    run(&mut osd, 0, 0, |_| record());
    assert_eq!(osd.screen().reads(OsdElement::Throttle), "0");
    // 42% throttle from 1 ms: not shown until the redraw at 83,333 µs.
    let throttle = |now: u64| DebugRecord {
        throttle_percent: if now >= 1000 { 42 } else { 0 },
        ..record()
    };
    run(&mut osd, 125, 83_250, throttle);
    assert_eq!(osd.screen().reads(OsdElement::Throttle), "0");
    run(&mut osd, 83_375, 83_375, throttle);
    assert_eq!(osd.screen().reads(OsdElement::Throttle), "42");
}

#[test]
fn fail_safe_blinks_at_2_hz_hidden_in_the_first_quarter_second_of_each_half() {
    let mut osd = osd();
    let failsafe = |_| {
        let mut r = record();
        r.failsafe.active = true;
        r
    };
    // The redraw at 250,000 µs: past the first 250 ms of its half second,
    // so drawn, and blinking from then on.
    run(&mut osd, 0, 250_000, failsafe);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "FAIL SAFE");
    assert!(osd.screen().shows(OsdElement::Warnings));
    assert!(osd.screen().blinks(OsdElement::Warnings));
    assert_eq!(osd.screen().reads(OsdElement::FlightMode), "!FS!");
    // The redraw at 583,333 µs (83,333 into its half second) hides it, but
    // it still reads FAIL SAFE.
    run(&mut osd, 250_125, 583_375, failsafe);
    assert!(!osd.screen().shows(OsdElement::Warnings));
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "FAIL SAFE");
    // At 833,333 µs (333,333 into its half second) it shows again.
    run(&mut osd, 583_500, 833_375, failsafe);
    assert!(osd.screen().shows(OsdElement::Warnings));
}

#[test]
fn armed_shows_alone_for_half_a_second_at_arming_with_crash_flip_under_it() {
    let mut osd = osd();
    run(&mut osd, 0, 100_000, |_| record());
    let armed = |_| DebugRecord {
        armed: true,
        crash_flip: true,
        ..record()
    };
    // Armed from 100 ms: the redraw at 166,666 µs shows ARMED until 666,666.
    run(&mut osd, 100_125, 600_000, armed);
    assert_eq!(osd.screen().page, Page::Armed);
    assert_eq!(osd.screen().message(), "ARMED / >CRASH FLIP<");
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "");
    assert_eq!(osd.screen().picture()[6], "             ARMED            ");
    run(&mut osd, 600_125, 700_000, armed);
    assert_eq!(osd.screen().page, Page::Elements);
    assert_eq!(osd.screen().message(), "");
}

#[test]
fn a_start_already_armed_shows_no_armed() {
    // Basis: Rule (a mid-air start isn't an arming).
    let mut osd = osd();
    run(&mut osd, 0, 100_000, |_| DebugRecord {
        armed: true,
        ..record()
    });
    assert_eq!(osd.screen().page, Page::Elements);
}

#[test]
fn the_reasons_arming_is_refused_cycle_every_half_second_the_arm_switch_only_alone() {
    let mut osd = osd();
    let refused = |_| DebugRecord {
        arm_switch: true,
        arming_blocks: ArmingBlocks {
            throttle: true,
            boot_grace: true,
            arm_switch: true,
            ..ArmingBlocks::default()
        },
        ..record()
    };
    run(&mut osd, 0, 0, refused);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "THROTTLE");
    // The redraws to 500 ms keep it; the one at 583,333 µs moves on.
    run(&mut osd, 125, 500_000, refused);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "THROTTLE");
    run(&mut osd, 500_125, 583_375, refused);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "BOOTGRACE");
    run(&mut osd, 583_500, 1_166_750, refused);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "THROTTLE");
    // Alone, the Arm switch's own reason shows.
    let alone = |_| DebugRecord {
        arm_switch: true,
        arming_blocks: ArmingBlocks {
            arm_switch: true,
            ..ArmingBlocks::default()
        },
        ..record()
    };
    run(&mut osd, 1_166_875, 1_250_000, alone);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "ARM_SWITCH");
    // With the switch off, no reason shows.
    run(&mut osd, 1_250_125, 1_333_375, |_| DebugRecord {
        arming_blocks: ArmingBlocks {
            throttle: true,
            ..ArmingBlocks::default()
        },
        ..record()
    });
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "");
}

#[test]
fn the_warnings_come_in_betaflights_order_first_match_wins() {
    let mut osd = osd();
    // Crash Flip's switch on while disarmed, and a low battery: CRASHFLIP SW
    // comes first.
    let both = |_| DebugRecord {
        crash_flip_switch: true,
        battery: BatteryReadings {
            state: BatteryState::Warning,
            ..record().battery
        },
        ..record()
    };
    run(&mut osd, 0, 0, both);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "CRASHFLIP SW");
    assert!(!osd.screen().blinks(OsdElement::Warnings));
    let low = |_| DebugRecord {
        battery: BatteryReadings {
            state: BatteryState::Warning,
            ..record().battery
        },
        ..record()
    };
    run(&mut osd, 125, 83_375, low);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "LOW BATTERY");
    assert!(osd.screen().blinks(OsdElement::Warnings));
    let critical = |_| DebugRecord {
        battery: BatteryReadings {
            state: BatteryState::Critical,
            ..record().battery
        },
        ..record()
    };
    // LOW BATTERY blinks, so the redraw at 166,666 µs, which falls in the
    // first 250 ms of its half second, leaves it hidden as it was; the one
    // at 250,000 µs draws the new warning.
    run(&mut osd, 83_500, 166_750, critical);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "LOW BATTERY");
    run(&mut osd, 166_875, 250_000, critical);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "LAND NOW");
    // Never armed since power-up, cells below 4.10 V: BATT < FULL.
    let tired = |_| DebugRecord {
        battery: BatteryReadings {
            voltage: 2400,
            ..record().battery
        },
        ..record()
    };
    run(&mut osd, 250_125, 333_375, tired);
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "BATT < FULL");
}

#[test]
fn a_warning_switched_off_in_the_layout_never_shows() {
    let mut layout = OsdLayout::opendrone_default();
    layout.warnings &= !Warning::FailSafe.bit();
    let mut osd = Osd::new(layout, "Freestyle 5″", &tune());
    run(&mut osd, 0, 250_000, |_| {
        let mut r = record();
        r.failsafe.active = true;
        r
    });
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "");
}

#[test]
fn the_visual_beeper_flashes_once_for_each_beep_that_starts() {
    let mut osd = osd();
    let beeps = |count: u32| {
        move |_| {
            let mut r = record();
            r.beeper.beeps = count;
            r
        }
    };
    // A beep started before the first redraw: shown once, then gone.
    run(&mut osd, 0, 0, beeps(1));
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "* * * *");
    run(&mut osd, 125, 83_375, beeps(1));
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "");
    // Two more start between redraws: one flash.
    run(&mut osd, 83_500, 166_750, beeps(3));
    assert_eq!(osd.screen().reads(OsdElement::Warnings), "* * * *");
}

#[test]
fn the_statistics_page_shows_on_disarming_and_throttle_up_dismisses_it() {
    let mut osd = osd();
    run(&mut osd, 0, 0, |_| record());
    let armed = |_| DebugRecord {
        armed: true,
        battery: BatteryReadings {
            voltage: 2380,
            current: Some(1234),
            mah_drawn: Some(56),
            ..record().battery
        },
        ..record()
    };
    run(&mut osd, 125, 5_000_000, armed);
    run(&mut osd, 5_000_125, 5_100_000, |_| record());
    let screen = osd.screen();
    assert_eq!(screen.page, Page::Statistics);
    // Each redraw falls on the first loop at or after its moment, so the
    // 60th, due at 4,999,980 µs, comes at 5 s, still armed: armed from the
    // redraw at 0 s on, 5.0 s in all.
    assert_eq!(screen.statistic("TOTAL ARM").as_deref(), Some("00:05"));
    assert_eq!(screen.statistic("MIN BATTERY").as_deref(), Some("23.80V"));
    assert_eq!(screen.statistic("MAX CURRENT").as_deref(), Some("12.34A"));
    assert_eq!(screen.statistic("USED MAH").as_deref(), Some("0mAh"));
    // No GPS, no RSSI and no Blackbox log: those don't show (#31 §4).
    assert_eq!(screen.statistic("MAX SPEED"), None);
    assert_eq!(screen.statistic("MIN RSSI"), None);
    assert_eq!(screen.statistic("BB LOG NUM"), None);
    assert_eq!(screen.picture()[4], "         --- STATS ---        ");
    // Throttle above 1750 µs dismisses it at the next redraw, and the one
    // after brings the elements back.
    let throttle_up = |_| {
        let mut r = record();
        r.sticks_us[3] = 1800.0;
        r
    };
    run(&mut osd, 5_100_125, 5_250_000, throttle_up);
    assert_eq!(osd.screen().page, Page::Elements);
}

#[test]
fn the_statistics_page_never_shows_after_a_failsafe() {
    let mut osd = osd();
    run(&mut osd, 0, 1_000_000, |_| DebugRecord {
        armed: true,
        ..record()
    });
    run(&mut osd, 1_000_125, 1_200_000, |_| {
        let mut r = record();
        r.failsafe.active = true;
        r
    });
    assert_eq!(osd.screen().page, Page::Elements);
}

#[test]
fn timers_count_the_flight_controllers_clock_and_start_again_at_power_up() {
    // Basis: Rule (ADR-0022: timers count Simulation Time and start from
    // zero at power-up).
    let mut osd = osd();
    run(&mut osd, 0, 83_000_000, |now| DebugRecord {
        armed: now >= 60_000_000,
        ..record()
    });
    // On for 83 s (the redraw at 82.999 s), armed from the redraw at 60 s.
    assert_eq!(osd.screen().reads(OsdElement::Timer1), "01:22");
    assert_eq!(osd.screen().reads(OsdElement::Timer2), "00:22");
    // Power-up: the clock starts again from zero.
    run(&mut osd, 0, 900_000, |_| record());
    assert_eq!(osd.screen().reads(OsdElement::Timer1), "00:00");
    assert_eq!(osd.screen().reads(OsdElement::Timer2), "00:00");
}

#[test]
fn elements_with_no_source_read_blank() {
    // Basis: Rule (#31 §3).
    let mut osd = osd();
    run(&mut osd, 0, 0, |_| DebugRecord {
        battery: BatteryReadings {
            current: None,
            mah_drawn: None,
            ..record().battery
        },
        ..record()
    });
    let screen = osd.screen();
    for element in [
        OsdElement::Rssi,
        OsdElement::RssiDbm,
        OsdElement::Rsnr,
        OsdElement::Current,
        OsdElement::MahDrawn,
        OsdElement::MotorRpm,
        OsdElement::LogStatus,
    ] {
        assert_eq!(screen.reads(element), "", "{element:?}");
    }
    assert_eq!(screen.reads(OsdElement::BatteryVoltage), "25.2V");
    assert_eq!(screen.reads(OsdElement::AverageCellVoltage), "4.20V");
    assert_eq!(screen.reads(OsdElement::LinkQuality), "7:100");
    assert_eq!(screen.reads(OsdElement::CraftName), "FREESTYLE 5\"");
    assert_eq!(screen.reads(OsdElement::Disarmed), "DISARMED");
    assert_eq!(screen.reads(OsdElement::FlightMode), "AIR");
}

#[test]
fn angle_and_horizon_read_angl_and_hor() {
    for (mode, text) in [(FlightMode::Angle, "ANGL"), (FlightMode::Horizon, "HOR")] {
        let mut osd = osd();
        run(&mut osd, 0, 0, |_| DebugRecord {
            flight_mode: mode,
            ..record()
        });
        assert_eq!(osd.screen().reads(OsdElement::FlightMode), text);
    }
}

#[test]
fn the_motor_rpm_shows_each_motor_and_the_log_status_its_log_number() {
    let mut osd = osd();
    osd.set_blackbox_log(Some(12));
    run(&mut osd, 0, 0, |_| DebugRecord {
        motor_rpm: Some([21_000, 21_100, 20_900, 21_050]),
        ..record()
    });
    let screen = osd.screen();
    assert_eq!(
        screen.reads(OsdElement::MotorRpm),
        "21000 / 21100 / 20900 / 21050"
    );
    assert_eq!(screen.reads(OsdElement::LogStatus), "12");
}

#[test]
fn link_quality_blinks_below_its_alarm_and_the_low_battery_voltage_blinks() {
    let mut osd = osd();
    run(&mut osd, 0, 250_000, |_| DebugRecord {
        link_quality: 0,
        battery: BatteryReadings {
            state: BatteryState::Warning,
            ..record().battery
        },
        ..record()
    });
    let screen = osd.screen();
    assert_eq!(screen.reads(OsdElement::LinkQuality), "7: 0");
    assert!(screen.blinks(OsdElement::LinkQuality));
    assert!(screen.blinks(OsdElement::AverageCellVoltage));
}
