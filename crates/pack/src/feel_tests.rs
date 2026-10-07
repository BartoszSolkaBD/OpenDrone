//! Each Quad's Feel Test log, `feel-tests.md`, and the rules CI checks when a
//! pull request changes a Quad definition (#16 §3, #15):
//!
//! - An Estimate that moves needs a new row in its Quad's log, and must stay
//!   inside its range. A relative range, such as `"×0.5–×2"`, is measured
//!   from the value the Estimate started at: the old value of its earliest row
//!   already in the log before this change, or, with none, the value before
//!   this change. A row added in the same change never sets the start.
//! - Every row a change adds records a move that change makes: the same
//!   Estimate, its value before as the old value and its value after as the
//!   new one, compared as numbers with their units, so "300 ms" matches
//!   "0.3 s". A row whose values can't be read is refused.
//! - A Measured, Manufacturer or Derived number is locked: it changes only
//!   with a new source.
//! - An Estimate's range, or any number's Confidence, also changes only with
//!   a new source, so a Feel Test can't widen its own range, and so does the
//!   number of points on a curve. An Estimate that moves in the same change
//!   still needs its row.
//! - The log only grows: earlier rows stay as they were.
//!
//! "A new source" means the number names a different `[sources]` key, or the
//! line of its `[sources]` key says something different.
//!
//! The log is a Markdown table, oldest row first:
//!
//! ```text
//! | Date | Number | Old → new | Why |
//! |---|---|---|---|
//! | 2026-11-02 | [props] rotor_drag | 0.3 s⁻¹ → 0.35 s⁻¹ | carved too wide after a sprint |
//! ```

use crate::document::{Problem, Problems};
use crate::quad::{self, Confidence, QuadFile, Setting, label};
use crate::schema::{self, Form, Kind};
use crate::units::Range;

/// One row of a Feel Test log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub date: String,
    /// The setting it moved, as `section.key`, such as `props.rotor_drag`.
    pub setting: String,
    pub old: String,
    pub new: String,
    pub why: String,
    pub line: usize,
}

impl Row {
    fn same_as(&self, other: &Row) -> bool {
        (&self.date, &self.setting, &self.old, &self.new, &self.why)
            == (
                &other.date,
                &other.setting,
                &other.old,
                &other.new,
                &other.why,
            )
    }
}

const HEADER: [&str; 4] = ["Date", "Number", "Old → new", "Why"];

/// Reads a Feel Test log's rows. The file may say anything before its table,
/// such as headings and a few words on how it's kept. Each row must name a
/// number a Feel Test can move, and its old and new values must read as that
/// number, with their units.
pub fn read_feel_tests(file: &str, text: &str) -> Result<Vec<Row>, Problems> {
    read_rows(file, text, true)
}

