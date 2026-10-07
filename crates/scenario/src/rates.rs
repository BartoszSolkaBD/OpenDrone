//! The active Rates in a Scenario's starting state.
//!
//! A Scenario spells out every field of a Betaflight 2026.6 rate profile
//! (`controlRateConfig_t` in `src/main/fc/controlrate_profile.h`; the profile
//! name aside). Each is written as the Betaflight App shows it, with its unit
//! where the number has one, and kept as Betaflight's CLI stores it, so the
//! Flight Controller reads exactly what Betaflight would.
//!
//! Each axis's three numbers, by Rates type. "CLI" is the stored number, such
//! as `roll_rc_rate`, `roll_srate` and `roll_expo`:
//!
//! | Type | First (`rc_rate`) | Second (`srate`) | Third (`expo`) |
//! |---|---|---|---|
//! | Betaflight | `rc rate 1.00` = CLI 100 | `rate 0.70` = CLI 70 | `rc expo 0.00` = CLI 0 |
//! | Raceflight | `rate 370 °/s` = CLI 37 | `acro+ 80%` = CLI 80 | `expo 50%` = CLI 50 |
//! | KISS | `rc rate 1.00` = CLI 100 | `rate 0.70` = CLI 70 | `rc curve 0.00` = CLI 0 |
//! | Actual | `center sensitivity 70 °/s` = CLI 7 | `max rate 670 °/s` = CLI 67 | `expo 0.54` = CLI 54 |
//! | Quick | `rc rate 1.00` = CLI 100 | `max rate 670 °/s` = CLI 67 | `expo 0.00` = CLI 0 |
//!
//! Labels and scales are the Betaflight App's own (its Rates tab,
//! `RatesSubTab.vue`, read 2026-10-07): it shows the stored number ÷ 100,
//! except Raceflight's rate and Actual's center sensitivity and max rate (× 10,
//! in °/s), Quick's max rate (× 10, in °/s), and Raceflight's acro+ and expo
//! (as stored). A number between two of the stored steps is refused.

use opendrone_maths::DEGREE;
use opendrone_pack::Problems;
use opendrone_pack::document::Table;
use opendrone_pack::units::{self, Dimension};

/// The active Rates: every field of a Betaflight 2026.6 rate profile, as the
/// CLI stores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rates {
    /// `rates_type`.
    pub rates_type: RatesType,
    pub roll: AxisRates,
    pub pitch: AxisRates,
    pub yaw: AxisRates,
    /// `roll_rate_limit`, `pitch_rate_limit` and `yaw_rate_limit`, in °/s,
    /// from 200 to 1998.
    pub rate_limit: [u16; 3],
    /// `thr_mid`: where on the stick the hover point sits, from 0 to 100.
    pub thr_mid: u8,
    /// `thr_hover`: the throttle at the hover point, from 0 to 100.
    pub thr_hover: u8,
    /// `thr_expo`, from 0 to 100.
    pub thr_expo: u8,
    /// `throttle_limit_type`.
    pub throttle_limit_type: ThrottleLimitType,
    /// `throttle_limit_percent`, from 25 to 100. Betaflight keeps it while the
    /// limit is off, unused; then it is 100, Betaflight's default.
    pub throttle_limit_percent: u8,
    /// `quickrates_rc_expo`.
    pub quickrates_rc_expo: bool,
}

/// One axis's three stored numbers, such as `roll_rc_rate`, `roll_srate` and
/// `roll_expo`. What each means depends on the Rates type (see the module).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisRates {
    pub rc_rate: u8,
    pub srate: u8,
    pub expo: u8,
}

/// Betaflight's five Rates types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatesType {
    Betaflight,
    Raceflight,
    Kiss,
    Actual,
    Quick,
}

/// `throttle_limit_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrottleLimitType {
    Off,
    /// Scales the whole throttle range down to the limit.
    Scale,
    /// Cuts the throttle off at the limit.
    Clip,
}

/// How the Betaflight App shows one stored number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    /// A plain number with two decimals: the stored number ÷ 100.
    Hundredths,
    /// Degrees a second: the stored number × 10.
    TensOfDegreesPerSecond,
    /// A percentage: the stored number, in %.
    Percent,
}

