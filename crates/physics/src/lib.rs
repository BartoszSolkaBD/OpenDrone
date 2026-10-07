//! Quad physics and Map collisions, including a "held on a thrust stand" set-up.
//!
//! It uses only `opendrone-maths`, and never the Flight Controller: the two
//! meet only inside `opendrone-sim`.
//!
//! So far it moves a Quad as a rigid body at a fixed step, with exact attitude
//! maths, under the Map's gravity: effects E1–E3 in the physics decisions
//! ([#10]). The other effects arrive with their own tickets and Scenarios: the
//! motor model and thrust (#41), drag and the rest of the air (#42),
//! collisions (#43) and so on.
//!
//! # How one step moves the Quad
//!
//! [`QuadBody::step`] moves the Quad on by one fixed step of `dt` seconds:
//!
//! 1. The forces on it give its acceleration. So far that is the Map's
//!    gravity alone.
//! 2. Semi-implicit Euler: the speed changes first, then the position moves
//!    with the new speed.
//! 3. The rotation changes by Euler's equation for a rigid body,
//!    `J·dω/dt = M − ω × J·ω`, with `J` the inertia, `ω` the rotation in body
//!    axes and `M` the torques (none yet). The `ω × J·ω` part is the
//!    gyroscopic coupling between the axes.
//! 4. The attitude turns by the new rotation over the step, with the
//!    exponential map, which is exact for a steady rotation and never drifts
//!    (flight-dynamics research §2.3).
//!
//! # House rules
//!
//! This is a core crate: part of the Simulation, which gives bit-identical
//! results on every computer ([ADR-0001], [ADR-0003]). So it follows the house
//! rules:
//!
//! - Every maths function comes from `libm`, never from std's float methods
//!   such as `f64::sin` or `f64::powf`, which give different results on
//!   different platforms.
//! - No `HashMap` or `HashSet`: their order changes from run to run. Use
//!   `BTreeMap`, `BTreeSet` or a `Vec`.
//! - No clock, no files, no Bevy and nothing else that touches the operating
//!   system.
//!
//! Clippy enforces the first two (and catches the usual clock and file calls)
//! with this crate's `clippy.toml`, which only the five core crates have.
//! `cargo xtask walls` checks the dependencies.
//!
//! [#10]: https://github.com/BartoszSolkaBD/OpenDrone/issues/10
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use opendrone_maths::{Attitude, Fingerprinter, Mat3, Vec3};

/// The Map's world values: what the air and the planet are like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    /// How hard gravity pulls straight down, in m/s².
    pub gravity: f64,
    /// The air's density, in kg/m³. Nothing reads it until the air effects
    /// arrive (#42); it is part of the set-up already, so a Map's fingerprint
    /// covers it.
    pub air_density: f64,
}

impl World {
    /// Feeds every world value into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64(self.gravity);
        f.write_f64(self.air_density);
    }
}

/// The numbers from a Quad definition that the physics receives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadParameters {
    /// All-up mass, in kg: the dry mass plus the battery.
    pub mass: f64,
    /// Inertia in body axes (forward, left, up), in kg·m².
    pub inertia: Mat3,
    pub drag: Drag,
}

/// The Quad's drag numbers.
///
/// They are part of the set-up the Simulation receives, so the Quad's
/// fingerprint covers them, but no drag force acts yet: drag arrives with the
/// air ticket (#42). Scenarios that must stay free of drag when it does, such
/// as free fall, use a Test Quad with all of these set to zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Body drag area (drag coefficient × area) facing forward, sideways and
    /// up, in m².
    pub body_area: Vec3,
    /// Rotor drag, in s⁻¹.
    pub rotor: f64,
    /// Whoop duct ram drag, in s⁻¹; zero for a Quad without ducts.
    pub duct_ram: f64,
}

impl QuadParameters {
    /// Feeds every parameter into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_f64(self.mass);
        f.write_f64s(&self.inertia.numbers());
        f.write_f64s(&[
            self.drag.body_area.x,
            self.drag.body_area.y,
            self.drag.body_area.z,
            self.drag.rotor,
            self.drag.duct_ram,
        ]);
    }
}

/// Where a Quad is and how it moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadState {
    /// From the Map's origin, in world axes (east, north, up), in metres.
    pub position: Vec3,
    /// In world axes, in m/s.
    pub velocity: Vec3,
    pub attitude: Attitude,
    /// The rotation in body axes (forward, left, up), in radians per second.
    pub rotation: Vec3,
}

impl QuadState {
    /// True when every number is a real number: not "not a number", and not
    /// endless.
    pub fn is_finite(&self) -> bool {
        self.position.is_finite()
            && self.velocity.is_finite()
            && self.attitude.is_finite()
            && self.rotation.is_finite()
    }

    /// Feeds every number into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let Vec3 { x, y, z } = self.position;
        f.write_f64s(&[x, y, z]);
        let Vec3 { x, y, z } = self.velocity;
        f.write_f64s(&[x, y, z]);
        f.write_f64s(&self.attitude.quaternion());
        let Vec3 { x, y, z } = self.rotation;
        f.write_f64s(&[x, y, z]);
    }
}

/// A set-up the physics can't move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetUpProblem {
    /// The mass must be above zero, and a real number.
    MassNotAboveZero,
    /// The inertia matrix has no inverse, so the Quad couldn't turn.
    InertiaHasNoInverse,
}

/// One Quad as a rigid body.
#[derive(Clone, Debug)]
pub struct QuadBody {
    parameters: QuadParameters,
    inertia_inverse: Mat3,
    state: QuadState,
}

impl QuadBody {
    pub fn new(parameters: QuadParameters, start: QuadState) -> Result<QuadBody, SetUpProblem> {
        // Written so that a "not a number" mass fails too.
        if !(parameters.mass > 0.0 && parameters.mass.is_finite()) {
            return Err(SetUpProblem::MassNotAboveZero);
        }
        let inertia_inverse = parameters
            .inertia
            .inverse()
            .ok_or(SetUpProblem::InertiaHasNoInverse)?;
        Ok(QuadBody {
            parameters,
            inertia_inverse,
            state: start,
        })
    }

    pub fn state(&self) -> &QuadState {
        &self.state
    }

    pub fn parameters(&self) -> &QuadParameters {
        &self.parameters
    }

    /// Moves the Quad on by one fixed step of `dt` seconds (see the crate's
    /// "How one step moves the Quad").
    pub fn step(&mut self, world: &World, dt: f64) {
        let state = &mut self.state;

        let gravity = Vec3::new(0.0, 0.0, -world.gravity);
        let acceleration = gravity;
        state.velocity += acceleration * dt;
        state.position += state.velocity * dt;

        let torque = Vec3::ZERO;
        let inertia = self.parameters.inertia;
        let w = state.rotation;
        let angular_acceleration = self.inertia_inverse * (torque - w.cross(inertia * w));
        state.rotation += angular_acceleration * dt;
        state.attitude = state.attitude.turned_by(state.rotation * dt);
    }

    /// Feeds the Quad's whole state into a fingerprint.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.state.write_fingerprint(f);
    }
}
