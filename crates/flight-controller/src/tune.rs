//! The Tune: the Flight Controller's settings, read from a Quad's `tune.txt`
//! (ADR-0015) under Betaflight 2026.6's CLI names and in Betaflight's own
//! units.
//!
//! This takes the Tune's `set` lines as name and value text, and gives the
//! settings out; it opens no files. Every setting this Flight Controller reads
//! must be there: a Tune spells out every setting, so no default hides in the
//! code ([ADR-0015]). Settings it doesn't read yet are left alone; later
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
}

/// Every setting this Flight Controller reads, grouped as Betaflight's App
/// shows them, tab by tab (Configuration, PID Tuning, Receiver, Motors), with
/// each tab's settings that only the CLI holds after the ones it shows. The
/// ranges are Betaflight 2026.6.2's (`src/main/cli/settings.c`).
const SPECS: &[(&str, Kind)] = &[
    ("small_angle", Kind::Number(0, 180)),
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

    /// Checks one setting's value, if it is one this Flight Controller reads:
    /// a plain sentence when it can't read it.
    pub fn check(name: &str, value: &str) -> Result<(), String> {
        match SPECS.iter().find(|(n, _)| *n == name) {
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

    /// Feeds every setting into a fingerprint, in [`Tune::settings`] order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let mut numbers: Vec<u64> = vec![u64::from(self.small_angle)];
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
