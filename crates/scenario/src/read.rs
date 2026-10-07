//! Reading a Scenario file (#11 §2 and §3, ADR-0002).

use opendrone_maths::{Attitude, DEGREE, PilotAngles, PilotRates, Vec3};
use opendrone_pack::Problems;
use opendrone_pack::document::{Document, Item, Table};
use opendrone_pack::units::{self, Dimension, Expected, Quantity};
use opendrone_sim::{MotorCommands, PhysicsRate, SimulationTime};

use crate::measure::Measure;

/// A Scenario, read and checked.
#[derive(Clone, Debug)]
pub struct Scenario {
    /// The file it came from, as problems and reports name it.
    pub file: String,
    pub name: String,
    pub start: Start,
    /// The Timeline of a Physics Scenario: motor commands and the moments
    /// they start, in time order.
    pub timeline: Vec<(SimulationTime, MotorCommands)>,
    pub expectations: Vec<Expectation>,
    /// The run lasts until the last moment the file mentions.
    pub length: SimulationTime,
}

/// The four kinds of Scenario (the Verification deep dive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flight,
    ThrustStand,
    FlightController,
    Physics,
}

/// A Scenario's starting state: every item that affects the Simulation, with
/// no hidden defaults (ADR-0002).
#[derive(Clone, Debug)]
pub struct Start {
    pub kind: Kind,
    pub quad: Named,
    pub map: Named,
    /// From the Map's origin, in metres (east, north, up).
    pub position: Vec3,
    /// In m/s (east, north, up).
    pub velocity: Vec3,
    pub attitude: Attitude,
    /// In body axes, in radians per second.
    pub rotation: Vec3,
    pub armed: bool,
    pub motors: StartingMotors,
    pub flight_controller: StartingFlightController,
    /// The battery's charge, from 0 to 1.
    pub battery: f64,
    pub flight_mode: FlightMode,
    pub assists: Assists,
    /// The Radio Link's Packet Rate, in Hz.
    pub packet_rate: u32,
    pub physics_rate: PhysicsRate,
    pub random_seed: u64,
    pub rates: Rates,
}

/// A Quad or Map named by its id, such as `opendrone/whoop-65`, with the line
/// that names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub line: usize,
}

/// How the motors start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartingMotors {
    /// The props aren't turning.
    Stopped,
    /// Spinning at the speed that holds the stated motion, with the ESCs
    /// already running (ADR-0002). Needs the motor model (#41).
    Settled,
}

/// How the Flight Controller starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartingFlightController {
    /// As right after Reset powers it up, with filters starting from the
    /// current sensor readings (ADR-0002).
    Fresh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightMode {
    Acro,
    Angle,
    Horizon,
}

/// Every Assist, on or off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assists {
    pub input_smoothing: bool,
    pub endless_battery: bool,
    pub auto_arm: bool,
}

