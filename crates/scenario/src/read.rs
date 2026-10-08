//! Reading a Scenario file (#11 §2 and §3, ADR-0002).

use opendrone_maths::{Attitude, DEGREE, PilotAngles, PilotRates, Vec3};
use opendrone_pack::document::{Document, Item, Table};
use opendrone_pack::units::{self, Dimension, Expected, Quantity};
use opendrone_pack::{Problem, Problems};
use opendrone_sim::{MotorCommands, PhysicsRate, SimulationTime, StartingMotors};

use crate::format::{SCENARIO_FORMAT, SCENARIO_STEPS};
use crate::measure::Measure;
use crate::rates::{self, Rates};

/// A Scenario, read and checked.
#[derive(Clone, Debug)]
pub struct Scenario {
    /// The file it came from, as problems and reports name it.
    pub file: String,
    pub name: String,
    pub start: Start,
    /// Its Timeline: motor commands for a Physics or Thrust Stand Scenario,
    /// the pilot's sticks and switches (and, for a Flight Controller
    /// Scenario, the sensor readings) otherwise. Empty for a Flight
    /// Controller Scenario fed a table of cases.
    pub inputs: Inputs,
    pub expectations: Vec<Expectation>,
    /// A Flight Controller Scenario's table of cases, each run on a fresh
    /// Flight Controller.
    pub cases: Vec<Case>,
    /// The run lasts until the last moment the file mentions.
    pub length: SimulationTime,
    /// The other runs its Expectations compare with: the same Scenario with
    /// a starting-state item or two changed.
    pub other_runs: Vec<OtherRun>,
}

/// A Timeline, in a run's own steps, in time order.
#[derive(Clone, Debug, PartialEq)]
pub enum Inputs {
    /// A Physics or Thrust Stand Scenario's motor commands and the moments
    /// they start.
    Motors(Vec<(SimulationTime, MotorCommands)>),
    /// A Flight or Flight Controller Scenario's sticks, switches and sensor
    /// readings, each moment with what changes then.
    Pilot(Vec<PilotEntry>),
}

/// One moment of a pilot's Timeline: what changes then. Anything left out
/// holds.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotEntry {
    pub at: SimulationTime,
    pub changes: PilotChanges,
}

/// What a Timeline moment (or a case) sets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotChanges {
    /// Roll, pitch and yaw from −1 to +1 (right, forward, right positive);
    /// throttle from 0 to 1.
    pub roll: Option<Stick>,
    pub pitch: Option<Stick>,
    pub yaw: Option<Stick>,
    pub throttle: Option<Stick>,
    /// The Arm switch (AUX1): on is high.
    pub arm: Option<bool>,
    /// A Flight Controller Scenario's gyro reading from now on, in body axes
    /// (forward, left, up), in radians per second.
    pub rotation: Option<Vec3>,
    /// A Flight Controller Scenario's attitude reading from now on.
    pub attitude: Option<Attitude>,
    /// The Flying Input Device lost or back. A Radio Link drop-out is the
    /// two, its length apart.
    pub input_device: Option<InputDevice>,
    /// Reset: the Quad back where the Scenario starts, its Launch Spot,
    /// powered up fresh.
    pub reset: bool,
}

/// The Flying Input Device lost or back: both are Flight Inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputDevice {
    Lost,
    Back,
}

/// A stick's new position, reached at once or by a ramp from where it was
/// last set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stick {
    pub share: f64,
    pub ramp: bool,
}

/// One case of a Flight Controller Scenario's table: a fresh Flight
/// Controller, one Radio Link frame with these Channels, and one loop with
/// these sensor readings.
#[derive(Clone, Debug)]
pub struct Case {
    /// The line its `[[case]]` starts on.
    pub line: usize,
    pub inputs: PilotChanges,
    pub expectations: Vec<Expectation>,
}

/// Another run of the same Scenario, for Expectations that compare with it.
#[derive(Clone, Debug, PartialEq)]
pub struct OtherRun {
    /// What differs, in words, such as "at 4 kHz" or "with the battery at
    /// 100%".
    pub words: String,
    pub physics_rate: PhysicsRate,
    /// The battery's charge, from 0 to 1, or `None` for the same as this
    /// run's.
    pub battery: Option<f64>,
    /// The random seed, or `None` for the same as this run's.
    pub random_seed: Option<u64>,
    /// Whether its roll and yaw sticks are this run's, mirrored.
    pub mirrored: bool,
    /// The Timeline, in that run's steps.
    pub inputs: Inputs,
    /// It lasts until the last moment its Expectations need.
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

impl Kind {
    /// True for the kinds whose motors are scripted.
    fn scripts_motors(self) -> bool {
        matches!(self, Kind::Physics | Kind::ThrustStand)
    }
}

/// A Scenario's starting state: every item that affects the Simulation, with
/// no hidden defaults (ADR-0002).
#[derive(Clone, Debug)]
pub struct Start {
    pub kind: Kind,
    pub quad: Named,
    /// `None` for a Flight Controller Scenario, which runs the Flight
    /// Controller alone, with no Map.
    pub map: Option<Named>,
    /// From the Map's origin, in metres (east, north, up); zero for a Flight
    /// Controller Scenario.
    pub position: Vec3,
    /// In m/s (east, north, up); zero for a Flight Controller Scenario.
    pub velocity: Vec3,
    /// The attitude, or for a Flight Controller Scenario the attitude
    /// reading it starts with.
    pub attitude: Attitude,
    /// In body axes, in radians per second; for a Flight Controller Scenario
    /// the gyro reading it starts with.
    pub rotation: Vec3,
    pub armed: bool,
    /// `None` for a Flight Controller Scenario, which has no motors.
    pub motors: Option<StartingMotors>,
    pub flight_controller: StartingFlightController,
    /// The battery's charge, from 0 to 1; `None` for a Flight Controller
    /// Scenario, which has no battery.
    pub battery: Option<f64>,
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

/// What an Expectation expects: a value with its tolerance, or, for
/// something that happens, that it never does.
#[derive(Clone, Debug, PartialEq)]
pub enum Expecting {
    Value(Expected),
    Never,
}

impl Expecting {
    /// As our tools write it, such as "670 °/s ± 3%" or "never".
    pub fn text(&self) -> String {
        match self {
            Expecting::Value(expected) => expected.text(),
            Expecting::Never => "never".to_string(),
        }
    }
}

/// One Expectation.
#[derive(Clone, Debug)]
pub struct Expectation {
    pub measure: Measure,
    pub when: When,
    pub expected: Expecting,
    pub basis: Basis,
    /// The line its `[[expect]]` starts on.
    pub line: usize,
    /// What it checks in words, such as "vertical speed at 1 s".
    pub description: String,
    /// Set when it compares this run with another.
    pub compared: Option<Compared>,
}

/// An Expectation that compares this run with another run of the Scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Compared {
    /// Which of the Scenario's other runs.
    pub run: usize,
    /// The same moment or stretch, in that run's steps.
    pub when: When,
    pub how: Comparison,
}

/// How two runs' values are compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// This run's value minus the other's.
    Difference,
    /// This run's value as a share of the other's.
    Ratio,
    /// How far apart this run's value and the other's are, whichever is the
    /// higher: never below zero.
    Gap,
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
    /// For something that happens: how long after the stretch's start it
    /// first does.
    First,
}

