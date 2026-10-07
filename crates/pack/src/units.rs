//! The shared unit list: how every number in a Scenario or Pack file is read.
//!
//! Every number is written as short text with its unit, such as `"9.81 m/s²"`,
//! `"23.0 g"` or `"670 °/s ± 3%"` (#11 §2, #16 §3). This module reads that
//! text, and the Scenario runner and the Pack reader both use it, so the two
//! kinds of file always agree.
//!
//! - **Symbols or plain-keyboard spellings.** `°` or `deg`, `Ω` or `ohm`,
//!   `µ` or `u`, `·` or `*` or a space between units, `²` or `^2`, `⁻¹` or
//!   `^-1`, `±` or `+-` or `+/-`, `−` or `-`, `×` or `x`, `–` or `-`. Our tools
//!   always write symbols ([`Unit::symbol`]).
//! - **Everyday prefixes**, so a number can stay ordinary: kilo `k`, mega `M`
//!   (hertz only), centi `c` (metres only), milli `m` and micro `µ`, on the
//!   units that take them: "140 g·cm²", not "1.4 × 10⁻⁵ kg·m²".
//! - **Percent** is its own kind of number (`"50%"`), never a plain fraction.
//! - **Tolerances** ([`Expected`]): `"± amount"`, `"± percent"` or
//!   `"between X and Y"`, always with units.
//! - **Refused:** radians (angles are degrees, spin speeds RPM), decimal
//!   commas and thousands separators (`"31,2 g"`, `"2,000 °/s"`), and any unit
//!   not on the list below.
//!
//! The list, as [`BASE_UNITS`]: metres, grams, seconds, minutes, hours,
//! hertz, degrees, RPM, volts, amps, ohms, amp-hours, watts, watt-hours, KV
//! (RPM per volt), newtons, percent, decibels, stops, lines and TVL, and any
//! product or quotient of them, such as `m/s²`, `kg·m²`, `kg/m³` or `s⁻¹`.
//!
//! Values are kept in SI units (metres, kilograms, seconds, radians, volts,
//! amps), because that is what the Simulation works in. The conversions use
//! only multiplication and division, which give the same bits on every
//! computer, so the Simulation receives the same numbers everywhere
//! (ADR-0001).

use core::f64::consts::PI;
use core::fmt;

/// A plain sentence saying what is wrong with a piece of number text. The
/// caller adds the file and line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitProblem(pub String);

impl fmt::Display for UnitProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn problem(sentence: impl Into<String>) -> UnitProblem {
    UnitProblem(sentence.into())
}

/// What a number measures, as powers of the base kinds: length, mass, time,
/// angle, current, voltage, and the kinds that never combine (percent,
/// decibels, stops, lines and TVL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dimension([i8; 11]);

const L: usize = 0;
const M: usize = 1;
const T: usize = 2;
const ANGLE_INDEX: usize = 3;
const I: usize = 4;
const V: usize = 5;
const PERCENT_INDEX: usize = 6;
const DECIBEL_INDEX: usize = 7;
const STOP_INDEX: usize = 8;
const LINE_INDEX: usize = 9;
const TVL_INDEX: usize = 10;

impl Dimension {
    /// A plain number with no unit, such as a coefficient.
    pub const NONE: Dimension = Dimension([0; 11]);
    pub const LENGTH: Dimension = Dimension::base(L);
    pub const MASS: Dimension = Dimension::base(M);
    pub const TIME: Dimension = Dimension::base(T);
    pub const ANGLE: Dimension = Dimension::base(ANGLE_INDEX);
    pub const CURRENT: Dimension = Dimension::base(I);
    pub const VOLTAGE: Dimension = Dimension::base(V);
    pub const PERCENT: Dimension = Dimension::base(PERCENT_INDEX);
    pub const DECIBELS: Dimension = Dimension::base(DECIBEL_INDEX);
    pub const STOPS: Dimension = Dimension::base(STOP_INDEX);
    pub const LINES: Dimension = Dimension::base(LINE_INDEX);
    pub const TVL: Dimension = Dimension::base(TVL_INDEX);
    pub const AREA: Dimension = Dimension::LENGTH.times(Dimension::LENGTH);
    pub const VOLUME: Dimension = Dimension::AREA.times(Dimension::LENGTH);
    pub const SPEED: Dimension = Dimension::LENGTH.per(Dimension::TIME);
    pub const ACCELERATION: Dimension = Dimension::SPEED.per(Dimension::TIME);
    pub const ROTATION_SPEED: Dimension = Dimension::ANGLE.per(Dimension::TIME);
    pub const PER_SECOND: Dimension = Dimension::NONE.per(Dimension::TIME);
    pub const INERTIA: Dimension = Dimension::MASS.times(Dimension::AREA);
    pub const DENSITY: Dimension = Dimension::MASS.per(Dimension::VOLUME);
    pub const FORCE: Dimension = Dimension::MASS.times(Dimension::ACCELERATION);
    pub const RESISTANCE: Dimension = Dimension::VOLTAGE.per(Dimension::CURRENT);
    pub const CHARGE: Dimension = Dimension::CURRENT.times(Dimension::TIME);
    pub const POWER: Dimension = Dimension::VOLTAGE.times(Dimension::CURRENT);
    pub const ENERGY: Dimension = Dimension::POWER.times(Dimension::TIME);
    pub const KV: Dimension = Dimension::ROTATION_SPEED.per(Dimension::VOLTAGE);