/// The active Rates, field for field from a Betaflight rate profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Rates {
    pub kind: RatesType,
    /// Each axis's three numbers, as Betaflight's rate profile holds them for
    /// this Rates type (for Actual: centre °/s, max °/s, expo).
    pub roll: [f64; 3],
    pub pitch: [f64; 3],
    pub yaw: [f64; 3],
    /// The rate limit for roll, pitch and yaw, in degrees per second.
    pub limit: [f64; 3],
    pub throttle: ThrottleCurve,
    /// Betaflight's `quickrates_rc_expo`: whether Quick rates apply their expo
    /// to the stick, as RC expo.
    pub quick_rates_rc_expo: bool,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrottleCurve {
    /// Where on the stick the hover point sits, from 0 to 1 (`thr_mid`).
    pub mid: f64,
    /// The throttle output at the hover point, from 0 to 1 (`thr_hover`).
    pub hover: f64,
    /// `thr_expo`, from 0 to 1.
    pub expo: f64,
    pub limit: ThrottleLimit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThrottleLimit {
    Off,
    /// Scales the whole throttle range down to this share (0 to 1).
    Scale(f64),
    /// Cuts the throttle off at this share (0 to 1).
    Clip(f64),
}

/// One Expectation.
#[derive(Clone, Debug)]
pub struct Expectation {
    pub measure: Measure,
    pub when: When,
    pub expected: Expected,
    pub basis: Basis,
    /// The line its `[[expect]]` starts on.
    pub line: usize,
    /// What it checks in words, such as "vertical speed at 1 s".
    pub description: String,
}

/// When an Expectation is measured, in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// The state after this step (0 is the start).
    At(u64),
    /// The states after each step from just after `from` up to `to`.
    Over {
        from: u64,
        to: u64,
        statistic: Statistic,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statistic {
    Mean,
    Lowest,
    Highest,
    Final,
}

impl Statistic {
    const ALL: [(&'static str, Statistic); 4] = [
        ("mean", Statistic::Mean),
        ("lowest", Statistic::Lowest),
        ("highest", Statistic::Highest),
        ("final", Statistic::Final),
    ];

    pub fn word(self) -> &'static str {
        Statistic::ALL
            .iter()
            .find(|(_, s)| *s == self)
            .map_or("", |(word, _)| word)
    }
}

/// Where an Expectation's number comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Basis {
    pub kind: BasisKind,
    /// The citation or the working, after the colon.
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasisKind {
    /// A cited outside reference. Locked.
    Source,
    /// Worked out from physics, with the working shown. Locked.
    Rule,
    /// What the Simulation did when the Expectation was written.
    Observed,
}

impl BasisKind {
    pub fn word(self) -> &'static str {
        match self {
            BasisKind::Source => "Source",
            BasisKind::Rule => "Rule",
            BasisKind::Observed => "Observed",
        }
    }
}

const TOP: &[&str] = &["format", "name", "start", "inputs", "expect"];
const START: &[&str] = &[
    "kind",
    "quad",
    "map",
    "position",
    "attitude",
    "speed",
    "rotation",
    "armed",
    "motors",
    "flight_controller",
    "battery",
    "flight_mode",
    "assists",
    "radio_link",
    "physics_rate",
    "random_seed",
    "rates",
];
const RATES: &[&str] = &[
    "type",
    "roll",
    "pitch",
    "yaw",
    "limit",
    "throttle",
    "quickrates_rc_expo",
];
const ASSISTS: &[&str] = &["input_smoothing", "endless_battery", "auto_arm"];
const PACKET_RATES: [u32; 7] = [50, 100, 150, 250, 333, 500, 1000];

/// Reads a Scenario file, naming it `file` in problems. Every problem is
/// listed at once.
pub fn read_scenario(file: &str, text: &str) -> Result<Scenario, Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    let root = doc.root();
    root.refuse_unknown(TOP, &mut problems);
    let name = root.text("name", &mut problems).map(|(n, _)| n.to_string());
    let start_table = root.table("start", &mut problems);
    let start = start_table
        .as_ref()
        .and_then(|start| read_start(start, &mut problems));
    // The rest is read even when the starting state has problems, so every
    // problem is listed at once; its moments need the physics rate.
    let rate = match (&start, &start_table) {
        (Some(start), _) => start.physics_rate,
        (None, Some(table)) => match physics_rate(table, &mut Problems::new()) {
            Some(rate) => rate,
            None => return Err(problems),
        },
        (None, None) => return Err(problems),
    };
    let kind = start.as_ref().map_or(Kind::Physics, |start| start.kind);
    let mut reader = Reader {
        problems: &mut problems,
        rate,
    };
    let timeline = reader.inputs(&root, kind);
    let expectations = reader.expectations(&root);
    let length = timeline
        .iter()
        .map(|(time, _)| time.ticks())
        .chain(expectations.iter().map(|e| match e.when {
            When::At(tick) => tick,
            When::Over { to, .. } => to,
        }))
        .max()
        .unwrap_or(0);
    let (Some(name), Some(start)) = (name, start) else {
        return Err(problems);
    };
    problems.or(Scenario {
        file: file.to_string(),
        name,
        start,
        timeline,
        expectations,
        length: SimulationTime::from_ticks(length),
    })
}

