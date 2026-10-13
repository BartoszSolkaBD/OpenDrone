//! The OSD paste: a pilot pastes `diff`, `diff all` or `dump` output (or
//! just `osd_*` lines) from the Betaflight CLI, and their OSD layout is read
//! from it (#31 §7).
//!
//! What it reads, starting from Betaflight's own defaults, as a `diff` lists
//! only what differs from them:
//!
//! - every `osd_<element>_pos` OpenDrone knows ([`OsdElement`]), in 4.3's
//!   encoding (5-bit columns) and 4.4's and later (a sixth column bit, for
//!   HD grids). An element shows if its bit for the active OSD profile is
//!   set: `osd_profile`, 1 unless the paste says. Rows 13 to 15 of a PAL
//!   grid move to row 12, as Betaflight does on NTSC; positions on an HD
//!   grid (`osd_canvas_width` above 30, or, with no canvas lines, a position
//!   past column 29 or row 15, as on 4.4's 53 × 20 default) are scaled onto
//!   the 30 × 13 grid;
//! - the timers, `osd_tim1` and `osd_tim2`;
//! - the warnings: 4.3's `osd_warn_*` lines, or 4.4's and later
//!   `osd_warn_bitmask`, whose bits Betaflight reordered in 2025.12 (it
//!   dropped `RC_SMOOTHING`): an older bitmask is translated into 2026.6's
//!   order, the warnings 2026.6 added staying on, as they are by default;
//! - the statistics: 4.3's `osd_stat_*` lines, or `osd_stat_bitmask`, whose
//!   bits have never moved;
//! - the alarms OpenDrone uses: RSSI, Link Quality, RSSI dBm, RSNR and
//!   capacity.
//!
//! Everything else is ignored: the craft name (the Quad's own name shows),
//! the canvas size (once it has scaled the positions), units, other OSD
//! settings and every line that isn't an OSD setting. An element OpenDrone
//! doesn't know that shows in the active profile gets a note.

use super::{CliText, Command, NowAfter, Refusal, Version};
use crate::osd::{
    COLUMNS, DEFAULT_STATISTICS, DEFAULT_WARNINGS, OsdElement, OsdLayout, Placement, ROWS,
    Statistic, Timer, Warning,
};

/// The OSD layout read from a paste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsdPaste {
    /// The paste's Betaflight version, if it says.
    pub version: Option<Version>,
    /// The OSD profile whose visibility counts.
    pub profile: u8,
    /// The layout after the paste: Betaflight's defaults with the paste's
    /// settings.
    pub layout: OsdLayout,
    /// The OSD settings read, as `set` lines, in the paste's order.
    pub read: Vec<String>,
    /// How many lines were ignored.
    pub ignored: usize,
    /// Plain sentences on what was adjusted or left out.
    pub notes: Vec<String>,
}

/// 4.3's warning switches, by their `osd_warn_<name>` names, with the
/// 2026.6 warning each is; `None` for one 2026.6 dropped.
const WARNINGS_4_3: [(&str, Option<Warning>); 17] = [
    ("arming_disable", Some(Warning::ArmingDisabled)),
    ("batt_not_full", Some(Warning::BatteryNotFull)),
    ("batt_warning", Some(Warning::BatteryWarning)),
    ("batt_critical", Some(Warning::BatteryCritical)),
    ("visual_beeper", Some(Warning::VisualBeeper)),
    ("crash_flip", Some(Warning::CrashFlip)),
    ("esc_fail", Some(Warning::EscFail)),
    ("core_temp", Some(Warning::CoreTemperature)),
    ("rc_smoothing", None),
    ("fail_safe", Some(Warning::FailSafe)),
    ("launch_control", Some(Warning::LaunchControl)),
    ("no_gps_rescue", Some(Warning::GpsRescueUnavailable)),
    ("gps_rescue_disabled", Some(Warning::GpsRescueFailing)),
    ("rssi", Some(Warning::Rssi)),
    ("link_quality", Some(Warning::LinkQuality)),
    ("rssi_dbm", Some(Warning::RssiDbm)),
    ("over_cap", Some(Warning::OverCapacity)),
];

