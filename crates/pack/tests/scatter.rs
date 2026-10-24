//! Readable checks for the Pack checker's grass tags (#338): `od_scatter`
//! says which plant a ground grows, `od_scatter_density` how many clumps a
//! square metre and `od_scatter_height` the shortest and tallest clump in
//! metres. The game grows the clumps; the checker only takes the tags or
//! refuses them in plain words. Each `.glb` is built here in plain Rust
//! (`common/glb.rs`). Basis: Rule, from #338 and
//! docs/verification/checking-a-pack.md.

mod common;

use common::Fixture;
use common::glb::{Glb, box_mesh, sheet_mesh};
use opendrone_pack::{MapDefinition, Scatter, ScatterKind, test_map, test_map_ids};
use serde_json::{Value, json};

/// Ground, a yard sheet on it with these tags, and a Launch Spot.
fn with_yard(tags: Value) -> Glb {
    Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .object(
            "yard",
            tags,
            sheet_mesh([0.0, -10.0, 0.0], 6.0),
            "grass_weeds",
        )
        .launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0)
}

fn problems_of(case: &str, glb: Glb) -> Vec<String> {
    Fixture::new(case).with_map(glb.build()).map_problems()
}

fn map_of(case: &str, glb: Glb) -> MapDefinition {
    Fixture::new(case)
        .with_map(glb.build())
        .packs()
        .map("fixture/yard")
        .unwrap_or_else(|p| panic!("the Map should pass the checker:\n{p}"))
}

fn one_problem(case: &str, glb: Glb, says: &str) {
    let problems = problems_of(case, glb);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains(says), "{problems:?}");
}

fn grass(density: Value, height: Value) -> Value {
    json!({
        "od_collider": "trimesh",
        "od_scatter": "grass",
        "od_scatter_density": density,
        "od_scatter_height": height,
    })
}

#[test]
fn a_ground_tagged_with_grass_is_drawn_with_its_density_and_heights_and_stays_solid() {
    let map = map_of(
        "scatter-good",
        with_yard(grass(json!(6), json!([0.12, 0.4]))),
    );
    let yard = map.drawing.iter().find(|p| p.name == "yard").unwrap();
    assert_eq!(
        yard.scatter,
        Some(Scatter {
            kind: ScatterKind::Grass,
            density: 6.0,
            height: [0.12, 0.4],
        })
    );
    // The ground under the grass is solid as tagged; the grass itself is
    // nothing the Simulation sees.
    assert_eq!(map.solid_names, ["ground", "yard"]);
    assert!(
        map.drawing
            .iter()
            .filter(|p| p.name != "yard")
            .all(|p| p.scatter.is_none())
    );
}

#[test]
fn a_plant_opendrone_doesnt_grow_is_refused_with_the_kinds_it_does() {
    for bad in [json!("weeds"), json!(1), json!("")] {
        let mut tags = grass(json!(6), json!([0.1, 0.3]));
        tags["od_scatter"] = bad;
        one_problem(
            "scatter-kind",
            with_yard(tags),
            "isn't a kind of plant OpenDrone grows: the kinds are \"grass\"",
        );
    }
}

#[test]
fn a_density_is_a_number_of_clumps_a_square_metre_from_a_tenth_to_twenty() {
    for bad in [json!(0), json!(0.05), json!(25), json!("lots")] {
        one_problem(
            "scatter-density",
            with_yard(grass(bad, json!([0.1, 0.3]))),
            "must be a number from 0.1 to 20: how many clumps grow on a square metre",
        );
    }
    for good in [json!(0.1), json!(20), json!(3.5)] {
        assert_eq!(
            problems_of(
                "scatter-density-good",
                with_yard(grass(good, json!([0.1, 0.3])))
            ),
            Vec::<String>::new()
        );
    }
}

#[test]
fn heights_are_two_numbers_in_metres_the_shortest_first_each_from_2_cm_to_2_m() {
    for bad in [
        json!([0.5, 0.2]),
        json!([0.1]),
        json!([0.1, 0.2, 0.3]),
        json!([0.0, 0.3]),
        json!([0.1, 2.5]),
        json!("tall"),
        json!(0.3),
    ] {
        one_problem(
            "scatter-height",
            with_yard(grass(json!(6), bad)),
            "must be two numbers in metres, the shortest clump then the tallest",
        );
    }
    assert_eq!(
        problems_of(
            "scatter-height-same",
            with_yard(grass(json!(6), json!([0.3, 0.3])))
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_density_or_height_with_no_plant_named_is_refused() {
    one_problem(
        "scatter-no-kind",
        with_yard(json!({
            "od_collider": "trimesh",
            "od_scatter_density": 6,
            "od_scatter_height": [0.1, 0.3],
        })),
        "has od_scatter_density or od_scatter_height but no od_scatter",
    );
}

#[test]
fn a_plant_named_with_no_density_or_height_is_refused() {
    for missing in ["od_scatter_density", "od_scatter_height"] {
        let mut tags = grass(json!(6), json!([0.1, 0.3]));
        tags.as_object_mut().unwrap().remove(missing);
        one_problem(
            "scatter-missing",
            with_yard(tags),
            "grows grass (od_scatter) but doesn't say how much and how tall",
        );
    }
}

#[test]
fn grass_on_a_decal_or_on_a_part_that_is_never_drawn_is_refused() {
    let mut on_decal = grass(json!(6), json!([0.1, 0.3]));
    on_decal["od_collider"] = json!("none");
    on_decal["od_decal"] = json!(1);
    let problems = problems_of("scatter-decal", with_yard(on_decal));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("is a decal (od_decal) and grows grass (od_scatter)")),
        "{problems:?}"
    );
    let mut hidden = grass(json!(6), json!([0.1, 0.3]));
    hidden["od_collision_only"] = json!(1);
    one_problem(
        "scatter-hidden",
        with_yard(hidden),
        "grows grass (od_scatter) but is od_collision_only",
    );
}

#[test]
fn the_grass_field_test_map_grows_grass_on_a_field_100_m_square_and_nothing_solid_but_its_ground() {
    // Basis: Rule (#338: a test Map with a 100 × 100 m grassy field; grass is
    // not a collider).
    assert!(test_map_ids().contains(&"test/grass-field".to_string()));
    let map = test_map("grass-field").unwrap().unwrap();
    let field: Vec<_> = map.drawing.iter().filter(|p| p.scatter.is_some()).collect();
    assert_eq!(field.len(), 1);
    let xs = field[0].corners.iter().map(|c| c[0]);
    let ys = field[0].corners.iter().map(|c| c[1]);
    let span = |v: Vec<f32>| {
        v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert_eq!(span(xs.collect()), 100.0);
    assert_eq!(span(ys.collect()), 100.0);
    // Only the 200 m ground slab is solid, as on the flat floor.
    assert_eq!(map.shapes, test_map("flat-floor").unwrap().unwrap().shapes);
}
