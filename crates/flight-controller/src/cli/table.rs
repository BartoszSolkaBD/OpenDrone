//! The settings the Betaflight CLI translator knows: one row a setting, under
//! its Betaflight 2026.6 name, saying where its value comes from in each
//! Betaflight version the translator reads ([`Family::ALL`]).
//!
//! Adding a setting is adding its row to [`SETTINGS`]. Every row carries a
//! [`Source`] for each version, oldest first (4.3, 4.4, 4.5, 2025.12,
//! 2026.6):
//!
//! - [`Source::Same`]: the version has the setting under the same name, with
//!   the same meaning; the text is that version's default.
//! - [`Source::Was`]: the version has it under an older name (a rename), with
//!   that version's default.
//! - [`Source::Adr0008`]: the version lacked it; the value makes 2026.6 behave
//!   as the version did ([ADR-0008]).
//! - [`Source::Newer`]: the version lacked it and no value behaves like it (or
//!   any value would): 2026.6's default.
//! - [`Source::Rule`]: worked out from the version's own settings, where the
//!   meaning changed ([`Rule`]).
//!
//! 2026.6's own column is always [`Source::Same`] with Betaflight 2026.6.2's
//! default. Defaults were read from each version's source: `src/main/cli/
//! settings.c` for names and ranges, and each parameter group's reset values
//! (`pg/rx.c`, `pg/motor.c`, `flight/pid.c`, `flight/imu.c`,
//! `flight/mixer_init.c`, `flight/failsafe.c`, `sensors/gyro.c`,
//! `fc/rc_controls.c`, `fc/controlrate_profile.c`, `blackbox/blackbox.c`) at
//! tags 4.3.0, 4.4.0, 4.5.0, 2025.12.1 and 2026.6.2, for a build with every
//! flight feature on.
//!
//! A row is written into a Tune under its [`Place`] once the Flight
//! Controller reads it ([`crate::Tune::settings`]); until then the importer
//! keeps it under "Not simulated yet" when the export sets it. The table also
//! holds rows for the settings the next Flight Controller tickets read (#49
//! filters, RC smoothing and feedforward; #50 anti-gravity, I-term relax, TPA,
//! Dynamic D and throttle boost; #51 Angle and Horizon; #52 Failsafe; #54
//! Crash Flip and yaw spin recovery), so their Tunes come out of the importer
//! as soon as the Flight Controller reads them; those tickets check their
//! rows.
//!
//! [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md

use super::Family;

/// Where a setting sits in a Tune: by the Betaflight App's tabs, in their
/// order, each tab's settings that only the CLI holds after the ones it shows
/// ([ADR-0015]). The tabs are placed from the App as pilots know it.
///
/// [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    Configuration,
    ConfigurationCli,
    Failsafe,
    FailsafeCli,
    PidTuning,
    PidTuningCli,
    Filters,
    FiltersCli,
    Receiver,
    ReceiverCli,
    Motors,
    MotorsCli,
    Blackbox,
}

impl Place {
    /// The heading a Tune writes above the settings in this place.
    pub fn heading(self) -> &'static str {
        match self {
            Place::Configuration => "Configuration",
            Place::ConfigurationCli => "Configuration, CLI only",
            Place::Failsafe => "Failsafe",
            Place::FailsafeCli => "Failsafe, CLI only",
            Place::PidTuning => "PID Tuning",
            Place::PidTuningCli => "PID Tuning, CLI only",
            Place::Filters => "PID Tuning, filters",
            Place::FiltersCli => "PID Tuning, filters, CLI only",
            Place::Receiver => "Receiver",
            Place::ReceiverCli => "Receiver, CLI only",
            Place::Motors => "Motors",
            Place::MotorsCli => "Motors, CLI only",
            Place::Blackbox => "Blackbox",
        }
    }

    /// The tab's heading, without the CLI-only split: the "Not simulated
    /// yet" part of a Tune groups by tab alone.
    pub fn tab(self) -> &'static str {
        let heading = self.heading();
        heading.strip_suffix(", CLI only").unwrap_or(heading)
    }
}