/// 4.4's and 4.5's `osd_warn_bitmask` bits, in order, as 2026.6's
/// warnings: 4.3's list, then RSNR and (4.5) LOAD.
const WARNING_BITS_4_4: [Option<Warning>; 19] = [
    Some(Warning::ArmingDisabled),
    Some(Warning::BatteryNotFull),
    Some(Warning::BatteryWarning),
    Some(Warning::BatteryCritical),
    Some(Warning::VisualBeeper),
    Some(Warning::CrashFlip),
    Some(Warning::EscFail),
    Some(Warning::CoreTemperature),
    None,
    Some(Warning::FailSafe),
    Some(Warning::LaunchControl),
    Some(Warning::GpsRescueUnavailable),
    Some(Warning::GpsRescueFailing),
    Some(Warning::Rssi),
    Some(Warning::LinkQuality),
    Some(Warning::RssiDbm),
    Some(Warning::OverCapacity),
    Some(Warning::Rsnr),
    Some(Warning::Load),
];

/// 4.3's statistic switches, by their `osd_stat_<name>` names, with their
/// bit.
const STATISTICS_4_3: [(&str, Statistic); 24] = [
    ("rtc_date_time", Statistic::RtcDateTime),
    ("tim_1", Statistic::Timer1),
    ("tim_2", Statistic::Timer2),
    ("max_spd", Statistic::MaxSpeed),
    ("max_dist", Statistic::MaxDistance),
    ("min_batt", Statistic::MinBattery),
    ("endbatt", Statistic::EndBattery),
    ("battery", Statistic::Battery),
    ("min_rssi", Statistic::MinRssi),
    ("max_curr", Statistic::MaxCurrent),
    ("used_mah", Statistic::UsedMah),
    ("max_alt", Statistic::MaxAltitude),
    ("bbox", Statistic::Blackbox),
    ("bb_no", Statistic::BlackboxNumber),
    ("max_g_force", Statistic::MaxGForce),
    ("max_esc_temp", Statistic::MaxEscTemperature),
    ("max_esc_rpm", Statistic::MaxEscRpm),
    ("min_link_quality", Statistic::MinLinkQuality),
    ("flight_dist", Statistic::FlightDistance),
    ("max_fft", Statistic::MaxFft),
    ("total_flights", Statistic::TotalFlights),
    ("total_time", Statistic::TotalTime),
    ("total_dist", Statistic::TotalDistance),
    ("min_rssi_dbm", Statistic::MinRssiDbm),
];

/// The visibility bits of OSD profiles 1 to 3 (`OSD_PROFILE_BITS_POS`).
const PROFILE_BIT: u16 = 11;

/// A position setting's column and row (`OSD_X`, `OSD_Y`): bit 10 is 4.4's
/// sixth column bit, always 0 in 4.3.
fn position(value: u16) -> (u16, u16) {
    let column = (value & 0x1F) | ((value & 0x400) >> 5);
    let row = (value >> 5) & 0x1F;
    (column, row)
}

