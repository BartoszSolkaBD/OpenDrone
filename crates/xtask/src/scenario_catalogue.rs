//! `cargo xtask scenario-catalogue`: one readable page listing every Scenario
//! under `scenarios/`, for the book's "Proving the physics" part (#15 §11).
//!
//! For each Scenario it shows its name, its kind, the Quad and the Map it
//! names, and each Expectation with its Basis. Scenarios are grouped by the
//! folder they sit in (`physics/`, `quads/<quad>/`, …), in file-name order.
//!
//! It finds and reads the Scenarios with the Scenario runner's own code
//! ([`opendrone_scenario`]), so it lists exactly the files the runner runs
//! (Results files and the Test Quads in `scenarios/test-quads/` aren't
//! Scenarios), read exactly as the runner reads them. A Scenario the runner
//! can't read stops the page, with the runner's own file, line and sentence.
//!
//! The book builds this page whenever it's built, so it can never be out of
//! date. This command prints the same page, to read it without building the
//! book.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use opendrone_scenario::{BasisKind, Kind, RESULTS_ENDING, Repo, read_scenario};

const USAGE: &str = "Usage: cargo xtask scenario-catalogue [<scenarios folder, default scenarios>]";

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

/// The catalogue of every Scenario in `folder`, a folder named `scenarios`
/// (relative to `root`, written with `/`), as Markdown. Its links start with
/// `/`, meaning `root`.
pub fn page(root: &Path, folder: &str) -> Result<String, Vec<String>> {
    let folder = folder.trim_end_matches('/');
    let (parent, name) = folder.rsplit_once('/').unwrap_or(("", folder));
    if name != "scenarios" {
        return Err(vec![format!(
            "the Scenario runner reads Scenarios from a folder named `scenarios`, not `{folder}`"
        )]);
    }
    let repo = Repo {
        root: root.join(parent),
    };
    // Links and headings name files from `root`.
    let from_root = |label: &str| {
        if parent.is_empty() {
            label.to_owned()
        } else {
            format!("{parent}/{label}")
        }
    };
    if !repo.scenarios_folder().is_dir() {
        return Ok(write_page(folder, &BTreeMap::new()));
    }
    let files = repo
        .scenario_files()
        .map_err(|error| vec![format!("`{folder}/` can't be read: {error}")])?;

    let mut problems = Vec::new();
    let mut groups: BTreeMap<String, Vec<Scenario>> = BTreeMap::new();
    for file in files {
        let label = file.label();
        let text = match fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(error) => {
                problems.push(format!("`{}` can't be read: {error}", from_root(&label)));
                continue;
            }
        };
        let read = match read_scenario(&from_root(&label), &text) {
            Ok(read) => read,
            Err(found) => {
                problems.extend(found.0.iter().map(ToString::to_string));
                continue;
            }
        };
        let results = file.results_path().is_file().then(|| {
            from_root(&format!(
                "{}{RESULTS_ENDING}",
                label.trim_end_matches(".toml")
            ))
        });
        let group = file
            .relative
            .rsplit_once('/')
            .map_or("", |(group, _)| group)
            .to_owned();
        groups.entry(group).or_default().push(Scenario::from_read(
            read,
            from_root(&label),
            results,
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(write_page(folder, &groups))
}

/// What the page shows of one Scenario.
struct Scenario {
    file: String,
    name: String,
    kind: Kind,
    quad: String,
    map: String,
    results: Option<String>,
    expectations: Vec<Expectation>,
}

struct Expectation {
    what: String,
    when: String,
    expected: String,
    basis: BasisKind,
    working: String,
}

const BASIS_KINDS: [BasisKind; 3] = [BasisKind::Source, BasisKind::Rule, BasisKind::Observed];

impl Scenario {
    fn from_read(
        read: opendrone_scenario::Scenario,
        file: String,
        results: Option<String>,
    ) -> Scenario {
        let expectations = read
            .expectations
            .iter()
            .map(|expectation| {
                let what = expectation.measure.name();
                // The runner describes it as "vertical speed at 1 s", or
                // "vertical acceleration, lowest over 0 s to 1 s".
                let when = expectation
                    .description
                    .strip_prefix(what)
                    .unwrap_or(&expectation.description)
                    .trim_start_matches(',')
                    .trim();
                Expectation {
                    what: what.to_owned(),
                    when: when.to_owned(),
                    expected: expectation.expected.text(),
                    basis: expectation.basis.kind,
                    working: expectation.basis.text.clone(),
                }
            })
            .collect();
        Scenario {
            file,
            name: read.name,
            kind: read.start.kind,
            quad: read.start.quad.id,
            map: read.start.map.map_or_else(
                || "none (the Flight Controller alone)".to_string(),
                |map| map.id,
            ),
            results,
            expectations,
        }
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Flight => "Flight Scenario",
        Kind::ThrustStand => "Thrust Stand Scenario",
        Kind::FlightController => "Flight Controller Scenario",
        Kind::Physics => "Physics Scenario",
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

    let counts: Vec<String> = BASIS_KINDS
        .iter()
        .map(|kind| {
            let count = expectations.iter().filter(|e| e.basis == *kind).count();
            format!("{count} {}", kind.word())
        })
        .collect();
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
    let mut files = format!("[`{}`](/{})", scenario.file, scenario.file);
    if let Some(results) = &scenario.results {
        let _ = write!(files, " and its [Results](/{results})");
    }
    let _ = writeln!(
        page,
        "{} · Quad `{}` · Map `{}` · {files}.",
        kind_name(scenario.kind),
        scenario.quad,
        scenario.map
    );

    let _ = writeln!(page, "\n| Expectation | When | Expected | Basis |");
    let _ = writeln!(page, "|---|---|---|---|");
    for expectation in &scenario.expectations {
        let _ = writeln!(
            page,
            "| {} | {} | {} | **{}:** {} |",
            cell(&expectation.what),
            cell(&expectation.when),
            cell(&expectation.expected),
            expectation.basis.word(),
            cell(&expectation.working)
        );
    }
}

/// Text that is safe inside a table cell: one line, with `|` and `<` escaped.
fn cell(text: &str) -> String {
    inline(&text.split_whitespace().collect::<Vec<_>>().join(" ")).replace('|', "\\|")
}

/// Text that can't start HTML by accident.
fn inline(text: &str) -> String {
    text.replace('<', "&lt;")
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}
