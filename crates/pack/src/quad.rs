//! Quad definitions (`quads/<id>/quad.toml`) and Test Quads
//! (`scenarios/test-quads/<id>.toml`), #16 §3, §4 and §6.
//!
//! Reading happens in two passes, so a Test Quad can change a value between
//! them:
//!
//! 1. [`read_quad_file`] reads the file's shape: every section and key is one
//!    the schema ([`crate::schema`]) knows, every physics number and camera
//!    limit is `{ value, confidence, source }` with a source from the file's
//!    `[sources]` list, and every Estimate (and only an Estimate) has a range.
//! 2. [`check_quad`] reads every value through the shared unit list, checks it
//!    is the right kind of number, inside its bounds and inside its range,
//!    cross-checks the values that must agree (the Tune's `motor_poles` and
//!    `yaw_motors_reversed` among them), and hands back the checked
//!    [`QuadDefinition`] with its fingerprint.

use std::collections::BTreeMap;

use opendrone_maths::{Fingerprint, Fingerprinter, Mat3, Vec3};
use opendrone_physics::{
    BatteryParameters, Drag, DuctRings, EscParameters, MotorParameters, PropParameters,
    QuadParameters, QuadShape, RotorLayout,
};

use crate::document::{Document, Item, Problem, Problems, Table};
use crate::migration::{self, FileKind};
use crate::schema::{self, Bounds, Form, Key, Kind, Need, SECTIONS, Section, TOP_TEXT};
use crate::tune::Tune;
use crate::units::{self, Dimension, Range};
use crate::values;

/// How well a physics number is known (the Flying deep dive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Measured,
    Manufacturer,
    /// Arithmetic on Measured or Manufacturer numbers.
    Derived,
    /// May move in a Feel Test, inside its range.
    Estimate,
}

impl Confidence {
    fn read(text: &str) -> Option<Confidence> {
        match text {
            "Measured" => Some(Confidence::Measured),
            "Manufacturer" => Some(Confidence::Manufacturer),
            "Derived" => Some(Confidence::Derived),
            "Estimate" => Some(Confidence::Estimate),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Confidence::Measured => "Measured",
            Confidence::Manufacturer => "Manufacturer",
            Confidence::Derived => "Derived",
            Confidence::Estimate => "Estimate",
        }
    }
}

/// A value as the file writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Text in quotes: a number with its unit, a choice or a shape.
    Text(String),
    /// A bare whole number: a count.
    Whole(i64),
    /// `true` or `false`.
    YesNo(bool),
}

impl Value {
    /// The value as the file writes it, without quotes.
    pub fn text(&self) -> String {
        match self {
            Value::Text(text) => text.clone(),
            Value::Whole(n) => n.to_string(),
            Value::YesNo(b) => b.to_string(),
        }
    }
}

/// One setting of a Quad definition, as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub value: Value,
    /// `None` for a Test Quad's change, which is a deliberate test value with
    /// no Confidence, and for settings that never carry one.
    pub confidence: Option<Confidence>,
    pub range: Option<Range>,
    /// The key of its line in `[sources]`.
    pub source: Option<String>,
    /// The file and line it was written on.
    pub file: String,
    pub line: usize,
}

impl Setting {
    pub fn problem(&self, sentence: impl Into<String>) -> Problem {
        Problem::of(&self.file, self.line, sentence)
    }
}

/// A Quad definition's file, read but not yet turned into numbers: every
/// setting by `section.key`, such as `props.grip`.
#[derive(Clone, Debug, PartialEq)]
pub struct QuadFile {
    /// The file it was read from.
    pub file: String,
    /// On-screen text.
    pub name: String,
    pub description: String,
    pub spec_line: String,
    /// The Quad picker's picture: a file in the Quad's folder.
    pub picture: String,
    pub settings: BTreeMap<String, Setting>,
    /// The `[sources]` list: each key and what it names.
    pub sources: BTreeMap<String, String>,
}

/// `[props] grip` for the setting `props.grip`, as the file writes it.
pub fn label(name: &str) -> String {
    match name.split_once('.') {
        Some((section, key)) => format!("[{section}] {key}"),
        None => name.to_string(),
    }
}

/// Reads a Quad definition's file: its shape, Confidences, sources and
/// ranges. [`check_quad`] reads the values.
pub fn read_quad_file(file: &str, text: &str) -> Result<QuadFile, Problems> {
    let (quad, problems) = read_quad_file_as_far_as_it_goes(file, text)?;
    problems.or(quad)
}