fn read_start(start: &Table<'_, '_>, problems: &mut Problems) -> Option<Start> {
    start.refuse_unknown(START, problems);
    let kind = choice(
        start,
        "kind",
        &[
            ("flight", Kind::Flight),
            ("thrust stand", Kind::ThrustStand),
            ("flight controller", Kind::FlightController),
            ("physics", Kind::Physics),
        ],
        problems,
    );
    if let (Some(kind), Some(item)) = (kind, start.get("kind"))
        && kind != Kind::Physics
    {
        problems.push(item.problem(
            "only Physics Scenarios can run so far: Flight, Thrust Stand and Flight Controller Scenarios arrive with the Flight Controller and the motor model (#41, #48)",
        ));
    }
    let id = |key: &str, problems: &mut Problems| {
        start.text(key, problems).map(|(id, item)| Named {
            id: id.to_string(),
            line: item.line(),
        })
    };
    let quad = id("quad", problems);
    let map = id("map", problems);
    let position = vector(
        start,
        "position",
        ["east", "north", "up"],
        Dimension::LENGTH,
        problems,
    );
    let velocity = vector(
        start,
        "speed",
        ["east", "north", "up"],
        Dimension::SPEED,
        problems,
    );
    let attitude = attitude(start, problems);
    let rotation = vector(
        start,
        "rotation",
        ["roll", "pitch", "yaw"],
        Dimension::ROTATION_SPEED,
        problems,
    )
    .map(|[roll, pitch, yaw]| PilotRates { roll, pitch, yaw }.to_body());
    let armed = start.require("armed", problems).and_then(|item| {
        let armed = item.boolean();
        if armed.is_none() {
            problems.push(item.problem("`armed` must be true or false"));
        }
        armed
    });
    let motors = choice(
        start,
        "motors",
        &[
            ("stopped", StartingMotors::Stopped),
            ("settled", StartingMotors::Settled),
        ],
        problems,
    );
    if motors == Some(StartingMotors::Settled)
        && let Some(item) = start.get("motors")
    {
        problems.push(item.problem(
            "motors \"settled\" need the motor model, which arrives with the Thrust Stand ticket (#41); write \"stopped\" until then",
        ));
    }
    let flight_controller = choice(
        start,
        "flight_controller",
        &[("fresh", StartingFlightController::Fresh)],
        problems,
    );
    let battery =
        quantity(start, "battery", Dimension::PERCENT, problems).and_then(|(value, item)| {
            if (0.0..=1.0).contains(&value) {
                Some(value)
            } else {
                problems.push(item.problem("the battery's charge must be from 0% to 100%"));
                None
            }
        });
    let flight_mode = choice(
        start,
        "flight_mode",
        &[
            ("Acro", FlightMode::Acro),
            ("Angle", FlightMode::Angle),
            ("Horizon", FlightMode::Horizon),
        ],
        problems,
    );
    let assists = start.table("assists", problems).and_then(|table| {
        table.refuse_unknown(ASSISTS, problems);
        let mut on = |key| choice(&table, key, &[("on", true), ("off", false)], problems);
        Some(Assists {
            input_smoothing: on("input_smoothing")?,
            endless_battery: on("endless_battery")?,
            auto_arm: on("auto_arm")?,
        })
    });
    let packet_rate =
        quantity(start, "radio_link", Dimension::PER_SECOND, problems).and_then(|(hz, item)| {
            match PACKET_RATES.iter().find(|rate| f64::from(**rate) == hz) {
                Some(rate) => Some(*rate),
                None => {
                    problems.push(item.problem(
                    "the Radio Link's Packet Rate must be 50, 100, 150, 250, 333, 500 or 1000 Hz",
                ));
                    None
                }
            }
        });
    let physics_rate = physics_rate(start, problems);
    let random_seed = start.require("random_seed", problems).and_then(|item| {
        match item.integer().and_then(|n| u64::try_from(n).ok()) {
            Some(seed) => Some(seed),
            None => {
                problems.push(item.problem(
                    "`random_seed` must be a whole number of 0 or more, written without quotes",
                ));
                None
            }
        }
    });
    let rates = start
        .table("rates", problems)
        .and_then(|rates| read_rates(&rates, problems));
    Some(Start {
        kind: kind?,
        quad: quad?,
        map: map?,
        position: position.map(|[x, y, z]| Vec3::new(x, y, z))?,
        velocity: velocity.map(|[x, y, z]| Vec3::new(x, y, z))?,
        attitude: attitude?,
        rotation: rotation?,
        armed: armed?,
        motors: motors?,
        flight_controller: flight_controller?,
        battery: battery?,
        flight_mode: flight_mode?,
        assists: assists?,
        packet_rate: packet_rate?,
        physics_rate: physics_rate?,
        random_seed: random_seed?,
        rates: rates?,
    })
}

