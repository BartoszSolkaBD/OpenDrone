//! Each Quad's Feel Test log, `feel-tests.md`, and the rules CI checks when a
//! pull request changes a Quad definition (#16 §3, #15):
//!
//! - An Estimate that moves needs a new row in its Quad's log, and must stay
//!   inside its range. A relative range, such as `"×0.5–×2"`, is measured
//!   from the value the Estimate started at: the new value of its latest
//!   "New source" row already in the log, or else the old value of its
//!   earliest row there, or else its value before this change. A row added
//!   in the same change never sets the start.
//! - Every row a change adds records a move that change makes: the same
//!   Estimate, its value before as the old value and its value after as the
//!   new one, compared as numbers with their units, so "300 ms" matches
//!   "0.3 s". A row whose values can't be read is refused.
//! - A Measured, Manufacturer or Derived number is locked: it changes only
//!   with a new source.
//! - What's known about a number also changes only with a new source: its
//!   Confidence, its range, and where its numbers hold (a curve's places and
//!   points, the condition of a value "at" one). So a Feel Test can't widen
//!   its own range, or move a curve's points sideways instead of up and down.
//! - An Estimate moved with a new source still needs its row, and that row's
//!   why starts with "New source": the range is measured from the new value
//!   from then on. A "New source" row needs a real new source.
//! - The log only grows: earlier rows stay as they were.
//! - Quads are paired by id, not by folder ([`compare_packs`]): a change that
//!   removes a Quad may add one only as a pure rename or move.
//! - A change takes a Quad out only by renaming or moving it, or by saying so
//!   in its Pack's `[retired]` list, and every Quad taken out is named.
//!
//! "A new source" means the number names a different `[sources]` key, or the
//! line of its `[sources]` key says something different. That is easy to do,
//! so CI lists every number that passed on its source alone: changed with a
//! new source, re-sourced with a "New source" row (even one that keeps its
//! value, as it still moves where the range is measured from), taken out, or
//! added to a Quad that already existed. The Reviewer judges whether each
//! source is real.
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

/// What comparing Quads before and after a change found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeelTestReport {
    /// Every break of the Feel Test log rules.
    pub problems: Problems,
    /// Every number that passed on its source alone, as a sentence each: one
    /// that changed with a new source, one re-sourced with a "New source"
    /// row (which moves where its range is measured from, even when its value
    /// stays put), one taken out, and one added to a Quad that already
    /// existed. The Reviewer judges whether each source is real, so CI lists
    /// them.
    pub passed_on_a_new_source: Vec<String>,
    /// Every count or choice the Simulation receives that changed. They carry
    /// no Confidence, so the rules can't judge them, and CI lists them for the
    /// Reviewer too. Camera defaults and the sound block don't reach the
    /// Simulation, so they aren't listed.
    pub changed_without_a_confidence: Vec<String>,
    /// The id of every Quad compared with its version before the change.
    pub compared: Vec<String>,
    /// The id of every Quad the change adds, with nothing before it to compare.
    pub new_quads: Vec<String>,
    /// Every Quad the change takes out, as a sentence each, saying what
    /// became of it: renamed or moved (and where to), or retired by its Pack
    /// (and why). A Quad taken out any other way is a problem instead.
    pub taken_out: Vec<String>,
}

impl FeelTestReport {
    fn extend(&mut self, other: FeelTestReport) {
        self.problems.extend(other.problems);
        self.passed_on_a_new_source
            .extend(other.passed_on_a_new_source);
        self.changed_without_a_confidence
            .extend(other.changed_without_a_confidence);
        self.compared.extend(other.compared);
        self.new_quads.extend(other.new_quads);
        self.taken_out.extend(other.taken_out);
    }
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
    compare_quad(quad_file, log_file, before, after).problems
}

/// One Quad's files in one version of the repo, named by the Quad's id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadFiles {
    /// Such as `opendrone/whoop-65`: the Pack's id from its `pack.toml`, and
    /// the Quad's folder name.
    pub id: String,
    /// The definition's path, such as
    /// `packs/opendrone/quads/whoop-65/quad.toml`, for problems.
    pub quad_file: String,
    pub log_file: String,
    pub quad: String,
    pub feel_tests: Option<String>,
}

