//! The Tune: the Flight Controller's settings, read from a Quad's `tune.txt`
//! (ADR-0015) under Betaflight 2026.6's CLI names and in Betaflight's own
//! units.
//!
//! This takes the Tune's `set` lines as name and value text, and gives the
//! settings out; it opens no files. Every setting this Flight Controller reads
//! must be there: a Tune spells out every setting, so no default hides in the
//! code ([ADR-0015]).
//!
//! One setting it knows but doesn't simulate yet ([`Tune::not_simulated_yet`]):
//! the dynamic notch, which waits for gyro noise (#21). A Tune may set it,
//! and its value is checked, but the Flight Controller flies as if it were
//! off; a Test Quad sets it off to fly exactly as Betaflight does. Settings
//! it doesn't know at all are left alone; later Flight Controller tickets
//! read more of them.
//!
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md

use std::collections::BTreeMap;

use opendrone_maths::Fingerprinter;

pub use crate::filter::LowpassType;

/// The Tune's settings this Flight Controller reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tune {
    /// `small_angle`: the most the Quad may be tilted to arm, in degrees.
    pub small_angle: u8,
    /// `yaw_spin_recovery`: whether a yaw spin past the threshold drops the
    /// I term and roll and pitch control and brakes the yaw.
    pub yaw_spin_recovery: YawSpinRecovery,
    /// `yaw_spin_threshold`: with `yaw_spin_recovery` ON, the yaw rate that
    /// starts it, in °/s (AUTO works its own out from the Rates).
    pub yaw_spin_threshold: u16,
    /// `runaway_takeoff_prevention`: whether a PID sum held high right after
    /// arming disarms the Quad.
    pub runaway_takeoff_prevention: bool,
    /// `runaway_takeoff_deactivate_delay`: how long the flight must count as
    /// stable before runaway takeoff prevention switches itself off, in ms.
    pub runaway_takeoff_deactivate_delay: u16,
    /// `runaway_takeoff_deactivate_throttle_percent`: the throttle, in
    /// percent, a stable flight needs with a stick moved (twice it, up to
    /// 75%, without).
    pub runaway_takeoff_deactivate_throttle_percent: u8,
    /// `current_meter`: where the battery current comes from. `ADC` and
    /// `ESC` read the physics' current; `NONE` has none; `VIRTUAL` and `MSP`
    /// aren't simulated yet, so they read none ([`Tune::not_simulated_values`]).
    pub current_meter: CurrentMeter,
    /// `vbat_min_cell_voltage`: below this a cell is critical (` LAND NOW`),
    /// in hundredths of a volt.
    pub vbat_min_cell_voltage: u16,
    /// `vbat_max_cell_voltage`: a full cell, in hundredths of a volt; the
    /// cell count is worked out against it at power-up.
    pub vbat_max_cell_voltage: u16,
    /// `vbat_warning_cell_voltage`: below this a cell is low (`LOW
    /// BATTERY`), in hundredths of a volt.
    pub vbat_warning_cell_voltage: u16,
    /// `vbat_full_cell_voltage`: below this, never armed since power-up, the
    /// OSD warns `BATT < FULL`, in hundredths of a volt.
    pub vbat_full_cell_voltage: u16,
    /// `force_battery_cell_count`: the cell count, or 0 to work it out.
    pub force_battery_cell_count: u8,
    /// `vbat_hysteresis`: how far below a threshold the voltage must fall
    /// before its state starts, in hundredths of a volt.
    pub vbat_hysteresis: u8,
    /// `vbat_detect_cell_voltage`: below this a reading counts as no battery,
    /// in hundredths of a volt.
    pub vbat_detect_cell_voltage: u16,
    /// `use_vbat_alerts`: whether a low or critical battery sounds the
    /// beeper.
    pub use_vbat_alerts: bool,
    /// `vbat_display_lpf_period`: the battery voltage's low-pass, as its
    /// period in tenths of a second.
    pub vbat_display_lpf_period: u8,
    /// `ibat_lpf_period`: the battery current's low-pass, as its period in
    /// tenths of a second (0: off).
    pub ibat_lpf_period: u8,
    /// `vbat_duration_for_warning`: how long the voltage must stay low
    /// before the battery counts as low, in tenths of a second.
    pub vbat_duration_for_warning: u8,
    /// `vbat_duration_for_critical`: the same for critical.
    pub vbat_duration_for_critical: u8,
    /// `rx_min_usec`: a Channel below this, in µs, is invalid. It is also
    /// the throttle Failsafe's stage 1 sets.
    pub rx_min_usec: u16,
    /// `rx_max_usec`: a Channel above this, in µs, is invalid.
    pub rx_max_usec: u16,
    /// `failsafe_delay`: how long after the last good frame Failsafe's stage
    /// 2 starts (DROP: the Quad disarms), in tenths of a second.
    pub failsafe_delay: u8,
    /// `failsafe_procedure`: only DROP is simulated; a Tune set to AUTO-LAND
    /// or GPS-RESCUE flies DROP ([`Tune::not_simulated_values`]).
    pub failsafe_procedure: FailsafeProcedure,
    /// `failsafe_throttle`: the throttle during stage 2, in µs.
    pub failsafe_throttle: u16,
    /// `failsafe_recovery_delay`: how long frames must arrive again before
    /// the link counts as back, in tenths of a second.
    pub failsafe_recovery_delay: u8,
    /// `p_roll`, `i_roll`, `d_roll`, `f_roll`, then pitch and yaw.
    pub pid: [Gains; 3],
    /// `d_max_roll`, `d_max_pitch`, `d_max_yaw`: the most Dynamic D may raise
    /// each axis's D to; at or below its D, Dynamic D is off on that axis.
    pub d_max: [u8; 3],
    /// `feedforward_transition`: below this stick deflection, in hundredths,
    /// each axis's feedforward (yaw's too) fades towards centre (0: no fade).
    pub feedforward_transition: u8,
    /// `feedforward_averaging`: how many frames' feedforward is averaged.
    pub feedforward_averaging: FeedforwardAveraging,
    /// `feedforward_smooth_factor`: how strongly the stick's speed is
    /// smoothed, in percent, at a 250 Hz Packet Rate.
    pub feedforward_smooth_factor: u8,
    /// `feedforward_jitter_factor`: feedforward is cut while the stick moves
    /// less than about this many µs a frame.
    pub feedforward_jitter_factor: u8,
    /// `feedforward_boost`: how much of the stick's acceleration is added, in
    /// tenths of a percent.
    pub feedforward_boost: u8,
    /// `feedforward_max_rate_limit`: near full stick, roll and pitch's
    /// feedforward is held back so it fades as the setpoint nears the max
    /// rate (0: off).
    pub feedforward_max_rate_limit: u8,
    /// `d_max_gain`: how strongly fast gyro movement raises D towards D max
    /// (0: only the sticks do).
    pub d_max_gain: u8,
    /// `d_max_advance`: how strongly fast stick movement raises D towards D
    /// max (0: only the gyro does).
    pub d_max_advance: u8,
    /// `iterm_relax`: on which axes I-term relax holds I back during fast
    /// stick moves, and whether it still lets I shrink meanwhile (the `_INC`
    /// kinds).
    pub iterm_relax: ItermRelax,
    /// `iterm_relax_type`: how it holds I back.
    pub iterm_relax_type: ItermRelaxType,
    /// `iterm_relax_cutoff`: the cutoff of the low-pass that tells a fast
    /// stick move from a slow one, in Hz.
    pub iterm_relax_cutoff: u8,
    /// `anti_gravity_gain`: how strongly a fast throttle change boosts I (and
    /// a little P). The code uses the number as it stands, 80 as 80; only
    /// the Betaflight App shows it in tenths, as 8.0 (0: off).
    pub anti_gravity_gain: u8,
    /// `tpa_mode`: which terms TPA cuts.
    pub tpa_mode: TpaMode,
    /// `tpa_rate`: how much TPA cuts at full throttle, in percent.
    pub tpa_rate: u8,
    /// `tpa_breakpoint`: the throttle where TPA starts, in µs.
    pub tpa_breakpoint: u16,
    /// `throttle_boost`: how strongly a fast throttle change is pushed
    /// further, in tenths (0: off).
    pub throttle_boost: u8,
    /// `motor_output_limit`: the top of the motors' range, in percent.
    pub motor_output_limit: u8,
    /// `angle_p_gain`: how hard Angle and Horizon turn the Quad towards the
    /// angle the sticks ask for, in tenths of °/s for each degree off.
    pub angle_p_gain: u8,
    /// `angle_limit`: the tilt full stick asks for in Angle and Horizon, in
    /// degrees.
    pub angle_limit: u8,
    /// `horizon_level_strength`: how much Horizon levels at most, in percent
    /// (100 or more levels as hard as Angle).
    pub horizon_level_strength: u8,
    /// `pidsum_limit`: the most roll or pitch may ask of the motors, where
    /// 1000 is the whole motor range.
    pub pidsum_limit: u16,
    /// `pidsum_limit_yaw`: the same for yaw.
    pub pidsum_limit_yaw: u16,
    /// `iterm_windup`: the I term's limit, in percent of the PID-sum limit.
    pub iterm_windup: u8,
    /// `pid_at_min_throttle`: whether P and D keep working at low throttle
    /// before Airmode starts.
    pub pid_at_min_throttle: bool,
    /// `feedforward_yaw_hold_gain`: how strongly yaw's feedforward keeps
    /// pushing while the yaw stick holds after a move (0: off).
    pub feedforward_yaw_hold_gain: u8,
    /// `feedforward_yaw_hold_time`: how long yaw's hold lasts: its time
    /// constant, in ms.
    pub feedforward_yaw_hold_time: u8,
    /// `anti_gravity_cutoff_hz`: the cutoff of the low-pass that smooths the
    /// throttle's speed for anti-gravity, in Hz.
    pub anti_gravity_cutoff_hz: u8,
    /// `anti_gravity_p_gain`: how much of anti-gravity reaches P, in percent.
    pub anti_gravity_p_gain: u8,
    /// `tpa_low_rate`: how much low-throttle TPA cuts at zero throttle, in
    /// percent (0: off).
    pub tpa_low_rate: u8,
    /// `tpa_low_breakpoint`: the throttle where low-throttle TPA ends, in µs.
    pub tpa_low_breakpoint: u16,
    /// `tpa_low_always`: whether low-throttle TPA keeps working after the
    /// throttle first passes its breakpoint (OFF: it stops until the next
    /// power-up).
    pub tpa_low_always: bool,
    /// `throttle_boost_cutoff`: the cutoff of the low-pass whose difference
    /// from the throttle throttle boost adds, in Hz.
    pub throttle_boost_cutoff: u8,
    /// `angle_feedforward`: how much of the sticks' feedforward pushes Angle
    /// and Horizon's tilt along, in percent.
    pub angle_feedforward: u8,
    /// `angle_feedforward_smoothing_ms`: how strongly that push is smoothed:
    /// its low-pass's time constant, in ms.
    pub angle_feedforward_smoothing_ms: u8,
    /// `angle_earth_ref`: how much a yaw while tilted in Angle and Horizon
    /// turns about the world's vertical instead of the Quad's own, in
    /// percent (0: off).
    pub angle_earth_ref: u8,
    /// `horizon_limit_sticks`: the roll or pitch stick, in hundredths, at
    /// which Horizon stops levelling.
    pub horizon_limit_sticks: u8,
    /// `horizon_limit_degrees`: the tilt, in degrees, at which Horizon stops
    /// levelling.
    pub horizon_limit_degrees: u8,
    /// `horizon_ignore_sticks`: whether Horizon levels as hard whatever the
    /// sticks do.
    pub horizon_ignore_sticks: bool,
    /// `horizon_delay_ms`: how slowly Horizon's levelling comes back after it
    /// fades: its low-pass's time constant, in ms.
    pub horizon_delay_ms: u16,
    /// `level_race_mode`: whether Angle and Horizon level only the roll, and
    /// leave pitch to the sticks as in Acro.
    pub level_race_mode: bool,
    /// `gyro_lpf1_type`: the gyro's first low-pass.
    pub gyro_lpf1_type: LowpassType,
    /// `gyro_lpf1_static_hz`: its cutoff when it isn't dynamic, in Hz (0:
    /// off, unless it's dynamic).
    pub gyro_lpf1_static_hz: u16,
    /// `gyro_lpf1_dyn_min_hz`: its lowest cutoff, at zero throttle, in Hz (0:
    /// not dynamic).
    pub gyro_lpf1_dyn_min_hz: u16,
    /// `gyro_lpf1_dyn_max_hz`: its highest cutoff, at full throttle, in Hz.
    pub gyro_lpf1_dyn_max_hz: u16,
    /// `gyro_lpf2_type`: the gyro's second low-pass.
    pub gyro_lpf2_type: LowpassType,
    /// `gyro_lpf2_static_hz`: its cutoff, in Hz (0: off).
    pub gyro_lpf2_static_hz: u16,
    /// `dterm_lpf1_type`: the D term's first low-pass.
    pub dterm_lpf1_type: LowpassType,
    /// `dterm_lpf1_static_hz`: its cutoff when it isn't dynamic, in Hz (0:
    /// off, unless it's dynamic).
    pub dterm_lpf1_static_hz: u16,
    /// `dterm_lpf1_dyn_min_hz`: its lowest cutoff, in Hz (0: not dynamic).
    pub dterm_lpf1_dyn_min_hz: u16,
    /// `dterm_lpf1_dyn_max_hz`: its highest cutoff, in Hz.
    pub dterm_lpf1_dyn_max_hz: u16,
    /// `dterm_lpf2_type`: the D term's second low-pass.
    pub dterm_lpf2_type: LowpassType,
    /// `dterm_lpf2_static_hz`: its cutoff, in Hz (0: off).
    pub dterm_lpf2_static_hz: u16,
    /// `gyro_lpf1_dyn_expo`: the curve of the gyro's dynamic low-pass from
    /// its lowest cutoff to its highest (0: Betaflight's older curve).
    pub gyro_lpf1_dyn_expo: u8,
    /// `dterm_lpf1_dyn_expo`: the same for the D term's.
    pub dterm_lpf1_dyn_expo: u8,
    /// `yaw_lowpass_hz`: the cutoff of the low-pass on yaw's P term, in Hz
    /// (0: off).
    pub yaw_lowpass_hz: u16,
    /// `min_check`: below this, in µs, the throttle counts as low.
    pub min_check: u16,
    /// `mid_rc`: the sticks' centre, in µs.
    pub mid_rc: u16,
    /// `deadband`: roll and pitch's deadband, in µs.
    pub deadband: u8,
    /// `yaw_deadband`: yaw's deadband, in µs.
    pub yaw_deadband: u8,
    /// `rc_smoothing`: whether the setpoints, the throttle and feedforward
    /// are smoothed between Radio Link frames.
    pub rc_smoothing: bool,
    /// `rc_smoothing_auto_factor`: how smooth the setpoints' automatic
    /// cutoff is; higher is smoother.
    pub rc_smoothing_auto_factor: u8,
    /// `rc_smoothing_auto_factor_throttle`: the same for the throttle.
    pub rc_smoothing_auto_factor_throttle: u8,
    /// `rc_smoothing_setpoint_cutoff`: the setpoints' cutoff, in Hz (0:
    /// worked out from the Packet Rate).
    pub rc_smoothing_setpoint_cutoff: u8,
    /// `rc_smoothing_throttle_cutoff`: the throttle's cutoff, in Hz (0:
    /// worked out from the Packet Rate).
    pub rc_smoothing_throttle_cutoff: u8,
    /// `yaw_control_reversed`.
    pub yaw_control_reversed: bool,
    /// `airmode_start_throttle_percent`: Airmode starts once the throttle
    /// first passes this, in percent, after arming.
    pub airmode_start_throttle_percent: u8,
    /// `mixer_type`: only Legacy is simulated.
    pub mixer_type: MixerType,
    /// `motor_poles`: the motors' magnet poles, which turn the ESCs'
    /// electrical RPM into RPM.
    pub motor_poles: u8,
    /// `yaw_motors_reversed`: props out.
    pub yaw_motors_reversed: bool,
    /// `motor_pwm_protocol`: one of the DShot speeds.
    pub motor_pwm_protocol: MotorProtocol,
    /// `dshot_bidir`: bidirectional DShot, which brings each motor's RPM
    /// back from its ESC.
    pub dshot_bidir: bool,
    /// `motor_idle`: the motors' idle, in hundredths of a percent (550 is
    /// 5.5%).
    pub motor_idle: u16,
    /// `crashflip_motor_percent`: in Crash Flip, the share of its mix, in
    /// percent, a motor that would push the wrong way gets (0: it stays
    /// stopped).
    pub crashflip_motor_percent: u8,
    /// `crashflip_rate`: in Crash Flip, the roll or pitch rate, in tens of
    /// °/s, at which power has faded to nothing (0: no fade).
    pub crashflip_rate: u8,
    /// `crashflip_auto_rearm`: whether turning the Crash Flip switch off
    /// while armed keeps the Quad armed, flying normally, instead of
    /// disarming it.
    pub crashflip_auto_rearm: bool,
}

