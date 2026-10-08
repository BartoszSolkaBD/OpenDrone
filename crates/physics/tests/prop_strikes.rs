//! Readable checks for Prop Strikes, stalled motors' restarts and the gyro's
//! range (#26 §1, §4 and §5, ADR-0012), beyond what the Physics Scenarios in
//! `scenarios/physics/` show through the Simulation. Basis: Rule, unless a
//! check says otherwise.
//!
//! The Quad here is the Whoop 65 without its duct rings, so its props are
//! open, like a 5″'s. Level, with its nose to the north-east, only its front
//! right prop (motor 2) reaches east of the rest, so a wall to the east meets
//! that prop's disc alone. Its motors are settled, spinning at the speed that
//! holds its weight.

mod common;

use common::{STEP, WORLD, whoop};
use opendrone_maths::{Attitude, DEGREE, PilotAngles, PilotRates, Vec3};
use opendrone_physics::{
    EscState, MapCollision, MapShape, MotorCommands, Mount, PropDirection, QuadBody,
    QuadParameters, QuadPart, QuadStart, QuadState, StartingMotors,
};

/// The drive that about holds the whoop's settled motors at their speed.
const HOVER: MotorCommands = MotorCommands::all(0.373);

/// Bluejay's minimum running speed on the whoop's 12-pole motors: 1,330
/// electrical RPM, in rad/s.
const MINIMUM: f64 = 1330.0 / 6.0 * 2.0 * core::f64::consts::PI / 60.0;

/// The whoop without its duct rings.
fn open_props() -> QuadParameters {
    let mut parameters = whoop();
    parameters.shape.duct_rings = None;
    parameters
}

/// How far east of the centre of mass prop 2's disc reaches, nose to the
/// north-east: its hub sits half the 66 mm diagonal due east, and the disc
/// reaches 17.5 mm beyond.
const PROP_2_REACH: f64 = 0.033 + 0.0175;

/// A wall 1 mm east of prop 2's disc, its face looking west.
fn wall() -> MapCollision {
    MapCollision::new(&[MapShape::Box {
        centre: Vec3::new(PROP_2_REACH + 0.001 + 0.1, 0.0, 0.0),
        size: Vec3::new(0.2, 20.0, 20.0),
        attitude: Attitude::BODY_IS_WORLD,
    }])
    .unwrap()
}

/// The Quad by the wall, flying north at 3 m/s and toward the wall at
/// `into` m/s.
fn by_the_wall(parameters: QuadParameters, into: f64) -> QuadBody {
    let start = QuadStart {
        state: QuadState {
            position: Vec3::ZERO,
            velocity: Vec3::new(into, 3.0, 0.0),
            attitude: Attitude::from_pilot_angles(PilotAngles {
                roll: 0.0,
                pitch: 0.0,
                heading: 45.0 * DEGREE,
            }),
            rotation: Vec3::ZERO,
        },
        motors: StartingMotors::Settled,
        battery: 1.0,
        mount: Mount::Free,
    };
    QuadBody::new(parameters, start, &WORLD).unwrap()
}

/// What one Prop Strike did, on the step prop 2 first touched the wall.
struct Strike {
    /// Prop 2's speed before and after the step, in rad/s.
    before: f64,
    after: f64,
    /// How hard prop 2 rubbed, in newtons.
    rub: f64,
    /// How the Quad's speed north changed, in m/s.
    pushed_north: f64,
    /// The Quad's yaw rate after the step, nose right positive, in rad/s.
    yaw: f64,
    /// How many steps it took to reach the wall.
    steps: u64,
}

/// Flies the Quad at the wall until prop 2 first touches it.
fn strike(parameters: QuadParameters, into: f64) -> Strike {
    let map = wall();
    let mut quad = by_the_wall(parameters, into);
    for steps in 1..8000 {
        let before = quad.motors()[1].speed;
        let north = quad.state().velocity.y;
        quad.step(&WORLD, &map, &HOVER, STEP);
        if let Some(contact) = quad.contacts().first() {
            assert_eq!(contact.part, QuadPart::PropDisc(2), "only prop 2 reaches");
            assert!(
                quad.contacts()
                    .iter()
                    .all(|c| c.part == QuadPart::PropDisc(2))
            );
            return Strike {
                before,
                after: quad.motors()[1].speed,
                rub: quad.prop_rubs()[1],
                pushed_north: quad.state().velocity.y - north,
                yaw: PilotRates::from_body(quad.state().rotation).yaw,
                steps,
            };
        }
    }
    panic!("prop 2 never reached the wall");
}

