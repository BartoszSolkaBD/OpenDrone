//! Readable checks for Bando, `opendrone/bando` (#64): every must-have
//! feature of "Unfinished Tower, with rooms" at the size #17's table gives,
//! the thin parts at their real thickness, and the Launch Spot rules every
//! Map keeps (World and content deep dive). They measure the finished Map as
//! the Simulation sees it, with the physics' line question, so a Map script
//! that drifts from the table fails here.
//!
//! The Basis of every check is Rule: #17's resolution (its table of must-have
//! features and its "Rules for every Map") and #10's comments from #17 (the
//! ceilings and floors are colliders, and thin parts are solid at their real
//! thickness) state the size, and this file only measures it. Positions are
//! east, north and up from the Map's origin, the middle of the site at ground
//! level, in metres. A floor's top is `0.15 + 3.35 * k` m up, the ground slab
//! being a 0.15 m plinth.

mod common;

use std::collections::BTreeSet;

use common::repo;
use opendrone_maths::Vec3;
use opendrone_pack::{MapDefinition, Packs};
use opendrone_physics::{MapCollision, MapShape};

fn bando() -> (MapDefinition, MapCollision) {
    let packs = Packs::open(&repo().join("packs"), "packs").unwrap();
    let map = packs
        .map("opendrone/bando")
        .unwrap_or_else(|p| panic!("Bando should pass the Pack checker:\n{p}"));
    let collision = MapCollision::new(&map.shapes).unwrap();
    (map, collision)
}

