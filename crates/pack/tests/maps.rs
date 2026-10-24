//! Readable checks for the Pack checker's Maps (#63): a Map's folder goes in,
//! and its solid parts, its Launch Spot and what to draw come out, or every
//! problem with its file and the object it is about. Each file is built here
//! in plain Rust (`common/glb.rs`), so each check shows exactly what it
//! broke. Basis: Rule, from #16 §7, #17's rules for every Map and ADR-0011.

mod common;

use common::glb::{Glb, box_mesh, sheet_mesh};
use common::{Fixture, MAP_GLB, MAP_PREVIEW, MAP_TOML, YARD_TOML, symbolic_link};
use opendrone_maths::Vec3;
use opendrone_pack::{MapDefinition, read_map_file};
use opendrone_physics::MapShape;
use serde_json::json;

/// A small good Map: ground, a rail, a sheet, a painted-on mural that
/// nothing can touch, a hidden solid and a Launch Spot.
fn yard() -> Glb {
    Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .object(
            "rail",
            json!({ "od_collider": "convex" }),
            box_mesh([5.0, 0.0, 0.5], [0.1, 4.0, 0.1]),
            "steel_painted",
        )
        .object(
            "bowl",
            json!({ "od_collider": "trimesh" }),
            sheet_mesh([10.0, 0.0, -1.0], 4.0),
            "concrete_smooth",
        )
        .object(
            "mural",
            json!({ "od_collider": "none" }),
            sheet_mesh([0.0, 8.0, 3.0], 2.0),
            "backdrop",
        )
        .object(
            "hidden_wall",
            json!({ "od_collider": "box", "od_collision_only": 1 }),
            box_mesh([-8.0, 0.0, 1.0], [0.5, 6.0, 2.0]),
            "backdrop",
        )
        .launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0)
}

/// The problems with a Map built from `glb`, in a scratch folder of its own
/// (the test's name, so the checks can run side by side).
fn problems_of(glb: Glb) -> Vec<String> {
    let test = std::thread::current()
        .name()
        .unwrap_or("map")
        .replace("::", "-");
    Fixture::new(&format!("map-problems-{test}"))
        .with_map(glb.build())
        .map_problems()
}

fn yard_map(case: &str) -> MapDefinition {
    Fixture::new(case)
        .with_map(yard().build())
        .packs()
        .map("fixture/yard")
        .unwrap_or_else(|p| panic!("the yard should pass the checker:\n{p}"))
}

#[test]
fn a_good_map_passes_and_is_named_pack_slash_folder() {
    let fixture = Fixture::new("map-good").with_map(yard().build());
    assert_eq!(fixture.problems(), Vec::<String>::new());
    let packs = fixture.packs();
    let ids: Vec<&str> = packs.maps().iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["fixture/yard"]);
    let map = packs.map("fixture/yard").unwrap();
    assert_eq!(map.name, "Yard");
    assert_eq!(map.description, "A small yard to check Maps with.");
    assert_eq!((map.world.gravity, map.world.air_density), (9.81, 1.225));
    assert_eq!(map.model.as_deref(), Some("yard.glb"));
    assert_eq!(map.preview.as_deref(), Some("preview.png"));
    assert_eq!(map.background_sound, None);
}

#[test]
fn the_solid_parts_come_in_the_files_order_each_with_its_objects_name() {
    let map = yard_map("map-solids");
    // The mural is drawn but nothing touches it, so it has no solid.
    assert_eq!(map.solid_names, ["ground", "rail", "bowl", "hidden_wall"]);
    assert!(matches!(map.shapes[0], MapShape::Box { .. }));
    assert!(matches!(map.shapes[1], MapShape::Convex { .. }));
    assert!(matches!(map.shapes[2], MapShape::TriangleMesh { .. }));
    assert!(matches!(map.shapes[3], MapShape::Box { .. }));
}