#[test]
fn the_harder_a_spinning_prop_is_pressed_the_harder_it_rubs_and_its_motor_is_braked() {
    let soft = strike(open_props(), 0.02);
    let hard = strike(open_props(), 0.05);
    assert!(soft.rub > 0.0 && hard.rub > soft.rub);
    let braked = |s: &Strike| s.before - s.after;
    assert!(braked(&soft) > 0.0);
    assert!(braked(&hard) > braked(&soft));
}

#[test]
fn a_rubbing_prop_pushes_and_twists_the_quad_by_its_spin_direction() {
    // Props in (Betaflight's default), prop 2 turns anticlockwise seen from
    // above: where it touches the wall, its east edge, its blades sweep north,
    // so the rub pushes the Quad south, east of its centre of mass, and turns
    // it clockwise: nose right.
    let props_in = strike(open_props(), 0.02);
    assert!(props_in.pushed_north < 0.0);
    assert!(props_in.yaw > 0.0);

    // Props out, prop 2 turns the other way, and so does everything else.
    let mut parameters = open_props();
    parameters.rotors.direction = PropDirection::PropsOut;
    let props_out = strike(parameters, 0.02);
    assert!(props_out.pushed_north > 0.0);
    assert!(props_out.yaw < 0.0);
    // The same rub, the other way: as big to within a few percent (the twist
    // changes how the Quad meets the wall a little).
    let ratio = -props_out.pushed_north / props_in.pushed_north;
    assert!((ratio - 1.0).abs() < 0.03, "{ratio}");
}

#[test]
fn a_hard_hit_stops_the_motor_at_once_and_a_graze_only_slows_it() {
    // A hard hit stops the prop's spin within the step. Sliding along the
    // wall at 3 m/s, the Quad then drags the stopped prop round backwards,
    // as a wheel rolls: either way it no longer turns the way its motor
    // drives it, faster than Bluejay's minimum speed, so its ESC finds it
    // stalled.
    let hit = strike(open_props(), 1.0);
    assert!(
        hit.after < MINIMUM,
        "a hard hit should stop prop 2 within the step, not leave it at {} rad/s",
        hit.after
    );
    let graze = strike(open_props(), 0.02);
    assert!(graze.after < graze.before);
    assert!(graze.after > graze.before / 2.0, "a graze only slows it");
}

#[test]
fn prop_grip_comes_from_the_quad_definition() {
    let with_grip = |grip| {
        let mut parameters = open_props();
        parameters.props.grip = grip;
        strike(parameters, 0.02)
    };
    // No grip, no rub: the prop's disc still stops the Quad going into the
    // wall, but the blades slide over it, and the motor isn't braked.
    let none = with_grip(0.0);
    assert_eq!(none.rub, 0.0);
    assert_eq!(none.pushed_north, 0.0);
    let free = {
        let mut quad = by_the_wall(open_props(), 0.02);
        let empty_air = MapCollision::default();
        for _ in 0..none.steps {
            quad.step(&WORLD, &empty_air, &HOVER, STEP);
        }
        quad.motors()[1].speed
    };
    assert_eq!(
        none.after, free,
        "without grip the wall doesn't touch the motor"
    );

    // More grip, more rub, and a slower prop.
    let little = with_grip(0.2);
    let much = with_grip(0.8);
    assert!(much.rub > 3.0 * little.rub);
    assert!(much.after < little.after);
    // A spinning prop's blade slides over the wall, so its rub is all its grip
    // allows: the grip times the push.
    for grip in [0.2, 0.5, 0.8] {
        let map = wall();
        let mut parameters = open_props();
        parameters.props.grip = grip;
        let mut quad = by_the_wall(parameters, 0.02);
        while quad.contacts().is_empty() {
            quad.step(&WORLD, &map, &HOVER, STEP);
        }
        let contact = quad.contacts()[0];
        assert!(
            (contact.rub / (grip * contact.push) - 1.0).abs() < 0.01,
            "grip {grip}: rub {} N, push {} N",
            contact.rub,
            contact.push
        );
    }
}

