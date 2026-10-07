//! Readable checks for the Scenario catalogue, the page of the book that lists
//! every Scenario under `scenarios/` (#15 §11), through
//! `cargo xtask scenario-catalogue`, which prints the same page the book
//! shows.
//!
//! Each check makes a scratch repo with the two fixture Scenarios in
//! `tests/fixtures/scenarios/` (in the format the Scenario runner reads) and
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

    catalogue.says("### A slow yaw keeps its rate");
    catalogue.says("Physics Scenario · Quad `opendrone/whoop-65` · Map `test/empty-air`");
    catalogue.says(
        "| yaw rate | at 0.5 s | 90 °/s ± 3% | **Source:** a made-up reference, for this fixture \
         only |",
    );
    catalogue.says(
        "| heading | final over 0 s to 0.5 s | between 40° and 50° | **Observed:** what a \
         made-up run did, for this fixture only |",
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
    let physics = catalogue.position("## `scenarios/physics/`");
    let free_fall = catalogue.position("### Free fall is exactly g");
    let whoop = catalogue.position("## `scenarios/quads/whoop-65/`");
    let slow_yaw = catalogue.position("### A slow yaw keeps its rate");
    assert!(
        physics < free_fall && free_fall < whoop && whoop < slow_yaw,
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
fn a_scenario_the_runner_cant_read_stops_the_catalogue_with_the_runners_own_words() {
    let repo = Repo::with_fixture_scenarios("catalogue-unreadable-scenario");
    let fixture = fs::read_to_string(repo.root.join("scenarios/physics/free-fall.toml"))
        .expect("read the fixture");
    repo.write(
        "scenarios/physics/no-basis.toml",
        &fixture.replace(
            "basis = \"rule: speed = g × t = 9.81 m/s² × 1 s, downward\"",
            "basis = \"it looked right\"",
        ),
    );
    let problem = repo.catalogue().expect_err("a Scenario the runner refuses");
    assert!(
        problem.contains("scenarios/physics/no-basis.toml"),
        "{problem}"
    );
    assert!(
        problem.contains("a basis starts with \"source:\""),
        "{problem}"
    );
}

#[test]
fn a_scenario_that_isnt_valid_toml_stops_the_catalogue_and_names_the_file() {
    let repo = Repo::with_fixture_scenarios("catalogue-broken-scenario");
    repo.write("scenarios/physics/broken.toml", "name = \"Broken\n");
    let problem = repo.catalogue().expect_err("a broken Scenario");
    assert!(
        problem.contains("scenarios/physics/broken.toml"),
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
        for file in ["physics/free-fall.toml", "quads/whoop-65/slow-yaw.toml"] {
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
