//! The Tune: the Flight Controller's settings, read from a Quad's `tune.txt`
//! (ADR-0015) under Betaflight 2026.6's CLI names and in Betaflight's own
//! units.
//!
//! This takes the Tune's `set` lines as name and value text, and gives the
//! settings out; it opens no files. Every setting this Flight Controller reads
//! must be there: a Tune spells out every setting, so no default hides in the
//! code ([ADR-0015]).
//!
//! Some settings it knows but doesn't simulate yet ([`Tune::not_simulated_yet`]):
//! the filters, RC smoothing and feedforward (#49), and Dynamic D, I-term
//! relax, anti-gravity, TPA and throttle boost (#50). A Tune may set them,
//! and their values are checked, but the Flight Controller flies as if each
//! were off until its ticket lands; a Test Quad sets them off to fly exactly
//! as Betaflight does. Settings it doesn't know at all are left alone; later
//! Flight Controller tickets read more of them.
//!
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md

use std::collections::BTreeMap;

use opendrone_maths::Fingerprinter;

/// The Tune's settings this Flight Controller reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tune {
    /// `small_angle`: the most the Quad may be tilted to arm, in degrees.
    pub small_angle: u8,
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
    /// `p_roll`, `i_roll`, `d_roll`, then pitch and yaw.
    pub pid: [Gains; 3],
    /// `motor_output_limit`: the top of the motors' range, in percent.
    pub motor_output_limit: u8,
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
    /// `min_check`: below this, in µs, the throttle counts as low.
    pub min_check: u16,
    /// `mid_rc`: the sticks' centre, in µs.
    pub mid_rc: u16,
    /// `deadband`: roll and pitch's deadband, in µs.
    pub deadband: u8,
    /// `yaw_deadband`: yaw's deadband, in µs.
    pub yaw_deadband: u8,
    /// `yaw_control_reversed`.
    pub yaw_control_reversed: bool,
    /// `airmode_start_throttle_percent`: Airmode starts once the throttle
    /// first passes this, in percent, after arming.
    pub airmode_start_throttle_percent: u8,
    /// `mixer_type`: only Legacy is simulated.
    pub mixer_type: MixerType,
    /// `yaw_motors_reversed`: props out.
    pub yaw_motors_reversed: bool,
    /// `motor_pwm_protocol`: one of the DShot speeds.
    pub motor_pwm_protocol: MotorProtocol,
    /// `motor_idle`: the motors' idle, in hundredths of a percent (550 is
    /// 5.5%).
    pub motor_idle: u16,
}

/// One axis's PID gains, as Betaflight stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gains {
    pub p: u8,
    pub i: u8,
    pub d: u8,
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
/// shows them, tab by tab (Configuration, Failsafe, PID Tuning, Receiver,
/// Motors), with each tab's settings that only the CLI holds after the ones
/// it shows. The ranges are Betaflight 2026.6.2's
/// (`src/main/cli/settings.c`).
const SPECS: &[(&str, Kind)] = &[
    ("small_angle", Kind::Number(0, 180)),
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
    ("p_pitch", Kind::Number(0, 250)),
    ("i_pitch", Kind::Number(0, 250)),
    ("d_pitch", Kind::Number(0, 250)),
    ("p_yaw", Kind::Number(0, 250)),
    ("i_yaw", Kind::Number(0, 250)),
    ("d_yaw", Kind::Number(0, 250)),
    ("motor_output_limit", Kind::Number(1, 100)),
    ("pidsum_limit", Kind::Number(100, 1000)),
    ("pidsum_limit_yaw", Kind::Number(100, 1000)),
    ("iterm_windup", Kind::Number(20, 100)),
    ("pid_at_min_throttle", Kind::OffOn),
    ("min_check", Kind::Number(750, 2250)),
    ("mid_rc", Kind::Number(1200, 1700)),
    ("deadband", Kind::Number(0, 32)),
    ("yaw_deadband", Kind::Number(0, 100)),
    ("yaw_control_reversed", Kind::OffOn),
    ("airmode_start_throttle_percent", Kind::Number(0, 100)),
    (
        "motor_pwm_protocol",
        Kind::Choice(
            &["DSHOT150", "DSHOT300", "DSHOT600"],
            "the simulated ESCs run Bluejay, which reads DShot: DSHOT150, DSHOT300 or DSHOT600",
        ),
    ),
    ("motor_idle", Kind::Number(0, 2000)),
    ("yaw_motors_reversed", Kind::OffOn),
    (
        "mixer_type",
        Kind::Choice(
            &["LEGACY"],
            "only the Legacy mixer is simulated; LINEAR, DYNAMIC and EZLANDING aren't",
        ),
    ),
];