/// Reads the rows; with `check_values`, each value must read in its
/// setting's form. The version of a log before a change is read without that,
/// so a stricter checker can still compare it.
fn read_rows(file: &str, text: &str, check_values: bool) -> Result<Vec<Row>, Problems> {
    let mut problems = Problems::new();
    let problem = |line: usize, sentence: String| Problem::of(file, line, sentence);
    let lines: Vec<&str> = text.lines().collect();
    let Some(header) = lines.iter().position(|line| cells(line) == HEADER) else {
        return Err(Problems(vec![problem(
            0,
            "a Feel Test log holds one table with the columns Date, Number, Old → new and Why: `| Date | Number | Old → new | Why |`".into(),
        )]));
    };
    let mut rows = Vec::new();
    for (index, raw) in lines.iter().enumerate().skip(header + 2) {
        let line = index + 1;
        if !raw.trim_start().starts_with('|') {
            break;
        }
        let found = cells(raw);
        let [date, number, change, why] = found.as_slice() else {
            problems.push(problem(
                line,
                "a Feel Test log row has four cells: the date, the number, old → new, and why"
                    .into(),
            ));
            continue;
        };
        let mut fine = true;
        if !is_a_date(date) {
            problems.push(problem(
                line,
                format!("\"{date}\" isn't a date: write it as year-month-day, such as 2026-11-02"),
            ));
            fine = false;
        }
        let setting = setting_name(number);
        let form = schema::find(&setting).map(|(_, key)| movable_form(key.kind));
        match form {
            None => {
                problems.push(problem(
                    line,
                    format!(
                        "\"{setting}\" isn't a setting of a Quad definition: name it as the file does, such as [props] rotor_drag"
                    ),
                ));
                fine = false;
            }
            Some(None) => {
                problems.push(problem(
                    line,
                    format!(
                        "{} isn't a number a Feel Test moves: only physics numbers and the camera's limits have Estimates",
                        label(&setting)
                    ),
                ));
                fine = false;
            }
            Some(Some(_)) => {}
        }
        let split = change
            .split_once('→')
            .or_else(|| change.split_once("->"))
            .map(|(old, new)| (plain(old), plain(new)));
        let Some((old, new)) = split.filter(|(old, new)| !old.is_empty() && !new.is_empty()) else {
            problems.push(problem(
                line,
                format!("\"{change}\" needs the old and the new value with an arrow between them, such as 0.3 s⁻¹ → 0.35 s⁻¹"),
            ));
            continue;
        };
        if let (true, Some(Some(form))) = (check_values, form) {
            for (which, value) in [("old", &old), ("new", &new)] {
                if let Err(sentence) = quad::numbers(&label(&setting), form, value) {
                    problems.push(problem(
                        line,
                        format!(
                            "the {which} value, \"{value}\", doesn't read as {}: {sentence}",
                            label(&setting)
                        ),
                    ));
                    fine = false;
                }
            }
        }
        if why.is_empty() {
            problems.push(problem(line, "every Feel Test log row says why".into()));
            fine = false;
        }
        if fine {
            rows.push(Row {
                date: date.to_string(),
                setting,
                old,
                new,
                why: why.to_string(),
                line,
            });
        }
    }
    problems.or(rows)
}

/// The cells of a Markdown table line, trimmed.
fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let Some(inner) = line.strip_prefix('|') else {
        return Vec::new();
    };
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// `[props] rotor_drag` or `props.rotor_drag`, with or without backticks, as
/// `props.rotor_drag`.
fn setting_name(cell: &str) -> String {
    let cell = cell.trim().trim_matches('`').trim();
    match cell.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((section, key)) => format!("{}.{}", section.trim(), key.trim()),
        None => cell.to_string(),
    }
}

/// A value with its spaces evened out and any backticks taken off, so the
/// log and the Quad file can be compared.
fn plain(text: &str) -> String {
    text.trim()
        .trim_matches('`')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The form of a number a Feel Test can move: a physics number or one of the
/// camera's limits, the kinds that carry a Confidence.
fn movable_form(kind: Kind) -> Option<Form> {
    match kind {
        Kind::Physics(form) | Kind::CameraLimit(form) => Some(form),
        _ => None,
    }
}

/// A value of `name` read as numbers in SI units, or why it can't be.
fn read_value(name: &str, text: &str) -> Result<Vec<f64>, String> {
    let form = schema::find(name)
        .and_then(|(_, key)| movable_form(key.kind))
        .ok_or_else(|| format!("{} isn't a number a Feel Test moves", label(name)))?;
    quad::numbers(&label(name), form, text)
}

/// Whether two readings are the same numbers, allowing for the last bit of a
/// unit conversion ("300 ms" and "0.3 s").
fn same_numbers(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            let scale = if a.abs() > b.abs() { a.abs() } else { b.abs() };
            (a - b).abs() <= scale * 1e-12
        })
}

/// Whether two values of `name` are the same, read with their units when
/// both can be, and as text when not.
fn same_value(name: &str, a: &str, b: &str) -> bool {
    match (read_value(name, a), read_value(name, b)) {
        (Ok(a), Ok(b)) => same_numbers(&a, &b),
        _ => plain(a) == plain(b),
    }
}

fn is_a_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    matches!(parts.as_slice(), [y, m, d]
        if y.len() == 4 && m.len() == 2 && d.len() == 2
        && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())))
}

/// One version of a Quad's two files: its definition and its Feel Test log,
/// either of which may be missing.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadVersion<'a> {
    pub quad: Option<&'a str>,
    pub feel_tests: Option<&'a str>,
}