impl Shown {
    /// The stored number for what a Scenario wrote, if it is one of the
    /// stored steps.
    fn stored(self, quantity: &units::Quantity) -> Result<f64, String> {
        let shown = match self {
            Shown::Hundredths => quantity.as_a(Dimension::NONE).map(|v| v * 100.0),
            Shown::TensOfDegreesPerSecond => quantity
                .as_a(Dimension::ROTATION_SPEED)
                .map(|v| v / DEGREE / 10.0),
            Shown::Percent => quantity.as_a(Dimension::PERCENT).map(|v| v * 100.0),
        };
        shown.map_err(|p| p.0)
    }

    /// How the App shows a stored number, such as "0.70", "670 °/s" or "80%".
    fn show(self, stored: u32) -> String {
        match self {
            Shown::Hundredths => format!("{}.{:02}", stored / 100, stored % 100),
            Shown::TensOfDegreesPerSecond => format!("{} °/s", stored * 10),
            Shown::Percent => format!("{stored}%"),
        }
    }

    fn step(self) -> &'static str {
        match self {
            Shown::Hundredths => "0.01",
            Shown::TensOfDegreesPerSecond => "10 °/s",
            Shown::Percent => "1%",
        }
    }
}

/// One of an axis's three numbers for one Rates type.
struct Field {
    label: &'static str,
    shown: Shown,
    lowest: u32,
    highest: u32,
}

const fn field(label: &'static str, shown: Shown, lowest: u32, highest: u32) -> Field {
    Field {
        label,
        shown,
        lowest,
        highest,
    }
}

