//! The Simulation: Flight Inputs, Radio Link, Assists, Flight Controller,
//! physics and session state.
//!
//! It is the only place where the physics and the Flight Controller meet.
//! Everything that can change a flight enters as Flight Inputs stamped with
//! Simulation Time.
//!
//! So far it holds:
//!
//! - [`SetUp`]: what a Simulation starts from. The world values and the Quad
//!   definitions come from `opendrone-pack`, already checked; the physics rate
//!   and the random seed from the caller.
//! - [`Simulation`]: steps every Quad once per tick, in a fixed order, and
//!   counts [`SimulationTime`] in whole ticks at the set-up's [`PhysicsRate`]
//!   (8 kHz in the alpha), never with the computer's clock.
//! - [`FlightControllerSeam`]: what turns readings into the four motor
//!   commands each tick. So far only the [`ScriptedMotors`] stand-in plugs in,
//!   for Physics Scenarios; our Flight Controller follows (#48), and later
//!   perhaps a SITL bridge in its own crate.
//! - [`Simulation::fingerprint`]: a fingerprint of the whole state, the same on
//!   every computer, for the repeat and agreement checks.
//! - After each tick, every Quad's state and its contacts with the Map
//!   ([`Simulation::contacts`]).
//! - The read-only line question ([`Simulation::line_question`]): which Map
//!   surfaces a straight line passes through, and where it goes in and comes
//!   out, for the Video Signal and the Where-you-stand sound. It never
//!   changes the Simulation.
//!
//! The Radio Link, Assists, Rates and the session state arrive with their
//! tickets.
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
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

mod motors;
mod time;

use opendrone_maths::{Fingerprint, Fingerprinter, Vec3};
use opendrone_physics::{MapCollision, QuadBody};

pub use motors::{
    FlightControllerSeam, MotorCommand, MotorCommands, ScriptedMotors, SpinDirection,
};
pub use opendrone_physics::{
    Contact, Drag, DuctRings, LineCrossing, MapShape, MapShapeProblem, QuadParameters, QuadPart,
    QuadShape, QuadState, SetUpProblem, World,
};
pub use time::{PhysicsRate, SimulationTime};

/// What a Simulation starts from.
pub struct SetUp {
    /// How many physics steps make one second of Simulation Time.
    pub physics_rate: PhysicsRate,
    /// The Map's world values.
    pub world: World,
    /// The Map's solid parts, as plain data, in a fixed order: contacts and
    /// the line question name them by their place in this list.
    pub map: Vec<MapShape>,
    /// The seed for the Simulation's random numbers, kept in its state. Nothing
    /// draws random numbers yet; Prop Wash will (#46).
    pub random_seed: u64,
    /// Every Quad, in the fixed order they are stepped in. The alpha flies one.
    pub quads: Vec<QuadSetUp>,
}

/// One Quad's set-up.
pub struct QuadSetUp {
    pub parameters: QuadParameters,
    pub start: QuadState,
    /// What plugs into the Flight Controller seam for this Quad.
    pub flight_controller: Box<dyn FlightControllerSeam>,
}

/// A set-up the Simulation can't start from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetUpError {
    /// A Quad the physics can't move: which one, counting from 0 in the
    /// set-up's order, and why.
    Quad { quad: usize, problem: SetUpProblem },
    /// A Map shape that can't be a solid part: which one, counting from 0 in
    /// the set-up's order, and why.
    MapShape {
        shape: usize,
        problem: MapShapeProblem,
    },
}

/// The world that moves every Quad forward in fixed steps.
pub struct Simulation {
    physics_rate: PhysicsRate,
    world: World,
    /// The Map's solid parts. They never change, so they are set-up rather
    /// than state: the Map's fingerprint covers them, the Simulation's
    /// doesn't.
    map: MapCollision,
    random_seed: u64,
    time: SimulationTime,
    quads: Vec<SimulatedQuad>,
}

struct SimulatedQuad {
    body: QuadBody,
    flight_controller: Box<dyn FlightControllerSeam>,
    motor_commands: MotorCommands,
}

