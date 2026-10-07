//! `cargo xtask scenario-catalogue`: one readable page listing every Scenario
//! under `scenarios/`, for the book's "Proving the physics" part (#15 §11).
//!
//! For each Scenario it shows its name, its kind, the Quad and the Map it
//! names, and each Expectation with its Basis. Scenarios are grouped by the
//! folder they sit in (`physics/`, `flight-controller/`, `quads/<quad>/`, …),
//! in file-name order.
//!
//! It reads the Scenario format the Scenario runner reads (#11, with the ids
//! from #16): the `name` line, the `kind`, `quad` and `map` lines of
//! `[start]`, and each `[[expect]]` with its `what` and `basis`, either `at` a
//! moment with its `value`, or `over` a stretch with its `mean`, `lowest`,
//! `highest` or `final`. A basis is written `"rule: …"`, `"source: …"` or
//! `"observed: …"`. Like the runner, it takes every `.toml` file under
//! `scenarios/` except Results files (`*.results.toml`) and the Test Quads in
//! `scenarios/test-quads/`. The page only shows Scenarios; the runner is what
//! checks them.
//!
//! The book builds this page whenever it's built, so it can never be out of
//! date. This command prints the same page, to read it without building the
//! book.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str =
    "Usage: cargo xtask scenario-catalogue [<folder of Scenarios, default scenarios>]";