fn at(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The top of floor `k` (0 is the ground slab, 5 the roof).
fn floor_top(k: u32) -> f64 {
    0.15 + 3.35 * f64::from(k)
}

/// The stretches of a line the Map is in the way, as (from, to) distances.
fn blocked(map: &MapCollision, from: Vec3, to: Vec3) -> Vec<(f64, f64)> {
    map.line_question(from, to)
        .iter()
        .map(|c| (c.entry_distance, c.exit_distance))
        .collect()
}

/// The same, as positions along the line's own axis, lowest first, with
/// touching parts joined: a line along x gives x metres, along y gives y
/// metres, along z gives z metres.
fn spans(map: &MapCollision, from: Vec3, to: Vec3) -> Vec<(f64, f64)> {
    let axis = |p: Vec3| {
        if from.x != to.x {
            p.x
        } else if from.y != to.y {
            p.y
        } else {
            p.z
        }
    };
    let mut parts: Vec<(f64, f64)> = map
        .line_question(from, to)
        .iter()
        .map(|c| {
            let (a, b) = (axis(c.entry), axis(c.exit));
            (a.min(b), a.max(b))
        })
        .collect();
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Parts that touch or overlap are one stretch of solid, and a line that
    // only grazes a surface leaves nothing.
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in parts.into_iter().filter(|(a, b)| b - a > 1e-3) {
        match merged.last_mut() {
            Some(last) if start <= last.1 + 1e-6 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// The widths of the gaps between the Map's parts along a line, in order.
fn gaps(map: &MapCollision, from: Vec3, to: Vec3) -> Vec<f64> {
    let parts = spans(map, from, to);
    parts.windows(2).map(|w| w[1].0 - w[0].1).collect()
}

fn close(a: f64, b: f64, within: f64) -> bool {
    (a - b).abs() <= within
}

/// The height of the highest surface at (`x`, `y`) below 40 m.
fn surface(map: &MapCollision, x: f64, y: f64) -> f64 {
    map.line_question(at(x, y, 40.0), at(x, y, -30.0))
        .first()
        .unwrap_or_else(|| panic!("nothing under ({x}, {y})"))
        .entry
        .z
}

/// The heights of every top surface a line coming down at (`x`, `y`) enters,
/// from `from` down to `to`.
fn entries(map: &MapCollision, x: f64, y: f64, from: f64, to: f64) -> Vec<f64> {
    map.line_question(at(x, y, from), at(x, y, to))
        .iter()
        .map(|c| c.entry.z)
        .collect()
}

fn names(map: &MapDefinition) -> Vec<&str> {
    map.solid_names.iter().map(String::as_str).collect()
}

#[test]
fn bando_is_a_checked_map_with_gravity_air_and_a_launch_spot_in_the_yard() {
    let (map, _) = bando();
    assert_eq!(map.name, "Bando");
    assert_eq!(map.world.gravity, 9.81);
    assert_eq!(map.world.air_density, 1.225);
    let spot = map.launch_spot.expect("Bando has a Launch Spot");
    assert_eq!((spot.position.x, spot.position.y), (0.0, -17.0));
    assert_eq!(spot.heading, 0.0);
}

#[test]
fn the_concrete_frame_is_24_by_15_metres_with_floors_every_3_35_metres_and_the_roof_at_16_9() {
    let (_, map) = bando();
    // Along the roof slab's own thickness, west to east and south to north,
    // through spots with no hole: the frame's outline.
    let roof = floor_top(5) - 0.1;
    let along_x = spans(&map, at(-20.0, -6.0, roof), at(20.0, -6.0, roof));
    assert!(
        close(along_x.first().unwrap().0, -12.0, 0.01),
        "{along_x:?}"
    );
    assert!(close(along_x.last().unwrap().1, 12.0, 0.01), "{along_x:?}");
    let along_y = spans(&map, at(-3.0, -20.0, roof), at(-3.0, 20.0, roof));
    assert!(close(along_y.first().unwrap().0, -7.5, 0.01), "{along_y:?}");
    assert!(close(along_y.last().unwrap().1, 7.5, 0.01), "{along_y:?}");
    // Down through a spot with no hole in any floor: the roof and each floor
    // are 3.35 m above the last, and the ground slab is under them.
    let tops = entries(&map, -3.0, -6.0, 30.0, 0.1);
    assert_eq!(tops.len(), 6, "{tops:?}");
    for (k, top) in tops.iter().rev().enumerate() {
        assert!(
            close(*top, floor_top(k as u32), 0.001),
            "floor {k} is {top} m up"
        );
    }
    assert!(close(tops[0], 16.9, 0.001), "the roof is {} m up", tops[0]);
}

#[test]
fn every_floor_and_ceiling_is_a_collider_with_3_1_metres_clear_under_each_ceiling() {
    let (_, map) = bando();
    let crossings = map.line_question(at(-3.0, -6.0, 30.0), at(-3.0, -6.0, 0.1));
    assert_eq!(crossings.len(), 6);
    // Each slab is 0.25 m thick, so it is a ceiling to the floor below it:
    // its underside is 3.1 m above that floor.
    for k in 1..=5u32 {
        let slab = &crossings[(5 - k) as usize];
        let underside = slab.exit.z;
        assert!(
            close(slab.entry.z - underside, 0.25, 0.001),
            "floor {k} is {} m thick",
            slab.entry.z - underside
        );
        assert!(
            close(underside - floor_top(k - 1), 3.1, 0.001),
            "{} m clear under the ceiling of floor {k}",
            underside - floor_top(k - 1)
        );
    }
    // A whoop can be pulled onto any of them: a line up from each floor's
    // top meets the ceiling.
    for k in 0..5u32 {
        let up = map.line_question(
            at(-3.0, -6.0, floor_top(k) + 0.01),
            at(-3.0, -6.0, floor_top(k) + 4.0),
        );
        assert!(close(up[0].entry.z, floor_top(k) + 3.1, 0.001), "{up:?}");
    }
}

#[test]
fn columns_are_0_4_metres_square_on_a_6_by_5_metre_grid() {
    let (_, map) = bando();
    // Along x on floor 2, through the column row at y = 2.5.
    let row = spans(&map, at(-1.0, 2.5, 8.0), at(13.0, 2.5, 8.0));
    let centres: Vec<f64> = row.iter().map(|(a, b)| (a + b) / 2.0).collect();
    assert_eq!(row.len(), 3, "{row:?}");
    for (a, b) in &row {
        assert!(close(b - a, 0.4, 0.001), "a column {} m wide", b - a);
    }
    assert!(close(centres[1] - centres[0], 6.0, 0.001), "{centres:?}");
    // Along y at x = 6: the columns at y = -2.5 and 2.5 are 5 m apart.
    let column = spans(&map, at(6.0, -6.0, 8.0), at(6.0, 6.0, 8.0));
    assert_eq!(column.len(), 2, "{column:?}");
    let spacing = (column[1].0 + column[1].1 - column[0].0 - column[0].1) / 2.0;
    assert!(close(spacing, 5.0, 0.001), "{column:?}");
    // And every floor has them: 20 a floor, five floors.
    let columns = names(&bando().0)
        .iter()
        .filter(|n| n.starts_with("column"))
        .count();
    assert_eq!(columns, 100);
}

#[test]
fn the_lift_shaft_is_open_and_2_2_metres_square_from_the_roof_to_the_ground() {
    let (_, map) = bando();
    // Straight down from above the roof to the ground slab, nothing between:
    // the first thing the line meets is the slab at the bottom, 0.15 m up.
    let dive = map.line_question(at(1.5, 0.0, 25.0), at(1.5, 0.0, -1.0));
    assert!(close(dive[0].entry.z, floor_top(0), 0.001), "{dive:?}");
    let dive_height = floor_top(5) - dive[0].entry.z;
    assert!(
        close(dive_height, 16.75, 0.01),
        "the dive is {dive_height} m from the roof"
    );
    // 2.2 m between its walls (north to south) and 2.2 m long (west to east,
    // where it is open at both ends).
    let across = gaps(&map, at(1.5, -3.0, 8.5), at(1.5, 3.0, 8.5));
    assert_eq!(across.len(), 1);
    assert!(close(across[0], 2.2, 0.001), "{across:?}");
    let wall = spans(&map, at(-3.0, 1.2, 8.5), at(6.0, 1.2, 8.5));
    assert_eq!(wall.len(), 1);
    assert!(close(wall[0].1 - wall[0].0, 2.2, 0.001), "{wall:?}");
    assert_eq!(blocked(&map, at(-3.0, 0.0, 8.5), at(6.0, 0.0, 8.5)), []);
    // The floors open onto it: every one has the shaft as a hole.
    for k in 1..=5 {
        for (x, y) in [(0.6, -0.9), (2.4, 0.9)] {
            let tops = entries(&map, x, y, 30.0, 0.1);
            assert!(
                tops.iter().all(|top| !close(*top, floor_top(k), 0.01)),
                "floor {k} blocks the shaft at ({x}, {y})"
            );
        }
    }
}

#[test]
fn one_flight_of_stairs_a_floor_climbs_through_matching_openings_with_6_cm_handrails() {
    let (map_definition, map) = bando();
    let rise = 3.35 / 17.0;
    // Flight k rises 17 steps from floor k to floor k + 1, in the lane at
    // x = 6.4–8.2 for even k and x = 9.4–11.2 for odd k.
    for k in 0..5u32 {
        let x = if k % 2 == 0 { 7.3 } else { 10.3 };
        for step in 0..17 {
            let y = -7.1 + 0.27 * (f64::from(step) + 0.5);
            let tread = entries(
                &map,
                x,
                y,
                floor_top(k) + rise * f64::from(step + 1) + 1.0,
                floor_top(k) - 0.1,
            );
            assert!(
                close(tread[0], floor_top(k) + rise * f64::from(step + 1), 0.002),
                "flight {k}, step {step} is {} m up",
                tread[0] - floor_top(k)
            );
        }
        // The floor above has its opening over that flight: nothing between
        // the middle of the flight and the floor above.
        let y = -5.0;
        let above = entries(&map, x, y, floor_top(k + 1) + 1.0, floor_top(k + 1) - 0.12);
        assert!(
            above.iter().all(|z| !close(*z, floor_top(k + 1), 0.001)),
            "floor {} has no opening over flight {k}",
            k + 1
        );
    }
    assert_eq!(
        names(&map_definition)
            .iter()
            .filter(|n| n.starts_with("stairs"))
            .count(),
        5
    );
    // A handrail down each side, 6 cm across: a line across one, square on,
    // is within a hair of 6 cm (its ring has ten corners).
    let (x, y) = (6.46, -7.1 + 2.4);
    let z = 0.15 + rise + 0.9 + 2.4 * rise / 0.27;
    let rail = spans(&map, at(x - 0.3, y, z), at(x + 0.3, y, z));
    assert_eq!(rail.len(), 1, "{rail:?}");
    let width = rail[0].1 - rail[0].0;
    assert!((0.057..=0.0601).contains(&width), "a rail {width} m across");
}

#[test]
fn every_floor_has_holes_2_to_5_metres_across_and_floor_2_has_a_collapsed_bay() {
    let (_, map) = bando();
    // (floor, hole: x from, y from, x to, y to), as the script cuts them.
    let holes: [(u32, [f64; 4]); 8] = [
        (1, [-10.5, -6.5, -7.5, -4.0]),
        (2, [-4.5, 3.0, -1.5, 6.5]),
        (2, [-11.4, 2.7, -6.4, 7.1]),
        (3, [-10.0, 0.5, -7.0, 4.0]),
        (3, [3.5, -6.8, 5.5, -4.5]),
        (4, [-5.5, -3.0, -1.0, -0.5]),
        (5, [-9.5, 3.0, -6.5, 6.0]),
        (5, [3.5, 3.5, 5.5, 6.5]),
    ];
    for (k, [x0, y0, x1, y1]) in holes {
        let z = floor_top(k) - 0.1;
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        // Open through the floor at its middle.
        let up = entries(&map, cx, cy, floor_top(k) + 0.5, floor_top(k) - 0.4);
        assert!(
            up.iter().all(|top| !close(*top, floor_top(k), 0.01)),
            "floor {k} has no hole at ({cx}, {cy})"
        );
        // And 2 to 5 m across, each way, between the slab's two sides.
        for (from, to, middle, name) in [
            (at(x0 - 1.0, cy, z), at(x1 + 1.0, cy, z), cx, "east-west"),
            (at(cx, y0 - 1.0, z), at(cx, y1 + 1.0, z), cy, "north-south"),
        ] {
            let across = spans(&map, from, to);
            let before = across.iter().rfind(|(_, end)| *end <= middle);
            let after = across.iter().find(|(start, _)| *start >= middle);
            let (before, after) = (before.expect("slab before"), after.expect("slab after"));
            let width = after.0 - before.1;
            assert!(
                (2.0..=5.0).contains(&width),
                "floor {k}'s hole is {width} m {name} at ({cx}, {cy})"
            );
        }
    }
    // The collapsed bay: floor 2's slab lies on floor 1, rising 3.1 m from
    // floor 1 at y = 2.9 to floor 2's edge at y = 7.0, a ramp.
    let low = entries(&map, -8.9, 3.2, floor_top(2) - 0.5, floor_top(1) - 0.1);
    let high = entries(&map, -8.9, 6.8, floor_top(2) - 0.05, floor_top(1) - 0.1);
    assert!(low[0] < floor_top(1) + 0.9, "the slab starts low: {low:?}");
    assert!(
        high[0] > floor_top(2) - 0.4,
        "and rises to floor 2: {high:?}"
    );
}

#[test]
fn four_6_by_5_metre_brick_rooms_a_floor_open_off_a_5_metre_corridor_on_the_two_lowest_floors() {
    let (_, map) = bando();
    for k in [0u32, 1] {
        let z = floor_top(k) + 0.5;
        // Along y at x = -5, clear of every doorway: the south wall, the
        // corridor's two walls (5 m apart, middle to middle) and the north
        // wall.
        let walls = spans(&map, at(-5.0, -9.0, z), at(-5.0, 9.0, z));
        assert_eq!(walls.len(), 4, "floor {k}: {walls:?}");
        let mid = |i: usize| (walls[i].0 + walls[i].1) / 2.0;
        assert!(
            close(mid(2) - mid(1), 5.0, 0.01),
            "the corridor is {} m",
            mid(2) - mid(1)
        );
        // The rooms are 5 m deep, give or take the outside wall's half
        // thickness, and 6 m wide (middle to middle of the walls).
        assert!(close(mid(1) - mid(0), 5.0, 0.2), "{walls:?}");
        assert!(close(mid(3) - mid(2), 5.0, 0.2), "{walls:?}");
        // Across the south rooms at y = -6.5: west wall, the wall between
        // the two rooms, and nothing else (the east side stays open).
        let rooms = spans(&map, at(-14.0, -6.5, z), at(2.0, -6.5, z));
        assert_eq!(rooms.len(), 2, "floor {k}: {rooms:?}");
        let width = (rooms[1].0 + rooms[1].1 - rooms[0].0 - rooms[0].1) / 2.0;
        assert!(close(width, 6.0, 0.2), "a room {width} m wide");
    }
    // The upper three floors stay open: no wall of any kind in the west half
    // between the columns.
    for k in [2u32, 3, 4] {
        assert_eq!(
            blocked(
                &map,
                at(-11.0, -9.0, floor_top(k) + 1.0),
                at(-11.0, 9.0, floor_top(k) + 1.0)
            ),
            [],
            "floor {k} is open"
        );
    }
}

#[test]
fn doorways_are_0_9_by_2_1_metres_and_each_floor_has_one_smashed_gap_of_about_2_metres() {
    let (_, map) = bando();
    // The doorway in the ground floor's corridor wall at y = -2.5, from
    // x = -9.6 to -8.7: 0.9 m wide, with 2.1 m of height.
    let floor = floor_top(0);
    // Along the wall, between the corner column and the first smashed gap.
    let low = gaps(
        &map,
        at(-11.0, -2.5, floor + 1.0),
        at(-7.0, -2.5, floor + 1.0),
    );
    assert_eq!(low.len(), 1, "{low:?}");
    assert!(close(low[0], 0.9, 0.001), "{low:?}");
    let just_under = gaps(
        &map,
        at(-11.0, -2.5, floor + 2.05),
        at(-7.0, -2.5, floor + 2.05),
    );
    assert_eq!(just_under.len(), 1, "the doorway is open up to 2.1 m");
    let just_over = gaps(
        &map,
        at(-11.0, -2.5, floor + 2.15),
        at(-7.0, -2.5, floor + 2.15),
    );
    assert!(just_over.is_empty(), "and the wall is back above it");
    let head = spans(&map, at(-9.15, -2.5, floor), at(-9.15, -2.5, floor + 3.0));
    assert!(close(head[0].0, floor + 2.1, 0.001), "{head:?}");
    // The smashed gap on the ground floor: 2 m wide and 2.4 m high, in the
    // same wall, from x = -4.6 to -2.6.
    let smashed = gaps(
        &map,
        at(-11.6, -2.5, floor + 1.0),
        at(-0.2, -2.5, floor + 1.0),
    );
    assert!(smashed.iter().any(|g| close(*g, 2.0, 0.001)), "{smashed:?}");
    let top = spans(&map, at(-3.6, -2.5, floor), at(-3.6, -2.5, floor + 3.0));
    assert!(close(top[0].0, floor + 2.4, 0.001), "{top:?}");
    // On the first floor it is in the wall between the south rooms, 1.8 m
    // wide and 2.2 m high.
    let first = floor_top(1);
    let wall = gaps(
        &map,
        at(-6.0, -7.1, first + 1.0),
        at(-6.0, -2.7, first + 1.0),
    );
    assert!(wall.iter().any(|g| close(*g, 1.8, 0.001)), "{wall:?}");
}

#[test]
fn windows_are_1_5_by_1_4_metres_with_their_sills_0_9_metres_up() {
    let (_, map) = bando();
    // The north wall at y = 7.375, a window centred on x = -9 on the ground
    // floor: 1.5 m wide at mid height, 1.4 m high with its sill 0.9 m up.
    let floor = floor_top(0);
    let wide = gaps(
        &map,
        at(-11.0, 7.375, floor + 1.5),
        at(-7.0, 7.375, floor + 1.5),
    );
    assert_eq!(wide.len(), 1, "{wide:?}");
    assert!(close(wide[0], 1.5, 0.001), "{wide:?}");
    let tall = spans(&map, at(-9.0, 7.375, floor), at(-9.0, 7.375, floor + 3.2));
    assert_eq!(tall.len(), 2, "{tall:?}");
    assert!(close(tall[0].1 - floor, 0.9, 0.001), "{tall:?}");
    assert!(close(tall[1].0 - tall[0].1, 1.4, 0.001), "{tall:?}");
}

#[test]
fn the_roof_has_a_parapet_with_gaps_a_water_tank_on_legs_and_rebar_stubs() {
    let (map_definition, map) = bando();
    let roof = floor_top(5);
    // A parapet 1 m high all round, with three Gaps: 3.5 m south, 3 m north
    // and 0.8 m east, which only a whoop fits.
    let south = gaps(
        &map,
        at(-13.0, -7.6, roof + 0.5),
        at(13.0, -7.6, roof + 0.5),
    );
    assert_eq!(south.len(), 1, "{south:?}");
    assert!(close(south[0], 3.5, 0.001), "{south:?}");
    let north = gaps(&map, at(13.0, 7.6, roof + 0.5), at(-13.0, 7.6, roof + 0.5));
    assert!(close(north[0], 3.0, 0.001), "{north:?}");
    let east = gaps(&map, at(12.1, -8.5, roof + 0.5), at(12.1, 8.5, roof + 0.5));
    assert!(close(east[0], 0.8, 0.001), "{east:?}");
    let top = spans(&map, at(-12.1, 0.0, roof), at(-12.1, 0.0, roof + 3.0));
    assert!(close(top[0].1 - roof, 1.0, 0.001), "{top:?}");
    // The water tank on four 1.2 m legs, its top 3.2 m above the roof.
    let tank = map.line_question(at(-9.0, -5.0, 30.0), at(-9.0, -5.0, roof));
    assert!(close(tank[0].entry.z, roof + 3.2, 0.001), "{tank:?}");
    let legs = blocked(&map, at(-9.8, -6.5, roof + 0.6), at(-9.8, -3.5, roof + 0.6));
    assert_eq!(legs.len(), 2, "two legs on this side: {legs:?}");
    // Sixteen rebar stubs in four groups.
    let rebar = names(&map_definition)
        .iter()
        .filter(|n| n.starts_with("rebar"))
        .count();
    assert_eq!(rebar, 16);
}

#[test]
fn a_tower_crane_stands_with_a_34_metre_mast_a_35_metre_jib_and_a_hook_hanging_at_20_metres() {
    let (_, map) = bando();
    // The mast's chords are 1.3 m apart (inner face to inner face), at every
    // level between the braces.
    for level in [0u32, 5, 12] {
        let z = f64::from(level) * 2.4 + 0.5;
        let faces = spans(&map, at(18.0, 1.25, z), at(22.0, 1.25, z));
        let opening = faces.last().unwrap().0 - faces.first().unwrap().1;
        assert!(close(opening, 1.3, 0.001), "level {level}: {faces:?}");
    }
    // Up to 34 m, the cab on top; the jib from there.
    let chord = map.line_question(at(19.25, 1.25, 0.5), at(19.25, 1.25, 40.0));
    assert!(close(chord[0].entry.z, 0.0, 0.5), "{chord:?}");
    let cab = spans(&map, at(20.0, 2.0, 33.0), at(20.0, 2.0, 40.0));
    assert!(close(cab[0].0, 34.0, 0.001), "the cab starts {cab:?} up");
    // The jib's tip is 35 m from the mast's middle, at 20 m east, and the jib
    // reaches over the building's whole length.
    let jib = spans(&map, at(-20.0, 2.0, 37.4), at(24.0, 2.0, 37.4));
    assert!(close(jib[0].0, -15.0, 0.001), "{jib:?}");
    assert!(close(20.0 - jib[0].0, 35.0, 0.001));
    // The hook block's underside is 20 m up, under its cable.
    let hook = map.line_question(at(-2.0, 2.0, 19.0), at(-2.0, 2.0, 40.0));
    assert!(close(hook[0].entry.z, 20.0, 0.001), "{hook:?}");
}

#[test]
fn thin_parts_are_solid_at_their_real_thickness_a_5_cm_hook_cable_and_3_cm_rebar() {
    let (map_definition, map) = bando();
    // The hook cable: a line across it through its middle is 5 cm of cable,
    // from the jib down to the hook, in any direction.
    for z in [22.0, 28.0, 35.0] {
        let along_x = spans(&map, at(-4.0, 2.0, z), at(0.0, 2.0, z));
        assert_eq!(along_x.len(), 1, "{along_x:?}");
        assert!(
            close(along_x[0].1 - along_x[0].0, 0.05, 0.0005),
            "{along_x:?}"
        );
        let along_y = spans(&map, at(-2.0, 0.0, z), at(-2.0, 4.0, z));
        assert_eq!(along_y.len(), 1, "{along_y:?}");
        let width = along_y[0].1 - along_y[0].0;
        assert!((0.048..=0.0501).contains(&width), "{width}");
    }
    // Rebar: 3 cm across, a metre tall, on the roof.
    let roof = floor_top(5);
    let stub = spans(&map, at(-6.4, 2.62, roof + 0.5), at(-6.0, 2.62, roof + 0.5));
    assert_eq!(stub.len(), 1, "{stub:?}");
    assert!(close(stub[0].1 - stub[0].0, 0.03, 0.0005), "{stub:?}");
    let height = spans(&map, at(-6.12, 2.62, roof), at(-6.12, 2.62, roof + 2.0));
    assert!(close(height[0].1 - roof, 1.0, 0.001), "{height:?}");
    // The site fence's rails are 6 cm across, the top one at 2 m.
    let rail = spans(&map, at(-20.0, -24.3, 2.0), at(-20.0, -23.7, 2.0));
    assert_eq!(rail.len(), 1, "{rail:?}");
    let width = rail[0].1 - rail[0].0;
    assert!(
        (0.057..=0.0601).contains(&width),
        "a fence rail {width} m across"
    );
    // Every one of them is a solid part of the Map's collision, by name.
    let names = names(&map_definition);
    for part in ["hook_cable", "rebar", "fence_rail", "handrail"] {
        assert!(names.contains(&part), "Bando has a {part}");
    }
    // The cable is a convex solid, as a post standing straight up is.
    let cable = names.iter().position(|n| *n == "hook_cable").unwrap();
    assert!(matches!(
        map_definition.shapes[cable],
        MapShape::Convex { .. }
    ));
}

#[test]
fn the_yard_has_three_containers_one_stacked_rubble_a_site_office_a_dirt_mound_and_the_fence() {
    let (map_definition, map) = bando();
    // 2.44 m wide and 2.59 m high, two side by side and one across them.
    let row = spans(&map, at(20.0, -15.0, 1.0), at(32.0, -15.0, 1.0));
    assert_eq!(row.len(), 2, "{row:?}");
    assert!(close(row[0].1 - row[0].0, 2.44, 0.001), "{row:?}");
    assert!(close(surface(&map, 25.0, -15.0), 5.18, 0.001));
    assert!(close(surface(&map, 26.6, -15.5), 5.18, 0.001));
    let kinds: BTreeSet<&str> = names(&map_definition)
        .iter()
        .map(|n| n.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_'))
        .collect();
    for kind in [
        "container",
        "rubble_a",
        "rubble_b",
        "rubble_c",
        "site_office",
        "dirt_mound",
        "fence_post",
        "fence_rail",
    ] {
        assert!(kinds.contains(kind), "the yard has {kind}");
    }
    // The dirt mound is 2 m high.
    assert!(close(surface(&map, -25.0, 15.0), 2.0, 0.001));
}

#[test]
fn the_ground_runs_to_the_horizon_and_everything_solid_beyond_the_site_is_backdrop_within_110_metres()
 {
    let (map_definition, map) = bando();
    // Ground a kilometre out in every direction, so no edge to fly off, level
    // with the site's own dirt.
    for (x, y) in [
        (900.0, 900.0),
        (-900.0, 900.0),
        (900.0, -900.0),
        (-900.0, -900.0),
        (0.0, 990.0),
        (-990.0, 0.0),
        (30.0, 20.0),
    ] {
        assert!(
            close(surface(&map, x, y), 0.0, 1e-9),
            "ground at ({x}, {y})"
        );
    }
    // Every other solid part is within 110 m of the site (nothing worth
    // flying to beyond about 100 m).
    for (name, shape) in map_definition
        .solid_names
        .iter()
        .zip(&map_definition.shapes)
    {
        if name.starts_with("ground") {
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
        assert!(far < 110.0, "{name} reaches {far} m from the site");
    }
    // A solid Backdrop of city blocks and trees.
    let kinds: BTreeSet<&str> = names(&map_definition)
        .iter()
        .map(|n| n.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_'))
        .collect();
    for kind in ["city_block", "tree_trunk", "tree_crown"] {
        assert!(kinds.contains(kind), "the Backdrop has {kind}s");
    }
}

#[test]
fn the_launch_spot_is_on_open_flat_ground_with_6_7_metres_clear_all_round_and_open_sky() {
    let (map_definition, map) = bando();
    let spot = map_definition.launch_spot.unwrap();
    let (x, y) = (spot.position.x, spot.position.y);
    // On the ground, 0 m.
    assert_eq!(spot.position.z, 0.0);
    assert!(close(surface(&map, x, y), 0.0, 1e-9));
    // Flat round it: the same height 6 m out in every direction.
    for k in 0..16 {
        let angle = f64::from(k) * core::f64::consts::TAU / 16.0;
        for radius in [0.5, 1.0, 2.0, 4.0, 6.0] {
            let height = surface(&map, x + radius * angle.cos(), y + radius * angle.sin());
            assert!(close(height, 0.0, 0.01), "{radius} m out at {k}: {height}");
        }
    }
    // 6.7 m clear all round: nothing to hit from just above the ground up to
    // 5 m, in every direction.
    for k in 0..32 {
        let angle = f64::from(k) * core::f64::consts::TAU / 32.0;
        for height in [0.05, 0.3, 1.0, 2.0, 3.0, 5.0] {
            let to = at(x + 6.7 * angle.cos(), y + 6.7 * angle.sin(), height);
            assert_eq!(
                blocked(&map, at(x, y, height), to),
                [],
                "direction {k} at {height} m up"
            );
        }
    }
    // Open sky above.
    assert_eq!(blocked(&map, at(x, y, 0.05), at(x, y, 200.0)), []);
    // 17 m south of the building and facing it.
    assert_eq!((x, y), (0.0, -17.0));
    let to_building = (0.0 - x).atan2(0.0 - y).to_degrees().rem_euclid(360.0);
    assert!(
        close(to_building, spot.heading, 15.0),
        "the building is at {to_building}°"
    );
}

#[test]
fn every_object_is_solid_or_says_it_is_not_and_the_checked_parts_keep_their_order() {
    let (map_definition, _) = bando();
    assert_eq!(
        map_definition.solid_names.len(),
        map_definition.shapes.len()
    );
    let again = bando().0;
    assert_eq!(again.solid_names, map_definition.solid_names);
    assert_eq!(again.fingerprint(), map_definition.fingerprint());
    assert!(map_definition.drawing.len() >= map_definition.shapes.len());
}