/// `yaw_spin_recovery`, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YawSpinRecovery {
    Off,
    /// Starts at `yaw_spin_threshold`.
    On,
    /// Starts at the Rates' max yaw rate plus a quarter of it, or 200 °/s if
    /// that's more, held within 500–1950 °/s.
    Auto,
}

/// `iterm_relax`, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItermRelax {
    Off,
    /// Roll and pitch.
    Rp,
    /// Roll, pitch and yaw.
    Rpy,
    /// Roll and pitch, and I may still shrink during a fast move.
    RpInc,
    /// Roll, pitch and yaw, and I may still shrink during a fast move.
    RpyInc,
}

impl ItermRelax {
    /// Betaflight's words for it, in its order.
    pub const WORDS: [&'static str; 5] = ["OFF", "RP", "RPY", "RP_INC", "RPY_INC"];
}

/// `iterm_relax_type`, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItermRelaxType {
    /// I grows only by how far the gyro is from the smoothed setpoint, less
    /// the stick's quick part.
    Gyro,
    /// I grows less the faster the setpoint moves.
    Setpoint,
}

impl ItermRelaxType {
    /// Betaflight's words for it, in its order.
    pub const WORDS: [&'static str; 2] = ["GYRO", "SETPOINT"];
}

/// `tpa_mode`, in Betaflight's order for a Quad (`PDS` is for wings only, and
/// a Quad's Betaflight doesn't offer it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TpaMode {
    /// TPA cuts P and D.
    Pd,
    /// TPA cuts D only.
    D,
}

