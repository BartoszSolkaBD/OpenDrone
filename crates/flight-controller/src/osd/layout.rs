//! The OSD layout: which OSD elements show and where on the 30 × 13 grid,
//! plus the OSD's own options: the two timers, which warnings and
//! statistics are on, and the alarms. It belongs to the pilot and is the
//! same on every Quad (#31 §7).

/// Betaflight's NTSC grid: 30 columns and 13 rows of characters.
pub const COLUMNS: usize = 30;
pub const ROWS: usize = 13;

/// An OSD element OpenDrone knows, by Betaflight's `osd_<name>_pos` setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OsdElement {
    /// `vbat`: the battery voltage.
    BatteryVoltage,
    /// `rssi`: RSSI in percent. No source in the sim: blank.
    Rssi,
    /// `crosshairs`.
    Crosshairs,
    /// `ah`: the artificial horizon. Not drawn yet: blank.
    ArtificialHorizon,
    /// `ah_sbar`: the horizon's sidebars. Not drawn yet: blank.
    HorizonSidebars,
    /// `tim_1`, `tim_2`.
    Timer1,
    Timer2,
    /// `flymode`: the Flight Mode.
    FlightMode,
    /// `craft_name`: the Quad's name.
    CraftName,
    /// `throttle`.
    Throttle,
    /// `current`: the battery current, where the Tune's `current_meter`
    /// reads one.
    Current,
    /// `mah_drawn`: the charge drawn, the same way.
    MahDrawn,
    /// `warnings`.
    Warnings,
    /// `avg_cell_voltage`: the average cell voltage.
    AverageCellVoltage,
    /// `disarmed`: `DISARMED` while disarmed.
    Disarmed,
    /// `esc_rpm`: each motor's RPM, where the Tune has bidirectional DShot.
    MotorRpm,
    /// `link_quality`.
    LinkQuality,
    /// `log_status`: the Blackbox log number while a log is written.
    LogStatus,
    /// `rssi_dbm`: no source in the sim: blank.
    RssiDbm,
    /// `rsnr`: no source in the sim: blank.
    Rsnr,
    /// `flip_arrow`: not drawn yet: blank.
    FlipArrow,
}

/// Whether OpenDrone draws an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawn {
    Yes,
    /// The element shows what the real quad couldn't know in the sim, such
    /// as RSSI: it reads blank (#31 §3).
    NoSource,
    /// The element has a source but isn't drawn yet: it reads blank.
    NotYet,
}

impl OsdElement {
    /// Every element, in the order a layout lists them.
    pub const ALL: [OsdElement; 21] = [
        OsdElement::BatteryVoltage,
        OsdElement::Rssi,
        OsdElement::Crosshairs,
        OsdElement::ArtificialHorizon,
        OsdElement::HorizonSidebars,
        OsdElement::Timer1,
        OsdElement::Timer2,
        OsdElement::FlightMode,
        OsdElement::CraftName,
        OsdElement::Throttle,
        OsdElement::Current,
        OsdElement::MahDrawn,
        OsdElement::Warnings,
        OsdElement::AverageCellVoltage,
        OsdElement::Disarmed,
        OsdElement::MotorRpm,
        OsdElement::LinkQuality,
        OsdElement::LogStatus,
        OsdElement::RssiDbm,
        OsdElement::Rsnr,
        OsdElement::FlipArrow,
    ];

    /// The order Betaflight draws them in (`osdElementDisplayOrder`, then
    /// the ESC elements): a later one covers an earlier one where they
    /// overlap.
    pub const DRAWING_ORDER: [OsdElement; 21] = [
        OsdElement::ArtificialHorizon,
        OsdElement::BatteryVoltage,
        OsdElement::Rssi,
        OsdElement::Crosshairs,
        OsdElement::HorizonSidebars,
        OsdElement::Timer1,
        OsdElement::Timer2,
        OsdElement::FlightMode,
        OsdElement::Throttle,
        OsdElement::Current,
        OsdElement::MahDrawn,
        OsdElement::CraftName,
        OsdElement::Warnings,
        OsdElement::AverageCellVoltage,
        OsdElement::Disarmed,
        OsdElement::LogStatus,
        OsdElement::FlipArrow,
        OsdElement::LinkQuality,
        OsdElement::RssiDbm,
        OsdElement::Rsnr,
        OsdElement::MotorRpm,
    ];