/// Compares a Quad before and after a change, and lists every break of the
/// Feel Test log rules. `quad_file` and `log_file` name the two files in
/// problems. A Quad that is new, or removed, has nothing to compare.
pub fn check_feel_test_rules(
    quad_file: &str,
    log_file: &str,
    before: QuadVersion<'_>,
    after: QuadVersion<'_>,
) -> Problems {
    let (Some(before_text), Some(after_text)) = (before.quad, after.quad) else {
        return Problems::new();
    };
    let whole_file = |sentence: &str| Problems(vec![Problem::of(quad_file, 0, sentence)]);
    let Ok(after_quad) = quad::read_quad_file(quad_file, after_text) else {
        return whole_file(
            "the Feel Test log rules can't be checked until this file passes the Pack checker (`cargo xtask packs`)",
        );
    };
    // The version before may come from an older checker that read less, so
    // every setting of it that still reads is compared.
    let Ok((before_quad, _)) = quad::read_quad_file_as_far_as_it_goes(quad_file, before_text)
    else {
        return whole_file(
            "the version before this change isn't readable TOML, so the Feel Test log rules can't compare it",
        );
    };
    let read_before = match before.feel_tests {
        Some(text) => read_rows(log_file, text, false),
        None => Ok(Vec::new()),
    };
    let read_after = match after.feel_tests {
        Some(text) => read_feel_tests(log_file, text),
        None => Ok(Vec::new()),
    };
    let after_rows = match read_after {
        Ok(rows) => rows,
        Err(problems) => return problems,
    };
    let before_rows = read_before.unwrap_or_default();

    let mut problems = Problems::new();
    let kept = before_rows.len() <= after_rows.len()
        && before_rows
            .iter()
            .zip(&after_rows)
            .all(|(before, after)| before.same_as(after));
    if !kept {
        problems.push(Problem::of(
            log_file,
            0,
            "the Feel Test log only grows: a row from before this change was changed or removed; put it back and add a new row instead",
        ));
    }
    let new_rows: &[Row] = if kept {
        &after_rows[before_rows.len()..]
    } else {
        &[]
    };

    // Every row this change adds must record a move this change makes.
    for (i, row) in new_rows.iter().enumerate() {
        let at_row = |sentence: String| Problem::of(log_file, row.line, sentence);
        let row_label = label(&row.setting);
        if new_rows[..i]
            .iter()
            .any(|earlier| earlier.setting == row.setting)
        {
            problems.push(at_row(format!(
                "this change already has a row for {row_label}; one move, one row"
            )));
            continue;
        }
        let (Some(was), Some(is)) = (
            before_quad.settings.get(&row.setting),
            after_quad.settings.get(&row.setting),
        ) else {
            problems.push(at_row(format!(
                "this row moves {row_label}, but this change doesn't: the Quad had no such number before it, or has none now"
            )));
            continue;
        };
        let (old_value, new_value) = (plain(&was.value.text()), plain(&is.value.text()));
        if is.confidence != Some(Confidence::Estimate) {
            problems.push(at_row(format!(
                "this row moves {row_label}, but it isn't an Estimate, so no Feel Test moves it"
            )));
        } else if same_value(&row.setting, &old_value, &new_value) {
            problems.push(at_row(format!(
                "this row records {row_label} moving from {} to {}, but this change doesn't move it",
                row.old, row.new
            )));
        } else if !same_value(&row.setting, &row.old, &old_value)
            || !same_value(&row.setting, &row.new, &new_value)
        {
            problems.push(at_row(format!(
                "this row records {row_label} moving from {} to {}, but this change moves it from {old_value} to {new_value}",
                row.old, row.new
            )));
        }
    }

    for (name, after) in &after_quad.settings {
        let Some(before) = before_quad.settings.get(name) else {
            continue;
        };
        let (Some(was), Some(is)) = (before.confidence, after.confidence) else {
            continue;
        };
        let new_source = source_changed(&before_quad, before, &after_quad, after);
        let range_text = |s: &Setting| s.range.as_ref().map(Range::text);
        let (old_value, new_value) = (plain(&before.value.text()), plain(&after.value.text()));
        let label = label(name);
        // What's known about the number changes only with a new source.
        if (was != is || range_text(before) != range_text(after)) && !new_source {
            let what = if was != is {
                format!(
                    "its Confidence changed from {} to {}",
                    was.word(),
                    is.word()
                )
            } else {
                format!(
                    "its range changed from {} to {}",
                    range_text(before).unwrap_or_default(),
                    range_text(after).unwrap_or_default()
                )
            };
            problems.push(after.problem(format!(
                "{label}: {what}, which changes what's known about it, so it needs a new source too"
            )));
        }
        if same_value(name, &old_value, &new_value) {
            continue;
        }
        if is != Confidence::Estimate {
            if !new_source {
                problems.push(after.problem(format!(
                    "{label} is {}, so it's locked: it changed from {old_value} to {new_value} without a new source; name a new source, or update its line in [sources]",
                    is.word()
                )));
            }
            continue;
        }
        // An Estimate that moves, whatever else changed with it, is logged
        // and stays inside its range.
        let logged = new_rows.iter().any(|row| {
            &row.setting == name
                && same_value(name, &row.old, &old_value)
                && same_value(name, &row.new, &new_value)
        });
        if !logged {
            problems.push(after.problem(format!(
                "{label} is an Estimate that moved from {old_value} to {new_value}, so {log_file} needs a new row for it: the date, {label}, {old_value} → {new_value}, and why"
            )));
        }
        // Its start: the earliest row already in the log before this change,
        // or the value before it. Never a row this change adds.
        let start = before_rows
            .iter()
            .find(|row| &row.setting == name)
            .map_or(old_value.clone(), |row| row.old.clone());
        if let Some(sentence) = outside_its_range(name, after, &new_value, &start, new_source) {
            problems.push(after.problem(sentence));
        }
    }
    problems
}