/// Reads as much of a Quad definition's file as passes, and every problem
/// with the rest: every setting whose own line is fine is kept. The Feel Test
/// log rules use it to read the version before a change, which an older
/// checker may have passed. It fails only when the file isn't readable TOML.
pub fn read_quad_file_as_far_as_it_goes(
    file: &str,
    text: &str,
) -> Result<(QuadFile, Problems), Problems> {
    let text = migration::upgraded(file, text, FileKind::Quad)?;
    let doc = Document::parse(file, &text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    if doc.is_newer() {
        return Err(problems);
    }
    let root = doc.root();

    let mut sources = BTreeMap::new();
    match root.get("sources") {
        Some(item) => {
            if let Some(table) = item.table(&mut problems) {
                for (key, item) in table.entries() {
                    match item.text(&mut problems) {
                        Some(text) if text.trim().is_empty() => {
                            problems.push(item.problem(format!("the source `{key}` says nothing")))
                        }
                        Some(text) => {
                            sources.insert(key, text.to_string());
                        }
                        None => {}
                    }
                }
            }
        }
        None => problems.push(Problem::of(
            file,
            0,
            "a Quad definition ends with a [sources] list saying where its numbers come from",
        )),
    }

    let mut texts = BTreeMap::new();
    let mut settings = BTreeMap::new();
    for (name, item) in root.entries() {
        match name.as_str() {
            "format" | "sources" => {}
            "based_on" => problems.push(item.problem(
                "only a Test Quad may use `based_on`: Test Quads live in scenarios/test-quads/, and only the Scenario runner reads them",
            )),
            top if TOP_TEXT.contains(&top) => {
                if let Some(text) = item.text(&mut problems) {
                    if text.trim().is_empty() {
                        problems.push(item.problem(format!("`{top}` can't be empty")));
                    }
                    texts.insert(top.to_string(), text.to_string());
                }
            }
            other => match schema::section(other) {
                Some(section) => {
                    if let Some(table) = item.table(&mut problems) {
                        read_section(&doc, section, &table, &sources, &mut settings, &mut problems);
                    }
                }
                None => problems.push(item.problem(format!(
                    "`{other}` isn't something OpenDrone reads in a Quad definition; it reads {}, and the sections {}, then [sources]",
                    TOP_TEXT
                        .iter()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    SECTIONS
                        .iter()
                        .map(|s| format!("[{}]", s.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))),
            },
        }
    }
    for key in TOP_TEXT {
        root.require(key, &mut problems);
    }
    for section in SECTIONS {
        if !section.optional && root.get(section.name).is_none() {
            problems.push(Problem::of(
                file,
                0,
                format!(
                    "the Quad definition is missing its [{}] section",
                    section.name
                ),
            ));
        }
    }
    let text = |key: &str| texts.get(key).cloned().unwrap_or_default();
    let quad = QuadFile {
        file: file.to_string(),
        name: text("name"),
        description: text("description"),
        spec_line: text("spec_line"),
        picture: text("picture"),
        settings,
        sources,
    };
    Ok((quad, problems))
}

fn read_section(
    doc: &Document<'_>,
    section: &Section,
    table: &Table<'_, '_>,
    sources: &BTreeMap<String, String>,
    settings: &mut BTreeMap<String, Setting>,
    problems: &mut Problems,
) {
    for (name, item) in table.entries() {
        let Some(key) = section.key(&name) else {
            problems.push(item.problem(format!(
                "`{name}` isn't something OpenDrone reads in [{}]; it reads {}",
                section.name,
                section
                    .keys
                    .iter()
                    .map(|k| format!("`{}`", k.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
            continue;
        };
        if let Some(setting) = read_setting(doc, section, key, &item, sources, problems) {
            settings.insert(format!("{}.{}", section.name, key.name), setting);
        }
    }
    for key in section.keys.iter().filter(|key| key.need == Need::Always) {
        table.require(key.name, problems);
    }
}

/// Reads one setting's shape: a physics number or camera limit as
/// `{ value, confidence, source }` (with a `range` for an Estimate), or a
/// plain value.
fn read_setting(
    doc: &Document<'_>,
    section: &Section,
    key: &Key,
    item: &Item<'_, '_>,
    sources: &BTreeMap<String, String>,
    problems: &mut Problems,
) -> Option<Setting> {
    let label = format!("[{}] {}", section.name, key.name);
    let file = doc.file().to_string();
    let line = item.line();
    if !key.kind.has_confidence() {
        if item.is_table() {
            let what = match key.kind {
                Kind::Count { .. } => "a count",
                Kind::Choice(_) => "a choice",
                Kind::YesNo => "a yes or no",
                _ if section.name == "camera" => "a camera default",
                _ => "a sound-block value",
            };
            problems.push(item.problem(format!(
                "{label} is {what}, so it carries no Confidence or source: write just the value"
            )));
            return None;
        }
        return Some(Setting {
            value: plain_value(item, key, problems)?,
            confidence: None,
            range: None,
            source: None,
            file,
            line,
        });
    }
    if !item.is_table() {
        let what = if matches!(key.kind, Kind::CameraLimit(_)) {
            "one of the FPV Camera's limits"
        } else {
            "a physics number"
        };
        problems.push(item.problem(format!(
            "{label} is {what}, so it's written {{ value = \"…\", confidence = \"…\", source = \"…\" }}, and an Estimate adds its range"
        )));
        return None;
    }
    let table = item.table(problems)?;
    table.refuse_unknown(&["value", "confidence", "range", "source"], problems);
    let value = table.text("value", problems).map(|(v, _)| v.to_string());
    let confidence = match table.get("confidence") {
        None => {
            problems.push(table.problem(format!(
                "{label} needs a Confidence: add confidence = \"Measured\", \"Manufacturer\", \"Derived\" or \"Estimate\""
            )));
            None
        }
        Some(item) => item.text(problems).and_then(|text| {
            let confidence = Confidence::read(text);
            if confidence.is_none() {
                problems.push(item.problem(format!(
                    "\"{text}\" isn't a Confidence: write Measured, Manufacturer, Derived or Estimate"
                )));
            }
            confidence
        }),
    };
    let source = match table.get("source") {
        None => {
            problems.push(table.problem(format!(
                "{label} needs a source: add source = \"…\", naming a line of this file's [sources] list"
            )));
            None
        }
        Some(item) => item.text(problems).and_then(|text| {
            if sources.contains_key(text) {
                Some(text.to_string())
            } else {
                problems.push(item.problem(format!(
                    "the source \"{text}\" isn't in this file's [sources] list"
                )));
                None
            }
        }),
    };
    let range = match (table.get("range"), confidence) {
        (Some(item), Some(Confidence::Estimate)) => {
            item.text(problems)
                .and_then(|text| match units::parse_range(text) {
                    Ok(range) => Some(range),
                    Err(p) => {
                        problems.push(item.problem(p.0));
                        None
                    }
                })
        }
        (Some(item), Some(other)) => {
            problems.push(item.problem(format!(
                "only an Estimate has a range: {label} is {}, so it's locked; take the range out",
                other.word()
            )));
            None
        }
        (None, Some(Confidence::Estimate)) => {
            problems.push(table.problem(format!(
                "{label} is an Estimate, so it needs the `range` it may move within, such as range = \"×0.5–×2\""
            )));
            None
        }
        _ => None,
    };
    Some(Setting {
        value: Value::Text(value?),
        confidence: Some(confidence?),
        range,
        source: Some(source?),
        file,
        line,
    })
}

/// A value with no Confidence: text, a whole number or `true`/`false`.
fn plain_value(item: &Item<'_, '_>, key: &Key, problems: &mut Problems) -> Option<Value> {
    if let Some(n) = item.integer() {
        return Some(Value::Whole(n));
    }
    if let Some(b) = item.boolean() {
        return Some(Value::YesNo(b));
    }
    if matches!(key.kind, Kind::Count { .. }) && item.as_str().is_none() {
        problems.push(item.problem(format!(
            "`{}` is a count: write a whole number without quotes, such as {} = 3",
            key.name, key.name
        )));
        return None;
    }
    item.text(problems)
        .map(|text| Value::Text(text.to_string()))
}

/// Reads a Test Quad over the file of the real Quad it builds on, and returns
/// the real Quad's id and its settings with the Test Quad's changes. A Test
/// Quad lists only what it changes, so a change to the real Quad carries into
/// it.
pub fn read_test_quad(
    file: &str,
    text: &str,
    base: impl FnOnce(&str) -> Result<QuadFile, Problems>,
) -> Result<(String, QuadFile), Problems> {
    let text = migration::upgraded(file, text, FileKind::TestQuad)?;
    let doc = Document::parse(file, &text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    if doc.is_newer() {
        return Err(problems);
    }
    let root = doc.root();
    let based_on = root.text("based_on", &mut problems);
    root.text("why", &mut problems);
    let Some((based_on, based_on_item)) = based_on else {
        return Err(problems);
    };
    if based_on.starts_with("test/") {
        problems.push(based_on_item.problem(format!(
            "a Test Quad builds on a real Quad, not on another Test Quad (\"{based_on}\")"
        )));
        return Err(problems);
    }
    let mut quad = match base(based_on) {
        Ok(quad) => quad,
        Err(base_problems) => {
            problems.push(based_on_item.problem(format!(
                "can't build on \"{based_on}\" until its own problems are fixed"
            )));
            problems.extend(base_problems);
            return Err(problems);
        }
    };
    for (section, item) in root.entries() {
        if matches!(section.as_str(), "format" | "based_on" | "why") {
            continue;
        }
        let Some(table) = item.table(&mut problems) else {
            continue;
        };
        for (key, item) in table.entries() {
            let name = format!("{section}.{key}");
            let Some(setting) = quad.settings.get_mut(&name) else {
                problems.push(item.problem(format!(
                    "[{section}] {key} isn't a setting of {based_on}, so it can't be changed here"
                )));
                continue;
            };
            if item.is_table() {
                problems.push(item.problem(format!(
                    "a Test Quad's change carries no Confidence or source: write just the value, such as {key} = \"{}\"",
                    setting.value.text()
                )));
                continue;
            }
            let value = if let Some(n) = item.integer() {
                Value::Whole(n)
            } else if let Some(b) = item.boolean() {
                Value::YesNo(b)
            } else if let Some(text) = item.text(&mut problems) {
                Value::Text(text.to_string())
            } else {
                continue;
            };
            *setting = Setting {
                value,
                confidence: None,
                range: None,
                source: None,
                file: doc.file().to_string(),
                line: item.line(),
            };
        }
    }
    problems.or((based_on.to_string(), quad))
}

/// A setting's value once read: numbers in SI units (in the order its form
/// writes them), a count, a choice or a yes/no.
#[derive(Clone, Debug, PartialEq)]
enum Reading {
    Numbers(Vec<f64>),
    Whole(i64),
    Word(String),
    YesNo(bool),
}

/// Reads `text` in `form`, checking each number is the right kind and inside
/// its bounds. A curve comes back as value, place, value, place, ….
pub(crate) fn numbers(label: &str, form: Form, text: &str) -> Result<Vec<f64>, String> {
    let bounded = |value: f64, bounds: Bounds| match bounds.refuses(value) {
        Some(sentence) => Err(format!("{label} {sentence}, but it's \"{}\"", text.trim())),
        None => Ok(value),
    };
    let unit = |p: units::UnitProblem| p.0;
    match form {
        Form::One(dimension, bounds) => {
            let value = units::parse_quantity(text)
                .and_then(|q| q.as_a(dimension))
                .map_err(unit)?;
            Ok(vec![bounded(value, bounds)?])
        }
        Form::Parts(labels, dimension, bounds) => {
            let parts = units::parse_parts(text, labels).map_err(unit)?;
            let mut values = Vec::new();
            for part_label in labels {
                let Some(part) = parts.iter().find(|part| part.label == *part_label) else {
                    return Err(format!(
                        "\"{}\" needs {}, each with its label",
                        text.trim(),
                        labels.join(", ")
                    ));
                };
                let value = part
                    .quantity
                    .as_a(dimension)
                    .map_err(|p| format!("{part_label}: {}", p.0))?;
                values.push(bounded(value, bounds)?);
            }
            Ok(values)
        }
        Form::Box => {
            let sides = values::parse_box(text).map_err(unit)?;
            let mut values = Vec::new();
            for side in sides {
                let value = side.as_a(Dimension::LENGTH).map_err(unit)?;
                values.push(bounded(value, Bounds::AboveZero)?);
            }
            Ok(values)
        }
        Form::At(dimension, condition, bounds) => {
            let (value, at) = values::parse_at(text).map_err(unit)?;
            Ok(vec![
                bounded(value.as_a(dimension).map_err(unit)?, bounds)?,
                bounded(at.as_a(condition).map_err(unit)?, Bounds::AboveZero)?,
            ])
        }
        Form::Curve(dimension, place) => {
            let mut values = Vec::new();
            for (value, at) in values::parse_curve(text).map_err(unit)? {
                values.push(bounded(
                    value.as_a(dimension).map_err(unit)?,
                    Bounds::AboveZero,
                )?);
                values.push(bounded(at.as_a(place).map_err(unit)?, Bounds::Share)?);
            }
            Ok(values)
        }
    }
}

/// The numbers a setting's range applies to: every number, except a value
/// "at" a condition (only the value) and a curve (only its values).
pub(crate) fn ranged(form: Form, numbers: &[f64]) -> Vec<f64> {
    match form {
        Form::At(..) => numbers.iter().take(1).copied().collect(),
        Form::Curve(..) => numbers.iter().step_by(2).copied().collect(),
        _ => numbers.to_vec(),
    }
}

/// Checks a value against its Estimate's absolute range. A relative range
/// (`"×0.5–×2"`) is measured from the value the Estimate started at, which
/// the Feel Test log rules know ([`crate::feel_tests`]), so it passes here.
pub(crate) fn outside_range(
    form: Form,
    range: &Range,
    value_text: &str,
    numbers: &[f64],
) -> Option<String> {
    if let Range::Absolute { low, .. } = range
        && low.dimension() != form.ranged_dimension()
    {
        return Some(format!(
            "its range is {}, but the value needs {}",
            low.text(),
            form.ranged_dimension().described()
        ));
    }
    if ranged(form, numbers).iter().any(|n| !range.holds(*n)) {
        return Some(format!(
            "\"{}\" is outside its range, {}",
            value_text.trim(),
            range.text()
        ));
    }
    None
}

fn read_value(name: &str, key: &Key, setting: &Setting) -> Result<Reading, String> {
    let label = label(name);
    match key.kind {
        Kind::Physics(form) | Kind::CameraLimit(form) | Kind::Plain(form) => {
            let Value::Text(text) = &setting.value else {
                return Err(format!(
                    "{label} is written as text with its unit, in quotes, such as {} = \"…\"",
                    key.name
                ));
            };
            let values = numbers(&label, form, text)?;
            if let Some(range) = &setting.range
                && let Some(sentence) = outside_range(form, range, text, &values)
            {
                return Err(sentence);
            }
            Ok(Reading::Numbers(values))
        }
        Kind::Count { least } => match setting.value {
            Value::Whole(n) if n >= least => Ok(Reading::Whole(n)),
            Value::Whole(n) => Err(format!("{label} must be at least {least}, but it's {n}")),
            _ => Err(format!(
                "{label} is a count: write a whole number without quotes, such as {} = 3",
                key.name
            )),
        },
        Kind::Choice(words) => match &setting.value {
            Value::Text(word) if words.contains(&word.as_str()) => Ok(Reading::Word(word.clone())),
            other => Err(format!(
                "\"{}\" isn't a choice for {label}: write {}",
                other.text(),
                words
                    .iter()
                    .map(|w| format!("\"{w}\""))
                    .collect::<Vec<_>>()
                    .join(" or ")
            )),
        },
        Kind::YesNo => match setting.value {
            Value::YesNo(b) => Ok(Reading::YesNo(b)),
            _ => Err(format!("{label} is written true or false, without quotes")),
        },
    }
}

/// Which way the props spin, seen from above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropDirection {
    /// Betaflight's default: `yaw_motors_reversed = OFF`.
    PropsIn,
    /// `yaw_motors_reversed = ON`.
    PropsOut,
}

impl PropDirection {
    /// The direction a checked `[props] direction` names.
    fn from_word(word: &str) -> PropDirection {
        if word == "props-out" {
            PropDirection::PropsOut
        } else {
            PropDirection::PropsIn
        }
    }

    /// The Tune's `yaw_motors_reversed` this direction needs.
    fn yaw_motors_reversed(self) -> &'static str {
        match self {
            PropDirection::PropsIn => "OFF",
            PropDirection::PropsOut => "ON",
        }
    }
}

/// A checked Quad definition: what the Simulation receives from it, plus its
/// on-screen text, camera and sound. Every number is in SI units: metres,
/// kilograms, seconds, radians, volts, amps, ohms, coulombs; a percentage is
/// a fraction (50% is 0.5).
#[derive(Clone, Debug, PartialEq)]
pub struct QuadDefinition {
    /// Such as `opendrone/whoop-65`, or `test/whoop-65-no-drag`.
    pub id: String,
    /// On-screen text.
    pub name: String,
    pub description: String,
    pub spec_line: String,
    /// The picture's file name, in the Quad's folder.
    pub picture: String,
    /// The real Quad a Test Quad builds on.
    pub based_on: Option<String>,
    /// What the physics receives: the mass (dry mass plus the battery, stored
    /// apart and added here), the inertia, the drag, the rotors' layout, the
    /// props, motors and ESCs, the battery, and the collision shape with its
    /// bounce and friction.
    pub parameters: QuadParameters,
    pub frame: Frame,
    pub collision: Collision,
    pub props: Props,
    pub motors: Motors,
    pub battery: Battery,
    /// `None` on a Quad without ducts.
    pub ducts: Option<Ducts>,
    pub feel: Feel,
    pub board: Board,
    pub camera: Camera,
    pub sound: Sound,
    pub tune: Tune,
    fingerprint: Fingerprint,
}

impl QuadDefinition {
    /// The fingerprint of what the Simulation receives from this Quad: its
    /// physics numbers, counts and choices, the ESC melody and the Tune. Never
    /// its id, on-screen text, picture, camera, sound block, Confidences,
    /// sources or comments (#16 §9).
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

/// `[frame]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub dry_mass: f64,
    /// Motor to motor, across the frame.
    pub diagonal: f64,
    /// Roll, pitch and yaw: about the forward, left and up axes.
    pub inertia: [f64; 3],
    /// How far the props' plane sits above the centre of mass (below when
    /// negative).
    pub rotor_height: f64,
    /// Drag coefficient × area facing forward, sideways and up.
    pub drag_area: [f64; 3],
}

/// `[collision]`: the Quad's shape as simple shapes (#16 §4, #26 §1).
#[derive(Clone, Debug, PartialEq)]
pub struct Collision {
    /// Front to back, side to side, top to bottom.
    pub body: [f64; 3],
    pub pack: [f64; 3],
    /// The pack's centre above the centre of mass (below when negative).
    pub pack_height: f64,
    /// Each duct's ring: inside diameter, wall and height.
    pub duct_rings: Option<[f64; 3]>,
    pub bounce: f64,
    pub friction: f64,
}

/// `[props]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
    pub diameter: f64,
    pub blades: u32,
    pub direction: PropDirection,
    /// C_T in T = C_T·ρ·n²·D⁴, with n in turns a second.
    pub thrust_coefficient: f64,
    /// C_P in P = C_P·ρ·n³·D⁵.
    pub power_coefficient: f64,
    pub rotor_drag: f64,
    pub rotor_inertia: f64,
    /// Spinning backwards, as a share of forward at the same speed.
    pub reverse_thrust: f64,
    pub reverse_torque: f64,
    /// The friction between a spinning prop and what it touches.
    pub grip: f64,
}

/// `[motors]`, with the ESC's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Motors {
    /// In rad/s per volt.
    pub kv: f64,
    pub poles: u32,
    pub winding_resistance: f64,
    pub no_load_current: f64,
    /// The voltage the no-load current was measured at.
    pub no_load_voltage: f64,
    pub spin_up: f64,
    pub slow_down: f64,
    /// How long the ESC waits before starting a stopped motor.
    pub start_wait: f64,
    /// How many times the ESC restarts a stalled motor before giving up.
    pub restart_tries: u32,
    /// The highest drive while a motor starts.
    pub startup_power_limit: f64,
}

/// The battery's chemistry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chemistry {
    LiPo,
    LiHv,
    LiIon,
}

impl Chemistry {
    /// The chemistry a checked `[battery] chemistry` names: `"LiPo"`,
    /// `"LiHV"` or `"Li-ion"`.
    fn from_word(word: &str) -> Chemistry {
        match word {
            "LiHV" => Chemistry::LiHv,
            "Li-ion" => Chemistry::LiIon,
            _ => Chemistry::LiPo,
        }
    }
}

/// `[battery]`. Voltages are per cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    pub cells: u32,
    pub chemistry: Chemistry,
    pub full: f64,
    pub empty: f64,
    pub capacity: f64,
    pub mass: f64,
    /// Points from full to empty: (charge left as a fraction, volts per cell).
    pub voltage_curve: Vec<(f64, f64)>,
    pub resistance: f64,
    /// How long the voltage takes to recover after a punch.
    pub recovery: f64,
    /// How big the slow part of the sag grows, in seconds: each cell's slow
    /// sag settles at this times the power it gives per coulomb of its
    /// capacity (written in mV·Ah/W).
    pub slow_sag: f64,
    pub connector: f64,
}

/// `[ducts]`, on a whoop.
#[derive(Clone, Debug, PartialEq)]
pub struct Ducts {
    pub ram_drag: f64,
    /// How far above an open rotor's the ducted rotor's centre of pressure
    /// sits.
    pub nose_up_offset: f64,
}

/// `[feel]`: Prop Wash and the ground effect's body term.
#[derive(Clone, Debug, PartialEq)]
pub struct Feel {
    pub prop_wash_strength: f64,
    /// In flickers a second.
    pub prop_wash_flicker: f64,
    pub ground_effect_body: f64,
}

/// `[board]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    /// The gyro reads up to ± this, in rad/s.
    pub gyro_range: f64,
}

/// `[camera]`: the FPV Camera's defaults and limits. The camera sits outside
/// the Simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// Forward, left and up from the centre of mass.
    pub position: [f64; 3],
    pub camera_tilt: f64,
    pub fov: f64,
    /// In watts.
    pub vtx_power: f64,
    /// In stops.
    pub analog_dynamic_range: f64,
    pub analog_lines: f64,
    /// In TVL.
    pub analog_sharpness: f64,
}

/// `[sound]` and the sound block, set by ear, with no Confidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub buzzer: bool,
    pub esc_melody: String,
    /// Every sound-block value by its key: frequencies in hertz, levels in
    /// decibels, times in seconds, shares as fractions, plain numbers as they
    /// are.
    pub block: BTreeMap<String, f64>,
}

