//! The Rates paste: a pilot pastes `diff`, `diff all` or `dump` output from
//! the Betaflight CLI, and their Rates are read from its active rate profile
//! (#13).
//!
//! The active rate profile is the last `rateprofile` line's: a `diff all`
//! lists every rate profile, then selects the active one again at its end.
//! Only that profile's rate settings are read; every other line is ignored.
//! A rate setting the profile doesn't set keeps Betaflight's default, which
//! has been the same since 4.3 (Actual, 70 °/s at centre, 670 °/s at full
//! stick, no expo). A paste with no `rateprofile` line at all, such as a few
//! copied `set` lines, is read as one rate profile.
//!
//! Like Betaflight when it checks its settings, a rate number above what its
//! Rates type allows is held at that limit (`ratesSettingLimits` in
//! `src/main/fc/controlrate_profile.c`), with a note. A paste from before
//! 2025.12 with throttle expo gets a note too: 2025.12 rebuilt the throttle
//! curve, so with expo the old curve bends differently (the Betaflight
//! research §4).

use super::{CliText, Command, Refusal, Section};
use crate::rates::{AxisRates, Rates, RatesType, ThrottleLimitType};

/// The Rates read from a paste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatesPaste {
    /// The rate profile read: the last `rateprofile` line's, if the paste
    /// had one.
    pub rate_profile: Option<u8>,
    /// The Rates after the paste: Betaflight's defaults, with the rate
    /// profile's settings.
    pub rates: Rates,
    /// The rate settings read, as `set` lines, in the paste's order.
    pub read: Vec<String>,
    /// How many lines were ignored: every line but comments, blank lines,
    /// `rateprofile` lines and the rate settings read.
    pub ignored: usize,
    /// Rate numbers held at their Rates type's limit, as Betaflight does.
    pub notes: Vec<String>,
}

/// One rate setting before and after the paste, as Betaflight's CLI writes
/// it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowAfter {
    pub setting: &'static str,
    pub now: String,
    pub after: String,
}

impl NowAfter {
    pub fn changed(&self) -> bool {
        self.now != self.after
    }
}

/// Every rate setting of a Betaflight 2026.6 rate profile, the profile name
/// aside, in the order the Rates hold them.
const SETTINGS: &[&str] = &[
    "rates_type",
    "roll_rc_rate",
    "roll_srate",
    "roll_expo",
    "pitch_rc_rate",
    "pitch_srate",
    "pitch_expo",
    "yaw_rc_rate",
    "yaw_srate",
    "yaw_expo",
    "roll_rate_limit",
    "pitch_rate_limit",
    "yaw_rate_limit",
    "thr_mid",
    "thr_hover",
    "thr_expo",
    "throttle_limit_type",
    "throttle_limit_percent",
    "quickrates_rc_expo",
];

const RATES_TYPES: [(&str, RatesType); 5] = [
    ("BETAFLIGHT", RatesType::Betaflight),
    ("RACEFLIGHT", RatesType::Raceflight),
    ("KISS", RatesType::Kiss),
    ("ACTUAL", RatesType::Actual),
    ("QUICK", RatesType::Quick),
];

const LIMIT_TYPES: [(&str, ThrottleLimitType); 3] = [
    ("OFF", ThrottleLimitType::Off),
    ("SCALE", ThrottleLimitType::Scale),
    ("CLIP", ThrottleLimitType::Clip),
];