/// Where one version's value for a 2026.6 setting comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The same name and meaning; the version's default.
    Same(&'static str),
    /// Renamed: the version's name for it, and its default.
    Was(&'static str, &'static str),
    /// The version lacked it; this value behaves as the version did
    /// (ADR-0008).
    Adr0008(&'static str),
    /// The version lacked it and no value behaves like it, or any would:
    /// 2026.6's default.
    Newer,
    /// Worked out from the version's own settings, given by their names in
    /// that version and their defaults there.
    Rule(Rule, &'static [(&'static str, &'static str)]),
}

/// How a setting whose meaning changed is worked out from the old version's
/// settings. Each takes its inputs in the order its row lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// `d_<axis>` from 4.3–4.5's `d_<axis>` and `d_min_<axis>`. Back then
    /// `d_min` was the base D and `d` the peak, and Dynamic D ran only when
    /// `d_min` was above 0 and below `d`; 2025.12 calls the base `d` and the
    /// peak `d_max`, and runs Dynamic D when `d_max` is above `d`. So with
    /// Dynamic D on, D is the old `d_min`; with it off, the old `d`.
    DBase,
    /// `d_max_<axis>` from the same two: the old `d` with Dynamic D on; with
    /// it off, the old `d_min` when that keeps it off (not above `d`), else 0.
    DPeak,
    /// `d_max_advance` from 4.3–4.5's `d_max_gain` and `d_max_advance`: the
    /// stick-driven boost was their product ÷ 100; from 2025.12 it's
    /// `d_max_advance` alone (the Betaflight research §6.3, #21: 37 × 20 ÷ 100
    /// ≈ 7).
    DMaxAdvance,
    /// `iterm_windup` from 4.3–4.5's `iterm_limit` and `pidsum_limit`: the I
    /// term's limit was `iterm_limit`; from 2025.12 it's `iterm_windup`
    /// percent of the PID-sum limit (the Betaflight research §3). 4.3–4.5's
    /// own `iterm_windup` meant something else (see [`RETIRED`]).
    ItermWindup,
}

/// How many versions each row covers: [`Family::ALL`]'s.
pub const VERSIONS: usize = 5;

/// One setting the translator knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    /// Its Betaflight 2026.6 name.
    pub name: &'static str,
    pub place: Place,
    /// A short note the Tune writes after the mark, such as its unit, or "".
    pub note: &'static str,
    /// Where its value comes from in each version, in [`Family::ALL`]'s
    /// order.
    pub from: [Source; VERSIONS],
}

impl Setting {
    /// Where its value comes from in `family`.
    pub fn source(&self, family: Family) -> Source {
        self.from[family.index()]
    }

    /// Betaflight 2026.6.2's default.
    pub fn default_2026(&self) -> &'static str {
        match self.from[Family::V2026_6.index()] {
            Source::Same(default) => default,
            _ => "",
        }
    }
}

const fn row(
    name: &'static str,
    place: Place,
    note: &'static str,
    from: [Source; VERSIONS],
) -> Setting {
    Setting {
        name,
        place,
        note,
        from,
    }
}

/// The same name and default in every version.
const fn all(
    name: &'static str,
    place: Place,
    note: &'static str,
    default: &'static str,
) -> Setting {
    row(name, place, note, [Source::Same(default); VERSIONS])
}

const fn rule(rule: Rule, inputs: &'static [(&'static str, &'static str)]) -> Source {
    Source::Rule(rule, inputs)
}

use Place::*;
use Source::{Adr0008, Newer, Same, Was};

const D_ROLL: &[(&str, &str)] = &[("d_roll", "40"), ("d_min_roll", "30")];
const D_PITCH: &[(&str, &str)] = &[("d_pitch", "46"), ("d_min_pitch", "34")];
const D_YAW: &[(&str, &str)] = &[("d_yaw", "0"), ("d_min_yaw", "0")];
const ADVANCE: &[(&str, &str)] = &[("d_max_gain", "37"), ("d_max_advance", "20")];
const WINDUP: &[(&str, &str)] = &[("iterm_limit", "400"), ("pidsum_limit", "500")];

