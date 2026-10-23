//! Readable checks for the Factory's first slice, `opendrone/factory` (#295):
//! the Map loads through the Pack checker with its baked light, its Launch
//! Spot keeps the rules every Map keeps, and it has a Gap of each size, as
//! the Simulation sees it with the physics' line question.
//!
//! The Basis of every check is Rule: the World and content deep dive's rules
//! (a Launch Spot on open flat ground with at least 3 m clear and open sky;
//! Gaps of 0.5 to 1 m for the whoop, 2 to 5 m for both Quads, and a dive of
//! 15 m or more) and #295's scene. Positions are east, north and up from the
//! hall's south-west corner on the floor, in metres, as in
//! `assets-src/maps/factory/build.py`.

mod common;

use common::repo;
use opendrone_maths::Vec3;
use opendrone_pack::{MapDefinition, Packs};
use opendrone_physics::MapCollision;

fn factory() -> (MapDefinition, MapCollision) {
    let packs = Packs::open(&repo().join("packs"), "packs").unwrap();
    let map = packs
        .map("opendrone/factory")
        .unwrap_or_else(|p| panic!("the Factory should pass the Pack checker:\n{p}"));
    let collision = MapCollision::new(&map.shapes).unwrap();
    (map, collision)
}

fn at(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Whether anything solid is on the straight line between two points.
fn clear(map: &MapCollision, from: Vec3, to: Vec3) -> bool {
    map.line_question(from, to).is_empty()
}

/// The height of the first solid a line coming straight down at (`x`, `y`)
/// from `from` metres meets.
fn first_below(map: &MapCollision, x: f64, y: f64, from: f64) -> f64 {
    map.line_question(at(x, y, from), at(x, y, -30.0))
        .first()
        .unwrap_or_else(|| panic!("nothing under ({x}, {y})"))
        .entry
        .z
}

#[test]
fn the_factory_is_a_checked_map_with_baked_light_and_a_sun_37_degrees_up() {
    let (map, _) = factory();
    assert_eq!(map.name, "Factory");
    assert_eq!(map.world.gravity, 9.81);
    let light = map.light.expect("the Factory is lit by baked light");
    assert!((light.sun_elevation.to_degrees() - 37.0).abs() < 1e-9);
    assert!(
        map.drawing.iter().all(|part| !part.light_uvs.is_empty()),
        "every drawn part is in the light atlas"
    );
}

#[test]
fn the_launch_spot_is_on_open_flat_ground_with_3_metres_clear_and_open_sky_facing_the_hall() {
    let (map_definition, map) = factory();
    let spot = map_definition.launch_spot.expect("a Launch Spot");
    let (x, y) = (spot.position.x, spot.position.y);
    assert_eq!(spot.position.z, 0.0);
    for k in 0..16 {
        let angle = f64::from(k) * core::f64::consts::TAU / 16.0;
        for radius in [0.5, 1.5, 3.0] {
            let height = first_below(
                &map,
                x + radius * angle.cos(),
                y + radius * angle.sin(),
                20.0,
            );
            assert!(height.abs() < 0.02, "{radius} m out at {k}: {height}");
        }
        for height in [0.05, 1.0, 3.0] {
            let to = at(x + 3.0 * angle.cos(), y + 3.0 * angle.sin(), height);
            assert!(
                clear(&map, at(x, y, height), to),
                "direction {k} at {height} m up"
            );
        }
    }
    assert!(
        clear(&map, at(x, y, 0.05), at(x, y, 200.0)),
        "open sky above"
    );
    // Facing north, at the loading door in the south wall.
    assert_eq!(spot.heading, 0.0);
    assert!((x - 12.5).abs() < 1e-9);
}

#[test]
fn the_breach_and_the_loading_door_are_gaps_both_quads_fly_through() {
    let (_, map) = factory();
    // The breach in the west wall: 2.8 m wide and 2.6 m tall, from 0.45 m up.
    for (y, z) in [
        (14.5, 1.75),
        (13.6, 1.75),
        (15.4, 1.75),
        (14.5, 0.8),
        (14.5, 2.7),
    ] {
        assert!(
            clear(&map, at(-0.6, y, z), at(0.6, y, z)),
            "through the breach at y {y}, {z} m up"
        );
    }
    assert!(
        !clear(&map, at(-0.6, 11.5, 1.75), at(0.6, 11.5, 1.75)),
        "the wall beside it"
    );
    // The loading door in the south wall: 3.6 m wide and 4.4 m tall.
    for (x, z) in [(11.0, 1.0), (12.5, 3.5), (14.0, 4.2)] {
        assert!(
            clear(&map, at(x, -0.6, z), at(x, 0.6, z)),
            "through the door at x {x}, {z} m up"
        );
    }
}

#[test]
fn the_whoop_has_a_vent_hole_and_a_man_door_under_a_metre_wide() {
    let (_, map) = factory();
    // The vent hole in the east wall: 0.8 m wide, from 0.9 to 1.5 m up.
    assert!(clear(&map, at(29.4, 12.5, 1.2), at(30.6, 12.5, 1.2)));
    assert!(!clear(&map, at(29.4, 13.2, 1.2), at(30.6, 13.2, 1.2)));
    // The man door beside it: 1.1 m wide and 2.2 m tall.
    assert!(clear(&map, at(29.4, 7.55, 1.5), at(30.6, 7.55, 1.5)));
}

#[test]
fn the_bay_where_the_roof_fell_in_is_a_dive_of_more_than_15_metres_from_above_the_roof() {
    let (_, map) = factory();
    // From 25 m, nothing stops a dive through the hole in the roof until the
    // rubble heap on the floor.
    for (x, y) in [(11.5, 5.5), (12.5, 6.5), (13.5, 5.0)] {
        let landing = first_below(&map, x, y, 25.0);
        assert!(
            25.0 - landing > 15.0 && landing < 1.5,
            "at ({x}, {y}) the dive ends at {landing} m"
        );
    }
    // Beside it the roof is whole.
    assert!(first_below(&map, 3.0, 12.0, 25.0) > 11.0);
}

#[test]
fn leaves_and_weeds_are_drawn_but_not_solid_and_the_trees_wood_is_solid() {
    let (map, _) = factory();
    let solid = |name: &str| {
        map.solid_names
            .iter()
            .any(|n| n == name || n.starts_with(&format!("{name}_0")))
    };
    let drawn = |name: &str| map.drawing.iter().any(|p| p.name == name);
    for name in ["tree_leaves", "weeds"] {
        assert!(drawn(name), "{name} is drawn");
        assert!(!solid(name), "{name} is not solid");
    }
    assert!(solid("tree"), "the trees' trunks and branches are solid");
}
