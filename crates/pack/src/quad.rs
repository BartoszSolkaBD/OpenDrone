//! Quad definitions (`quads/<id>/quad.toml`) and Test Quads
//! (`scenarios/test-quads/<id>.toml`), #16 §3, §4 and §6.
//!
//! So far this reads only the numbers the Simulation uses: the mass (dry mass
//! plus the battery, stored apart and added here), the inertia and the drag.
//! Each physics number is written `{ value, confidence, source }`, and an
//! Estimate adds the `range` it may move within. The rest of the Quad
//! definition, and the full Pack checker, arrive with #40.

use std::collections::BTreeMap;

use opendrone_maths::{Fingerprint, Fingerprinter, Mat3, Vec3};
use opendrone_physics::{Drag, QuadParameters};

use crate::document::{Document, Item, Problem, Problems, Table};
use crate::units::{self, Dimension, Range};

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
}

/// One setting of a Quad definition, as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    /// The value as written, such as `"23.0 g"`.
    pub value: String,
    /// `None` for a Test Quad's override, which is a deliberate test value
    /// with no Confidence, and for settings that never carry one.
    pub confidence: Option<Confidence>,
    pub range: Option<Range>,
    /// The key of its line in `[sources]`.
    pub source: Option<String>,
    /// The file and line it was written on.
    pub file: String,
    pub line: usize,
}

impl Setting {
    fn problem(&self, sentence: impl Into<String>) -> Problem {
        Problem {
            file: self.file.clone(),
            line: self.line,
            sentence: sentence.into(),
        }
    }
}

/// A Quad definition's file, read but not yet turned into numbers: every
/// setting by `section.key`.
#[derive(Clone, Debug, PartialEq)]
pub struct QuadFile {
    /// The file it was read from.
    pub file: String,
    pub name: String,
    pub settings: BTreeMap<String, Setting>,
}

/// A checked Quad definition: what the Simulation receives from it.
#[derive(Clone, Debug, PartialEq)]
pub struct QuadDefinition {
    /// Such as `opendrone/whoop-65`, or `test/whoop-65-no-drag`.
    pub id: String,
    /// The on-screen name.
    pub name: String,
    /// The real Quad a Test Quad builds on.
    pub based_on: Option<String>,
    pub parameters: QuadParameters,
}

impl QuadDefinition {
    /// The fingerprint of what the Simulation receives from this Quad: its
    /// numbers only, never its id, name or comments (#16 §9).
    pub fn fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        self.parameters.write_fingerprint(&mut f);
        f.finish()
    }
}

/// The sections of a Quad definition this reads, so far.
const SECTIONS: &[&str] = &["frame", "props", "battery", "ducts"];

/// Reads a Quad definition's file.
pub fn read_quad_file(file: &str, text: &str) -> Result<QuadFile, Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    let root = doc.root();
    let name = root
        .text("name", &mut problems)
        .map(|(name, _)| name.to_string());

    let mut sources = BTreeMap::new();
    if let Some(item) = root.get("sources")
        && let Some(table) = item.table(&mut problems)
    {
        for (key, item) in table.entries() {
            if item.text(&mut problems).is_some() {
                sources.insert(key, ());
            }
        }
    }

    let mut settings = BTreeMap::new();
    for section in SECTIONS {
        let Some(item) = root.get(section) else {
            continue;
        };
        let Some(table) = item.table(&mut problems) else {
            continue;
        };
        for (key, item) in table.entries() {
            if let Some(setting) = read_setting(&doc, &item, &sources, &mut problems) {
                settings.insert(format!("{section}.{key}"), setting);
            }
        }
    }
    match name {
        Some(name) => problems.or(QuadFile {
            file: file.to_string(),
            name,
            settings,
        }),
        None => Err(problems),
    }
}