impl TpaMode {
    /// Betaflight's words for it, in its order.
    pub const WORDS: [&'static str; 2] = ["PD", "D"];
}

/// One axis's PID gains, as Betaflight stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gains {
    pub p: u8,
    pub i: u8,
    pub d: u8,
    /// Feedforward, from 0 to 1000.
    pub f: u16,
}

/// `feedforward_averaging`, in Betaflight's order: feedforward averaged over
/// this many frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedforwardAveraging {
    Off,
    TwoPoint,
    ThreePoint,
    FourPoint,
}

impl FeedforwardAveraging {
    /// Betaflight's words for it, in its order.
    pub const WORDS: [&'static str; 4] = ["OFF", "2_POINT", "3_POINT", "4_POINT"];

    /// How many frames it averages: 1 when off.
    pub fn frames(self) -> usize {
        match self {
            FeedforwardAveraging::Off => 1,
            FeedforwardAveraging::TwoPoint => 2,
            FeedforwardAveraging::ThreePoint => 3,
            FeedforwardAveraging::FourPoint => 4,
        }
    }
}

/// `failsafe_procedure`, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailsafeProcedure {
    /// `AUTO-LAND`: not simulated yet, so it flies DROP.
    AutoLand,
    /// `DROP`: the Quad disarms and falls.
    Drop,
    /// `GPS-RESCUE`: not simulated yet, so it flies DROP.
    GpsRescue,
}

