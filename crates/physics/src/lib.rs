//! Quad physics and Map collisions, including a "held on a thrust stand" set-up.
//!
//! It uses only `opendrone-maths`, and never the Flight Controller: the two
//! meet only inside `opendrone-sim`.
//!
//! So far it moves a Quad as a rigid body at a fixed step, with exact attitude
//! maths, under the Map's gravity: effects E1–E3 in the physics decisions
//! ([#10]). It collides with the Map (E29, #43). The other effects arrive with
//! their own tickets and Scenarios: the motor model and thrust (#41), drag and
//! the rest of the air (#42), Prop Strikes (#45) and so on.
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
//! 5. The collision stage (below): if that move would touch the Map, the Map
//!    pushes back, friction holds or drags, the Quad bounces and the move is
//!    redone, so no part ever passes through a surface between steps.
//!
//! # Collisions
//!
//! parry3d's 64-bit edition (`parry3d-f64`), in its `enhanced-determinism`
//! mode, answers the geometry questions only: what touches what and where,
//! and how far along a move the Quad first meets something ([ADR-0004]). The
//! bounce and the slide are ours. Its types never leave this crate: every
//! conversion is in the `geometry` module, the Quad's shape comes from its
//! definition ([`QuadShape`]) and the Map arrives as plain data
//! ([`MapShape`]). The same Map answers the read-only line question
//! ([`MapCollision::line_question`]).
//!
//! # The collision stage
//!
//! 1. **What could it reach?** Every surface within this step's reach: the
//!    distance its speed and spin can carry any part in one step, plus
//!    [`SKIN`], so a Quad resting on a surface always finds it.
//! 2. **Push and friction.** At each contact point the Map may push the Quad
//!    out, never pull it in, and friction may hold or drag it sideways, up to
//!    the Quad's friction times the push (Coulomb's law). A surface still a
//!    gap away may only be closed to, never crossed: that is the continuous
//!    check that stops a Quad passing through a thin rail between steps. The
//!    pushes are worked out together, point by point, [`PASSES`] times, with
//!    the Quad's mass and inertia, so a hit off its centre also spins it.
//! 3. **Bounce.** A point that hit at more than [`BOUNCE_SPEED`] comes back
//!    out at the Quad's bounce times the speed it came in at
//!    ([`BOUNCE_PASSES`] times through). A hit found a gap away bounces from
//!    where the step started, at most one step's travel short of the surface:
//!    3.75 mm at 30 m/s and 8 kHz. Friction was worked out in step 2, so it
//!    is capped by the push that stops the Quad, not by the bigger push that
//!    also bounces it. So on a slanting hit, friction takes at most friction
//!    × the speed into the surface off the speed along it, not friction ×
//!    (1 + bounce) × that. Many physics engines simplify the same way, and
//!    bounce and friction are Estimates that Feel Tests tune.
//! 4. **Rest.** When the Map holds the Quad (it pushes and nothing bounced)
//!    and what is left of its motion is below [`REST_SPEED`] and
//!    [`REST_TURN`], the Quad stays exactly still. Step 2 works the pushes out
//!    point by point a fixed number of times, so on a Quad resting on several
//!    points it leaves a tiny motion behind, which would make a landed Quad
//!    creep. A step's gravity alone gives a Quad 1.2 mm/s at 8 kHz, a hundred
//!    times [`REST_SPEED`], so a Quad that friction can't hold still slides
//!    away; only on a slope within about half a degree of the steepest its
//!    friction holds does it stay put instead of creeping off. The rule acts
//!    on each step's leftover motion, so what it holds depends on the physics
//!    rate: any net push below [`REST_SPEED`] per step's length (0.08 m/s² at
//!    8 kHz, 0.01 m/s² at 1 kHz) and any net turning below [`REST_TURN`] per
//!    step's length (about 0.8 rad/s² at 8 kHz) is held too.
//! 5. **The move,** redone from where the step started with the new speeds,
//!    as the free move does it.
//! 6. **The guard.** parry3d sweeps the Quad's shape along that move (a
//!    time-of-impact shape cast). If it meets a surface before the move ends,
//!    the Quad stops there: whatever step 2 missed, no part ever crosses a
//!    surface between steps.
//! 7. **Out of the Map.** A part more than [`SINK_ALLOWANCE`] inside a
//!    surface, as when a Scenario starts a Quad sunk into the floor, is moved
//!    straight out, without changing its speed.
//!
//! Nothing here remembers anything from one step to the next, so the
//! Simulation's state is still only each Quad's position, attitude and
//! speeds. Each step's [`Contact`]s say where the Map pushed.
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
//! [ADR-0004]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0004-parry3d-geometry-only-f64.md

mod collide;
mod geometry;
mod map;
mod shape;

use opendrone_maths::{Attitude, Fingerprinter, Mat3, Vec3};

pub use collide::{
    BOUNCE_PASSES, BOUNCE_SPEED, Contact, PASSES, REST_SPEED, REST_TURN, SINK_ALLOWANCE, SKIN,
};
pub use map::{LineCrossing, MapCollision, MapShape, MapShapeError, MapShapeProblem};
pub use shape::{DUCT_RING_SEGMENTS, DuctRings, PROP_DISC_THICKNESS, QuadPart, QuadShape};

use collide::{Collider, collide};
use shape::Part;

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
    /// The Quad's collision shape, and how it bounces and slides.
    pub shape: QuadShape,
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
        self.shape.write_fingerprint(f);
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
    /// Every size in the collision shape must be above zero, its bounce from
    /// 0 to 1 and its friction 0 or more.
    ShapeCantBeBuilt,
}

/// One Quad as a rigid body.
#[derive(Clone, Debug)]
pub struct QuadBody {
    parameters: QuadParameters,
    inertia_inverse: Mat3,
    /// The collision shape's simple shapes, built once.
    parts: Vec<Part>,
    /// How far the shape's farthest point is from the centre of mass.
    reach: f64,
    state: QuadState,
    /// Where the Map pushed the Quad during the last step.
    contacts: Vec<Contact>,
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
        if !parameters.shape.is_buildable() {
            return Err(SetUpProblem::ShapeCantBeBuilt);
        }
        let parts = parameters.shape.parts();
        let reach = parts
            .iter()
            .map(|part| part.reach)
            .fold(0.0, opendrone_maths::functions::max);
        Ok(QuadBody {
            parameters,
            inertia_inverse,
            parts,
            reach,
            state: start,
            contacts: Vec::new(),
        })
    }

    pub fn state(&self) -> &QuadState {
        &self.state
    }

    pub fn parameters(&self) -> &QuadParameters {
        &self.parameters
    }

    /// Every place where the Map pushed the Quad during the last step, in a
    /// fixed order: by part, then by Map shape.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Moves the Quad on by one fixed step of `dt` seconds through the Map's
    /// world values and solid parts (see the crate's "How one step moves the
    /// Quad").
    pub fn step(&mut self, world: &World, map: &MapCollision, dt: f64) {
        let before = self.state;
        self.move_freely(world, dt);
        let collider = Collider {
            mass: self.parameters.mass,
            inertia_inverse: self.inertia_inverse,
            bounce: self.parameters.shape.bounce,
            friction: self.parameters.shape.friction,
            parts: &self.parts,
            reach: self.reach,
        };
        collide(
            &collider,
            map,
            &before,
            &mut self.state,
            dt,
            &mut self.contacts,
        );
    }

    /// Steps 1 to 4: the move as if nothing were in the way.
    fn move_freely(&mut self, world: &World, dt: f64) {
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
