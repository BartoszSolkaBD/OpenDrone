//! The Expectations in Scenario files, compared between the base and the head
//! (#11 §3, #15 §5).
//!
//! Source and Rule Expectations are locked: changing one in any way, removing
//! it, or loosening its tolerance waits for the maintainer. Observed ones may
//! be updated with a one-line reason; a loosened tolerance on one, or removing
//! one, is for the Reviewer to decide. A deleted Scenario is for the Reviewer
//! too. An Expectation is matched by what it measures and when (with its
//! moment read as a time, so "1 s" and "1.0 s" match), so moving it within the
//! file changes nothing. A deleted Scenario's Expectations are looked for in
//! every Scenario the pull request adds, whatever its file or name: one found
//! unchanged has only moved, and a Source or Rule one found nowhere unchanged
//! waits for the maintainer.

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

/// The keys that hold an Expectation's expected value, one per kind: at a
/// moment, or a statistic over a stretch.
const VALUE_KEYS: [&str; 5] = ["value", "mean", "lowest", "highest", "final"];

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
}

impl ScenarioFile {
    fn read(text: &str) -> Option<ScenarioFile> {
        let table: toml::Table = text.parse().ok()?;
        let mut expectations = Vec::new();
        collect(&table, "", &mut expectations);
        Some(ScenarioFile {
            name: table
                .get("name")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            expectations,
        })
    }
}

/// Every `[[expect]]` in the file, including ones inside a section such as
/// `[osd]`.
fn collect(table: &toml::Table, section: &str, into: &mut Vec<Expectation>) {
    for (key, value) in table {
        if key == "expect" {
            for entry in value.as_array().into_iter().flatten() {
                if let Some(entry) = entry.as_table() {
                    into.push(Expectation {
                        section: section.to_string(),
                        table: entry.clone(),
                    });
                }
            }
        } else if let Some(inner) = value.as_table() {
            let section = if section.is_empty() {
                key.clone()
            } else {
                format!("{section}.{key}")
            };
            collect(inner, &section, into);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Expectation {
    /// The section it sits in, such as `osd`, or nothing for the top.
    section: String,
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
    /// 1 s" or "roll rate, lowest over 0 s to 1 s".
    fn description(&self) -> String {
        let what = self.text("what").unwrap_or("(no what)");
        let section = if self.section.is_empty() {
            String::new()
        } else {
            format!("[{}] ", self.section)
        };
        match (self.text("at"), self.text("over"), self.statistic()) {
            (Some(at), _, _) => format!("{section}{what} at {at}"),
            (None, Some(over), Some((statistic, _))) => {
                format!("{section}{what}, {statistic} over {over}")
            }
            (None, Some(over), None) => format!("{section}{what} over {over}"),
            (None, None, _) => format!("{section}{what}"),
        }
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
    /// the same moment or with the same statistic over the same stretch.
    fn same_check(&self, other: &Expectation) -> bool {
        self.section == other.section
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

/// Every Red Flag about Scenarios and their Expectations.
pub fn flags(changes: &Changes) -> Vec<RedFlag> {
    let mut flags = Vec::new();
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
    flags
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