impl FailsafeProcedure {
    /// Betaflight's word for it.
    pub fn word(self) -> &'static str {
        match self {
            FailsafeProcedure::AutoLand => "AUTO-LAND",
            FailsafeProcedure::Drop => "DROP",
            FailsafeProcedure::GpsRescue => "GPS-RESCUE",
        }
    }
}

/// `current_meter`, in Betaflight's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentMeter {
    None,
    /// The board's current sensor: the battery current the physics gives.
    Adc,
    /// Worked out from the throttle: not simulated yet, so no current.
    Virtual,
    /// The ESCs' telemetry: the battery current the physics gives.
    Esc,
    /// From MSP: not simulated yet, so no current.
    Msp,
}

impl CurrentMeter {
    /// Betaflight's words for it, in its order.
    pub const WORDS: [&'static str; 5] = ["NONE", "ADC", "VIRTUAL", "ESC", "MSP"];

    /// Whether the Flight Controller reads a current: the meter is one the
    /// sim simulates.
    pub fn measures(self) -> bool {
        matches!(self, CurrentMeter::Adc | CurrentMeter::Esc)
    }

    fn place(self) -> u64 {
        match self {
            CurrentMeter::None => 0,
            CurrentMeter::Adc => 1,
            CurrentMeter::Virtual => 2,
            CurrentMeter::Esc => 3,
            CurrentMeter::Msp => 4,
        }
    }
}

/// `mixer_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixerType {
    Legacy,
}

/// `motor_pwm_protocol`: the DShot speeds. The Flight Controller sends every
/// one the same way; the ESC finds which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotorProtocol {
    Dshot150,
    Dshot300,
    Dshot600,
}

/// What a Tune lacks, or says that this Flight Controller can't read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TuneProblems {
    /// Settings it reads that the Tune doesn't set, in [`Tune::settings`]'s
    /// order.
    pub missing: Vec<&'static str>,
    /// Settings whose value it can't read, each with a plain sentence.
    pub wrong: Vec<(&'static str, String)>,
}

impl TuneProblems {
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.wrong.is_empty()
    }
}

/// How one setting's value is written.
enum Kind {
    /// A whole number from the first to the second, inclusive.
    Number(u32, u32),
    /// `OFF` or `ON`.
    OffOn,
    /// One of these words; any other Betaflight word is refused with the
    /// reason.
    Choice(&'static [&'static str], &'static str),
    /// One of these words: every one Betaflight knows. Those OpenDrone
    /// doesn't simulate yet are read all the same, and
    /// [`Tune::not_simulated_values`] says what flies instead.
    Word(&'static [&'static str]),
}

