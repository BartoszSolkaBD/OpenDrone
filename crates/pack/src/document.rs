//! Reading TOML 1.1 files with line numbers, so every problem the Pack reader
//! and the Scenario runner report names its file, its line and a plain
//! sentence (#16 §10), and all of a file's problems are listed at once.

use core::fmt;
use core::ops::Range;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::migration::{PACK_STEPS, Step};

/// One problem in one file, as a plain sentence. `line` 0 means the file as a
/// whole.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    pub file: String,
    pub line: usize,
    pub sentence: String,
}

impl Problem {
    /// A problem on `line` of `file` (0 for the file as a whole).
    pub fn of(file: impl Into<String>, line: usize, sentence: impl Into<String>) -> Problem {
        Problem {
            file: file.into(),
            line,
            sentence: sentence.into(),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.sentence)
        } else {
            write!(f, "{} line {}: {}", self.file, self.line, self.sentence)
        }
    }
}

/// Every problem found, in the order they were found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problems(pub Vec<Problem>);

impl Problems {
    pub fn new() -> Problems {
        Problems(Vec::new())
    }

    pub fn push(&mut self, problem: Problem) {
        self.0.push(problem);
    }

    pub fn extend(&mut self, other: Problems) {
        self.0.extend(other.0);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `Ok(value)` when there are no problems, or every problem.
    pub fn or<T>(self, value: T) -> Result<T, Problems> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
    }

    /// A problem with a whole file, not one line.
    pub fn of_file(file: impl Into<String>, sentence: impl Into<String>) -> Problems {
        Problems(vec![Problem {
            file: file.into(),
            line: 0,
            sentence: sentence.into(),
        }])
    }
}

impl fmt::Display for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, problem) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{problem}")?;
        }
        Ok(())
    }
}

/// The newest Pack file format this OpenDrone reads. Every Pack file starts
/// with `format = N` (ADR-0011). Format 1 is the first, and each step in
/// [`PACK_STEPS`] adds one, so the number follows the steps. Scenarios are
/// numbered apart (`opendrone_scenario::SCENARIO_FORMAT`).
pub const FORMAT: i64 = 1 + PACK_STEPS.len() as i64;

/// A TOML file read with the place of every key and value.
pub struct Document<'t> {
    file: String,
    text: &'t str,
    root: Spanned<DeTable<'t>>,
}

