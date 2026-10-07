//! Readable checks for the Pack checker, the spec's second place where tests
//! meet the code: a Pack folder goes in, and checked Quads with their
//! fingerprints come out, or every problem with its file, line and a plain
//! sentence. The committed broken fixture shows every kind of problem at
//! once; each other broken case is the good fixture Pack with one line
//! changed (see `common/mod.rs`). Basis: Rule, from #16 and ADR-0011,
//! ADR-0015.

mod common;

use common::{Fixture, LOG, MANIFEST, QUAD, TEST_QUAD, TUNE};
use opendrone_pack::document::{FORMAT, Upgrade, upgrade_text};
use opendrone_pack::{Confidence, PropDirection, read_quad_file};

fn at(file: &str, line: usize, sentence: &str) -> String {
    format!("{file} line {line}: {sentence}")
}

/// The fixture with one line changed in its Quad definition, and the line it
/// changed.
fn quad_with(case: &str, old: &str, new: &str) -> (Fixture, usize) {
    let fixture = Fixture::new(case).change(QUAD, old, new);
    let line = fixture.line_of(QUAD, new);
    (fixture, line)
}

// The good fixture

#[test]
fn the_good_fixture_pack_passes_and_names_its_items_pack_slash_item() {
    let fixture = Fixture::new("good");
    assert_eq!(fixture.problems(), Vec::<String>::new());
    let packs = fixture.packs();
    let ducted = packs.quad("fixture/ducted").unwrap();
    assert_eq!(ducted.id, "fixture/ducted");
    assert_eq!(ducted.name, "Ducted");
    assert_eq!(
        packs.manifests()[0].licences[0].path,
        "quads/ducted/picture.png"
    );
    let no_drag = packs.quad("test/ducted-no-drag").unwrap();
    assert_eq!(no_drag.based_on.as_deref(), Some("fixture/ducted"));
}

#[test]
fn a_quad_weighs_its_dry_mass_plus_its_pack_which_are_stored_apart() {
    let ducted = Fixture::new("mass").packs().quad("fixture/ducted").unwrap();
    assert_eq!(ducted.frame.dry_mass, 0.023);
    assert!((ducted.battery.mass - 0.0082).abs() < 1e-15);
    assert!((ducted.parameters.mass - 0.0312).abs() < 1e-15);
}

#[test]
fn every_section_of_the_quad_definition_reaches_the_checked_quad_in_si_units() {
    let q = Fixture::new("every-section")
        .packs()
        .quad("fixture/ducted")
        .unwrap();
    assert_eq!(q.props.blades, 3);
    assert_eq!(q.props.direction, PropDirection::PropsIn);
    assert_eq!(q.props.reverse_thrust, 0.5);
    assert_eq!(q.motors.poles, 12);
    assert!((q.motors.kv - 19500.0 * 2.0 * std::f64::consts::PI / 60.0).abs() < 1e-9);
    assert_eq!(
        (q.motors.no_load_current, q.motors.no_load_voltage),
        (0.3, 4.0)
    );
    assert_eq!(q.motors.restart_tries, 3);
    assert_eq!(q.collision.body, [0.035, 0.030, 0.020]);
    assert_eq!(q.collision.duct_rings, Some([0.037, 0.0015, 0.014]));
    assert_eq!(
        q.battery.voltage_curve,
        [(1.0, 4.35), (0.5, 3.92), (0.0, 3.30)]
    );
    assert!((q.battery.capacity - 0.32 * 3600.0).abs() < 1e-9);
    assert!(q.ducts.is_some());
    assert!(q.sound.buzzer);
    assert_eq!(q.sound.block["buzzer_pitch"], 2700.0);
    assert_eq!(q.sound.block["harmonics"], 8.0);
    assert!((q.camera.fov - 160f64.to_radians()).abs() < 1e-12);
    assert_eq!(q.tune.settings["motor_poles"].value, "12");
}

// The broken fixture