#[test]
fn what_the_game_draws_is_every_part_that_isnt_collision_only_with_its_material() {
    let map = yard_map("map-drawing");
    let names: Vec<&str> = map.drawing.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["ground", "rail", "bowl", "mural"]);
    assert_eq!(map.drawing[0].material, "concrete_plaza");
    assert_eq!(map.drawing[0].colour, [0.4, 0.4, 0.38, 1.0]);
    assert_eq!(map.drawing[0].triangles.len(), 12);
}

#[test]
fn the_launch_spot_is_where_its_empty_stands_facing_its_compass_heading() {
    let map = yard_map("map-launch-spot");
    let spot = map.launch_spot.unwrap();
    // glTF's axes were turned back into east, north and up.
    assert_eq!(spot.position, Vec3::new(0.0, -3.0, 0.0));
    assert_eq!(spot.heading, 90.0);
}

#[test]
fn a_box_is_found_from_its_eight_corners_and_a_level_one_has_no_turn() {
    let map = yard_map("map-box");
    let MapShape::Box {
        centre,
        size,
        attitude,
    } = &map.shapes[0]
    else {
        panic!("the ground is a box")
    };
    assert_eq!(*centre, Vec3::new(0.0, 0.0, -1.0));
    assert_eq!(*size, Vec3::new(100.0, 100.0, 2.0));
    assert_eq!(attitude.quaternion(), [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_box_turned_to_a_heading_is_a_box_with_that_turn() {
    // A bench 5 m by 0.5 m, turned clockwise 30° seen from above.
    let heading = 30f64.to_radians();
    let (sin, cos) = heading.sin_cos();
    let (mut corners, triangles) = box_mesh([0.0, 0.0, 0.0], [5.0, 0.5, 0.45]);
    for c in &mut corners {
        let (x, y) = (f64::from(c[0]), f64::from(c[1]));
        c[0] = (x * cos + y * sin) as f32;
        c[1] = (-x * sin + y * cos) as f32;
    }
    let glb = yard().object(
        "bench",
        json!({ "od_collider": "box" }),
        (corners, triangles),
        "concrete_smooth",
    );
    let map = Fixture::new("map-turned-box")
        .with_map(glb.build())
        .packs()
        .map("fixture/yard")
        .unwrap();
    let Some(MapShape::Box { size, attitude, .. }) = map.shapes.last() else {
        panic!("the bench is a box")
    };
    assert!(
        (size.x - 5.0).abs() < 1e-4 && (size.y - 0.5).abs() < 1e-4 && (size.z - 0.45).abs() < 1e-4
    );
    // Its own forward points east turned 30° clockwise: (cos 30°, -sin 30°).
    let forward = attitude.body_to_world(Vec3::new(1.0, 0.0, 0.0));
    assert!(
        (forward.x - cos).abs() < 1e-4 && (forward.y + sin).abs() < 1e-4 && forward.z.abs() < 1e-6
    );
}

#[test]
fn a_box_turned_a_quarter_is_still_level_with_its_sides_swapped() {
    // The same bench turned 90°: it lies north to south, so it still has no
    // turn, and its size reads east, north and up.
    let (mut corners, triangles) = box_mesh([0.0, 0.0, 0.0], [5.0, 0.5, 0.45]);
    for c in &mut corners {
        *c = [c[1], -c[0], c[2]];
    }
    let glb = yard().object(
        "bench",
        json!({ "od_collider": "box" }),
        (corners, triangles),
        "concrete_smooth",
    );
    let map = Fixture::new("map-quarter-box")
        .with_map(glb.build())
        .packs()
        .map("fixture/yard")
        .unwrap();
    let Some(MapShape::Box { size, attitude, .. }) = map.shapes.last() else {
        panic!("the bench is a box")
    };
    assert!((size.x - 0.5).abs() < 1e-6 && (size.y - 5.0).abs() < 1e-6);
    assert_eq!(attitude.quaternion(), [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_maps_fingerprint_covers_its_parts_and_its_launch_spot_but_not_their_names() {
    let a = yard_map("map-fingerprint-a");
    assert_eq!(a.fingerprint(), yard_map("map-fingerprint-b").fingerprint());
    let mut renamed = a.clone();
    renamed.solid_names[0] = "floor".to_string();
    renamed.description = "Another sentence.".to_string();
    assert_eq!(renamed.fingerprint(), a.fingerprint());
    let mut moved = a.clone();
    moved.launch_spot.as_mut().unwrap().heading = 91.0;
    assert_ne!(moved.fingerprint(), a.fingerprint());
    let mut without = a.clone();
    without.launch_spot = None;
    assert_ne!(without.fingerprint(), a.fingerprint());
}

// Every problem, with the file and the object

#[test]
fn an_object_with_no_collider_tag_is_named_with_the_file() {
    let glb = yard().object(
        "planter",
        json!({}),
        box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"planter\" has no od_collider tag: every object says whether it is solid, with \"box\", \"convex\", \"trimesh\" or \"none\" (none for a part nothing can touch)"
        )]
    );
}

#[test]
fn an_unknown_od_tag_is_refused_so_a_misspelt_one_never_passes_for_nothing() {
    let glb = yard().object(
        "planter",
        json!({ "od_collider": "box", "od_colision_only": 1 }),
        box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"planter\" has the tag \"od_colision_only\", which OpenDrone doesn't know: the tags that start with od_ are od_collider, od_collision_only, od_launch_spot, od_decal, od_blend"
        )]
    );
}

#[test]
fn tags_that_dont_start_with_od_are_nobodys_business() {
    let glb = yard().object(
        "planter",
        json!({ "od_collider": "box", "rigid_body": "static" }),
        box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
        "concrete_smooth",
    );
    assert_eq!(problems_of(glb), Vec::<String>::new());
}

#[test]
fn a_collider_that_isnt_box_convex_trimesh_or_none_is_refused() {
    let glb = yard().object(
        "dome",
        json!({ "od_collider": "sphere" }),
        box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"dome\" has od_collider \"sphere\", which isn't one of \"box\", \"convex\", \"trimesh\" or \"none\""
        )]
    );
}

