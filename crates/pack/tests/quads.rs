//! Readable checks for reading Quad definitions, Test Quads, manifests and the
//! built-in Test Maps. Each broken case writes a small fixture Pack and reads
//! it the way the Scenario runner does.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::{Packs, Problems, read_manifest};

const MANIFEST: &str = r#"format = 1
id = "fixture"
name = "Fixture"
description = "A Pack made by a readable check."
version = "1"
author = "OpenDrone contributors"
licence = "CC0-1.0"
"#;

/// A Quad definition with the numbers the Simulation needs; `{dry_mass}` is
/// on line 5.
const QUAD: &str = r#"format = 1
name = "Fixture Quad"

[frame]
dry_mass  = { value = "{dry_mass}", confidence = "Manufacturer", source = "maker" }
inertia   = { value = "roll 70, pitch 90, yaw 140 g·cm²", confidence = "Estimate", range = "×0.5–×2", source = "guess" }
drag_area = { value = "front 9, side 9, top 25 cm²", confidence = "Estimate", range = "×0.5–×2", source = "guess" }

[props]
rotor_drag = { value = "0.3 s⁻¹", confidence = "Estimate", range = "0.1–0.6 s⁻¹", source = "guess" }

[battery]
mass = { value = "8.2 g", confidence = "Manufacturer", source = "maker" }

[sources]
maker = "the maker's page"
guess = "a guess"
"#;

/// A folder of fixture files, with a `packs/` folder and a Test Quad folder.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(case: &str) -> Fixture {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("pack-fixtures")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Fixture { root }
    }

    fn with_quad(case: &str, quad: &str) -> Fixture {
        let fixture = Fixture::new(case);
        fixture.write("packs/fixture/pack.toml", MANIFEST);
        fixture.write("packs/fixture/quads/quad/quad.toml", quad);
        fixture
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn packs(&self) -> Result<Packs, Problems> {
        Ok(Packs::open(&self.root.join("packs"), "packs")?
            .with_test_quads(&self.root.join("test-quads"), "scenarios/test-quads"))
    }

    fn problems(&self, id: &str) -> Vec<String> {
        match self.packs().and_then(|packs| packs.quad(id)) {
            Ok(quad) => panic!("{id} should be refused, but read as {quad:?}"),
            Err(problems) => problems.0.iter().map(ToString::to_string).collect(),
        }
    }
}