#[test]
fn the_broken_fixture_packs_list_every_problem_at_once_and_load_what_passes() {
    // crates/pack/tests/fixtures/broken: one Pack with a fine Quad, a Quad
    // with a problem on each line marked BROKEN, and a Quad in a newer format;
    // and a Pack whose manifest is broken.
    let root = common::good_fixture().join("../broken");
    let packs = opendrone_pack::Packs::open(&root.join("packs"), "packs").unwrap();
    let quad = "packs/broken/quads/many-problems/quad.toml";
    let text = std::fs::read_to_string(root.join(quad)).unwrap();
    let line = |needle: &str| common::line_of(&text, needle);
    assert_eq!(
        packs.problems().0.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "packs/broken/quads/from-a-newer-opendrone/quad.toml line 2: this file is format 2, so it needs a newer OpenDrone: this one reads format 1".to_string(),
            at(quad, line("no Confidence"), "[frame] inertia needs a Confidence: add confidence = \"Measured\", \"Manufacturer\", \"Derived\" or \"Estimate\""),
            at(quad, line("without a range"), "[frame] rotor_height is an Estimate, so it needs the `range` it may move within, such as range = \"×0.5–×2\""),
            at(quad, line("4.7 in"), "`pitch` isn't something OpenDrone reads in [props]; it reads `diameter`, `blades`, `direction`, `thrust_coefficient`, `power_coefficient`, `rotor_drag`, `rotor_inertia`, `reverse_thrust`, `reverse_torque`, `grip`"),
            at(quad, line("in metres"), "\"23 m\" is a length, but this needs a mass, such as \"23.0 g\""),
            at(quad, line("decimal comma"), "\"66,5 mm\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"66.5 mm\""),
            at(quad, line("outside its range"), "\"0.9 s⁻¹\" is outside its range, 0.1–0.6 s⁻¹"),
            at("packs/broken/quads/many-problems/tune.txt", 4, "motor_poles is 14, but the Quad's motors have 12 poles ([motors] poles in packs/broken/quads/many-problems/quad.toml); they must match"),
            "packs/broken-manifest/pack.toml line 1: the top of the file is missing `licence`".to_string(),
            "packs/broken-manifest/pack.toml line 3: the Pack's id \"Broken Manifest\" must be lowercase words joined by dashes, such as \"opendrone\"".to_string(),
            "packs/broken-manifest: its manifest (pack.toml) is broken, so the whole Pack is skipped".to_string(),
        ]
    );
    let loaded: Vec<&str> = packs.quads().iter().map(|q| q.id.as_str()).collect();
    assert_eq!(loaded, ["broken/fine"]);
    assert_eq!(packs.manifests().len(), 1);
}

// The manifest

#[test]
fn a_broken_manifest_skips_the_whole_pack_and_lists_every_problem() {
    let fixture = Fixture::new("broken-manifest")
        .change(
            MANIFEST,
            "id          = \"fixture\"",
            "id          = \"Fixture Pack\"",
        )
        .change(MANIFEST, "licence     = \"CC0-1.0\"\n", "");
    assert_eq!(
        fixture.problems(),
        [
            at(MANIFEST, 1, "the top of the file is missing `licence`"),
            at(
                MANIFEST,
                4,
                "the Pack's id \"Fixture Pack\" must be lowercase words joined by dashes, such as \"opendrone\""
            ),
            "packs/fixture: its manifest (pack.toml) is broken, so the whole Pack is skipped"
                .to_string(),
            "test-quads/ducted-no-drag.toml line 3: can't build on \"fixture/ducted\" until its own problems are fixed".to_string(),
            "fixture/ducted: there's no Pack with the id \"fixture\"; these Packs were skipped because of their problems: packs/fixture".to_string(),
        ]
    );
}

#[test]
fn a_manifest_lists_only_what_opendrone_reads() {
    let fixture = Fixture::new("manifest-unknown-key").change(
        MANIFEST,
        "author      = ",
        "website     = \"example.com\"\nauthor      = ",
    );
    assert_eq!(
        fixture.problems()[0],
        at(
            MANIFEST,
            8,
            "`website` isn't something OpenDrone reads in the top of the file; it reads `format`, `id`, `name`, `description`, `version`, `author`, `licence`, `licences`"
        )
    );
}

#[test]
fn a_file_under_cc_by_needs_a_credit_line() {
    let fixture = Fixture::new("no-credit").change(
        MANIFEST,
        ", credit = \"Picture by the OpenDrone contributors\"",
        "",
    );
    assert_eq!(
        fixture.problems()[0],
        at(
            MANIFEST,
            12,
            "\"quads/ducted/picture.png\" is under CC-BY-4.0, so it needs a credit line: add credit = \"…\""
        )
    );
}

