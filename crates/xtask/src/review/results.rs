//! What moved: every Expectation whose measured value differs between the
//! base's and the head's Results files, biggest move first (#11 §4).
//!
//! The Results hold each measured value to 3 significant figures, so a move
//! inside that shows only in the fingerprints, which are listed too.

use std::cmp::Ordering;

use opendrone_pack::units::parse_quantity;

use super::changes::Changes;
use super::markdown::plain;
use super::scenarios::{FOLDER, RESULTS_ENDING};

/// One Expectation whose measured value moved.
#[derive(Clone, Debug)]
pub struct Move {
    /// The Scenario's name, from its Results file.
    pub scenario: String,
    /// What was measured, and when, as the Results file words it.
    pub what: String,
    pub before: String,
    pub after: String,
    pub size: Size,
}

/// How big a move is.
#[derive(Clone, Debug, PartialEq)]
pub enum Size {
    /// Not numbers, such as "none broken" becoming a broken number, or a
    /// value in a different kind of unit.
    Changed,
    /// From zero to something: no share of zero can say how far.
    FromZero,
    /// The change as a share of the value before: 0.05 is 5% more.
    Share(f64),
}

impl Size {
    /// How the Report writes it.
    pub fn text(&self) -> String {
        match self {
            Size::Changed => "changed".to_string(),
            Size::FromZero => "from zero".to_string(),
            Size::Share(share) => {
                let percent = share * 100.0;
                let sign = if percent > 0.0 { "+" } else { "−" };
                let magnitude = percent.abs();
                let digits = if magnitude >= 10.0 {
                    0
                } else if magnitude >= 1.0 {
                    1
                } else if magnitude >= 0.01 {
                    2
                } else {
                    4
                };
                format!("{sign}{magnitude:.digits$}%")
            }
        }
    }

    /// Bigger moves sort first: changes that aren't numbers, then moves from
    /// zero, then by share.
    fn rank(&self) -> (u8, f64) {
        match self {
            Size::Changed => (2, 0.0),
            Size::FromZero => (1, 0.0),
            Size::Share(share) => (0, share.abs()),
        }
    }
}

/// Everything the Results files say moved.
#[derive(Clone, Debug, Default)]
pub struct Moved {
    /// Measured values that moved, biggest first.
    pub moves: Vec<Move>,
    /// Expectations only the head measures: "Scenario: what", as Markdown.
    pub new: Vec<String>,
    /// Expectations only the base measured: "Scenario: what", as Markdown.
    pub gone: Vec<String>,
    /// For each Scenario whose fingerprints moved, which ones, in plain words,
    /// as Markdown.
    pub fingerprints: Vec<String>,
}

impl Moved {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
            && self.new.is_empty()
            && self.gone.is_empty()
            && self.fingerprints.is_empty()
    }
}

/// A Results file as far as What moved needs it.
#[derive(Default)]
struct ResultsFile {
    scenario: String,
    /// What, and the measured value, in file order.
    measured: Vec<(String, String)>,
    quad: Option<String>,
    map: Option<String>,
    run: Option<String>,
    /// Each checkpoint's time and fingerprint, in file order.
    checkpoints: Vec<(String, String)>,
}