impl OsdPaste {
    /// Reads the OSD layout from pasted CLI text. Refused, with a sentence,
    /// when it comes from Betaflight older than 4.3, holds no OSD setting,
    /// gives an `osd_warn_bitmask` with no `# version` line to say its bits'
    /// order, or sets a value Betaflight would refuse.
    pub fn read(text: &str) -> Result<OsdPaste, Refusal> {
        let cli = CliText::read(text);
        if let Some(version) = cli.version
            && (version.major, version.minor) < (4, 3)
        {
            return Err(Refusal::one(format!(
                "This is Betaflight {version}, which is older than 4.3: OpenDrone reads OSD layouts from 4.3 or newer."
            )));
        }
        let sets: Vec<(usize, &str, &str)> = cli
            .lines
            .iter()
            .filter_map(|line| match line.command {
                Command::Set { name, value } => Some((line.number, name, value)),
                _ => None,
            })
            .collect();
        let value_of = |wanted: &str| {
            sets.iter()
                .rev()
                .find(|(_, name, _)| *name == wanted)
                .map(|(number, _, value)| (*number, *value))
        };
        let mut problems = Vec::new();
        let profile = match value_of("osd_profile") {
            Some((line, value)) => {
                whole(&mut problems, line, "osd_profile", value, 1, 3).unwrap_or(1) as u8
            }
            None => 1,
        };
        // The canvas: HD positions are scaled onto the 30 × 13 grid.
        let canvas_width = value_of("osd_canvas_width")
            .and_then(|(line, value)| whole(&mut problems, line, "osd_canvas_width", value, 1, 63));
        let canvas_height = value_of("osd_canvas_height").and_then(|(line, value)| {
            whole(&mut problems, line, "osd_canvas_height", value, 1, 31)
        });
        let visible_bit = 1u16 << (PROFILE_BIT + u16::from(profile) - 1);
        let mut positions: Vec<(usize, &str, u16)> = Vec::new();
        for (line, name, value) in &sets {
            let Some(element) = name
                .strip_prefix("osd_")
                .and_then(|n| n.strip_suffix("_pos"))
            else {
                continue;
            };
            if let Some(v) = whole(&mut problems, *line, name, value, 0, 65535) {
                positions.push((*line, element, v as u16));
            }
        }
        let past_sd = positions.iter().any(|(_, _, value)| {
            let (column, row) = position(*value);
            value & visible_bit != 0 && (column >= COLUMNS as u16 || row > 15)
        });
        let hd = match (canvas_width, canvas_height) {
            (Some(width), height) if width > COLUMNS as i64 => {
                Some((width as u16, height.unwrap_or(20) as u16))
            }
            (None, None) if past_sd => Some((53, 20)),
            _ => None,
        };

        let mut layout = OsdLayout::betaflight_default();
        let mut read = Vec::new();
        let mut notes = Vec::new();
        let mut used = 0usize;
        let mut moved_from_pal = Vec::new();
        for (_, element_name, value) in &positions {
            let line_text = format!("set osd_{element_name}_pos = {value}");
            let Some(element) = OsdElement::ALL
                .into_iter()
                .find(|e| e.cli_name() == *element_name)
            else {
                if value & visible_bit != 0 {
                    notes.push(format!(
                        "`osd_{element_name}_pos` shows an element OpenDrone doesn't have, so it's left out."
                    ));
                }
                continue;
            };
            let (mut column, mut row) = position(*value);
            if let Some((width, height)) = hd {
                column = column * COLUMNS as u16 / width.max(1);
                row = row * ROWS as u16 / height.max(1);
            } else if row >= ROWS as u16 && row <= 15 {
                moved_from_pal.push(element.cli_name());
                row = ROWS as u16 - 1;
            }
            layout.place(
                element,
                Placement {
                    column: column.min(u16::from(u8::MAX)) as u8,
                    row: row.min(u16::from(u8::MAX)) as u8,
                    visible: value & visible_bit != 0,
                },
            );
            read.push(line_text);
            used += 1;
        }
        if let Some((width, height)) = hd {
            notes.push(format!(
                "The positions are on Betaflight's HD grid of {width} × {height}, so they're scaled onto the 30 × 13 grid."
            ));
        }
        if !moved_from_pal.is_empty() {
            notes.push(format!(
                "{} sat below row 12, on a PAL grid, so moved to row 12, as Betaflight does on NTSC.",
                moved_from_pal.join(", ")
            ));
        }

        for (index, name) in ["osd_tim1", "osd_tim2"].into_iter().enumerate() {
            if let Some((line, value)) = value_of(name)
                && let Some(v) = whole(&mut problems, line, name, value, 0, 32767)
            {
                match Timer::from_setting(v as u16) {
                    Some(timer) => {
                        layout.timers[index] = timer;
                        read.push(format!("set {name} = {value}"));
                        used += 1;
                    }
                    None => problems.push(format!(
                        "Line {line}: `{name}` names a timer source or precision Betaflight doesn't have."
                    )),
                }
            }
        }

        // The warnings.
        if let Some((line, value)) = value_of("osd_warn_bitmask") {
            match value.parse::<u32>() {
                Err(_) => problems.push(format!(
                    "Line {line}: `osd_warn_bitmask` is a whole number, not \"{value}\"."
                )),
                Ok(bits) => match cli.version {
                    None => problems.push(
                        "There's no `# version` line, so the order of `osd_warn_bitmask`'s bits is unknown (Betaflight 2025.12 changed it): paste the whole output of `diff all`, from its first line.".to_string(),
                    ),
                    Some(version) if (version.major, version.minor) < (2025, 12) => {
                        // 4.5 added LOAD as its 19th bit.
                        let count = if (version.major, version.minor) < (4, 5) {
                            18
                        } else {
                            19
                        };
                        layout.warnings = translate_warnings(bits, count);
                        read.push(format!("set osd_warn_bitmask = {value}"));
                        notes.push(format!(
                            "`osd_warn_bitmask` from Betaflight {version} is translated into 2026.6's order."
                        ));
                        used += 1;
                    }
                    Some(_) => {
                        layout.warnings = bits;
                        read.push(format!("set osd_warn_bitmask = {value}"));
                        used += 1;
                    }
                },
            }
        }
        for (_, name, value) in &sets {
            let Some(warning) = name.strip_prefix("osd_warn_") else {
                continue;
            };
            let Some((_, found)) = WARNINGS_4_3.iter().find(|(n, _)| *n == warning) else {
                continue;
            };
            let on = match *value {
                "ON" => true,
                "OFF" => false,
                _ => {
                    problems.push(format!("`{name}` is OFF or ON, not \"{value}\"."));
                    continue;
                }
            };
            read.push(format!("set {name} = {value}"));
            used += 1;
            if let Some(warning) = found {
                if on {
                    layout.warnings |= warning.bit();
                } else {
                    layout.warnings &= !warning.bit();
                }
            }
        }

        // The statistics.
        if let Some((line, value)) = value_of("osd_stat_bitmask") {
            match value.parse::<u32>() {
                Ok(bits) => {
                    layout.statistics = bits;
                    read.push(format!("set osd_stat_bitmask = {value}"));
                    used += 1;
                }
                Err(_) => problems.push(format!(
                    "Line {line}: `osd_stat_bitmask` is a whole number, not \"{value}\"."
                )),
            }
        }
        for (_, name, value) in &sets {
            let Some(statistic) = name.strip_prefix("osd_stat_") else {
                continue;
            };
            let Some((_, statistic)) = STATISTICS_4_3.iter().find(|(n, _)| *n == statistic) else {
                continue;
            };
            match *value {
                "ON" => layout.statistics |= statistic.bit(),
                "OFF" => layout.statistics &= !statistic.bit(),
                _ => {
                    problems.push(format!("`{name}` is OFF or ON, not \"{value}\"."));
                    continue;
                }
            }
            read.push(format!("set {name} = {value}"));
            used += 1;
        }

        // The alarms.
        let alarms: [(&str, i64, i64); 5] = [
            ("osd_rssi_alarm", 0, 100),
            ("osd_link_quality_alarm", 0, 100),
            ("osd_rssi_dbm_alarm", -130, 0),
            ("osd_rsnr_alarm", -30, 20),
            ("osd_cap_alarm", 0, 20000),
        ];
        for (name, lowest, highest) in alarms {
            let Some((line, value)) = value_of(name) else {
                continue;
            };
            let Some(v) = whole(&mut problems, line, name, value, lowest, highest) else {
                continue;
            };
            match name {
                "osd_rssi_alarm" => layout.alarms.rssi = v as u8,
                "osd_link_quality_alarm" => layout.alarms.link_quality = v as u16,
                "osd_rssi_dbm_alarm" => layout.alarms.rssi_dbm = v as i16,
                "osd_rsnr_alarm" => layout.alarms.rsnr = v as i16,
                _ => layout.alarms.capacity = v as u16,
            }
            read.push(format!("set {name} = {value}"));
            used += 1;
        }
        if value_of("osd_profile").is_some() {
            read.push(format!("set osd_profile = {profile}"));
            used += 1;
        }

        if !problems.is_empty() {
            return Err(Refusal(problems));
        }
        if read.is_empty() {
            return Err(Refusal::one(
                "There's no OSD setting here: paste the output of `diff all`, `diff` or `dump`, or the `osd_` lines from it.",
            ));
        }
        let ignored = cli.lines.len().saturating_sub(used);
        // Elements that show but have nothing to show in the sim.
        let blank: Vec<&str> = OsdElement::ALL
            .into_iter()
            .filter(|e| layout.placement(*e).visible && e.drawn() != crate::osd::Drawn::Yes)
            .map(OsdElement::words)
            .collect();
        if !blank.is_empty() {
            notes.push(format!(
                "{} {} on, but read{} blank: the sim has nothing to show there yet.",
                blank.join(", "),
                if blank.len() == 1 { "is" } else { "are" },
                if blank.len() == 1 { "s" } else { "" },
            ));
        }
        Ok(OsdPaste {
            version: cli.version,
            profile,
            layout,
            read,
            ignored,
            notes,
        })
    }