fn source_changed(
    before_quad: &QuadFile,
    before: &Setting,
    after_quad: &QuadFile,
    after: &Setting,
) -> bool {
    let line = |quad: &QuadFile, s: &Setting| {
        s.source
            .as_ref()
            .and_then(|key| quad.sources.get(key))
            .map(|text| plain(text))
    };
    before.source != after.source || line(before_quad, before) != line(after_quad, after)
}

/// Whether an Estimate's new value is outside its range: an absolute range
/// directly, a relative one measured from `start`, point by point on a curve.
/// A value that can't be read is a problem, never a pass.
fn outside_its_range(
    name: &str,
    setting: &Setting,
    new_value: &str,
    start: &str,
    new_source: bool,
) -> Option<String> {
    let range = setting.range.as_ref()?;
    let label = label(name);
    let form = schema::find(name).and_then(|(_, key)| movable_form(key.kind))?;
    let new = match read_value(name, new_value) {
        Ok(new) => new,
        Err(sentence) => return Some(format!("{label}'s new value can't be read: {sentence}")),
    };
    match range {
        Range::Absolute { .. } => quad::outside_range(form, range, new_value, &new)
            .map(|sentence| format!("{label}: {sentence}")),
        Range::Relative { low, high } => {
            let from = match read_value(name, start) {
                Ok(from) => from,
                Err(sentence) => {
                    return Some(format!(
                        "{label}'s relative range is measured from the value it started at, {start}, which can't be read: {sentence}"
                    ));
                }
            };
            let (new, from) = (quad::ranged(form, &new), quad::ranged(form, &from));
            if new.len() != from.len() {
                return (!new_source).then(|| {
                    format!(
                        "{label} now has {} points and started with {}; a change in its points changes what's known about it, so it needs a new source",
                        new.len(),
                        from.len()
                    )
                });
            }
            let fits = new.iter().zip(&from).all(|(new, from)| {
                if *from == 0.0 {
                    *new == 0.0
                } else {
                    let ratio = new / from;
                    *low <= ratio && ratio <= *high
                }
            });
            (!fits).then(|| {
                format!(
                    "{label} moved to {new_value}, outside its range, {}, measured from {start}, the value it started at",
                    range.text()
                )
            })
        }
    }
}