    /// Betaflight's name in its `osd_<name>_pos` setting.
    pub fn cli_name(self) -> &'static str {
        match self {
            OsdElement::BatteryVoltage => "vbat",
            OsdElement::Rssi => "rssi",
            OsdElement::Crosshairs => "crosshairs",
            OsdElement::ArtificialHorizon => "ah",
            OsdElement::HorizonSidebars => "ah_sbar",
            OsdElement::Timer1 => "tim_1",
            OsdElement::Timer2 => "tim_2",
            OsdElement::FlightMode => "flymode",
            OsdElement::CraftName => "craft_name",
            OsdElement::Throttle => "throttle",
            OsdElement::Current => "current",
            OsdElement::MahDrawn => "mah_drawn",
            OsdElement::Warnings => "warnings",
            OsdElement::AverageCellVoltage => "avg_cell_voltage",
            OsdElement::Disarmed => "disarmed",
            OsdElement::MotorRpm => "esc_rpm",
            OsdElement::LinkQuality => "link_quality",
            OsdElement::LogStatus => "log_status",
            OsdElement::RssiDbm => "rssi_dbm",
            OsdElement::Rsnr => "rsnr",
            OsdElement::FlipArrow => "flip_arrow",
        }
    }

    /// Its name in plain words, as pilots and Scenarios call it.
    pub fn words(self) -> &'static str {
        match self {
            OsdElement::BatteryVoltage => "battery voltage",
            OsdElement::Rssi => "RSSI",
            OsdElement::Crosshairs => "crosshairs",
            OsdElement::ArtificialHorizon => "artificial horizon",
            OsdElement::HorizonSidebars => "horizon sidebars",
            OsdElement::Timer1 => "timer 1",
            OsdElement::Timer2 => "timer 2",
            OsdElement::FlightMode => "Flight Mode",
            OsdElement::CraftName => "Quad name",
            OsdElement::Throttle => "throttle",
            OsdElement::Current => "current",
            OsdElement::MahDrawn => "mAh drawn",
            OsdElement::Warnings => "warnings",
            OsdElement::AverageCellVoltage => "average cell voltage",
            OsdElement::Disarmed => "disarmed",
            OsdElement::MotorRpm => "motor RPM",
            OsdElement::LinkQuality => "Link Quality",
            OsdElement::LogStatus => "Blackbox log status",
            OsdElement::RssiDbm => "RSSI dBm",
            OsdElement::Rsnr => "RSNR",
            OsdElement::FlipArrow => "flip arrow",
        }
    }

    /// The element named in plain words or by its Betaflight name, any
    /// case.
    pub fn named(name: &str) -> Option<OsdElement> {
        let name = name.trim();
        OsdElement::ALL.into_iter().find(|element| {
            element.words().eq_ignore_ascii_case(name)
                || element.cli_name().eq_ignore_ascii_case(name)
        })
    }

    /// A typical reading, for a picture of a layout with no flight behind
    /// it, its glyphs written as letters as #31 §2 draws them: `b` the
    /// battery, `T` the throttle, `L` Link Quality, `F` and `O` the timers'
    /// flying and on clocks, `B` the Blackbox log.
    pub fn sample(self) -> &'static str {
        match self {
            OsdElement::BatteryVoltage => "b4.20V",
            OsdElement::Rssi => "R99",
            OsdElement::Crosshairs => "-+-",
            OsdElement::ArtificialHorizon => "---------",
            OsdElement::HorizonSidebars => "|",
            OsdElement::Timer1 => "O00:42",
            OsdElement::Timer2 => "F01:23",
            OsdElement::FlightMode => "AIR",
            OsdElement::CraftName => "QUAD",
            OsdElement::Throttle => "T 42",
            OsdElement::Current => " 12.30A",
            OsdElement::MahDrawn => " 123mAh",
            OsdElement::Warnings => "LOW BATTERY",
            OsdElement::AverageCellVoltage => "b3.48V",
            OsdElement::Disarmed => "DISARMED",
            OsdElement::MotorRpm => "21000",
            OsdElement::LinkQuality => "L7:100",
            OsdElement::LogStatus => "B12",
            OsdElement::RssiDbm => "-42",
            OsdElement::Rsnr => "9",
            OsdElement::FlipArrow => ">",
        }
    }

    pub fn drawn(self) -> Drawn {
        match self {
            OsdElement::Rssi | OsdElement::RssiDbm | OsdElement::Rsnr => Drawn::NoSource,
            OsdElement::ArtificialHorizon | OsdElement::HorizonSidebars | OsdElement::FlipArrow => {
                Drawn::NotYet
            }
            _ => Drawn::Yes,
        }
    }

    pub(crate) fn index(self) -> usize {
        OsdElement::ALL.iter().position(|e| *e == self).unwrap_or(0)
    }
}