/// Reads one setting: a physics number `{ value, confidence, source }` (with a
/// `range` for an Estimate), or plain text such as a choice.
fn read_setting(
    doc: &Document<'_>,
    item: &Item<'_, '_>,
    sources: &BTreeMap<String, ()>,
    problems: &mut Problems,
) -> Option<Setting> {
    let file = doc.file().to_string();
    let line = item.line();
    if !item.is_table() {
        // A count or a choice, such as `blades = 3`: no Confidence.
        return Some(Setting {
            value: item.as_str().unwrap_or_default().to_string(),
            confidence: None,
            range: None,
            source: None,
            file,
            line,
        });
    }
    let table: Table<'_, '_> = item.table(problems)?;
    table.refuse_unknown(&["value", "confidence", "range", "source"], problems);
    let value = table.text("value", problems).map(|(v, _)| v.to_string());
    let confidence = table.text("confidence", problems).and_then(|(text, item)| {
        let confidence = Confidence::read(text);
        if confidence.is_none() {
            problems.push(item.problem(format!(
                "\"{text}\" isn't a Confidence: write Measured, Manufacturer, Derived or Estimate"
            )));
        }
        confidence
    });
    let source = table.text("source", problems).and_then(|(text, item)| {
        if sources.contains_key(text) {
            Some(text.to_string())
        } else {
            problems.push(item.problem(format!(
                "the source \"{text}\" isn't in this file's [sources] list"
            )));
            None
        }
    });
    let range = match (table.get("range"), confidence) {
        (Some(item), _) => item
            .text(problems)
            .and_then(|text| match units::parse_range(text) {
                Ok(range) => Some(range),
                Err(p) => {
                    problems.push(item.problem(p.0));
                    None
                }
            }),
        (None, Some(Confidence::Estimate)) => {
            problems.push(table.problem(format!(
                "`{}` is an Estimate, so it needs the `range` it may move within, such as range = \"×0.5–×2\"",
                item.key()
            )));
            None
        }
        (None, _) => None,
    };
    Some(Setting {
        value: value?,
        confidence: Some(confidence?),
        range,
        source,
        file,
        line,
    })
}