/// Every setting the translator knows, in the order a Tune lists them: by
/// [`Place`], and within a place as the App shows them.
#[rustfmt::skip]
pub const SETTINGS: &[Setting] = &[
    // Configuration
    all("small_angle", Configuration, "degrees: the most it may tilt and still arm", "25"),
    all("yaw_spin_recovery", ConfigurationCli, "", "AUTO"),
    all("yaw_spin_threshold", ConfigurationCli, "°/s", "1950"),
    // Failsafe
    all("failsafe_delay", Failsafe, "tenths of a second", "15"),
    all("failsafe_procedure", Failsafe, "", "DROP"),
    all("failsafe_switch_mode", Failsafe, "", "STAGE1"),
    all("failsafe_throttle", FailsafeCli, "µs", "1000"),
    all("failsafe_throttle_low_delay", FailsafeCli, "tenths of a second", "100"),
    row("failsafe_recovery_delay", FailsafeCli, "tenths of a second",
        [Same("10"), Same("10"), Same("5"), Same("5"), Same("5")]),
    all("failsafe_stick_threshold", FailsafeCli, "percent", "30"),
    // PID Tuning: the PIDs, axis by axis, as the App's table shows them.
    // Until 2025.12, d_min was the base D and d the peak (see Rule::DBase).
    all("p_roll", PidTuning, "", "45"),
    all("i_roll", PidTuning, "", "80"),
    row("d_roll", PidTuning, "",
        [rule(Rule::DBase, D_ROLL), rule(Rule::DBase, D_ROLL), rule(Rule::DBase, D_ROLL), Same("30"), Same("30")]),
    row("d_max_roll", PidTuning, "",
        [rule(Rule::DPeak, D_ROLL), rule(Rule::DPeak, D_ROLL), rule(Rule::DPeak, D_ROLL), Same("40"), Same("40")]),
    all("f_roll", PidTuning, "", "120"),
    all("p_pitch", PidTuning, "", "47"),
    all("i_pitch", PidTuning, "", "84"),
    row("d_pitch", PidTuning, "",
        [rule(Rule::DBase, D_PITCH), rule(Rule::DBase, D_PITCH), rule(Rule::DBase, D_PITCH), Same("34"), Same("34")]),
    row("d_max_pitch", PidTuning, "",
        [rule(Rule::DPeak, D_PITCH), rule(Rule::DPeak, D_PITCH), rule(Rule::DPeak, D_PITCH), Same("46"), Same("46")]),
    all("f_pitch", PidTuning, "", "125"),
    all("p_yaw", PidTuning, "", "45"),
    all("i_yaw", PidTuning, "", "80"),
    row("d_yaw", PidTuning, "",
        [rule(Rule::DBase, D_YAW), rule(Rule::DBase, D_YAW), rule(Rule::DBase, D_YAW), Same("0"), Same("0")]),
    row("d_max_yaw", PidTuning, "",
        [rule(Rule::DPeak, D_YAW), rule(Rule::DPeak, D_YAW), rule(Rule::DPeak, D_YAW), Same("0"), Same("0")]),
    all("f_yaw", PidTuning, "", "120"),
    // Feedforward. Before 2025.12 there was no yaw hold: its gain 0 turns it
    // off.
    all("feedforward_transition", PidTuning, "", "0"),
    row("feedforward_averaging", PidTuning, "",
        [Same("OFF"), Same("OFF"), Same("OFF"), Same("2_POINT"), Same("2_POINT")]),
    row("feedforward_smooth_factor", PidTuning, "",
        [Same("25"), Same("25"), Same("25"), Same("65"), Same("65")]),
    all("feedforward_jitter_factor", PidTuning, "", "7"),
    all("feedforward_boost", PidTuning, "", "15"),
    all("feedforward_max_rate_limit", PidTuning, "", "90"),
    // Dynamic D.
    row("d_max_gain", PidTuning, "",
        [Same("37"), Same("37"), Same("37"), Same("37"), Same("0")]),
    row("d_max_advance", PidTuning, "",
        [rule(Rule::DMaxAdvance, ADVANCE), rule(Rule::DMaxAdvance, ADVANCE), rule(Rule::DMaxAdvance, ADVANCE), Same("20"), Same("35")]),
    // I-term relax, anti-gravity, TPA and throttle boost. 4.3's anti-gravity
    // worked differently (its gain 3500 meant ×3.5, with a mode and a
    // threshold) and no 2026.6 value behaves like it (ADR-0008's
    // consequences); 4.4 brought the one 2026.6 has. 4.3 kept TPA in its rate
    // profile (see TUNE_IN_RATE_PROFILE).
    all("iterm_relax", PidTuning, "", "RP"),
    all("iterm_relax_type", PidTuning, "", "SETPOINT"),
    all("iterm_relax_cutoff", PidTuning, "Hz", "15"),
    row("anti_gravity_gain", PidTuning, "",
        [Newer, Same("80"), Same("80"), Same("80"), Same("80")]),
    all("tpa_mode", PidTuning, "", "D"),
    all("tpa_rate", PidTuning, "percent", "65"),
    all("tpa_breakpoint", PidTuning, "µs", "1350"),
    all("throttle_boost", PidTuning, "", "5"),
    all("motor_output_limit", PidTuning, "percent", "100"),
    // Angle and Horizon. 4.5 rebuilt both; 4.3 and 4.4 named Angle's strength
    // angle_level_strength, its limit level_limit and Horizon's stick
    // transition horizon_transition, in the same places. They had no earth
    // reference: 0 turns it off.
    row("angle_p_gain", PidTuning, "",
        [Was("angle_level_strength", "50"), Was("angle_level_strength", "50"), Same("50"), Same("50"), Same("50")]),
    row("angle_limit", PidTuning, "degrees",
        [Was("level_limit", "55"), Was("level_limit", "55"), Same("60"), Same("60"), Same("60")]),
    row("horizon_level_strength", PidTuning, "",
        [Same("50"), Same("50"), Same("75"), Same("75"), Same("75")]),
    // PID Tuning, CLI only
    all("pidsum_limit", PidTuningCli, "1000 is the whole motor range", "500"),
    all("pidsum_limit_yaw", PidTuningCli, "", "400"),
    row("iterm_windup", PidTuningCli, "the I limit, in percent of the PID-sum limit",
        [rule(Rule::ItermWindup, WINDUP), rule(Rule::ItermWindup, WINDUP), rule(Rule::ItermWindup, WINDUP), Same("80"), Same("80")]),
    all("pid_at_min_throttle", PidTuningCli, "", "ON"),
    row("feedforward_yaw_hold_gain", PidTuningCli, "",
        [Adr0008("0"), Adr0008("0"), Adr0008("0"), Same("15"), Same("15")]),
    row("feedforward_yaw_hold_time", PidTuningCli, "ms",
        [Newer, Newer, Newer, Same("100"), Same("100")]),
    row("anti_gravity_cutoff_hz", PidTuningCli, "Hz",
        [Newer, Same("5"), Same("5"), Same("5"), Same("5")]),
    row("anti_gravity_p_gain", PidTuningCli, "",
        [Newer, Same("100"), Same("100"), Same("100"), Same("100")]),
    // Low-throttle TPA arrived in 4.5: a rate of 0 turns it off.
    row("tpa_low_rate", PidTuningCli, "percent",
        [Adr0008("0"), Adr0008("0"), Same("20"), Same("20"), Same("20")]),
    row("tpa_low_breakpoint", PidTuningCli, "µs",
        [Newer, Newer, Same("1050"), Same("1050"), Same("1050")]),
    row("tpa_low_always", PidTuningCli, "",
        [Newer, Newer, Same("OFF"), Same("OFF"), Same("OFF")]),
    all("throttle_boost_cutoff", PidTuningCli, "Hz", "15"),
    row("angle_feedforward", PidTuningCli, "",
        [Newer, Newer, Same("50"), Same("50"), Same("50")]),
    row("angle_feedforward_smoothing_ms", PidTuningCli, "ms",
        [Newer, Newer, Same("80"), Same("80"), Same("80")]),
    row("angle_earth_ref", PidTuningCli, "percent",
        [Adr0008("0"), Adr0008("0"), Same("100"), Same("100"), Same("100")]),
    row("horizon_limit_sticks", PidTuningCli, "",
        [Was("horizon_transition", "75"), Was("horizon_transition", "75"), Same("75"), Same("75"), Same("75")]),
    row("horizon_limit_degrees", PidTuningCli, "degrees",
        [Newer, Newer, Same("135"), Same("135"), Same("135")]),
    row("horizon_ignore_sticks", PidTuningCli, "",
        [Newer, Newer, Same("OFF"), Same("OFF"), Same("OFF")]),
    row("horizon_delay_ms", PidTuningCli, "ms",
        [Newer, Newer, Same("500"), Same("500"), Same("500")]),
    // PID Tuning, filters
    all("gyro_lpf1_type", Filters, "", "PT1"),
    all("gyro_lpf1_static_hz", Filters, "Hz; 0 is off", "250"),
    all("gyro_lpf1_dyn_min_hz", Filters, "Hz; 0 is off", "250"),
    all("gyro_lpf1_dyn_max_hz", Filters, "Hz", "500"),
    all("gyro_lpf2_type", Filters, "", "PT1"),
    all("gyro_lpf2_static_hz", Filters, "Hz; 0 is off", "500"),
    // The dynamic notch isn't in the alpha: it waits for gyro noise (#21).
    all("dyn_notch_count", Filters, "0 is off", "3"),
    all("dyn_notch_q", Filters, "", "300"),
    row("dyn_notch_min_hz", Filters, "Hz",
        [Same("150"), Same("100"), Same("100"), Same("100"), Same("100")]),
    all("dyn_notch_max_hz", Filters, "Hz", "600"),
    all("dterm_lpf1_type", Filters, "", "PT1"),
    all("dterm_lpf1_static_hz", Filters, "Hz; 0 is off", "75"),
    all("dterm_lpf1_dyn_min_hz", Filters, "Hz; 0 is off", "75"),
    all("dterm_lpf1_dyn_max_hz", Filters, "Hz", "150"),
    all("dterm_lpf2_type", Filters, "", "PT1"),
    all("dterm_lpf2_static_hz", Filters, "Hz; 0 is off", "150"),
    all("gyro_lpf1_dyn_expo", FiltersCli, "", "5"),
    all("dterm_lpf1_dyn_expo", FiltersCli, "", "5"),
    all("yaw_lowpass_hz", FiltersCli, "Hz; 0 is off", "100"),
    // Receiver
    all("min_check", Receiver, "µs", "1050"),
    all("mid_rc", Receiver, "µs", "1500"),
    all("deadband", Receiver, "µs", "0"),
    all("yaw_deadband", Receiver, "µs", "0"),
    all("rc_smoothing", Receiver, "", "ON"),
    all("rc_smoothing_auto_factor", Receiver, "", "30"),
    all("rc_smoothing_auto_factor_throttle", Receiver, "", "30"),
    all("rc_smoothing_setpoint_cutoff", Receiver, "Hz; 0 is automatic", "0"),
    all("rc_smoothing_throttle_cutoff", Receiver, "Hz; 0 is automatic", "0"),
    all("yaw_control_reversed", ReceiverCli, "", "OFF"),
    all("airmode_start_throttle_percent", ReceiverCli, "", "25"),
    // Motors. Before 4.5, Betaflight's own default protocol was DISABLED:
    // each board's settings chose one.
    row("motor_pwm_protocol", Motors, "",
        [Same("DISABLED"), Same("DISABLED"), Same("DSHOT600"), Same("DSHOT600"), Same("DSHOT600")]),
    row("motor_idle", Motors, "hundredths of a percent",
        [Was("dshot_idle_value", "550"), Was("dshot_idle_value", "550"), Was("dshot_idle_value", "550"), Same("550"), Same("550")]),
    all("motor_poles", Motors, "must match [motors] poles", "14"),
    all("yaw_motors_reversed", Motors, "must match [props] direction", "OFF"),
    all("mixer_type", MotorsCli, "", "LEGACY"),
    // Crash Flip. Before 2025.12 it had no rate fade: 0 turns it off. Leaving
    // Crash Flip changed then, and no setting brings the old way back (#26).
    all("crashflip_motor_percent", MotorsCli, "percent", "0"),
    row("crashflip_rate", MotorsCli, "°/s; 0 is off",
        [Adr0008("0"), Adr0008("0"), Adr0008("0"), Same("0"), Same("0")]),
    row("crashflip_auto_rearm", MotorsCli, "",
        [Newer, Newer, Newer, Same("OFF"), Same("OFF")]),
    // Blackbox
    all("blackbox_sample_rate", Blackbox, "", "1/4"),
];