    const fn base(index: usize) -> Dimension {
        let mut powers = [0; 11];
        powers[index] = 1;
        Dimension(powers)
    }

    const fn times(self, other: Dimension) -> Dimension {
        self.combine(other, 1)
    }

    const fn per(self, other: Dimension) -> Dimension {
        self.combine(other, -1)
    }

    const fn raised(self, power: i8) -> Dimension {
        let mut powers = self.0;
        let mut i = 0;
        while i < powers.len() {
            powers[i] *= power;
            i += 1;
        }
        Dimension(powers)
    }

    const fn combine(self, other: Dimension, sign: i8) -> Dimension {
        let mut powers = self.0;
        let mut i = 0;
        while i < powers.len() {
            powers[i] += sign * other.0[i];
            i += 1;
        }
        Dimension(powers)
    }

    /// What this kind of number is called in a sentence, with an example,
    /// such as `a speed, such as "5 m/s"`.
    pub fn described(self) -> String {
        match DESCRIPTIONS
            .iter()
            .find(|(dimension, ..)| *dimension == self)
        {
            Some((_, name, example)) => format!("{name}, such as \"{example}\""),
            None => "a number in another unit".to_string(),
        }
    }

    fn name(self) -> &'static str {
        DESCRIPTIONS
            .iter()
            .find(|(dimension, ..)| *dimension == self)
            .map_or("a number in another unit", |(_, name, _)| name)
    }
}

const DESCRIPTIONS: &[(Dimension, &str, &str)] = &[
    (Dimension::NONE, "a plain number", "0.5"),
    (Dimension::LENGTH, "a length", "2 m"),
    (Dimension::MASS, "a mass", "23.0 g"),
    (Dimension::TIME, "a time", "1.5 s"),
    (Dimension::ANGLE, "an angle", "30°"),
    (Dimension::CURRENT, "a current", "3.4 A"),
    (Dimension::VOLTAGE, "a voltage", "3.7 V"),
    (Dimension::PERCENT, "a percentage", "50%"),
    (Dimension::DECIBELS, "a level in decibels", "-12 dB"),
    (Dimension::STOPS, "a number of stops", "7 stops"),
    (Dimension::LINES, "a number of lines", "480 lines"),
    (Dimension::TVL, "a sharpness in TVL", "300 TVL"),
    (Dimension::AREA, "an area", "25 cm²"),
    (Dimension::SPEED, "a speed", "5 m/s"),
    (Dimension::ACCELERATION, "an acceleration", "9.81 m/s²"),
    (Dimension::ROTATION_SPEED, "a rotation speed", "670 °/s"),
    (Dimension::PER_SECOND, "a rate per second", "250 Hz"),
    (Dimension::INERTIA, "an inertia", "140 g·cm²"),
    (Dimension::DENSITY, "a density", "1.225 kg/m³"),
    (Dimension::FORCE, "a force", "0.3 N"),
    (Dimension::RESISTANCE, "a resistance", "29 mΩ"),
    (Dimension::CHARGE, "a charge", "320 mAh"),
    (Dimension::POWER, "a power", "25 mW"),
    (Dimension::ENERGY, "an energy", "1.22 Wh"),
    (Dimension::KV, "a motor KV", "19500 KV"),
];