/// The starting state's physics rate: a whole number of steps a second.
fn physics_rate(start: &Table<'_, '_>, problems: &mut Problems) -> Option<PhysicsRate> {
    let (hz, item) = quantity(start, "physics_rate", Dimension::PER_SECOND, problems)?;
    let whole = (1.0..=f64::from(u32::MAX)).contains(&hz) && hz.fract() == 0.0;
    let rate = whole.then(|| PhysicsRate::from_hz(hz as u32)).flatten();
    if rate.is_none() {
        problems.push(item.problem(
            "the physics rate must be a whole number of steps a second, such as \"8 kHz\"",
        ));
    }
    rate
}

/// One of a few words. Problems list the words.
fn choice<T: Copy>(
    table: &Table<'_, '_>,
    key: &str,
    words: &[(&str, T)],
    problems: &mut Problems,
) -> Option<T> {
    let (text, item) = table.text(key, problems)?;
    match words.iter().find(|(word, _)| *word == text) {
        Some((_, value)) => Some(*value),
        None => {
            let list: Vec<String> = words.iter().map(|(w, _)| format!("\"{w}\"")).collect();
            problems.push(item.problem(format!(
                "`{key}` must be one of {}, not \"{text}\"",
                list.join(", ")
            )));
            None
        }
    }
}

/// One number of the given kind, in SI units, with its item.
fn quantity<'d, 't>(
    table: &Table<'d, 't>,
    key: &str,
    dimension: Dimension,
    problems: &mut Problems,
) -> Option<(f64, Item<'d, 't>)> {
    let (text, item) = table.text(key, problems)?;
    match units::parse_quantity(text).and_then(|q| q.as_a(dimension)) {
        Ok(value) => Some((value, item)),
        Err(p) => {
            problems.push(item.problem(p.0));
            None
        }
    }
}

/// Three labelled parts, such as "0 m east, 0 m north, 2 m up". A single
/// unlabelled zero, such as "0 m/s", means all three are zero.
fn vector(
    table: &Table<'_, '_>,
    key: &str,
    labels: [&str; 3],
    dimension: Dimension,
    problems: &mut Problems,
) -> Option<[f64; 3]> {
    let (text, item) = table.text(key, problems)?;
    // One number on its own, with no label: only zero, meaning all three.
    let starts_with_a_number = text
        .trim_start()
        .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '−' | '.'));
    if starts_with_a_number && !text.contains(',') {
        let single = match units::parse_quantity(text) {
            Ok(single) => single,
            Err(p) => {
                problems.push(item.problem(p.0));
                return None;
            }
        };
        return match single.as_a(dimension) {
            Ok(0.0) => Some([0.0; 3]),
            Ok(_) => {
                problems.push(item.problem(format!(
                    "\"{text}\" needs a direction: give {} each, such as \"{}\"",
                    labels.join(", "),
                    example(&labels, &single)
                )));
                None
            }
            Err(p) => {
                problems.push(item.problem(p.0));
                None
            }
        };
    }
    let parts = match units::parse_parts(text, &labels) {
        Ok(parts) => parts,
        Err(p) => {
            problems.push(item.problem(p.0));
            return None;
        }
    };
    let mut values = [0.0; 3];
    for (value, label) in values.iter_mut().zip(labels) {
        let Some(part) = parts.iter().find(|p| p.label == label) else {
            problems.push(item.problem(format!(
                "\"{text}\" is missing {label}: give {} each",
                labels.join(", ")
            )));
            return None;
        };
        match part.quantity.as_a(dimension) {
            Ok(v) => *value = v,
            Err(p) => {
                problems.push(item.problem(format!("{label}: {}", p.0)));
                return None;
            }
        }
    }
    Some(values)
}