/// Settings that sat in an older version's rate profile but belong to the
/// Tune: 4.3 kept TPA there; 4.4 moved it into the PID profile.
pub const TUNE_IN_RATE_PROFILE: &[(Family, &str)] = &[
    (Family::V4_3, "tpa_mode"),
    (Family::V4_3, "tpa_rate"),
    (Family::V4_3, "tpa_breakpoint"),
];

/// A setting of an older version that 2026.6 has no counterpart for, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retired {
    /// Its name in those versions.
    pub name: &'static str,
    pub versions: &'static [Family],
    pub why: &'static str,
}

const UP_TO_4_4: &[Family] = &[Family::V4_3, Family::V4_4];
const UP_TO_4_5: &[Family] = &[Family::V4_3, Family::V4_4, Family::V4_5];
const UP_TO_2025_12: &[Family] = &[Family::V4_3, Family::V4_4, Family::V4_5, Family::V2025_12];

/// Older settings with no 2026.6 counterpart. The importer leaves them out
/// and says why.
pub const RETIRED: &[Retired] = &[
    Retired {
        name: "iterm_windup",
        versions: UP_TO_4_5,
        why: "before 2025.12 it slowed the I term's growth once the motors passed that share of their range (on yaw only in 4.3); 2026.6 has no such thing, and its iterm_windup is the I limit, worked out from iterm_limit instead",
    },
    Retired {
        name: "anti_gravity_gain",
        versions: &[Family::V4_3],
        why: "4.3's anti-gravity worked differently (3500 meant ×3.5) and no 2026.6 value behaves like it, so the Tune takes 2026.6's default (ADR-0008's consequences)",
    },
    Retired {
        name: "anti_gravity_mode",
        versions: &[Family::V4_3],
        why: "4.3's anti-gravity worked differently, and 2026.6 has no mode",
    },
    Retired {
        name: "anti_gravity_threshold",
        versions: &[Family::V4_3],
        why: "4.3's anti-gravity worked differently, and 2026.6 has no threshold",
    },
    Retired {
        name: "abs_control_gain",
        versions: UP_TO_2025_12,
        why: "2026.6 has no absolute control",
    },
    Retired {
        name: "abs_control_limit",
        versions: UP_TO_2025_12,
        why: "2026.6 has no absolute control",
    },
    Retired {
        name: "abs_control_error_limit",
        versions: UP_TO_2025_12,
        why: "2026.6 has no absolute control",
    },
    Retired {
        name: "abs_control_cutoff",
        versions: UP_TO_2025_12,
        why: "2026.6 has no absolute control",
    },
    Retired {
        name: "crashflip_expo",
        versions: UP_TO_4_5,
        why: "2026.6 has none: its Crash Flip power is linear (#26)",
    },
    Retired {
        name: "failsafe_off_delay",
        versions: UP_TO_4_5,
        why: "it timed the AUTO-LAND procedure, which OpenDrone doesn't simulate (#21); 2026.6 times it in seconds as failsafe_landing_time",
    },
    Retired {
        name: "horizon_tilt_effect",
        versions: UP_TO_4_4,
        why: "2026.6's Horizon fades by horizon_limit_degrees instead, which works differently",
    },
    Retired {
        name: "horizon_tilt_expert_mode",
        versions: UP_TO_4_4,
        why: "2026.6's Horizon has no expert mode",
    },
    Retired {
        name: "min_throttle",
        versions: UP_TO_4_5,
        why: "it set the lowest output of analog ESC protocols; with DShot, motor_idle does, and 2026.6 has no min_throttle",
    },
    Retired {
        name: "rc_smoothing_feedforward_cutoff",
        versions: UP_TO_4_5,
        why: "2026.6 has none: feedforward_smooth_factor smooths feedforward",
    },
    Retired {
        name: "transient_throttle_limit",
        versions: UP_TO_2025_12,
        why: "2026.6 has none",
    },
    Retired {
        name: "dyn_idle_start_increase",
        versions: &[Family::V4_5],
        why: "2026.6 has none",
    },
    Retired {
        name: "pos_hold_without_mag",
        versions: &[Family::V2025_12],
        why: "2026.6 has none",
    },
];