    /// One plain sentence on what was read, such as "Used OSD profile 1
    /// (12 OSD settings); ignored 160 lines."
    pub fn summary(&self) -> String {
        let settings = match self.read.len() {
            1 => "1 OSD setting".to_string(),
            n => format!("{n} OSD settings"),
        };
        let lines = if self.ignored == 1 { "line" } else { "lines" };
        format!(
            "Used OSD profile {} ({settings}); ignored {} {lines}.",
            self.profile, self.ignored
        )
    }

    /// Every element's place, the timers, the warnings, the statistics and
    /// the alarms, now and after the paste, so the pilot sees what changes
    /// before applying it. Pictures of both grids come from
    /// [`OsdLayout::picture`].
    pub fn now_after(&self, now: &OsdLayout) -> Vec<NowAfter> {
        let after = &self.layout;
        let mut rows: Vec<NowAfter> = OsdElement::ALL
            .into_iter()
            .map(|element| NowAfter {
                setting: element.cli_name(),
                now: placement_words(now.placement(element)),
                after: placement_words(after.placement(element)),
            })
            .collect();
        for (index, setting) in ["osd_tim1", "osd_tim2"].into_iter().enumerate() {
            rows.push(NowAfter {
                setting,
                now: timer_words(now.timers[index]),
                after: timer_words(after.timers[index]),
            });
        }
        rows.push(NowAfter {
            setting: "osd_warn_bitmask",
            now: warning_words(now.warnings),
            after: warning_words(after.warnings),
        });
        rows.push(NowAfter {
            setting: "osd_stat_bitmask",
            now: statistic_words(now.statistics),
            after: statistic_words(after.statistics),
        });
        let alarms = |a: &OsdLayout| {
            [
                format!("{}%", a.alarms.rssi),
                format!("{}%", a.alarms.link_quality),
                format!("{} dBm", a.alarms.rssi_dbm),
                format!("{} dB", a.alarms.rsnr),
                format!("{} mAh", a.alarms.capacity),
            ]
        };
        let settings = [
            "osd_rssi_alarm",
            "osd_link_quality_alarm",
            "osd_rssi_dbm_alarm",
            "osd_rsnr_alarm",
            "osd_cap_alarm",
        ];
        for ((setting, now), after) in settings.into_iter().zip(alarms(now)).zip(alarms(after)) {
            rows.push(NowAfter {
                setting,
                now,
                after,
            });
        }
        rows
    }
}

