//! Readable checks for the Pack checker's decals and vertex-colour blends
//! (#321): a decal (`od_decal`) is a picture lying on a surface, only drawn,
//! with its picture coordinates from 0 to 1; a blend (`od_blend` and
//! `COLOR_0`) names a second material and says, at each corner, how much of
//! it shows. Each `.glb` is built here in plain Rust (`common/glb.rs`), so
//! each check shows exactly what it broke. Basis: Rule, from #321 and
//! docs/verification/checking-a-pack.md.

mod common;

use common::glb::{Glb, box_mesh, sheet_mesh};
use common::{Fixture, MAP_TOML, YARD_TOML};
use opendrone_pack::{Blend, MapDefinition};
use serde_json::{Value, json};

/// The four corners of `sheet_mesh`, as a decal's picture coordinates: the
/// whole picture across the sheet.
const WHOLE_PICTURE: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

/// Ground, a wall and a Launch Spot: what every check here adds to.
fn ground() -> Glb {
    Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .object(
            "wall",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 6.0, 2.0], [8.0, 0.3, 4.0]),
            "brick",
        )
}

fn finished(glb: Glb) -> Glb {
    glb.launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0)
}

/// A decal on the ground with these tags and picture coordinates.
fn with_decal(tags: Value, uvs: &[[f32; 2]]) -> Glb {
    finished(
        ground()
            .object(
                "damp_patch",
                tags,
                sheet_mesh([2.0, 0.0, 0.002], 1.5),
                "decal_damp",
            )
            .uvs(uvs),
    )
}