/// One unit on the list, as written without a prefix.
pub struct BaseUnit {
    /// How our tools write it.
    pub symbol: &'static str,
    /// Other spellings that read the same, for plain keyboards.
    pub spellings: &'static [&'static str],
    pub dimension: Dimension,
    /// One of this unit, in SI units (degrees in radians, grams in kg).
    pub in_si: f64,
    /// The prefixes it takes.
    pub prefixes: &'static [Prefix],
}

/// An everyday prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    Kilo,
    Mega,
    Centi,
    Milli,
    Micro,
}

impl Prefix {
    fn symbol(self) -> &'static str {
        match self {
            Prefix::Kilo => "k",
            Prefix::Mega => "M",
            Prefix::Centi => "c",
            Prefix::Milli => "m",
            Prefix::Micro => "µ",
        }
    }

    fn spellings(self) -> &'static [&'static str] {
        match self {
            Prefix::Kilo => &["k"],
            Prefix::Mega => &["M"],
            Prefix::Centi => &["c"],
            Prefix::Milli => &["m"],
            Prefix::Micro => &["µ", "μ", "u"],
        }
    }

    fn scale(self) -> f64 {
        match self {
            Prefix::Kilo => 1e3,
            Prefix::Mega => 1e6,
            Prefix::Centi => 1e-2,
            Prefix::Milli => 1e-3,
            Prefix::Micro => 1e-6,
        }
    }
}

use Prefix::{Centi, Kilo, Mega, Micro, Milli};

/// Every unit OpenDrone files may use. Radians are left out on purpose.
pub const BASE_UNITS: &[BaseUnit] = &[
    unit("m", &[], Dimension::LENGTH, 1.0, &[Kilo, Centi, Milli]),
    unit("g", &[], Dimension::MASS, 1e-3, &[Kilo, Milli]),
    unit("s", &[], Dimension::TIME, 1.0, &[Milli, Micro]),
    unit("min", &[], Dimension::TIME, 60.0, &[]),
    unit("h", &[], Dimension::TIME, 3600.0, &[]),
    unit("Hz", &["hz"], Dimension::PER_SECOND, 1.0, &[Kilo, Mega]),
    unit("°", &["deg"], Dimension::ANGLE, PI / 180.0, &[]),
    unit(
        "RPM",
        &["rpm"],
        Dimension::ROTATION_SPEED,
        2.0 * PI / 60.0,
        &[],
    ),
    unit("V", &[], Dimension::VOLTAGE, 1.0, &[Milli]),
    unit("A", &[], Dimension::CURRENT, 1.0, &[Milli]),
    unit(
        "Ω",
        &["ohm", "ohms"],
        Dimension::RESISTANCE,
        1.0,
        &[Milli, Kilo],
    ),
    unit("Ah", &[], Dimension::CHARGE, 3600.0, &[Milli]),
    unit("W", &[], Dimension::POWER, 1.0, &[Milli, Kilo]),
    unit("Wh", &[], Dimension::ENERGY, 3600.0, &[]),
    unit("KV", &["kv", "Kv"], Dimension::KV, 2.0 * PI / 60.0, &[]),
    unit("N", &[], Dimension::FORCE, 1.0, &[Milli]),
    unit("%", &["percent"], Dimension::PERCENT, 0.01, &[]),
    unit("dB", &["db"], Dimension::DECIBELS, 1.0, &[]),
    unit("stops", &["stop"], Dimension::STOPS, 1.0, &[]),
    unit("lines", &[], Dimension::LINES, 1.0, &[]),
    unit("TVL", &["tvl"], Dimension::TVL, 1.0, &[]),
];

const fn unit(
    symbol: &'static str,
    spellings: &'static [&'static str],
    dimension: Dimension,
    in_si: f64,
    prefixes: &'static [Prefix],
) -> BaseUnit {
    BaseUnit {
        symbol,
        spellings,
        dimension,
        in_si,
        prefixes,
    }
}

/// Spellings that mean radians, which OpenDrone files never use.
const RADIANS: &[&str] = &["rad", "rads", "radian", "radians"];

/// A unit as written: base units with their prefixes and powers, such as
/// `kg·m²` or `m/s²`.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    factors: Vec<Factor>,
    dimension: Dimension,
    in_si: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Factor {
    prefix: Option<Prefix>,
    base: usize,
    power: i8,
}