fn example(labels: &[&str; 3], quantity: &Quantity) -> String {
    let zero = quantity.unit.write(0.0);
    let first = quantity.text();
    if matches!(labels[0], "roll") {
        format!(
            "{} {first}, {} {zero}, {} {zero}",
            labels[0], labels[1], labels[2]
        )
    } else {
        format!(
            "{first} {}, {zero} {}, {zero} {}",
            labels[0], labels[1], labels[2]
        )
    }
}

/// "level, heading 0°" or "roll 10°, pitch -5°, heading 90°".
fn attitude(start: &Table<'_, '_>, problems: &mut Problems) -> Option<Attitude> {
    let (text, item) = start.text("attitude", problems)?;
    let (level, rest) = match text.trim().strip_prefix("level") {
        Some(rest) => (true, rest.trim_start().trim_start_matches(',').trim()),
        None => (false, text),
    };
    let labels: &[&str] = if level {
        &["heading"]
    } else {
        &["roll", "pitch", "heading"]
    };
    let parts = match units::parse_parts(rest, labels) {
        Ok(parts) => parts,
        Err(p) => {
            problems.push(item.problem(format!(
                "{}; write it like \"level, heading 0°\" or \"roll 10°, pitch -5°, heading 90°\"",
                p.0
            )));
            return None;
        }
    };
    let mut angle = |label: &str| -> Option<f64> {
        if level && label != "heading" {
            return Some(0.0);
        }
        let Some(part) = parts.iter().find(|p| p.label == label) else {
            problems.push(item.problem(format!(
                "\"{text}\" is missing {label}; write it like \"level, heading 0°\" or \"roll 10°, pitch -5°, heading 90°\""
            )));
            return None;
        };
        match part.quantity.as_a(Dimension::ANGLE) {
            Ok(value) => Some(value),
            Err(p) => {
                problems.push(item.problem(format!("{label}: {}", p.0)));
                None
            }
        }
    };
    let roll = angle("roll");
    let pitch = angle("pitch");
    let heading = angle("heading");
    let (roll, pitch, heading) = (roll?, pitch?, heading?);
    if pitch.abs() > 90.0 * DEGREE {
        problems.push(
            item.problem("pitch must be from -90° to 90°; to start upside down, roll by 180°"),
        );
        return None;
    }
    Some(Attitude::from_pilot_angles(PilotAngles {
        roll,
        pitch,
        heading,
    }))
}

fn read_rates(rates: &Table<'_, '_>, problems: &mut Problems) -> Option<Rates> {
    rates.refuse_unknown(RATES, problems);
    let kind = choice(
        rates,
        "type",
        &[
            ("Betaflight", RatesType::Betaflight),
            ("Raceflight", RatesType::Raceflight),
            ("KISS", RatesType::Kiss),
            ("Actual", RatesType::Actual),
            ("Quick", RatesType::Quick),
        ],
        problems,
    );
    let mut axis = |key: &str| -> Option<[f64; 3]> {
        let (text, item) = rates.text(key, problems)?;
        // Read through the shared unit list, as plain numbers: Betaflight's
        // rate profile gives each its meaning for the Rates type.
        let numbers: Vec<Option<f64>> = text
            .split('/')
            .map(|n| {
                units::parse_quantity(n)
                    .and_then(|q| q.as_a(Dimension::NONE))
                    .ok()
            })
            .collect();
        match numbers.as_slice() {
            [Some(a), Some(b), Some(c)] => Some([*a, *b, *c]),
            _ => {
                problems.push(item.problem(format!(
                    "`{key}` must be the rate profile's three numbers for this axis, separated by \"/\", such as \"70 / 670 / 0\""
                )));
                None
            }
        }
    };
    let roll = axis("roll");
    let pitch = axis("pitch");
    let yaw = axis("yaw");
    let limit = rates.text("limit", problems).and_then(|(text, item)| {
        let read = units::parse_quantity(text)
            .and_then(|q| q.as_a(Dimension::ROTATION_SPEED))
            .map(|v| [v; 3])
            .or_else(|_| {
                units::parse_parts(text, &["roll", "pitch", "yaw"]).and_then(|parts| {
                    let mut values = [0.0; 3];
                    for (value, label) in values.iter_mut().zip(["roll", "pitch", "yaw"]) {
                        let part = parts.iter().find(|p| p.label == label).ok_or_else(|| {
                            units::UnitProblem(format!("\"{text}\" is missing {label}"))
                        })?;
                        *value = part.quantity.as_a(Dimension::ROTATION_SPEED)?;
                    }
                    Ok(values)
                })
            });
        match read {
            Ok(values) => Some(values.map(|v| v / DEGREE)),
            Err(p) => {
                problems.push(item.problem(format!(
                    "{}; write one rate limit, such as \"1998 °/s\", or one per axis, such as \"roll 1998, pitch 1998, yaw 1998 °/s\"",
                    p.0
                )));
                None
            }
        }
    });
    let throttle =
        rates
            .text("throttle", problems)
            .and_then(|(text, item)| match throttle_curve(text) {
                Ok(curve) => Some(curve),
                Err(sentence) => {
                    problems.push(item.problem(sentence));
                    None
                }
            });
    let quick_rates_rc_expo = choice(
        rates,
        "quickrates_rc_expo",
        &[("on", true), ("off", false)],
        problems,
    );
    Some(Rates {
        kind: kind?,
        roll: roll?,
        pitch: pitch?,
        yaw: yaw?,
        limit: limit?,
        throttle: throttle?,
        quick_rates_rc_expo: quick_rates_rc_expo?,
    })
}

