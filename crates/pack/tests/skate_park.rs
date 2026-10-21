//! Readable checks for the Skate Park, `opendrone/skate-park` (#63): every
//! must-have feature of "Bowl Park + Street Corner" at the size #17's table
//! gives, and the Launch Spot rules every Map keeps (World and content deep
//! dive). They measure the finished Map as the Simulation sees it, with the
//! physics' line question, so a Map script that drifts from the table fails
//! here.
//!
//! The Basis of every check is Rule: #17's resolution (its table of must-have
//! features and its "Rules for every Map") states the size, and this file only
//! measures it. Positions are east, north and up from the Map's origin, the
//! park's deck level, in metres.

mod common;

use std::collections::BTreeSet;

use common::repo;
use opendrone_maths::Vec3;
use opendrone_pack::{MapDefinition, Packs};
use opendrone_physics::{MapCollision, MapShape};

fn skate_park() -> (MapDefinition, MapCollision) {
    let packs = Packs::open(&repo().join("packs"), "packs").unwrap();
    let map = packs
        .map("opendrone/skate-park")
        .unwrap_or_else(|p| panic!("the Skate Park should pass the Pack checker:\n{p}"));
    let collision = MapCollision::new(&map.shapes).unwrap();
    (map, collision)
}

fn at(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The height of the highest surface at (`x`, `y`): where a line coming down
/// from 30 m first meets the Map.
fn surface(map: &MapCollision, x: f64, y: f64) -> f64 {
    let crossings = map.line_question(at(x, y, 30.0), at(x, y, -30.0));
    crossings
        .first()
        .unwrap_or_else(|| panic!("nothing under ({x}, {y})"))
        .entry
        .z
}

/// How deep the ground is below the deck at (`x`, `y`), in metres.
fn depth(map: &MapCollision, x: f64, y: f64) -> f64 {
    -surface(map, x, y)
}

/// The stretches of a line the Map is in the way, as (from, to) distances.
fn blocked(map: &MapCollision, from: Vec3, to: Vec3) -> Vec<(f64, f64)> {
    map.line_question(from, to)
        .iter()
        .map(|c| (c.entry_distance, c.exit_distance))
        .collect()
}

/// The first and last metre along `from` to `to` (a line of `steps` samples)
/// where the ground is deeper than `threshold`: how wide a sunken part is.
fn sunken_width(map: &MapCollision, from: (f64, f64), to: (f64, f64), threshold: f64) -> f64 {
    let steps = 1000;
    let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
    let sunk: Vec<f64> = (0..=steps)
        .filter(|i| {
            let t = f64::from(*i) / f64::from(steps);
            depth(
                map,
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
            ) > threshold
        })
        .map(|i| f64::from(i) / f64::from(steps) * length)
        .collect();
    sunk.last().unwrap() - sunk.first().unwrap()
}

#[test]
fn the_skate_park_is_a_checked_map_with_gravity_air_and_a_launch_spot() {
    let (map, _) = skate_park();
    assert_eq!(map.name, "Skate Park");
    assert_eq!(map.world.gravity, 9.81);
    assert_eq!(map.world.air_density, 1.225);
    let spot = map.launch_spot.expect("the Skate Park has a Launch Spot");
    // The file keeps 32-bit numbers: -1.8 m is stored to within 0.0000001 m.
    assert!((spot.position.x - -1.8).abs() < 1e-6 && spot.position.y == 6.0);
    assert_eq!(spot.heading, 270.0);
}

#[test]
fn the_pool_bowl_is_16_by_10_metres_and_1_8_deep_at_the_shallow_end_and_3_2_at_the_deep_end() {
    let (_, map) = skate_park();
    // Along its length and across it, through its middle.
    let length = sunken_width(&map, (-26.0, 6.0), (0.0, 6.0), 0.3);
    let width = sunken_width(&map, (-12.0, -2.0), (-12.0, 14.0), 0.3);
    assert!((length - 16.0).abs() < 0.6, "the pool is {length} m long");
    assert!((width - 10.0).abs() < 0.6, "the pool is {width} m wide");
    // The floor at each end, in the flat of it.
    let shallow = depth(&map, -17.5, 6.0);
    let deep = depth(&map, -6.8, 6.0);
    assert!(
        (shallow - 1.8).abs() < 0.05,
        "the shallow end is {shallow} m deep"
    );
    assert!((deep - 3.2).abs() < 0.05, "the deep end is {deep} m deep");
    // 0.6 m of vertical wall: right at the lip, 0.2 m in, the wall has
    // already dropped past 0.6 m at both ends.
    for x in [-19.8, -4.2] {
        assert!(
            depth(&map, x, 6.0) > 0.6,
            "the wall at x = {x} is too gentle"
        );
    }
}

#[test]
fn the_cradle_is_round_10_metres_across_and_1_9_deep() {
    let (_, map) = skate_park();
    let east_west = sunken_width(&map, (-1.0, 8.0), (15.0, 8.0), 0.3);
    let north_south = sunken_width(&map, (7.0, 0.0), (7.0, 16.0), 0.3);
    assert!((east_west - 10.0).abs() < 0.6, "{east_west}");
    assert!((north_south - 10.0).abs() < 0.6, "{north_south}");
    let middle = depth(&map, 7.0, 8.0);
    assert!((middle - 1.9).abs() < 0.05, "the cradle is {middle} m deep");
}

#[test]
fn the_snake_run_is_about_55_metres_long_4_4_wide_and_up_to_1_5_deep() {
    let (_, map) = skate_park();
    // It winds along these points (the Map script's own path).
    let path: [(f64, f64); 6] = [
        (-25.0, -16.0),
        (-15.0, -9.5),
        (-5.0, -15.5),
        (5.0, -9.5),
        (15.0, -15.5),
        (25.0, -10.0),
    ];
    let length: f64 = path
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    assert!((50.0..=60.0).contains(&length), "{length}");
    // Its deepest point, 1.5 m, is on that path, and it is nowhere deeper.
    let mut deepest = 0.0f64;
    for pair in path.windows(2) {
        for i in 0..200 {
            let t = f64::from(i) / 200.0;
            deepest = deepest.max(depth(
                &map,
                pair[0].0 + (pair[1].0 - pair[0].0) * t,
                pair[0].1 + (pair[1].1 - pair[0].1) * t,
            ));
        }
    }
    assert!(
        (1.45..=1.55).contains(&deepest),
        "the snake run is {deepest} m deep at its deepest"
    );
    assert!((depth(&map, -5.0, -15.5) - 1.5).abs() < 0.05);
    // Across a straight stretch, halfway along its first arm, at right angles
    // to it: 4.4 m wide at the top (a little narrower where it is 0.3 m deep).
    let middle = (-10.0, -12.5);
    let across = (0.5144, 0.8575);
    let width = sunken_width(
        &map,
        (middle.0 - 3.0 * across.0, middle.1 - 3.0 * across.1),
        (middle.0 + 3.0 * across.0, middle.1 + 3.0 * across.1),
        0.3,
    );
    assert!((4.0..=4.5).contains(&width), "{width}");
    // Ramps in at both ends: level where it starts.
    assert!(depth(&map, -25.0, -16.0) < 0.2);
    assert!(depth(&map, 25.0, -10.0) < 0.2);
}

#[test]
fn the_full_pipe_is_3_6_metres_across_inside_and_7_long() {
    let (_, map) = skate_park();
    // Along its axis, clear from end to end, and along its wall 7 m of it.
    assert_eq!(blocked(&map, at(10.0, 0.0, 1.8), at(30.0, 0.0, 1.8)), []);
    let wall = blocked(&map, at(10.0, 1.9, 1.8), at(30.0, 1.9, 1.8));
    assert_eq!(wall.len(), 1);
    assert!((wall[0].1 - wall[0].0 - 7.0).abs() < 0.05, "{wall:?}");
    // Across it, 3.6 m between the walls at the middle height.
    assert_eq!(blocked(&map, at(18.5, -1.78, 1.8), at(18.5, 1.78, 1.8)), []);
    assert!(!blocked(&map, at(18.5, -1.9, 1.8), at(18.5, 1.9, 1.8)).is_empty());
    // And 3.6 m from the floor to the roof inside.
    let roof = map.line_question(at(18.5, 0.0, 0.1), at(18.5, 0.0, 6.0));
    assert!((roof[0].entry.z - 3.6).abs() < 0.03, "{}", roof[0].entry.z);
}

#[test]
fn six_steps_climb_to_a_terrace_1_2_metres_up_with_two_handrails_a_hubba_and_a_bank() {
    let (map_definition, map) = skate_park();
    // Each step is 0.2 m higher than the one before, from the first to the
    // sixth, 1.2 m up, and the terrace goes on at the same height.
    for step in 0..6 {
        let x = 26.9 + 0.35 * (f64::from(step) + 0.5);
        let height = surface(&map, x, 11.0);
        assert!(
            (height - 0.2 * f64::from(step + 1)).abs() < 0.01,
            "step {step} is {height} m up"
        );
    }
    assert!((surface(&map, 31.5, 11.0) - 1.2).abs() < 0.01);
    let names: Vec<&str> = map_definition
        .solid_names
        .iter()
        .map(String::as_str)
        .collect();
    for name in [
        "handrail",
        "handrail_001",
        "hubba",
        "bank_to_terrace",
        "stair_set",
    ] {
        assert!(names.contains(&name), "the Skate Park has a {name}");
    }
    // The handrails run beside the stairs at 0.9 m above the steps' noses.
    for y in [9.4, 12.6] {
        let rail = map.line_question(at(27.0, y, 3.0), at(27.0, y, 0.0));
        assert!(!rail.is_empty(), "a handrail at y = {y}");
    }
}

#[test]
fn the_pergola_has_slats_at_3_6_metres_with_0_6_metre_gaps_and_3_3_metres_clear_underneath() {
    let (_, map) = skate_park();
    // Along the slats at their height: each slat then 0.6 m of air.
    let slats = blocked(&map, at(13.0, 15.5, 3.7), at(25.0, 15.5, 3.7));
    assert!(slats.len() >= 10, "{} slats", slats.len());
    for pair in slats.windows(2) {
        let gap = pair[1].0 - pair[0].1;
        assert!((gap - 0.6).abs() < 0.01, "a gap of {gap} m");
    }
    // 3.3 m from the deck to the beams and slats, between the columns.
    let roof = map.line_question(at(18.65, 15.5, 0.1), at(18.65, 15.5, 6.0));
    assert!((roof[0].entry.z - 3.6).abs() < 0.02, "{}", roof[0].entry.z);
    let beam = map.line_question(at(21.0, 12.8, 0.1), at(21.0, 12.8, 6.0));
    assert!((beam[0].entry.z - 3.3).abs() < 0.02, "{}", beam[0].entry.z);
}

#[test]
fn the_ring_has_a_3_1_metre_opening_with_its_centre_2_2_metres_up() {
    let (_, map) = skate_park();
    // Through the opening, east to west, and across it either way.
    assert_eq!(blocked(&map, at(15.0, 7.0, 2.2), at(22.0, 7.0, 2.2)), []);
    assert_eq!(
        blocked(&map, at(18.5, 7.0 - 1.5, 2.2), at(18.5, 7.0 + 1.5, 2.2)),
        []
    );
    assert!(!blocked(&map, at(18.5, 7.0 - 1.7, 2.2), at(18.5, 7.0 + 1.7, 2.2)).is_empty());
    assert_eq!(
        blocked(&map, at(18.5, 7.0, 2.2 - 1.5), at(18.5, 7.0, 2.2 + 1.5)),
        []
    );
    assert!(!blocked(&map, at(18.5, 7.0, 2.2 - 1.7), at(18.5, 7.0, 2.2 + 1.7)).is_empty());
}

#[test]
fn nine_metre_light_poles_stand_on_the_corners_and_the_long_sides() {
    let (map_definition, map) = skate_park();
    let poles = map_definition
        .solid_names
        .iter()
        .filter(|name| {
            name.starts_with("light_pole") && !name.ends_with("_lamp") && !name.contains("_lamp_")
        })
        .count();
    assert_eq!(poles, 6);
    for (x, y, base) in [
        (-27.5, -19.5, 0.0),
        (33.5, -19.5, 0.0),
        (-27.5, 19.5, 0.0),
        (0.0, 19.5, 0.0),
        (0.0, -19.5, 0.0),
        (33.5, 19.5, 1.2),
    ] {
        let top = map.line_question(at(x, y, 15.0), at(x, y, -1.0));
        let height = top[0].entry.z - base;
        assert!(
            (9.0..=9.15).contains(&height),
            "the pole at ({x}, {y}) is {height} m tall"
        );
    }
}

#[test]
fn the_ground_runs_to_the_horizon_and_everything_solid_beyond_it_is_backdrop_within_110_metres() {
    let (map_definition, map) = skate_park();
    // Ground a kilometre out in every direction, so no edge to fly off.
    for (x, y) in [
        (900.0, 900.0),
        (-900.0, 900.0),
        (900.0, -900.0),
        (-900.0, -900.0),
        (0.0, 990.0),
        (-990.0, 0.0),
    ] {
        assert!(surface(&map, x, y) < 0.0, "ground at ({x}, {y})");
    }
    // Nothing but flat ground and the street is out there: every other solid
    // part is within 110 m of the park (nothing worth flying to beyond about
    // 100 m).
    for (name, shape) in map_definition
        .solid_names
        .iter()
        .zip(&map_definition.shapes)
    {
        if name.starts_with("ground") || name.starts_with("street") {
            continue;
        }
        let far = match shape {
            MapShape::Box { centre, size, .. } => {
                let half = size.x.max(size.y) / 2.0;
                (centre.x.abs() + half).max(centre.y.abs() + half)
            }
            MapShape::Convex { corners, .. } | MapShape::TriangleMesh { corners, .. } => corners
                .iter()
                .map(|c| c.x.abs().max(c.y.abs()))
                .fold(0.0, f64::max),
        };
        assert!(far < 110.0, "{name} reaches {far} m from the park");
    }
    // A solid Backdrop of houses, blocks and trees.
    let kinds: BTreeSet<&str> = map_definition
        .solid_names
        .iter()
        .map(|n| n.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_'))
        .collect();
    for kind in ["house", "block", "tree_trunk", "tree_crown"] {
        assert!(kinds.contains(kind), "the Backdrop has {kind}s");
    }
}

#[test]
fn the_launch_spot_is_on_open_flat_ground_with_3_metres_clear_all_round_and_open_sky() {
    let (map_definition, map) = skate_park();
    let spot = map_definition.launch_spot.unwrap();
    let (x, y) = (spot.position.x, spot.position.y);
    // On the ground, the deck at 0 m.
    assert!((surface(&map, x, y) - spot.position.z).abs() < 1e-6);
    assert_eq!(spot.position.z, 0.0);
    // Flat under where the Quad sits: the deck at the same height 2 m out in
    // every direction (a lip of the pool bowl is 2.2 m to the west).
    for k in 0..16 {
        let angle = f64::from(k) * core::f64::consts::TAU / 16.0;
        for radius in [0.5, 1.0, 2.0] {
            let height = surface(&map, x + radius * angle.cos(), y + radius * angle.sin());
            assert!(
                (height - spot.position.z).abs() < 0.01,
                "{radius} m out at {k}: {height}"
            );
        }
    }
    // 3 m clear all round: nothing to hit from just above the ground (the
    // bowl's coping is 3 cm high) up to 3 m, in every direction.
    for k in 0..16 {
        let angle = f64::from(k) * core::f64::consts::TAU / 16.0;
        for height in [0.05, 0.3, 1.0, 2.0, 3.0] {
            let to = at(x + 3.0 * angle.cos(), y + 3.0 * angle.sin(), height);
            assert_eq!(
                blocked(&map, at(x, y, height), to),
                [],
                "direction {k} at {height} m up"
            );
        }
    }
    // Open sky above.
    assert_eq!(blocked(&map, at(x, y, 0.05), at(x, y, 200.0)), []);
    // Facing the Map's main feature: the pool bowl, to the west.
    let to_pool = (-12.0 - x).atan2(6.0 - y).to_degrees().rem_euclid(360.0);
    assert!(
        (to_pool - spot.heading).abs() < 15.0,
        "the pool is at {to_pool}°"
    );
}

#[test]
fn every_object_is_solid_or_says_it_is_not_and_the_checked_parts_keep_their_order() {
    let (map_definition, _) = skate_park();
    // One name for each solid shape, in the file's order, the same on every
    // read.
    assert_eq!(
        map_definition.solid_names.len(),
        map_definition.shapes.len()
    );
    let again = skate_park().0;
    assert_eq!(again.solid_names, map_definition.solid_names);
    assert_eq!(again.fingerprint(), map_definition.fingerprint());
    // What is drawn is every part of the file, and the Skate Park hides none.
    assert!(map_definition.drawing.len() >= map_definition.shapes.len());
}