/// Reads a Test Quad over the file of the real Quad it builds on, and returns
/// the real Quad's settings with the Test Quad's changes. A Test Quad lists
/// only what it changes, so a change to the real Quad carries into it.
pub fn read_test_quad(
    file: &str,
    text: &str,
    base: impl FnOnce(&str) -> Result<QuadFile, Problems>,
) -> Result<(String, QuadFile), Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
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
                "can't read \"{based_on}\", the Quad this builds on"
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
                    setting.value
                )));
                continue;
            }
            let Some(value) = item.text(&mut problems) else {
                continue;
            };
            *setting = Setting {
                value: value.to_string(),
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

/// Turns a Quad's settings into the numbers the Simulation receives.
pub fn quad_definition(
    id: &str,
    based_on: Option<String>,
    quad: &QuadFile,
) -> Result<QuadDefinition, Problems> {
    let mut problems = Problems::new();
    let mut reader = Reader {
        quad,
        problems: &mut problems,
    };
    let dry_mass = reader.one("frame.dry_mass", Dimension::MASS);
    let battery_mass = reader.one("battery.mass", Dimension::MASS);
    let inertia = reader.parts(
        "frame.inertia",
        &["roll", "pitch", "yaw"],
        Dimension::INERTIA,
    );
    let drag_area = reader.parts(
        "frame.drag_area",
        &["front", "side", "top"],
        Dimension::AREA,
    );
    let rotor_drag = reader.one("props.rotor_drag", Dimension::PER_SECOND);
    // A Quad without ducts has no duct drag.
    let duct_drag = if quad.settings.keys().any(|key| key.starts_with("ducts.")) {
        reader.one("ducts.ram_drag", Dimension::PER_SECOND)
    } else {
        Some(0.0)
    };

    for (name, value) in [("frame.dry_mass", dry_mass), ("battery.mass", battery_mass)] {
        if let (Some(value), Some(setting)) = (value, quad.settings.get(name))
            && value <= 0.0
        {
            problems.push(setting.problem(format!("{name} must be above zero")));
        }
    }
    if let (Some([roll, pitch, yaw]), Some(setting)) = (inertia, quad.settings.get("frame.inertia"))
        && [roll, pitch, yaw].iter().any(|n| *n <= 0.0)
    {
        problems.push(setting.problem("every part of the inertia must be above zero"));
    }
    for (name, values) in [
        ("frame.drag_area", drag_area.map(Vec::from)),
        ("props.rotor_drag", rotor_drag.map(|n| vec![n])),
        ("ducts.ram_drag", duct_drag.map(|n| vec![n])),
    ] {
        if let (Some(values), Some(setting)) = (values, quad.settings.get(name))
            && values.iter().any(|n| *n < 0.0)
        {
            problems.push(setting.problem(format!("{name} can't be below zero")));
        }
    }

    let (
        Some(dry_mass),
        Some(battery_mass),
        Some([roll, pitch, yaw]),
        Some([front, side, top]),
        Some(rotor),
        Some(duct_ram),
    ) = (
        dry_mass,
        battery_mass,
        inertia,
        drag_area,
        rotor_drag,
        duct_drag,
    )
    else {
        return Err(problems);
    };
    problems.or(QuadDefinition {
        id: id.to_string(),
        name: quad.name.clone(),
        based_on,
        parameters: QuadParameters {
            // Stored apart, added here, so a heavier pack can't be counted
            // twice (#16 §4).
            mass: dry_mass + battery_mass,
            // Roll turns about the forward axis, pitch about the left axis and
            // yaw about the up axis.
            inertia: Mat3::diagonal(Vec3::new(roll, pitch, yaw)),
            drag: Drag {
                body_area: Vec3::new(front, side, top),
                rotor,
                duct_ram,
            },
        },
    })
}

struct Reader<'q, 'p> {
    quad: &'q QuadFile,
    problems: &'p mut Problems,
}

impl Reader<'_, '_> {
    fn setting(&mut self, name: &str) -> Option<&Setting> {
        let setting = self.quad.settings.get(name);
        if setting.is_none() {
            let (section, key) = name.split_once('.').unwrap_or((name, ""));
            self.problems.push(Problem {
                file: self.quad.file.clone(),
                line: 0,
                sentence: format!("the Quad definition is missing [{section}] {key}"),
            });
        }
        setting
    }

    fn one(&mut self, name: &str, dimension: Dimension) -> Option<f64> {
        let setting = self.setting(name)?.clone();
        let value = units::parse_quantity(&setting.value)
            .and_then(|q| q.as_a(dimension))
            .map_err(|p| self.problems.push(setting.problem(p.0)))
            .ok()?;
        self.check_range(&setting, &[value], dimension);
        Some(value)
    }

    fn parts<const N: usize>(
        &mut self,
        name: &str,
        labels: &[&str; N],
        dimension: Dimension,
    ) -> Option<[f64; N]> {
        let setting = self.setting(name)?.clone();
        let parts = match units::parse_parts(&setting.value, labels) {
            Ok(parts) => parts,
            Err(p) => {
                self.problems.push(setting.problem(p.0));
                return None;
            }
        };
        let mut values = [0.0; N];
        for (value, label) in values.iter_mut().zip(labels) {
            let Some(part) = parts.iter().find(|part| part.label == *label) else {
                self.problems.push(setting.problem(format!(
                    "\"{}\" needs {}, each with its label",
                    setting.value,
                    labels.join(", ")
                )));
                return None;
            };
            *value = match part.quantity.as_a(dimension) {
                Ok(value) => value,
                Err(p) => {
                    self.problems
                        .push(setting.problem(format!("{label}: {}", p.0)));
                    return None;
                }
            };
        }
        self.check_range(&setting, &values, dimension);
        Some(values)
    }

    fn check_range(&mut self, setting: &Setting, values: &[f64], dimension: Dimension) {
        let Some(range) = &setting.range else {
            return;
        };
        if let Range::Absolute { low, .. } = range
            && low.dimension() != dimension
        {
            self.problems.push(setting.problem(format!(
                "its range is {}, but the value needs {}",
                low.text(),
                dimension.described()
            )));
            return;
        }
        if values.iter().any(|value| !range.holds(*value)) {
            self.problems.push(setting.problem(format!(
                "\"{}\" is outside its range, {}",
                setting.value,
                range.text()
            )));
        }
    }
}