#[test]
fn the_id_test_is_kept_for_test_quads_and_test_maps() {
    let fixture = Fixture::new("id-test").change(MANIFEST, "\"fixture\"", "\"test\"");
    assert_eq!(
        fixture.problems()[0],
        at(
            MANIFEST,
            4,
            "the id \"test\" is kept for Test Quads and Test Maps, which only Scenarios use; pick another"
        )
    );
}

#[test]
fn a_second_pack_with_the_same_id_is_skipped() {
    let fixture = Fixture::new("same-id");
    fixture.write("packs/other/pack.toml", &fixture.read(MANIFEST));
    assert_eq!(
        fixture.problems(),
        [
            "packs/other/pack.toml: packs/fixture already has the id \"fixture\", and every Pack's id is its own, so this Pack is skipped"
        ]
    );
}

// Items

#[test]
fn a_broken_item_is_skipped_and_the_rest_of_its_pack_loads() {
    let fixture = Fixture::new("one-broken-item");
    for file in ["quad.toml", "tune.txt", "picture.png"] {
        let from = fixture.root.join("packs/fixture/quads/ducted").join(file);
        let to = fixture.root.join("packs/fixture/quads/second").join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(from, to).unwrap();
    }
    let fixture = fixture.change(
        "packs/fixture/quads/second/quad.toml",
        "\"23.0 g\"",
        "\"23,0 g\"",
    );
    let packs = fixture.packs();
    assert!(packs.quad("fixture/ducted").is_ok());
    assert_eq!(
        packs.quad("fixture/second").unwrap_err().to_string(),
        "packs/fixture/quads/second/quad.toml line 20: \"23,0 g\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"23.0 g\""
    );
    assert_eq!(fixture.problems().len(), 1);
}

#[test]
fn an_item_folder_must_be_named_as_an_id() {
    let fixture = Fixture::new("folder-not-an-id");
    std::fs::rename(
        fixture.root.join("packs/fixture/quads/ducted"),
        fixture.root.join("packs/fixture/quads/Ducted_Quad"),
    )
    .unwrap();
    assert_eq!(
        fixture.problems()[0],
        "packs/fixture/quads/Ducted_Quad: the folder's name is the Quad's id, so \"Ducted_Quad\" must be lowercase words joined by dashes, such as \"whoop-65\""
    );
}

#[test]
fn a_kind_of_item_this_opendrone_doesnt_know_is_skipped_and_reported() {
    let fixture = Fixture::new("unknown-kind");
    fixture.write("packs/fixture/modifiers/low-gravity.toml", "format = 1\n");
    assert_eq!(
        fixture.problems(),
        [
            "packs/fixture/modifiers: `modifiers` isn't a kind of item this OpenDrone knows, so it's skipped; a Pack holds quads/, maps/, input-devices/"
        ]
    );
}

#[test]
fn every_quad_folder_holds_its_quad_definition_and_tune_and_a_picture() {
    let fixture = Fixture::new("missing-files");
    fixture.remove(TUNE);
    fixture.remove("packs/fixture/quads/ducted/picture.png");
    assert_eq!(
        fixture.problems()[..2],
        [
            "packs/fixture/quads/ducted: every Quad's folder holds a tune.txt, but this one has none",
            "packs/fixture/quads/ducted/picture.png: doesn't exist, but the Quad definition's `picture` names it for the Quad picker",
        ]
    );
}

#[test]
fn the_picture_must_be_a_png() {
    let fixture = Fixture::new("not-a-png");
    fixture.write("packs/fixture/quads/ducted/picture.png", "not a picture");
    assert_eq!(
        fixture.quad_problems(),
        ["packs/fixture/quads/ducted/picture.png: isn't a PNG picture"]
    );
}

// Numbers in a Quad definition

#[test]
fn a_bad_unit_is_refused() {
    let (fixture, line) = quad_with("bad-unit", "\"23.0 g\"", "\"23 m\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "\"23 m\" is a length, but this needs a mass, such as \"23.0 g\""
        )]
    );
}

#[test]
fn a_decimal_comma_is_refused_with_the_fix() {
    let (fixture, line) = quad_with("decimal-comma", "\"23.0 g\"", "\"23,0 g\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "\"23,0 g\" isn't a number OpenDrone can read: numbers take a decimal point, so write \"23.0 g\""
        )]
    );
}