fn quad_text(dry_mass: &str) -> String {
    QUAD.replace("{dry_mass}", dry_mass)
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn built_in() -> Packs {
    Packs::open(&repo().join("packs"), "packs")
        .unwrap_or_else(|p| panic!("the built-in Pack should read:\n{p}"))
        .with_test_quads(&repo().join("scenarios/test-quads"), "scenarios/test-quads")
}

#[test]
fn the_whoop_65_weighs_its_dry_mass_plus_its_battery() {
    let whoop = built_in().quad("opendrone/whoop-65").unwrap();
    assert_eq!(whoop.name, "Whoop 65");
    assert!((whoop.parameters.mass - 0.0312).abs() < 1e-15);
    let inertia = whoop.parameters.inertia.numbers();
    assert!(
        (inertia[0] - 7e-6).abs() < 1e-18,
        "roll inertia {}",
        inertia[0]
    );
    assert!(
        (inertia[4] - 9e-6).abs() < 1e-18,
        "pitch inertia {}",
        inertia[4]
    );
    assert!(
        (inertia[8] - 1.4e-5).abs() < 1e-18,
        "yaw inertia {}",
        inertia[8]
    );
    assert!(whoop.parameters.drag.duct_ram > 0.0);
}

#[test]
fn a_test_quad_changes_only_what_it_lists() {
    let packs = built_in();
    let whoop = packs.quad("opendrone/whoop-65").unwrap();
    let no_drag = packs.quad("test/whoop-65-no-drag").unwrap();
    assert_eq!(no_drag.based_on.as_deref(), Some("opendrone/whoop-65"));
    assert_eq!(no_drag.parameters.mass, whoop.parameters.mass);
    assert_eq!(no_drag.parameters.inertia, whoop.parameters.inertia);
    let drag = no_drag.parameters.drag;
    assert_eq!(
        (drag.body_area.x, drag.body_area.y, drag.body_area.z),
        (0.0, 0.0, 0.0)
    );
    assert_eq!((drag.rotor, drag.duct_ram), (0.0, 0.0));
    assert_ne!(no_drag.fingerprint(), whoop.fingerprint());
}

#[test]
fn a_change_to_the_real_quad_carries_into_its_test_quads() {
    let fixture = Fixture::with_quad("carries", &quad_text("40 g"));
    fixture.write(
        "test-quads/no-rotor-drag.toml",
        "format = 1\nbased_on = \"fixture/quad\"\nwhy = \"a check\"\n[props]\nrotor_drag = \"0 s⁻¹\"\n",
    );
    let test_quad = fixture.packs().unwrap().quad("test/no-rotor-drag").unwrap();
    assert!((test_quad.parameters.mass - 0.0482).abs() < 1e-15);
}

#[test]
fn a_test_quad_changing_a_setting_its_quad_lacks_is_refused() {
    let fixture = Fixture::with_quad("lacks", &quad_text("23.0 g"));
    fixture.write(
        "test-quads/typo.toml",
        "format = 1\nbased_on = \"fixture/quad\"\nwhy = \"a check\"\n[frame]\ndrag_aera = \"front 0, side 0, top 0 cm²\"\n",
    );
    assert_eq!(
        fixture.problems("test/typo"),
        [
            "scenarios/test-quads/typo.toml line 5: [frame] drag_aera isn't a setting of fixture/quad, so it can't be changed here"
        ]
    );
}

#[test]
fn a_test_quads_change_carries_no_confidence() {
    let fixture = Fixture::with_quad("no-confidence", &quad_text("23.0 g"));
    fixture.write(
        "test-quads/confident.toml",
        "format = 1\nbased_on = \"fixture/quad\"\nwhy = \"a check\"\n[props]\nrotor_drag = { value = \"0 s⁻¹\", confidence = \"Measured\", source = \"maker\" }\n",
    );
    let problems = fixture.problems("test/confident");
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].starts_with("scenarios/test-quads/confident.toml line 5: a Test Quad's change carries no Confidence or source"),
        "{problems:?}"
    );
}

#[test]
fn a_test_quad_builds_on_a_real_quad_only() {
    let fixture = Fixture::with_quad("on-a-test-quad", &quad_text("23.0 g"));
    fixture.write(
        "test-quads/twice.toml",
        "format = 1\nbased_on = \"test/other\"\nwhy = \"a check\"\n",
    );
    assert_eq!(
        fixture.problems("test/twice"),
        [
            "scenarios/test-quads/twice.toml line 2: a Test Quad builds on a real Quad, not on another Test Quad (\"test/other\")"
        ]
    );
}

#[test]
fn a_decimal_comma_in_a_quad_definition_is_refused_with_its_file_and_line() {
    let fixture = Fixture::with_quad("decimal-comma", &quad_text("31,2 g"));
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 5: \"31,2 g\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"31.2 g\""
        ]
    );
}

#[test]
fn a_number_in_the_wrong_kind_of_unit_is_refused() {
    let fixture = Fixture::with_quad("wrong-unit", &quad_text("23 m"));
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 5: \"23 m\" is a length, but this needs a mass, such as \"23.0 g\""
        ]
    );
}

#[test]
fn an_estimate_without_a_range_is_refused() {
    let quad = quad_text("23.0 g").replace(", range = \"0.1–0.6 s⁻¹\"", "");
    let fixture = Fixture::with_quad("no-range", &quad);
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 10: `props.rotor_drag` is an Estimate, so it needs the `range` it may move within, such as range = \"×0.5–×2\""
        ]
    );
}

#[test]
fn an_estimate_outside_its_range_is_refused() {
    let quad = quad_text("23.0 g").replace("\"0.3 s⁻¹\"", "\"0.9 s⁻¹\"");
    let fixture = Fixture::with_quad("outside-range", &quad);
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 10: \"0.9 s⁻¹\" is outside its range, 0.1–0.6 s⁻¹"
        ]
    );
}