/// Settings this Flight Controller knows but doesn't simulate yet, with
/// their ticket. A Tune may set them, and each value is checked as Betaflight
/// 2026.6.2's CLI checks it, but nothing reads them yet, so the Flight
/// Controller flies as if each were off. A Test Quad that sets them off
/// (Betaflight's way: a cutoff, gain or count of 0, `OFF`) flies exactly as
/// Betaflight does now, and stays so once their tickets land. Grouped as the
/// Betaflight App shows them.
const NOT_SIMULATED_YET: &[(&str, Kind, &str)] = &[
    // PID Tuning: feedforward, Dynamic D, I-term relax, anti-gravity, TPA and
    // throttle boost.
    ("f_roll", Kind::Number(0, 1000), "#49"),
    ("f_pitch", Kind::Number(0, 1000), "#49"),
    ("f_yaw", Kind::Number(0, 1000), "#49"),
    ("d_max_roll", Kind::Number(0, 250), "#50"),
    ("d_max_pitch", Kind::Number(0, 250), "#50"),
    ("d_max_yaw", Kind::Number(0, 250), "#50"),
    (
        "iterm_relax",
        Kind::Choice(&["OFF", "RP", "RPY", "RP_INC", "RPY_INC"], ""),
        "#50",
    ),
    ("anti_gravity_gain", Kind::Number(0, 250), "#50"),
    ("tpa_rate", Kind::Number(0, 100), "#50"),
    ("tpa_low_rate", Kind::Number(0, 100), "#50"),
    ("throttle_boost", Kind::Number(0, 100), "#50"),
    // PID Tuning, filters: the gyro and D-term low-passes, the dynamic notch
    // and the yaw P low-pass.
    ("gyro_lpf1_static_hz", Kind::Number(0, 1000), "#49"),
    ("gyro_lpf1_dyn_min_hz", Kind::Number(0, 1000), "#49"),
    ("gyro_lpf2_static_hz", Kind::Number(0, 1000), "#49"),
    ("dyn_notch_count", Kind::Number(0, 7), "#49"),
    ("dterm_lpf1_static_hz", Kind::Number(0, 1000), "#49"),
    ("dterm_lpf1_dyn_min_hz", Kind::Number(0, 1000), "#49"),
    ("dterm_lpf2_static_hz", Kind::Number(0, 1000), "#49"),
    ("yaw_lowpass_hz", Kind::Number(0, 500), "#49"),
    // Receiver: RC smoothing.
    ("rc_smoothing", Kind::OffOn, "#49"),
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
        let gains = |axis: &str| Gains {
            p: small(&format!("p_{axis}")),
            i: small(&format!("i_{axis}")),
            d: small(&format!("d_{axis}")),
        };
        Ok(Tune {
            small_angle: small("small_angle"),
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
            motor_output_limit: small("motor_output_limit"),
            pidsum_limit: n("pidsum_limit") as u16,
            pidsum_limit_yaw: n("pidsum_limit_yaw") as u16,
            iterm_windup: small("iterm_windup"),
            pid_at_min_throttle: n("pid_at_min_throttle") == 1,
            min_check: n("min_check") as u16,
            mid_rc: n("mid_rc") as u16,
            deadband: small("deadband"),
            yaw_deadband: small("yaw_deadband"),
            yaw_control_reversed: n("yaw_control_reversed") == 1,
            airmode_start_throttle_percent: small("airmode_start_throttle_percent"),
            mixer_type: MixerType::Legacy,
            yaw_motors_reversed: n("yaw_motors_reversed") == 1,
            motor_pwm_protocol: match n("motor_pwm_protocol") {
                0 => MotorProtocol::Dshot150,
                1 => MotorProtocol::Dshot300,
                _ => MotorProtocol::Dshot600,
            },
            motor_idle: n("motor_idle") as u16,
        })
    }

    /// The settings whose value this Flight Controller reads but doesn't
    /// simulate yet, each with a plain sentence saying what flies instead:
    /// so far, a `failsafe_procedure` of AUTO-LAND or GPS-RESCUE, which flies
    /// DROP (#21).
    pub fn not_simulated_values(&self) -> Vec<(&'static str, String)> {
        match self.failsafe_procedure {
            FailsafeProcedure::Drop => Vec::new(),
            other => vec![(
                "failsafe_procedure",
                format!(
                    "`failsafe_procedure` is {}, which isn't simulated yet: the Quad flies DROP, disarming as Failsafe's stage 2 starts",
                    other.word()
                ),
            )],
        }
    }

    /// Feeds every setting into a fingerprint, in [`Tune::settings`] order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let mut numbers: Vec<u64> = vec![
            u64::from(self.small_angle),
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
        for gains in self.pid {
            numbers.extend([gains.p, gains.i, gains.d].map(u64::from));
        }
        numbers.extend([
            u64::from(self.motor_output_limit),
            u64::from(self.pidsum_limit),
            u64::from(self.pidsum_limit_yaw),
            u64::from(self.iterm_windup),
            u64::from(self.pid_at_min_throttle),
            u64::from(self.min_check),
            u64::from(self.mid_rc),
            u64::from(self.deadband),
            u64::from(self.yaw_deadband),
            u64::from(self.yaw_control_reversed),
            u64::from(self.airmode_start_throttle_percent),
            match self.motor_pwm_protocol {
                MotorProtocol::Dshot150 => 0,
                MotorProtocol::Dshot300 => 1,
                MotorProtocol::Dshot600 => 2,
            },
            u64::from(self.motor_idle),
            u64::from(self.yaw_motors_reversed),
            match self.mixer_type {
                MixerType::Legacy => 0,
            },
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
