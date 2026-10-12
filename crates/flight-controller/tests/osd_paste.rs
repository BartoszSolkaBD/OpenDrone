//! Readable checks for the OSD paste (#31 §7): a pilot's `osd_*` lines from
//! the Betaflight CLI become their OSD layout. They read the maintainer's
//! real exports in `docs/research/quad-settings/`: the Meteor65 Pro's
//! (Betaflight 4.3.0) and the Cetus X's (4.4.0). Basis: Source (those
//! exports, and Betaflight 4.3.0's, 4.4.0's and 2026.6.2's `osd.h`,
//! `osd.c` and `cli/settings.c`) unless said.

use opendrone_flight_controller::cli::OsdPaste;
use opendrone_flight_controller::osd::{
    DEFAULT_STATISTICS, DEFAULT_WARNINGS, OsdElement, OsdLayout, Placement, Statistic,
    TimerPrecision, TimerSource, Warning,
};

const METEOR: &str = include_str!("../../../docs/research/quad-settings/meteor65-pro.diff-all.txt");
const CETUS: &str = include_str!("../../../docs/research/quad-settings/cetus-x.diff-all.txt");

fn paste(text: &str) -> OsdPaste {
    OsdPaste::read(text).unwrap_or_else(|refusal| panic!("should be read:\n{refusal}"))
}

/// The elements the layout shows, each with its column and row.
fn shown(layout: &OsdLayout) -> Vec<(OsdElement, u8, u8)> {
    OsdElement::ALL
        .into_iter()
        .filter(|e| layout.placement(*e).visible)
        .map(|e| {
            let p = layout.placement(e);
            (e, p.column, p.row)
        })
        .collect()
}

#[test]
fn the_meteor65_pros_paste_gives_back_its_own_layout_with_rssi_dbm_blank() {
    // Basis: Source (the goggles-OSD research §1 decodes the same lines) and
    // Rule (#31 §7: the Meteor's paste gives back its own layout, with RSSI
    // dBm blank).
    let meteor = paste(METEOR);
    assert_eq!(meteor.profile, 1);
    assert_eq!(
        shown(&meteor.layout),
        [
            (OsdElement::BatteryVoltage, 1, 12),
            (OsdElement::Crosshairs, 13, 6),
            (OsdElement::Timer2, 22, 12),
            (OsdElement::FlightMode, 22, 11),
            (OsdElement::CraftName, 11, 12),
            (OsdElement::Throttle, 22, 10),
            (OsdElement::Warnings, 10, 10),
            (OsdElement::LinkQuality, 1, 10),
            (OsdElement::RssiDbm, 1, 9),
        ]
    );
    // Hidden elements keep their place.
    assert_eq!(
        meteor.layout.placement(OsdElement::AverageCellVoltage),
        Placement::hidden(12, 1)
    );
    assert!(
        meteor.notes.iter().any(|note| note
            == "RSSI dBm is on, but reads blank: the sim has nothing to show there yet."),
        "{:?}",
        meteor.notes
    );
    // Its timers, warnings, statistics and alarms are Betaflight's defaults.
    assert_eq!(meteor.layout.timers, OsdLayout::betaflight_default().timers);
    assert_eq!(meteor.layout.warnings, DEFAULT_WARNINGS);
    assert_eq!(meteor.layout.statistics, DEFAULT_STATISTICS);
    assert!(
        meteor
            .summary()
            .starts_with("Used OSD profile 1 (14 OSD settings); ignored ")
    );
}

#[test]
fn the_cetus_xs_4_4_paste_reads_its_sixth_column_bit_and_all_three_profiles() {
    // Basis: Source (the goggles-OSD research §1: throttle 2,9; RSSI dBm
    // 2,10; LQ 2,11; voltage 2,12; Flight Mode 24,10; timer 2 23,12;
    // crosshair 13,7; craft name 11,12; warnings 9,9).
    let cetus = paste(CETUS);
    assert_eq!(
        shown(&cetus.layout),
        [
            (OsdElement::BatteryVoltage, 2, 12),
            (OsdElement::Crosshairs, 13, 7),
            (OsdElement::Timer2, 23, 12),
            (OsdElement::FlightMode, 24, 10),
            (OsdElement::CraftName, 11, 12),
            (OsdElement::Throttle, 2, 9),
            (OsdElement::Warnings, 9, 9),
            (OsdElement::LinkQuality, 2, 11),
            (OsdElement::RssiDbm, 2, 10),
        ]
    );
    // Its canvas is the SD grid, so nothing is scaled.
    assert!(!cetus.notes.iter().any(|note| note.contains("HD grid")));
}