#[test]
fn a_map_with_no_launch_spot_is_refused() {
    let glb = Glb::new().object(
        "ground",
        json!({ "od_collider": "box" }),
        box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
        "concrete_plaza",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: the Map has no Launch Spot: exactly one object must carry the od_launch_spot tag, an empty at the spot with the compass heading the Quad faces as its value"
        )]
    );
}

#[test]
fn a_map_with_two_launch_spots_is_refused_naming_both() {
    let glb = yard().launch_spot("second_spot", [4.0, 4.0, 0.0], 0.0);
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: the Map has 2 Launch Spots (\"launch_spot\", \"second_spot\"), and exactly one is allowed"
        )]
    );
}

#[test]
fn a_launch_spot_heading_must_be_a_compass_heading() {
    let glb = Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .launch_spot("launch_spot", [0.0, 0.0, 0.0], 400.0);
    let problems = problems_of(glb);
    assert!(
        problems[0].contains("object \"launch_spot\" has od_launch_spot 400.0, which must be a number: the compass heading"),
        "{problems:?}"
    );
}

#[test]
fn a_compressed_mesh_is_refused() {
    for (extension, what) in [
        ("KHR_draco_mesh_compression", "Draco compression"),
        ("EXT_meshopt_compression", "meshopt compression"),
        ("KHR_mesh_quantization", "quantised (whole-number) corners"),
    ] {
        let problems = problems_of(yard().uses_extension(extension));
        assert_eq!(
            problems,
            [format!(
                "{MAP_GLB}: uses {extension} ({what}): no compressed meshes, because every program must read the very same corners; export the Map with mesh compression off"
            )]
        );
    }
}