/// Where an element sits, and whether it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// From 0 at the left to 29.
    pub column: u8,
    /// From 0 at the top to 12.
    pub row: u8,
    pub visible: bool,
}

impl Placement {
    pub const fn at(column: u8, row: u8) -> Placement {
        Placement {
            column,
            row,
            visible: true,
        }
    }

    pub const fn hidden(column: u8, row: u8) -> Placement {
        Placement {
            column,
            row,
            visible: false,
        }
    }
}

/// What a timer counts (`osd_timer_source_e`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerSource {
    /// `ON TIME`: since power-up.
    OnTime,
    /// `TOTAL ARM`: armed time since power-up.
    TotalArmed,
    /// `LAST ARM`: the last arming's armed time.
    LastArmed,
    /// `ON/ARM`: the armed time while armed, else the time since power-up.
    OnOrArmed,
    /// `LAUNCH TIME`: the armed time outside launch control, which the sim
    /// doesn't have, so the armed time since power-up.
    LaunchTime,
}

impl TimerSource {
    pub const ALL: [TimerSource; 5] = [
        TimerSource::OnTime,
        TimerSource::TotalArmed,
        TimerSource::LastArmed,
        TimerSource::OnOrArmed,
        TimerSource::LaunchTime,
    ];

    /// How the statistics page labels it (`osdTimerSourceNames`).
    pub fn label(self) -> &'static str {
        match self {
            TimerSource::OnTime => "ON TIME  ",
            TimerSource::TotalArmed => "TOTAL ARM",
            TimerSource::LastArmed => "LAST ARM ",
            TimerSource::OnOrArmed => "ON/ARM   ",
            TimerSource::LaunchTime => "LAUNCH TIME",
        }
    }

    /// In plain words, as a Scenario writes it.
    pub fn words(self) -> &'static str {
        match self {
            TimerSource::OnTime => "on time",
            TimerSource::TotalArmed => "total armed time",
            TimerSource::LastArmed => "last armed time",
            TimerSource::OnOrArmed => "on or armed time",
            TimerSource::LaunchTime => "launch time",
        }
    }
}

/// How finely a timer counts (`osd_timer_precision_e`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerPrecision {
    Seconds,
    Hundredths,
    Tenths,
}

impl TimerPrecision {
    pub const ALL: [TimerPrecision; 3] = [
        TimerPrecision::Seconds,
        TimerPrecision::Hundredths,
        TimerPrecision::Tenths,
    ];