impl RatesType {
    const ALL: [(&'static str, RatesType); 5] = [
        ("Betaflight", RatesType::Betaflight),
        ("Raceflight", RatesType::Raceflight),
        ("KISS", RatesType::Kiss),
        ("Actual", RatesType::Actual),
        ("Quick", RatesType::Quick),
    ];

    /// The three numbers of each axis, as the App labels them, with the
    /// stored ranges: 1 up to this type's `rc_rate` limit, 0 up to its `srate`
    /// limit and 0 to 100 for `expo` (`ratesSettingLimits` in
    /// `controlrate_profile.c`).
    fn fields(self) -> [Field; 3] {
        use Shown::{Hundredths, Percent, TensOfDegreesPerSecond};
        match self {
            RatesType::Betaflight => [
                field("rc rate", Hundredths, 1, 255),
                field("rate", Hundredths, 0, 100),
                field("rc expo", Hundredths, 0, 100),
            ],
            RatesType::Raceflight => [
                field("rate", TensOfDegreesPerSecond, 1, 200),
                field("acro+", Percent, 0, 255),
                field("expo", Percent, 0, 100),
            ],
            RatesType::Kiss => [
                field("rc rate", Hundredths, 1, 255),
                field("rate", Hundredths, 0, 99),
                field("rc curve", Hundredths, 0, 100),
            ],
            RatesType::Actual => [
                field("center sensitivity", TensOfDegreesPerSecond, 1, 200),
                field("max rate", TensOfDegreesPerSecond, 0, 200),
                field("expo", Hundredths, 0, 100),
            ],
            RatesType::Quick => [
                field("rc rate", Hundredths, 1, 255),
                field("max rate", TensOfDegreesPerSecond, 0, 200),
                field("expo", Hundredths, 0, 100),
            ],
        }
    }
}

/// The keys of `[start.rates]`.
pub const KEYS: &[&str] = &[
    "type",
    "roll",
    "pitch",
    "yaw",
    "rate_limit",
    "throttle",
    "quickrates_rc_expo",
];

/// Reads `[start.rates]`, listing every problem.
pub fn read_rates(rates: &Table<'_, '_>, problems: &mut Problems) -> Option<Rates> {
    rates.refuse_unknown(KEYS, problems);
    let rates_type = rates.text("type", problems).and_then(|(text, item)| {
        let found = RatesType::ALL.iter().find(|(name, _)| *name == text);
        if found.is_none() {
            problems.push(item.problem(format!(
                "`type` must be one of \"Betaflight\", \"Raceflight\", \"KISS\", \"Actual\", \"Quick\", not \"{text}\""
            )));
        }
        found.map(|(_, t)| *t)
    });
    let mut axis = |key: &str| -> Option<AxisRates> {
        let (text, item) = rates.text(key, problems)?;
        let rates_type = rates_type?;
        match axis_rates(rates_type, text) {
            Ok(axis) => Some(axis),
            Err(sentence) => {
                problems.push(item.problem(format!("`{key}`: {sentence}")));
                None
            }
        }
    };
    let roll = axis("roll");
    let pitch = axis("pitch");
    let yaw = axis("yaw");
    let rate_limit =
        rates
            .text("rate_limit", problems)
            .and_then(|(text, item)| match rate_limit(text) {
                Ok(limit) => Some(limit),
                Err(sentence) => {
                    problems.push(item.problem(sentence));
                    None
                }
            });
    let throttle = rates
        .text("throttle", problems)
        .and_then(|(text, item)| match throttle(text) {
            Ok(throttle) => Some(throttle),
            Err(sentence) => {
                problems.push(item.problem(sentence));
                None
            }
        });
    let quickrates_rc_expo = rates
        .text("quickrates_rc_expo", problems)
        .and_then(|(text, item)| match text {
            "on" => Some(true),
            "off" => Some(false),
            _ => {
                problems.push(item.problem(format!(
                    "`quickrates_rc_expo` must be \"on\" or \"off\", not \"{text}\""
                )));
                None
            }
        });
    let (thr_mid, thr_hover, thr_expo, throttle_limit_type, throttle_limit_percent) = throttle?;
    Some(Rates {
        rates_type: rates_type?,
        roll: roll?,
        pitch: pitch?,
        yaw: yaw?,
        rate_limit: rate_limit?,
        thr_mid,
        thr_hover,
        thr_expo,
        throttle_limit_type,
        throttle_limit_percent,
        quickrates_rc_expo: quickrates_rc_expo?,
    })
}

/// One axis, such as "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00".
fn axis_rates(rates_type: RatesType, text: &str) -> Result<AxisRates, String> {
    let fields = rates_type.fields();
    let labels: Vec<&str> = fields.iter().map(|f| f.label).collect();
    let example = || {
        fields
            .iter()
            .map(|f| {
                let typical = match (rates_type, f.label) {
                    (RatesType::Actual, "center sensitivity") => 7,
                    (_, "max rate") => 67,
                    (RatesType::Raceflight, "rate") => 37,
                    (RatesType::Raceflight, "acro+") => 80,
                    (_, "rc rate") => 100,
                    (_, "rate") => 70,
                    _ => 0,
                };
                format!("{} {}", f.label, f.shown.show(typical))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let type_name = RatesType::ALL
        .iter()
        .find(|(_, t)| *t == rates_type)
        .map_or("", |(name, _)| name);
    let parts = units::parse_parts(text, &labels).map_err(|p| {
        format!(
            "{}; {type_name} rates write each axis as the Betaflight App shows it, such as \"{}\"",
            p.0,
            example()
        )
    })?;
    let mut stored = [0_u8; 3];
    for (value, field) in stored.iter_mut().zip(&fields) {
        let part = parts
            .iter()
            .find(|p| p.label == field.label)
            .ok_or_else(|| {
                format!(
                    "\"{text}\" is missing {}; {type_name} rates write each axis like \"{}\"",
                    field.label,
                    example()
                )
            })?;
        let number = field.shown.stored(&part.quantity)?;
        let whole = number.round();
        let in_range = (f64::from(field.lowest)..=f64::from(field.highest)).contains(&whole);
        if (number - whole).abs() > 1e-6 || !in_range {
            return Err(format!(
                "{type_name} rates' {} goes in steps of {}, from {} to {}",
                field.label,
                field.shown.step(),
                field.shown.show(field.lowest),
                field.shown.show(field.highest)
            ));
        }
        *value = whole as u8;
    }
    Ok(AxisRates {
        rc_rate: stored[0],
        srate: stored[1],
        expo: stored[2],
    })
}

/// "1998 °/s" for all three axes, or "roll 1998 °/s, pitch 1998 °/s, yaw
/// 1998 °/s": whole degrees a second, from 200 to 1998.
fn rate_limit(text: &str) -> Result<[u16; 3], String> {
    let help = "write one rate limit, such as \"1998 °/s\", or one per axis, such as \"roll 1998, pitch 1998, yaw 1998 °/s\"; each is a whole number of °/s from 200 to 1998";
    let degrees: [f64; 3] = match units::parse_quantity(text) {
        Ok(single) => [single.as_a(Dimension::ROTATION_SPEED).map_err(|p| p.0)? / DEGREE; 3],
        Err(_) => {
            let parts = units::parse_parts(text, &["roll", "pitch", "yaw"])
                .map_err(|p| format!("{}; {help}", p.0))?;
            let mut values = [0.0; 3];
            for (value, label) in values.iter_mut().zip(["roll", "pitch", "yaw"]) {
                let part = parts
                    .iter()
                    .find(|p| p.label == label)
                    .ok_or_else(|| format!("\"{text}\" is missing {label}; {help}"))?;
                *value = part
                    .quantity
                    .as_a(Dimension::ROTATION_SPEED)
                    .map_err(|p| p.0)?
                    / DEGREE;
            }
            values
        }
    };
    let mut limits = [0_u16; 3];
    for (limit, value) in limits.iter_mut().zip(degrees) {
        let whole = value.round();
        if (value - whole).abs() > 1e-6 || !(200.0..=1998.0).contains(&whole) {
            return Err(format!("\"{text}\": {help}"));
        }
        *limit = whole as u16;
    }
    Ok(limits)
}

/// "mid 0.50, hover 0.50, expo 0.00, limit off" (or "limit scale 80%", "limit
/// clip 80%"): the App's Throttle MID, Hover Point, Throttle EXPO and
/// Throttle Limit.
fn throttle(text: &str) -> Result<(u8, u8, u8, ThrottleLimitType, u8), String> {
    let help = "write the throttle curve as the Betaflight App shows it, like \"mid 0.50, hover 0.50, expo 0.00, limit off\"; mid, hover and expo go from 0.00 to 1.00 in steps of 0.01, and the limit is \"off\", \"scale 80%\" or \"clip 80%\" (25% to 100%)";
    let mut mid = None;
    let mut hover = None;
    let mut expo = None;
    let mut limit = None;
    let hundredths = |value: &str| -> Result<u8, String> {
        let number = units::parse_quantity(value)
            .and_then(|q| q.as_a(Dimension::NONE))
            .map_err(|p| format!("{}; {help}", p.0))?
            * 100.0;
        let whole = number.round();
        if (number - whole).abs() > 1e-6 || !(0.0..=100.0).contains(&whole) {
            return Err(format!("\"{value}\" can't be read; {help}"));
        }
        Ok(whole as u8)
    };
    for part in text.split(',').map(str::trim) {
        let (word, value) = part
            .split_once(' ')
            .ok_or_else(|| format!("\"{part}\" can't be read; {help}"))?;
        let value = value.trim();
        match word {
            "mid" => mid = Some(hundredths(value)?),
            "hover" => hover = Some(hundredths(value)?),
            "expo" => expo = Some(hundredths(value)?),
            "limit" => {
                let (kind, percent) = match value.split_once(' ') {
                    None if value == "off" => (ThrottleLimitType::Off, None),
                    Some(("scale", percent)) => (ThrottleLimitType::Scale, Some(percent)),
                    Some(("clip", percent)) => (ThrottleLimitType::Clip, Some(percent)),
                    _ => return Err(format!("\"{part}\" can't be read; {help}")),
                };
                let percent = match percent {
                    None => 100,
                    Some(percent) => {
                        let share = units::parse_quantity(percent)
                            .and_then(|q| q.as_a(Dimension::PERCENT))
                            .map_err(|p| format!("{}; {help}", p.0))?
                            * 100.0;
                        let whole = share.round();
                        if (share - whole).abs() > 1e-6 || !(25.0..=100.0).contains(&whole) {
                            return Err(format!("\"{part}\" can't be read; {help}"));
                        }
                        whole as u8
                    }
                };
                limit = Some((kind, percent));
            }
            _ => return Err(format!("\"{part}\" can't be read; {help}")),
        }
    }
    match (mid, hover, expo, limit) {
        (Some(mid), Some(hover), Some(expo), Some((kind, percent))) => {
            Ok((mid, hover, expo, kind, percent))
        }
        _ => Err(format!(
            "\"{text}\" needs mid, hover, expo and limit; {help}"
        )),
    }
}