impl ResultsFile {
    fn read(text: &str, path: &str) -> ResultsFile {
        let table: toml::Table = text.parse().unwrap_or_default();
        let text_of =
            |value: Option<&toml::Value>| value.and_then(toml::Value::as_str).map(str::to_string);
        let measured = table
            .get("expect")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                Some((
                    entry.get("what")?.as_str()?.to_string(),
                    entry.get("measured")?.as_str()?.to_string(),
                ))
            })
            .collect();
        let fingerprints = table.get("fingerprints");
        let checkpoints = fingerprints
            .and_then(|f| f.get("checkpoints"))
            .and_then(toml::Value::as_table)
            .map(|c| {
                c.iter()
                    .filter_map(|(time, value)| Some((time.clone(), value.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        ResultsFile {
            scenario: text_of(table.get("scenario")).unwrap_or_else(|| {
                path.trim_start_matches(FOLDER)
                    .trim_end_matches(RESULTS_ENDING)
                    .to_string()
            }),
            measured,
            quad: text_of(fingerprints.and_then(|f| f.get("quad"))),
            map: text_of(fingerprints.and_then(|f| f.get("map"))),
            run: text_of(fingerprints.and_then(|f| f.get("run"))),
            checkpoints,
        }
    }
}

/// Compares every changed Results file.
pub fn moved(changes: &Changes) -> Moved {
    let mut moved = Moved::default();
    for path in changes
        .under(FOLDER)
        .filter(|p| p.ends_with(RESULTS_ENDING))
    {
        let base = changes.base.text(path).map(|t| ResultsFile::read(&t, path));
        let head = changes.head.text(path).map(|t| ResultsFile::read(&t, path));
        let (base, head) = match (base, head) {
            (Some(base), Some(head)) => (base, head),
            (None, Some(head)) => {
                moved.new.push(format!(
                    "{}: a new Scenario, with {}",
                    plain(&head.scenario),
                    expectations(head.measured.len())
                ));
                continue;
            }
            (Some(base), None) => {
                moved.gone.push(format!(
                    "{}: the whole Scenario, with {}",
                    plain(&base.scenario),
                    expectations(base.measured.len())
                ));
                continue;
            }
            (None, None) => continue,
        };
        let mut any_value_moved = false;
        for (what, after) in &head.measured {
            match base.measured.iter().find(|(w, _)| w == what) {
                Some((_, before)) if before != after => {
                    any_value_moved = true;
                    moved.moves.push(Move {
                        scenario: head.scenario.clone(),
                        what: what.clone(),
                        before: before.clone(),
                        after: after.clone(),
                        size: size(before, after),
                    });
                }
                Some(_) => {}
                None => moved
                    .new
                    .push(format!("{}: {}", plain(&head.scenario), plain(what))),
            }
        }
        for (what, _) in &base.measured {
            if !head.measured.iter().any(|(w, _)| w == what) {
                moved
                    .gone
                    .push(format!("{}: {}", plain(&base.scenario), plain(what)));
            }
        }
        if let Some(line) = fingerprint_line(&base, &head, any_value_moved) {
            moved.fingerprints.push(line);
        }
    }
    moved.moves.sort_by(|a, b| {
        let (a_kind, a_share) = a.size.rank();
        let (b_kind, b_share) = b.size.rank();
        b_kind
            .cmp(&a_kind)
            .then(b_share.partial_cmp(&a_share).unwrap_or(Ordering::Equal))
            .then_with(|| a.scenario.cmp(&b.scenario))
            .then_with(|| a.what.cmp(&b.what))
    });
    moved
}

fn expectations(count: usize) -> String {
    if count == 1 {
        "1 Expectation".to_string()
    } else {
        format!("{count} Expectations")
    }
}

/// How far a measured value moved, as a share of where it was.
fn size(before: &str, after: &str) -> Size {
    let (Ok(before), Ok(after)) = (parse_quantity(before), parse_quantity(after)) else {
        return Size::Changed;
    };
    if before.dimension() != after.dimension() {
        return Size::Changed;
    }
    if before.value == 0.0 {
        return if after.value == 0.0 {
            Size::Share(0.0)
        } else {
            Size::FromZero
        };
    }
    Size::Share((after.value - before.value) / before.value.abs())
}

/// Which fingerprints moved in one Scenario, in plain words.
fn fingerprint_line(
    base: &ResultsFile,
    head: &ResultsFile,
    any_value_moved: bool,
) -> Option<String> {
    let mut parts = Vec::new();
    if base.quad != head.quad {
        parts.push("the Quad's fingerprint moved, so its Quad definition changed".to_string());
    }
    if base.map != head.map {
        parts.push("the Map's fingerprint moved, so its Map changed".to_string());
    }
    if base.run != head.run {
        let first = head
            .checkpoints
            .iter()
            .find(|(time, fingerprint)| {
                base.checkpoints
                    .iter()
                    .find(|(t, _)| t == time)
                    .is_none_or(|(_, f)| f != fingerprint)
            })
            .map(|(time, _)| time.clone());
        let when = match first {
            Some(time) => format!(", first seen at the {} checkpoint", plain(&time)),
            None => String::new(),
        };
        let inside = if any_value_moved {
            ""
        } else {
            " while every measured value stayed the same to 3 significant figures"
        };
        parts.push(format!("the flight's fingerprint moved{when}{inside}"));
    }
    (!parts.is_empty()).then(|| format!("{}: {}", plain(&head.scenario), parts.join("; ")))
}