/// Whether a setting only describes the board, its wiring or its other
/// hardware (OSD, VTX, LEDs, ports, receiver protocol, sensors, beeper,
/// `expresslrs_*`), or names the quad: the importer drops those (#21).
pub fn is_hardware_only(name: &str) -> bool {
    HARDWARE_ONLY.contains(&name)
        || HARDWARE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        // gyro_1_..., gyro_2_...: which gyro chip, where, and how it sits.
        || name
            .strip_prefix("gyro_")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

const HARDWARE_PREFIXES: &[&str] = &[
    "osd_",
    "vtx_",
    "vcd_",
    "max7456_",
    "displayport_",
    "led_",
    "ledstrip_",
    "beeper_",
    "beacon_",
    "expresslrs_",
    "serialrx_",
    "rx_spi_",
    "spektrum_",
    "sbus_",
    "crsf_",
    "srxl2_",
    "ibus_",
    "fport_",
    "frsky_",
    "hott_",
    "smartport_",
    "tlm_",
    "ltm_",
    "mavlink_",
    "msp_",
    "gps_",
    "mag_",
    "baro_",
    "rangefinder_",
    "opticalflow_",
    "pinio_",
    "acc_",
    "align_",
    "sdcard_",
    "flash_",
    "usb_",
    "dashboard_",
    "camera_control_",
    "rcdevice_",
    "box_user_",
    "ibata_",
    "ibatv_",
    "esc_sensor_",
    "dshot_bitbang",
    "dshot_burst",
    "transponder_",
    "scheduler_",
    "stats_",
    "rssi_",
    "position_",
    "altitude_",
    "dronecan_",
    "can_",
    "i2c",
    "gimbal_",
    "rx_min_usec",
    "rx_max_usec",
    "cpu_",
];

const HARDWARE_ONLY: &[&str] = &[
    "name",
    "craft_name",
    "pilot_name",
    "display_name",
    "profile_name",
    "rateprofile_name",
    "board_name",
    "manufacturer_id",
    "debug_mode",
    "task_statistics",
    "timezone_offset_minutes",
    "motor_pwm_rate",
    "motor_pwm_inversion",
    "use_unsynced_pwm",
    "dshot_edt",
    "pid_process_denom",
    "imu_process_denom",
    "gyro_to_use",
    "gyro_hardware_lpf",
    "gyro_high_range",
    "gyro_calib_duration",
    "gyro_calib_noise_limit",
    "gyro_offset_yaw",
    "gyro_overflow_detect",
    "gyro_filter_debug_axis",
    "gyro_enabled_bitmask",
    "rc_smoothing_debug_axis",
    "vbat_scale",
    "vbat_divider",
    "vbat_multiplier",
    "current_meter",
    "battery_meter",
    "serial_update_rate_hz",
    "system_hse_mhz",
    "mco2_on_pc9",
    "report_cell_voltage",
];

/// Settings every Tune spells out even before the Flight Controller reads
/// them, because the Pack checker compares them with the Quad definition
/// (ADR-0015).
pub const CHECKED_AGAINST_THE_QUAD: &[&str] = &["motor_poles", "yaw_motors_reversed"];

/// The `simplified_*` slider settings: the importer reads the final numbers
/// they set instead (#21).
pub fn is_slider(name: &str) -> bool {
    name.starts_with("simplified_")
}