impl Statistic {
    const ALL: [(&'static str, Statistic); 5] = [
        ("mean", Statistic::Mean),
        ("lowest", Statistic::Lowest),
        ("highest", Statistic::Highest),
        ("final", Statistic::Final),
        ("first", Statistic::First),
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

const TOP: &[&str] = &["format", "name", "start", "inputs", "expect", "case"];
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
/// The starting-state items a Flight Controller Scenario leaves out: it runs
/// the Flight Controller alone, with no Map, place, motors or battery.
const NOT_FOR_THE_FLIGHT_CONTROLLER_ALONE: &[&str] =
    &["map", "position", "speed", "motors", "battery"];
const ASSISTS: &[&str] = &["input_smoothing", "endless_battery", "auto_arm"];
const PACKET_RATES: [u32; 7] = [50, 100, 150, 250, 333, 500, 1000];
/// The sticks a pilot's Timeline moves, in the order they are read.
const STICKS: [&str; 4] = ["roll", "pitch", "yaw", "throttle"];

/// Reads a Scenario file, naming it `file` in problems. Every problem is
/// listed at once.
pub fn read_scenario(file: &str, text: &str) -> Result<Scenario, Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format_against(SCENARIO_FORMAT, SCENARIO_STEPS, &mut problems);
    let root = doc.root();
    root.refuse_unknown(TOP, &mut problems);
    let name = root.text("name", &mut problems).map(|(n, _)| n.to_string());
    let start_table = root.table("start", &mut problems);
    let start = start_table
        .as_ref()
        .and_then(|start| read_start(start, &mut problems));
    // The rest is read even when the starting state has problems, so every
    // problem is listed at once; its moments need the physics rate and the
    // kind.
    let rate = match (&start, &start_table) {
        (Some(start), _) => start.physics_rate,
        (None, Some(table)) => match physics_rate(table, &mut Problems::new()) {
            Some(rate) => rate,
            None => return Err(problems),
        },
        (None, None) => return Err(problems),
    };
    let kind = match (&start, &start_table) {
        (Some(start), _) => start.kind,
        (None, Some(table)) => kind(table, &mut Problems::new()).unwrap_or(Kind::Physics),
        (None, None) => Kind::Physics,
    };
    let mut reader = Reader {
        file: file.to_string(),
        problems: &mut problems,
        rate,
        kind,
        start: start.clone(),
        moments: Vec::new(),
        other_runs: Vec::new(),
    };
    let has_cases = root.get("case").is_some();
    let inputs = if kind == Kind::FlightController && has_cases {
        if let Some(inputs) = root.get("inputs") {
            reader.problems.push(inputs.problem(
                "a Flight Controller Scenario is fed either a Timeline in [inputs] or a table of [[case]]s, not both",
            ));
        }
        Inputs::Pilot(Vec::new())
    } else {
        if has_cases && let Some(case) = root.get("case") {
            reader.problems.push(case.problem(
                "a table of [[case]]s feeds only a Flight Controller Scenario, which runs the Flight Controller alone",
            ));
        }
        reader.inputs(&root)
    };
    let cases = if kind == Kind::FlightController && has_cases {
        if let Some(expect) = root.get("expect") {
            reader.problems.push(expect.problem(
                "a Flight Controller Scenario fed a table of cases puts each Expectation in its [[case]], as [[case.expect]]",
            ));
        }
        reader.cases(&root)
    } else {
        Vec::new()
    };
    let expectations = if cases.is_empty() {
        reader.expectations(&root)
    } else {
        Vec::new()
    };
    let other_runs = reader.other_runs;
    let last_input = match &inputs {
        Inputs::Motors(timeline) => timeline.iter().map(|(time, _)| time.ticks()).max(),
        Inputs::Pilot(entries) => entries.iter().map(|e| e.at.ticks()).max(),
    };
    let length = last_input
        .into_iter()
        .chain(expectations.iter().map(|e| match e.when {
            When::At(tick) => tick,
            When::Over { to, .. } => to,
        }))
        .chain((!cases.is_empty()).then_some(1))
        .max()
        .unwrap_or(0);
    let (Some(name), Some(start)) = (name, start) else {
        return Err(problems);
    };
    problems.or(Scenario {
        file: file.to_string(),
        name,
        start,
        inputs,
        expectations,
        cases,
        length: SimulationTime::from_ticks(length),
        other_runs,
    })
}

fn kind(start: &Table<'_, '_>, problems: &mut Problems) -> Option<Kind> {
    choice(
        start,
        "kind",
        &[
            ("flight", Kind::Flight),
            ("thrust stand", Kind::ThrustStand),
            ("flight controller", Kind::FlightController),
            ("physics", Kind::Physics),
        ],
        problems,
    )
}

fn read_start(start: &Table<'_, '_>, problems: &mut Problems) -> Option<Start> {
    start.refuse_unknown(START, problems);
    let kind = kind(start, problems);
    let alone = kind == Some(Kind::FlightController);
    if alone {
        for key in NOT_FOR_THE_FLIGHT_CONTROLLER_ALONE {
            if let Some(item) = start.get(key) {
                problems.push(item.problem(format!(
                    "a Flight Controller Scenario runs the Flight Controller alone, with no Map, place, motors or battery, so its [start] has no `{key}`"
                )));
            }
        }
    }
    // A Flight Controller Scenario has no physics items, so they are read
    // only for the other kinds: `None` here means "not read".
    let id = |key: &str, problems: &mut Problems| {
        start.text(key, problems).map(|(id, item)| Named {
            id: id.to_string(),
            line: item.line(),
        })
    };
    let quad = id("quad", problems);
    let map = (!alone).then(|| id("map", problems));
    let position = (!alone).then(|| {
        vector(
            start,
            "position",
            ["east", "north", "up"],
            Dimension::LENGTH,
            problems,
        )
    });
    let velocity = (!alone).then(|| {
        vector(
            start,
            "speed",
            ["east", "north", "up"],
            Dimension::SPEED,
            problems,
        )
    });
    let attitude = attitude(start, "attitude", problems);
    let rotation = rotation(start, "rotation", problems);
    let armed = start.require("armed", problems).and_then(|item| {
        let armed = item.boolean();
        if armed.is_none() {
            problems.push(item.problem("`armed` must be true or false"));
        }
        armed
    });
    let motors = (!alone).then(|| {
        choice(
            start,
            "motors",
            &[
                ("powering up", StartingMotors::PoweringUp),
                ("stopped", StartingMotors::Stopped),
                ("settled", StartingMotors::Settled),
            ],
            problems,
        )
    });
    // A Quad on the thrust stand is held still, so there is no motion for
    // "settled" motors to hold.
    if motors == Some(Some(StartingMotors::Settled))
        && kind == Some(Kind::ThrustStand)
        && let Some(item) = start.get("motors")
    {
        problems.push(item.problem(
            "a Quad on the thrust stand is held still, so there is no motion for \"settled\" motors to hold; start them \"stopped\" (ESCs ready) or \"powering up\" (ESCs just powered)",
        ));
    }
    // "Stopped" means the ESCs are already powered up and ready, and
    // "powering up" that they were just powered. Where our Flight Controller
    // runs, a "fresh" Flight Controller with its ESCs at rest is exactly
    // Reset: the ESCs power up first. So a Flight Scenario's landed start is
    // written "powering up", and starts as Reset leaves the Quad: disarmed
    // and still.
    if motors == Some(Some(StartingMotors::Stopped))
        && kind == Some(Kind::Flight)
        && let Some(item) = start.get("motors")
    {
        problems.push(item.problem(
            "motors \"stopped\" (at rest, with the ESCs already ready) are only for Physics and Thrust Stand Scenarios, which script their motors; where the Flight Controller runs, a start at rest with a \"fresh\" Flight Controller is Reset, whose ESCs power up first: write \"powering up\"",
        ));
    }
    let reset_start =
        motors == Some(Some(StartingMotors::PoweringUp)) && kind == Some(Kind::Flight);
    if reset_start {
        let still = |v: &Option<Option<[f64; 3]>>| v.flatten().is_none_or(|v| v == [0.0; 3]);
        if armed == Some(true)
            && let Some(item) = start.get("armed")
        {
            problems.push(item.problem(
                "a Flight Scenario whose motors start \"powering up\" starts as Reset leaves the Quad, disarmed: `armed` must be false",
            ));
        }
        for (key, still) in [
            ("speed", still(&velocity)),
            ("rotation", rotation.is_none_or(|r| r == Vec3::ZERO)),
        ] {
            if !still && let Some(item) = start.get(key) {
                problems.push(item.problem(format!(
                    "a Flight Scenario whose motors start \"powering up\" starts as Reset leaves the Quad, still: its `{key}` must be zero"
                )));
            }
        }
    }
    let flight_controller = choice(
        start,
        "flight_controller",
        &[("fresh", StartingFlightController::Fresh)],
        problems,
    );
    let battery = (!alone).then(|| {
        quantity(start, "battery", Dimension::PERCENT, problems).and_then(|(value, item)| {
            if (0.0..=1.0).contains(&value) {
                Some(value)
            } else {
                problems.push(item.problem("the battery's charge must be from 0% to 100%"));
                None
            }
        })
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
    let flies = matches!(kind, Some(Kind::Flight | Kind::FlightController));
    if flies
        && matches!(flight_mode, Some(FlightMode::Angle | FlightMode::Horizon))
        && let Some(item) = start.get("flight_mode")
    {
        problems.push(item.problem(
            "the Flight Controller flies Acro so far: Angle and Horizon arrive with their ticket (#51)",
        ));
    }
    let assists = start.table("assists", problems).and_then(|table| {
        table.refuse_unknown(ASSISTS, problems);
        let mut on = |key| {
            let on = choice(&table, key, &[("on", true), ("off", false)], problems);
            let refusal = match key {
                "input_smoothing" => Some("Input smoothing doesn't run yet, so `input_smoothing` must be \"off\": Input smoothing arrives with the Radio Link ticket (#56)"),
                "auto_arm" if alone => Some("Auto-arm is an Assist of the Simulation, in front of the Flight Controller, and a Flight Controller Scenario runs the Flight Controller alone, so `auto_arm` must be \"off\""),
                "auto_arm" => None,
                _ => Some("Endless Battery doesn't run yet, so `endless_battery` must be \"off\": Endless Battery arrives with its ticket (#57)"),
            };
            if flies
                && on == Some(true)
                && let Some(sentence) = refusal
                && let Some(item) = table.get(key)
            {
                problems.push(item.problem(sentence));
            }
            on
        };
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
        .and_then(|rates| rates::read_rates(&rates, problems));
    if kind == Some(Kind::ThrustStand) {
        let moving = |v: &Option<Option<[f64; 3]>>| v.flatten().is_some_and(|v| v != [0.0; 3]);
        for (key, moving) in [
            ("speed", moving(&velocity)),
            ("rotation", rotation.is_some_and(|r| r != Vec3::ZERO)),
        ] {
            if moving && let Some(item) = start.get(key) {
                problems.push(item.problem(format!(
                    "a Quad on the thrust stand is held still: its `{key}` must be zero"
                )));
            }
        }
    }
    let kind = kind?;
    let vector3 = |v: Option<Option<[f64; 3]>>| -> Option<Vec3> {
        match v {
            None => Some(Vec3::ZERO),
            Some(v) => v.map(|[x, y, z]| Vec3::new(x, y, z)),
        }
    };
    Some(Start {
        kind,
        quad: quad?,
        map: optional(map)?,
        position: vector3(position)?,
        velocity: vector3(velocity)?,
        attitude: attitude?,
        rotation: rotation?,
        armed: armed?,
        motors: optional(motors)?,
        flight_controller: flight_controller?,
        battery: optional(battery)?,
        flight_mode: flight_mode?,
        assists: assists?,
        packet_rate: packet_rate?,
        physics_rate: physics_rate?,
        random_seed: random_seed?,
        rates: rates?,
    })
}

/// An item only some kinds read: `None` (not read) stays `None`; one that was
/// read gives its value, or `None` overall when it had a problem.
fn optional<T>(item: Option<Option<T>>) -> Option<Option<T>> {
    match item {
        None => Some(None),
        Some(value) => value.map(Some),
    }
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
    vector_text(text, &item, labels, dimension, problems)
}

fn vector_text(
    text: &str,
    item: &Item<'_, '_>,
    labels: [&str; 3],
    dimension: Dimension,
    problems: &mut Problems,
) -> Option<[f64; 3]> {
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

/// A rotation in pilot words, "roll 100 °/s, pitch 0 °/s, yaw 0 °/s", as body
/// axes.
fn rotation(table: &Table<'_, '_>, key: &str, problems: &mut Problems) -> Option<Vec3> {
    vector(
        table,
        key,
        ["roll", "pitch", "yaw"],
        Dimension::ROTATION_SPEED,
        problems,
    )
    .map(|[roll, pitch, yaw]| PilotRates { roll, pitch, yaw }.to_body())
}

/// "level, heading 0°" or "roll 10°, pitch -5°, heading 90°".
fn attitude(table: &Table<'_, '_>, key: &str, problems: &mut Problems) -> Option<Attitude> {
    let (text, item) = table.text(key, problems)?;
    attitude_text(text, &item, problems)
}

fn attitude_text(text: &str, item: &Item<'_, '_>, problems: &mut Problems) -> Option<Attitude> {
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

/// One Timeline moment as written, in seconds, so another run can count it in
/// its own steps.
#[derive(Clone, Debug)]
struct Moment {
    seconds: f64,
    line: usize,
    text: String,
    entry: Entry,
}

#[derive(Clone, Copy, Debug)]
enum Entry {
    Motors(MotorCommands),
    Pilot(PilotChanges),
}

struct Reader<'p> {
    file: String,
    problems: &'p mut Problems,
    rate: PhysicsRate,
    kind: Kind,
    start: Option<Start>,
    /// Every Timeline moment.
    moments: Vec<Moment>,
    other_runs: Vec<OtherRun>,
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

    fn inputs(&mut self, root: &Table<'_, '_>) -> Inputs {
        let Some(inputs) = root.table("inputs", self.problems) else {
            return self
                .timeline_at(self.rate, false)
                .unwrap_or(Inputs::Motors(Vec::new()));
        };
        inputs.refuse_unknown(&["timeline"], self.problems);
        let Some(timeline) = inputs
            .require("timeline", self.problems)
            .and_then(|t| t.array(self.problems))
        else {
            return self
                .timeline_at(self.rate, false)
                .unwrap_or(Inputs::Motors(Vec::new()));
        };
        let known: &[&str] = match self.kind {
            Kind::Physics | Kind::ThrustStand => &["at", "motors"],
            Kind::Flight => &[
                "at",
                "roll",
                "pitch",
                "yaw",
                "throttle",
                "arm",
                "input_device",
                "radio_link",
                "reset",
            ],
            Kind::FlightController => &[
                "at",
                "roll",
                "pitch",
                "yaw",
                "throttle",
                "arm",
                "rotation",
                "attitude",
                "input_device",
                "radio_link",
            ],
        };
        for entry in timeline {
            let line = entry.line();
            let Some(entry) = entry.table(self.problems) else {
                continue;
            };
            entry.refuse_unknown(known, self.problems);
            let at = entry
                .text("at", self.problems)
                .and_then(|(text, item)| self.moment(text, &item));
            let read = if self.kind.scripts_motors() {
                entry
                    .text("motors", self.problems)
                    .and_then(|(text, item)| self.motor_commands(text, &item))
                    .map(Entry::Motors)
            } else {
                self.pilot_changes(&entry).map(Entry::Pilot)
            };
            // A Radio Link drop-out: the Flying Input Device lost for a
            // while, then back.
            let drop_out = entry
                .get("radio_link")
                .and_then(|item| self.drop_out(&item));
            if drop_out.is_some()
                && let Some(item) = entry.get("input_device")
            {
                self.problems.push(item.problem(
                    "a Radio Link drop-out is the Flying Input Device lost and back already, so one moment says one or the other",
                ));
            }
            if let (Some((_, quantity)), Some(read)) = (at, read) {
                let moment = Moment {
                    seconds: quantity.value,
                    line,
                    text: quantity.text(),
                    entry: read,
                };
                if let Some((length, length_text)) = drop_out {
                    let mut lost = moment.clone();
                    let mut back = moment.clone();
                    if let Entry::Pilot(changes) = &mut lost.entry {
                        changes.input_device = Some(InputDevice::Lost);
                    }
                    back.entry = Entry::Pilot(PilotChanges {
                        input_device: Some(InputDevice::Back),
                        ..PilotChanges::default()
                    });
                    back.seconds = quantity.value + length;
                    back.text = format!(
                        "the end of the {length_text} drop-out from {}",
                        quantity.text()
                    );
                    self.moments.push(lost);
                    self.moments.push(back);
                } else {
                    self.moments.push(moment);
                }
            }
        }
        if !self.kind.scripts_motors() {
            self.check_pilot_timeline(&inputs);
            self.check_flight_inputs();
        }
        self.timeline_at(self.rate, false)
            .unwrap_or(Inputs::Motors(Vec::new()))
    }

    /// A pilot's Timeline must set every stick and the Arm switch at 0 s, so
    /// the run starts from values the file states (ADR-0002); a ramp needs a
    /// value to ramp from.
    fn check_pilot_timeline(&mut self, inputs: &Table<'_, '_>) {
        // Every moment at 0 s counts, in case the file splits them.
        let at_the_start = || self.moments.iter().filter(|m| m.seconds == 0.0);
        let first: Option<&Moment> = at_the_start().next();
        let mut given = [false; 5];
        for moment in at_the_start() {
            if let Entry::Pilot(changes) = moment.entry {
                let sets = [
                    changes.roll.is_some(),
                    changes.pitch.is_some(),
                    changes.yaw.is_some(),
                    changes.throttle.is_some(),
                    changes.arm.is_some(),
                ];
                for (given, sets) in given.iter_mut().zip(sets) {
                    *given |= sets;
                }
            }
        }
        let missing: Vec<&str> = ["roll", "pitch", "yaw", "throttle", "arm"]
            .into_iter()
            .zip(given)
            .filter(|(_, given)| !given)
            .map(|(name, _)| name)
            .collect();
        let problem = |sentence: String| match first {
            Some(moment) => Problem::of(self.file.clone(), moment.line, sentence),
            None => inputs.problem(sentence),
        };
        if !missing.is_empty() {
            let found = problem(format!(
                "the Timeline starts with a moment at 0 s that sets every stick and the Arm switch (ADR-0002); it doesn't set {}",
                missing
                    .iter()
                    .map(|m| format!("`{m}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            self.problems.push(found);
        }
        let mut ramps = Vec::new();
        for (index, moment) in self.moments.iter().enumerate() {
            let Entry::Pilot(changes) = moment.entry else {
                continue;
            };
            for (name, stick) in STICKS.into_iter().zip(sticks(&changes)) {
                if stick.is_some_and(|s| s.ramp) {
                    let earlier = self.moments[..index].iter().any(|m| {
                        m.seconds < moment.seconds
                            && matches!(m.entry, Entry::Pilot(c) if sticks(&c)[stick_index(name)].is_some())
                    });
                    if !earlier {
                        ramps.push(Problem::of(
                            self.file.clone(),
                            moment.line,
                            format!(
                                "`{name}` ramps to its value from where an earlier moment set it, but no earlier moment sets `{name}`"
                            ),
                        ));
                    }
                }
            }
        }
        for found in ramps {
            self.problems.push(found);
        }
    }

    /// The Timeline at `rate` steps a second, with roll and yaw mirrored if
    /// asked; `None` (with a problem) when a moment falls between two steps.
    fn timeline_at(&self, rate: PhysicsRate, mirrored: bool) -> Option<Inputs> {
        let hz = f64::from(rate.hz());
        let mut motors = Vec::new();
        let mut pilot: Vec<PilotEntry> = Vec::new();
        let mut fine = true;
        for moment in &self.moments {
            let Some(tick) = whole_steps(moment.seconds, hz) else {
                fine = false;
                continue;
            };
            let at = SimulationTime::from_ticks(tick);
            match moment.entry {
                Entry::Motors(commands) => motors.push((at, commands)),
                Entry::Pilot(mut changes) => {
                    if mirrored {
                        for stick in [&mut changes.roll, &mut changes.yaw].into_iter().flatten() {
                            stick.share = -stick.share;
                        }
                    }
                    pilot.push(PilotEntry { at, changes });
                }
            }
        }
        if !fine {
            return None;
        }
        // In time order; moments at the same time keep the file's order.
        if self.kind.scripts_motors() {
            Some(Inputs::Motors(motors))
        } else {
            pilot.sort_by_key(|e| e.at);
            Some(Inputs::Pilot(pilot))
        }
    }

    /// The sticks, the Arm switch and, for a Flight Controller Scenario, the
    /// sensor readings a Timeline moment or a case sets.
    fn pilot_changes(&mut self, entry: &Table<'_, '_>) -> Option<PilotChanges> {
        let mut changes = PilotChanges::default();
        let mut fine = true;
        for name in STICKS {
            let Some(item) = entry.get(name) else {
                continue;
            };
            let Some(text) = item.text(self.problems) else {
                fine = false;
                continue;
            };
            match stick(name, text) {
                Ok(stick) => *sticks_mut(&mut changes)[stick_index(name)] = Some(stick),
                Err(sentence) => {
                    self.problems.push(item.problem(sentence));
                    fine = false;
                }
            }
        }
        if let Some(item) = entry.get("arm") {
            match item.text(self.problems) {
                Some("on") => changes.arm = Some(true),
                Some("off") => changes.arm = Some(false),
                Some(other) => {
                    self.problems.push(item.problem(format!(
                        "`arm` is the Arm switch on AUX1: \"on\" (high, armed) or \"off\", not \"{other}\""
                    )));
                    fine = false;
                }
                None => fine = false,
            }
        }
        if entry.get("rotation").is_some() {
            changes.rotation = rotation(entry, "rotation", self.problems);
            fine &= changes.rotation.is_some();
        }
        if let Some(item) = entry.get("attitude") {
            changes.attitude = item
                .text(self.problems)
                .and_then(|text| attitude_text(text, &item, self.problems));
            fine &= changes.attitude.is_some();
        }
        if let Some(item) = entry.get("input_device") {
            match item.text(self.problems) {
                Some("lost") => changes.input_device = Some(InputDevice::Lost),
                Some("back") => changes.input_device = Some(InputDevice::Back),
                Some(other) => {
                    self.problems.push(item.problem(format!(
                        "`input_device` is the Flying Input Device \"lost\" (unplugged, so the Radio Link sends no frames) or \"back\", not \"{other}\""
                    )));
                    fine = false;
                }
                None => fine = false,
            }
        }
        if let Some(item) = entry.get("reset") {
            match item.boolean() {
                Some(true) => changes.reset = true,
                _ => {
                    self.problems.push(item.problem(
                        "`reset = true` presses Reset at this moment; leave `reset` out otherwise",
                    ));
                    fine = false;
                }
            }
        }
        fine.then_some(changes)
    }

    /// A Radio Link drop-out, "drops out for 0.2 s": how long, in seconds and
    /// as written. It must be a whole number of physics steps.
    fn drop_out(&mut self, item: &Item<'_, '_>) -> Option<(f64, String)> {
        let help = "`radio_link` in a Timeline is a drop-out: the Radio Link sends no frames for a while, written like \"drops out for 0.2 s\"";
        let text = item.text(self.problems)?;
        let Some(length) = text.trim().strip_prefix("drops out for ") else {
            self.problems
                .push(item.problem(format!("{help}, not \"{text}\"")));
            return None;
        };
        let (steps, quantity) = self.moment(length, item)?;
        if steps == 0 {
            self.problems
                .push(item.problem(format!("{help}: a drop-out lasts longer than 0 s")));
            return None;
        }
        Some((quantity.value, quantity.text()))
    }

    /// The Flight Inputs a Timeline sends besides the Channels: the Flying
    /// Input Device lost and back (and drop-outs, which are the two), and
    /// Reset. Lost and back take turns, starting with lost. Reset puts the
    /// Quad back where the Scenario starts, its Launch Spot, so only a
    /// Flight Scenario that starts as Reset leaves the Quad (motors
    /// "powering up") has one. With Auto-arm on, no Arm switch is bound.
    fn check_flight_inputs(&mut self) {
        let mut ordered: Vec<&Moment> = self.moments.iter().collect();
        ordered.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
        let mut lost = false;
        let mut found = Vec::new();
        for moment in ordered {
            let Entry::Pilot(changes) = moment.entry else {
                continue;
            };
            match changes.input_device {
                Some(InputDevice::Lost) if lost => found.push((
                    moment.line,
                    format!(
                        "the Flying Input Device is lost at {}, but it was already lost: it must come back first",
                        moment.text
                    ),
                )),
                Some(InputDevice::Back) if !lost => found.push((
                    moment.line,
                    format!(
                        "the Flying Input Device is back at {}, but it wasn't lost",
                        moment.text
                    ),
                )),
                Some(InputDevice::Lost) => lost = true,
                Some(InputDevice::Back) => lost = false,
                None => {}
            }
        }
        // A Flight Controller Scenario is refused `reset` as a key it
        // doesn't read, and Auto-arm as an Assist it leaves out.
        let flight = self.kind == Kind::Flight;
        let reset_start = self
            .start
            .as_ref()
            .is_some_and(|start| start.motors == Some(StartingMotors::PoweringUp));
        let auto_arm = flight
            && self
                .start
                .as_ref()
                .is_some_and(|start| start.assists.auto_arm);
        for moment in &self.moments {
            let Entry::Pilot(changes) = moment.entry else {
                continue;
            };
            if changes.reset && flight && self.start.is_some() && !reset_start {
                found.push((
                    moment.line,
                    "Reset puts the Quad back where the Scenario starts, as its Launch Spot, so a Scenario with Reset starts as Reset leaves the Quad: landed and still, disarmed, with its motors \"powering up\"".to_string(),
                ));
            }
            if auto_arm && changes.arm == Some(true) {
                found.push((
                    moment.line,
                    "with Auto-arm on, no Arm switch is bound (it would take over), so `arm` stays \"off\": Auto-arm turns Arm on itself".to_string(),
                ));
            }
        }
        for (line, sentence) in found {
            self.problems
                .push(Problem::of(self.file.clone(), line, sentence));
        }
    }

    /// "0%" for all four motors, or four percentages in Betaflight's motor
    /// order.
    fn motor_commands(&mut self, text: &str, item: &Item<'_, '_>) -> Option<MotorCommands> {
        if let Err(p) = units::refuse_commas_in_numbers(text) {
            self.problems.push(item.problem(p.0));
            return None;
        }
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

    /// A Flight Controller Scenario's `[[case]]`s, each with its inputs and
    /// its `[[case.expect]]`s.
    fn cases(&mut self, root: &Table<'_, '_>) -> Vec<Case> {
        let Some(list) = root
            .require("case", self.problems)
            .and_then(|c| c.array(self.problems))
        else {
            return Vec::new();
        };
        let mut cases = Vec::new();
        for item in list {
            let line = item.line();
            let Some(table) = item.table(self.problems) else {
                continue;
            };
            table.refuse_unknown(
                &[
                    "roll", "pitch", "yaw", "throttle", "arm", "rotation", "attitude", "expect",
                ],
                self.problems,
            );
            let inputs = self.pilot_changes(&table);
            if let Some(inputs) = &inputs {
                let missing: Vec<String> = ["roll", "pitch", "yaw", "throttle", "arm"]
                    .into_iter()
                    .zip([
                        inputs.roll.is_some(),
                        inputs.pitch.is_some(),
                        inputs.yaw.is_some(),
                        inputs.throttle.is_some(),
                        inputs.arm.is_some(),
                    ])
                    .filter(|(_, given)| !given)
                    .map(|(name, _)| format!("`{name}`"))
                    .collect();
                if !missing.is_empty() {
                    self.problems.push(table.problem(format!(
                        "a case sets every stick and the Arm switch (ADR-0002); this one doesn't set {}",
                        missing.join(", ")
                    )));
                }
                for name in STICKS {
                    if sticks(inputs)[stick_index(name)].is_some_and(|s| s.ramp) {
                        self.problems.push(table.problem(format!(
                            "a case is one loop, so `{name}` can't ramp: give the value itself"
                        )));
                    }
                }
            }
            let words = inputs.as_ref().map_or_else(String::new, case_words);
            let Some(expect) = table
                .require("expect", self.problems)
                .and_then(|e| e.array(self.problems))
            else {
                continue;
            };
            if expect.is_empty() {
                self.problems
                    .push(table.problem("a case needs at least one [[case.expect]]"));
            }
            let expectations: Vec<Expectation> = expect
                .iter()
                .filter_map(|item| {
                    let table = item.table(self.problems)?;
                    self.case_expectation(&table, item.line(), &words)
                })
                .collect();
            if let Some(inputs) = inputs {
                cases.push(Case {
                    line,
                    inputs,
                    expectations,
                });
            }
        }
        cases
    }

    /// One `[[case.expect]]`: `what`, `value` and `basis`, measured after the
    /// case's one loop.
    fn case_expectation(
        &mut self,
        table: &Table<'_, '_>,
        line: usize,
        words: &str,
    ) -> Option<Expectation> {
        table.refuse_unknown(&["what", "value", "basis"], self.problems);
        let measure = self.measure(table);
        let basis = table
            .text("basis", self.problems)
            .and_then(|(text, item)| self.basis(text, &item));
        let expected =
            table.text("value", self.problems).and_then(
                |(text, item)| match units::parse_expected(text) {
                    Ok(expected) => Some((expected, item)),
                    Err(p) => {
                        self.problems.push(item.problem(p.0));
                        None
                    }
                },
            );
        let (measure, basis, (expected, item)) = (measure?, basis?, expected?);
        if measure.event().is_some() {
            self.problems.push(item.problem(format!(
                "a case is one loop, so \"{}\", something that happens between two loops, can't be seen in one",
                measure.name()
            )));
            return None;
        }
        if expected.dimension() != measure.dimension() {
            self.problems.push(item.problem(format!(
                "{} is {}",
                measure.name(),
                measure.dimension().described()
            )));
            return None;
        }
        Some(Expectation {
            measure,
            when: When::At(1),
            expected: Expecting::Value(expected),
            basis,
            line,
            description: format!("{} with {words}", measure.name()),
            compared: None,
        })
    }

    /// `what`: something this kind of Scenario can measure.
    fn measure(&mut self, table: &Table<'_, '_>) -> Option<Measure> {
        let (text, item) = table.text("what", self.problems)?;
        let Some(measure) = Measure::named(text) else {
            self.problems.push(item.problem(format!(
                "the runner can't measure \"{text}\" yet; it measures {}; and it sees when these happen: {}",
                Measure::names().join(", "),
                Measure::events().join(", ")
            )));
            return None;
        };
        let refusal = match self.kind {
            Kind::FlightController if !measure.of_the_flight_controller() => Some(format!(
                "a Flight Controller Scenario runs the Flight Controller alone, so it measures only what the Flight Controller does, not {text}"
            )),
            Kind::Physics | Kind::ThrustStand if measure.of_the_flight_controller() => {
                Some(format!(
                    "{text} is our Flight Controller's, but a {} Scenario's motors are scripted",
                    if self.kind == Kind::Physics {
                        "Physics"
                    } else {
                        "Thrust Stand"
                    }
                ))
            }
            _ => None,
        };
        if let Some(sentence) = refusal {
            self.problems.push(item.problem(sentence));
            return None;
        }
        Some(measure)
    }

    fn expectation(&mut self, table: &Table<'_, '_>, line: usize) -> Option<Expectation> {
        table.refuse_unknown(
            &[
                "what", "at", "value", "over", "mean", "lowest", "highest", "final", "first",
                "against", "compare", "basis",
            ],
            self.problems,
        );
        let measure = self.measure(table);
        let basis = table
            .text("basis", self.problems)
            .and_then(|(text, item)| self.basis(text, &item));
        let compared = self.comparison(table);
        // When, in seconds: a moment, or a stretch's start and end, with the
        // line that says it.
        let (when, seconds, expected_text, description_time) = match (
            table.get("at"),
            table.get("over"),
        ) {
            (Some(_), Some(_)) | (None, None) => {
                self.problems.push(table.problem(
                        "an Expectation says either `at` a moment, with `value`, or `over` a stretch, with one of `mean`, `lowest`, `highest` or `final` (or `first`, for something that happens)",
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
                (
                    When::At(tick),
                    (time.value, None, at.line()),
                    (value, item),
                    format!("at {}", time.text()),
                )
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
                            "an Expectation over a stretch gives exactly one of `mean`, `lowest`, `highest` or `final` (or `first`, for something that happens)",
                        ));
                    return None;
                };
                let text = item.text(self.problems)?;
                let ((from, to), (from_time, to_time)) = stretch?;
                (
                    When::Over {
                        from,
                        to,
                        statistic: *statistic,
                    },
                    (from_time.value, Some(to_time.value), over.line()),
                    (text, item.clone()),
                    format!("{word} over {} to {}", from_time.text(), to_time.text()),
                )
            }
        };
        let (expected_text, expected_item) = expected_text;
        let first = matches!(
            when,
            When::Over {
                statistic: Statistic::First,
                ..
            }
        );
        let never = expected_text.trim() == "never";
        let expected = if first && never {
            Some(Expecting::Never)
        } else if never {
            self.problems.push(expected_item.problem(
                "\"never\" is for something that happens, over a stretch with `first`, such as `what = \"the Quad disarms\"`; a quantity needs a value with a tolerance",
            ));
            None
        } else {
            match units::parse_expected(expected_text) {
                Ok(expected) => Some(Expecting::Value(expected)),
                Err(p) => {
                    self.problems.push(expected_item.problem(p.0));
                    None
                }
            }
        };
        let (measure, basis, expected) = (measure?, basis?, expected?);
        let compared = match compared {
            Some(found) => Some(found?),
            None => None,
        };
        // Something that happens is measured as when it first does, over a
        // stretch; `first` is only for that.
        match (measure.event(), first) {
            (Some(_), false) => {
                self.problems.push(table.problem(format!(
                    "\"{}\" is something that happens: say `over` a stretch, with `first`, how long after the stretch's start it first happens, such as \"1.5 s ± 0.01 s\", or \"never\"",
                    measure.name()
                )));
                return None;
            }
            (None, true) => {
                self.problems.push(expected_item.problem(format!(
                    "`first` is for something that happens, such as \"the Quad disarms\", not {}",
                    measure.name()
                )));
                return None;
            }
            (Some(_), true) if compared.is_some() => {
                self.problems.push(table.problem(format!(
                    "\"{}\" is something that happens, which can't be compared with another run yet",
                    measure.name()
                )));
                return None;
            }
            _ => {}
        }
        let expected = match expected {
            Expecting::Never => {
                return Some(Expectation {
                    measure,
                    when,
                    expected: Expecting::Never,
                    basis,
                    line,
                    description: format!("{}, {description_time}", measure.name()),
                    compared: None,
                });
            }
            Expecting::Value(expected) => expected,
        };
        let wanted = match compared {
            Some((Comparison::Ratio, _)) => Dimension::PERCENT,
            _ => measure.dimension(),
        };
        if expected.dimension() != wanted {
            self.problems.push(expected_item.problem(match compared {
                Some((Comparison::Ratio, _)) => format!(
                    "a ratio is a share of the other run's value, such as \"75% ± 3%\", not {}",
                    expected.dimension().described()
                ),
                _ => format!("{} is {}", measure.name(), measure.dimension().described()),
            }));
            return None;
        }
        if compared.is_some() && measure.is_an_angle() {
            self.problems.push(table.problem(format!(
                "{} can't be compared with another run yet, because angles wrap round; compare a rate or a position instead",
                measure.name()
            )));
            return None;
        }
        // Angles are compared the short way round, so no two are more than
        // half a turn apart: a range a whole turn wide accepts every angle.
        if measure.is_an_angle() && expected.width() >= 2.0 * core::f64::consts::PI - 1e-9 {
            self.problems.push(expected_item.problem(format!(
                "\"{}\" accepts every angle, since angles are compared the short way round, so this Expectation checks nothing; give a tolerance of less than half a turn each way",
                expected.text()
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
        if measure.of_the_flight_controller() && when == When::At(0) {
            self.problems.push(expected_item.problem(format!(
                "{} is what a Flight Controller loop did, and the first loop runs during the first step, so it can't be measured at 0 s",
                measure.name()
            )));
            return None;
        }
        let mut description = match when {
            When::At(_) => format!("{} {description_time}", measure.name()),
            When::Over { .. } => format!("{}, {description_time}", measure.name()),
        };
        let compared = match compared {
            None => None,
            Some((how, run)) => {
                let when = self.in_other_run(run, when, seconds)?;
                let words = &self.other_runs[run].words;
                description.push_str(&match how {
                    Comparison::Difference => format!(", minus the same run {words}"),
                    Comparison::Ratio => format!(", as a share of the same run {words}"),
                    Comparison::Gap => format!(", how far from the same run {words}"),
                });
                Some(Compared { run, when, how })
            }
        };
        Some(Expectation {
            measure,
            when,
            expected: Expecting::Value(expected),
            basis,
            line,
            description,
            compared,
        })
    }

    /// `against` and `compare`: the other run an Expectation compares with,
    /// and how. `None` when it compares with nothing; `Some(None)` when it
    /// tries to and can't.
    fn comparison(&mut self, table: &Table<'_, '_>) -> Option<Option<(Comparison, usize)>> {
        let help = "`compare` says how this run's value meets the other's: \"difference\" (this run's minus the other's), \"ratio\" (this run's as a share of the other's) or \"gap\" (how far apart the two are, whichever is higher)";
        match (table.get("against"), table.get("compare")) {
            (None, None) => None,
            (None, Some(compare)) => {
                self.problems.push(compare.problem(
                    "`compare` needs `against`: the other run to compare with, such as `against = { physics_rate = \"4 kHz\" }`",
                ));
                Some(None)
            }
            (Some(against), compare) => {
                let how = match compare {
                    None => {
                        self.problems.push(against.problem(format!(
                            "an Expectation `against` another run needs `compare`: {help}"
                        )));
                        None
                    }
                    Some(item) => match item.text(self.problems) {
                        Some("difference") => Some(Comparison::Difference),
                        Some("ratio") => Some(Comparison::Ratio),
                        Some("gap") => Some(Comparison::Gap),
                        Some(other) => {
                            self.problems
                                .push(item.problem(format!("{help}, not \"{other}\"")));
                            None
                        }
                        None => None,
                    },
                };
                let run = self.other_run(&against);
                Some(how.zip(run))
            }
        }
    }

    /// The other run `against` names, added to the Scenario's other runs
    /// unless an earlier Expectation named the same one.
    fn other_run(&mut self, against: &Item<'_, '_>) -> Option<usize> {
        let table = against.table(self.problems)?;
        table.refuse_unknown(
            &["physics_rate", "battery", "random_seed", "sticks"],
            self.problems,
        );
        let mut problems = Problems::new();
        let rate = table
            .get("physics_rate")
            .map(|_| physics_rate(&table, &mut problems));
        let battery = table.get("battery").map(|item| {
            if self.kind == Kind::FlightController {
                problems.push(item.problem(
                    "a Flight Controller Scenario has no battery, so its other run can't change one",
                ));
                return None;
            }
            quantity(&table, "battery", Dimension::PERCENT, &mut problems).and_then(
                |(value, item)| {
                    if (0.0..=1.0).contains(&value) {
                        Some((value, item.as_str().unwrap_or_default().to_string()))
                    } else {
                        problems.push(item.problem("the battery's charge must be from 0% to 100%"));
                        None
                    }
                },
            )
        });
        let random_seed = table.get("random_seed").map(|item| {
            if self.kind == Kind::FlightController {
                problems.push(item.problem(
                    "a Flight Controller Scenario draws no random numbers, so its other run can't change the random seed",
                ));
                return None;
            }
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
        let mirrored = table.get("sticks").map(|item| {
            match item.text(&mut problems) {
                Some("mirrored") => {}
                Some(other) => {
                    problems.push(item.problem(format!(
                        "`sticks` says how the other run's sticks differ: \"mirrored\" (roll and yaw the other way), not \"{other}\""
                    )));
                    return false;
                }
                None => return false,
            }
            if self.kind != Kind::Flight {
                problems.push(item.problem(
                    "only a Flight Scenario's sticks can be mirrored, where the pilot flies the Quad",
                ));
                return false;
            }
            if let Some(start) = &self.start
                && !mirror_symmetric(start)
            {
                problems.push(item.problem(
                    "mirrored sticks give a mirrored flight only from a start that is its own mirror image: no roll, no roll or yaw rotation, and no speed sideways to the heading",
                ));
                return false;
            }
            true
        });
        let failed = !problems.is_empty();
        self.problems.extend(problems);
        if failed {
            return None;
        }
        if rate.is_none() && battery.is_none() && random_seed.is_none() && mirrored.is_none() {
            self.problems.push(against.problem(
                "`against` names what the other run changes: `physics_rate`, `battery` or `random_seed`, written as in [start], or `sticks = \"mirrored\"`",
            ));
            return None;
        }
        let rate = rate.flatten().unwrap_or(self.rate);
        let battery = battery.flatten();
        let random_seed = random_seed.flatten();
        let mirrored = mirrored.unwrap_or(false);
        let mut words = Vec::new();
        if rate != self.rate {
            words.push(format!("at {}", rate_words(rate)));
        }
        if let Some((_, text)) = &battery {
            words.push(format!("with the battery at {text}"));
        }
        if let Some(seed) = random_seed {
            words.push(format!("with random seed {seed}"));
        }
        if mirrored {
            words.push("with roll and yaw mirrored".to_string());
        }
        let words = if words.is_empty() {
            "with the same starting state".to_string()
        } else {
            words.join(" ")
        };
        let battery = battery.map(|(value, _)| value);
        if let Some(found) = self.other_runs.iter().position(|run| {
            run.physics_rate == rate
                && run.battery == battery
                && run.random_seed == random_seed
                && run.mirrored == mirrored
        }) {
            return Some(found);
        }
        let hz = f64::from(rate.hz());
        let in_steps = self.timeline_at(rate, mirrored);
        if in_steps.is_none() {
            for moment in &self.moments {
                if whole_steps(moment.seconds, hz).is_none() {
                    self.problems.push(Problem::of(
                        self.file.clone(),
                        moment.line,
                        format!(
                            "\"{}\" isn't a whole number of physics steps at {} Hz, the rate of the run compared with on line {}: there one step is {} s",
                            moment.text,
                            rate.hz(),
                            against.line(),
                            1.0 / hz
                        ),
                    ));
                }
            }
        }
        let inputs = in_steps.unwrap_or(Inputs::Motors(Vec::new()));
        let length = match &inputs {
            Inputs::Motors(timeline) => timeline.iter().map(|(t, _)| t.ticks()).max(),
            Inputs::Pilot(entries) => entries.iter().map(|e| e.at.ticks()).max(),
        }
        .unwrap_or(0);
        self.other_runs.push(OtherRun {
            words,
            physics_rate: rate,
            battery,
            random_seed,
            mirrored,
            inputs,
            length: SimulationTime::from_ticks(length),
        });
        Some(self.other_runs.len() - 1)
    }

    /// The same moment or stretch in another run's steps; its length grows
    /// to cover it.
    fn in_other_run(
        &mut self,
        run: usize,
        when: When,
        (from, to, line): (f64, Option<f64>, usize),
    ) -> Option<When> {
        let rate = self.other_runs[run].physics_rate;
        let hz = f64::from(rate.hz());
        let mut steps = |seconds: f64| {
            let tick = whole_steps(seconds, hz);
            if tick.is_none() {
                self.problems.push(Problem::of(
                    self.file.clone(),
                    line,
                    format!(
                        "this moment isn't a whole number of physics steps at {} Hz, the rate of the run it's compared with: there one step is {} s",
                        rate.hz(),
                        1.0 / hz
                    ),
                ));
            }
            tick
        };
        let other = match (when, to) {
            (When::At(_), _) => When::At(steps(from)?),
            (When::Over { statistic, .. }, Some(to)) => {
                let (from, to) = (steps(from), steps(to));
                When::Over {
                    from: from?,
                    to: to?,
                    statistic,
                }
            }
            (When::Over { .. }, None) => return None,
        };
        let end = match other {
            When::At(tick) => tick,
            When::Over { to, .. } => to,
        };
        let length = &mut self.other_runs[run].length;
        *length = SimulationTime::from_ticks(length.ticks().max(end));
        Some(other)
    }

    /// "0 s to 1 s".
    fn stretch(
        &mut self,
        text: &str,
        item: &Item<'_, '_>,
    ) -> Option<((u64, u64), (Quantity, Quantity))> {
        let Some((from, to)) = text.split_once(" to ") else {
            self.problems.push(item.problem(format!(
                "\"{text}\" isn't a stretch of time: write it like \"0 s to 1 s\""
            )));
            return None;
        };
        let from = self.moment(from, item);
        let to = self.moment(to, item);
        let ((from, from_time), (to, to_time)) = (from?, to?);
        if from >= to {
            self.problems
                .push(item.problem(format!("\"{text}\" must end after it starts")));
            return None;
        }
        Some(((from, to), (from_time, to_time)))
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

/// A stick's text: "50%", "-100%", or "ramp to 100%". Roll, pitch and yaw go
/// from −100% to +100%, the throttle from 0% to 100%.
fn stick(name: &str, text: &str) -> Result<Stick, String> {
    let (ramp, value) = match text.trim().strip_prefix("ramp to ") {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let share = units::parse_quantity(value)
        .and_then(|q| q.as_a(Dimension::PERCENT))
        .map_err(|p| p.0)?;
    let throttle = name == "throttle";
    let range = if throttle { 0.0..=1.0 } else { -1.0..=1.0 };
    if !range.contains(&share) {
        return Err(if throttle {
            "the throttle goes from 0% (bottom) to 100% (top)".to_string()
        } else {
            format!("{name} goes from -100% to 100%")
        });
    }
    Ok(Stick { share, ramp })
}

fn sticks(changes: &PilotChanges) -> [Option<Stick>; 4] {
    [changes.roll, changes.pitch, changes.yaw, changes.throttle]
}

fn sticks_mut(changes: &mut PilotChanges) -> [&mut Option<Stick>; 4] {
    [
        &mut changes.roll,
        &mut changes.pitch,
        &mut changes.yaw,
        &mut changes.throttle,
    ]
}

fn stick_index(name: &str) -> usize {
    STICKS.iter().position(|s| *s == name).unwrap_or(0)
}

/// A case in words, for its Expectations' descriptions: what it sets that
/// isn't at rest, such as "roll 50%, arm on".
fn case_words(changes: &PilotChanges) -> String {
    let mut words = Vec::new();
    for (name, stick) in STICKS.into_iter().zip(sticks(changes)) {
        if let Some(stick) = stick
            && stick.share != 0.0
        {
            words.push(format!("{name} {}", percent(stick.share)));
        }
    }
    if changes.arm == Some(true) {
        words.push("arm on".to_string());
    }
    if let Some(rotation) = changes.rotation {
        let r = PilotRates::from_body(rotation);
        words.push(format!(
            "rotation roll {}, pitch {}, yaw {}",
            degrees_per_second(r.roll),
            degrees_per_second(r.pitch),
            degrees_per_second(r.yaw)
        ));
    }
    if let Some(attitude) = changes.attitude {
        let a = attitude.pilot_angles();
        words.push(format!(
            "attitude roll {}°, pitch {}°",
            round(a.roll / DEGREE),
            round(a.pitch / DEGREE)
        ));
    }
    if words.is_empty() {
        "the sticks at rest".to_string()
    } else {
        words.join(", ")
    }
}

fn percent(share: f64) -> String {
    format!("{}%", round(share * 100.0))
}

fn degrees_per_second(rate: f64) -> String {
    format!("{} °/s", round(rate / DEGREE))
}

/// A number written plainly, to at most six decimals.
fn round(value: f64) -> String {
    let text = format!("{:.6}", value);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// True when a start is its own mirror image across the Quad's own upright
/// plane through its nose: no roll, no roll or yaw rotation, and no speed
/// sideways to its heading.
fn mirror_symmetric(start: &Start) -> bool {
    let angles = start.attitude.pilot_angles();
    let rates = PilotRates::from_body(start.rotation);
    let (sin, cos) = opendrone_maths::functions::sin_cos(angles.heading);
    // The heading's right-hand side, in world axes (east, north).
    let sideways = start.velocity.x * cos - start.velocity.y * sin;
    angles.roll.abs() < 1e-12 && rates.roll == 0.0 && rates.yaw == 0.0 && sideways.abs() < 1e-12
}

/// `seconds` as a whole number of steps at `hz` steps a second, or `None`
/// when it falls between two steps.
fn whole_steps(seconds: f64, hz: f64) -> Option<u64> {
    let steps = seconds * hz;
    let whole = steps.round();
    (seconds >= 0.0 && (steps - whole).abs() <= 1e-6).then_some(whole as u64)
}

/// A physics rate as our tools write it, such as "4 kHz" or "8 kHz".
fn rate_words(rate: PhysicsRate) -> String {
    let hz = rate.hz();
    if hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{hz} Hz")
    }
}