impl Unit {
    /// No unit: a plain number.
    pub fn none() -> Unit {
        Unit {
            factors: Vec::new(),
            dimension: Dimension::NONE,
            in_si: 1.0,
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    /// One of this unit, in SI units.
    pub fn in_si(&self) -> f64 {
        self.in_si
    }

    /// How our tools write it: symbols, `·` between units, superscript
    /// powers, and anything divided by after one `/`.
    pub fn symbol(&self) -> String {
        let write = |factors: &[&Factor], negate: bool| -> String {
            let mut text = String::new();
            for (i, factor) in factors.iter().enumerate() {
                if i > 0 {
                    text.push('·');
                }
                if let Some(prefix) = factor.prefix {
                    text.push_str(prefix.symbol());
                }
                text.push_str(BASE_UNITS[factor.base].symbol);
                let power = if negate { -factor.power } else { factor.power };
                text.push_str(&superscript(power));
            }
            text
        };
        let above: Vec<&Factor> = self.factors.iter().filter(|f| f.power > 0).collect();
        let below: Vec<&Factor> = self.factors.iter().filter(|f| f.power < 0).collect();
        match (above.is_empty(), below.is_empty()) {
            (_, true) => write(&above, false),
            (true, false) => write(&below, false),
            (false, false) => format!("{}/{}", write(&above, false), write(&below, true)),
        }
    }

    /// A number in this unit, written by our tools: 3 significant figures,
    /// then the unit's symbol. `value` is in SI units.
    pub fn write(&self, value: f64) -> String {
        write_number(&significant_figures(value / self.in_si, 3), self)
    }

    /// Reads the unit part of `whole`, the full text being read.
    fn parse(text: &str, whole: &str) -> Result<Unit, UnitProblem> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(Unit::none());
        }
        let (above, below) = match text.split_once('/') {
            Some((above, below)) => (above.trim(), Some(below.trim())),
            None => (text, None),
        };
        let mut factors = Vec::new();
        if above != "1" {
            for factor in above.split(['·', '*', '.', ' ']).filter(|f| !f.is_empty()) {
                factors.push(parse_factor(factor, whole)?);
            }
        }
        if let Some(below) = below {
            if below.contains('/') {
                return Err(problem(format!(
                    "\"{whole}\" divides twice; write one \"/\", such as \"kg/m³\""
                )));
            }
            for factor in below.split(['·', '*', '.', ' ']).filter(|f| !f.is_empty()) {
                let mut factor = parse_factor(factor, whole)?;
                factor.power = -factor.power;
                factors.push(factor);
            }
        }
        let mut dimension = Dimension::NONE;
        let mut in_si = 1.0;
        for factor in &factors {
            let base = &BASE_UNITS[factor.base];
            dimension = dimension.times(base.dimension.raised(factor.power));
            let one = base.in_si * factor.prefix.map_or(1.0, Prefix::scale);
            for _ in 0..factor.power.unsigned_abs() {
                if factor.power > 0 {
                    in_si *= one;
                } else {
                    in_si /= one;
                }
            }
        }
        Ok(Unit {
            factors,
            dimension,
            in_si,
        })
    }
}

fn superscript(power: i8) -> String {
    match power {
        1 => String::new(),
        2 => "²".into(),
        3 => "³".into(),
        -1 => "⁻¹".into(),
        -2 => "⁻²".into(),
        -3 => "⁻³".into(),
        other => format!("^{other}"),
    }
}

/// Reads one unit with its prefix and power, such as `cm²` or `s^-1`.
fn parse_factor(factor: &str, whole: &str) -> Result<Factor, UnitProblem> {
    let (name, power) = split_power(factor).ok_or_else(|| {
        problem(format!(
            "\"{whole}\": can't read the power in \"{factor}\"; write it like \"m²\" or \"m^2\""
        ))
    })?;
    if RADIANS.contains(&name) {
        return Err(problem(format!(
            "\"{whole}\" is in radians, which OpenDrone files never use: write degrees (°), or RPM for spin speeds"
        )));
    }
    let base_named = |name: &str| {
        BASE_UNITS
            .iter()
            .position(|unit| unit.symbol == name || unit.spellings.contains(&name))
    };
    if let Some(base) = base_named(name) {
        return Ok(Factor {
            prefix: None,
            base,
            power,
        });
    }
    for prefix in [Kilo, Mega, Centi, Milli, Micro] {
        for spelling in prefix.spellings() {
            if let Some(rest) = name.strip_prefix(spelling)
                && let Some(base) = base_named(rest)
                && BASE_UNITS[base].prefixes.contains(&prefix)
            {
                return Ok(Factor {
                    prefix: Some(prefix),
                    base,
                    power,
                });
            }
        }
    }
    Err(problem(format!(
        "\"{whole}\": \"{name}\" isn't a unit on OpenDrone's unit list"
    )))
}