/// The ground's grass with dirt showing through where its colours say.
fn with_blend(tags: Value, colours: &[[f32; 4]]) -> Glb {
    finished(
        ground()
            .object(
                "yard",
                tags,
                sheet_mesh([0.0, -10.0, 0.0], 6.0),
                "grass_weeds",
            )
            .colours(colours),
    )
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

#[test]
fn a_decal_is_drawn_with_its_layer_and_is_nothing_solid() {
    let map = map_of(
        "decal-good",
        with_decal(
            json!({ "od_collider": "none", "od_decal": 3 }),
            &WHOLE_PICTURE,
        ),
    );
    let decal = map.drawing.iter().find(|p| p.name == "damp_patch").unwrap();
    assert_eq!(decal.decal, Some(3));
    assert_eq!(decal.blend, None);
    assert_eq!(decal.uvs, WHOLE_PICTURE);
    assert_eq!(map.solid_names, ["ground", "wall"]);
    // Every other part is no decal.
    assert!(
        map.drawing
            .iter()
            .filter(|p| p.name != "damp_patch")
            .all(|p| p.decal.is_none())
    );
}

#[test]
fn a_decals_layer_is_a_whole_number_from_0_to_15() {
    for bad in [json!(16), json!(-1), json!(1.5), json!("top")] {
        one_problem(
            "decal-layer",
            with_decal(
                json!({ "od_collider": "none", "od_decal": bad }),
                &WHOLE_PICTURE,
            ),
            "must be a whole number from 0 to 15",
        );
    }
    for good in [json!(0), json!(15), json!(7.0)] {
        assert_eq!(
            problems_of(
                "decal-layer-good",
                with_decal(
                    json!({ "od_collider": "none", "od_decal": good }),
                    &WHOLE_PICTURE
                )
            ),
            Vec::<String>::new()
        );
    }
}

#[test]
fn a_solid_decal_is_refused_because_a_decal_is_only_drawn() {
    one_problem(
        "decal-solid",
        with_decal(
            json!({ "od_collider": "trimesh", "od_decal": 0 }),
            &WHOLE_PICTURE,
        ),
        "object \"damp_patch\" is a decal (od_decal) but its od_collider is \"trimesh\": a decal is a picture lying on a surface and is never solid",
    );
}

#[test]
fn a_decal_whose_picture_would_repeat_is_refused_because_its_rectangle_would_show() {
    let repeating = [[0.0, 2.0], [2.0, 2.0], [2.0, 0.0], [0.0, 0.0]];
    one_problem(
        "decal-repeats",
        with_decal(json!({ "od_collider": "none", "od_decal": 0 }), &repeating),
        "outside 0 to 1 the picture would repeat, and its edge show as a rectangle",
    );
}

#[test]
fn a_decal_with_no_picture_coordinates_is_refused() {
    let glb = finished(ground().object(
        "stain",
        json!({ "od_collider": "none", "od_decal": 0 }),
        sheet_mesh([2.0, 0.0, 0.002], 1.5),
        "decal_stain",
    ));
    one_problem(
        "decal-no-uvs",
        glb,
        "is a decal (od_decal), so its mesh needs picture coordinates (TEXCOORD_0)",
    );
}

#[test]
fn a_decal_cant_also_blend_two_materials() {
    let glb = finished(
        ground()
            .object(
                "stain",
                json!({ "od_collider": "none", "od_decal": 0, "od_blend": "brick" }),
                sheet_mesh([2.0, 0.0, 0.002], 1.5),
                "decal_stain",
            )
            .uvs(&WHOLE_PICTURE)
            .colours(&[[0.5, 0.5, 0.5, 1.0]; 4]),
    );
    one_problem(
        "decal-blend",
        glb,
        "is a decal (od_decal) and has od_blend: a decal is one picture, with nothing to blend",
    );
}

#[test]
fn a_blend_gives_the_second_material_and_its_weight_at_every_corner() {
    let colours = [
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        [0.5, 0.5, 0.5, 1.0],
        [0.2, 0.2, 0.2, 1.0],
    ];
    let map = map_of(
        "blend-good",
        with_blend(
            json!({ "od_collider": "trimesh", "od_blend": "ground_dirt_rubble" }),
            &colours,
        ),
    );
    let yard = map.drawing.iter().find(|p| p.name == "yard").unwrap();
    assert_eq!(
        yard.blend,
        Some(Blend {
            material: "ground_dirt_rubble".to_string(),
            weights: vec![0.0, 1.0, 0.5, 0.2],
        })
    );
    // The blend is drawn only: the yard is as solid as it would be without it.
    assert_eq!(map.solid_names, ["ground", "wall", "yard"]);
    assert!(
        map.drawing
            .iter()
            .filter(|p| p.name != "yard")
            .all(|p| p.blend.is_none())
    );
}

#[test]
fn a_blend_reads_vertex_colours_written_as_whole_numbers_too() {
    let glb = finished(
        ground()
            .object(
                "yard",
                json!({ "od_collider": "trimesh", "od_blend": "ground_dirt_rubble" }),
                sheet_mesh([0.0, -10.0, 0.0], 6.0),
                "grass_weeds",
            )
            .byte_colours(&[[0, 0, 0], [255, 255, 255], [51, 51, 51], [255, 255, 255]]),
    );
    let map = map_of("blend-bytes", glb);
    let yard = map.drawing.iter().find(|p| p.name == "yard").unwrap();
    assert_eq!(yard.blend.as_ref().unwrap().weights, [0.0, 1.0, 0.2, 1.0]);
}

#[test]
fn vertex_colours_with_no_second_material_are_refused_in_plain_words() {
    one_problem(
        "blend-no-material",
        with_blend(
            json!({ "od_collider": "trimesh" }),
            &[[0.5, 0.5, 0.5, 1.0]; 4],
        ),
        "object \"yard\" has vertex colours (COLOR_0) but no od_blend",
    );
}

#[test]
fn a_second_material_with_no_vertex_colours_to_say_where_is_refused() {
    let glb = finished(ground().object(
        "yard",
        json!({ "od_collider": "trimesh", "od_blend": "ground_dirt_rubble" }),
        sheet_mesh([0.0, -10.0, 0.0], 6.0),
        "grass_weeds",
    ));
    one_problem(
        "blend-no-colours",
        glb,
        "names a material to blend with (od_blend), but its mesh has no vertex colours (COLOR_0) to say where it shows",
    );
}

#[test]
fn a_blend_weight_must_be_grey_from_0_to_1_and_one_for_every_corner() {
    let tags = json!({ "od_collider": "trimesh", "od_blend": "ground_dirt_rubble" });
    let tinted = [
        [1.0, 0.5, 0.5, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    one_problem(
        "blend-tinted",
        with_blend(tags.clone(), &tinted),
        "which isn't grey",
    );
    let too_much = [
        [1.5, 1.5, 1.5, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    one_problem(
        "blend-too-much",
        with_blend(tags.clone(), &too_much),
        "a blend weight is from 0 to 1",
    );
    one_problem(
        "blend-short",
        with_blend(tags, &[[0.0, 0.0, 0.0, 1.0]; 3]),
        "has vertex colours (COLOR_0) that aren't one for every corner",
    );
}

#[test]
fn od_blend_must_name_a_material() {
    for bad in [json!(""), json!(1)] {
        one_problem(
            "blend-bad-name",
            with_blend(
                json!({ "od_collider": "trimesh", "od_blend": bad }),
                &[[0.5, 0.5, 0.5, 1.0]; 4],
            ),
            "which must be the name of a material",
        );
    }
}

#[test]
fn a_map_with_decals_and_a_blend_has_the_same_fingerprint_as_without_them() {
    // Decals and blends are only drawn: what the Simulation meets is the same.
    let plain = Fixture::new("blend-fingerprint-plain")
        .with_map(finished(ground()).build())
        .packs()
        .map("fixture/yard")
        .unwrap();
    let dressed = map_of(
        "blend-fingerprint-dressed",
        ground()
            .object(
                "damp_patch",
                json!({ "od_collider": "none", "od_decal": 1 }),
                sheet_mesh([2.0, 0.0, 0.002], 1.5),
                "decal_damp",
            )
            .uvs(&WHOLE_PICTURE)
            .launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0),
    );
    assert_eq!(plain.shapes, dressed.shapes);
    assert_eq!(plain.launch_spot, dressed.launch_spot);
}

#[test]
fn the_map_toml_needs_nothing_new_for_decals_or_blends() {
    let fixture = Fixture::new("blend-toml").with_map(
        with_blend(
            json!({ "od_collider": "trimesh", "od_blend": "ground_dirt_rubble" }),
            &[[0.5, 0.5, 0.5, 1.0]; 4],
        )
        .build(),
    );
    fixture.write(MAP_TOML, YARD_TOML);
    assert_eq!(fixture.map_problems(), Vec::<String>::new());
}