#[test]
fn a_file_that_isnt_a_glb_is_refused() {
    let fixture = Fixture::new("map-not-glb").with_map(b"this is not a model".to_vec());
    assert_eq!(
        fixture.map_problems(),
        [format!(
            "{MAP_GLB}: isn't a binary glTF file (.glb): export the Map as GLB"
        )]
    );
    let mut cut = yard().build();
    cut.truncate(cut.len() - 40);
    let fixture = Fixture::new("map-cut-short").with_map(cut);
    assert_eq!(
        fixture.map_problems(),
        [format!(
            "{MAP_GLB}: its length doesn't match its header: the file is cut short or has extra bytes"
        )]
    );
}

#[test]
fn a_box_tag_on_something_that_isnt_a_box_is_refused() {
    // A sheet has four corners, and a rail with a corner cut off has seven.
    let glb = yard().object(
        "slab",
        json!({ "od_collider": "box" }),
        sheet_mesh([3.0, 3.0, 0.0], 1.0),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"slab\" is tagged box, but its mesh isn't a box: it has 4 corners, and a box has 8"
        )]
    );
    let (mut corners, triangles) = box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]);
    corners[7] = [3.2, 3.2, 1.2];
    let glb = yard().object(
        "wedge",
        json!({ "od_collider": "box" }),
        (corners, triangles),
        "concrete_smooth",
    );
    let problems = problems_of(glb);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].starts_with(&format!(
        "{MAP_GLB}: object \"wedge\" is tagged box, but its mesh isn't a box: its corners"
    )));
}

#[test]
fn a_convex_tag_on_an_open_mesh_or_a_dented_one_is_refused() {
    let glb = yard().object(
        "fan",
        json!({ "od_collider": "convex" }),
        sheet_mesh([3.0, 3.0, 0.0], 1.0),
        "concrete_smooth",
    );
    let problems = problems_of(glb);
    assert_eq!(
        problems,
        [format!(
            "{MAP_GLB}: object \"fan\" is tagged convex, but a convex solid needs at least four triangles that enclose some volume"
        )]
    );
    // A box with one corner pushed in: closed, but dented.
    let (mut corners, triangles) = box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]);
    corners[7] = [3.2, 3.2, 0.7];
    let glb = yard().object(
        "dent",
        json!({ "od_collider": "convex" }),
        (corners, triangles),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"dent\" is tagged convex, but its mesh isn't convex, or its triangles don't face outward: tag it trimesh, or split it into convex parts"
        )]
    );
}

#[test]
fn a_part_that_is_neither_drawn_nor_solid_is_refused() {
    let glb = yard().object(
        "ghost",
        json!({ "od_collider": "none", "od_collision_only": 1 }),
        box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
        "concrete_smooth",
    );
    assert_eq!(
        problems_of(glb),
        [format!(
            "{MAP_GLB}: object \"ghost\" is od_collision_only but its od_collider is \"none\": it would be neither drawn nor solid"
        )]
    );
}