#[test]
fn a_physics_number_without_a_confidence_is_refused() {
    let (fixture, line) = quad_with(
        "missing-confidence",
        "dry_mass     = { value = \"23.0 g\", confidence = \"Manufacturer\", source = \"maker\" }",
        "dry_mass     = { value = \"23.0 g\", source = \"maker\" }",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "[frame] dry_mass needs a Confidence: add confidence = \"Measured\", \"Manufacturer\", \"Derived\" or \"Estimate\""
        )]
    );
}

#[test]
fn a_physics_number_written_without_its_confidence_and_source_is_refused() {
    let (fixture, line) = quad_with(
        "bare-physics-number",
        "dry_mass     = { value = \"23.0 g\", confidence = \"Manufacturer\", source = \"maker\" }",
        "dry_mass     = \"23.0 g\"",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "[frame] dry_mass is a physics number, so it's written { value = \"…\", confidence = \"…\", source = \"…\" }, and an Estimate adds its range"
        )]
    );
}

#[test]
fn an_estimate_without_a_range_is_refused() {
    let (fixture, _) = quad_with("no-range", ", range = \"0.1–0.6 s⁻¹\"", "");
    let line = fixture.line_of(QUAD, "rotor_drag");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "[props] rotor_drag is an Estimate, so it needs the `range` it may move within, such as range = \"×0.5–×2\""
        )]
    );
}

#[test]
fn an_estimate_outside_its_range_is_refused() {
    let (fixture, line) = quad_with("outside-range", "\"0.3 s⁻¹\"", "\"0.9 s⁻¹\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "\"0.9 s⁻¹\" is outside its range, 0.1–0.6 s⁻¹"
        )]
    );
}

#[test]
fn only_an_estimate_has_a_range() {
    let (fixture, line) = quad_with(
        "locked-range",
        "\"66 mm\", confidence = \"Manufacturer\",",
        "\"66 mm\", confidence = \"Manufacturer\", range = \"60–70 mm\",",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "only an Estimate has a range: [frame] diagonal is Manufacturer, so it's locked; take the range out"
        )]
    );
}

#[test]
fn a_number_outside_what_it_can_be_is_refused() {
    let (fixture, line) = quad_with("tilt-too-far", "\"30°\"", "\"90°\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "[camera] camera_tilt must be from 0° to 80°, but it's \"90°\""
        )]
    );
}

#[test]
fn a_confidence_or_source_that_isnt_known_is_refused_and_every_problem_is_listed() {
    let fixture = Fixture::new("every-problem")
        .change(
            QUAD,
            "\"23.0 g\", confidence = \"Manufacturer\"",
            "\"23.0 g\", confidence = \"Sure\"",
        )
        .change(
            QUAD,
            "\"66 mm\", confidence = \"Manufacturer\", source = \"maker\"",
            "\"66 mm\", confidence = \"Manufacturer\", source = \"the shop\"",
        )
        .change(QUAD, "\"0.3 s⁻¹\"", "\"0.9 s⁻¹\"");
    assert_eq!(
        fixture.quad_problems(),
        [
            at(
                QUAD,
                20,
                "\"Sure\" isn't a Confidence: write Measured, Manufacturer, Derived or Estimate"
            ),
            at(
                QUAD,
                21,
                "the source \"the shop\" isn't in this file's [sources] list"
            ),
            at(
                QUAD,
                fixture.line_of(QUAD, "0.9 s⁻¹"),
                "\"0.9 s⁻¹\" is outside its range, 0.1–0.6 s⁻¹"
            ),
        ]
    );
}

