//! Readable checks for the Scenario catalogue, the page of the book that lists
//! every Scenario under `scenarios/` (#15 §11), through
//! `cargo xtask scenario-catalogue`, which prints the same page the book
//! shows.
//!
//! Each check makes a scratch repo with the two fixture Scenarios in
//! `tests/fixtures/scenarios/` (written in the format the Scenario runner
//! reads) and
//! whatever else it needs, and reads the catalogue it gets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn the_catalogue_shows_each_scenarios_name_kind_quad_map_and_expectations_with_their_basis() {
    let repo = Repo::with_fixture_scenarios("catalogue-shows-each-scenario");
    let catalogue = repo.catalogue().expect("the catalogue");

    catalogue.says("### Free fall is exactly g");
    catalogue.says(
        "Physics Scenario · Quad `test/whoop-65-no-drag` · Map `test/empty-air` · \
         [`scenarios/physics/free-fall.toml`](/scenarios/physics/free-fall.toml).",
    );
    catalogue.says("| Expectation | When | Expected | Basis |");
    catalogue.says(
        "| vertical acceleration | lowest over 0 s to 1 s | -9.81 m/s² ± 0.00001 m/s² | \
         **Rule:** with no drag and no thrust, gravity is the only force, so the acceleration is \
         the Map's gravity |",
    );
    catalogue.says(
        "| vertical speed | at 1 s | -9.81 m/s ± 0.00001 m/s | **Rule:** speed = g × t = \
         9.81 m/s² × 1 s, downward |",
    );

    catalogue.says("### Full right roll reaches the max rate");
    catalogue.says("Flight Scenario · Quad `opendrone/whoop-65` · Map `test/empty-air`");
    catalogue.says(
        "| roll rate | at 1.25 s | 670 °/s ± 3% | **Source:** Betaflight Actual rates, full \
         stick gives the max rate |",
    );
    catalogue.says(
        "| roll | mean over 1.0 s to 1.3 s | between 80° and 110° | **Observed:** what the sim \
         did when this was written |",
    );
}

#[test]
fn the_catalogue_counts_the_scenarios_and_each_kind_of_basis() {
    let repo = Repo::with_fixture_scenarios("catalogue-counts");
    let catalogue = repo.catalogue().expect("the catalogue");
    catalogue.says("2 Scenarios with 4 Expectations: 1 Source, 2 Rule, 1 Observed.");
}

#[test]
fn the_catalogue_groups_scenarios_by_their_folder() {
    let repo = Repo::with_fixture_scenarios("catalogue-groups");
    let catalogue = repo.catalogue().expect("the catalogue");
    let flight_controller = catalogue.position("## `scenarios/flight-controller/`");
    let full_roll = catalogue.position("### Full right roll reaches the max rate");
    let physics = catalogue.position("## `scenarios/physics/`");
    let free_fall = catalogue.position("### Free fall is exactly g");
    assert!(
        flight_controller < full_roll && full_roll < physics && physics < free_fall,
        "{}",
        catalogue.0
    );
}

#[test]
fn the_catalogue_links_each_scenario_to_its_results_when_they_are_there() {
    let repo = Repo::with_fixture_scenarios("catalogue-links-results");
    repo.write("scenarios/physics/free-fall.results.toml", "# Results\n");
    let catalogue = repo.catalogue().expect("the catalogue");
    catalogue.says(
        "[`scenarios/physics/free-fall.toml`](/scenarios/physics/free-fall.toml) and its \
         [Results](/scenarios/physics/free-fall.results.toml).",
    );
}

#[test]
fn results_files_and_test_quads_are_not_listed_as_scenarios() {
    let repo = Repo::with_fixture_scenarios("catalogue-skips-results-and-test-quads");
    repo.write(
        "scenarios/physics/free-fall.results.toml",
        "name = \"not a Scenario\"\n",
    );
    repo.write(
        "scenarios/test-quads/whoop-65-no-drag.toml",
        "format = 1\nbased_on = \"opendrone/whoop-65\"\nwhy = \"drag off\"\n",
    );
    let catalogue = repo.catalogue().expect("the catalogue");
    catalogue.says("2 Scenarios with 4 Expectations");
    catalogue.never_says("not a Scenario");
    catalogue.never_says("test-quads");
}