#[test]
fn every_problem_is_listed_at_once_each_with_its_object() {
    let glb = yard()
        .object(
            "a",
            json!({}),
            box_mesh([3.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
            "concrete_smooth",
        )
        .object(
            "b",
            json!({ "od_collider": "ball" }),
            box_mesh([6.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
            "concrete_smooth",
        )
        .object(
            "c",
            json!({ "od_typo": true }),
            box_mesh([9.0, 3.0, 0.5], [1.0, 1.0, 1.0]),
            "concrete_smooth",
        )
        .launch_spot("another", [1.0, 1.0, 0.0], 10.0);
    let problems = problems_of(glb);
    assert_eq!(problems.len(), 5, "{problems:#?}");
    for object in ["\"a\"", "\"b\"", "\"c\"", "2 Launch Spots"] {
        assert!(
            problems.iter().any(|p| p.contains(object)),
            "{object} in {problems:#?}"
        );
    }
    assert!(problems.iter().all(|p| p.starts_with(MAP_GLB)));
}

// map.toml and the files it names

#[test]
fn map_toml_must_name_the_model_and_the_preview() {
    let fixture = Fixture::new("map-toml-names").with_map(yard().build());
    fixture.write(
        MAP_TOML,
        &YARD_TOML
            .replace("model       = \"yard.glb\"\n", "")
            .replace("preview     = \"preview.png\"\n", ""),
    );
    assert_eq!(
        fixture.map_problems(),
        [
            format!(
                "{MAP_TOML}: every Map names its .glb with `model`, such as model = \"skate-park.glb\", but this one doesn't"
            ),
            format!(
                "{MAP_TOML}: every Map names its preview picture with `preview`, such as preview = \"preview.png\", but this one doesn't"
            ),
        ]
    );
}

#[test]
fn the_model_and_preview_must_be_files_in_the_maps_own_folder() {
    let fixture = Fixture::new("map-toml-files").with_map(yard().build());
    fixture.write(MAP_TOML, &YARD_TOML.replace("yard.glb", "../elsewhere.glb"));
    assert_eq!(
        fixture.map_problems(),
        [format!(
            "{MAP_TOML}: `model` names a .glb file in the Map's own folder, such as \"map.glb\", not \"../elsewhere.glb\""
        )]
    );
    fixture.write(MAP_TOML, &YARD_TOML.replace("yard.glb", "missing.glb"));
    assert_eq!(
        fixture.map_problems(),
        [
            "packs/fixture/maps/yard/missing.glb: doesn't exist, but map.toml's `model` names it"
                .to_string()
        ]
    );
    fixture.write(MAP_TOML, YARD_TOML);
    fixture.write_bytes(MAP_PREVIEW, b"not a picture");
    assert_eq!(
        fixture.map_problems(),
        [format!("{MAP_PREVIEW}: isn't a PNG picture")]
    );
}

#[test]
fn an_unknown_key_in_map_toml_is_refused_with_its_line() {
    let fixture = Fixture::new("map-toml-unknown").with_map(yard().build());
    fixture.write(
        MAP_TOML,
        &YARD_TOML.replace("[world]", "wind = \"3 m/s\"\n\n[world]"),
    );
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with(&format!(
        "{MAP_TOML} line 7: `wind` isn't something OpenDrone reads"
    )));
}

#[test]
fn the_background_sound_slot_takes_an_ogg_file_in_the_folder_and_a_level_in_decibels() {
    let fixture = Fixture::new("map-background-sound").with_map(yard().build());
    let with_sound =
        format!("{YARD_TOML}\n[background_sound]\nfile  = \"wind.ogg\"\nlevel = \"-12 dB\"\n");
    fixture.write(MAP_TOML, &with_sound);
    // The file is missing.
    assert_eq!(
        fixture.map_problems(),
        ["packs/fixture/maps/yard/wind.ogg: doesn't exist, but map.toml's `background_sound.file` names it".to_string()]
    );
    // Not an Ogg file.
    fixture.write_bytes("packs/fixture/maps/yard/wind.ogg", b"RIFF....WAVE");
    assert_eq!(
        fixture.map_problems(),
        ["packs/fixture/maps/yard/wind.ogg: isn't an Ogg sound file".to_string()]
    );
    // A real one.
    fixture.write_bytes("packs/fixture/maps/yard/wind.ogg", b"OggS\0\x02rest");
    let map = fixture.packs().map("fixture/yard").unwrap();
    let sound = map.background_sound.unwrap();
    assert_eq!((sound.file.as_str(), sound.level_db), ("wind.ogg", -12.0));
    // A level that isn't in decibels, or is absurd.
    fixture.write(MAP_TOML, &with_sound.replace("-12 dB", "12 m"));
    assert!(
        fixture.map_problems()[0].contains("a level in decibels"),
        "{:?}",
        fixture.map_problems()
    );
    fixture.write(MAP_TOML, &with_sound.replace("-12 dB", "-90 dB"));
    assert!(fixture.map_problems()[0].contains("must be from -60 dB to 12 dB"));
}

/// What a Map's `[light]` looks like when it is right: a sun 37 ° up in the
/// south-west, and a blue sky.
const LIGHT: &str = "\n[light]\nsun_elevation = \"37 °\"\nsun_azimuth   = \"225 °\"\nsun_colour    = \"#fff2d6\"\nsun_strength  = \"3.0\"\nsky_colour    = \"#a5c4f0\"\nsky_strength  = \"0.5\"\n";

#[test]
fn a_map_without_a_light_table_has_no_baked_light() {
    assert_eq!(yard_map("map-no-light").light, None);
}

/// The first bytes of an OpenEXR picture, which is all the checker looks at.
const ATLAS: &[u8] = &[0x76, 0x2f, 0x31, 0x01, 0, 0, 0, 0];
const ATLAS_FILE: &str = "packs/fixture/maps/yard/yard.light.exr";

/// A Map whose ground has a second set of coordinates into a light atlas.
fn baked_yard() -> Glb {
    Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .light_uvs(&[[0.0, 0.0]; 8])
        .launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0)
}

/// A baked Map as it is when right: `[light]`, the coordinates and the atlas.
fn baked_fixture(case: &str) -> Fixture {
    let fixture = Fixture::new(case).with_map(baked_yard().build());
    fixture.write(MAP_TOML, &format!("{YARD_TOML}{LIGHT}"));
    fixture.write_bytes(ATLAS_FILE, ATLAS);
    fixture
}

#[test]
fn a_map_with_light_coordinates_an_atlas_and_a_light_table_passes_and_names_its_atlas() {
    let fixture = baked_fixture("map-baked");
    assert_eq!(fixture.problems(), Vec::<String>::new());
    let map = fixture.packs().map("fixture/yard").unwrap();
    assert!(map.light.is_some());
    assert!(
        map.light_atlas
            .unwrap()
            .ends_with("maps/yard/yard.light.exr")
    );
    assert_eq!(map.drawing[0].light_uvs.len(), 8);
}

#[test]
fn a_map_without_light_has_no_atlas_and_no_light_coordinates() {
    let map = yard_map("map-unbaked");
    assert_eq!(map.light_atlas, None);
    assert!(map.drawing.iter().all(|part| part.light_uvs.is_empty()));
}

#[test]
fn a_map_with_light_coordinates_but_no_atlas_is_refused_in_plain_words() {
    let fixture = baked_fixture("map-no-atlas");
    fixture.remove(ATLAS_FILE);
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("has light atlas coordinates (TEXCOORD_1)")
            && problems[0].contains("has no yard.light.exr"),
        "{problems:?}"
    );
}

#[test]
fn a_map_with_an_atlas_but_no_light_coordinates_is_refused_in_plain_words() {
    let fixture = Fixture::new("map-no-coordinates").with_map(yard().build());
    fixture.write(MAP_TOML, &format!("{YARD_TOML}{LIGHT}"));
    fixture.write_bytes(ATLAS_FILE, ATLAS);
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("yard.light.exr: is a light atlas, but no mesh")
            && problems[0].contains("TEXCOORD_1"),
        "{problems:?}"
    );
}