    pub fn words(self) -> &'static str {
        match self {
            TimerPrecision::Seconds => "whole seconds",
            TimerPrecision::Hundredths => "hundredths",
            TimerPrecision::Tenths => "tenths",
        }
    }
}

/// One timer: what it counts, how finely, and the minutes past which it
/// blinks (0: never).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    pub source: TimerSource,
    pub precision: TimerPrecision,
    pub alarm_minutes: u8,
}

impl Timer {
    /// Betaflight's `osd_tim1`/`osd_tim2` number: source, precision << 4 and
    /// alarm << 8. `None` for a source or precision it doesn't have.
    pub fn from_setting(value: u16) -> Option<Timer> {
        Some(Timer {
            source: *TimerSource::ALL.get(usize::from(value & 0x0F))?,
            precision: *TimerPrecision::ALL.get(usize::from((value >> 4) & 0x0F))?,
            alarm_minutes: (value >> 8) as u8,
        })
    }

    pub fn setting(self) -> u16 {
        let source = TimerSource::ALL
            .iter()
            .position(|s| *s == self.source)
            .unwrap_or(0) as u16;
        let precision = TimerPrecision::ALL
            .iter()
            .position(|p| *p == self.precision)
            .unwrap_or(0) as u16;
        source | precision << 4 | u16::from(self.alarm_minutes) << 8
    }
}

/// An OSD warning, by its bit in Betaflight 2026.6's `osd_warn_bitmask`
/// (`osdWarningsFlags_e`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    ArmingDisabled = 0,
    BatteryNotFull = 1,
    BatteryWarning = 2,
    BatteryCritical = 3,
    VisualBeeper = 4,
    CrashFlip = 5,
    EscFail = 6,
    CoreTemperature = 7,
    FailSafe = 8,
    LaunchControl = 9,
    GpsRescueUnavailable = 10,
    GpsRescueFailing = 11,
    Rssi = 12,
    LinkQuality = 13,
    RssiDbm = 14,
    OverCapacity = 15,
    Rsnr = 16,
    Load = 17,
    PositionHoldFailed = 18,
    AutopilotAbort = 19,
}

impl Warning {
    /// Every warning, by its bit.
    pub const ALL: [Warning; 20] = [
        Warning::ArmingDisabled,
        Warning::BatteryNotFull,
        Warning::BatteryWarning,
        Warning::BatteryCritical,
        Warning::VisualBeeper,
        Warning::CrashFlip,
        Warning::EscFail,
        Warning::CoreTemperature,
        Warning::FailSafe,
        Warning::LaunchControl,
        Warning::GpsRescueUnavailable,
        Warning::GpsRescueFailing,
        Warning::Rssi,
        Warning::LinkQuality,
        Warning::RssiDbm,
        Warning::OverCapacity,
        Warning::Rsnr,
        Warning::Load,
        Warning::PositionHoldFailed,
        Warning::AutopilotAbort,
    ];

    /// The warnings the sim can raise, in plain words, in Betaflight's order
    /// of priority.
    pub const SIMULATED: [Warning; 12] = [
        Warning::ArmingDisabled,
        Warning::FailSafe,
        Warning::CrashFlip,
        Warning::Rssi,
        Warning::RssiDbm,
        Warning::Rsnr,
        Warning::LinkQuality,
        Warning::BatteryCritical,
        Warning::BatteryWarning,
        Warning::OverCapacity,
        Warning::BatteryNotFull,
        Warning::VisualBeeper,
    ];