/// Every setting's reading, by `section.key`. Asking for a key the schema
/// doesn't have is a mistake in this file, so tests catch it.
struct Readings(BTreeMap<String, Reading>);

impl Readings {
    fn get(&self, name: &str) -> Option<&Reading> {
        debug_assert!(schema::find(name).is_some(), "{name} isn't in the schema");
        self.0.get(name)
    }

    fn numbers(&self, name: &str) -> &[f64] {
        match self.get(name) {
            Some(Reading::Numbers(values)) => values,
            _ => &[],
        }
    }

    fn one(&self, name: &str) -> f64 {
        self.numbers(name).first().copied().unwrap_or(0.0)
    }

    fn three(&self, name: &str) -> [f64; 3] {
        match self.numbers(name) {
            [a, b, c, ..] => [*a, *b, *c],
            _ => [0.0; 3],
        }
    }

    /// A curve's points, as (where it holds, value): for the voltage curve,
    /// (charge left, volts).
    fn curve(&self, name: &str) -> Vec<(f64, f64)> {
        self.numbers(name)
            .chunks(2)
            .map(|point| (point[1], point[0]))
            .collect()
    }

    fn whole(&self, name: &str) -> u32 {
        match self.get(name) {
            Some(Reading::Whole(n)) => u32::try_from(*n).unwrap_or(0),
            _ => 0,
        }
    }