#[test]
fn a_baked_map_with_no_light_table_is_refused_because_its_quads_need_the_sky() {
    let fixture = baked_fixture("map-baked-no-table");
    fixture.write(MAP_TOML, YARD_TOML);
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("needs a `[light]` table too"),
        "{problems:?}"
    );
}

#[test]
fn a_light_table_with_nothing_baked_is_refused_and_says_to_bake_or_remove_it() {
    let fixture = Fixture::new("map-table-only").with_map(yard().build());
    fixture.write(MAP_TOML, &format!("{YARD_TOML}{LIGHT}"));
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("bake the Map, or remove `[light]`"),
        "{problems:?}"
    );
}

#[test]
fn an_atlas_that_isnt_an_openexr_picture_or_has_another_name_is_refused() {
    let fixture = baked_fixture("map-atlas-wrong");
    fixture.write_bytes(ATLAS_FILE, b"not a picture");
    assert!(fixture.map_problems()[0].contains("isn't an OpenEXR picture"));
    fixture.remove(ATLAS_FILE);
    fixture.write_bytes("packs/fixture/maps/yard/yard.light.png", ATLAS);
    let problems = fixture.map_problems().join("\n");
    assert!(
        problems.contains("an OpenEXR picture named yard.light.exr, not yard.light.png"),
        "{problems}"
    );
}

