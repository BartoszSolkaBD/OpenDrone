//! Readable checks for collisions with the Map (E29 in #10, ADR-0004,
//! ADR-0012), beyond what the Physics Scenarios in `scenarios/physics/` show
//! through the Simulation. Basis: Rule, unless a check says otherwise.
//!
//! The Quads here carry the alpha Quads' shapes and masses (the Whoop 65's
//! and the Freestyle 5″'s Quad definitions, written out, since physics can't
//! read Packs); the Maps are built from plain shapes, as the Test Maps are.

use opendrone_maths::functions::{max, sin_cos};
use opendrone_maths::{Attitude, DEGREE, Mat3, PilotAngles, Vec3};
use opendrone_physics::{
    Contact, Drag, DuctRings, MapCollision, MapShape, QuadBody, QuadParameters, QuadPart,
    QuadShape, QuadState, World,
};

const WORLD: World = World {
    gravity: 9.81,
    air_density: 1.225,
};

const NO_DRAG: Drag = Drag {
    body_area: Vec3::ZERO,
    rotor: 0.0,
    duct_ram: 0.0,
};

/// The Whoop 65: 31.2 g, its body box 20 mm tall round the centre of mass.
fn whoop() -> QuadParameters {
    QuadParameters {
        mass: 0.0312,
        inertia: Mat3::diagonal(Vec3::new(7.0e-6, 9.0e-6, 14.0e-6)),
        drag: NO_DRAG,
        shape: QuadShape {
            body: Vec3::new(0.035, 0.030, 0.020),
            pack: Vec3::new(0.064, 0.010, 0.006),
            pack_height: -0.006,
            diagonal: 0.066,
            rotor_height: 0.008,
            prop_diameter: 0.035,
            duct_rings: Some(DuctRings {
                inside_diameter: 0.037,
                wall: 0.0015,
                height: 0.014,
            }),
            bounce: 0.3,
            friction: 0.5,
        },
    }
}

/// The Freestyle 5″: 644 g, its pack on top, its props 5 mm below the centre
/// of mass.
fn freestyle() -> QuadParameters {
    QuadParameters {
        mass: 0.644,
        inertia: Mat3::diagonal(Vec3::new(1.4e-3, 1.5e-3, 2.5e-3)),
        drag: NO_DRAG,
        shape: QuadShape {
            body: Vec3::new(0.080, 0.045, 0.035),
            pack: Vec3::new(0.075, 0.035, 0.040),
            pack_height: 0.026,
            diagonal: 0.225,
            rotor_height: -0.005,
            prop_diameter: 0.1295,
            duct_rings: None,
            bounce: 0.3,
            friction: 0.5,
        },
    }
}

/// How far the Whoop 65's centre of mass sits above a floor it rests on:
/// half its body box.
const WHOOP_RESTING_HEIGHT: f64 = 0.010;

fn ground() -> MapShape {
    MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -1.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    }
}

fn map(shapes: Vec<MapShape>) -> MapCollision {
    MapCollision::new(&shapes).unwrap()
}

/// A round rail as a 16-sided convex shape, `diameter` across, from `start`
/// to `end`, running north–south or standing up.
fn rail(start: Vec3, end: Vec3, diameter: f64) -> MapShape {
    let runs_north = start.x == end.x && start.z == end.z;
    let mut corners = Vec::new();
    for k in 0..16 {
        let angle = 2.0 * core::f64::consts::PI * f64::from(k) / 16.0;
        let (sine, cosine) = sin_cos(angle);
        let r = diameter / 2.0;
        let around = if runs_north {
            Vec3::new(r * cosine, 0.0, r * sine)
        } else {
            Vec3::new(r * cosine, r * sine, 0.0)
        };
        corners.push(start + around);
        corners.push(end + around);
    }
    MapShape::Convex { corners }
}

fn state(position: Vec3, velocity: Vec3, attitude: Attitude) -> QuadState {
    QuadState {
        position,
        velocity,
        attitude,
        rotation: Vec3::ZERO,
    }
}

