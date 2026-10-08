//! Readable checks for the built-in Pack, `packs/opendrone`: it passes the
//! Pack checker, and its two Quads carry the numbers their tickets settled.
//! Each check names its Basis.

mod common;

use core::f64::consts::PI;
use std::fs;

use common::repo;
use opendrone_maths::{Fingerprinter, Vec3};
use opendrone_pack::{
    Confidence, Packs, PropDirection, QuadDefinition, read_quad_file, test_map_ids,
};
use opendrone_physics::MapCollision;

fn built_in() -> Packs {
    Packs::open(&repo().join("packs"), "packs")
        .unwrap()
        .with_test_quads(&repo().join("scenarios/test-quads"), "scenarios/test-quads")
}

fn quad(id: &str) -> QuadDefinition {
    built_in()
        .quad(id)
        .unwrap_or_else(|p| panic!("{id} should pass the Pack checker:\n{p}"))
}

/// Sea level, as the alpha Maps and `test/empty-air` have it.
const AIR_DENSITY: f64 = 1.225;
const GRAVITY: f64 = 9.81;

/// The thrust a prop gives at `rpm`: T = C_T·ρ·n²·D⁴, with n in turns a
/// second.
fn thrust(q: &QuadDefinition, rpm: f64) -> f64 {
    let n = rpm / 60.0;
    q.props.thrust_coefficient * AIR_DENSITY * n * n * q.props.diameter.powi(4)
}

#[test]
fn the_built_in_pack_and_every_test_quad_pass_the_pack_checker() {
    // Basis: Rule (ADR-0011: the built-in Pack goes through the same checker).
    let packs = built_in();
    assert_eq!(packs.problems().to_string(), "");
    let ids: Vec<&str> = packs.quads().iter().map(|q| q.id.as_str()).collect();
    assert_eq!(ids, ["opendrone/freestyle-5", "opendrone/whoop-65"]);
    let test_ids: Vec<&str> = packs.test_quads().iter().map(|q| q.id.as_str()).collect();
    assert_eq!(
        test_ids,
        ["test/freestyle-5-no-drag", "test/whoop-65-no-drag"]
    );
}

#[test]
fn the_whoop_65_weighs_its_dry_mass_plus_its_lava_ii_pack() {
    // Basis: Source (#10 §5: 31.2 g, 23.0 g dry plus an 8.2 g pack).
    let whoop = quad("opendrone/whoop-65");
    assert_eq!(whoop.name, "Whoop 65");
    assert!((whoop.parameters.mass - 0.0312).abs() < 1e-15);
    let [roll, pitch, yaw] = whoop.frame.inertia;
    assert!(
        (roll - 7e-6).abs() < 1e-18 && (pitch - 9e-6).abs() < 1e-18 && (yaw - 1.4e-5).abs() < 1e-18
    );
    assert!(whoop.parameters.drag.duct_ram > 0.0);
}

#[test]
fn the_freestyle_5_weighs_about_650_g_with_its_pack() {
    // Basis: Source (#10 §5: about 650 g, no action camera; iFlight's 419 g
    // dry, and GNB's 233 g 6S 1400 mAh pack).
    let five = quad("opendrone/freestyle-5");
    assert!((five.parameters.mass - 0.652).abs() < 1e-12);
    assert!(five.ducts.is_none());
    assert_eq!(five.parameters.drag.duct_ram, 0.0);
}

#[test]
fn the_whoop_65s_35_mm_props_hover_near_22700_rpm() {
    // Basis: Source (#34's correction to the flight-dynamics research §9.1:
    // C_T ≈ 0.29 on 35 mm props hovers at about 22,700 RPM), ± 1%.
    let whoop = quad("opendrone/whoop-65");
    let per_motor = whoop.parameters.mass * GRAVITY / 4.0;
    let rpm = 60.0 * (per_motor / thrust(&whoop, 60.0)).sqrt();
    assert!((rpm - 22_700.0).abs() < 227.0, "hover at {rpm} RPM");
    // And full thrust, BetaFPV's 30.6 g a motor, near 45,000 RPM.
    let full = 60.0 * (0.0306 * GRAVITY / thrust(&whoop, 60.0)).sqrt();
    assert!((full - 45_000.0).abs() < 450.0, "full thrust at {full} RPM");
}

#[test]
fn the_freestyle_5s_props_meet_t_motors_thrust_table() {
    // Basis: Source (T-Motor's Velox V2207 V3 1750KV table with T5147 props:
    // 1,591.1 g at 29,447.1 RPM and 784.6 g at 20,865.8 RPM), ± 3%.
    let five = quad("opendrone/freestyle-5");
    for (rpm, grams) in [(29_447.1, 1591.1), (20_865.8, 784.6)] {
        let made = thrust(&five, rpm) / GRAVITY * 1000.0;
        assert!(
            (made / grams - 1.0).abs() < 0.03,
            "{made} g at {rpm} RPM, T-Motor says {grams} g"
        );
    }
}

