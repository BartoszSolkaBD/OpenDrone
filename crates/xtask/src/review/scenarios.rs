//! The Expectations in Scenario files, compared between the base and the head
//! (#11 §3, #15 §5).
//!
//! Source and Rule Expectations are locked: changing one in any way, removing
//! it, or loosening its tolerance waits for the maintainer. Observed ones may
//! be updated with a one-line reason; a loosened tolerance on one, or removing
//! one, is for the Reviewer to decide. A deleted Scenario is for the Reviewer
//! too. An Expectation is matched by what it measures and when (with its
//! moment read as a time, so "1 s" and "1.0 s" match), and, inside a Flight
//! Controller Scenario's `[[case]]`, by its case: the case's sticks, Arm
//! switch and sensor readings (with each number read as a value, so "0%" and
//! "0.0 %" match). So moving it within the file changes nothing. Expectations
//! are read wherever the Scenario format lets them sit. A deleted Scenario's
//! Expectations are looked for in every Scenario the pull request adds,
//! whatever its file or name: one found unchanged has only moved, and a
//! Source or Rule one found nowhere unchanged waits for the maintainer.
//!
//! A Scenario's setup, the flight its Expectations check, is compared too:
//! every field but its Expectations and the cases they sit in, and the Test
//! Quad it flies. When a deleted Scenario's Expectations went to several new
//! files, each new file's setup is compared with the old one.

use std::collections::{BTreeMap, BTreeSet};

use opendrone_pack::units::{Dimension, Expected, Tolerance, parse_expected, parse_quantity};

use super::changes::Changes;
use super::flags::{Level, RedFlag};
use super::markdown::{code, plain};

/// Where the Scenarios live.
pub const FOLDER: &str = "scenarios/";
/// The ending of the Results files beside the Scenarios.
pub const RESULTS_ENDING: &str = ".results.toml";
/// The folder of Test Quads, which aren't Scenarios.
const TEST_QUADS: &str = "scenarios/test-quads/";
/// The setup field naming the Quad a Scenario flies, such as
/// `test/whoop-65-no-drag`.
const QUAD_FIELD: &str = "start.quad";

/// The keys that hold an Expectation's expected value, one per kind: at a
/// moment, or a statistic over a stretch (`first` for when something
/// happens).
const VALUE_KEYS: [&str; 6] = ["value", "mean", "lowest", "highest", "final", "first"];

/// Whether a path is a Scenario file.
pub fn is_scenario(path: &str) -> bool {
    path.starts_with(FOLDER)
        && path.ends_with(".toml")
        && !path.ends_with(RESULTS_ENDING)
        && !path.starts_with(TEST_QUADS)
}

/// A Scenario file as far as Red Flags need it.
struct ScenarioFile {
    name: Option<String>,
    expectations: Vec<Expectation>,
    /// Everything that sets up the flight: every field but the Expectations
    /// (with the cases they sit in), the name and the format, by dotted
    /// name, such as `start.physics_rate`.
    setup: BTreeMap<String, toml::Value>,
}

impl ScenarioFile {
    fn read(text: &str) -> Option<ScenarioFile> {
        let table: toml::Table = text.parse().ok()?;
        let mut expectations = Vec::new();
        collect(&table, &Place::default(), &mut expectations);
        let mut setup = BTreeMap::new();
        flatten(&table, "", &mut setup);
        setup.remove("name");
        setup.remove("format");
        Some(ScenarioFile {
            name: table
                .get("name")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            expectations,
            setup,
        })
    }
}

/// Every field of a table, by dotted name, but its Expectations and the
/// tables they sit in, such as a Flight Controller Scenario's `[[case]]`s: a
/// case's sticks are part of what its Expectations measure, as a moment is.
fn flatten(table: &toml::Table, prefix: &str, into: &mut BTreeMap<String, toml::Value>) {
    for (key, value) in table {
        if key == "expect" {
            continue;
        }
        let name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(inner) => flatten(inner, &name, into),
            toml::Value::Array(items) if items.iter().any(holds_expectations) => {}
            _ => {
                into.insert(name, value.clone());
            }
        }
    }
}