#[test]
fn an_object_left_without_light_coordinates_in_a_baked_map_is_named_and_refused() {
    let glb = baked_yard().object(
        "mural",
        json!({ "od_collider": "none" }),
        sheet_mesh([0.0, 8.0, 3.0], 2.0),
        "backdrop",
    );
    let fixture = Fixture::new("map-unlit-object").with_map(glb.build());
    fixture.write(MAP_TOML, &format!("{YARD_TOML}{LIGHT}"));
    fixture.write_bytes(ATLAS_FILE, ATLAS);
    let problems = fixture.map_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("the object \"mural\" in the .glb has no light atlas coordinates"),
        "{problems:?}"
    );
}

#[test]
fn light_coordinates_that_are_not_one_for_every_corner_are_refused() {
    let glb = Glb::new()
        .object(
            "ground",
            json!({ "od_collider": "box" }),
            box_mesh([0.0, 0.0, -1.0], [100.0, 100.0, 2.0]),
            "concrete_plaza",
        )
        .light_uvs(&[[0.0, 0.0]; 3])
        .launch_spot("launch_spot", [0.0, -3.0, 0.0], 90.0);
    let fixture = Fixture::new("map-short-coordinates").with_map(glb.build());
    fixture.write(MAP_TOML, &format!("{YARD_TOML}{LIGHT}"));
    fixture.write_bytes(ATLAS_FILE, ATLAS);
    let problems = fixture.map_problems().join("\n");
    assert!(
        problems.contains("aren't one for every corner"),
        "{problems}"
    );
}

#[test]
fn the_light_table_gives_the_suns_direction_and_colour_and_the_skys() {
    let fixture = baked_fixture("map-light");
    let light = fixture.packs().map("fixture/yard").unwrap().light.unwrap();
    assert!((light.sun_elevation - 37.0_f64.to_radians()).abs() < 1e-12);
    assert!((light.sun_azimuth - 225.0_f64.to_radians()).abs() < 1e-12);
    assert_eq!(light.sun_colour, [0xff, 0xf2, 0xd6]);
    assert_eq!(light.sun_strength, 3.0);
    assert_eq!(light.sky_colour, [0xa5, 0xc4, 0xf0]);
    assert_eq!(light.sky_strength, 0.5);
}

#[test]
fn a_light_table_that_leaves_out_a_value_or_has_a_wrong_one_is_refused_in_plain_words() {
    let fixture = Fixture::new("map-light-wrong").with_map(yard().build());
    let refused = |light: String| {
        fixture.write(MAP_TOML, &format!("{YARD_TOML}{light}"));
        fixture.map_problems().join("\n")
    };
    let without = LIGHT.replace("sky_strength  = \"0.5\"\n", "");
    assert!(
        refused(without).contains("is missing `sky_strength`"),
        "a value left out"
    );
    let low = LIGHT.replace("37 °", "-5 °");
    assert!(
        refused(low)
            .contains("`light.sun_elevation` must be from 0 ° (on the horizon) to 90 ° (overhead)"),
        "a sun under the horizon"
    );
    let radians = LIGHT.replace("225 °", "2 rad");
    assert!(
        refused(radians).contains("radians"),
        "an angle that isn't in degrees"
    );
    let colour = LIGHT.replace("#fff2d6", "orange");
    assert!(
        refused(colour).contains("`light.sun_colour` is a colour written like \"#fff2d6\""),
        "a colour that isn't hex"
    );
    let strength = LIGHT.replace("\"3.0\"", "\"-1\"");
    assert!(
        refused(strength).contains("`light.sun_strength` must be 0 or more"),
        "a negative strength"
    );
    let unknown = LIGHT.replace("[light]\n", "[light]\nmoon = \"1\"\n");
    assert!(
        refused(unknown).contains("`moon` isn't something OpenDrone reads in [light]"),
        "an unknown key"
    );
}