/// Every setting this Flight Controller reads, grouped as Betaflight's App
/// shows them, tab by tab (Configuration, Failsafe, PID Tuning, its filters,
/// Receiver, Motors), with each tab's settings that only the CLI holds after
/// the ones it shows. The ranges are Betaflight 2026.6.2's
/// (`src/main/cli/settings.c`).
const SPECS: &[(&str, Kind)] = &[
    ("small_angle", Kind::Number(0, 180)),
    ("yaw_spin_recovery", Kind::Word(&["OFF", "ON", "AUTO"])),
    ("yaw_spin_threshold", Kind::Number(500, 1950)),
    ("runaway_takeoff_prevention", Kind::OffOn),
    ("runaway_takeoff_deactivate_delay", Kind::Number(100, 1000)),
    (
        "runaway_takeoff_deactivate_throttle_percent",
        Kind::Number(0, 100),
    ),
    ("current_meter", Kind::Word(&CurrentMeter::WORDS)),
    ("vbat_min_cell_voltage", Kind::Number(100, 500)),
    ("vbat_max_cell_voltage", Kind::Number(100, 500)),
    ("vbat_warning_cell_voltage", Kind::Number(100, 500)),
    ("vbat_full_cell_voltage", Kind::Number(100, 500)),
    ("force_battery_cell_count", Kind::Number(0, 24)),
    ("vbat_hysteresis", Kind::Number(0, 250)),
    ("vbat_detect_cell_voltage", Kind::Number(0, 2000)),
    ("use_vbat_alerts", Kind::OffOn),
    ("vbat_display_lpf_period", Kind::Number(1, 255)),
    ("ibat_lpf_period", Kind::Number(0, 255)),
    ("vbat_duration_for_warning", Kind::Number(0, 150)),
    ("vbat_duration_for_critical", Kind::Number(0, 150)),
    ("rx_min_usec", Kind::Number(750, 2250)),
    ("rx_max_usec", Kind::Number(750, 2250)),
    ("failsafe_delay", Kind::Number(1, 200)),
    (
        "failsafe_procedure",
        Kind::Word(&["AUTO-LAND", "DROP", "GPS-RESCUE"]),
    ),
    ("failsafe_throttle", Kind::Number(750, 2250)),
    ("failsafe_recovery_delay", Kind::Number(1, 200)),
    ("p_roll", Kind::Number(0, 250)),
    ("i_roll", Kind::Number(0, 250)),
    ("d_roll", Kind::Number(0, 250)),
    ("d_max_roll", Kind::Number(0, 250)),
    ("f_roll", Kind::Number(0, 1000)),
    ("p_pitch", Kind::Number(0, 250)),
    ("i_pitch", Kind::Number(0, 250)),
    ("d_pitch", Kind::Number(0, 250)),
    ("d_max_pitch", Kind::Number(0, 250)),
    ("f_pitch", Kind::Number(0, 1000)),
    ("p_yaw", Kind::Number(0, 250)),
    ("i_yaw", Kind::Number(0, 250)),
    ("d_yaw", Kind::Number(0, 250)),
    ("d_max_yaw", Kind::Number(0, 250)),
    ("f_yaw", Kind::Number(0, 1000)),
    ("feedforward_transition", Kind::Number(0, 100)),
    (
        "feedforward_averaging",
        Kind::Word(&FeedforwardAveraging::WORDS),
    ),
    ("feedforward_smooth_factor", Kind::Number(0, 95)),
    ("feedforward_jitter_factor", Kind::Number(0, 20)),
    ("feedforward_boost", Kind::Number(0, 50)),
    ("feedforward_max_rate_limit", Kind::Number(0, 200)),
    ("d_max_gain", Kind::Number(0, 100)),
    ("d_max_advance", Kind::Number(0, 200)),
    ("iterm_relax", Kind::Word(&ItermRelax::WORDS)),
    ("iterm_relax_type", Kind::Word(&ItermRelaxType::WORDS)),
    ("iterm_relax_cutoff", Kind::Number(1, 50)),
    ("anti_gravity_gain", Kind::Number(0, 250)),
    ("tpa_mode", Kind::Word(&TpaMode::WORDS)),
    ("tpa_rate", Kind::Number(0, 100)),
    ("tpa_breakpoint", Kind::Number(1000, 2000)),
    ("throttle_boost", Kind::Number(0, 100)),
    ("motor_output_limit", Kind::Number(1, 100)),
    // Angle and Horizon: each range below, down to `level_race_mode`, was
    // checked against Betaflight 2026.6.2's `settings.c` (#192).
    ("angle_p_gain", Kind::Number(0, 200)),
    ("angle_limit", Kind::Number(10, 80)),
    ("horizon_level_strength", Kind::Number(0, 100)),
    ("pidsum_limit", Kind::Number(100, 1000)),
    ("pidsum_limit_yaw", Kind::Number(100, 1000)),
    ("iterm_windup", Kind::Number(20, 100)),
    ("pid_at_min_throttle", Kind::OffOn),
    ("feedforward_yaw_hold_gain", Kind::Number(0, 100)),
    ("feedforward_yaw_hold_time", Kind::Number(10, 250)),
    ("anti_gravity_cutoff_hz", Kind::Number(2, 50)),
    ("anti_gravity_p_gain", Kind::Number(0, 250)),
    ("tpa_low_rate", Kind::Number(0, 100)),
    ("tpa_low_breakpoint", Kind::Number(1000, 2000)),
    ("tpa_low_always", Kind::OffOn),
    ("throttle_boost_cutoff", Kind::Number(5, 50)),
    ("angle_feedforward", Kind::Number(0, 200)),
    ("angle_feedforward_smoothing_ms", Kind::Number(10, 250)),
    ("angle_earth_ref", Kind::Number(0, 100)),
    ("horizon_limit_sticks", Kind::Number(10, 200)),
    ("horizon_limit_degrees", Kind::Number(10, 250)),
    ("horizon_ignore_sticks", Kind::OffOn),
    ("horizon_delay_ms", Kind::Number(10, 5000)),
    ("level_race_mode", Kind::OffOn),
    ("gyro_lpf1_type", Kind::Word(&LowpassType::WORDS)),
    ("gyro_lpf1_static_hz", Kind::Number(0, 1000)),
    ("gyro_lpf1_dyn_min_hz", Kind::Number(0, 1000)),
    ("gyro_lpf1_dyn_max_hz", Kind::Number(0, 1000)),
    ("gyro_lpf2_type", Kind::Word(&LowpassType::WORDS)),
    ("gyro_lpf2_static_hz", Kind::Number(0, 1000)),
    ("dterm_lpf1_type", Kind::Word(&LowpassType::WORDS)),
    ("dterm_lpf1_static_hz", Kind::Number(0, 1000)),
    ("dterm_lpf1_dyn_min_hz", Kind::Number(0, 1000)),
    ("dterm_lpf1_dyn_max_hz", Kind::Number(0, 1000)),
    ("dterm_lpf2_type", Kind::Word(&LowpassType::WORDS)),
    ("dterm_lpf2_static_hz", Kind::Number(0, 1000)),
    ("gyro_lpf1_dyn_expo", Kind::Number(0, 10)),
    ("dterm_lpf1_dyn_expo", Kind::Number(0, 10)),
    ("yaw_lowpass_hz", Kind::Number(0, 500)),
    ("min_check", Kind::Number(750, 2250)),
    ("mid_rc", Kind::Number(1200, 1700)),
    ("deadband", Kind::Number(0, 32)),
    ("yaw_deadband", Kind::Number(0, 100)),
    ("rc_smoothing", Kind::OffOn),
    ("rc_smoothing_auto_factor", Kind::Number(0, 250)),
    ("rc_smoothing_auto_factor_throttle", Kind::Number(0, 250)),
    ("rc_smoothing_setpoint_cutoff", Kind::Number(0, 255)),
    ("rc_smoothing_throttle_cutoff", Kind::Number(0, 255)),
    ("yaw_control_reversed", Kind::OffOn),
    ("airmode_start_throttle_percent", Kind::Number(0, 100)),
    (
        "motor_pwm_protocol",
        Kind::Choice(
            &["DSHOT150", "DSHOT300", "DSHOT600"],
            "the simulated ESCs run Bluejay, which reads DShot: DSHOT150, DSHOT300 or DSHOT600",
        ),
    ),
    ("dshot_bidir", Kind::OffOn),
    ("motor_idle", Kind::Number(0, 2000)),
    ("motor_poles", Kind::Number(4, 255)),
    ("yaw_motors_reversed", Kind::OffOn),
    (
        "mixer_type",
        Kind::Choice(
            &["LEGACY"],
            "only the Legacy mixer is simulated; LINEAR, DYNAMIC and EZLANDING aren't",
        ),
    ),
    ("crashflip_motor_percent", Kind::Number(0, 100)),
    ("crashflip_rate", Kind::Number(0, 250)),
    ("crashflip_auto_rearm", Kind::OffOn),
];

/// Settings this Flight Controller knows but doesn't simulate yet, with
/// their ticket. A Tune may set them, and each value is checked as Betaflight
/// 2026.6.2's CLI checks it, but nothing reads them yet, so the Flight
/// Controller flies as if each were off. A Test Quad that sets them off
/// (Betaflight's way: a cutoff, gain or count of 0, `OFF`) flies exactly as
/// Betaflight does now, and stays so once their tickets land. Grouped as the
/// Betaflight App shows them.
const NOT_SIMULATED_YET: &[(&str, Kind, &str)] = &[
    // PID Tuning, filters: the dynamic notch isn't in the alpha. Like the
    // RPM filter, it comes only once gyro noise is simulated (#21, #37).
    (
        "dyn_notch_count",
        Kind::Number(0, 7),
        "waits for gyro noise, #21",
    ),
];

/// Betaflight's words for every value of the two lookups read here, so a
/// value Betaflight knows but OpenDrone doesn't simulate is told apart from a
/// typo.
const KNOWN_WORDS: &[(&str, &[&str])] = &[
    ("mixer_type", &["LEGACY", "LINEAR", "DYNAMIC", "EZLANDING"]),
    (
        "motor_pwm_protocol",
        &[
            "PWM",
            "ONESHOT125",
            "ONESHOT42",
            "MULTISHOT",
            "BRUSHED",
            "DSHOT150",
            "DSHOT300",
            "DSHOT600",
            "PROSHOT1000",
            "DISABLED",
            "DRONECAN",
        ],
    ),
];