/// "mid 50%, hover 50%, expo 0, limit off" (or "limit scale 80%",
/// "limit clip 80%"): Betaflight's `thr_mid`, `thr_hover`, `thr_expo`,
/// `throttle_limit_type` and `throttle_limit_percent`.
fn throttle_curve(text: &str) -> Result<ThrottleCurve, String> {
    let help = "write the throttle curve like \"mid 50%, hover 50%, expo 0, limit off\"; the limit is \"off\", \"scale 80%\" or \"clip 80%\"";
    let mut mid = None;
    let mut hover = None;
    let mut expo = None;
    let mut limit = None;
    for part in text.split(',').map(str::trim) {
        let (word, value) = part
            .split_once(' ')
            .ok_or_else(|| format!("\"{part}\" can't be read; {help}"))?;
        let value = value.trim();
        let percent = |text: &str| {
            units::parse_quantity(text)
                .and_then(|q| q.as_a(Dimension::PERCENT))
                .map_err(|p| format!("{}; {help}", p.0))
        };
        match word {
            "mid" => mid = Some(percent(value)?),
            "hover" => hover = Some(percent(value)?),
            "expo" => {
                expo = Some(
                    units::parse_quantity(value)
                        .and_then(|q| q.as_a(Dimension::NONE))
                        .map_err(|p| format!("{}; {help}", p.0))?,
                );
            }
            "limit" => {
                limit = Some(match value.split_once(' ') {
                    None if value == "off" => ThrottleLimit::Off,
                    Some(("scale", share)) => ThrottleLimit::Scale(percent(share)?),
                    Some(("clip", share)) => ThrottleLimit::Clip(percent(share)?),
                    _ => return Err(format!("\"{part}\" can't be read; {help}")),
                });
            }
            _ => return Err(format!("\"{part}\" can't be read; {help}")),
        }
    }
    match (mid, hover, expo, limit) {
        (Some(mid), Some(hover), Some(expo), Some(limit)) => Ok(ThrottleCurve {
            mid,
            hover,
            expo,
            limit,
        }),
        _ => Err(format!(
            "\"{text}\" needs mid, hover, expo and limit; {help}"
        )),
    }
}

struct Reader<'p> {
    problems: &'p mut Problems,
    rate: PhysicsRate,
}