/// A setting's whole number, within Betaflight's range, or a problem.
fn whole(
    problems: &mut Vec<String>,
    line: usize,
    name: &str,
    value: &str,
    lowest: i64,
    highest: i64,
) -> Option<i64> {
    match value.parse::<i64>() {
        Ok(v) if (lowest..=highest).contains(&v) => Some(v),
        _ => {
            problems.push(format!(
                "Line {line}: `{name}` is a whole number from {lowest} to {highest}, not \"{value}\"."
            ));
            None
        }
    }
}

/// An older `osd_warn_bitmask` (4.4, 4.5) in 2026.6's order, reading its
/// first `count` bits. The warnings it doesn't have are on, as they are by
/// default.
fn translate_warnings(bits: u32, count: usize) -> u32 {
    let mut translated = DEFAULT_WARNINGS;
    for (bit, warning) in WARNING_BITS_4_4.iter().enumerate().take(count) {
        if let Some(warning) = warning {
            if bits & (1 << bit) != 0 {
                translated |= warning.bit();
            } else {
                translated &= !warning.bit();
            }
        }
    }
    translated
}

fn placement_words(placement: Placement) -> String {
    format!(
        "{},{} {}",
        placement.column,
        placement.row,
        if placement.visible { "on" } else { "off" }
    )
}

fn timer_words(timer: Timer) -> String {
    let alarm = match timer.alarm_minutes {
        0 => "no alarm".to_string(),
        minutes => format!("alarm at {minutes} min"),
    };
    format!(
        "{}, {}, {alarm}",
        timer.source.words(),
        timer.precision.words()
    )
}

fn warning_words(bits: u32) -> String {
    let on: Vec<&str> = Warning::SIMULATED
        .into_iter()
        .filter(|w| bits & w.bit() != 0)
        .map(Warning::words)
        .collect();
    if on.is_empty() {
        "none".to_string()
    } else {
        on.join(", ")
    }
}

fn statistic_words(bits: u32) -> String {
    if bits == DEFAULT_STATISTICS {
        return "Betaflight's defaults".to_string();
    }
    let on: Vec<&str> = Statistic::DISPLAY_ORDER
        .into_iter()
        .filter(|s| bits & s.bit() != 0)
        .map(Statistic::words)
        .collect();
    if on.is_empty() {
        "none".to_string()
    } else {
        on.join(", ")
    }
}