impl Tune {
    /// The name of every setting this Flight Controller reads, in the order a
    /// Tune lists them.
    pub fn settings() -> Vec<&'static str> {
        SPECS.iter().map(|(name, _)| *name).collect()
    }

    /// The settings this Flight Controller knows but doesn't simulate yet,
    /// each with the ticket that brings it, in the order a Tune lists them.
    /// A Tune may set them; until their tickets land the Flight Controller
    /// flies as if each were off.
    pub fn not_simulated_yet() -> Vec<(&'static str, &'static str)> {
        NOT_SIMULATED_YET
            .iter()
            .map(|(name, _, ticket)| (*name, *ticket))
            .collect()
    }

    /// Checks one setting's value, if it is one this Flight Controller reads
    /// or knows it will read: a plain sentence when it can't read it.
    pub fn check(name: &str, value: &str) -> Result<(), String> {
        let known = SPECS
            .iter()
            .map(|(n, kind)| (*n, kind))
            .chain(NOT_SIMULATED_YET.iter().map(|(n, kind, _)| (*n, kind)))
            .find(|(n, _)| *n == name);
        match known {
            Some((name, kind)) => self::value(name, kind, value.trim()).map(|_| ()),
            None => Ok(()),
        }
    }

    /// Reads the settings from a Tune's `set` lines, given as name and value
    /// text (such as `p_roll` and `45`). Settings it doesn't read are left
    /// alone. Every problem is listed at once.
    pub fn read<'a>(
        lines: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Tune, TuneProblems> {
        let given: BTreeMap<&str, &str> = lines.into_iter().collect();
        let mut problems = TuneProblems::default();
        let mut values: BTreeMap<&str, u32> = BTreeMap::new();
        for (name, kind) in SPECS {
            let Some(text) = given.get(name) else {
                problems.missing.push(name);
                continue;
            };
            match value(name, kind, text.trim()) {
                Ok(v) => {
                    values.insert(name, v);
                }
                Err(sentence) => problems.wrong.push((name, sentence)),
            }
        }
        // The settings it doesn't simulate yet needn't be there, but any that
        // are must hold a value Betaflight would take.
        for (name, kind, _) in NOT_SIMULATED_YET {
            if let Some(text) = given.get(name)
                && let Err(sentence) = value(name, kind, text.trim())
            {
                problems.wrong.push((name, sentence));
            }
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        let n = |name: &str| values[name];
        let small = |name: &str| n(name) as u8;
        let hz = |name: &str| n(name) as u16;
        let gains = |axis: &str| Gains {
            p: small(&format!("p_{axis}")),
            i: small(&format!("i_{axis}")),
            d: small(&format!("d_{axis}")),
            f: hz(&format!("f_{axis}")),
        };
        Ok(Tune {
            small_angle: small("small_angle"),
            yaw_spin_recovery: match n("yaw_spin_recovery") {
                0 => YawSpinRecovery::Off,
                1 => YawSpinRecovery::On,
                _ => YawSpinRecovery::Auto,
            },
            yaw_spin_threshold: n("yaw_spin_threshold") as u16,
            runaway_takeoff_prevention: n("runaway_takeoff_prevention") == 1,
            runaway_takeoff_deactivate_delay: n("runaway_takeoff_deactivate_delay") as u16,
            runaway_takeoff_deactivate_throttle_percent: small(
                "runaway_takeoff_deactivate_throttle_percent",
            ),
            current_meter: match n("current_meter") {
                0 => CurrentMeter::None,
                1 => CurrentMeter::Adc,
                2 => CurrentMeter::Virtual,
                3 => CurrentMeter::Esc,
                _ => CurrentMeter::Msp,
            },
            vbat_min_cell_voltage: n("vbat_min_cell_voltage") as u16,
            vbat_max_cell_voltage: n("vbat_max_cell_voltage") as u16,
            vbat_warning_cell_voltage: n("vbat_warning_cell_voltage") as u16,
            vbat_full_cell_voltage: n("vbat_full_cell_voltage") as u16,
            force_battery_cell_count: small("force_battery_cell_count"),
            vbat_hysteresis: small("vbat_hysteresis"),
            vbat_detect_cell_voltage: n("vbat_detect_cell_voltage") as u16,
            use_vbat_alerts: n("use_vbat_alerts") == 1,
            vbat_display_lpf_period: small("vbat_display_lpf_period"),
            ibat_lpf_period: small("ibat_lpf_period"),
            vbat_duration_for_warning: small("vbat_duration_for_warning"),
            vbat_duration_for_critical: small("vbat_duration_for_critical"),
            rx_min_usec: n("rx_min_usec") as u16,
            rx_max_usec: n("rx_max_usec") as u16,
            failsafe_delay: small("failsafe_delay"),
            failsafe_procedure: match n("failsafe_procedure") {
                0 => FailsafeProcedure::AutoLand,
                1 => FailsafeProcedure::Drop,
                _ => FailsafeProcedure::GpsRescue,
            },
            failsafe_throttle: n("failsafe_throttle") as u16,
            failsafe_recovery_delay: small("failsafe_recovery_delay"),
            pid: [gains("roll"), gains("pitch"), gains("yaw")],
            d_max: [
                small("d_max_roll"),
                small("d_max_pitch"),
                small("d_max_yaw"),
            ],
            feedforward_transition: small("feedforward_transition"),
            feedforward_averaging: match n("feedforward_averaging") {
                0 => FeedforwardAveraging::Off,
                1 => FeedforwardAveraging::TwoPoint,
                2 => FeedforwardAveraging::ThreePoint,
                _ => FeedforwardAveraging::FourPoint,
            },
            feedforward_smooth_factor: small("feedforward_smooth_factor"),
            feedforward_jitter_factor: small("feedforward_jitter_factor"),
            feedforward_boost: small("feedforward_boost"),
            feedforward_max_rate_limit: small("feedforward_max_rate_limit"),
            d_max_gain: small("d_max_gain"),
            d_max_advance: small("d_max_advance"),
            iterm_relax: match n("iterm_relax") {
                0 => ItermRelax::Off,
                1 => ItermRelax::Rp,
                2 => ItermRelax::Rpy,
                3 => ItermRelax::RpInc,
                _ => ItermRelax::RpyInc,
            },
            iterm_relax_type: match n("iterm_relax_type") {
                0 => ItermRelaxType::Gyro,
                _ => ItermRelaxType::Setpoint,
            },
            iterm_relax_cutoff: small("iterm_relax_cutoff"),
            anti_gravity_gain: small("anti_gravity_gain"),
            tpa_mode: match n("tpa_mode") {
                0 => TpaMode::Pd,
                _ => TpaMode::D,
            },
            tpa_rate: small("tpa_rate"),
            tpa_breakpoint: n("tpa_breakpoint") as u16,
            throttle_boost: small("throttle_boost"),
            motor_output_limit: small("motor_output_limit"),
            angle_p_gain: small("angle_p_gain"),
            angle_limit: small("angle_limit"),
            horizon_level_strength: small("horizon_level_strength"),
            pidsum_limit: n("pidsum_limit") as u16,
            pidsum_limit_yaw: n("pidsum_limit_yaw") as u16,
            iterm_windup: small("iterm_windup"),
            pid_at_min_throttle: n("pid_at_min_throttle") == 1,
            feedforward_yaw_hold_gain: small("feedforward_yaw_hold_gain"),
            feedforward_yaw_hold_time: small("feedforward_yaw_hold_time"),
            anti_gravity_cutoff_hz: small("anti_gravity_cutoff_hz"),
            anti_gravity_p_gain: small("anti_gravity_p_gain"),
            tpa_low_rate: small("tpa_low_rate"),
            tpa_low_breakpoint: n("tpa_low_breakpoint") as u16,
            tpa_low_always: n("tpa_low_always") == 1,
            throttle_boost_cutoff: small("throttle_boost_cutoff"),
            angle_feedforward: small("angle_feedforward"),
            angle_feedforward_smoothing_ms: small("angle_feedforward_smoothing_ms"),
            angle_earth_ref: small("angle_earth_ref"),
            horizon_limit_sticks: small("horizon_limit_sticks"),
            horizon_limit_degrees: small("horizon_limit_degrees"),
            horizon_ignore_sticks: n("horizon_ignore_sticks") == 1,
            horizon_delay_ms: hz("horizon_delay_ms"),
            level_race_mode: n("level_race_mode") == 1,
            gyro_lpf1_type: LowpassType::from_place(n("gyro_lpf1_type")),
            gyro_lpf1_static_hz: hz("gyro_lpf1_static_hz"),
            gyro_lpf1_dyn_min_hz: hz("gyro_lpf1_dyn_min_hz"),
            gyro_lpf1_dyn_max_hz: hz("gyro_lpf1_dyn_max_hz"),
            gyro_lpf2_type: LowpassType::from_place(n("gyro_lpf2_type")),
            gyro_lpf2_static_hz: hz("gyro_lpf2_static_hz"),
            dterm_lpf1_type: LowpassType::from_place(n("dterm_lpf1_type")),
            dterm_lpf1_static_hz: hz("dterm_lpf1_static_hz"),
            dterm_lpf1_dyn_min_hz: hz("dterm_lpf1_dyn_min_hz"),
            dterm_lpf1_dyn_max_hz: hz("dterm_lpf1_dyn_max_hz"),
            dterm_lpf2_type: LowpassType::from_place(n("dterm_lpf2_type")),
            dterm_lpf2_static_hz: hz("dterm_lpf2_static_hz"),
            gyro_lpf1_dyn_expo: small("gyro_lpf1_dyn_expo"),
            dterm_lpf1_dyn_expo: small("dterm_lpf1_dyn_expo"),
            yaw_lowpass_hz: hz("yaw_lowpass_hz"),
            min_check: n("min_check") as u16,
            mid_rc: n("mid_rc") as u16,
            deadband: small("deadband"),
            yaw_deadband: small("yaw_deadband"),
            rc_smoothing: n("rc_smoothing") == 1,
            rc_smoothing_auto_factor: small("rc_smoothing_auto_factor"),
            rc_smoothing_auto_factor_throttle: small("rc_smoothing_auto_factor_throttle"),
            rc_smoothing_setpoint_cutoff: small("rc_smoothing_setpoint_cutoff"),
            rc_smoothing_throttle_cutoff: small("rc_smoothing_throttle_cutoff"),
            yaw_control_reversed: n("yaw_control_reversed") == 1,
            airmode_start_throttle_percent: small("airmode_start_throttle_percent"),
            mixer_type: MixerType::Legacy,
            motor_poles: small("motor_poles"),
            yaw_motors_reversed: n("yaw_motors_reversed") == 1,
            motor_pwm_protocol: match n("motor_pwm_protocol") {
                0 => MotorProtocol::Dshot150,
                1 => MotorProtocol::Dshot300,
                _ => MotorProtocol::Dshot600,
            },
            dshot_bidir: n("dshot_bidir") == 1,
            motor_idle: n("motor_idle") as u16,
            crashflip_motor_percent: small("crashflip_motor_percent"),
            crashflip_rate: small("crashflip_rate"),
            crashflip_auto_rearm: n("crashflip_auto_rearm") == 1,
        })
    }

    /// The settings whose value this Flight Controller reads but doesn't
    /// simulate yet, each with a plain sentence saying what flies instead: a
    /// `failsafe_procedure` of AUTO-LAND or GPS-RESCUE, which flies DROP
    /// (#21), and a `current_meter` of VIRTUAL or MSP, which reads no current
    /// (#31 §3).
    pub fn not_simulated_values(&self) -> Vec<(&'static str, String)> {
        let mut values = Vec::new();
        if self.failsafe_procedure != FailsafeProcedure::Drop {
            values.push((
                "failsafe_procedure",
                format!(
                    "`failsafe_procedure` is {}, which isn't simulated yet: the Quad flies DROP, disarming as Failsafe's stage 2 starts",
                    self.failsafe_procedure.word()
                ),
            ));
        }
        if matches!(
            self.current_meter,
            CurrentMeter::Virtual | CurrentMeter::Msp
        ) {
            values.push((
                "current_meter",
                format!(
                    "`current_meter` is {}, which isn't simulated yet: the Flight Controller reads no current, so the OSD's current and mAh read blank",
                    CurrentMeter::WORDS[self.current_meter.place() as usize]
                ),
            ));
        }
        values
    }

    /// The ticket that brings a value [`Tune::not_simulated_values`] names.
    pub fn not_simulated_ticket(name: &str) -> &'static str {
        match name {
            "current_meter" => "#31",
            _ => "#21",
        }
    }

    /// Feeds every setting into a fingerprint, in [`Tune::settings`] order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let mut numbers: Vec<u64> = vec![
            u64::from(self.small_angle),
            match self.yaw_spin_recovery {
                YawSpinRecovery::Off => 0,
                YawSpinRecovery::On => 1,
                YawSpinRecovery::Auto => 2,
            },
            u64::from(self.yaw_spin_threshold),
            u64::from(self.runaway_takeoff_prevention),
            u64::from(self.runaway_takeoff_deactivate_delay),
            u64::from(self.runaway_takeoff_deactivate_throttle_percent),
            self.current_meter.place(),
            u64::from(self.vbat_min_cell_voltage),
            u64::from(self.vbat_max_cell_voltage),
            u64::from(self.vbat_warning_cell_voltage),
            u64::from(self.vbat_full_cell_voltage),
            u64::from(self.force_battery_cell_count),
            u64::from(self.vbat_hysteresis),
            u64::from(self.vbat_detect_cell_voltage),
            u64::from(self.use_vbat_alerts),
            u64::from(self.vbat_display_lpf_period),
            u64::from(self.ibat_lpf_period),
            u64::from(self.vbat_duration_for_warning),
            u64::from(self.vbat_duration_for_critical),
            u64::from(self.rx_min_usec),
            u64::from(self.rx_max_usec),
            u64::from(self.failsafe_delay),
            match self.failsafe_procedure {
                FailsafeProcedure::AutoLand => 0,
                FailsafeProcedure::Drop => 1,
                FailsafeProcedure::GpsRescue => 2,
            },
            u64::from(self.failsafe_throttle),
            u64::from(self.failsafe_recovery_delay),
        ];
        for (gains, d_max) in self.pid.iter().zip(self.d_max) {
            numbers.extend([gains.p, gains.i, gains.d, d_max].map(u64::from));
            numbers.push(u64::from(gains.f));
        }
        numbers.extend([
            u64::from(self.feedforward_transition),
            match self.feedforward_averaging {
                FeedforwardAveraging::Off => 0,
                FeedforwardAveraging::TwoPoint => 1,
                FeedforwardAveraging::ThreePoint => 2,
                FeedforwardAveraging::FourPoint => 3,
            },
            u64::from(self.feedforward_smooth_factor),
            u64::from(self.feedforward_jitter_factor),
            u64::from(self.feedforward_boost),
            u64::from(self.feedforward_max_rate_limit),
            u64::from(self.d_max_gain),
            u64::from(self.d_max_advance),
            match self.iterm_relax {
                ItermRelax::Off => 0,
                ItermRelax::Rp => 1,
                ItermRelax::Rpy => 2,
                ItermRelax::RpInc => 3,
                ItermRelax::RpyInc => 4,
            },
            match self.iterm_relax_type {
                ItermRelaxType::Gyro => 0,
                ItermRelaxType::Setpoint => 1,
            },
            u64::from(self.iterm_relax_cutoff),
            u64::from(self.anti_gravity_gain),
            match self.tpa_mode {
                TpaMode::Pd => 0,
                TpaMode::D => 1,
            },
            u64::from(self.tpa_rate),
            u64::from(self.tpa_breakpoint),
            u64::from(self.throttle_boost),
            u64::from(self.motor_output_limit),
            u64::from(self.angle_p_gain),
            u64::from(self.angle_limit),
            u64::from(self.horizon_level_strength),
            u64::from(self.pidsum_limit),
            u64::from(self.pidsum_limit_yaw),
            u64::from(self.iterm_windup),
            u64::from(self.pid_at_min_throttle),
            u64::from(self.feedforward_yaw_hold_gain),
            u64::from(self.feedforward_yaw_hold_time),
            u64::from(self.anti_gravity_cutoff_hz),
            u64::from(self.anti_gravity_p_gain),
            u64::from(self.tpa_low_rate),
            u64::from(self.tpa_low_breakpoint),
            u64::from(self.tpa_low_always),
            u64::from(self.throttle_boost_cutoff),
            u64::from(self.angle_feedforward),
            u64::from(self.angle_feedforward_smoothing_ms),
            u64::from(self.angle_earth_ref),
            u64::from(self.horizon_limit_sticks),
            u64::from(self.horizon_limit_degrees),
            u64::from(self.horizon_ignore_sticks),
            u64::from(self.horizon_delay_ms),
            u64::from(self.level_race_mode),
            self.gyro_lpf1_type.place(),
            u64::from(self.gyro_lpf1_static_hz),
            u64::from(self.gyro_lpf1_dyn_min_hz),
            u64::from(self.gyro_lpf1_dyn_max_hz),
            self.gyro_lpf2_type.place(),
            u64::from(self.gyro_lpf2_static_hz),
            self.dterm_lpf1_type.place(),
            u64::from(self.dterm_lpf1_static_hz),
            u64::from(self.dterm_lpf1_dyn_min_hz),
            u64::from(self.dterm_lpf1_dyn_max_hz),
            self.dterm_lpf2_type.place(),
            u64::from(self.dterm_lpf2_static_hz),
            u64::from(self.gyro_lpf1_dyn_expo),
            u64::from(self.dterm_lpf1_dyn_expo),
            u64::from(self.yaw_lowpass_hz),
            u64::from(self.min_check),
            u64::from(self.mid_rc),
            u64::from(self.deadband),
            u64::from(self.yaw_deadband),
            u64::from(self.rc_smoothing),
            u64::from(self.rc_smoothing_auto_factor),
            u64::from(self.rc_smoothing_auto_factor_throttle),
            u64::from(self.rc_smoothing_setpoint_cutoff),
            u64::from(self.rc_smoothing_throttle_cutoff),
            u64::from(self.yaw_control_reversed),
            u64::from(self.airmode_start_throttle_percent),
            match self.motor_pwm_protocol {
                MotorProtocol::Dshot150 => 0,
                MotorProtocol::Dshot300 => 1,
                MotorProtocol::Dshot600 => 2,
            },
            u64::from(self.dshot_bidir),
            u64::from(self.motor_idle),
            u64::from(self.motor_poles),
            u64::from(self.yaw_motors_reversed),
            match self.mixer_type {
                MixerType::Legacy => 0,
            },
            u64::from(self.crashflip_motor_percent),
            u64::from(self.crashflip_rate),
            u64::from(self.crashflip_auto_rearm),
        ]);
        for n in numbers {
            f.write_u64(n);
        }
    }
}