#[test]
fn an_unknown_key_or_section_is_refused_naming_what_is_read_there() {
    let fixture = Fixture::new("unknown-key")
        .change(
            QUAD,
            "blades             = 3",
            "blades             = 3\npitch              = \"4.7 in\"",
        )
        .change(QUAD, "[board]", "[boards]");
    let problems = fixture.quad_problems();
    assert_eq!(
        problems[0],
        at(
            QUAD,
            fixture.line_of(QUAD, "pitch              ="),
            "`pitch` isn't something OpenDrone reads in [props]; it reads `diameter`, `blades`, `direction`, `thrust_coefficient`, `power_coefficient`, `rotor_drag`, `rotor_inertia`, `reverse_thrust`, `reverse_torque`, `grip`"
        )
    );
    assert!(
        problems[1].starts_with(&at(
            QUAD,
            fixture.line_of(QUAD, "[boards]"),
            "`boards` isn't something OpenDrone reads in a Quad definition"
        )),
        "{problems:?}"
    );
    assert_eq!(
        problems[2],
        "packs/fixture/quads/ducted/quad.toml: the Quad definition is missing its [board] section"
    );
}

#[test]
fn a_missing_key_is_refused_naming_its_section() {
    let fixture = Fixture::new("missing-key").change(
        QUAD,
        "grip               = { value = \"0.5\", confidence = \"Estimate\", range = \"0.2–0.8\", source = \"guess\" }\n",
        "",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            fixture.line_of(QUAD, "[props]"),
            "[props] is missing `grip`"
        ),]
    );
}

#[test]
fn counts_choices_camera_defaults_and_the_sound_block_carry_no_confidence() {
    let fixture = Fixture::new("no-confidence-there")
        .change(QUAD, "blades             = 3", "blades             = \"3\"")
        .change(QUAD, "fov         = \"160°\"", "fov         = { value = \"160°\", confidence = \"Manufacturer\", source = \"maker\" }")
        .change(QUAD, "hit_level             = \"0.5\"", "hit_level             = { value = \"0.5\", confidence = \"Estimate\", range = \"0–1\", source = \"guess\" }");
    assert_eq!(
        fixture.quad_problems(),
        [
            at(
                QUAD,
                fixture.line_of(QUAD, "fov"),
                "[camera] fov is a camera default, so it carries no Confidence or source: write just the value"
            ),
            at(
                QUAD,
                fixture.line_of(QUAD, "hit_level"),
                "[sound_block] hit_level is a sound-block value, so it carries no Confidence or source: write just the value"
            ),
            at(
                QUAD,
                fixture.line_of(QUAD, "blades"),
                "[props] blades is a count: write a whole number without quotes, such as blades = 3"
            ),
        ]
    );
}

#[test]
fn a_choice_must_be_one_of_its_words() {
    let (fixture, line) = quad_with("bad-choice", "\"props-in\"", "\"props in\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "\"props in\" isn't a choice for [props] direction: write \"props-in\" or \"props-out\""
        )]
    );
}

#[test]
fn only_a_test_quad_may_use_based_on() {
    let fixture = Fixture::new("based-on-in-a-pack").change(
        QUAD,
        "name        = \"Ducted\"",
        "based_on    = \"fixture/other\"\nname        = \"Ducted\"",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            fixture.line_of(QUAD, "based_on"),
            "only a Test Quad may use `based_on`: Test Quads live in scenarios/test-quads/, and only the Scenario runner reads them"
        )]
    );
}

#[test]
fn ducts_need_their_rings_and_buzzer_values_need_a_buzzer() {
    let fixture = Fixture::new("cross-checks")
        .change(QUAD, "duct_rings  = { value = \"37 mm inside, 1.5 mm wall, 14 mm tall\", confidence = \"Estimate\", range = \"×0.9–×1.1\", source = \"guess\" }\n", "")
        .change(QUAD, "buzzer     = true", "buzzer     = false");
    assert_eq!(
        fixture.quad_problems(),
        [
            at(
                QUAD,
                fixture.line_of(QUAD, "ram_drag"),
                "a Quad with [ducts] needs [collision] duct_rings"
            ),
            at(
                QUAD,
                fixture.line_of(QUAD, "buzzer_pitch"),
                "[sound_block] buzzer_pitch is for the buzzer, but this Quad has none ([sound] buzzer = false)"
            ),
            at(
                QUAD,
                fixture.line_of(QUAD, "buzzer_level"),
                "[sound_block] buzzer_level is for the buzzer, but this Quad has none ([sound] buzzer = false)"
            ),
        ]
    );
}

#[test]
fn the_voltage_curve_runs_from_full_to_empty() {
    let (fixture, line) = quad_with("curve-ends", "3.30 V at 0%\"", "3.40 V at 0%\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            line,
            "[battery] voltage_curve runs from [battery] full at 100% down to [battery] empty at 0%, with the voltage never rising on the way"
        )]
    );
}