/// Whether a value is a table that holds Expectations, at any depth.
fn holds_expectations(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => {
            table.contains_key("expect") || table.values().any(holds_expectations)
        }
        toml::Value::Array(items) => items.iter().any(holds_expectations),
        _ => false,
    }
}

/// Every Expectation in the file, wherever it sits: `[[expect]]` at the top,
/// inside a section such as `[osd]`, or inside each table of an array of
/// tables, such as `[[case.expect]]` in a Flight Controller Scenario's
/// `[[case]]`s. Every `expect` list is read, in whatever table it sits; the
/// check at the end of this file fails if the Scenario runner ever measures
/// an Expectation this doesn't read.
fn collect(table: &toml::Table, place: &Place, into: &mut Vec<Expectation>) {
    for (key, value) in table {
        match value {
            toml::Value::Array(items) if key == "expect" => {
                for entry in items {
                    if let Some(entry) = entry.as_table() {
                        into.push(Expectation {
                            place: place.clone(),
                            table: entry.clone(),
                        });
                    }
                }
            }
            toml::Value::Table(inner) => collect(inner, &place.section(key), into),
            toml::Value::Array(items) => {
                for item in items {
                    if let Some(inner) = item.as_table() {
                        collect(inner, &place.case(key, inner), into);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Where in its file an Expectation sits.
#[derive(Clone, Debug, Default, PartialEq)]
struct Place {
    /// The section, such as `osd`, or nothing for the top.
    section: String,
    /// The tables of an array of tables it sits in, such as one `[[case]]`,
    /// each as the Report shows it: its key and its own fields, such as
    /// `case with arm on; pitch 0%; …`.
    cases: Vec<String>,
    /// What it's matched by: the same, with each case's fields read as
    /// values where they're numbers, so `0%` and `0.0 %` are the same stick.
    read: String,
}

impl Place {
    /// A section inside this place, such as `[osd]`.
    fn section(&self, key: &str) -> Place {
        let join = |outer: &str| {
            if outer.is_empty() {
                key.to_string()
            } else {
                format!("{outer}.{key}")
            }
        };
        Place {
            section: join(&self.section),
            cases: self.cases.clone(),
            read: format!("{}/{key}", self.read),
        }
    }

    /// One table of an array of tables in this place, such as one
    /// `[[case]]`, known by its own fields: a case's sticks, Arm switch and
    /// sensor readings, but not its Expectations.
    fn case(&self, key: &str, table: &toml::Table) -> Place {
        let mut fields = BTreeMap::new();
        flatten(table, "", &mut fields);
        let shown: Vec<String> = fields
            .iter()
            .map(|(name, value)| {
                format!(
                    "{name} {}",
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_string)
                )
            })
            .collect();
        let read: Vec<String> = fields
            .iter()
            .map(|(name, value)| match value.as_str() {
                Some(text) => format!("{name} {}", read_values(text)),
                None => format!("{name} {value}"),
            })
            .collect();
        let mut cases = self.cases.clone();
        cases.push(format!(
            "{} with {}",
            self.section(key).section,
            shown.join("; ")
        ));
        Place {
            section: self.section.clone(),
            cases,
            read: format!("{}/{key}[{}]", self.read, read.join("; ")),
        }
    }
}

/// Text with each of its comma-separated parts that is a number with its
/// unit, alone or after a word ("0%" or "roll -100 °/s"), read as a value in
/// SI units, so "0%" and "0.0 %", or "roll -100 °/s" and "roll -100.0°/s",
/// read the same. Anything else stays as written.
fn read_values(text: &str) -> String {
    let value = |part: &str| {
        parse_quantity(part)
            .ok()
            .map(|q| format!("{:e} {:?}", q.value, q.dimension()))
    };
    text.split(',')
        .map(|part| {
            let part = part.trim();
            value(part)
                .or_else(|| {
                    let (word, rest) = part.split_once(' ')?;
                    Some(format!("{word} {}", value(rest)?))
                })
                .unwrap_or_else(|| part.to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Debug, PartialEq)]
struct Expectation {
    place: Place,
    table: toml::Table,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Basis {
    Source,
    Rule,
    Observed,
    /// A basis the Scenario runner would refuse. Treated as locked.
    Unreadable,
}

impl Basis {
    fn word(self) -> &'static str {
        match self {
            Basis::Source => "Source",
            Basis::Rule => "Rule",
            Basis::Observed => "Observed",
            Basis::Unreadable => "unreadable",
        }
    }

    fn locked(self) -> bool {
        self != Basis::Observed
    }
}

impl Expectation {
    fn text(&self, key: &str) -> Option<&str> {
        self.table.get(key).and_then(toml::Value::as_str)
    }

    /// Which value key it uses, and the expected value written there.
    fn statistic(&self) -> Option<(&'static str, &str)> {
        VALUE_KEYS
            .iter()
            .find_map(|key| self.text(key).map(|value| (*key, value)))
    }

    /// What it measures and when, as the Results file words it: "height at
    /// 1 s", "roll rate, lowest over 0 s to 1 s", or, in a case, "motor 1
    /// DShot, in the case with arm on; pitch 0%; …".
    fn description(&self) -> String {
        let what = self.text("what").unwrap_or("(no what)");
        let section = if self.place.section.is_empty() {
            String::new()
        } else {
            format!("[{}] ", self.place.section)
        };
        let checks = match (self.text("at"), self.text("over"), self.statistic()) {
            (Some(at), _, _) => format!("{section}{what} at {at}"),
            (None, Some(over), Some((statistic, _))) => {
                format!("{section}{what}, {statistic} over {over}")
            }
            (None, Some(over), None) => format!("{section}{what} over {over}"),
            (None, None, _) => format!("{section}{what}"),
        };
        let cases: String = self
            .place
            .cases
            .iter()
            .map(|case| format!(", in the {case}"))
            .collect();
        format!("{checks}{cases}")
    }

    /// The expected value with its tolerance, as written.
    fn expected(&self) -> &str {
        self.statistic().map(|(_, value)| value).unwrap_or("(none)")
    }

    fn basis(&self) -> Basis {
        let text = self.text("basis").unwrap_or_default();
        match text
            .split_once(':')
            .map(|(kind, _)| kind.trim().to_lowercase())
        {
            Some(kind) if kind == "source" => Basis::Source,
            Some(kind) if kind == "rule" => Basis::Rule,
            Some(kind) if kind == "observed" => Basis::Observed,
            _ => Basis::Unreadable,
        }
    }

    /// The citation, working or reason after the basis's colon.
    fn basis_text(&self) -> &str {
        let text = self.text("basis").unwrap_or_default();
        text.split_once(':').map_or(text, |(_, rest)| rest).trim()
    }

    /// A moment or stretch as a value, so "1 s", "1.0 s" and "1000 ms" are
    /// the same moment. Text that isn't a time stays as written.
    fn when(&self, key: &str) -> Option<String> {
        let text = self.text(key)?;
        let moment = |part: &str| match parse_quantity(part) {
            Ok(q) if q.dimension() == Dimension::TIME => format!("{:e} s", q.value),
            _ => part.trim().to_string(),
        };
        Some(match text.split_once(" to ") {
            Some((from, to)) => format!("{} to {}", moment(from), moment(to)),
            None => moment(text),
        })
    }

    /// Two Expectations are the same check if they measure the same thing at
    /// the same moment or with the same statistic over the same stretch, in
    /// the same section and the same case.
    fn same_check(&self, other: &Expectation) -> bool {
        self.place.read == other.place.read
            && self.text("what") == other.text("what")
            && self.when("at") == other.when("at")
            && self.when("over") == other.when("over")
            && self.statistic().map(|(key, _)| key) == other.statistic().map(|(key, _)| key)
    }

    /// The same check with the same expected value, Basis and every other
    /// field, however its moment is spelled.
    fn same_as(&self, other: &Expectation) -> bool {
        self.same_check(other)
            && self
                .table
                .keys()
                .chain(other.table.keys())
                .filter(|key| key.as_str() != "at" && key.as_str() != "over")
                .all(|key| self.table.get(key) == other.table.get(key))
    }

    /// Whether this Expectation accepts a wider spread of values than `base`
    /// did: its tolerance was loosened.
    fn looser_than(&self, base: &Expectation) -> bool {
        match (width(self.expected()), width(base.expected())) {
            (Some((head, head_kind)), Some((base, base_kind))) => {
                head_kind == base_kind && head > base
            }
            _ => false,
        }
    }
}

/// How wide a spread of values an expected value accepts, in SI units, with
/// what it measures.
fn width(text: &str) -> Option<(f64, Dimension)> {
    let expected = parse_expected(text).ok()?;
    let kind = expected.dimension();
    let width = match &expected {
        Expected::Around { value, tolerance } => match tolerance {
            Tolerance::Amount(amount) => 2.0 * amount.value.abs(),
            Tolerance::Share(share) => 2.0 * (value.value * share.value).abs(),
        },
        Expected::Between { low, high } => high.value - low.value,
    };
    Some((width, kind))
}

/// The Test Quads a pull request adds, changes or deletes, by id, such as
/// `test/whoop-65-no-drag` for `scenarios/test-quads/whoop-65-no-drag.toml`.
fn changed_test_quads(changes: &Changes) -> BTreeSet<String> {
    changes
        .under(TEST_QUADS)
        .filter_map(|path| path.strip_prefix(TEST_QUADS)?.strip_suffix(".toml"))
        .map(|name| format!("test/{name}"))
        .collect()
}

/// Every Red Flag about Scenarios and their Expectations.
pub fn flags(changes: &Changes) -> Vec<RedFlag> {
    let mut flags = Vec::new();
    let test_quads = changed_test_quads(changes);
    let changed: Vec<&str> = changes
        .paths
        .iter()
        .map(String::as_str)
        .filter(|p| is_scenario(p))
        .collect();
    // Scenarios the pull request adds: where a deleted Scenario's
    // Expectations may have gone, whatever the new file or name.
    let added: Vec<(&str, ScenarioFile)> = changed
        .iter()
        .filter(|path| changes.base.read(path).is_none())
        .filter_map(|path| Some((*path, ScenarioFile::read(&changes.head.text(path)?)?)))
        .collect();
    for path in &changed {
        let Some(base) = changes.base.text(path).and_then(|t| ScenarioFile::read(&t)) else {
            continue;
        };
        match changes.head.text(path) {
            None => {
                let candidates: Vec<(&str, &ScenarioFile)> =
                    added.iter().map(|(p, f)| (*p, f)).collect();
                let all_found = compare(path, &base, &candidates, &mut flags);
                // Where its Source and Rule Expectations went, if it moved or
                // was split: every added file that holds one of them, each
                // with its own setup.
                for (now, head) in &candidates {
                    let holds_a_locked_one = base.expectations.iter().any(|before| {
                        before.basis().locked()
                            && head.expectations.iter().any(|e| e.same_check(before))
                    });
                    if holds_a_locked_one {
                        setup_changed(path, now, &base, head, &test_quads, &mut flags);
                    }
                }
                if !all_found {
                    let name = base
                        .name
                        .as_deref()
                        .map(|name| format!(" (\"{}\")", plain(name)))
                        .unwrap_or_default();
                    flags.push(RedFlag::new(
                        Level::ReviewerDecides,
                        "A Scenario deleted",
                        format!(
                            "{}{name}. It must be replaced, or the ticket must ask for it.",
                            code(path)
                        ),
                    ));
                }
            }
            Some(text) => match ScenarioFile::read(&text) {
                Some(head) => {
                    compare(path, &base, &[(path, &head)], &mut flags);
                    setup_changed(path, path, &base, &head, &test_quads, &mut flags);
                }
                None => {
                    let locked = base
                        .expectations
                        .iter()
                        .filter(|e| e.basis().locked())
                        .count();
                    if locked > 0 {
                        flags.push(RedFlag::new(
                            Level::WaitsForMaintainer,
                            "A Scenario with Source or Rule Expectations can't be read",
                            format!(
                                "{} isn't readable TOML any more, so its {locked} Source or Rule \
                                 Expectations can't be shown unchanged.",
                                code(path)
                            ),
                        ));
                    }
                }
            },
        }
    }
    // A changed Test Quad changes the flight of every Scenario that flies it,
    // even one the pull request leaves alone.
    if !test_quads.is_empty() {
        for path in changes.base.files_under(FOLDER) {
            if !is_scenario(&path) || changes.touches(&path) {
                continue;
            }
            if let Some(file) = changes
                .base
                .text(&path)
                .and_then(|t| ScenarioFile::read(&t))
            {
                setup_changed(&path, &path, &file, &file, &test_quads, &mut flags);
            }
        }
    }
    flags
}

/// A change to how a Scenario holding Source or Rule Expectations sets up its
/// flight: its starting state, its inputs, any other field but its
/// Expectations, or the Test Quad it flies (`test_quads` are the changed
/// ones). Those Expectations then check a different flight, so the Reviewer
/// decides. It doesn't wait for the maintainer, because the format migration
/// tool (#60) writes new starting-state items into every Scenario.
fn setup_changed(
    path: &str,
    now: &str,
    base: &ScenarioFile,
    head: &ScenarioFile,
    test_quads: &BTreeSet<String>,
    flags: &mut Vec<RedFlag>,
) {
    if !base.expectations.iter().any(|e| e.basis().locked()) {
        return;
    }
    let mut changed: Vec<String> = base
        .setup
        .keys()
        .chain(
            head.setup
                .keys()
                .filter(|key| !base.setup.contains_key(*key)),
        )
        .filter(|key| base.setup.get(*key) != head.setup.get(*key))
        .map(|key| code(key))
        .collect();
    // The same Test Quad by name, but its file changed.
    let quad = |file: &ScenarioFile| {
        file.setup
            .get(QUAD_FIELD)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    if let Some(id) = quad(head).filter(|id| quad(base).as_ref() == Some(id))
        && test_quads.contains(&id)
    {
        changed.push(format!("the Test Quad {}", code(&id)));
    }
    if changed.is_empty() {
        return;
    }
    let place = if now == path {
        code(path)
    } else {
        format!("{}, moved from {}", code(now), code(path))
    };
    let shown: Vec<String> = changed.iter().take(10).cloned().collect();
    let more = match changed.len().saturating_sub(10) {
        0 => String::new(),
        more => format!(" and {more} more"),
    };
    flags.push(RedFlag::new(
        Level::ReviewerDecides,
        "A Scenario's setup changed under its Source or Rule Expectations",
        format!(
            "{place}: {}{more} changed, so its Source and Rule Expectations now check a \
             different flight.",
            shown.join(", ")
        ),
    ));
}

/// Compares a base Scenario's Expectations with where they may be now: the
/// same file's new version, or, for a deleted file, every added Scenario.
/// Returns whether every one of them was found, changed or not.
fn compare(
    path: &str,
    base: &ScenarioFile,
    candidates: &[(&str, &ScenarioFile)],
    flags: &mut Vec<RedFlag>,
) -> bool {
    let mut all_found = true;
    let find = |test: &dyn Fn(&Expectation) -> bool| {
        candidates
            .iter()
            .find_map(|(p, file)| file.expectations.iter().find(|e| test(e)).map(|e| (*p, e)))
    };
    for before in &base.expectations {
        if find(&|e| e.same_as(before)).is_some() {
            continue;
        }
        let after = find(&|e| e.same_check(before));
        all_found &= after.is_some();
        let basis = before.basis();
        let what = plain(&before.description());
        // Where the Expectation is now, if it moved to another file.
        let place = |now: &str| {
            if now == path {
                format!("{}: \"{what}\"", code(path))
            } else {
                format!("{}, moved from {}: \"{what}\"", code(now), code(path))
            }
        };
        let removed = format!(
            "{}: \"{what}\" ({}) was removed, or now measures something else.",
            code(path),
            code(before.expected())
        );
        if basis.locked() {
            let detail = match after {
                Some((now, after)) => format!(
                    "{} {}.",
                    place(now),
                    differences(before, after).join(", and ")
                ),
                None => removed,
            };
            flags.push(RedFlag::new(
                Level::WaitsForMaintainer,
                if basis == Basis::Source {
                    "A Source Expectation changed"
                } else if basis == Basis::Rule {
                    "A Rule Expectation changed"
                } else {
                    "An Expectation with an unreadable Basis changed"
                },
                detail,
            ));
            continue;
        }
        match after {
            None => flags.push(RedFlag::new(
                Level::ReviewerDecides,
                "An Observed Expectation removed",
                removed,
            )),
            Some((now, after)) => {
                let reason = if after.basis_text() != before.basis_text() {
                    format!("Reason given: \"{}\"", plain(after.basis_text()))
                } else {
                    "Its basis line is unchanged, so it gives no new reason".to_string()
                };
                let (level, title) = if after.looser_than(before) {
                    (
                        Level::ReviewerDecides,
                        "A tolerance loosened on an Observed Expectation",
                    )
                } else {
                    (Level::ListedOnly, "An Observed Expectation updated")
                };
                flags.push(RedFlag::new(
                    level,
                    title,
                    format!(
                        "{} {}. {reason}.",
                        place(now),
                        differences(before, after).join(", and ")
                    ),
                ));
            }
        }
    }
    all_found
}

/// What changed in one Expectation, in plain words.
fn differences(before: &Expectation, after: &Expectation) -> Vec<String> {
    let mut differences = Vec::new();
    if before.expected() != after.expected() {
        let loosened = if after.looser_than(before) {
            ", a loosened tolerance"
        } else {
            ""
        };
        differences.push(format!(
            "went from {} to {}{loosened}",
            code(before.expected()),
            code(after.expected())
        ));
    }
    if before.basis() != after.basis() {
        let word = after.basis().word();
        let article = if word.starts_with(['O', 'u']) {
            "an"
        } else {
            "a"
        };
        differences.push(format!(
            "has {article} {word} Basis instead of {}",
            before.basis().word()
        ));
    } else if before.basis_text() != after.basis_text() {
        differences.push("has a new basis line".to_string());
    }
    let other = before
        .table
        .keys()
        .chain(after.table.keys())
        .filter(|key| {
            !VALUE_KEYS.contains(&key.as_str()) && !["basis", "at", "over"].contains(&key.as_str())
        })
        .any(|key| before.table.get(key) != after.table.get(key));
    if other {
        differences.push("has other fields changed".to_string());
    }
    if differences.is_empty() {
        differences.push("changed".to_string());
    }
    differences
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use opendrone_scenario::Repo;

    use super::ScenarioFile;

    /// The check the Scenario runner adds to the end of every Results file,
    /// which no Scenario file writes.
    const NO_BROKEN_NUMBERS: &str =
        "no broken numbers: every number in the state is a real number after every step";

    /// The Scenario runner writes every Expectation it reads into the Results
    /// file beside its Scenario, wherever the Scenario format lets it sit. So
    /// if the runner ever reads an Expectation somewhere the review tool
    /// doesn't look, the first Scenario that puts one there fails this.
    #[test]
    fn the_review_tool_reads_every_expectation_the_scenario_runner_measures() {
        let repo = Repo {
            root: Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
        };
        let files = repo.scenario_files().expect("can list the Scenarios");
        assert!(!files.is_empty(), "the repo has Scenarios");
        for file in files {
            let label = file.label();
            let text = fs::read_to_string(&file.path).expect("can read the Scenario");
            let read = ScenarioFile::read(&text)
                .unwrap_or_else(|| panic!("the review tool can read {label}"));
            let results: toml::Table = fs::read_to_string(file.results_path())
                .expect("every Scenario has its Results")
                .parse()
                .expect("a Results file is TOML");
            let mut measured: Vec<&str> = results
                .get("expect")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter(|e| e.get("what").and_then(toml::Value::as_str) != Some(NO_BROKEN_NUMBERS))
                .map(|e| {
                    e.get("basis")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("none")
                })
                .collect();
            let mut seen: Vec<&str> = read.expectations.iter().map(|e| e.basis().word()).collect();
            measured.sort_unstable();
            seen.sort_unstable();
            assert_eq!(
                seen, measured,
                "{label}: the review tool reads these Expectations' Bases (left), but the \
                 Scenario runner measured these (right)"
            );
        }
    }
}