#[test]
fn the_whoop_has_ducts_and_no_buzzer_and_the_5_inch_has_a_buzzer_and_no_ducts() {
    // Basis: Source (#10 §1, #32 §3 and §6, #34: the 5″'s buzzer at 2.7 kHz).
    let (whoop, five) = (quad("opendrone/whoop-65"), quad("opendrone/freestyle-5"));
    assert!(whoop.ducts.is_some() && whoop.collision.duct_rings.is_some());
    assert!(!whoop.sound.buzzer);
    assert!(!whoop.sound.block.contains_key("buzzer_pitch"));
    assert!(five.ducts.is_none() && five.collision.duct_rings.is_none());
    assert!(five.sound.buzzer);
    assert_eq!(five.sound.block["buzzer_pitch"], 2700.0);
    assert_eq!(whoop.sound.esc_melody, "Bluejay default");
}

#[test]
fn both_quads_carry_the_camera_defaults_and_vtx_power_from_14() {
    // Basis: Source (#14 §2 and §5, #28 §2).
    let (whoop, five) = (quad("opendrone/whoop-65"), quad("opendrone/freestyle-5"));
    let degrees = |radians: f64| radians * 180.0 / PI;
    assert!((degrees(whoop.camera.fov) - 160.0).abs() < 1e-9);
    assert!((degrees(five.camera.fov) - 155.0).abs() < 1e-9);
    assert!((degrees(whoop.camera.camera_tilt) - 30.0).abs() < 1e-9);
    assert!((degrees(five.camera.camera_tilt) - 30.0).abs() < 1e-9);
    assert!((whoop.camera.vtx_power - 0.025).abs() < 1e-15);
    assert!((five.camera.vtx_power - 0.4).abs() < 1e-15);
    assert_eq!(
        (
            whoop.camera.analog_dynamic_range,
            five.camera.analog_dynamic_range
        ),
        (7.0, 8.5)
    );
    assert_eq!(
        (whoop.camera.analog_sharpness, five.camera.analog_sharpness),
        (300.0, 400.0)
    );
}

#[test]
fn both_tunes_agree_with_their_quads_poles_and_prop_direction() {
    // Basis: Source (the Meteor65 Pro's diff all: motor_poles 12, props-in;
    // Betaflight 2026.6.2's defaults: 14 poles, yaw_motors_reversed OFF).
    let (whoop, five) = (quad("opendrone/whoop-65"), quad("opendrone/freestyle-5"));
    assert_eq!(whoop.tune.settings["motor_poles"].value, "12");
    assert_eq!(
        whoop.tune.settings["motor_poles"].mark,
        "diff; must match [motors] poles"
    );
    assert_eq!(five.tune.settings["motor_poles"].value, "14");
    for q in [&whoop, &five] {
        assert_eq!(q.props.direction, PropDirection::PropsIn);
        assert_eq!(q.tune.settings["yaw_motors_reversed"].value, "OFF");
    }
}

#[test]
fn the_5_inchs_reverse_thrust_is_measured_and_locked_and_the_whoops_is_an_estimate() {
    // Basis: Source (#16 §4 and #26 §3: 48% Measured on a similar 5.1″ prop;
    // the whoop's 50% is an Estimate, range 25–75%).
    let read = |id: &str| {
        let path = repo().join(format!("packs/opendrone/quads/{id}/quad.toml"));
        read_quad_file(id, &fs::read_to_string(path).unwrap()).unwrap()
    };
    let five = read("freestyle-5");
    let whoop = read("whoop-65");
    assert_eq!(
        five.settings["props.reverse_thrust"].confidence,
        Some(Confidence::Measured)
    );
    assert_eq!(
        whoop.settings["props.reverse_thrust"].confidence,
        Some(Confidence::Estimate)
    );
    assert_eq!(quad("opendrone/freestyle-5").props.reverse_thrust, 0.48);
}

#[test]
fn every_physics_number_in_the_built_in_pack_has_a_confidence_and_every_estimate_a_range() {
    // Basis: Rule (#16 §3). The checker refuses anything else, so this reads
    // the two files to show it.
    for id in ["whoop-65", "freestyle-5"] {
        let path = repo().join(format!("packs/opendrone/quads/{id}/quad.toml"));
        let file = read_quad_file(id, &fs::read_to_string(path).unwrap()).unwrap();
        for (name, setting) in &file.settings {
            if setting.confidence == Some(Confidence::Estimate) {
                assert!(setting.range.is_some(), "{id} {name}");
            }
            if let Some(source) = &setting.source {
                assert!(file.sources.contains_key(source), "{id} {name}");
            }
        }
    }
}