fn level(heading: f64) -> Attitude {
    Attitude::from_pilot_angles(PilotAngles {
        roll: 0.0,
        pitch: 0.0,
        heading,
    })
}

/// Steps a Quad `steps` times at `hz`, calling `each` after every step.
fn fly(
    quad: &mut QuadBody,
    map: &MapCollision,
    hz: f64,
    steps: u64,
    mut each: impl FnMut(&QuadBody),
) {
    for _ in 0..steps {
        quad.step(&WORLD, map, 1.0 / hz);
        each(quad);
    }
}

#[test]
fn a_landed_whoop_stays_exactly_still_with_no_creep_or_jitter() {
    let floor = map(vec![ground()]);
    let start = state(
        Vec3::new(1.0, 2.0, WHOOP_RESTING_HEIGHT),
        Vec3::ZERO,
        level(30.0 * DEGREE),
    );
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    // Ten seconds here; the Scenario landed-whoop-stays-still checks minutes.
    fly(&mut quad, &floor, 8000.0, 80_000, |quad| {
        assert_eq!(quad.state().position, start.position);
        assert_eq!(quad.state().attitude, start.attitude);
        assert_eq!(quad.state().velocity, Vec3::ZERO);
        assert_eq!(quad.state().rotation, Vec3::ZERO);
    });
}

#[test]
fn landed_the_floors_pushes_add_up_to_the_quads_weight() {
    let floor = map(vec![ground()]);
    let start = state(
        Vec3::new(0.0, 0.0, WHOOP_RESTING_HEIGHT),
        Vec3::ZERO,
        level(0.0),
    );
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    fly(&mut quad, &floor, 8000.0, 10, |_| {});
    let contacts = quad.contacts();
    assert!(!contacts.is_empty());
    let weight = 0.0312 * 9.81;
    let push: f64 = contacts.iter().map(|c| c.push * c.normal.z).sum();
    assert!(
        (push - weight).abs() < weight * 1e-6,
        "the floor pushed {push} N against a weight of {weight} N"
    );
    // Nothing pulls it sideways, so the friction at its corners adds up to
    // nothing.
    let friction = contacts.iter().fold(Vec3::ZERO, |sum, c| sum + c.friction);
    assert!(friction.length() < weight * 1e-6, "{friction:?}");
    // The whoop rests on its body box: nothing else reaches the floor.
    for contact in contacts {
        assert_eq!(contact.part, QuadPart::Body);
        assert_eq!(contact.shape, 0);
        assert_eq!(contact.normal, Vec3::new(0.0, 0.0, 1.0));
        assert!(contact.point.z.abs() < 1e-9);
    }
}

#[test]
fn an_upside_down_5_inch_rests_on_its_pack_with_its_props_clear() {
    // #26 §1: the pack sits on top of the 5″, 26 mm above the centre of mass
    // and 40 mm tall, so upside down its far face is 46 mm from the centre.
    let floor = map(vec![ground()]);
    let upside_down = Attitude::from_pilot_angles(PilotAngles {
        roll: 180.0 * DEGREE,
        pitch: 0.0,
        heading: 0.0,
    });
    let start = state(Vec3::new(0.0, 0.0, 0.046), Vec3::ZERO, upside_down);
    let mut quad = QuadBody::new(freestyle(), start).unwrap();
    fly(&mut quad, &floor, 8000.0, 8000, |_| {});
    let contacts = quad.contacts();
    assert!(!contacts.is_empty());
    assert!(
        contacts.iter().all(|c| c.part == QuadPart::Pack),
        "{contacts:?}"
    );
    assert_eq!(quad.state().position, start.position);
}