#[test]
fn a_broken_map_is_skipped_and_the_rest_of_the_pack_still_loads() {
    let fixture = Fixture::new("map-skipped").with_map(b"nonsense".to_vec());
    let packs = fixture.packs();
    assert!(packs.map("fixture/yard").is_err());
    assert!(packs.maps().is_empty());
    assert!(packs.quad("fixture/ducted").is_ok());
}

#[test]
fn a_map_folder_must_have_a_map_toml_and_an_id_for_a_name() {
    let fixture = Fixture::new("map-folders").with_map(yard().build());
    fixture.remove(MAP_TOML);
    assert_eq!(
        fixture.problems(),
        [
            "packs/fixture/maps/yard: every Map's folder holds a map.toml, but this one has none"
                .to_string()
        ]
    );
    let fixture = Fixture::new("map-folder-name").with_map(yard().build());
    fixture.write("packs/fixture/maps/Big Yard/map.toml", YARD_TOML);
    assert_eq!(
        fixture.problems(),
        ["packs/fixture/maps/Big Yard: the folder's name is the Map's id, so \"Big Yard\" must be lowercase words joined by dashes, such as \"skate-park\"".to_string()]
    );
}

#[test]
fn asking_for_a_map_that_isnt_there_says_which_pack_and_folder() {
    let fixture = Fixture::new("map-missing");
    let packs = fixture.packs();
    let missing = packs.map("fixture/nowhere").unwrap_err().to_string();
    assert_eq!(
        missing,
        "fixture/nowhere: there's no Map here: packs/fixture/maps/nowhere/map.toml doesn't exist"
    );
    let nobody = packs.map("nobody/yard").unwrap_err().to_string();
    assert!(
        nobody.contains("there's no Pack with the id \"nobody\""),
        "{nobody}"
    );
    let test = packs.map("test/nowhere").unwrap_err().to_string();
    assert!(test.contains("the built-in Test Maps are test/"), "{test}");
}

#[test]
fn a_map_file_that_is_a_symbolic_link_is_refused_rather_than_followed() {
    let fixture = Fixture::new("map-link").with_map(yard().build());
    fixture.write_bytes("packs/fixture/maps/yard/real.glb", &yard().build());
    fixture.remove(MAP_GLB);
    let Some(()) = symbolic_link("real.glb", &fixture.root.join(MAP_GLB)) else {
        return;
    };
    let problems = fixture.problems();
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with(MAP_GLB) && p.contains("is a symbolic link")),
        "{problems:?}"
    );
}

#[test]
fn a_test_maps_file_reads_the_new_keys_as_nothing_and_still_refuses_unknown_ones() {
    // The Test Maps have no model and no preview, and keep reading as before.
    let map = read_map_file(
        "test/flat-floor",
        "flat-floor.toml",
        include_str!("../test-maps/flat-floor.toml"),
    )
    .unwrap();
    assert_eq!(
        (map.model, map.preview, map.launch_spot),
        (None, None, None)
    );
    let broken = format!(
        "{}wind = \"3 m/s\"\n",
        include_str!("../test-maps/flat-floor.toml")
    )
    .replace("[world]", "");
    assert!(read_map_file("test/flat-floor", "flat-floor.toml", &broken).is_err());
}