#[test]
fn a_stalled_motor_waits_the_start_wait_then_restarts_with_its_drive_capped() {
    // Source: Bluejay v0.21.0 (see the esc module): a hard hit stops prop 2;
    // its ESC switches the motor off for 0.1 s, then restarts it, capped at
    // Startup Power Max (1.96%) for 15 electrical turns, then runs it again.
    let map = wall();
    let mut quad = by_the_wall(open_props(), 1.0);
    while quad.contacts().is_empty() {
        quad.step(&WORLD, &map, &HOVER, STEP);
    }
    quad.step(&WORLD, &map, &HOVER, STEP);
    assert_eq!(quad.motors()[1].esc, EscState::RestartWait);
    assert_eq!(quad.motors()[1].drive, 0.0);
    // The others never stalled.
    assert_eq!(quad.motors()[0].esc, EscState::Running);

    let steps = |seconds: f64| (seconds / STEP).round() as u64;
    for _ in 1..steps(0.1) {
        quad.step(&WORLD, &map, &HOVER, STEP);
        assert_eq!(quad.motors()[1].esc, EscState::RestartWait);
    }
    quad.step(&WORLD, &map, &HOVER, STEP);
    let motor = quad.motors()[1];
    assert_eq!(motor.esc, EscState::Restarting);
    assert_eq!(motor.restarts, 1);
    assert!((motor.drive - 0.0196).abs() < 1e-12);

    // Free of the wall, the restart works and the motor runs again, its
    // count of restarts cleared.
    for _ in 0..steps(0.5) {
        quad.step(&WORLD, &map, &HOVER, STEP);
    }
    let motor = quad.motors()[1];
    assert_eq!(motor.esc, EscState::Running);
    assert_eq!(motor.restarts, 0);
    assert!(motor.speed > 1000.0);
}

#[test]
fn only_a_prop_disc_rubs_and_its_rub_is_the_size_of_its_friction() {
    let map = wall();
    let mut quad = by_the_wall(open_props(), 0.05);
    while quad.contacts().is_empty() {
        quad.step(&WORLD, &map, &HOVER, STEP);
    }
    for contact in quad.contacts() {
        assert!(contact.rub > 0.0);
        assert!((contact.rub - contact.friction.length()).abs() < 1e-12);
    }

    // A whoop resting on a floor touches it with its body box only: it is
    // held by friction there, but nothing rubs.
    let floor = MapCollision::new(&[MapShape::Box {
        centre: Vec3::new(0.0, 0.0, -1.0),
        size: Vec3::new(200.0, 200.0, 2.0),
        attitude: Attitude::BODY_IS_WORLD,
    }])
    .unwrap();
    let mut landed = QuadBody::new(
        open_props(),
        QuadStart {
            state: QuadState {
                position: Vec3::new(0.0, 0.0, 0.010),
                velocity: Vec3::ZERO,
                attitude: Attitude::BODY_IS_WORLD,
                rotation: Vec3::ZERO,
            },
            motors: StartingMotors::Stopped,
            battery: 1.0,
            mount: Mount::Free,
        },
        &WORLD,
    )
    .unwrap();
    landed.step(&WORLD, &floor, &MotorCommands::STOPPED, STEP);
    assert!(!landed.contacts().is_empty());
    assert!(landed.contacts().iter().all(|c| c.part == QuadPart::Body));
    assert!(landed.contacts().iter().all(|c| c.rub == 0.0));
    assert_eq!(landed.prop_rubs(), [0.0; 4]);
}

#[test]
fn the_gyro_reads_the_true_rotation_up_to_its_range_on_each_axis_on_its_own() {
    // #26 §4: the Quad definition's gyro range, ±2,000 °/s.
    let gyro = |rotation: Vec3| {
        let start = QuadStart {
            state: QuadState {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                attitude: Attitude::BODY_IS_WORLD,
                rotation,
            },
            motors: StartingMotors::Stopped,
            battery: 1.0,
            mount: Mount::Free,
        };
        QuadBody::new(whoop(), start, &WORLD).unwrap().gyro()
    };
    let within = Vec3::new(1999.0, -1500.0, 0.0) * DEGREE;
    assert_eq!(gyro(within), within);
    assert_eq!(
        gyro(Vec3::new(-3000.0, 1500.0, 2500.0) * DEGREE),
        Vec3::new(-2000.0, 1500.0, 2000.0) * DEGREE
    );
    assert_eq!(
        gyro(Vec3::new(10000.0, -2000.0, -2000.1) * DEGREE),
        Vec3::new(2000.0, -2000.0, -2000.0) * DEGREE
    );
}