    fn word(&self, name: &str) -> &str {
        match self.get(name) {
            Some(Reading::Word(word)) => word,
            _ => "",
        }
    }

    fn yes(&self, name: &str) -> bool {
        matches!(self.get(name), Some(Reading::YesNo(true)))
    }

    fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// Whether each file read in full. When a file had problems with its shape,
/// the settings or Tune lines that failed are missing from what was read, so
/// the checks that ask whether something is there wait until those problems
/// are fixed, rather than adding a second sentence about the same line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReadInFull {
    pub quad: bool,
    pub tune: bool,
}

/// Checks a Quad's values and Tune, and turns them into what the Simulation
/// receives.
pub fn check_quad(
    id: &str,
    based_on: Option<String>,
    quad: &QuadFile,
    tune: &Tune,
) -> Result<QuadDefinition, Problems> {
    let read_in_full = ReadInFull {
        quad: true,
        tune: true,
    };
    check_quad_as_read(id, based_on, quad, tune, read_in_full)
}

/// [`check_quad`] on files that may not have read in full, so every value
/// that did read is checked too, and all of a Quad's problems are listed at
/// once.
pub(crate) fn check_quad_as_read(
    id: &str,
    based_on: Option<String>,
    quad: &QuadFile,
    tune: &Tune,
    read_in_full: ReadInFull,
) -> Result<QuadDefinition, Problems> {
    let mut problems = Problems::new();
    let has_ducts = quad.settings.keys().any(|name| name.starts_with("ducts."));
    let mut readings = BTreeMap::new();
    for section in SECTIONS {
        for key in section.keys {
            let name = format!("{}.{}", section.name, key.name);
            let Some(setting) = quad.settings.get(&name) else {
                if read_in_full.quad && key.need == Need::Always && (!section.optional || has_ducts)
                {
                    problems.push(Problem::of(
                        &quad.file,
                        0,
                        format!("the Quad definition is missing {}", label(&name)),
                    ));
                }
                continue;
            };
            match read_value(&name, key, setting) {
                Ok(reading) => {
                    readings.insert(name, reading);
                }
                Err(sentence) => problems.push(setting.problem(sentence)),
            }
        }
    }
    let readings = Readings(readings);
    cross_check(
        quad,
        tune,
        &readings,
        has_ducts,
        read_in_full,
        &mut problems,
    );
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(definition(id, based_on, quad, tune, &readings, has_ducts))
}