#[test]
fn a_dropped_whoop_comes_back_up_at_its_bounce_times_its_landing_speed() {
    // Dropped level from 1 m above where it rests, it lands on its body box's
    // four bottom corners at once, so the floor pushes straight up through
    // its centre of mass: no spin, and it leaves at bounce × landing speed.
    let floor = map(vec![ground()]);
    let start = state(
        Vec3::new(0.0, 0.0, WHOOP_RESTING_HEIGHT + 1.0),
        Vec3::ZERO,
        level(0.0),
    );
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    let mut landing_speed = 0.0_f64;
    let mut leaving_speed = None;
    let mut highest_after = 0.0_f64;
    fly(&mut quad, &floor, 8000.0, 8000, |quad| {
        let v = quad.state().velocity.z;
        if leaving_speed.is_none() {
            if v < 0.0 {
                landing_speed = -v;
            } else {
                leaving_speed = Some(v);
            }
        } else {
            highest_after = max(highest_after, quad.state().position.z);
        }
    });
    let landing_speed = landing_speed;
    let leaving_speed = leaving_speed.expect("it bounced");
    // About √(2 g h) = 4.43 m/s, less the last step's fall.
    assert!((landing_speed - 4.43).abs() < 0.01, "{landing_speed}");
    assert!(
        (leaving_speed - 0.3 * landing_speed).abs() < 0.3 * landing_speed * 1e-3,
        "left at {leaving_speed} m/s after landing at {landing_speed} m/s"
    );
    // It rises (0.3 × 4.43)² / (2 × 9.81) = 0.090 m.
    let rise = highest_after - WHOOP_RESTING_HEIGHT;
    assert!((rise - 0.090).abs() < 0.002, "rose {rise} m");
    assert_eq!(quad.state().rotation, Vec3::ZERO);
}

#[test]
fn a_whoop_sliding_on_the_floor_stops_where_coulomb_friction_says() {
    // Friction 0.5 slows it at 0.5 × 9.81 = 4.905 m/s², so from 3 m/s it
    // stops after 3² / (2 × 4.905) = 0.917 m.
    let floor = map(vec![ground()]);
    let start = state(
        Vec3::new(0.0, 0.0, WHOOP_RESTING_HEIGHT),
        Vec3::new(3.0, 0.0, 0.0),
        level(90.0 * DEGREE),
    );
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    fly(&mut quad, &floor, 8000.0, 8000, |_| {});
    let slid = quad.state().position.x;
    assert!((slid - 0.917).abs() < 0.005, "slid {slid} m");
    assert_eq!(quad.state().velocity, Vec3::ZERO);
    assert!((quad.state().position.z - WHOOP_RESTING_HEIGHT).abs() < 1e-4);
}

/// Flies `quad` at 30 m/s east into `target`, which stands across its path
/// at `x` metres east, `half_width` either side of that, and checks no part
/// of it ever gets past the near side.
fn never_passes(
    parameters: QuadParameters,
    target: MapShape,
    x: f64,
    half_width: f64,
    height: f64,
    hz: f64,
) -> Vec<Contact> {
    let map = map(vec![ground(), target]);
    let start = state(
        Vec3::new(x - 1.0, 0.0, height),
        Vec3::new(30.0, 0.0, 0.0),
        level(90.0 * DEGREE),
    );
    let mut quad = QuadBody::new(parameters, start).unwrap();
    let mut furthest = f64::MIN;
    let mut hit = Vec::new();
    fly(&mut quad, &map, hz, (hz * 0.1) as u64, |quad| {
        furthest = max(furthest, quad.state().position.x);
        hit.extend(quad.contacts().iter().filter(|c| c.shape == 1));
    });
    assert!(
        furthest < x - half_width,
        "the centre reached {furthest} m east, past the near side at {} m",
        x - half_width
    );
    assert!(!hit.is_empty());
    hit
}

#[test]
fn at_30_m_s_a_whoop_never_passes_through_a_6_cm_rail() {
    let rail = rail(Vec3::new(3.0, -5.0, 1.0), Vec3::new(3.0, 5.0, 1.0), 0.06);
    let hit = never_passes(whoop(), rail, 3.0, 0.03, 1.0, 8000.0);
    // The front ducts are the first thing it meets.
    assert!(
        hit.iter()
            .any(|c| matches!(c.part, QuadPart::DuctRing(2 | 4))),
        "{hit:?}"
    );
}