/// Splits `m²` into `m` and 2. `None` when the power can't be read.
fn split_power(factor: &str) -> Option<(&str, i8)> {
    for (ending, power) in [
        ("⁻¹", -1),
        ("⁻²", -2),
        ("⁻³", -3),
        ("²", 2),
        ("³", 3),
        ("^-1", -1),
        ("^-2", -2),
        ("^-3", -3),
        ("^2", 2),
        ("^3", 3),
        ("^1", 1),
    ] {
        if let Some(name) = factor.strip_suffix(ending) {
            return Some((name, power));
        }
    }
    if factor.contains('^') || factor.contains('⁻') {
        return None;
    }
    Some((factor, 1))
}

/// A number with its unit, such as "9.81 m/s²".
#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    /// The value in SI units (metres, kilograms, seconds, radians, …).
    pub value: f64,
    pub unit: Unit,
    /// The number as written, with a plain `-` for minus.
    pub number: String,
}

impl Quantity {
    pub fn dimension(&self) -> Dimension {
        self.unit.dimension
    }

    /// The value in SI units, if it measures the right kind of thing.
    pub fn as_a(&self, dimension: Dimension) -> Result<f64, UnitProblem> {
        if self.dimension() == dimension {
            Ok(self.value)
        } else {
            Err(problem(format!(
                "\"{}\" is {}, but this needs {}",
                self.text(),
                self.dimension().name(),
                dimension.described()
            )))
        }
    }

    /// The quantity as our tools write it, with the unit's symbol.
    pub fn text(&self) -> String {
        write_number(&self.number, &self.unit)
    }
}

/// Writes a number and a unit: `°` and `%` follow the number directly, as in
/// "30°" and "50%"; every other unit after a space.
fn write_number(number: &str, unit: &Unit) -> String {
    let symbol = unit.symbol();
    if symbol.is_empty() {
        number.to_string()
    } else if symbol == "°" || symbol == "%" {
        format!("{number}{symbol}")
    } else {
        format!("{number} {symbol}")
    }
}

/// Reads a number with its unit, such as `"9.81 m/s²"`, `"-3 m"`, `"30°"`,
/// `"50%"` or `"0.12"`.
pub fn parse_quantity(text: &str) -> Result<Quantity, UnitProblem> {
    refuse_commas_in_numbers(text)?;
    let trimmed = text.trim();
    let (number, rest) = split_number(trimmed).ok_or_else(|| {
        if trimmed.is_empty() {
            problem("there's no number here")
        } else {
            problem(format!(
                "\"{trimmed}\" doesn't start with a number, such as \"2 m\" or \"-9.81 m/s²\""
            ))
        }
    })?;
    let unit = Unit::parse(rest, trimmed)?;
    // Rust reads decimal text exactly the same way on every computer.
    let value: f64 = number.parse().map_err(|_| {
        problem(format!(
            "\"{trimmed}\" doesn't start with a readable number"
        ))
    })?;
    Ok(Quantity {
        value: value * unit.in_si,
        unit,
        number,
    })
}

/// Splits `"-9.81 m/s²"` into `"-9.81"` (with a plain minus) and `"m/s²"`.
fn split_number(text: &str) -> Option<(String, &str)> {
    let mut number = String::new();
    let mut chars = text.char_indices().peekable();
    if let Some(&(_, sign)) = chars.peek()
        && matches!(sign, '+' | '-' | '−')
    {
        if sign != '+' {
            number.push('-');
        }
        chars.next();
    }
    let mut end = text.len();
    let mut digits = 0;
    let mut points = 0;
    for (index, c) in chars {
        match c {
            '0'..='9' => digits += 1,
            '.' if points == 0 => points += 1,
            _ => {
                end = index;
                break;
            }
        }
        number.push(c);
    }
    if digits == 0 {
        return None;
    }
    Some((number, &text[end..]))
}