impl QuadFiles {
    fn version(&self) -> QuadVersion<'_> {
        QuadVersion {
            quad: Some(&self.quad),
            feel_tests: self.feel_tests.as_deref(),
        }
    }
}

/// A Quad that its Pack's `pack.toml` retires, in the version after a change:
/// one line of its `[retired]` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetiredQuad {
    /// Such as `opendrone/whoop-65`: the Pack's id and the Quad's folder name.
    pub id: String,
    /// The Pack's manifest, such as `packs/opendrone/pack.toml`, and the line
    /// that retires the Quad.
    pub manifest_file: String,
    pub line: usize,
    pub why: String,
}

/// Compares every Quad before a change with the same Quad after it, paired by
/// id, not by path, so moving a Pack's folder changes nothing.
///
/// A Quad that keeps its id is compared as [`check_feel_test_rules`] does.
/// When a change removes any Quad, each Quad it adds is read as that Quad
/// renamed or moved (to a new folder, another Pack, or a Pack with a new id),
/// so it must be a pure rename: every setting the same as a removed Quad's. A
/// rename that also moves a number would compare it with nothing, so it's
/// refused. Adding a Quad in a change that removes none is a new Quad, so
/// retiring a Quad and adding a different one takes two changes.
///
/// Every Quad the change takes out is named: one renamed or moved, with
/// where it went, and one its Pack retires (`retired`, from the `[retired]`
/// lists after the change), with why. Any other Quad taken out is refused, so
/// no Quad leaves the comparison unseen, to come back later as new with any
/// numbers.
pub fn compare_packs(
    before: &[QuadFiles],
    after: &[QuadFiles],
    retired: &[RetiredQuad],
) -> FeelTestReport {
    let mut report = FeelTestReport::default();
    let compare = |was: &QuadFiles, is: &QuadFiles| FeelTestReport {
        compared: vec![is.id.clone()],
        ..compare_quad(&is.quad_file, &is.log_file, was.version(), is.version())
    };
    let removed: Vec<&QuadFiles> = before
        .iter()
        .filter(|was| !after.iter().any(|is| is.id == was.id))
        .collect();
    let mut renamed_to: Vec<Option<&str>> = vec![None; removed.len()];
    for is in after {
        if let Some(was) = before.iter().find(|was| was.id == is.id) {
            report.extend(compare(was, is));
            continue;
        }
        if removed.is_empty() {
            report.new_quads.push(is.id.clone());
            continue;
        }
        let pure_rename = removed
            .iter()
            .enumerate()
            .find(|(i, was)| renamed_to[*i].is_none() && same_settings(&was.quad, &is.quad));
        match pure_rename {
            Some((i, was)) => {
                renamed_to[i] = Some(&is.id);
                report.extend(compare(was, is));
            }
            None => report.problems.push(Problem::of(
                &is.quad_file,
                0,
                format!(
                    "this change adds the Quad {} and removes {}, so it reads as a rename or a move, which must keep every setting as it was; none of the removed Quads matches it. Rename or move a Quad in a change of its own, and change its numbers in another. To retire a Quad and add a different one, take it out in one change and add the new one in another",
                    is.id,
                    removed
                        .iter()
                        .map(|was| was.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }
    for (was, renamed_to) in removed.iter().zip(renamed_to) {
        if let Some(is) = renamed_to {
            report.taken_out.push(format!(
                "{}, renamed or moved to {is} with every setting as it was",
                was.id
            ));
        } else if let Some(line) = retired.iter().find(|line| line.id == was.id) {
            report.taken_out.push(format!(
                "{}, retired by {} line {}: \"{}\"",
                was.id, line.manifest_file, line.line, line.why
            ));
        } else {
            let folder = was
                .id
                .split_once('/')
                .map_or(was.id.as_str(), |(_, quad)| quad);
            report.problems.push(Problem::of(
                &was.quad_file,
                0,
                format!(
                    "this change takes out the Quad {} without saying so. Put it back, or retire it: add \"quads/{folder}\" = \"<why>\" under [retired] in its Pack's pack.toml. To take a whole Pack out, retire its Quads in one change and take the Pack out in the next",
                    was.id
                ),
            ));
        }
    }
    report
}

/// Whether two Quad files hold the same settings: the same keys, values (read
/// with their units), Confidences and ranges.
fn same_settings(a: &str, b: &str) -> bool {
    let (Ok((a, _)), Ok((b, _))) = (
        quad::read_quad_file_as_far_as_it_goes("a", a),
        quad::read_quad_file_as_far_as_it_goes("b", b),
    ) else {
        return false;
    };
    a.settings.len() == b.settings.len()
        && a.settings.iter().all(|(name, x)| {
            b.settings.get(name).is_some_and(|y| {
                same_value(name, &x.value.text(), &y.value.text())
                    && x.confidence == y.confidence
                    && x.range.as_ref().map(Range::text) == y.range.as_ref().map(Range::text)
            })
        })
}

/// Whether a log row records a re-sourcing: its why starts with "New source".
fn is_new_source(row: &Row) -> bool {
    row.why.to_lowercase().starts_with("new source")
}

/// The value a relative range is measured from, from the log before a change:
/// the new value of the latest "New source" row for `name`, or else the old
/// value of its earliest row, or else the value before the change.
fn start_of(name: &str, rows: &[Row], value_before: &str) -> String {
    let mut start: Option<&str> = None;
    for row in rows.iter().filter(|row| row.setting == name) {
        if is_new_source(row) {
            start = Some(&row.new);
        } else if start.is_none() {
            start = Some(&row.old);
        }
    }
    start.unwrap_or(value_before).to_string()
}

/// The part of a value that says where its numbers hold, rather than the
/// numbers a range limits: a curve's places (and how many points it has), and
/// the condition of a value "at" one. Moving it changes what the number is,
/// so it needs a new source.
fn shape_change(name: &str, before: &str, after: &str) -> Option<String> {
    let form = schema::find(name).and_then(|(_, key)| movable_form(key.kind))?;
    let (before, after) = (
        read_value(name, before).ok()?,
        read_value(name, after).ok()?,
    );
    let places = |numbers: &[f64]| -> Vec<f64> {
        match form {
            Form::Curve(..) => numbers.iter().skip(1).step_by(2).copied().collect(),
            Form::At(..) => numbers.iter().skip(1).copied().collect(),
            _ => Vec::new(),
        }
    };
    let (was, is) = (places(&before), places(&after));
    if same_numbers(&was, &is) {
        return None;
    }
    Some(match form {
        Form::Curve(..) if was.len() != is.len() => {
            format!("it now has {} points and had {}", is.len(), was.len())
        }
        Form::Curve(..) => "the places its points hold at moved".to_string(),
        _ => "the condition it holds at changed".to_string(),
    })
}

/// Compares one Quad before and after a change.
fn compare_quad(
    quad_file: &str,
    log_file: &str,
    before: QuadVersion<'_>,
    after: QuadVersion<'_>,
) -> FeelTestReport {
    let mut report = FeelTestReport::default();
    let (Some(before_text), Some(after_text)) = (before.quad, after.quad) else {
        return report;
    };
    let whole_file = |sentence: &str| FeelTestReport {
        problems: Problems(vec![Problem::of(quad_file, 0, sentence)]),
        ..FeelTestReport::default()
    };
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
        Err(problems) => {
            report.problems = problems;
            return report;
        }
    };
    let before_rows = read_before.unwrap_or_default();
    let problems = &mut report.problems;

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
    let new_source_of = |name: &str| match (
        before_quad.settings.get(name),
        after_quad.settings.get(name),
    ) {
        (Some(was), Some(is)) => source_changed(&before_quad, was, &after_quad, is),
        _ => false,
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
        let new_source = new_source_of(&row.setting);
        let moved = !same_value(&row.setting, &old_value, &new_value);
        if is.confidence != Some(Confidence::Estimate) {
            problems.push(at_row(format!(
                "this row moves {row_label}, but it isn't an Estimate, so no Feel Test moves it"
            )));
        } else if is_new_source(row) && !new_source {
            problems.push(at_row(format!(
                "this row says {row_label} has a new source, but this change doesn't change its source; a \"New source\" row records a re-sourcing, which moves where its range is measured from"
            )));
        } else if !moved && !is_new_source(row) {
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
        } else if is_new_source(row) {
            // A re-sourcing moves where a relative range is measured from,
            // even when the value stays put, so it's listed either way.
            let from_there = match is.range {
                Some(Range::Relative { .. }) => ", so its range is measured from there from now on",
                _ => "",
            };
            report.passed_on_a_new_source.push(format!(
                "{log_file} line {}: {row_label} was re-sourced at {new_value} ({}){from_there}",
                row.line,
                what_is_known(&after_quad, is)
            ));
        }
    }

    // A number taken out (an optional section, such as [ducts], removed)
    // changes the Quad as much as moving it, so it needs a new source: its
    // source's line changes, or goes.
    for (name, before) in &before_quad.settings {
        if before.confidence.is_none() || after_quad.settings.contains_key(name) {
            continue;
        }
        let line_of_source = |quad: &QuadFile| {
            before
                .source
                .as_ref()
                .and_then(|key| quad.sources.get(key))
                .map(|text| plain(text))
        };
        let label = label(name);
        if line_of_source(&before_quad) == line_of_source(&after_quad) {
            problems.push(Problem::of(
                quad_file,
                0,
                format!(
                    "{label} was taken out, which changes the Quad as much as moving it, so it needs a new source: change or remove its source's line in [sources]"
                ),
            ));
        } else {
            report.passed_on_a_new_source.push(format!(
                "{quad_file}: {label} was taken out, with a new source"
            ));
        }
    }

    for (name, after) in &after_quad.settings {
        let Some(before) = before_quad.settings.get(name) else {
            // A number put in (an optional section, such as [ducts], added)
            // passes on its source alone, as one taken out does, so it's
            // listed with what's known about it.
            if after.confidence.is_some() {
                report.passed_on_a_new_source.push(format!(
                    "{quad_file} line {}: {} was added as {} ({})",
                    after.line,
                    label(name),
                    plain(&after.value.text()),
                    what_is_known(&after_quad, after)
                ));
            }
            continue;
        };
        let (Some(was), Some(is)) = (before.confidence, after.confidence) else {
            if after.confidence.is_none()
                && before.confidence.is_none()
                && schema::reaches_the_simulation(name)
                && plain(&before.value.text()) != plain(&after.value.text())
            {
                report.changed_without_a_confidence.push(format!(
                    "{quad_file} line {}: {} changed ({} → {}); it carries no Confidence",
                    after.line,
                    label(name),
                    plain(&before.value.text()),
                    plain(&after.value.text())
                ));
            }
            continue;
        };
        let new_source = new_source_of(name);
        let range_text = |s: &Setting| s.range.as_ref().map(Range::text);
        let (old_value, new_value) = (plain(&before.value.text()), plain(&after.value.text()));
        let label = label(name);
        let moved = !same_value(name, &old_value, &new_value);
        let known_changed = was != is || range_text(before) != range_text(after);
        let shape = if moved {
            shape_change(name, &old_value, &new_value)
        } else {
            None
        };
        // What's known about a number changes only with a new source: its
        // Confidence, its range, and where its numbers hold.
        if !new_source {
            if was != is {
                problems.push(after.problem(format!(
                    "{label}: its Confidence changed from {} to {}, which changes what's known about it, so it needs a new source too",
                    was.word(),
                    is.word()
                )));
            } else if range_text(before) != range_text(after) {
                problems.push(after.problem(format!(
                    "{label}: its range changed from {} to {}, which changes what's known about it, so it needs a new source too",
                    range_text(before).unwrap_or_default(),
                    range_text(after).unwrap_or_default()
                )));
            }
            if let Some(what) = &shape {
                problems.push(after.problem(format!(
                    "{label}: {what}, which changes what's known about it, so it needs a new source"
                )));
            }
        } else if moved || known_changed {
            report.passed_on_a_new_source.push(format!(
                "{quad_file} line {}: {label} changed ({}) with a new source",
                after.line,
                [
                    moved.then(|| format!("{old_value} → {new_value}")),
                    (was != is).then(|| format!("Confidence {} → {}", was.word(), is.word())),
                    (range_text(before) != range_text(after)).then(|| {
                        format!(
                            "range {} → {}",
                            range_text(before).unwrap_or_default(),
                            range_text(after).unwrap_or_default()
                        )
                    }),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ")
            ));
        }
        if !moved {
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
        let row = new_rows.iter().find(|row| {
            &row.setting == name
                && same_value(name, &row.old, &old_value)
                && same_value(name, &row.new, &new_value)
        });
        let Some(row) = row else {
            problems.push(after.problem(format!(
                "{label} is an Estimate that moved from {old_value} to {new_value}, so {log_file} needs a new row for it: the date, {label}, {old_value} → {new_value}, and why"
            )));
            continue;
        };
        // A move that comes with a new source, recorded as such, starts its
        // range afresh: the new value is where it's measured from next time.
        if new_source && is_new_source(row) {
            if let Some(sentence) = outside_absolute_range(name, after, &new_value) {
                problems.push(after.problem(sentence));
            }
            continue;
        }
        if new_source {
            problems.push(Problem::of(
                log_file,
                row.line,
                format!(
                    "{label} moved with a new source, so its row's why starts with \"New source\", which records where its range is measured from now"
                ),
            ));
            continue;
        }
        // A shape change without a new source is refused above already.
        if shape.is_some() {
            continue;
        }
        let start = start_of(name, &before_rows, &old_value);
        if let Some(sentence) = outside_its_range(name, after, &new_value, &start) {
            problems.push(after.problem(sentence));
        }
    }
    report
}

/// A number's Confidence, its range and its source, with that source's line
/// in `[sources]`, such as `Estimate, range ×0.5–×2, from the source guess:
/// "a guess"`.
fn what_is_known(quad: &QuadFile, setting: &Setting) -> String {
    let mut parts = Vec::new();
    if let Some(confidence) = setting.confidence {
        parts.push(confidence.word().to_string());
    }
    if let Some(range) = &setting.range {
        parts.push(format!("range {}", range.text()));
    }
    if let Some(key) = &setting.source {
        parts.push(match quad.sources.get(key) {
            Some(line) => format!("from the source {key}: \"{}\"", plain(line)),
            None => format!("from the source {key}"),
        });
    }
    parts.join(", ")
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

/// An Estimate's new value against an absolute range.
fn outside_absolute_range(name: &str, setting: &Setting, new_value: &str) -> Option<String> {
    let range @ Range::Absolute { .. } = setting.range.as_ref()? else {
        return None;
    };
    let form = schema::find(name).and_then(|(_, key)| movable_form(key.kind))?;
    let label = label(name);
    let new = match read_value(name, new_value) {
        Ok(new) => new,
        Err(sentence) => return Some(format!("{label}'s new value can't be read: {sentence}")),
    };
    quad::outside_range(form, range, new_value, &new).map(|sentence| format!("{label}: {sentence}"))
}

/// Whether an Estimate's new value is outside its range: an absolute range
/// directly, a relative one measured from `start`, point by point on a curve
/// (whose places the shape check holds still). A value that can't be read is
/// a problem, never a pass.
fn outside_its_range(
    name: &str,
    setting: &Setting,
    new_value: &str,
    start: &str,
) -> Option<String> {
    let range = setting.range.as_ref()?;
    let Range::Relative { low, high } = range else {
        return outside_absolute_range(name, setting, new_value);
    };
    let label = label(name);
    let form = schema::find(name).and_then(|(_, key)| movable_form(key.kind))?;
    let new = match read_value(name, new_value) {
        Ok(new) => new,
        Err(sentence) => return Some(format!("{label}'s new value can't be read: {sentence}")),
    };
    let from = match read_value(name, start) {
        Ok(from) => from,
        Err(sentence) => {
            return Some(format!(
                "{label}'s relative range is measured from the value it started at, {start}, which can't be read: {sentence}"
            ));
        }
    };
    if shape_change(name, start, new_value).is_some() {
        return Some(format!(
            "{label}'s relative range is measured from the value it started at, {start}, whose points or condition differ from its value now; record its re-sourcing with a \"New source\" row"
        ));
    }
    let (new, from) = (quad::ranged(form, &new), quad::ranged(form, &from));
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
