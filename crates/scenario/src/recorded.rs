//! Recorded flights: what a real Quad did, from a CSV file beside the
//! Scenario, for its "matches a recorded flight" Expectations (#11 §3).
//!
//! The file starts with comment lines (`#`) saying where it came from, then
//! its header, then a row for each recorded moment, in time order:
//!
//! ```text
//! time (s),roll rate (°/s),felt acceleration up (m/s²)
//! 0,12.3,9.81
//! 0.002,12.9,
//! ```
//!
//! - **`time`** is Simulation Time from the Scenario's start, in seconds, a
//!   whole number of physics steps.
//! - **Every other column** is something the Scenario can measure, named as
//!   its `what` names it, with its unit in brackets, such as
//!   `roll rate (°/s)`. An empty cell means nothing was recorded for it at
//!   that moment.
//!
//! An Expectation over a stretch with `mean_gap` or `rms_gap` compares the
//! run with the column its `what` names, at each recorded moment in the
//! stretch.

use opendrone_pack::units::{self, Unit};
use opendrone_pack::{Problem, Problems};
use opendrone_sim::PhysicsRate;

use crate::measure::Measure;
use crate::track::moment;

/// A recorded flight a Scenario's Expectations compare with.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedFlight {
    /// The file, as the Scenario names it.
    pub file: String,
    /// The Scenario's line that names it.
    pub line: usize,
    /// Each column after the time: empty until the file is read
    /// ([`crate::Scenario::read_recorded_flight`]).
    pub columns: Vec<RecordedColumn>,
    /// Whether the file has been read.
    pub read: bool,
}

/// One column of a recorded flight.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedColumn {
    pub measure: Measure,
    /// Each recorded value, in SI units, at its step, in time order.
    pub values: Vec<(u64, f64)>,
}

impl RecordedFlight {
    /// The column for `measure`, if the file has one.
    pub fn column(&self, measure: Measure) -> Option<&RecordedColumn> {
        self.columns.iter().find(|c| c.measure == measure)
    }
}

/// Reads a recorded flight's columns, naming the file `file` in problems.
/// `measurable` says why the Scenario can't measure a column's measure, if it
/// can't. Every problem is listed at once.
pub(crate) fn read_recorded_flight(
    file: &str,
    text: &str,
    rate: PhysicsRate,
    measurable: &dyn Fn(Measure) -> Option<String>,
) -> Result<Vec<RecordedColumn>, Problems> {
    let mut problems = Problems::new();
    let mut rows = text
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'));
    let Some((header_line, header)) = rows.next() else {
        return Err(Problems::of_file(
            file,
            "a recorded flight needs its header, \"time (s)\" and the columns it records, such as \"roll rate (°/s)\", and its rows",
        ));
    };
    let mut cells = header.split(',').map(str::trim);
    if cells.next() != Some("time (s)") {
        return Err(Problems(vec![Problem::of(
            file,
            header_line,
            "a recorded flight's header starts with \"time (s)\"",
        )]));
    }
    // Each column's measure and unit, where it can be read.
    let mut heads: Vec<Option<(Measure, Unit)>> = Vec::new();
    for cell in cells {
        heads.push(column_head(cell, measurable).map_or_else(
            |sentence| {
                problems.push(Problem::of(file, header_line, sentence));
                None
            },
            Some,
        ));
    }
    for (n, head) in heads.iter().enumerate() {
        if let Some((measure, _)) = head
            && heads[..n]
                .iter()
                .flatten()
                .any(|(earlier, _)| earlier == measure)
        {
            problems.push(Problem::of(
                file,
                header_line,
                format!("\"{}\" has two columns", measure.name()),
            ));
        }
    }
    if heads.is_empty() {
        problems.push(Problem::of(
            file,
            header_line,
            "a recorded flight records something after its time, such as \"roll rate (°/s)\"",
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let heads: Vec<(Measure, Unit)> = heads.into_iter().flatten().collect();
    let hz = f64::from(rate.hz());
    let mut columns: Vec<RecordedColumn> = heads
        .iter()
        .map(|(measure, _)| RecordedColumn {
            measure: *measure,
            values: Vec::new(),
        })
        .collect();
    let mut last: Option<u64> = None;
    for (line, row) in rows {
        let cells: Vec<&str> = row.split(',').map(str::trim).collect();
        if cells.len() != heads.len() + 1 {
            problems.push(Problem::of(
                file,
                line,
                format!(
                    "a row has {} cells, as the header does, not {}",
                    heads.len() + 1,
                    cells.len()
                ),
            ));
            continue;
        }
        let Some(tick) = moment(cells[0], hz) else {
            problems.push(Problem::of(
                file,
                line,
                format!(
                    "\"{}\" isn't a time OpenDrone can use here: write seconds from the Scenario's start, such as 0.002, each a whole number of physics steps (at {} Hz one step is {} s)",
                    cells[0],
                    rate.hz(),
                    1.0 / hz
                ),
            ));
            continue;
        };
        if last.is_some_and(|last| tick <= last) {
            problems.push(Problem::of(
                file,
                line,
                "rows go in time order, one row a moment",
            ));
            continue;
        }
        last = Some(tick);
        for ((column, (measure, unit)), cell) in columns.iter_mut().zip(&heads).zip(&cells[1..]) {
            if cell.is_empty() {
                continue;
            }
            match plain_number(cell) {
                Some(number) => column.values.push((tick, number * unit.in_si())),
                None => problems.push(Problem::of(
                    file,
                    line,
                    format!(
                        "\"{cell}\" isn't a {} OpenDrone can read: write a plain number in {}, such as -12.5",
                        measure.name(),
                        unit.symbol()
                    ),
                )),
            }
        }
    }
    problems.or(columns)
}

/// `roll rate (°/s)`: a measure and the unit its values are in.
fn column_head(
    cell: &str,
    measurable: &dyn Fn(Measure) -> Option<String>,
) -> Result<(Measure, Unit), String> {
    let help = "each column after the time is something the Scenario measures, named as `what` names it, with its unit in brackets, such as \"roll rate (°/s)\"";
    let Some((name, unit)) = cell
        .strip_suffix(')')
        .and_then(|cell| cell.rsplit_once(" ("))
    else {
        return Err(format!("\"{cell}\": {help}"));
    };
    let Some(measure) = Measure::named(name) else {
        return Err(format!(
            "\"{name}\" isn't something the runner measures: {help}"
        ));
    };
    if measure.event().is_some() {
        return Err(format!(
            "\"{name}\" is something that happens, which a recorded flight can't hold: {help}"
        ));
    }
    if measure.is_an_angle() {
        return Err(format!(
            "\"{name}\" is an angle, which wraps round, so a run can't be compared with a recording of it yet; record its rate instead"
        ));
    }
    if let Some(why) = measurable(measure) {
        return Err(why);
    }
    let unit = units::parse_quantity(&format!("1 {unit}"))
        .map_err(|p| format!("\"{cell}\": {}", p.0))?
        .unit;
    if unit.dimension() != measure.dimension() {
        return Err(format!(
            "\"{cell}\": {} is {}",
            measure.name(),
            measure.dimension().described()
        ));
    }
    Ok((measure, unit))
}

/// A plain decimal number, such as `-12.5`.
fn plain_number(text: &str) -> Option<f64> {
    let plain = text
        .chars()
        .enumerate()
        .all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'));
    if !plain {
        return None;
    }
    text.parse().ok()
}
