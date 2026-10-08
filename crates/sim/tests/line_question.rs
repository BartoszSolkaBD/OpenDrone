//! Readable checks for the line question: "which Map surfaces does this line
//! pass through, and where?" The Video Signal (#67) and the Where-you-stand
//! sound (#75) ask it between ticks, to weigh how much wall stands between
//! the pilot and the Quad (#12, #14). Basis: Rule (the geometry of the shapes
//! given).

use opendrone_maths::{Attitude, Mat3, Vec3};
use opendrone_sim::{
    BatteryParameters, Drag, EscParameters, LineCrossing, MapShape, MapShapeProblem,
    MotorParameters, Mount, PacketRate, PhysicsRate, PropDirection, PropParameters, QuadParameters,
    QuadSetUp, QuadShape, QuadState, RotorLayout, ScriptedMotors, SetUp, SetUpError, Simulation,
    StartingMotors, World,
};

fn level_box(centre: Vec3, size: Vec3) -> MapShape {
    MapShape::Box {
        centre,
        size,
        attitude: Attitude::BODY_IS_WORLD,
    }
}

/// The ground, its top at 0 m.
fn ground() -> MapShape {
    level_box(Vec3::new(0.0, 0.0, -1.0), Vec3::new(200.0, 200.0, 2.0))
}

/// A wall 0.2 m thick whose west face is 5 m east.
fn wall() -> MapShape {
    level_box(Vec3::new(5.1, 0.0, 2.5), Vec3::new(0.2, 20.0, 5.0))
}

/// A concrete slab from 3.1 m to 3.3 m up, as a closed triangle mesh with
/// every triangle facing out, as a Map's `.glb` gives a ceiling.
fn ceiling() -> MapShape {
    let (low, high) = (Vec3::new(-20.0, -20.0, 3.1), Vec3::new(20.0, 20.0, 3.3));
    MapShape::TriangleMesh {
        corners: vec![
            Vec3::new(low.x, low.y, low.z),
            Vec3::new(high.x, low.y, low.z),
            Vec3::new(high.x, high.y, low.z),
            Vec3::new(low.x, high.y, low.z),
            Vec3::new(low.x, low.y, high.z),
            Vec3::new(high.x, low.y, high.z),
            Vec3::new(high.x, high.y, high.z),
            Vec3::new(low.x, high.y, high.z),
        ],
        triangles: vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 6, 2],
            [3, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ],
    }
}

/// A whoop-sized Quad, its motors stopped: the Whoop 65's numbers in SI
/// units.
fn whoop_at(position: Vec3, velocity: Vec3) -> QuadSetUp {
    let kv = 19500.0 * 2.0 * core::f64::consts::PI / 60.0;
    QuadSetUp {
        parameters: QuadParameters {
            mass: 0.0312,
            inertia: Mat3::diagonal(Vec3::new(7.0e-6, 9.0e-6, 14.0e-6)),
            drag: Drag {
                body_area: Vec3::ZERO,
                rotor: 0.0,
                duct_ram: 0.0,
                duct_offset: 0.0,
            },
            rotors: RotorLayout {
                diagonal: 0.066,
                rotor_height: 0.008,
                direction: PropDirection::PropsIn,
            },
            props: PropParameters {
                diameter: 0.035,
                thrust_coefficient: 0.29,
                power_coefficient: 0.26,
                rotor_inertia: 0.25e-7,
                reverse_thrust: 0.5,
                reverse_torque: 1.0,
            },
            motors: MotorParameters {
                kv,
                poles: 12,
                winding_resistance: 0.5,
                no_load_current: 0.3,
                no_load_voltage: 4.0,
                spin_up: 0.035,
                slow_down: 0.035,
            },
            esc: EscParameters {
                start_wait: 0.1,
                startup_power_limit: 0.0196,
                restart_tries: 3,
            },
            battery: BatteryParameters {
                cells: 1,
                capacity: 0.320 * 3600.0,
                voltage_curve: vec![(1.0, 4.35), (0.5, 3.92), (0.0, 3.30)],
                resistance: 0.029,
                connector: 0.010,
                recovery: 3.3,
                slow_sag: 0.0,
            },
            shape: QuadShape {
                body: Vec3::new(0.035, 0.030, 0.020),
                pack: Vec3::new(0.064, 0.010, 0.006),
                pack_height: -0.006,
                diagonal: 0.066,
                rotor_height: 0.008,
                prop_diameter: 0.035,
                duct_rings: None,
                bounce: 0.3,
                friction: 0.5,
            },
        },
        start: QuadState {
            position,
            velocity,
            attitude: Attitude::BODY_IS_WORLD,
            rotation: Vec3::ZERO,
        },
        motors: StartingMotors::Stopped,
        battery: 1.0,
        mount: Mount::Free,
        packet_rate: PacketRate::from_hz(250).unwrap(),
        flight_controller: Box::new(ScriptedMotors::new(Vec::new())),
    }
}