/// The values that must agree with each other, and with the Tune.
fn cross_check(
    quad: &QuadFile,
    tune: &Tune,
    readings: &Readings,
    has_ducts: bool,
    read_in_full: ReadInFull,
    problems: &mut Problems,
) {
    let file_of = |name: &str| {
        quad.settings
            .get(name)
            .map_or(quad.file.as_str(), |s| s.file.as_str())
    };
    let at = |name: &str, sentence: String| match quad.settings.get(name) {
        Some(setting) => setting.problem(sentence),
        None => Problem::of(&quad.file, 0, sentence),
    };
    // The keys that go only with ducts, or only with a buzzer.
    let only_with = |need: Need| {
        SECTIONS.iter().flat_map(move |section| {
            section
                .keys
                .iter()
                .filter(move |key| key.need == need)
                .map(move |key| format!("{}.{}", section.name, key.name))
        })
    };
    if read_in_full.quad {
        for name in only_with(Need::WithDucts) {
            match (has_ducts, quad.settings.contains_key(&name)) {
                (true, false) => problems.push(at(
                    "ducts.ram_drag",
                    format!("a Quad with [ducts] needs {}", label(&name)),
                )),
                (false, true) => problems.push(at(
                    &name,
                    format!(
                        "{} describes the ducts, so it goes with a [ducts] section",
                        label(&name)
                    ),
                )),
                _ => {}
            }
        }
    }
    if read_in_full.quad && readings.has("sound.buzzer") {
        let buzzer = readings.yes("sound.buzzer");
        for name in only_with(Need::WithBuzzer) {
            match (buzzer, quad.settings.contains_key(&name)) {
                (true, false) => problems.push(at(
                    "sound.buzzer",
                    format!(
                        "a Quad with a buzzer needs {} in its sound block",
                        label(&name)
                    ),
                )),
                (false, true) => problems.push(at(
                    &name,
                    format!(
                        "{} is for the buzzer, but this Quad has none ([sound] buzzer = false)",
                        label(&name)
                    ),
                )),
                _ => {}
            }
        }
    }
    let poles = readings.whole("motors.poles");
    if readings.has("motors.poles") && poles % 2 == 1 {
        problems.push(at(
            "motors.poles",
            format!("[motors] poles is {poles}, but a motor has an even number of poles"),
        ));
    }
    let full = readings.one("battery.full");
    let empty = readings.one("battery.empty");
    let both_ends = readings.has("battery.full") && readings.has("battery.empty");
    if both_ends && empty >= full {
        problems.push(at(
            "battery.empty",
            "[battery] empty must be below [battery] full".into(),
        ));
    }
    if both_ends && readings.has("battery.voltage_curve") {
        let points = readings.curve("battery.voltage_curve");
        let falling = points
            .windows(2)
            .all(|pair| pair[1].0 < pair[0].0 && pair[1].1 <= pair[0].1);
        let ends = points.first() == Some(&(1.0, full)) && points.last() == Some(&(0.0, empty));
        if !falling || !ends {
            problems.push(at(
                "battery.voltage_curve",
                "[battery] voltage_curve runs from [battery] full at 100% down to [battery] empty at 0%, with the voltage never rising on the way".into(),
            ));
        }
    }
    if readings.has("motors.poles") {
        match tune.settings.get("motor_poles") {
            None if read_in_full.tune => problems.push(tune.problem(
                "motor_poles",
                format!(
                    "the Tune must set motor_poles, which must match the Quad's {poles} motor poles"
                ),
            )),
            Some(setting) if setting.value.parse::<u32>() != Ok(poles) => {
                problems.push(tune.problem(
                    "motor_poles",
                    format!(
                        "motor_poles is {}, but the Quad's motors have {poles} poles ([motors] poles in {}); they must match",
                        setting.value,
                        file_of("motors.poles")
                    ),
                ));
            }
            _ => {}
        }
    }
    if readings.has("props.direction") {
        let direction = readings.word("props.direction");
        let expected = PropDirection::from_word(direction).yaw_motors_reversed();
        match tune.settings.get("yaw_motors_reversed") {
            None if read_in_full.tune => problems.push(tune.problem(
                "yaw_motors_reversed",
                format!(
                    "the Tune must set yaw_motors_reversed, which must be {expected} for {direction}"
                ),
            )),
            Some(setting) if setting.value != expected => problems.push(tune.problem(
                "yaw_motors_reversed",
                format!(
                    "yaw_motors_reversed is {}, but the props spin {direction} ([props] direction in {}), which needs {expected}",
                    setting.value,
                    file_of("props.direction")
                ),
            )),
            _ => {}
        }
    }
}