#[test]
fn only_the_active_osd_profiles_visibility_counts() {
    // Basis: Source (`osdElementVisible`: bit 11 + profile − 1).
    let lines = "set osd_vbat_pos = 4481\nset osd_flymode_pos = 2422\nset osd_profile = 2\n";
    let pasted = paste(lines);
    assert_eq!(pasted.profile, 2);
    // 4481 = 4096 (profile 2) + 385 (1,12); 2422 shows only in profile 1;
    // the warnings show in every profile by default.
    assert_eq!(
        shown(&pasted.layout),
        [
            (OsdElement::BatteryVoltage, 1, 12),
            (OsdElement::Warnings, 9, 10)
        ]
    );
}

#[test]
fn pal_rows_move_to_row_12_and_hd_positions_are_scaled_onto_the_30_by_13_grid() {
    // Basis: Rule (#31 §7: PAL rows 13–15 move to row 12, as Betaflight does
    // on NTSC; HD positions are scaled into 30 × 13).
    // 2048 + 14 × 32 + 5: column 5, row 14 of a PAL grid.
    let pal = paste("set osd_vbat_pos = 2501\n");
    assert_eq!(
        pal.layout.placement(OsdElement::BatteryVoltage),
        Placement::at(5, 12)
    );
    assert!(pal.notes.iter().any(|n| n.contains("PAL grid")));
    // Column 40 (bit 10 + 8), row 18 on 4.4's default HD canvas of 53 × 20:
    // 40 × 30 ÷ 53 = 22, 18 × 13 ÷ 20 = 11.
    let hd = paste(
        "# version\n# Betaflight / STM32F7X2 (S7X2) 4.4.0 Dec 25 2022 / 12:00:00 (abc) MSP API: 1.45\nset osd_vbat_pos = 3656\n",
    );
    assert_eq!(
        hd.layout.placement(OsdElement::BatteryVoltage),
        Placement::at(22, 11)
    );
    assert!(hd.notes.iter().any(|n| n.contains("HD grid of 53 × 20")));
}

#[test]
fn a_4_4_warnings_bitmask_is_translated_into_2026_6s_order() {
    // Basis: Source (4.4.0's osdWarningsFlags_e has RC_SMOOTHING at bit 8,
    // FAIL_SAFE at 9; 2026.6.2's dropped RC_SMOOTHING, so FAIL_SAFE is bit 8).
    // 4.4's defaults with FAIL_SAFE (bit 9) off: 0x3FFFF less RSSI (13), LQ
    // (14), RSSI dBm (15), over cap (16), RSNR (17) and bit 9.
    let bits = 0x3FFFF & !(1 << 9 | 1 << 13 | 1 << 14 | 1 << 15 | 1 << 16 | 1 << 17);
    let text = format!(
        "# version\n# Betaflight / STM32F411 (S411) 4.4.0 Oct  9 2023 / 12:17:54 (4605309d8) MSP API: 1.45\nset osd_warn_bitmask = {bits}\n"
    );
    let pasted = paste(&text);
    assert!(!pasted.layout.warning_on(Warning::FailSafe));
    assert!(pasted.layout.warning_on(Warning::LaunchControl));
    assert!(pasted.layout.warning_on(Warning::ArmingDisabled));
    assert!(!pasted.layout.warning_on(Warning::LinkQuality));
    assert_eq!(
        pasted.layout.warnings,
        DEFAULT_WARNINGS & !Warning::FailSafe.bit()
    );
    // From 2025.12 the bits are 2026.6's already.
    let latest = text.replace("4.4.0 Oct", "2026.6.2 Oct");
    let read = paste(&latest);
    assert_eq!(read.layout.warnings, bits);
}

