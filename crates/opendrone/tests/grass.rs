//! Readable checks for real grass (#338): where the game grows a Map's
//! clumps, that it grows the same ones every run, how it thins them with
//! distance, and the mesh it draws a cell of them with. Nothing here opens a
//! window: whether grass looks like grass is judged on the pictures
//! `--render-views` saves, which the PR shows.
//! Basis: Rule (the ticket: blade-level clumps of 6 to 12 blades, placed on
//! load from a fixed seed, drawn a cell at a time, thinning smoothly with
//! distance to none at the draw distance; existing Maps don't change).

mod common;

use bevy::mesh::{Indices, Mesh, VertexAttributeValues};
use opendrone::grass::{
    ATTRIBUTE_LOOK, ATTRIBUTE_ROOT, BLADES, CELL, DRAW_DISTANCE, FULL_UNTIL, TAPER_FROM,
    clumps_mesh, drawn_from, grow, grow_the_map, keep,
};
use opendrone_pack::{DrawnPart, GRASS_FIELD, GRASS_FIELD_DENSITY, Scatter, ScatterKind, test_map};

fn grass(density: f32) -> Scatter {
    Scatter {
        kind: ScatterKind::Grass,
        density,
        height: [0.12, 0.38],
    }
}

/// A flat square of ground `side` metres across, its south-west corner at
/// the origin, and a wall standing on its north edge.
fn yard(name: &str, side: f32) -> DrawnPart {
    DrawnPart {
        name: name.to_string(),
        material: "grass".to_string(),
        colour: [0.16, 0.26, 0.09, 1.0],
        corners: vec![
            [0.0, 0.0, 0.0],
            [side, 0.0, 0.0],
            [side, side, 0.0],
            [0.0, side, 0.0],
            // The wall: straight up from the north edge.
            [side, side, 3.0],
            [0.0, side, 3.0],
        ],
        uvs: Vec::new(),
        light_uvs: Vec::new(),
        triangles: vec![[0, 1, 2], [0, 2, 3], [3, 2, 4], [3, 4, 5]],
        decal: None,
        blend: None,
        scatter: Some(grass(6.0)),
    }
}

#[test]
fn a_ground_grows_as_many_clumps_as_its_area_and_density_make_and_a_wall_grows_none() {
    let part = yard("yard", 20.0);
    let clumps = grow(&part, &grass(6.0));
    // 400 m² at 6 a square metre, from its two flat triangles of 200 m² each.
    assert_eq!(clumps.len(), 2400);
    for clump in &clumps {
        let [east, north, up] = clump.at;
        assert!((0.0..=20.0).contains(&east) && (0.0..=20.0).contains(&north));
        assert_eq!(up, 0.0, "every root is on the ground, none on the wall");
        assert!((0.12..=0.38).contains(&clump.height));
        assert!((BLADES.0..=BLADES.1).contains(&clump.blades));
        assert!((0.0..1.0).contains(&clump.rank));
    }
}

#[test]
fn the_same_map_grows_the_same_grass_on_every_run_and_another_part_its_own() {
    let part = yard("yard", 20.0);
    assert_eq!(grow(&part, &grass(6.0)), grow(&part, &grass(6.0)));
    let other = yard("orchard", 20.0);
    assert_ne!(grow(&part, &grass(6.0)), grow(&other, &grass(6.0)));
}

#[test]
fn every_clump_is_drawn_near_the_camera_fewer_further_away_and_none_at_the_draw_distance() {
    assert_eq!(keep(0.0), 1.0);
    assert_eq!(keep(FULL_UNTIL), 1.0);
    assert_eq!(keep(DRAW_DISTANCE), 0.0);
    assert_eq!(keep(DRAW_DISTANCE + 10.0), 0.0);
    // It never rises with distance, and it has no step: between two points
    // 10 cm apart it moves by under 2% of all the clumps.
    let mut last = keep(0.0);
    for step in 1..=700 {
        let now = keep(step as f32 * 0.1);
        assert!(now <= last, "at {} m", step as f32 * 0.1);
        assert!(last - now < 0.02, "at {} m", step as f32 * 0.1);
        last = now;
    }
    // Halfway out, a share of them is left.
    assert!((0.3..0.6).contains(&keep(30.0)), "{}", keep(30.0));
}