fn simulation(map: Vec<MapShape>, quads: Vec<QuadSetUp>) -> Simulation {
    Simulation::new(SetUp {
        physics_rate: PhysicsRate::from_hz(8000).unwrap(),
        world: World {
            gravity: 9.81,
            air_density: 1.225,
        },
        map,
        random_seed: 1,
        quads,
    })
    .unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn near_point(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn a_line_through_a_wall_goes_in_at_its_near_face_and_comes_out_at_its_far_one() {
    let sim = simulation(vec![ground(), wall()], Vec::new());
    let crossings = sim.line_question(Vec3::new(0.0, 0.0, 1.7), Vec3::new(10.0, 0.0, 1.7));
    assert_eq!(crossings.len(), 1, "{crossings:?}");
    let wall = crossings[0];
    assert_eq!(wall.shape, 1);
    assert!(near_point(wall.entry, Vec3::new(5.0, 0.0, 1.7)), "{wall:?}");
    assert!(near_point(wall.exit, Vec3::new(5.2, 0.0, 1.7)), "{wall:?}");
    assert!(near(wall.entry_distance, 5.0) && near(wall.exit_distance, 5.2));
}

#[test]
fn a_line_through_a_closed_mesh_goes_in_and_comes_out_where_its_triangles_face() {
    let sim = simulation(vec![ceiling()], Vec::new());
    let crossings = sim.line_question(Vec3::new(1.0, 2.0, 1.7), Vec3::new(1.0, 2.0, 5.0));
    assert_eq!(crossings.len(), 1, "{crossings:?}");
    assert!(near(crossings[0].entry.z, 3.1) && near(crossings[0].exit.z, 3.3));
    // The same slab, the other way.
    let down = sim.line_question(Vec3::new(1.0, 2.0, 5.0), Vec3::new(1.0, 2.0, 1.7));
    assert!(near(down[0].entry.z, 3.3) && near(down[0].exit.z, 3.1));
}

#[test]
fn the_line_question_lists_every_surface_in_order_along_the_line() {
    // From under the ceiling, out through the wall, up through the ceiling:
    // the line rises 1.6 m over 10 m, so it meets the wall first.
    let sim = simulation(vec![ceiling(), ground(), wall()], Vec::new());
    let from = Vec3::new(0.0, 0.0, 2.0);
    let to = Vec3::new(10.0, 0.0, 3.6);
    let crossings = sim.line_question(from, to);
    let shapes: Vec<usize> = crossings.iter().map(|c| c.shape).collect();
    assert_eq!(shapes, [2, 0], "{crossings:?}");
    for pair in crossings.windows(2) {
        assert!(pair[0].entry_distance <= pair[1].entry_distance);
    }
    for c in &crossings {
        assert!(c.entry_distance < c.exit_distance);
        let along = (to - from) / (to - from).length();
        assert!(near_point(c.entry, from + along * c.entry_distance));
        assert!(near_point(c.exit, from + along * c.exit_distance));
    }
}

#[test]
fn a_line_that_starts_or_ends_inside_a_surface_is_in_it_from_its_start_or_to_its_end() {
    let sim = simulation(vec![wall(), ceiling()], Vec::new());
    let from_inside = sim.line_question(Vec3::new(5.1, 0.0, 1.0), Vec3::new(8.0, 0.0, 1.0));
    assert_eq!(from_inside.len(), 1);
    assert!(near(from_inside[0].entry_distance, 0.0));
    assert!(near(from_inside[0].exit.x, 5.2));
    let into = sim.line_question(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 3.2));
    assert_eq!(into.len(), 1);
    assert!(near(into[0].entry.z, 3.1) && near(into[0].exit.z, 3.2));
}

#[test]
fn a_line_that_misses_everything_crosses_nothing() {
    let sim = simulation(vec![ground(), wall(), ceiling()], Vec::new());
    let crossings = sim.line_question(Vec3::new(0.0, 0.0, 1.0), Vec3::new(4.0, 3.0, 2.0));
    assert_eq!(crossings, Vec::<LineCrossing>::new());
    // A line of no length, too.
    let point = Vec3::new(5.1, 0.0, 1.0);
    assert!(sim.line_question(point, point).is_empty());
}

#[test]
fn asking_the_line_question_never_changes_the_simulation() {
    let map = || vec![ground(), wall(), ceiling()];
    let quad = || whoop_at(Vec3::new(4.0, 0.0, 1.0), Vec3::new(5.0, 0.0, 2.0));
    let mut asked = simulation(map(), vec![quad()]);
    let mut never_asked = simulation(map(), vec![quad()]);
    for step in 0..4000 {
        let before = asked.fingerprint();
        let from = Vec3::new(0.0, 0.0, 1.7);
        let crossings = asked.line_question(from, asked.quad_state(0).position);
        assert_eq!(asked.fingerprint(), before, "step {step}: {crossings:?}");
        asked.step();
        never_asked.step();
        assert_eq!(asked.fingerprint(), never_asked.fingerprint());
        assert_eq!(asked.contacts(0), never_asked.contacts(0));
    }
}

#[test]
fn a_map_shape_that_isnt_a_solid_is_refused_naming_it() {
    let flat = MapShape::Convex {
        corners: vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        ],
    };
    let missing_corner = MapShape::TriangleMesh {
        corners: vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)],
        triangles: vec![[0, 1, 2]],
    };
    let no_size = level_box(Vec3::ZERO, Vec3::new(1.0, 0.0, 1.0));
    for (shape, problem) in [
        (flat, MapShapeProblem::ConvexHasNoVolume),
        (missing_corner, MapShapeProblem::MeshCornerMissing),
        (no_size, MapShapeProblem::BoxHasNoSize),
    ] {
        let error = Simulation::new(SetUp {
            physics_rate: PhysicsRate::from_hz(8000).unwrap(),
            world: World {
                gravity: 9.81,
                air_density: 1.225,
            },
            map: vec![ground(), shape],
            random_seed: 1,
            quads: Vec::new(),
        })
        .err()
        .expect("refused");
        assert_eq!(error, SetUpError::MapShape { shape: 1, problem });
    }
}