/// Refuses a comma between two digits, such as `"31,2 g"` or `"2,000 °/s"`,
/// with the fix: a decimal point, or no thousands separator. Commas between
/// parts, such as in `"mid 0.50, hover 0.50"`, are fine. Anything that splits
/// number text on commas runs this on the whole text first.
///
/// The fix is offered only when it can't be wrong: one decimal comma, or
/// thousands separators only. Several commas between digits, as in
/// `"25,25,25,25%"`, may be a list written without spaces, so then the
/// sentence says how to write both instead.
pub fn refuse_commas_in_numbers(text: &str) -> Result<(), UnitProblem> {
    let text = text.trim();
    let chars: Vec<char> = text.chars().collect();
    let mut fixed = String::with_capacity(text.len());
    let mut decimal_commas = 0;
    let mut thousands = false;
    for (i, &c) in chars.iter().enumerate() {
        let between_digits = c == ','
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !between_digits {
            fixed.push(c);
            continue;
        }
        let digits_after = chars[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits_after == 3 {
            thousands = true;
        } else {
            decimal_commas += 1;
            fixed.push('.');
        }
    }
    if decimal_commas == 0 && !thousands {
        return Ok(());
    }
    let what = if text.contains(", ") {
        format!("\"{text}\" has a number OpenDrone can't read")
    } else {
        format!("\"{text}\" isn't a number OpenDrone can read")
    };
    Err(problem(match (decimal_commas, thousands) {
        (0, _) => format!("{what}: numbers have no thousands separators, so write \"{fixed}\""),
        (1, false) => format!("{what}: numbers take a decimal point, so write \"{fixed}\""),
        _ => format!(
            "{what}: numbers take a decimal point and have no thousands separators, and values are separated by a comma and a space, such as \"0.5%, 0.5%\""
        ),
    }))
}

/// Parts of a value with a label each, such as
/// `"roll 70, pitch 90, yaw 140 g·cm²"` or `"0 m east, 0 m north, 2 m up"`.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub label: String,
    pub quantity: Quantity,
}

/// Reads labelled parts, separated by commas. A label may come before its
/// number (`"roll 70"`) or after it (`"2 m up"`), and may be several words
/// (`"max rate 670 °/s"`). A unit may be written once, at the end, for every
/// part: `"front 9, side 9, top 25 cm²"`. That happens only when the last part
/// is the only one with a unit, so in `"rc rate 1.00, max rate 670 °/s, expo
/// 0.10"` each number keeps its own.
pub fn parse_parts(text: &str, labels: &[&str]) -> Result<Vec<Part>, UnitProblem> {
    refuse_commas_in_numbers(text)?;
    let mut parts: Vec<Part> = Vec::new();
    for piece in text.split(',').map(str::trim) {
        let words = piece.split_whitespace().collect::<Vec<_>>().join(" ");
        // A label may be several words, such as "center sensitivity"; the
        // longest label that fits wins, so "max rate" isn't read as "rate".
        let found = labels
            .iter()
            .filter_map(|label| {
                let before = words
                    .strip_prefix(label)
                    .and_then(|rest| rest.strip_prefix(' '));
                let after = words
                    .strip_suffix(label)
                    .and_then(|rest| rest.strip_suffix(' '));
                before.or(after).map(|number| (*label, number.to_string()))
            })
            .max_by_key(|(label, _)| label.len());
        let Some((label, number)) = found else {
            return Err(problem(format!(
                "\"{piece}\" in \"{}\" needs one of these labels: {}",
                text.trim(),
                labels.join(", ")
            )));
        };
        if parts.iter().any(|part| part.label == label) {
            return Err(problem(format!("\"{}\" gives {label} twice", text.trim())));
        }
        let quantity = parse_quantity(&number)
            .map_err(|p| problem(format!("{label} in \"{}\": {}", text.trim(), p.0)))?;
        parts.push(Part {
            label: label.to_string(),
            quantity,
        });
    }
    // A unit written once, on the last part only, is every part's unit.
    if let Some((last, others)) = parts.split_last_mut()
        && !last.quantity.unit.factors.is_empty()
        && others
            .iter()
            .all(|part| part.quantity.unit.factors.is_empty())
    {
        for part in others {
            part.quantity.value *= last.quantity.unit.in_si;
            part.quantity.unit = last.quantity.unit.clone();
        }
    }
    Ok(parts)
}

/// An Expectation's expected value with its tolerance.
#[derive(Clone, Debug, PartialEq)]
pub enum Expected {
    /// `"670 °/s ± 3%"` or `"-9.81 m/s² ± 0.001 m/s²"`.
    Around {
        value: Quantity,
        tolerance: Tolerance,
    },
    /// `"between 40% and 50%"`.
    Between { low: Quantity, high: Quantity },
}