/// One setting's value as a number: the number itself, 0 or 1 for `OFF` and
/// `ON`, or the word's place in its list of choices.
fn value(name: &str, kind: &Kind, text: &str) -> Result<u32, String> {
    match kind {
        Kind::Number(lowest, highest) => match text.parse::<u32>() {
            Ok(v) if (*lowest..=*highest).contains(&v) => Ok(v),
            _ => Err(format!(
                "`{name}` is a whole number from {lowest} to {highest}, not \"{text}\""
            )),
        },
        Kind::OffOn => match text {
            "OFF" => Ok(0),
            "ON" => Ok(1),
            _ => Err(format!("`{name}` is OFF or ON, not \"{text}\"")),
        },
        Kind::Word(words) => match words.iter().position(|w| *w == text) {
            Some(place) => Ok(place as u32),
            None => Err(format!(
                "`{name}` must be one of {}, not \"{text}\"",
                words.join(", ")
            )),
        },
        Kind::Choice(words, why) => {
            if let Some(place) = words.iter().position(|w| *w == text) {
                return Ok(place as u32);
            }
            let known = KNOWN_WORDS
                .iter()
                .find(|(n, _)| *n == name)
                .is_some_and(|(_, all)| all.contains(&text));
            if known {
                Err(format!("`{name}` is {text}, but {why}"))
            } else {
                Err(format!(
                    "`{name}` must be one of {}, not \"{text}\"",
                    words.join(", ")
                ))
            }
        }
    }
}