impl RatesPaste {
    /// Reads the Rates from pasted CLI text. Refused, with a sentence, when
    /// it comes from Betaflight older than 4.3 (whose default Rates differ),
    /// holds no rate profile and no rate settings, or sets a value Betaflight
    /// would refuse.
    pub fn read(text: &str) -> Result<RatesPaste, Refusal> {
        let cli = CliText::read(text);
        if let Some(version) = cli.version
            && (version.major, version.minor) < (4, 3)
        {
            return Err(Refusal::one(format!(
                "This is Betaflight {version}, which is older than 4.3: its default Rates differ (Actual rates became the default in 4.3), so OpenDrone reads Rates from 4.3 or newer."
            )));
        }
        let rate_profile = cli.active_rate_profile();
        let mut rates = Rates::BETAFLIGHT_DEFAULT;
        let mut read = Vec::new();
        let mut ignored = 0;
        let mut problems = Vec::new();
        for line in &cli.lines {
            let in_profile = match (line.section, rate_profile) {
                (Section::RateProfile(n), Some(active)) => n == active,
                (_, None) => true,
                _ => false,
            };
            match line.command {
                Command::RateProfile(_) => {}
                Command::Set { name, value } if in_profile && SETTINGS.contains(&name) => {
                    match set(&mut rates, name, value) {
                        Ok(()) => read.push(format!("set {name} = {value}")),
                        Err(sentence) => {
                            problems.push(format!("Line {}: {sentence}.", line.number))
                        }
                    }
                }
                _ => ignored += 1,
            }
        }
        if !problems.is_empty() {
            return Err(Refusal(problems));
        }
        if rate_profile.is_none() && read.is_empty() {
            return Err(Refusal::one(
                "There's no rate profile here: paste the output of `diff all`, `diff` or `dump`, which holds the `rateprofile` lines.",
            ));
        }
        let mut notes = hold_within_limits(&mut rates);
        if let Some(version) = cli.version
            && (version.major, version.minor) < (2025, 12)
            && rates.thr_expo > 0
        {
            notes.push(format!(
                "Betaflight 2025.12 changed the throttle curve: with thr_expo at {}, Betaflight {version}'s curve bends differently from the 2026.6 curve OpenDrone flies.",
                rates.thr_expo
            ));
        }
        Ok(RatesPaste {
            rate_profile,
            rates,
            read,
            ignored,
            notes,
        })
    }

    /// One plain sentence on what was read, such as "Used rate profile 0
    /// (3 rate settings); ignored 160 lines."
    pub fn summary(&self) -> String {
        let settings = match self.read.len() {
            0 => "no rate settings, so every one keeps Betaflight's default".to_string(),
            1 => "1 rate setting".to_string(),
            n => format!("{n} rate settings"),
        };
        let used = match self.rate_profile {
            Some(n) => format!("Used rate profile {n}"),
            None => {
                "Found no `rateprofile` line, so read the pasted lines as one rate profile".into()
            }
        };
        let lines = if self.ignored == 1 { "line" } else { "lines" };
        format!("{used} ({settings}); ignored {} {lines}.", self.ignored)
    }

    /// Every rate setting now and after the paste, so the pilot sees what
    /// changes before applying it.
    pub fn now_after(&self, now: &Rates) -> Vec<NowAfter> {
        SETTINGS
            .iter()
            .map(|&setting| NowAfter {
                setting,
                now: text(now, setting),
                after: text(&self.rates, setting),
            })
            .collect()
    }
}

/// The axis rates a setting name such as `roll_srate` belongs to, and the
/// rest of its name.
fn axis<'r>(rates: &'r mut Rates, name: &str) -> Option<(&'r mut AxisRates, usize)> {
    let (axis, rest) = name.split_once('_')?;
    let rates = match axis {
        "roll" => &mut rates.roll,
        "pitch" => &mut rates.pitch,
        "yaw" => &mut rates.yaw,
        _ => return None,
    };
    let index = ["rc_rate", "srate", "expo"]
        .iter()
        .position(|n| *n == rest)?;
    Some((rates, index))
}

fn number(name: &str, value: &str, lowest: u32, highest: u32) -> Result<u32, String> {
    match value.parse::<u32>() {
        Ok(v) if (lowest..=highest).contains(&v) => Ok(v),
        _ => Err(format!(
            "`{name}` is a whole number from {lowest} to {highest}, not \"{value}\""
        )),
    }
}

fn word<T: Copy>(name: &str, value: &str, words: &[(&str, T)]) -> Result<T, String> {
    words
        .iter()
        .find(|(w, _)| *w == value)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            let list: Vec<&str> = words.iter().map(|(w, _)| *w).collect();
            format!("`{name}` is one of {}, not \"{value}\"", list.join(", "))
        })
}