impl Simulation {
    pub fn new(set_up: SetUp) -> Result<Simulation, SetUpError> {
        let map = MapCollision::new(&set_up.map).map_err(|error| SetUpError::MapShape {
            shape: error.shape,
            problem: error.problem,
        })?;
        let mut quads = Vec::with_capacity(set_up.quads.len());
        for (index, quad) in set_up.quads.into_iter().enumerate() {
            let body =
                QuadBody::new(quad.parameters, quad.start).map_err(|problem| SetUpError::Quad {
                    quad: index,
                    problem,
                })?;
            quads.push(SimulatedQuad {
                body,
                flight_controller: quad.flight_controller,
                motor_commands: MotorCommands::STOPPED,
            });
        }
        Ok(Simulation {
            physics_rate: set_up.physics_rate,
            world: set_up.world,
            map,
            random_seed: set_up.random_seed,
            time: SimulationTime::START,
            quads,
        })
    }

    /// One tick: for every Quad in order, the Flight Controller seam gives the
    /// motor commands, then the physics moves the Quad on by one step,
    /// colliding with the Map. Then Simulation Time moves on by one step.
    pub fn step(&mut self) {
        let dt = self.physics_rate.step_length();
        for quad in &mut self.quads {
            quad.motor_commands = quad.flight_controller.step(self.time);
            quad.body.step(&self.world, &self.map, dt);
        }
        self.time = self.time.next();
    }

    pub fn time(&self) -> SimulationTime {
        self.time
    }

    pub fn physics_rate(&self) -> PhysicsRate {
        self.physics_rate
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// How many Quads there are.
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }

    /// A Quad's state after the last tick, counting from 0 in the set-up's
    /// order.
    pub fn quad_state(&self, quad: usize) -> &QuadState {
        self.quads[quad].body.state()
    }

    /// Every place where the Map pushed a Quad during the last tick, by part
    /// and then by Map shape (none before the first tick).
    pub fn contacts(&self, quad: usize) -> &[Contact] {
        self.quads[quad].body.contacts()
    }

    /// The line question: every Map surface the straight line from `from` to
    /// `to` (world axes, metres) passes through, with where the line goes in
    /// and comes out, in order along the line. Each names its Map shape by its
    /// place in the set-up's list. It reads the Map only, so it never changes
    /// the Simulation, and it may be asked between any two ticks.
    pub fn line_question(&self, from: Vec3, to: Vec3) -> Vec<LineCrossing> {
        self.map.line_question(from, to)
    }

    /// The motor commands the Flight Controller seam gave a Quad on the last
    /// tick (all stopped before the first).
    pub fn motor_commands(&self, quad: usize) -> MotorCommands {
        self.quads[quad].motor_commands
    }

    /// True when every number in every Quad's state is a real number.
    pub fn is_finite(&self) -> bool {
        self.quads
            .iter()
            .all(|quad| quad.body.state().is_finite() && quad.motor_commands.is_finite())
    }

    /// A fingerprint of the whole state, in a fixed order: Simulation Time,
    /// the physics rate, the world, the random seed, then every Quad's state,
    /// motor commands and Flight Controller seam. Two runs, or two computers,
    /// that give the same fingerprint are in exactly the same state.
    ///
    /// The Map's solid parts never change, so they are left out, as the
    /// Quads' parameters are; the Results' Map fingerprint covers them. So
    /// are the contacts: they are what happened during the last tick, worked
    /// out from the state, and every push they report is already in the
    /// Quad's speeds.
    pub fn fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        f.write_u64(self.time.ticks());
        f.write_u64(u64::from(self.physics_rate.hz()));
        self.world.write_fingerprint(&mut f);
        f.write_u64(self.random_seed);
        f.write_u64(self.quads.len() as u64);
        for quad in &self.quads {
            quad.body.write_fingerprint(&mut f);
            quad.motor_commands.write_fingerprint(&mut f);
            quad.flight_controller.write_fingerprint(&mut f);
        }
        f.finish()
    }
}