pub fn run(args: &[String]) -> ExitCode {
    let folder = match args {
        [] => "scenarios",
        [folder] if !folder.starts_with('-') => folder.as_str(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match page(Path::new("."), folder) {
        Ok(page) => {
            print!("{page}");
            ExitCode::SUCCESS
        }
        Err(problems) => {
            eprintln!("Could not make the Scenario catalogue:");
            for problem in problems {
                eprintln!("- {problem}");
            }
            ExitCode::FAILURE
        }
    }
}

/// The catalogue of every Scenario under `folder` (relative to `root`, written
/// with `/`), as Markdown. Its links start with `/`, meaning the repo's root.
pub fn page(root: &Path, folder: &str) -> Result<String, Vec<String>> {
    let folder = folder.trim_end_matches('/');
    let mut files = Vec::new();
    collect_scenario_files(root, folder, folder, &mut files);
    files.sort();

    let mut problems = Vec::new();
    let mut groups: BTreeMap<String, Vec<Scenario>> = BTreeMap::new();
    for file in files {
        match Scenario::read(root, &file) {
            Ok(scenario) => {
                let group = crate::book::paths::folder(&file)
                    .strip_prefix(folder)
                    .unwrap_or("")
                    .trim_start_matches('/')
                    .to_owned();
                groups.entry(group).or_default().push(scenario);
            }
            Err(problem) => problems.push(problem),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(write_page(folder, &groups))
}

/// Every Scenario file under `folder`: `.toml` files, except Results files
/// and the Test Quads.
fn collect_scenario_files(root: &Path, top: &str, folder: &str, files: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root.join(folder)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{folder}/{name}");
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_folder {
            if path != format!("{top}/test-quads") {
                collect_scenario_files(root, top, &path, files);
            }
        } else if name.ends_with(".toml") && !name.ends_with(".results.toml") {
            files.push(path);
        }
    }
}

struct Scenario {
    file: String,
    name: String,
    kind: Option<String>,
    quad: Option<String>,
    map: Option<String>,
    results: Option<String>,
    expectations: Vec<Expectation>,
}

struct Expectation {
    what: String,
    when: String,
    expected: String,
    basis: Basis,
}

/// Where an Expectation's number comes from.
struct Basis {
    /// Source, Rule or Observed; `None` when the text names none of them.
    kind: Option<&'static str>,
    text: String,
}

const BASIS_KINDS: [&str; 3] = ["Source", "Rule", "Observed"];

/// What an Expectation over a stretch of time measures there.
const STATISTICS: [&str; 4] = ["mean", "lowest", "highest", "final"];

impl Scenario {
    fn read(root: &Path, file: &str) -> Result<Scenario, String> {
        let text = fs::read_to_string(root.join(file))
            .map_err(|error| format!("`{file}` can't be read: {error}"))?;
        let table = text.parse::<toml::Table>().map_err(|error| {
            format!(
                "`{file}` isn't valid TOML: {}",
                one_line(&error.to_string())
            )
        })?;
        let start = table.get("start").and_then(toml::Value::as_table);
        let line = |key: &str| {
            start
                .and_then(|start| start.get(key))
                .or_else(|| table.get(key))
                .map(shown)
        };
        let stem = file
            .rsplit('/')
            .next()
            .unwrap_or(file)
            .trim_end_matches(".toml");
        let results = format!("{}/{stem}.results.toml", crate::book::paths::folder(file));
        let expectations = match table.get("expect") {
            None => Vec::new(),
            Some(toml::Value::Array(list)) => list
                .iter()
                .map(|item| match item.as_table() {
                    Some(expectation) => Ok(Expectation::read(expectation)),
                    None => Err(format!("`{file}`: each `[[expect]]` must be a table")),
                })
                .collect::<Result<_, _>>()?,
            Some(_) => {
                return Err(format!(
                    "`{file}`: `expect` must be a list of `[[expect]]` tables"
                ));
            }
        };
        Ok(Scenario {
            file: file.to_owned(),
            name: table.get("name").map_or_else(|| stem.to_owned(), shown),
            kind: line("kind"),
            quad: line("quad"),
            map: line("map"),
            results: root.join(&results).is_file().then_some(results),
            expectations,
        })
    }

    /// "Physics Scenario", from `kind = "physics"`.
    fn kind_name(&self) -> String {
        let Some(kind) = &self.kind else {
            return "Scenario of no stated kind".to_owned();
        };
        let words = kind.to_lowercase().replace(['-', '_'], " ");
        match words.trim() {
            "flight" => "Flight Scenario".to_owned(),
            "thrust stand" => "Thrust Stand Scenario".to_owned(),
            "flight controller" => "Flight Controller Scenario".to_owned(),
            "physics" => "Physics Scenario".to_owned(),
            _ => format!("Scenario of kind \"{kind}\""),
        }
    }
}

impl Expectation {
    fn read(table: &toml::Table) -> Expectation {
        let text = |key: &str| table.get(key).map(shown);
        let statistic = STATISTICS
            .into_iter()
            .find(|word| table.contains_key(*word));
        let (when, expected) = match (text("at"), text("over"), statistic) {
            (Some(at), _, _) => (format!("at {at}"), text("value")),
            (None, Some(over), Some(statistic)) => {
                (format!("{statistic} over {over}"), text(statistic))
            }
            (None, Some(over), None) => (format!("over {over}"), None),
            (None, None, _) => (String::new(), text("value")),
        };
        Expectation {
            what: text("what").unwrap_or_default(),
            when,
            expected: expected.unwrap_or_default(),
            basis: Basis::read(table.get("basis")),
        }
    }
}

impl Basis {
    /// Reads `basis = "rule: …"`, `"source: …"` or `"observed: …"`.
    fn read(value: Option<&toml::Value>) -> Basis {
        let text = value.map(shown).unwrap_or_default();
        if let Some((kind, rest)) = text.split_once(':')
            && let Some(kind) = BASIS_KINDS
                .into_iter()
                .find(|k| k.eq_ignore_ascii_case(kind.trim()))
        {
            return Basis {
                kind: Some(kind),
                text: rest.trim().to_owned(),
            };
        }
        Basis {
            kind: None,
            text: text.trim().to_owned(),
        }
    }

    fn shown(&self) -> String {
        match (self.kind, self.text.is_empty()) {
            (None, true) => "**No Basis given**".to_owned(),
            (None, false) => format!(
                "**No Source, Rule or Observed Basis:** {}",
                cell(&self.text)
            ),
            (Some(kind), true) => format!("**{kind}**"),
            (Some(kind), false) => format!("**{kind}:** {}", cell(&self.text)),
        }
    }
}

fn write_page(folder: &str, groups: &BTreeMap<String, Vec<Scenario>>) -> String {
    let scenarios: Vec<&Scenario> = groups.values().flatten().collect();
    let expectations: Vec<&Expectation> = scenarios.iter().flat_map(|s| &s.expectations).collect();

    let mut page = String::new();
    if scenarios.is_empty() {
        let _ = writeln!(
            page,
            "There are no Scenarios yet. Each one will appear here once it's in `{folder}/`."
        );
        return page;
    }

    let mut counts = Vec::new();
    for kind in BASIS_KINDS {
        let count = expectations
            .iter()
            .filter(|e| e.basis.kind == Some(kind))
            .count();
        counts.push(format!("{count} {kind}"));
    }
    let unnamed = expectations
        .iter()
        .filter(|e| e.basis.kind.is_none())
        .count();
    if unnamed > 0 {
        counts.push(format!("{unnamed} with no Source, Rule or Observed Basis"));
    }
    let _ = writeln!(
        page,
        "{} with {}: {}.",
        plural(scenarios.len(), "Scenario", "Scenarios"),
        plural(expectations.len(), "Expectation", "Expectations"),
        counts.join(", ")
    );

    for (group, scenarios) in groups {
        let heading = if group.is_empty() {
            format!("{folder}/")
        } else {
            format!("{folder}/{group}/")
        };
        let _ = write!(page, "\n## `{heading}`\n");
        for scenario in scenarios {
            write_scenario(&mut page, scenario);
        }
    }
    page
}

fn write_scenario(page: &mut String, scenario: &Scenario) {
    let _ = write!(page, "\n### {}\n\n", inline(&scenario.name));
    let mut facts = vec![scenario.kind_name()];
    facts.push(match &scenario.quad {
        Some(quad) => format!("Quad `{quad}`"),
        None => "no Quad named".to_owned(),
    });
    facts.push(match &scenario.map {
        Some(map) => format!("Map `{map}`"),
        None => "no Map named".to_owned(),
    });
    let mut files = format!("[`{}`](/{})", scenario.file, scenario.file);
    if let Some(results) = &scenario.results {
        let _ = write!(files, " and its [Results](/{results})");
    }
    facts.push(files);
    let _ = writeln!(page, "{}.", facts.join(" · "));

    if scenario.expectations.is_empty() {
        let _ = writeln!(page, "\nIt has no Expectations.");
        return;
    }
    let _ = writeln!(page, "\n| Expectation | When | Expected | Basis |");
    let _ = writeln!(page, "|---|---|---|---|");
    for expectation in &scenario.expectations {
        let _ = writeln!(
            page,
            "| {} | {} | {} | {} |",
            cell(&expectation.what),
            cell(&expectation.when),
            cell(&expectation.expected),
            expectation.basis.shown()
        );
    }
}

/// A value from a Scenario as plain text: strings without their quotes.
fn shown(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Text that is safe inside a table cell: one line, with `|` and `<` escaped.
fn cell(text: &str) -> String {
    inline(&one_line(text)).replace('|', "\\|")
}

/// Text that can't start HTML by accident.
fn inline(text: &str) -> String {
    text.replace('<', "&lt;")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}