/// Sets one rate setting, in Betaflight 2026.6's ranges
/// (`src/main/cli/settings.c`).
fn set(rates: &mut Rates, name: &str, value: &str) -> Result<(), String> {
    let small = |lowest, highest| number(name, value, lowest, highest).map(|v| v as u8);
    match name {
        "rates_type" => rates.rates_type = word(name, value, &RATES_TYPES)?,
        "throttle_limit_type" => rates.throttle_limit_type = word(name, value, &LIMIT_TYPES)?,
        "quickrates_rc_expo" => {
            rates.quickrates_rc_expo = word(name, value, &[("OFF", false), ("ON", true)])?
        }
        "thr_mid" => rates.thr_mid = small(0, 100)?,
        "thr_hover" => rates.thr_hover = small(0, 100)?,
        "thr_expo" => rates.thr_expo = small(0, 100)?,
        "throttle_limit_percent" => rates.throttle_limit_percent = small(25, 100)?,
        "roll_rate_limit" | "pitch_rate_limit" | "yaw_rate_limit" => {
            let index = ["roll", "pitch", "yaw"]
                .iter()
                .position(|axis| name.starts_with(axis))
                .unwrap_or(0);
            rates.rate_limit[index] = number(name, value, 200, 1998)? as u16;
        }
        _ => {
            let Some((axis, index)) = axis(rates, name) else {
                return Ok(());
            };
            match index {
                0 => axis.rc_rate = small(1, 255)?,
                1 => axis.srate = small(0, 255)?,
                _ => axis.expo = small(0, 100)?,
            }
        }
    }
    Ok(())
}

/// The most each Rates type lets its RC rate, super rate and expo be.
fn limits(rates_type: RatesType) -> [u8; 3] {
    match rates_type {
        RatesType::Betaflight => [255, 100, 100],
        RatesType::Raceflight => [200, 255, 100],
        RatesType::Kiss => [255, 99, 100],
        RatesType::Actual => [200, 200, 100],
        RatesType::Quick => [255, 200, 100],
    }
}

/// Holds each axis's numbers within its Rates type's limits, as Betaflight
/// does when it checks its settings, and says which were held.
fn hold_within_limits(rates: &mut Rates) -> Vec<String> {
    let limits = limits(rates.rates_type);
    let type_name = RATES_TYPES
        .iter()
        .find(|(_, t)| *t == rates.rates_type)
        .map_or("", |(w, _)| *w);
    let mut notes = Vec::new();
    for (axis, numbers) in [
        ("roll", &mut rates.roll),
        ("pitch", &mut rates.pitch),
        ("yaw", &mut rates.yaw),
    ] {
        for ((part, value), limit) in [
            ("rc_rate", &mut numbers.rc_rate),
            ("srate", &mut numbers.srate),
            ("expo", &mut numbers.expo),
        ]
        .into_iter()
        .zip(limits)
        {
            if *value > limit {
                notes.push(format!(
                    "{axis}_{part} {value} is above {limit}, the most {type_name} rates allow, so Betaflight keeps {limit}."
                ));
                *value = limit;
            }
        }
    }
    notes
}

/// One rate setting's value, as the CLI writes it.
fn text(rates: &Rates, setting: &str) -> String {
    let off_on = |on: bool| if on { "ON" } else { "OFF" }.to_string();
    match setting {
        "rates_type" => RATES_TYPES
            .iter()
            .find(|(_, t)| *t == rates.rates_type)
            .map_or(String::new(), |(w, _)| (*w).to_string()),
        "throttle_limit_type" => LIMIT_TYPES
            .iter()
            .find(|(_, t)| *t == rates.throttle_limit_type)
            .map_or(String::new(), |(w, _)| (*w).to_string()),
        "quickrates_rc_expo" => off_on(rates.quickrates_rc_expo),
        "thr_mid" => rates.thr_mid.to_string(),
        "thr_hover" => rates.thr_hover.to_string(),
        "thr_expo" => rates.thr_expo.to_string(),
        "throttle_limit_percent" => rates.throttle_limit_percent.to_string(),
        "roll_rate_limit" => rates.rate_limit[0].to_string(),
        "pitch_rate_limit" => rates.rate_limit[1].to_string(),
        "yaw_rate_limit" => rates.rate_limit[2].to_string(),
        _ => {
            let mut copy = rates.clone();
            match axis(&mut copy, setting) {
                Some((numbers, 0)) => numbers.rc_rate.to_string(),
                Some((numbers, 1)) => numbers.srate.to_string(),
                Some((numbers, _)) => numbers.expo.to_string(),
                None => String::new(),
            }
        }
    }
}