    pub fn words(self) -> &'static str {
        match self {
            Warning::ArmingDisabled => "arming disabled",
            Warning::BatteryNotFull => "battery not full",
            Warning::BatteryWarning => "low battery",
            Warning::BatteryCritical => "battery critical",
            Warning::VisualBeeper => "visual beeper",
            Warning::CrashFlip => "crash flip",
            Warning::EscFail => "esc fail",
            Warning::CoreTemperature => "core temperature",
            Warning::FailSafe => "fail safe",
            Warning::LaunchControl => "launch control",
            Warning::GpsRescueUnavailable => "gps rescue unavailable",
            Warning::GpsRescueFailing => "gps rescue failing",
            Warning::Rssi => "rssi",
            Warning::LinkQuality => "link quality",
            Warning::RssiDbm => "rssi dbm",
            Warning::OverCapacity => "over capacity",
            Warning::Rsnr => "rsnr",
            Warning::Load => "load",
            Warning::PositionHoldFailed => "position hold failed",
            Warning::AutopilotAbort => "autopilot abort",
        }
    }

    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// Betaflight 2026.6's default warnings: all on but RSSI, Link Quality,
/// RSSI dBm, RSNR and over capacity (`pgResetFn_osdConfig`).
pub const DEFAULT_WARNINGS: u32 = 0xF_FFFF
    & !(1 << Warning::Rssi as u32
        | 1 << Warning::LinkQuality as u32
        | 1 << Warning::RssiDbm as u32
        | 1 << Warning::Rsnr as u32
        | 1 << Warning::OverCapacity as u32);

/// A post-flight statistic, by its bit in `osd_stat_bitmask` (the same in
/// every version since 4.3, `osd_stats_e`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statistic {
    RtcDateTime = 0,
    Timer1 = 1,
    Timer2 = 2,
    MaxSpeed = 3,
    MaxDistance = 4,
    MinBattery = 5,
    EndBattery = 6,
    Battery = 7,
    MinRssi = 8,
    MaxCurrent = 9,
    UsedMah = 10,
    MaxAltitude = 11,
    Blackbox = 12,
    BlackboxNumber = 13,
    MaxGForce = 14,
    MaxEscTemperature = 15,
    MaxEscRpm = 16,
    MinLinkQuality = 17,
    FlightDistance = 18,
    MaxFft = 19,
    TotalFlights = 20,
    TotalTime = 21,
    TotalDistance = 22,
    MinRssiDbm = 23,
    WattHoursDrawn = 24,
    MinRsnr = 25,
    BestThreeLaps = 26,
    BestLap = 27,
    FullThrottleTime = 28,
    FullThrottleCounter = 29,
    AverageThrottle = 30,
}

impl Statistic {
    /// The order the page lists them in (`osdStatsDisplayOrder`).
    pub const DISPLAY_ORDER: [Statistic; 31] = [
        Statistic::RtcDateTime,
        Statistic::Timer1,
        Statistic::Timer2,
        Statistic::MaxAltitude,
        Statistic::MaxSpeed,
        Statistic::MaxDistance,
        Statistic::FlightDistance,
        Statistic::MinBattery,
        Statistic::EndBattery,
        Statistic::Battery,
        Statistic::MinRssi,
        Statistic::MaxCurrent,
        Statistic::UsedMah,
        Statistic::Blackbox,
        Statistic::BlackboxNumber,
        Statistic::MaxGForce,
        Statistic::MaxEscTemperature,
        Statistic::MaxEscRpm,
        Statistic::MinLinkQuality,
        Statistic::MaxFft,
        Statistic::MinRssiDbm,
        Statistic::MinRsnr,
        Statistic::TotalFlights,
        Statistic::TotalTime,
        Statistic::TotalDistance,
        Statistic::WattHoursDrawn,
        Statistic::BestThreeLaps,
        Statistic::BestLap,
        Statistic::FullThrottleTime,
        Statistic::FullThrottleCounter,
        Statistic::AverageThrottle,
    ];

