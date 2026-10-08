//! The Test Maps: simple Maps built into the code for Scenarios only, with
//! the `test/` prefix (#16 §2). Each is a `map.toml` in `crates/pack/test-maps/`
//! (so `cargo xtask migrate` keeps its format up), read like any Map's, plus
//! its solid parts written here as plain [`MapShape`]s, as a Map's `.glb`
//! colliders will be.
//!
//! Every Test Map but empty air stands on the same ground: a slab 200 m
//! square whose top is the Map's origin, 0 m up. Positions are east, north
//! and up from the origin.

use core::f64::consts::PI;

use opendrone_maths::functions::sin_cos;
use opendrone_maths::{Attitude, Vec3};
use opendrone_physics::MapShape;

/// One Test Map.
pub(crate) struct TestMap {
    /// Its id after `test/`.
    pub name: &'static str,
    /// Its `map.toml`, a file in `crates/pack/test-maps/`.
    pub text: &'static str,
    /// Its solid parts, in a fixed order.
    pub shapes: fn() -> Vec<MapShape>,
}

pub(crate) const TEST_MAPS: &[TestMap] = &[
    TestMap {
        name: "empty-air",
        text: include_str!("../test-maps/empty-air.toml"),
        shapes: Vec::new,
    },
    TestMap {
        name: "flat-floor",
        text: include_str!("../test-maps/flat-floor.toml"),
        shapes: || vec![ground()],
    },
    TestMap {
        name: "wall",
        text: include_str!("../test-maps/wall.toml"),
        shapes: || {
            vec![
                ground(),
                level_box(Vec3::new(5.1, 0.0, 2.5), Vec3::new(0.2, 20.0, 5.0)),
            ]
        },
    },
    TestMap {
        name: "thin-rail",
        text: include_str!("../test-maps/thin-rail.toml"),
        shapes: || {
            vec![
                ground(),
                rail(
                    Vec3::new(3.0, -5.0, 1.0),
                    Vec3::new(3.0, 5.0, 1.0),
                    RAIL_DIAMETER,
                ),
                rail(
                    Vec3::new(3.0, 10.0, 0.0),
                    Vec3::new(3.0, 10.0, 2.0),
                    REBAR_DIAMETER,
                ),
            ]
        },
    },
    TestMap {
        name: "floor-and-ceiling",
        text: include_str!("../test-maps/floor-and-ceiling.toml"),
        shapes: || {
            vec![
                MapShape::TriangleMesh {
                    corners: vec![
                        Vec3::new(-20.0, -20.0, 0.0),
                        Vec3::new(20.0, -20.0, 0.0),
                        Vec3::new(20.0, 20.0, 0.0),
                        Vec3::new(-20.0, 20.0, 0.0),
                    ],
                    // Anticlockwise seen from above: the sheet faces up.
                    triangles: vec![[0, 1, 2], [0, 2, 3]],
                },
                closed_slab(Vec3::new(-20.0, -20.0, 3.1), Vec3::new(20.0, 20.0, 3.3)),
            ]
        },
    },
    TestMap {
        name: "ledge",
        text: include_str!("../test-maps/ledge.toml"),
        shapes: || {
            let (west, east) = (-20.0, 0.0);
            let (south, north) = (-20.0, 20.0);
            let (bottom, top) = (0.0, 1.0);
            let mut corners = Vec::new();
            for x in [west, east] {
                for y in [south, north] {
                    for z in [bottom, top] {
                        corners.push(Vec3::new(x, y, z));
                    }
                }
            }
            vec![ground(), MapShape::Convex { corners }]
        },
    },
];

/// The rail on `test/thin-rail`, across: 6 cm, the alpha Maps' handrails
/// (#17).
const RAIL_DIAMETER: f64 = 0.06;
/// The rebar stub on `test/thin-rail`, across: 3 cm, the alpha Maps' rebar
/// (#17).
const REBAR_DIAMETER: f64 = 0.03;
/// How many flat sides a round rail has.
const RAIL_SIDES: u32 = 16;

/// The ground every Test Map but empty air stands on: a slab 200 m square and
/// 2 m thick, its top at 0 m.
fn ground() -> MapShape {
    level_box(Vec3::new(0.0, 0.0, -1.0), Vec3::new(200.0, 200.0, 2.0))
}

/// A box with its sides facing east, north and up.
fn level_box(centre: Vec3, size: Vec3) -> MapShape {
    MapShape::Box {
        centre,
        size,
        attitude: Attitude::BODY_IS_WORLD,
    }
}

/// A round rail from `start` to `end`, as a convex shape with [`RAIL_SIDES`]
/// flat sides, its corners on a circle `diameter` across. The rail must run
/// straight north–south or straight up.
fn rail(start: Vec3, end: Vec3, diameter: f64) -> MapShape {
    let runs_north = start.x == end.x && start.z == end.z;
    let radius = diameter / 2.0;
    let mut corners = Vec::new();
    for k in 0..RAIL_SIDES {
        let (sine, cosine) = sin_cos(2.0 * PI * f64::from(k) / f64::from(RAIL_SIDES));
        // Round the rail: east and up for a rail running north, east and
        // north for one standing up.
        let around = if runs_north {
            Vec3::new(radius * cosine, 0.0, radius * sine)
        } else {
            Vec3::new(radius * cosine, radius * sine, 0.0)
        };
        corners.push(start + around);
        corners.push(end + around);
    }
    MapShape::Convex { corners }
}

/// A closed slab between two opposite corners, as a triangle mesh, every
/// triangle anticlockwise seen from outside.
fn closed_slab(low: Vec3, high: Vec3) -> MapShape {
    let corners = vec![
        Vec3::new(low.x, low.y, low.z),
        Vec3::new(high.x, low.y, low.z),
        Vec3::new(high.x, high.y, low.z),
        Vec3::new(low.x, high.y, low.z),
        Vec3::new(low.x, low.y, high.z),
        Vec3::new(high.x, low.y, high.z),
        Vec3::new(high.x, high.y, high.z),
        Vec3::new(low.x, high.y, high.z),
    ];
    let triangles = vec![
        [0, 2, 1], // underneath, facing down
        [0, 3, 2],
        [4, 5, 6], // on top, facing up
        [4, 6, 7],
        [0, 1, 5], // the south side
        [0, 5, 4],
        [3, 6, 2], // the north side
        [3, 7, 6],
        [0, 4, 7], // the west side
        [0, 7, 3],
        [1, 2, 6], // the east side
        [1, 6, 5],
    ];
    MapShape::TriangleMesh { corners, triangles }
}