impl Reader<'_> {
    /// A moment, such as "1.25 s", as a whole number of steps.
    fn moment(&mut self, text: &str, item: &Item<'_, '_>) -> Option<(u64, Quantity)> {
        let quantity = match units::parse_quantity(text) {
            Ok(q) => q,
            Err(p) => {
                self.problems.push(item.problem(p.0));
                return None;
            }
        };
        let seconds = match quantity.as_a(Dimension::TIME) {
            Ok(seconds) => seconds,
            Err(p) => {
                self.problems.push(item.problem(p.0));
                return None;
            }
        };
        let hz = self.rate.hz();
        let steps = seconds * f64::from(hz);
        let whole = steps.round();
        if seconds < 0.0 || (steps - whole).abs() > 1e-6 {
            self.problems.push(item.problem(format!(
                "\"{}\" isn't a whole number of physics steps from the start: at {} Hz one step is {} s",
                quantity.text(),
                hz,
                1.0 / f64::from(hz)
            )));
            return None;
        }
        Some((whole as u64, quantity))
    }

    fn inputs(&mut self, root: &Table<'_, '_>, kind: Kind) -> Vec<(SimulationTime, MotorCommands)> {
        let mut moments = Vec::new();
        let Some(inputs) = root.table("inputs", self.problems) else {
            return moments;
        };
        inputs.refuse_unknown(&["timeline"], self.problems);
        let Some(timeline) = inputs
            .require("timeline", self.problems)
            .and_then(|t| t.array(self.problems))
        else {
            return moments;
        };
        for entry in timeline {
            let Some(entry) = entry.table(self.problems) else {
                continue;
            };
            if kind == Kind::Physics {
                entry.refuse_unknown(&["at", "motors"], self.problems);
            }
            let at = entry
                .text("at", self.problems)
                .and_then(|(text, item)| self.moment(text, &item));
            let motors = entry
                .text("motors", self.problems)
                .and_then(|(text, item)| self.motor_commands(text, &item));
            if let (Some((tick, _)), Some(motors)) = (at, motors) {
                moments.push((SimulationTime::from_ticks(tick), motors));
            }
        }
        moments
    }

    /// "0%" for all four motors, or four percentages in Betaflight's motor
    /// order.
    fn motor_commands(&mut self, text: &str, item: &Item<'_, '_>) -> Option<MotorCommands> {
        let mut commands = Vec::new();
        for part in text.split(',') {
            match units::parse_quantity(part).and_then(|q| q.as_a(Dimension::PERCENT)) {
                Ok(share) if (0.0..=1.0).contains(&share) => commands.push(share),
                Ok(_) => {
                    self.problems
                        .push(item.problem("a motor command must be from 0% to 100%"));
                    return None;
                }
                Err(p) => {
                    self.problems.push(item.problem(p.0));
                    return None;
                }
            }
        }
        let all = match commands.as_slice() {
            [one] => [*one; 4],
            [a, b, c, d] => [*a, *b, *c, *d],
            _ => {
                self.problems.push(item.problem(
                    "write one motor command for all four motors, such as \"0%\", or four, in Betaflight's motor order",
                ));
                return None;
            }
        };
        if all.iter().any(|share| *share > 0.0) {
            self.problems.push(item.problem(
                "motor commands above 0% need the motor model, which arrives with the Thrust Stand ticket (#41)",
            ));
            return None;
        }
        let mut motors = MotorCommands::STOPPED;
        for (motor, share) in motors.0.iter_mut().zip(all) {
            motor.throttle = share;
        }
        Some(motors)
    }

    fn expectations(&mut self, root: &Table<'_, '_>) -> Vec<Expectation> {
        let Some(list) = root
            .require("expect", self.problems)
            .and_then(|e| e.array(self.problems))
        else {
            return Vec::new();
        };
        if list.is_empty() {
            self.problems
                .push(root.problem("a Scenario needs at least one [[expect]]"));
        }
        list.iter()
            .filter_map(|item| {
                let table = item.table(self.problems)?;
                self.expectation(&table, item.line())
            })
            .collect()
    }

    fn expectation(&mut self, table: &Table<'_, '_>, line: usize) -> Option<Expectation> {
        table.refuse_unknown(
            &[
                "what", "at", "value", "over", "mean", "lowest", "highest", "final", "basis",
            ],
            self.problems,
        );
        let measure = table.text("what", self.problems).and_then(|(text, item)| {
            let measure = Measure::named(text);
            if measure.is_none() {
                self.problems.push(item.problem(format!(
                    "the runner can't measure \"{text}\" yet; it measures {}",
                    Measure::names().join(", ")
                )));
            }
            measure
        });
        let basis = table
            .text("basis", self.problems)
            .and_then(|(text, item)| self.basis(text, &item));
        let (when, expected_text, description_time) = match (table.get("at"), table.get("over")) {
            (Some(_), Some(_)) | (None, None) => {
                self.problems.push(table.problem(
                    "an Expectation says either `at` a moment, with `value`, or `over` a stretch, with one of `mean`, `lowest`, `highest` or `final`",
                ));
                return None;
            }
            (Some(at), None) => {
                let tick = at
                    .text(self.problems)
                    .and_then(|text| self.moment(text, &at));
                let value = table.text("value", self.problems);
                let (tick, time) = tick?;
                let (value, item) = value?;
                (When::At(tick), (value, item), format!("at {}", time.text()))
            }
            (None, Some(over)) => {
                let stretch = over
                    .text(self.problems)
                    .and_then(|text| self.stretch(text, &over));
                let statistics: Vec<(Statistic, &str, Item<'_, '_>)> = Statistic::ALL
                    .iter()
                    .filter_map(|(word, statistic)| {
                        let item = table.get(word)?;
                        Some((*statistic, *word, item))
                    })
                    .collect();
                let [(statistic, word, item)] = statistics.as_slice() else {
                    self.problems.push(table.problem(
                        "an Expectation over a stretch gives exactly one of `mean`, `lowest`, `highest` or `final`",
                    ));
                    return None;
                };
                let text = item.text(self.problems)?;
                let ((from, to), (from_text, to_text)) = stretch?;
                (
                    When::Over {
                        from,
                        to,
                        statistic: *statistic,
                    },
                    (text, item.clone()),
                    format!("{word} over {from_text} to {to_text}"),
                )
            }
        };
        let (expected_text, expected_item) = expected_text;
        let expected = match units::parse_expected(expected_text) {
            Ok(expected) => Some(expected),
            Err(p) => {
                self.problems.push(expected_item.problem(p.0));
                None
            }
        };
        let (measure, basis, expected) = (measure?, basis?, expected?);
        if expected.dimension() != measure.dimension() {
            self.problems.push(expected_item.problem(format!(
                "{} is {}",
                measure.name(),
                measure.dimension().described()
            )));
            return None;
        }
        if measure.needs_a_step_before() && when == When::At(0) {
            self.problems.push(expected_item.problem(format!(
                "{} needs a step before it, so it can't be measured at 0 s",
                measure.name()
            )));
            return None;
        }
        let description = match when {
            When::At(_) => format!("{} {description_time}", measure.name()),
            When::Over { .. } => format!("{}, {description_time}", measure.name()),
        };
        Some(Expectation {
            measure,
            when,
            expected,
            basis,
            line,
            description,
        })
    }

    /// "0 s to 1 s".
    fn stretch(
        &mut self,
        text: &str,
        item: &Item<'_, '_>,
    ) -> Option<((u64, u64), (String, String))> {
        let Some((from, to)) = text.split_once(" to ") else {
            self.problems.push(item.problem(format!(
                "\"{text}\" isn't a stretch of time: write it like \"0 s to 1 s\""
            )));
            return None;
        };
        let from = self.moment(from, item);
        let to = self.moment(to, item);
        let ((from, from_text), (to, to_text)) = (from?, to?);
        if from >= to {
            self.problems
                .push(item.problem(format!("\"{text}\" must end after it starts")));
            return None;
        }
        Some(((from, to), (from_text.text(), to_text.text())))
    }

    /// "rule: …", "source: …" or "observed: …".
    fn basis(&mut self, text: &str, item: &Item<'_, '_>) -> Option<Basis> {
        let help = "a basis starts with \"source:\" (a cited outside reference), \"rule:\" (worked out from physics, with the working shown) or \"observed:\" (what the Simulation did when the Expectation was written), followed by the citation or the working";
        let Some((kind, rest)) = text.split_once(':') else {
            self.problems.push(item.problem(help));
            return None;
        };
        let kind = match kind.trim().to_lowercase().as_str() {
            "source" => BasisKind::Source,
            "rule" => BasisKind::Rule,
            "observed" => BasisKind::Observed,
            _ => {
                self.problems.push(item.problem(help));
                return None;
            }
        };
        if rest.trim().is_empty() {
            self.problems.push(item.problem(help));
            return None;
        }
        Some(Basis {
            kind,
            text: rest.trim().to_string(),
        })
    }
}