fn definition(
    id: &str,
    based_on: Option<String>,
    quad: &QuadFile,
    tune: &Tune,
    r: &Readings,
    has_ducts: bool,
) -> QuadDefinition {
    let frame = Frame {
        dry_mass: r.one("frame.dry_mass"),
        diagonal: r.one("frame.diagonal"),
        inertia: r.three("frame.inertia"),
        rotor_height: r.one("frame.rotor_height"),
        drag_area: r.three("frame.drag_area"),
    };
    let battery = Battery {
        cells: r.whole("battery.cells"),
        chemistry: Chemistry::from_word(r.word("battery.chemistry")),
        full: r.one("battery.full"),
        empty: r.one("battery.empty"),
        capacity: r.one("battery.capacity"),
        mass: r.one("battery.mass"),
        voltage_curve: r.curve("battery.voltage_curve"),
        resistance: r.one("battery.resistance"),
        recovery: r.one("battery.recovery"),
        slow_sag: r.one("battery.slow_sag"),
        connector: r.one("battery.connector"),
    };
    let ducts = has_ducts.then(|| Ducts {
        ram_drag: r.one("ducts.ram_drag"),
        nose_up_offset: r.one("ducts.nose_up_offset"),
    });
    let props = Props {
        diameter: r.one("props.diameter"),
        blades: r.whole("props.blades"),
        direction: PropDirection::from_word(r.word("props.direction")),
        thrust_coefficient: r.one("props.thrust_coefficient"),
        power_coefficient: r.one("props.power_coefficient"),
        rotor_drag: r.one("props.rotor_drag"),
        rotor_inertia: r.one("props.rotor_inertia"),
        reverse_thrust: r.one("props.reverse_thrust"),
        reverse_torque: r.one("props.reverse_torque"),
        grip: r.one("props.grip"),
    };
    let no_load = r.numbers("motors.no_load_current");
    let motors = Motors {
        kv: r.one("motors.kv"),
        poles: r.whole("motors.poles"),
        winding_resistance: r.one("motors.winding_resistance"),
        no_load_current: no_load.first().copied().unwrap_or(0.0),
        no_load_voltage: no_load.get(1).copied().unwrap_or(0.0),
        spin_up: r.one("motors.spin_up"),
        slow_down: r.one("motors.slow_down"),
        start_wait: r.one("motors.start_wait"),
        restart_tries: r.whole("motors.restart_tries"),
        startup_power_limit: r.one("motors.startup_power_limit"),
    };
    let [roll, pitch, yaw] = frame.inertia;
    let [front, side, top] = frame.drag_area;
    let collision = Collision {
        body: r.three("collision.body"),
        pack: r.three("collision.pack"),
        pack_height: r.one("collision.pack_height"),
        duct_rings: r
            .has("collision.duct_rings")
            .then(|| r.three("collision.duct_rings")),
        bounce: r.one("collision.bounce"),
        friction: r.one("collision.friction"),
    };
    let size = |[a, b, c]: [f64; 3]| Vec3::new(a, b, c);
    let parameters = QuadParameters {
        // Stored apart, added here, so a heavier pack can't be counted twice
        // (#16 §4).
        mass: frame.dry_mass + battery.mass,
        // Roll turns about the forward axis, pitch about the left axis and
        // yaw about the up axis.
        inertia: Mat3::diagonal(Vec3::new(roll, pitch, yaw)),
        drag: Drag {
            body_area: Vec3::new(front, side, top),
            rotor: props.rotor_drag,
            // A Quad without ducts has no duct drag.
            duct_ram: ducts.as_ref().map_or(0.0, |d| d.ram_drag),
            duct_offset: ducts.as_ref().map_or(0.0, |d| d.nose_up_offset),
        },
        rotors: RotorLayout {
            diagonal: frame.diagonal,
            rotor_height: frame.rotor_height,
            direction: match props.direction {
                PropDirection::PropsIn => opendrone_physics::PropDirection::PropsIn,
                PropDirection::PropsOut => opendrone_physics::PropDirection::PropsOut,
            },
        },
        props: PropParameters {
            diameter: props.diameter,
            thrust_coefficient: props.thrust_coefficient,
            power_coefficient: props.power_coefficient,
            rotor_inertia: props.rotor_inertia,
            reverse_thrust: props.reverse_thrust,
            reverse_torque: props.reverse_torque,
        },
        motors: MotorParameters {
            kv: motors.kv,
            poles: motors.poles,
            winding_resistance: motors.winding_resistance,
            no_load_current: motors.no_load_current,
            no_load_voltage: motors.no_load_voltage,
            spin_up: motors.spin_up,
            slow_down: motors.slow_down,
        },
        esc: EscParameters {
            start_wait: motors.start_wait,
            startup_power_limit: motors.startup_power_limit,
            restart_tries: motors.restart_tries,
        },
        battery: BatteryParameters {
            cells: battery.cells,
            capacity: battery.capacity,
            voltage_curve: battery.voltage_curve.clone(),
            resistance: battery.resistance,
            connector: battery.connector,
            recovery: battery.recovery,
            slow_sag: battery.slow_sag,
        },
        shape: QuadShape {
            body: size(collision.body),
            pack: size(collision.pack),
            pack_height: collision.pack_height,
            diagonal: frame.diagonal,
            rotor_height: frame.rotor_height,
            prop_diameter: props.diameter,
            duct_rings: collision
                .duct_rings
                .map(|[inside_diameter, wall, height]| DuctRings {
                    inside_diameter,
                    wall,
                    height,
                }),
            bounce: collision.bounce,
            friction: collision.friction,
        },
    };
    let block = SECTIONS
        .iter()
        .find(|s| s.name == "sound_block")
        .map(|section| {
            section
                .keys
                .iter()
                .filter_map(|key| {
                    let name = format!("sound_block.{}", key.name);
                    match r.get(&name)? {
                        Reading::Numbers(values) => Some((key.name.to_string(), values[0])),
                        Reading::Whole(n) => Some((key.name.to_string(), *n as f64)),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    QuadDefinition {
        id: id.to_string(),
        name: quad.name.clone(),
        description: quad.description.clone(),
        spec_line: quad.spec_line.clone(),
        picture: quad.picture.clone(),
        based_on,
        parameters,
        frame,
        collision,
        props,
        motors,
        battery,
        ducts,
        feel: Feel {
            prop_wash_strength: r.one("feel.prop_wash_strength"),
            prop_wash_flicker: r.one("feel.prop_wash_flicker"),
            ground_effect_body: r.one("feel.ground_effect_body"),
        },
        board: Board {
            gyro_range: r.one("board.gyro_range"),
        },
        camera: Camera {
            position: r.three("camera.position"),
            camera_tilt: r.one("camera.camera_tilt"),
            fov: r.one("camera.fov"),
            vtx_power: r.one("camera.vtx_power"),
            analog_dynamic_range: r.one("camera.analog_dynamic_range"),
            analog_lines: r.one("camera.analog_lines"),
            analog_sharpness: r.one("camera.analog_sharpness"),
        },
        sound: Sound {
            buzzer: r.yes("sound.buzzer"),
            esc_melody: r.word("sound.esc_melody").to_string(),
            block,
        },
        tune: tune.clone(),
        fingerprint: fingerprint(r, tune),
    }
}

/// Every value the Simulation receives, in a fixed order: the schema's, then
/// the Tune's settings by name.
fn fingerprint(readings: &Readings, tune: &Tune) -> Fingerprint {
    let mut f = Fingerprinter::new();
    for section in SECTIONS {
        for key in section.keys {
            let name = format!("{}.{}", section.name, key.name);
            if !schema::reaches_the_simulation(&name) {
                continue;
            }
            match readings.get(&name) {
                None => f.write_u64(0),
                Some(Reading::Numbers(values)) => {
                    f.write_u64(1);
                    f.write_u64(values.len() as u64);
                    f.write_f64s(values);
                }
                Some(Reading::Whole(n)) => {
                    f.write_u64(2);
                    f.write_u64(*n as u64);
                }
                Some(Reading::Word(word)) => {
                    f.write_u64(3);
                    write_text(&mut f, word);
                }
                Some(Reading::YesNo(b)) => {
                    f.write_u64(4);
                    f.write_u64(u64::from(*b));
                }
            }
        }
    }
    f.write_u64(tune.settings.len() as u64);
    for (name, setting) in &tune.settings {
        write_text(&mut f, name);
        write_text(&mut f, &setting.value);
    }
    f.finish()
}

fn write_text(f: &mut Fingerprinter, text: &str) {
    f.write_u64(text.len() as u64);
    for chunk in text.as_bytes().chunks(8) {
        let mut bytes = [0u8; 8];
        bytes[..chunk.len()].copy_from_slice(chunk);
        f.write_u64(u64::from_le_bytes(bytes));
    }
}