/// How far a measured value may be from the expected one.
#[derive(Clone, Debug, PartialEq)]
pub enum Tolerance {
    /// `"± 0.001 m/s²"`: an amount in the value's own kind of unit. For a
    /// value in percent, `"± 5%"` is five percentage points.
    Amount(Quantity),
    /// `"± 3%"` of a value that isn't itself in percent: a share of the value.
    Share(Quantity),
}

impl Expected {
    /// What the expected value measures.
    pub fn dimension(&self) -> Dimension {
        match self {
            Expected::Around { value, .. } => value.dimension(),
            Expected::Between { low, .. } => low.dimension(),
        }
    }

    /// The unit the value is written in, for writing the measured value.
    pub fn unit(&self) -> &Unit {
        match self {
            Expected::Around { value, .. } => &value.unit,
            Expected::Between { low, .. } => &low.unit,
        }
    }

    /// The middle of what is expected, in SI units.
    pub fn centre(&self) -> f64 {
        match self {
            Expected::Around { value, .. } => value.value,
            Expected::Between { low, high } => (low.value + high.value) / 2.0,
        }
    }

    /// Whether a measured value (in SI units) is close enough.
    pub fn accepts(&self, measured: f64) -> bool {
        match self {
            Expected::Around { value, tolerance } => {
                let allowed = match tolerance {
                    Tolerance::Amount(amount) => amount.value.abs(),
                    Tolerance::Share(share) => (value.value * share.value).abs(),
                };
                (measured - value.value).abs() <= allowed
            }
            Expected::Between { low, high } => low.value <= measured && measured <= high.value,
        }
    }

    /// The expected value as our tools write it.
    pub fn text(&self) -> String {
        match self {
            Expected::Around { value, tolerance } => {
                let (Tolerance::Amount(tolerance) | Tolerance::Share(tolerance)) = tolerance;
                let tolerance = tolerance.text();
                format!("{} ± {tolerance}", value.text())
            }
            Expected::Between { low, high } => {
                format!("between {} and {}", low.text(), high.text())
            }
        }
    }
}

/// Reads an expected value with its tolerance: `"X ± amount"`,
/// `"X ± percent"` or `"between X and Y"`, always with units.
pub fn parse_expected(text: &str) -> Result<Expected, UnitProblem> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("between ") {
        let (low, high) = rest.split_once(" and ").ok_or_else(|| {
            problem(format!(
                "\"{trimmed}\" needs \"and\" between its two ends, such as \"between 40% and 50%\""
            ))
        })?;
        let mut low = parse_quantity(low)?;
        let high = parse_quantity(high)?;
        if low.unit.factors.is_empty() && !high.unit.factors.is_empty() {
            low.value *= high.unit.in_si;
            low.unit = high.unit.clone();
        }
        if low.dimension() != high.dimension() {
            return Err(problem(format!(
                "\"{trimmed}\": the two ends measure different things ({} and {})",
                low.dimension().name(),
                high.dimension().name()
            )));
        }
        if low.value > high.value {
            return Err(problem(format!(
                "\"{trimmed}\": the lower end comes first, so write \"between {} and {}\"",
                high.text(),
                low.text()
            )));
        }
        return Ok(Expected::Between { low, high });
    }
    let split = ["±", "+/-", "+-"]
        .iter()
        .find_map(|sign| trimmed.split_once(sign));
    let Some((value, tolerance)) = split else {
        return Err(problem(format!(
            "\"{trimmed}\" needs a tolerance: add \"± amount\" or \"± percent\", or write \"between X and Y\""
        )));
    };
    let value = parse_quantity(value)?;
    let tolerance = parse_quantity(tolerance)?;
    let tolerance = if tolerance.dimension() == Dimension::PERCENT
        && value.dimension() != Dimension::PERCENT
    {
        Tolerance::Share(tolerance)
    } else if tolerance.dimension() == value.dimension() {
        if tolerance.value < 0.0 {
            return Err(problem(format!(
                "\"{trimmed}\": a tolerance can't be below zero"
            )));
        }
        Tolerance::Amount(tolerance)
    } else {
        return Err(problem(format!(
            "\"{trimmed}\": the tolerance must be in the value's own kind of unit, or a percentage; \"{}\" is {}",
            tolerance.text(),
            tolerance.dimension().name()
        )));
    };
    Ok(Expected::Around { value, tolerance })
}