#[test]
fn at_30_m_s_a_5_inch_never_passes_through_a_3_cm_rebar_stub() {
    let rebar = rail(Vec3::new(3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 2.0), 0.03);
    never_passes(freestyle(), rebar, 3.0, 0.015, 1.0, 8000.0);
}

#[test]
fn even_stepping_at_1_khz_a_whoop_never_passes_a_3_cm_rebar_stub() {
    // At 1 kHz the whoop moves 30 mm a step, twenty times its duct wall:
    // only the continuous check stops the rebar slipping inside the duct.
    let rebar = rail(Vec3::new(3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 2.0), 0.03);
    never_passes(whoop(), rebar, 3.0, 0.015, 1.0, 1000.0);
}

#[test]
fn even_stepping_at_1_khz_a_quad_never_passes_a_wall_with_no_thickness() {
    // A single sheet of two triangles facing west, as a Map's mesh may be.
    let sheet = MapShape::TriangleMesh {
        corners: vec![
            Vec3::new(3.0, -5.0, 0.0),
            Vec3::new(3.0, 5.0, 0.0),
            Vec3::new(3.0, 5.0, 3.0),
            Vec3::new(3.0, -5.0, 3.0),
        ],
        triangles: vec![[0, 2, 1], [0, 3, 2]],
    };
    never_passes(whoop(), sheet.clone(), 3.0, 0.0, 1.0, 1000.0);
    never_passes(freestyle(), sheet, 3.0, 0.0, 1.0, 1000.0);
}

#[test]
fn a_whoop_sliding_along_a_wall_touches_it_with_its_duct_rings_never_its_props() {
    // #26 §6: the duct ring takes the contact, so a whoop scraping a wall
    // never has a Prop Strike.
    let wall = MapShape::Box {
        centre: Vec3::new(1.1, 0.0, 2.5),
        size: Vec3::new(0.2, 20.0, 5.0),
        attitude: Attitude::BODY_IS_WORLD,
    };
    let map = map(vec![ground(), wall]);
    let start = state(
        Vec3::new(0.9, -2.0, 1.5),
        Vec3::new(1.0, 3.0, 0.0),
        level(0.0),
    );
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    let mut parts = Vec::new();
    fly(&mut quad, &map, 8000.0, 4000, |quad| {
        parts.extend(
            quad.contacts()
                .iter()
                .filter(|c| c.shape == 1)
                .map(|c| c.part),
        );
    });
    assert!(!parts.is_empty(), "it should have touched the wall");
    assert!(
        parts.iter().all(|p| matches!(p, QuadPart::DuctRing(_))),
        "{parts:?}"
    );
}

#[test]
fn a_quad_started_sunk_into_the_floor_is_moved_out_without_being_thrown() {
    let floor = map(vec![ground()]);
    let start = state(Vec3::new(0.0, 0.0, 0.005), Vec3::ZERO, level(0.0));
    let mut quad = QuadBody::new(whoop(), start).unwrap();
    fly(&mut quad, &floor, 8000.0, 8000, |quad| {
        assert!(quad.state().velocity.length() < 0.01);
    });
    let height = quad.state().position.z;
    assert!(
        (height - WHOOP_RESTING_HEIGHT).abs() <= 1.1e-4,
        "it rests at {height} m"
    );
}

#[test]
fn in_empty_air_collisions_change_nothing() {
    let empty = MapCollision::default();
    let floor_far_below = map(vec![MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -101.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    }]);
    let start = QuadState {
        position: Vec3::new(0.0, 0.0, 1.0),
        velocity: Vec3::new(3.0, -2.0, 1.0),
        attitude: level(45.0 * DEGREE),
        rotation: Vec3::new(20.0, 3.0, 5.0),
    };
    let mut alone = QuadBody::new(whoop(), start).unwrap();
    let mut over_a_floor = QuadBody::new(whoop(), start).unwrap();
    for _ in 0..8000 {
        alone.step(&WORLD, &empty, 1.0 / 8000.0);
        over_a_floor.step(&WORLD, &floor_far_below, 1.0 / 8000.0);
        assert_eq!(alone.state(), over_a_floor.state());
        assert!(over_a_floor.contacts().is_empty());
    }
}