#[test]
fn a_warnings_bitmask_with_no_version_line_is_refused() {
    // Basis: Rule: its bits' order can't be known.
    let refusal = OsdPaste::read("set osd_warn_bitmask = 1\n").unwrap_err();
    assert!(
        refusal.to_string().contains("no `# version` line"),
        "{refusal}"
    );
}

#[test]
fn the_4_3_warning_and_statistic_switches_read_one_line_each() {
    let text = "set osd_warn_fail_safe = OFF\nset osd_warn_link_quality = ON\nset osd_stat_max_spd = OFF\nset osd_stat_tim_1 = ON\n";
    let pasted = paste(text);
    assert!(!pasted.layout.warning_on(Warning::FailSafe));
    assert!(pasted.layout.warning_on(Warning::LinkQuality));
    assert!(!pasted.layout.statistic_on(Statistic::MaxSpeed));
    assert!(pasted.layout.statistic_on(Statistic::Timer1));
    assert!(pasted.layout.statistic_on(Statistic::Timer2));
}

#[test]
fn the_timers_alarms_and_statistics_bitmask_read_as_betaflight_packs_them() {
    // osd_tim1 = source | precision << 4 | alarm << 8: 3 | 2 << 4 | 5 << 8.
    let text = "set osd_tim1 = 1315\nset osd_stat_bitmask = 6\nset osd_cap_alarm = 300\nset osd_link_quality_alarm = 70\n";
    let pasted = paste(text);
    let timer = pasted.layout.timers[0];
    assert_eq!(timer.source, TimerSource::OnOrArmed);
    assert_eq!(timer.precision, TimerPrecision::Tenths);
    assert_eq!(timer.alarm_minutes, 5);
    assert_eq!(timer.setting(), 1315);
    assert_eq!(pasted.layout.statistics, 6);
    assert_eq!(pasted.layout.alarms.capacity, 300);
    assert_eq!(pasted.layout.alarms.link_quality, 70);
}

#[test]
fn now_and_after_show_every_change_before_it_is_applied() {
    // Basis: Rule (#31 §7: a Now / After grid and Apply).
    let meteor = paste(METEOR);
    let now = OsdLayout::opendrone_default();
    let rows = meteor.now_after(&now);
    let vbat = rows.iter().find(|r| r.setting == "vbat").unwrap();
    assert_eq!(
        (vbat.now.as_str(), vbat.after.as_str()),
        ("10,7 off", "1,12 on")
    );
    let crosshairs = rows.iter().find(|r| r.setting == "crosshairs").unwrap();
    assert!(!crosshairs.changed());
    // The pictures of the two grids.
    assert_eq!(now.picture()[12], " b3.48V    QUAD       F01:23  ");
    assert_eq!(
        meteor.layout.picture()[12],
        " b4.20V    QUAD       F01:23  "
    );
}

#[test]
fn opendrones_default_layout_is_the_meteor65_pros_adjusted_as_31_draws_it() {
    // Basis: Rule (#31 §2).
    let picture = OsdLayout::opendrone_default().picture();
    assert_eq!(
        picture,
        [
            "                              ",
            "                          B12 ",
            "                              ",
            "                              ",
            "                              ",
            "                              ",
            "             -+-              ",
            "                              ",
            "           DISARMED           ",
            "                              ",
            "          LOW BATTERY T 42    ",
            " L7:100               AIR     ",
            " b3.48V    QUAD       F01:23  ",
        ]
    );
}

#[test]
fn a_paste_with_no_osd_settings_or_from_before_4_3_is_refused() {
    assert!(OsdPaste::read("set p_roll = 45\n").is_err());
    let old = "# version\n# Betaflight / STM32F411 (S411) 4.2.11 Oct  9 2021 / 12:00:00 (abc) MSP API: 1.43\nset osd_vbat_pos = 2433\n";
    assert!(
        OsdPaste::read(old)
            .unwrap_err()
            .to_string()
            .contains("older than 4.3")
    );
}