#[test]
fn the_empty_air_test_map_has_standard_gravity_and_sea_level_air() {
    let map = built_in().map("test/empty-air").unwrap();
    assert_eq!(map.name, "Empty air");
    assert_eq!(map.world.gravity, 9.81);
    assert_eq!(map.world.air_density, 1.225);
}

#[test]
fn every_test_map_reads_and_each_of_its_shapes_is_a_solid_the_physics_can_build() {
    // Basis: Rule (#43: a Map's shapes reach the Simulation as plain data,
    // and the Simulation refuses one that isn't a solid).
    let packs = built_in();
    for id in test_map_ids() {
        let map = packs.map(&id).unwrap();
        assert_eq!(map.world.gravity, GRAVITY, "{id}");
        assert_eq!(map.world.air_density, AIR_DENSITY, "{id}");
        assert!(MapCollision::new(&map.shapes).is_ok(), "{id}");
        assert_eq!(map.shapes.is_empty(), id == "test/empty-air", "{id}");
    }
}

#[test]
fn a_maps_fingerprint_follows_its_shapes_and_empty_air_keeps_its_world_values_alone() {
    // Basis: Rule (#16 §9: the fingerprint covers what the Simulation
    // receives, and empty air has no shapes to add).
    let packs = built_in();
    let empty_air = packs.map("test/empty-air").unwrap();
    let mut world_alone = Fingerprinter::new();
    empty_air.world.write_fingerprint(&mut world_alone);
    assert_eq!(empty_air.fingerprint(), world_alone.finish());
    let floor = packs.map("test/flat-floor").unwrap();
    assert_ne!(floor.fingerprint(), empty_air.fingerprint());
    let mut moved = floor.clone();
    moved.shapes.reverse();
    moved.shapes.push(moved.shapes[0].clone());
    assert_ne!(moved.fingerprint(), floor.fingerprint());
}

#[test]
fn each_quads_collision_shape_comes_from_its_definition() {
    // Basis: Rule (#26 §1: the body, the pack where it really sits, the
    // whoop's duct rings and a disc per prop, all from the Quad definition).
    // Millimetres become metres, so each length is compared to within a
    // femtometre.
    let close = |a: f64, b: f64| (a - b).abs() < 1e-15;
    let close3 = |a: Vec3, b: [f64; 3]| close(a.x, b[0]) && close(a.y, b[1]) && close(a.z, b[2]);
    let whoop = quad("opendrone/whoop-65").parameters.shape;
    assert!(close3(whoop.body, [0.035, 0.030, 0.020]), "{whoop:?}");
    assert!(close3(whoop.pack, [0.064, 0.010, 0.006]), "{whoop:?}");
    assert!(close(whoop.pack_height, -0.006));
    assert!(close(whoop.diagonal, 0.066));
    assert!(close(whoop.rotor_height, 0.008));
    assert!(close(whoop.prop_diameter, 0.035));
    let rings = whoop.duct_rings.expect("the whoop has ducts");
    assert!(close(rings.inside_diameter, 0.037) && close(rings.wall, 0.0015));
    assert!(close(rings.height, 0.014));
    assert_eq!((whoop.bounce, whoop.friction), (0.3, 0.5));

    let freestyle = quad("opendrone/freestyle-5").parameters.shape;
    assert_eq!(freestyle.duct_rings, None);
    assert!(
        close3(freestyle.body, [0.080, 0.045, 0.035]),
        "{freestyle:?}"
    );
    assert!(close(freestyle.pack_height, 0.026));
    assert!(close(freestyle.diagonal, 0.225));
    assert!(close(freestyle.prop_diameter, 0.1295));
}

#[test]
fn a_map_that_isnt_built_in_yet_is_refused() {
    let problems = built_in().map("opendrone/skate-park").unwrap_err();
    assert_eq!(
        problems.to_string(),
        "opendrone/skate-park: there's no Map with this id; so far only the built-in Test Maps exist: test/empty-air, test/flat-floor, test/wall, test/thin-rail, test/floor-and-ceiling, test/ledge"
    );
}

#[test]
fn a_quad_that_isnt_there_is_refused_saying_where_it_looked() {
    let packs = built_in();
    assert_eq!(
        packs.quad("opendrone/whoop-99").unwrap_err().to_string(),
        "opendrone/whoop-99: there's no Quad here: packs/opendrone/quads/whoop-99/quad.toml doesn't exist"
    );
    assert_eq!(
        packs.quad("Whoop 65").unwrap_err().to_string(),
        "Whoop 65: \"Whoop 65\" isn't an id: write the Pack's id, a slash and the item's folder name, such as \"opendrone/whoop-65\""
    );
}