// The Tune

#[test]
fn a_motor_poles_that_disagrees_with_the_quads_poles_is_refused() {
    let fixture =
        Fixture::new("pole-mismatch").change(TUNE, "motor_poles = 12", "motor_poles = 14");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            TUNE,
            4,
            "motor_poles is 14, but the Quad's motors have 12 poles ([motors] poles in packs/fixture/quads/ducted/quad.toml); they must match"
        )]
    );
}

#[test]
fn a_yaw_motors_reversed_that_disagrees_with_the_prop_direction_is_refused() {
    let fixture = Fixture::new("direction-mismatch").change(QUAD, "\"props-in\"", "\"props-out\"");
    assert_eq!(
        fixture.quad_problems(),
        [at(
            TUNE,
            5,
            "yaw_motors_reversed is OFF, but the props spin props-out ([props] direction in packs/fixture/quads/ducted/quad.toml), which needs ON"
        )]
    );
}

#[test]
fn every_tune_line_carries_a_mark_saying_where_its_value_came_from() {
    let fixture = Fixture::new("tune-marks")
        .change(TUNE, "# diff (was dshot_idle_value)", "")
        .change(TUNE, "# ADR-0008", "# from a forum")
        .change(
            TUNE,
            "set crashflip_rate = 0",
            "aux 0 0 0 1700 2100 0 0\nset crashflip_rate = 0",
        );
    assert_eq!(
        fixture.quad_problems(),
        [
            at(
                TUNE,
                6,
                "`motor_idle` has no mark saying where its value came from: end the line with # and `diff`, a version's default such as `4.3 default`, `ADR-0008` or `hand-set: <reason>`, optionally followed by `(was <old name>)`"
            ),
            at(
                TUNE,
                8,
                "`feedforward_yaw_hold_gain`'s mark \"from a forum\" isn't one OpenDrone knows: write `diff`, a version's default such as `4.3 default`, `ADR-0008` or `hand-set: <reason>`, optionally followed by `(was <old name>)`"
            ),
            at(
                TUNE,
                9,
                "`aux` isn't a `set` line: a Tune holds only `set` lines and comments, and Betaflight's other commands, such as `aux` lines and rate profiles, belong to the pilot (ADR-0015)"
            ),
        ]
    );
}

#[test]
fn a_tune_must_set_the_two_settings_the_quad_cross_checks() {
    let fixture = Fixture::new("tune-missing-poles").change(
        TUNE,
        "set motor_poles = 12                   # diff; must match [motors] poles\n",
        "",
    );
    assert_eq!(
        fixture.quad_problems(),
        [
            "packs/fixture/quads/ducted/tune.txt: the Tune must set motor_poles, which must match the Quad's 12 motor poles"
        ]
    );
}

// Test Quads

#[test]
fn a_test_quad_changes_only_what_it_lists() {
    let packs = Fixture::new("test-quad").packs();
    let ducted = packs.quad("fixture/ducted").unwrap();
    let no_drag = packs.quad("test/ducted-no-drag").unwrap();
    assert_eq!(no_drag.parameters.mass, ducted.parameters.mass);
    assert_eq!(no_drag.parameters.inertia, ducted.parameters.inertia);
    assert_eq!(no_drag.frame.drag_area, [0.0; 3]);
    assert_eq!(no_drag.props.rotor_drag, 0.0);
    assert_eq!(no_drag.ducts.as_ref().unwrap().ram_drag, 0.0);
    assert_eq!(no_drag.motors, ducted.motors);
    assert_ne!(no_drag.fingerprint(), ducted.fingerprint());
}

#[test]
fn a_change_to_the_real_quad_carries_into_its_test_quads() {
    let packs = Fixture::new("carries")
        .change(QUAD, "\"23.0 g\"", "\"40.0 g\"")
        .packs();
    let no_drag = packs.quad("test/ducted-no-drag").unwrap();
    assert!((no_drag.parameters.mass - 0.0482).abs() < 1e-15);
}

#[test]
fn a_test_quad_changing_a_setting_its_quad_lacks_is_refused() {
    let fixture = Fixture::new("lacks").change(TEST_QUAD, "drag_area =", "drag_aera =");
    assert_eq!(
        fixture.problems(),
        [at(
            TEST_QUAD,
            8,
            "[frame] drag_aera isn't a setting of fixture/ducted, so it can't be changed here"
        )]
    );
}