/// The range an Estimate may move within: absolute, such as `"20–50 ms"`, or
/// relative to its value, such as `"×0.5–×2"`.
#[derive(Clone, Debug, PartialEq)]
pub enum Range {
    Absolute { low: Quantity, high: Quantity },
    Relative { low: f64, high: f64 },
}

impl Range {
    /// The range as our tools write it.
    pub fn text(&self) -> String {
        match self {
            Range::Absolute { low, high } if low.unit == high.unit => {
                format!("{}–{}", low.number, high.text())
            }
            Range::Absolute { low, high } => format!("{}–{}", low.text(), high.text()),
            Range::Relative { low, high } => format!("×{low}–×{high}"),
        }
    }

    /// Whether a value (in SI units) is inside an absolute range. A relative
    /// range is measured from the value the Estimate started at, so this
    /// can't tell, and says yes.
    pub fn holds(&self, value: f64) -> bool {
        match self {
            Range::Absolute { low, high } => low.value <= value && value <= high.value,
            Range::Relative { .. } => true,
        }
    }
}

/// Reads an Estimate's range: `"20–50 ms"`, `"0.1–0.6 s⁻¹"`, `"×0.5–×2"`. The
/// unit may be written once, at the end.
pub fn parse_range(text: &str) -> Result<Range, UnitProblem> {
    let trimmed = text.trim();
    let split = trimmed.split_once('–').or_else(|| {
        // A plain-keyboard dash: the first "-" that follows a digit.
        let bytes = trimmed.as_bytes();
        (1..bytes.len())
            .find(|&i| {
                bytes[i] == b'-'
                    && trimmed[..i]
                        .trim_end()
                        .ends_with(|c: char| c.is_ascii_digit())
            })
            .map(|i| (&trimmed[..i], &trimmed[i + 1..]))
    });
    let Some((low, high)) = split else {
        return Err(problem(format!(
            "\"{trimmed}\" isn't a range: write its two ends with a dash, such as \"20–50 ms\" or \"×0.5–×2\""
        )));
    };
    let factor = |side: &str| {
        side.trim()
            .strip_prefix('×')
            .or_else(|| side.trim().strip_prefix('x'))
            .map(str::to_string)
    };
    if let (Some(low), Some(high)) = (factor(low), factor(high)) {
        let read = |side: &str| -> Result<f64, UnitProblem> {
            parse_quantity(side)?.as_a(Dimension::NONE)
        };
        return Ok(Range::Relative {
            low: read(&low)?,
            high: read(&high)?,
        });
    }
    let mut low = parse_quantity(low)?;
    let high = parse_quantity(high)?;
    if low.unit.factors.is_empty() && !high.unit.factors.is_empty() {
        low.value *= high.unit.in_si;
        low.unit = high.unit.clone();
    }
    if low.dimension() != high.dimension() {
        return Err(problem(format!(
            "\"{trimmed}\": the two ends measure different things"
        )));
    }
    if low.value > high.value {
        return Err(problem(format!("\"{trimmed}\": the lower end comes first")));
    }
    Ok(Range::Absolute { low, high })
}

/// `value` rounded to `figures` significant figures, written as a plain
/// decimal with a plain `-` for minus: 9.8066 → "9.81", 2000.4 → "2000",
/// 0.000125 → "0.000125", 90 → "90.0". Rust writes decimals the same way on
/// every computer, so this text is too.
pub fn significant_figures(value: f64, figures: usize) -> String {
    if value.is_nan() {
        return "not a number".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "endless" } else { "-endless" }.to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    let scientific = format!("{:.*e}", figures.saturating_sub(1), value);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust's scientific format has an exponent");
    let exponent: i32 = exponent.parse().expect("the exponent is a whole number");
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let before_point = exponent + 1;
    let mut text = String::new();
    if negative {
        text.push('-');
    }
    if before_point <= 0 {
        text.push_str("0.");
        text.push_str(&"0".repeat(before_point.unsigned_abs() as usize));
        text.push_str(&digits);
    } else if before_point as usize >= digits.len() {
        text.push_str(&digits);
        text.push_str(&"0".repeat(before_point as usize - digits.len()));
    } else {
        let (whole, fraction) = digits.split_at(before_point as usize);
        text.push_str(whole);
        text.push('.');
        text.push_str(fraction);
    }
    text
}