#[test]
fn the_shader_thins_the_grass_with_the_same_numbers() {
    let shader = include_str!("../src/grass.wesl");
    for (name, value) in [
        ("FULL_UNTIL", FULL_UNTIL),
        ("TAPER_FROM", TAPER_FROM),
        ("DRAW_DISTANCE", DRAW_DISTANCE),
    ] {
        assert!(
            shader.contains(&format!("const {name}: f32 = {value:.1};")),
            "{name} should be {value} in grass.wesl"
        );
    }
}

fn floats4(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<[f32; 4]> {
    match mesh.attribute(attribute) {
        Some(VertexAttributeValues::Float32x4(v)) => v.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cell_is_one_mesh_of_five_corners_and_three_triangles_a_blade_each_corner_knowing_its_clump() {
    let part = yard("yard", 4.0);
    let clumps = grow(&part, &grass(6.0));
    let refs: Vec<_> = clumps.iter().collect();
    let mesh = clumps_mesh(&refs);
    let blades: usize = clumps.iter().map(|c| c.blades as usize).sum();
    assert_eq!(mesh.count_vertices(), blades * 5);
    let indices = match mesh.indices() {
        Some(Indices::U16(i)) => i.len(),
        Some(Indices::U32(i)) => i.len(),
        None => 0,
    };
    assert_eq!(indices, blades * 9);
    // Every corner carries its clump's root (in Bevy's axes: east, up,
    // south) and rank, and its height up the blade, 0 at the root to 1 at
    // the tip.
    let roots = floats4(&mesh, ATTRIBUTE_ROOT);
    let first = clumps[0].at;
    assert_eq!(roots[0], [first[0], first[2], -first[1], clumps[0].rank]);
    let Some(VertexAttributeValues::Unorm8x4(looks)) = mesh.attribute(ATTRIBUTE_LOOK) else {
        panic!("a look for every corner");
    };
    assert_eq!(
        looks[..5].iter().map(|l| l[3]).collect::<Vec<_>>(),
        [0, 0, 128, 128, 255]
    );
    // No light atlas on this ground, so no place in one.
    assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_none());
}

#[test]
fn a_clump_on_a_baked_ground_reads_the_atlas_where_its_root_is_on_every_corner() {
    let mut part = yard("yard", 4.0);
    part.light_uvs = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    let clumps = grow(&part, &grass(6.0));
    let clump = &clumps[0];
    let [u, v] = clump.light_uv.expect("the ground has an atlas place");
    // On this square the atlas runs with east and north.
    assert!((u - clump.at[0] / 4.0).abs() < 1e-5 && (v - clump.at[1] / 4.0).abs() < 1e-5);
    let mesh = clumps_mesh(&[clump]);
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1) else {
        panic!("an atlas place for every corner");
    };
    assert_eq!(uvs.len(), clump.blades as usize * 5);
    assert!(uvs.iter().all(|uv| *uv == [u, v]));
}

#[test]
fn the_grass_field_grows_its_clumps_in_cells_of_8_m_and_a_camera_on_it_draws_about_two_fifths() {
    let map = test_map("grass-field").unwrap().unwrap();
    let grown = grow_the_map(&map.drawing);
    let area = GRASS_FIELD * GRASS_FIELD;
    assert_eq!(grown.clumps, (area * GRASS_FIELD_DENSITY) as usize);
    let side = (GRASS_FIELD / CELL).ceil() as usize + 1;
    assert!(grown.cells.len() <= side * side);
    // From a Quad landed in the middle, all round, before the view's own
    // edges cut more away: thinning with distance leaves about two fifths
    // (Observed: 24,263 of 60,000).
    let field = &map.drawing.iter().find(|p| p.scatter.is_some()).unwrap();
    let clumps = grow(field, field.scatter.as_ref().unwrap());
    let drawn = drawn_from(&clumps, [0.0, 0.0, 0.05]);
    assert!((20_000..30_000).contains(&drawn), "{drawn}");
}

#[test]
fn the_skate_park_and_bando_grow_no_grass_so_they_draw_as_before() {
    // Basis: Rule (#338: existing Maps' render counts don't change).
    let packs = common::packs();
    for id in ["opendrone/skate-park", "opendrone/bando"] {
        let map = packs.map(id).unwrap();
        assert!(map.drawing.iter().all(|p| p.scatter.is_none()), "{id}");
        assert_eq!(grow_the_map(&map.drawing).clumps, 0, "{id}");
    }
}