impl<'t> Document<'t> {
    /// Reads `text`, naming it `file` in problems.
    pub fn parse(file: impl Into<String>, text: &'t str) -> Result<Document<'t>, Problems> {
        let file = file.into();
        match DeTable::parse(text) {
            Ok(root) => Ok(Document { file, text, root }),
            Err(error) => {
                let line = error.span().map_or(0, |span| line_of(text, span.start));
                let message = error.message().trim().trim_end_matches('.');
                Err(Problems(vec![Problem {
                    file,
                    line,
                    sentence: format!("this isn't readable TOML: {message}"),
                }]))
            }
        }
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    /// The whole file, as a table.
    pub fn root(&self) -> Table<'_, 't> {
        Table {
            doc: self,
            table: self.root.get_ref(),
            start: 0,
            name: String::new(),
        }
    }

    /// A problem at a place in the file.
    pub fn problem_at(&self, offset: usize, sentence: impl Into<String>) -> Problem {
        Problem {
            file: self.file.clone(),
            line: line_of(self.text, offset),
            sentence: sentence.into(),
        }
    }

    /// Checks the `format = N` line every Pack file starts with, after
    /// [`crate::migration::upgraded`] has brought an older file up to date.
    pub fn check_format(&self, problems: &mut Problems) {
        self.check_format_against(FORMAT, PACK_STEPS, problems);
    }

    /// Checks the `format = N` line every file starts with, against `newest`,
    /// the newest format this OpenDrone reads for this kind of file. A file
    /// in an older format is refused, naming the `steps` that bring it up to
    /// date with `cargo xtask migrate`.
    pub fn check_format_against(&self, newest: i64, steps: &[Step], problems: &mut Problems) {
        let root = self.root();
        let Some(item) = root.get("format") else {
            problems.push(Problem {
                file: self.file.clone(),
                line: 1,
                sentence: format!("every file starts with `format = {newest}`"),
            });
            return;
        };
        if root
            .entries()
            .first()
            .is_some_and(|(key, _)| key != "format")
        {
            problems.push(item.problem(format!(
                "every file starts with `format = {newest}`, before anything else but comments"
            )));
        }
        match item.integer() {
            Some(n) if n == newest => {}
            Some(n) if n > newest => problems.push(item.problem(format!(
                "this file is format {n}, so it needs a newer OpenDrone: this one reads format {newest}"
            ))),
            Some(n) if n >= 1 => problems.push(item.problem(older(n, newest, steps))),
            _ => problems.push(item.problem(format!(
                "`format` must be a whole number from 1 to {newest}"
            ))),
        }
    }

    /// Whether the file is written in a newer format than this OpenDrone
    /// reads. Its other keys may mean something this one doesn't know, so a
    /// reader stops after saying it needs a newer OpenDrone.
    pub fn is_newer(&self) -> bool {
        self.root()
            .get("format")
            .and_then(|item| item.integer())
            .is_some_and(|n| n > FORMAT)
    }
}

/// Says that a file in format `n` is older than `newest`, and which steps
/// bring it up to date.
fn older(n: i64, newest: i64, steps: &[Step]) -> String {
    let mut commands = Vec::new();
    for format in n..newest {
        match steps.iter().find(|step| step.from == format) {
            Some(step) => commands.push(format!("`cargo xtask migrate {}`", step.name)),
            None => {
                return format!(
                    "this file is format {n}, older than the format {newest} this OpenDrone reads, and no step upgrades format {format}"
                );
            }
        }
    }
    format!(
        "this file is format {n}, older than the format {newest} this OpenDrone reads: bring it up to date with {}",
        commands.join(", then ")
    )
}

fn line_of(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.as_bytes()[..offset]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// A table in a [`Document`], such as `[start]` or an inline `{ … }`.
#[derive(Clone)]
pub struct Table<'d, 't> {
    doc: &'d Document<'t>,
    table: &'d DeTable<'t>,
    start: usize,
    name: String,
}

/// One value in a [`Document`], with where it is.
#[derive(Clone)]
pub struct Item<'d, 't> {
    doc: &'d Document<'t>,
    value: &'d Spanned<DeValue<'t>>,
    key: String,
}

impl<'d, 't> Table<'d, 't> {
    /// The table's name as the file writes it, such as `[start.rates]`, or
    /// "the top of the file".
    pub fn name(&self) -> String {
        if self.name.is_empty() {
            "the top of the file".to_string()
        } else {
            format!("[{}]", self.name)
        }
    }

    pub fn get(&self, key: &str) -> Option<Item<'d, 't>> {
        self.table.get(key).map(|value| Item {
            doc: self.doc,
            value,
            key: self.key_path(key),
        })
    }

    /// Every key, in the order the file writes them.
    pub fn entries(&self) -> Vec<(String, Item<'d, 't>)> {
        let mut entries: Vec<(&Spanned<_>, &Spanned<DeValue<'t>>)> = self.table.iter().collect();
        entries.sort_by_key(|(key, _)| key.span().start);
        entries
            .into_iter()
            .map(|(key, value)| {
                let name = key.get_ref().to_string();
                let item = Item {
                    doc: self.doc,
                    value,
                    key: self.key_path(&name),
                };
                (name, item)
            })
            .collect()
    }

    /// A problem on the table's first line.
    pub fn problem(&self, sentence: impl Into<String>) -> Problem {
        self.doc.problem_at(self.start, sentence)
    }

    /// The value at `key`, or a problem saying it is missing.
    pub fn require(&self, key: &str, problems: &mut Problems) -> Option<Item<'d, 't>> {
        let item = self.get(key);
        if item.is_none() {
            problems.push(self.problem(format!("{} is missing `{key}`", self.name())));
        }
        item
    }

    /// The text at `key`, or a problem.
    pub fn text(&self, key: &str, problems: &mut Problems) -> Option<(&'d str, Item<'d, 't>)> {
        let item = self.require(key, problems)?;
        let text = item.text(problems)?;
        Some((text, item))
    }

    /// The table at `key`, or a problem.
    pub fn table(&self, key: &str, problems: &mut Problems) -> Option<Table<'d, 't>> {
        self.require(key, problems)?.table(problems)
    }

    /// Refuses every key not in `known`, naming the ones that are.
    pub fn refuse_unknown(&self, known: &[&str], problems: &mut Problems) {
        for (key, item) in self.entries() {
            if !known.contains(&key.as_str()) {
                problems.push(item.problem(format!(
                    "`{key}` isn't something OpenDrone reads in {}; it reads {}",
                    self.name(),
                    known
                        .iter()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }
    }

    fn key_path(&self, key: &str) -> String {
        if self.name.is_empty() {
            key.to_string()
        } else {
            format!("{}.{key}", self.name)
        }
    }
}

impl<'d, 't> Item<'d, 't> {
    /// The key's full name, such as `start.rates.roll`.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn line(&self) -> usize {
        line_of(self.doc.text, self.value.span().start)
    }

    /// A problem on this value's line.
    pub fn problem(&self, sentence: impl Into<String>) -> Problem {
        self.doc.problem_at(self.value.span().start, sentence)
    }

    /// The value as text, if it is text.
    pub fn as_str(&self) -> Option<&'d str> {
        self.value.get_ref().as_str()
    }

    /// The value as text, or a problem. A bare TOML number is refused,
    /// because every number is written as text with its unit.
    pub fn text(&self, problems: &mut Problems) -> Option<&'d str> {
        if let Some(text) = self.as_str() {
            return Some(text);
        }
        let sentence = match self.value.get_ref() {
            DeValue::Integer(_) | DeValue::Float(_) => format!(
                "`{}` is a bare number: write it as text with its unit, in quotes, such as \"9.81 m/s²\"",
                self.key
            ),
            other => format!(
                "`{}` must be text in quotes, not {}",
                self.key,
                other.type_str()
            ),
        };
        problems.push(self.problem(sentence));
        None
    }

    /// The value as a whole number, if it is one.
    pub fn integer(&self) -> Option<i64> {
        let integer = self.value.get_ref().as_integer()?;
        i64::from_str_radix(integer.as_str(), integer.radix()).ok()
    }

    /// The value as `true` or `false`, if it is one.
    pub fn boolean(&self) -> Option<bool> {
        self.value.get_ref().as_bool()
    }

    /// The value as a table, or a problem.
    pub fn table(&self, problems: &mut Problems) -> Option<Table<'d, 't>> {
        match self.value.get_ref().as_table() {
            Some(table) => Some(Table {
                doc: self.doc,
                table,
                start: self.value.span().start,
                name: self.key.clone(),
            }),
            None => {
                problems.push(self.problem(format!("`{}` must be a table", self.key)));
                None
            }
        }
    }

    /// True when the value is a table.
    pub fn is_table(&self) -> bool {
        self.value.get_ref().is_table()
    }

    /// The value's items, if it is a list, or a problem.
    pub fn array(&self, problems: &mut Problems) -> Option<Vec<Item<'d, 't>>> {
        match self.value.get_ref().as_array() {
            Some(array) => Some(
                array
                    .iter()
                    .enumerate()
                    .map(|(i, value)| Item {
                        doc: self.doc,
                        value,
                        key: format!("{}[{}]", self.key, i + 1),
                    })
                    .collect(),
            ),
            None => {
                problems.push(self.problem(format!("`{}` must be a list in [ ]", self.key)));
                None
            }
        }
    }

    /// Where the value starts and ends in the file.
    pub fn span(&self) -> Range<usize> {
        self.value.span()
    }
}