    /// In plain words, as a Scenario writes it, which is its Betaflight
    /// 4.3 `osd_stat_<name>` setting's name with spaces.
    pub fn words(self) -> &'static str {
        match self {
            Statistic::RtcDateTime => "rtc date time",
            Statistic::Timer1 => "timer 1",
            Statistic::Timer2 => "timer 2",
            Statistic::MaxSpeed => "max speed",
            Statistic::MaxDistance => "max distance",
            Statistic::MinBattery => "min battery",
            Statistic::EndBattery => "end battery",
            Statistic::Battery => "battery",
            Statistic::MinRssi => "min rssi",
            Statistic::MaxCurrent => "max current",
            Statistic::UsedMah => "used mah",
            Statistic::MaxAltitude => "max altitude",
            Statistic::Blackbox => "blackbox",
            Statistic::BlackboxNumber => "blackbox log number",
            Statistic::MaxGForce => "max g force",
            Statistic::MaxEscTemperature => "max esc temperature",
            Statistic::MaxEscRpm => "max esc rpm",
            Statistic::MinLinkQuality => "min link quality",
            Statistic::FlightDistance => "flight distance",
            Statistic::MaxFft => "max fft",
            Statistic::TotalFlights => "total flights",
            Statistic::TotalTime => "total time",
            Statistic::TotalDistance => "total distance",
            Statistic::MinRssiDbm => "min rssi dbm",
            Statistic::WattHoursDrawn => "watt hours drawn",
            Statistic::MinRsnr => "min rsnr",
            Statistic::BestThreeLaps => "best 3 consecutive laps",
            Statistic::BestLap => "best lap",
            Statistic::FullThrottleTime => "full throttle time",
            Statistic::FullThrottleCounter => "full throttle counter",
            Statistic::AverageThrottle => "average throttle",
        }
    }

    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// Betaflight's default statistics (`osd_stat_bitmask` 14124): timer 2, max
/// speed, min battery, min RSSI, max current, used mAh, Blackbox and the
/// Blackbox log number.
pub const DEFAULT_STATISTICS: u32 = 14124;

/// The alarms an element blinks past, or a warning starts at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alarms {
    /// `osd_rssi_alarm`, in percent.
    pub rssi: u8,
    /// `osd_link_quality_alarm`, in percent.
    pub link_quality: u16,
    /// `osd_rssi_dbm_alarm`, in dBm.
    pub rssi_dbm: i16,
    /// `osd_rsnr_alarm`, in dB.
    pub rsnr: i16,
    /// `osd_cap_alarm`: the mAh drawn past which the mAh blink.
    pub capacity: u16,
}

impl Alarms {
    /// Betaflight's defaults, the same since 4.3.
    pub const DEFAULT: Alarms = Alarms {
        rssi: 20,
        link_quality: 80,
        rssi_dbm: -60,
        rsnr: 4,
        capacity: 2200,
    };
}

/// The pilot's OSD layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsdLayout {
    /// Each element's place, in [`OsdElement::ALL`]'s order.
    placements: [Placement; 21],
    pub timers: [Timer; 2],
    /// The warnings that are on, as Betaflight 2026.6's `osd_warn_bitmask`.
    pub warnings: u32,
    /// The statistics that are on, as `osd_stat_bitmask`.
    pub statistics: u32,
    pub alarms: Alarms,
}

/// Betaflight's default timers: timer 1 counts since power-up, timer 2 the
/// armed time, both in whole seconds with a 10-minute alarm.
pub const DEFAULT_TIMERS: [Timer; 2] = [
    Timer {
        source: TimerSource::OnTime,
        precision: TimerPrecision::Seconds,
        alarm_minutes: 10,
    },
    Timer {
        source: TimerSource::TotalArmed,
        precision: TimerPrecision::Seconds,
        alarm_minutes: 10,
    },
];

impl OsdLayout {
    /// Betaflight's own defaults: every element hidden at its default place
    /// but the warnings (#31's research §1), with the default timers,
    /// warnings, statistics and alarms.
    pub fn betaflight_default() -> OsdLayout {
        let mut placements = [Placement::hidden(10, 7); 21];
        let mut set = |element: OsdElement, placement: Placement| {
            placements[element.index()] = placement;
        };
        set(OsdElement::Warnings, Placement::at(9, 10));
        set(OsdElement::Crosshairs, Placement::hidden(13, 6));
        set(OsdElement::ArtificialHorizon, Placement::hidden(14, 2));
        set(OsdElement::HorizonSidebars, Placement::hidden(14, 6));
        OsdLayout {
            placements,
            timers: DEFAULT_TIMERS,
            warnings: DEFAULT_WARNINGS,
            statistics: DEFAULT_STATISTICS,
            alarms: Alarms::DEFAULT,
        }
    }