#[test]
fn a_confidence_or_source_that_isnt_known_is_refused_and_every_problem_is_listed() {
    let quad = quad_text("23.0 g")
        .replace(
            "confidence = \"Manufacturer\", source = \"maker\" }\ninertia",
            "confidence = \"Sure\", source = \"maker\" }\ninertia",
        )
        .replace(
            "source = \"maker\" }\n\n[sources]",
            "source = \"the shop\" }\n\n[sources]",
        );
    let fixture = Fixture::with_quad("every-problem", &quad);
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 5: \"Sure\" isn't a Confidence: write Measured, Manufacturer, Derived or Estimate",
            "packs/fixture/quads/quad/quad.toml line 13: the source \"the shop\" isn't in this file's [sources] list",
        ]
    );
}

#[test]
fn a_newer_format_is_refused_as_needing_a_newer_opendrone() {
    let quad = quad_text("23.0 g").replace("format = 1", "format = 2");
    let fixture = Fixture::with_quad("newer", &quad);
    assert_eq!(
        fixture.problems("fixture/quad"),
        [
            "packs/fixture/quads/quad/quad.toml line 1: this file is format 2, so it needs a newer OpenDrone: this one reads format 1"
        ]
    );
}

#[test]
fn a_broken_manifest_is_refused_listing_every_problem() {
    let manifest = MANIFEST
        .replace("id = \"fixture\"", "id = \"Fixture Pack\"")
        .replace("licence = \"CC0-1.0\"\n", "");
    let problems: Vec<String> = read_manifest("packs/fixture/pack.toml", &manifest)
        .unwrap_err()
        .0
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        problems,
        [
            "packs/fixture/pack.toml line 1: the top of the file is missing `licence`",
            "packs/fixture/pack.toml line 2: the Pack's id \"Fixture Pack\" must be lowercase words joined by dashes, such as \"opendrone\"",
        ]
    );
}

#[test]
fn a_quad_that_isnt_there_is_refused_saying_where_it_looked() {
    let fixture = Fixture::with_quad("missing", &quad_text("23.0 g"));
    assert_eq!(
        fixture.problems("fixture/whoop-99"),
        [
            "fixture/whoop-99: there's no Quad here: packs/fixture/quads/whoop-99/quad.toml doesn't exist"
        ]
    );
    assert_eq!(
        fixture.problems("Whoop 65"),
        [
            "Whoop 65: \"Whoop 65\" isn't an id: write the Pack's id, a slash and the item's folder name, such as \"opendrone/whoop-65\""
        ]
    );
}

#[test]
fn a_quads_fingerprint_covers_its_numbers_but_not_its_name_or_comments() {
    let plain = Fixture::with_quad("fingerprint-plain", &quad_text("23.0 g"));
    let renamed = Fixture::with_quad(
        "fingerprint-renamed",
        &(quad_text("23.0 g").replace("Fixture Quad", "Renamed") + "# a comment\n"),
    );
    let heavier = Fixture::with_quad("fingerprint-heavier", &quad_text("23.1 g"));
    let fingerprint = |fixture: &Fixture| {
        fixture
            .packs()
            .unwrap()
            .quad("fixture/quad")
            .unwrap()
            .fingerprint()
    };
    assert_eq!(fingerprint(&plain), fingerprint(&renamed));
    assert_ne!(fingerprint(&plain), fingerprint(&heavier));
}

#[test]
fn the_empty_air_test_map_has_standard_gravity_and_sea_level_air() {
    let map = built_in().map("test/empty-air").unwrap();
    assert_eq!(map.name, "Empty air");
    assert_eq!(map.world.gravity, 9.81);
    assert_eq!(map.world.air_density, 1.225);
}

#[test]
fn a_map_that_isnt_built_in_yet_is_refused() {
    let problems = built_in().map("opendrone/skate-park").unwrap_err();
    assert_eq!(
        problems.to_string(),
        "opendrone/skate-park: there's no Map with this id; so far only the built-in Test Maps exist: test/empty-air"
    );
}