#[test]
fn a_test_quads_change_carries_no_confidence() {
    let fixture = Fixture::new("test-quad-confidence").change(
        TEST_QUAD,
        "rotor_drag = \"0 s⁻¹\"",
        "rotor_drag = { value = \"0 s⁻¹\", confidence = \"Measured\", source = \"maker\" }",
    );
    assert_eq!(
        fixture.problems(),
        [at(
            TEST_QUAD,
            11,
            "a Test Quad's change carries no Confidence or source: write just the value, such as rotor_drag = \"0.3 s⁻¹\""
        )]
    );
}

#[test]
fn a_test_quad_builds_on_a_real_quad_only() {
    let fixture =
        Fixture::new("on-a-test-quad").change(TEST_QUAD, "\"fixture/ducted\"", "\"test/other\"");
    assert_eq!(
        fixture.problems(),
        [at(
            TEST_QUAD,
            3,
            "a Test Quad builds on a real Quad, not on another Test Quad (\"test/other\")"
        )]
    );
}

#[test]
fn a_test_quads_change_is_checked_like_the_real_quads_values() {
    let fixture = Fixture::new("test-quad-cross-check").change(
        TEST_QUAD,
        "[props]\n",
        "[props]\ndirection = \"props-out\"\n",
    );
    assert_eq!(
        fixture.problems(),
        [at(
            TUNE,
            5,
            "yaw_motors_reversed is OFF, but the props spin props-out ([props] direction in test-quads/ducted-no-drag.toml), which needs ON"
        )]
    );
}

#[test]
fn only_test_quads_live_in_the_test_quad_folder() {
    let fixture = Fixture::new("stray-file");
    fixture.write("test-quads/notes.txt", "a note");
    assert_eq!(
        fixture.problems(),
        ["test-quads/notes.txt: only Test Quads, each written as <id>.toml, live in test-quads"]
    );
}

// Formats

#[test]
fn a_newer_format_is_refused_as_needing_a_newer_opendrone() {
    let fixture = Fixture::new("newer")
        .change(QUAD, "format = 1", "format = 2")
        .change(TEST_QUAD, "format   = 1", "format   = 2");
    let problems = fixture.problems();
    assert_eq!(
        problems[0],
        at(
            QUAD,
            2,
            "this file is format 2, so it needs a newer OpenDrone: this one reads format 1"
        )
    );
    assert_eq!(
        problems[1],
        at(
            TEST_QUAD,
            2,
            "this file is format 2, so it needs a newer OpenDrone: this one reads format 1"
        )
    );
}

#[test]
fn every_file_starts_with_its_format() {
    let fixture = Fixture::new("format-not-first").change(
        QUAD,
        "format = 1\n\nname        = \"Ducted\"",
        "name        = \"Ducted\"\nformat = 1",
    );
    assert_eq!(
        fixture.quad_problems(),
        [at(
            QUAD,
            fixture.line_of(QUAD, "format = 1"),
            "every file starts with `format = 1`, before anything else but comments"
        )]
    );
}

#[test]
fn an_older_format_is_upgraded_in_memory_one_step_at_a_time() {
    fn one_to_two(text: &str) -> Result<String, String> {
        Ok(text
            .replace("format = 1", "format = 2")
            .replace("frame_mass", "dry_mass"))
    }
    fn two_to_three(text: &str) -> Result<String, String> {
        Ok(text.replace("format = 2", "format = 3") + "# upgraded\n")
    }
    let steps = [
        Upgrade {
            from: 1,
            rewrite: one_to_two,
        },
        Upgrade {
            from: 2,
            rewrite: two_to_three,
        },
    ];
    assert_eq!(
        upgrade_text("format = 1\nframe_mass = \"23 g\"\n", 1, 3, &steps).unwrap(),
        "format = 3\ndry_mass = \"23 g\"\n# upgraded\n"
    );
    assert_eq!(
        upgrade_text("format = 0\n", 0, 3, &steps).unwrap_err(),
        "this file is format 0, and this OpenDrone has no step that upgrades it"
    );
    // Format 1 is the first, so today nothing is older than what this
    // OpenDrone reads.
    assert_eq!(FORMAT, 1);
}