    /// OpenDrone's default: the maintainer's Meteor65 Pro layout, adjusted
    /// (#31 §2), with Betaflight's default timers, warnings, statistics and
    /// alarms.
    ///
    /// ```text
    ///           012345678901234567890123456789
    ///      1  |                          B12 |  Blackbox log number
    ///      6  |             -+-              |  crosshairs
    ///      8  |           DISARMED           |
    ///     10  |          LOW BATTERY T 42    |  warnings, throttle
    ///     11  | L7:100               AIR     |  Link Quality, Flight Mode
    ///     12  | b3.48V    WHOOP 65   F01:23  |  average cell, Quad name, timer 2
    /// ```
    pub fn opendrone_default() -> OsdLayout {
        let mut layout = OsdLayout::betaflight_default();
        for (element, placement) in [
            (OsdElement::LogStatus, Placement::at(26, 1)),
            (OsdElement::Crosshairs, Placement::at(13, 6)),
            (OsdElement::Disarmed, Placement::at(11, 8)),
            (OsdElement::Warnings, Placement::at(10, 10)),
            (OsdElement::Throttle, Placement::at(22, 10)),
            (OsdElement::LinkQuality, Placement::at(1, 11)),
            (OsdElement::FlightMode, Placement::at(22, 11)),
            (OsdElement::AverageCellVoltage, Placement::at(1, 12)),
            (OsdElement::CraftName, Placement::at(11, 12)),
            (OsdElement::Timer2, Placement::at(22, 12)),
        ] {
            layout.place(element, placement);
        }
        layout
    }

    pub fn placement(&self, element: OsdElement) -> Placement {
        self.placements[element.index()]
    }

    /// Moves an element and shows or hides it; a place off the grid is
    /// brought onto its last column and row.
    pub fn place(&mut self, element: OsdElement, placement: Placement) {
        self.placements[element.index()] = Placement {
            column: placement.column.min(COLUMNS as u8 - 1),
            row: placement.row.min(ROWS as u8 - 1),
            visible: placement.visible,
        };
    }

    /// Shows every element where it stands: for a Scenario, whose
    /// Expectations read elements, never cells, so overlaps don't matter.
    pub fn show_everything(&mut self) {
        for placement in &mut self.placements {
            placement.visible = true;
        }
    }

    pub fn warning_on(&self, warning: Warning) -> bool {
        self.warnings & warning.bit() != 0
    }

    pub fn statistic_on(&self, statistic: Statistic) -> bool {
        self.statistics & statistic.bit() != 0
    }

    /// A picture of the layout: its 13 rows of 30 characters, each visible
    /// element drawn as its [`OsdElement::sample`], in Betaflight's drawing
    /// order. For the "Now / After" of a paste and the settings page's
    /// preview.
    pub fn picture(&self) -> Vec<String> {
        let mut grid = [[' '; COLUMNS]; ROWS];
        for element in OsdElement::DRAWING_ORDER {
            let placement = self.placement(element);
            if !placement.visible {
                continue;
            }
            let row = usize::from(placement.row);
            for (i, c) in element.sample().chars().enumerate() {
                if let Some(cell) = grid[row].get_mut(usize::from(placement.column) + i) {
                    *cell = c;
                }
            }
        }
        grid.iter().map(|row| row.iter().collect()).collect()
    }
}

impl Default for OsdLayout {
    fn default() -> OsdLayout {
        OsdLayout::opendrone_default()
    }
}
