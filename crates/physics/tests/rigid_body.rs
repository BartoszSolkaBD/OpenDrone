//! Readable checks for the rigid body, beyond what the Physics Scenarios in
//! `scenarios/physics/` show. Basis: Rule (Euler's equations for a rigid
//! body).

use opendrone_maths::{Attitude, DEGREE, Mat3, PilotAngles, Vec3};
use opendrone_physics::{Drag, QuadBody, QuadParameters, QuadState, SetUpProblem, World};

const NO_DRAG: Drag = Drag {
    body_area: Vec3::ZERO,
    rotor: 0.0,
    duct_ram: 0.0,
};

fn whoop_sized(inertia: Vec3) -> QuadParameters {
    QuadParameters {
        mass: 0.0312,
        inertia: Mat3::diagonal(inertia),
        drag: NO_DRAG,
    }
}

fn still_at(attitude: Attitude, rotation: Vec3) -> QuadState {
    QuadState {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        attitude,
        rotation,
    }
}

#[test]
fn a_tumble_about_a_tilted_axis_keeps_its_angular_momentum() {
    // With no torque, angular momentum (the inertia times the rotation, seen
    // in the world) can't change. The rotation itself wanders between the
    // axes, which only the gyroscopic part of Euler's equation explains, so a
    // sign slip there breaks this.
    let inertia = Vec3::new(7.0e-6, 9.0e-6, 14.0e-6);
    let start = still_at(
        Attitude::from_pilot_angles(PilotAngles {
            roll: 10.0 * DEGREE,
            pitch: 20.0 * DEGREE,
            heading: 30.0 * DEGREE,
        }),
        Vec3::new(20.0, 3.0, 5.0),
    );
    let mut quad = QuadBody::new(whoop_sized(inertia), start).unwrap();
    let momentum = |state: &QuadState| {
        let body = Mat3::diagonal(inertia) * state.rotation;
        state.attitude.body_to_world(body)
    };
    let before = momentum(quad.state());
    let world = World {
        gravity: 9.81,
        air_density: 1.225,
    };
    for _ in 0..8000 {
        quad.step(&world, 1.0 / 8000.0);
    }
    let after = momentum(quad.state());
    let drift = (after - before).length() / before.length();
    assert!(
        drift < 1e-3,
        "angular momentum drifted by {:.4} %",
        drift * 100.0
    );
    assert!(
        (quad.state().rotation - start.rotation).length() > 1.0,
        "the rotation should have wandered between the axes"
    );
}

#[test]
fn a_quad_without_mass_cant_be_set_up() {
    let mut parameters = whoop_sized(Vec3::new(1.0, 1.0, 1.0));
    parameters.mass = 0.0;
    let start = still_at(Attitude::BODY_IS_WORLD, Vec3::ZERO);
    assert_eq!(
        QuadBody::new(parameters, start).unwrap_err(),
        SetUpProblem::MassNotAboveZero
    );
}

#[test]
fn a_quad_whose_inertia_has_no_inverse_cant_be_set_up() {
    let parameters = whoop_sized(Vec3::new(1.0, 0.0, 1.0));
    let start = still_at(Attitude::BODY_IS_WORLD, Vec3::ZERO);
    assert_eq!(
        QuadBody::new(parameters, start).unwrap_err(),
        SetUpProblem::InertiaHasNoInverse
    );
}