#[test]
fn an_expectation_without_a_source_rule_or_observed_basis_is_shown_as_having_none() {
    let repo = Repo::new("catalogue-no-basis");
    repo.write(
        "scenarios/physics/hover.toml",
        r#"name = "Hover holds"
[start]
kind = "physics"
quad = "opendrone/whoop-65"
map  = "test/empty-air"

[[expect]]
what  = "height"
at    = "1 s"
value = "2 m ± 0.1 m"

[[expect]]
what  = "height"
at    = "2 s"
value = "2 m ± 0.1 m"
basis = "it looked right"
"#,
    );
    let catalogue = repo.catalogue().expect("the catalogue");
    catalogue.says("| height | at 1 s | 2 m ± 0.1 m | **No Basis given** |");
    catalogue.says(
        "| height | at 2 s | 2 m ± 0.1 m | **No Source, Rule or Observed Basis:** it looked right |",
    );
    catalogue.says(
        "1 Scenario with 2 Expectations: 0 Source, 0 Rule, 0 Observed, 2 with no Source, Rule or \
         Observed Basis.",
    );
}

#[test]
fn a_scenario_that_isnt_valid_toml_stops_the_catalogue_and_names_the_file() {
    let repo = Repo::with_fixture_scenarios("catalogue-broken-scenario");
    repo.write("scenarios/physics/broken.toml", "name = \"Broken\n");
    let problem = repo.catalogue().expect_err("a broken Scenario");
    assert!(
        problem.contains("`scenarios/physics/broken.toml` isn't valid TOML"),
        "{problem}"
    );
}

#[test]
fn with_no_scenarios_the_catalogue_says_so() {
    let repo = Repo::new("catalogue-empty");
    let catalogue = repo.catalogue().expect("the catalogue");
    catalogue
        .says("There are no Scenarios yet. Each one will appear here once it's in `scenarios/`.");
}

/// A scratch repo for one check.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("scenario-catalogue")
            .join(name);
        if root.exists() {
            fs::remove_dir_all(&root).expect("clear the scratch repo");
        }
        fs::create_dir_all(&root).expect("make the scratch repo");
        Repo { root }
    }

    /// A scratch repo whose `scenarios/` holds the fixture Scenarios.
    fn with_fixture_scenarios(name: &str) -> Repo {
        let repo = Repo::new(name);
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scenarios");
        for file in ["physics/free-fall.toml", "flight-controller/full-roll.toml"] {
            let text = fs::read_to_string(fixtures.join(file)).expect("read a fixture Scenario");
            repo.write(&format!("scenarios/{file}"), &text);
        }
        repo
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().expect("a folder")).expect("make the folder");
        fs::write(path, text).expect("write the file");
    }

    /// The catalogue `cargo xtask scenario-catalogue` prints, or what it says
    /// is wrong.
    fn catalogue(&self) -> Result<Page, String> {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["scenario-catalogue", "scenarios"])
            .current_dir(&self.root)
            .output()
            .expect("run xtask");
        if output.status.success() {
            Ok(Page(String::from_utf8_lossy(&output.stdout).into_owned()))
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

#[derive(Debug)]
struct Page(String);

impl Page {
    fn says(&self, text: &str) {
        assert!(self.0.contains(text), "expected {text:?} in:\n{}", self.0);
    }

    fn never_says(&self, text: &str) {
        assert!(
            !self.0.contains(text),
            "didn't expect {text:?} in:\n{}",
            self.0
        );
    }

    fn position(&self, text: &str) -> usize {
        self.0
            .find(text)
            .unwrap_or_else(|| panic!("expected {text:?} in:\n{}", self.0))
    }
}