// Fingerprints

#[test]
fn a_fingerprint_covers_only_what_the_simulation_receives() {
    let fingerprint = |fixture: Fixture| {
        fixture
            .packs()
            .quad("fixture/ducted")
            .unwrap()
            .fingerprint()
    };
    let plain = fingerprint(Fixture::new("fp-plain"));
    // Not received: on-screen text, comments, the picture, sources, a
    // Confidence's wording, the camera, the sound block and the Feel Test log.
    let unchanged = [
        Fixture::new("fp-name").change(QUAD, "\"Ducted\"", "\"Renamed\""),
        Fixture::new("fp-comment").change(
            QUAD,
            "# A good fixture Quad",
            "# A comment changed. A good fixture Quad",
        ),
        Fixture::new("fp-source").change(
            QUAD,
            "\"the maker's page\"",
            "\"the maker's page, read again\"",
        ),
        Fixture::new("fp-camera").change(QUAD, "\"160°\"", "\"150°\""),
        Fixture::new("fp-sound").change(
            QUAD,
            "hit_level             = \"0.5\"",
            "hit_level             = \"0.6\"",
        ),
        Fixture::new("fp-log").change(LOG, "carved too wide", "carved much too wide"),
        Fixture::new("fp-tune-mark").change(TUNE, "# ADR-0008", "# hand-set: a test"),
    ];
    for fixture in unchanged {
        let case = fixture.root.clone();
        assert_eq!(fingerprint(fixture), plain, "{}", case.display());
    }
    let picture = Fixture::new("fp-picture");
    let mut bytes =
        std::fs::read(picture.root.join("packs/fixture/quads/ducted/picture.png")).unwrap();
    bytes.extend_from_slice(b"changed");
    std::fs::write(
        picture.root.join("packs/fixture/quads/ducted/picture.png"),
        bytes,
    )
    .unwrap();
    assert_eq!(fingerprint(picture), plain);
    // Received: every physics number, count and choice, and the Tune.
    let changed = [
        Fixture::new("fp-mass").change(QUAD, "\"23.0 g\"", "\"23.1 g\""),
        Fixture::new("fp-grip").change(
            QUAD,
            "grip               = { value = \"0.5\"",
            "grip               = { value = \"0.6\"",
        ),
        Fixture::new("fp-blades").change(QUAD, "blades             = 3", "blades             = 4"),
        Fixture::new("fp-tune").change(TUNE, "set motor_idle = 600", "set motor_idle = 550"),
    ];
    for fixture in changed {
        let case = fixture.root.clone();
        assert_ne!(fingerprint(fixture), plain, "{}", case.display());
    }
}

#[test]
fn the_same_number_written_two_ways_gives_the_same_fingerprint() {
    let plain = Fixture::new("fp-written")
        .packs()
        .quad("fixture/ducted")
        .unwrap();
    let other = Fixture::new("fp-written-other")
        .change(QUAD, "\"23.0 g\"", "\"0.023 kg\"")
        .packs()
        .quad("fixture/ducted")
        .unwrap();
    assert_eq!(plain.fingerprint(), other.fingerprint());
}

// The Feel Test log

#[test]
fn a_feel_test_log_row_says_when_what_from_and_to_and_why() {
    let fixture = Fixture::new("log-row")
        .change(LOG, "2026-11-02", "2 Nov 2026")
        .change(LOG, " carved too wide after a sprint ", " ");
    assert_eq!(
        fixture.quad_problems(),
        [
            at(
                LOG,
                5,
                "\"2 Nov 2026\" isn't a date: write it as year-month-day, such as 2026-11-02"
            ),
            at(LOG, 5, "every Feel Test log row says why"),
        ]
    );
}

#[test]
fn reading_a_quad_file_keeps_each_numbers_confidence_and_source() {
    let fixture = Fixture::new("read-file");
    let file = read_quad_file(QUAD, &fixture.read(QUAD)).unwrap();
    let dry_mass = &file.settings["frame.dry_mass"];
    assert_eq!(dry_mass.confidence, Some(Confidence::Manufacturer));
    assert_eq!(dry_mass.source.as_deref(), Some("maker"));
    assert_eq!(file.sources["maker"], "the maker's page");
}
