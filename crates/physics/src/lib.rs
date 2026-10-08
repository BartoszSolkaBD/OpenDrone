//! Quad physics and Map collisions, including a "held on a thrust stand" set-up.
//!
//! It uses only `opendrone-maths`, and never the Flight Controller: the two
//! meet only inside `opendrone-sim`.
//!
//! So far it holds, from the physics decisions ([#10]):
//!
//! - the rigid body: motion at a fixed step with exact attitude maths, under
//!   the Map's gravity (E1–E3);
//! - the motors: thrust and the props' drag torque growing with the square of
//!   each motor's speed, from a motor model driven by the battery's voltage,
//!   with separate spin-up and slow-down times (E4–E6, E9; [`motor`] and
//!   ADR-0006);
//! - each motor's ESC, copying Bluejay's power-up, start wait and start-up
//!   power limit ([`esc`]);
//! - the battery: its voltage curve, sag, recovery and charge counting, with
//!   no cutoff (E13, E14; [`battery`]);
//! - collisions with the Map (E29, #43; see "Collisions" below);
//! - the thrust stand: a Quad held still, its motors, ESCs and battery working
//!   as in flight ([`Mount::ThrustStand`]);
//! - the air: thrust falling in a climb and rising in a descent and at speed,
//!   rotor drag and the nose lifting at speed, body drag that changes with
//!   attitude, and a whoop's duct drag and nose-up moment (E10, E11, E16–E18,
//!   E25, E26; [`air`]);
//! - the rotors' own spin: the frame turning against a rotor that speeds up or
//!   slows down, and their spin's gyroscopic push in a flip (E7, E8; step 6
//!   below);
//! - ground and ceiling effect: each rotor looks down and up at the Map, and
//!   gets a cushion near a floor and a pull under a ceiling, so a Quad half
//!   over a ledge tips away from it (E20, E21; [`ground_and_ceiling`]). They
//!   are the only flight effects that read the Map.
//!
//! The other effects arrive with their own tickets and Scenarios: Prop
//! Strikes and stalled motors' restarts (#45), Prop Wash (#46), and so on.
//!
//! # How one step moves the Quad
//!
//! [`QuadBody::step`] moves the Quad on by one fixed step of `dt` seconds:
//!
//! 1. Each motor's ESC reads its command and says how it drives its motor
//!    ([`esc`]).
//! 2. Each motor's speed moves on, from the battery's voltage after the last
//!    step ([`motor`]).
//! 3. The battery gives the power the ESCs drew: its voltage sags and its
//!    charge goes down ([`battery`]).
//! 4. The forces on the Quad give its acceleration: the Map's gravity, each
//!    rotor's thrust along the body's up axis in the air the rotor moves
//!    through, the air's drag ([`air`]), and the gain a surface close to a
//!    rotor gives its thrust, found by looking down and up from each rotor
//!    at the Map ([`ground_and_ceiling`]). They are worked out from where the
//!    step started, with the motors' new speeds.
//! 5. Semi-implicit Euler: the speed changes first, then the position moves
//!    with the new speed.
//! 6. The rotation changes by Euler's equation for a rigid body carrying
//!    spinning rotors, `J·dω/dt = M − dh/dt − ω × (J·ω + h)`, with `J` the
//!    inertia, `ω` the rotation in body axes, `h` the rotors' own spin (each
//!    rotor's inertia times its speed, along the body's up axis, one way or
//!    the other as it turns) and `M` the torques: each rotor's thrust and
//!    drag about the centre of mass, and each prop's drag torque, which twists
//!    the frame against the prop's spin.
//!    - `dh/dt`: a motor speeding its rotor up turns the frame the other way,
//!      as hard as it speeds it up, and slowing it down turns the frame with
//!      it (E7): the yaw twitch of a punch. It counts only the motors' own
//!      step (2), never a change the collision stage makes to a rotor's
//!      speed: a prop slowed by what it touches pushes on that, not on the
//!      frame.
//!    - `ω × h`: the rotors' spin, turned by a flip, pushes the frame at right
//!      angles to the flip (E8). With the four at the same speed their spins
//!      cancel; it shows only while one pair turns faster.
//!    - `ω × J·ω` is the gyroscopic coupling between the frame's own axes.
//! 7. The attitude turns by the new rotation over the step, with the
//!    exponential map, which is exact for a steady rotation and never drifts
//!    (flight-dynamics research §2.3).
//! 8. The collision stage (below): if that move would touch the Map, the Map
//!    pushes back, friction holds or drags, the Quad bounces and the move is
//!    redone, so no part ever passes through a surface between steps. It
//!    starts from the speed and rotation steps 5 and 6 gave, so the motors'
//!    thrust and torques are already in them.
//!
//! On the thrust stand, steps 4 to 8 are skipped: the Quad doesn't move.
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
//!    as the free move does it (steps 5 and 7 above).
//! 6. **The guard.** parry3d sweeps the Quad's shape along that move (a
//!    time-of-impact shape cast). If it meets a surface before the move ends,
//!    the Quad stops there: whatever step 2 missed, no part ever crosses a
//!    surface between steps.
//! 7. **Out of the Map.** A part more than [`SINK_ALLOWANCE`] inside a
//!    surface, as when a Scenario starts a Quad sunk into the floor, is moved
//!    straight out, without changing its speed.
//!
//! The collision stage remembers nothing from one step to the next, so it
//! adds nothing to the Simulation's state. Each step's [`Contact`]s say where
//! the Map pushed.
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

pub mod air;
mod collide;
mod geometry;
pub mod ground_and_ceiling;
mod map;
mod shape;

pub mod battery;
mod commands;
pub mod esc;
pub mod motor;
mod rotors;

use opendrone_maths::{Attitude, Fingerprinter, Mat3, Vec3};

use battery::Battery;
use esc::Esc;
use motor::{Model, Motor};

pub use battery::{BatteryOutput, BatteryParameters};
pub use collide::{
    BOUNCE_PASSES, BOUNCE_SPEED, Contact, PASSES, REST_SPEED, REST_TURN, SINK_ALLOWANCE, SKIN,
};
pub use commands::{MotorCommand, MotorCommands, SpinDirection};
pub use esc::{EscParameters, EscState, StartUpStep};
pub use map::{LineCrossing, MapCollision, MapShape, MapShapeError, MapShapeProblem};
pub use motor::{MotorParameters, PropParameters};
pub use rotors::{PropDirection, RotorLayout};
pub use shape::{DUCT_RING_SEGMENTS, DuctRings, PROP_DISC_THICKNESS, QuadPart, QuadShape};

use collide::{Collider, collide};
use shape::Part;

/// The Map's world values: what the air and the planet are like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    /// How hard gravity pulls straight down, in m/s².
    pub gravity: f64,
    /// The air's density, in kg/m³: the props' thrust and drag grow with it.
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
#[derive(Clone, Debug, PartialEq)]
pub struct QuadParameters {
    /// All-up mass, in kg: the dry mass plus the battery.
    pub mass: f64,
    /// Inertia in body axes (forward, left, up), in kg·m².
    pub inertia: Mat3,
    pub drag: Drag,
    pub rotors: RotorLayout,
    pub props: PropParameters,
    pub motors: MotorParameters,
    pub esc: EscParameters,
    pub battery: BatteryParameters,
    /// The Quad's collision shape, and how it bounces and slides.
    pub shape: QuadShape,
    /// How strong ground and ceiling effect are.
    pub ground_and_ceiling: GroundAndCeiling,
}

/// The Quad's drag numbers ([`air`] says how each acts). Scenarios that must
/// stay free of drag, such as free fall, use a Test Quad with all of these
/// set to zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Body drag area (drag coefficient × area) facing forward, sideways and
    /// up, in m².
    pub body_area: Vec3,
    /// Rotor drag, in s⁻¹: per kilogram of the Quad, per m/s of air across
    /// the props, with the rotors at the speed that hovers the Quad.
    pub rotor: f64,
    /// Whoop duct ram drag, in s⁻¹, counted the same way; zero for a Quad
    /// without ducts.
    pub duct_ram: f64,
    /// How far above the props' plane the ducts' ram drag acts, in metres: a
    /// ducted rotor's centre of pressure sits that much higher than an open
    /// rotor's. Zero for a Quad without ducts.
    pub duct_offset: f64,
}

impl Drag {
    /// No drag of any kind.
    pub const NONE: Drag = Drag {
        body_area: Vec3::ZERO,
        rotor: 0.0,
        duct_ram: 0.0,
        duct_offset: 0.0,
    };
}

/// The Quad definition's numbers for ground and ceiling effect
/// ([`ground_and_ceiling`] says how each acts). The rest of both effects
/// comes from the rotors' size and layout alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundAndCeiling {
    /// Sanchez-Cuevas et al.'s body-lift coefficient `K_b`: how much the air
    /// thrown back up under the body adds to the ground effect. 0 or more;
    /// close to 2 in their tests.
    pub ground_effect_body: f64,
    /// Hsiao and Chirarattananon's `α₀`: how unevenly the air flows round a
    /// rotor under a ceiling. 1 is their plain model; more pulls harder, and
    /// 0 gives no ceiling effect.
    pub ceiling_effect_asymmetry: f64,
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

/// How the Quad is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    /// Free to fly.
    Free,
    /// Held still on a thrust stand: its motors, ESCs and battery work as in
    /// flight, but nothing moves it.
    ThrustStand,
}

/// How the motors and their ESCs start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartingMotors {
    /// Stopped, with the ESCs just powered: they play their start-up tones and
    /// answer only after the ready beep, about 1.7 s later. This is how Reset
    /// powers a Quad up.
    PoweringUp,
    /// Stopped, with the ESCs already powered up and ready, so each starts its
    /// motor on its first command above zero, after the start wait.
    Stopped,
    /// Spinning at the speed that holds the Quad's height at the stated
    /// motion, with the ESCs running (ADR-0002): the same speed on every
    /// motor, whose thrust's upward part, with the air's push at that motion
    /// and the gain from any floor or ceiling near the rotors (on a Map,
    /// [`QuadBody::new_on_map`]), carries the weight. Tilted, that is more
    /// thrust than the weight, and
    /// the thrust's sideways part holds the speed only where the air's drag
    /// matches it. Never more than full drive gives: tilted further than that
    /// can hold, full drive. None upside down or past its side, where thrust
    /// would only push it down.
    Settled,
}

/// How a Quad starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadStart {
    pub state: QuadState,
    pub motors: StartingMotors,
    /// The battery's charge, as a share of its capacity (0 to 1).
    pub battery: f64,
    pub mount: Mount,
}

/// A set-up the physics can't move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetUpProblem {
    /// The mass must be above zero, and a real number.
    MassNotAboveZero,
    /// The inertia matrix has no inverse, so the Quad couldn't turn.
    InertiaHasNoInverse,
    /// A number the motors or the battery divide by must be above zero, and a
    /// real number: it names which.
    NotAboveZero(&'static str),
    /// The battery's voltage curve has no points.
    NoVoltageCurve,
    /// Every size in the collision shape must be above zero, its bounce from
    /// 0 to 1 and its friction 0 or more.
    ShapeCantBeBuilt,
    /// The ground effect's body term and the ceiling effect's asymmetry must
    /// be 0 or more, and the body term small enough for the rotors' layout
    /// that close to a floor the rotors still push air down.
    GroundOrCeilingEffectCantWork,
}

/// What one motor reports after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorOutput {
    /// In rad/s, positive the normal way.
    pub speed: f64,
    /// Along the body's up axis, in newtons, over the last step, in the air
    /// the rotor moved through: negative spinning backwards.
    pub thrust: f64,
    /// The air's drag torque on the prop, in N·m, with the speed's sign. The
    /// frame feels it the other way round the prop's axis.
    pub torque: f64,
    /// Through the motor, in amps.
    pub current: f64,
    /// What its ESC draws from the battery, in amps.
    pub supply_current: f64,
    /// The share of the battery's voltage its ESC puts across it.
    pub drive: f64,
    pub esc: EscState,
}

/// One Quad as a rigid body, with its motors, their ESCs and its battery.
#[derive(Clone, Debug)]
pub struct QuadBody {
    parameters: QuadParameters,
    inertia_inverse: Mat3,
    mount: Mount,
    state: QuadState,
    motors: [Motor; 4],
    escs: [Esc; 4],
    battery: Battery,
    positions: [Vec3; 4],
    turning: [f64; 4],
    /// Each rotor's thrust over the last step, in newtons.
    thrusts: [f64; 4],
    /// The air density the motors were last worked out in.
    air_density: f64,
    /// The collision shape's simple shapes, built once.
    parts: Vec<Part>,
    /// How far the shape's farthest point is from the centre of mass.
    reach: f64,
    /// Where the Map pushed the Quad during the last step.
    contacts: Vec<Contact>,
}

impl QuadBody {
    /// A Quad starting in open air, with no Map near it: as
    /// [`QuadBody::new_on_map`] with a Map that has no solid parts.
    pub fn new(
        parameters: QuadParameters,
        start: QuadStart,
        world: &World,
    ) -> Result<QuadBody, SetUpProblem> {
        QuadBody::new_on_map(parameters, start, world, &MapCollision::default())
    }

    /// A Quad starting on a Map. The Map's surfaces near its rotors at the
    /// start count, through ground and ceiling effect, for the speed
    /// "settled" motors spin at and the thrust it reports before its first
    /// step.
    pub fn new_on_map(
        parameters: QuadParameters,
        start: QuadStart,
        world: &World,
        map: &MapCollision,
    ) -> Result<QuadBody, SetUpProblem> {
        // Written so that "not a number" fails too.
        let above_zero = |value: f64| value > 0.0 && value.is_finite();
        if !above_zero(parameters.mass) {
            return Err(SetUpProblem::MassNotAboveZero);
        }
        let inertia_inverse = parameters
            .inertia
            .inverse()
            .ok_or(SetUpProblem::InertiaHasNoInverse)?;
        for (name, value) in [
            ("the motors' KV", parameters.motors.kv),
            (
                "the motors' winding resistance",
                parameters.motors.winding_resistance,
            ),
            ("the motors' spin-up time", parameters.motors.spin_up),
            ("the motors' slow-down time", parameters.motors.slow_down),
            ("the props' rotor inertia", parameters.props.rotor_inertia),
            ("the battery's capacity", parameters.battery.capacity),
            ("the battery's recovery time", parameters.battery.recovery),
        ] {
            if !above_zero(value) {
                return Err(SetUpProblem::NotAboveZero(name));
            }
        }
        if parameters.battery.voltage_curve.is_empty() {
            return Err(SetUpProblem::NoVoltageCurve);
        }
        if !parameters.shape.is_buildable() {
            return Err(SetUpProblem::ShapeCantBeBuilt);
        }
        let positions = parameters.rotors.positions();
        if !ground_and_ceiling::can_work(&rotors(&parameters, &positions)) {
            return Err(SetUpProblem::GroundOrCeilingEffectCantWork);
        }

        let battery = Battery::new(&parameters.battery, start.battery);
        let model = Model::new(&parameters.motors, &parameters.props, world.air_density);
        let turning = parameters.rotors.turning();
        // On the thrust stand the Quad is held in still air, away from the
        // Map; free, its rotors look at the Map from where it starts.
        let surfaces = |pushing: [bool; 4]| match start.mount {
            Mount::ThrustStand => [1.0; 4],
            Mount::Free => {
                let rotors = rotors(&parameters, &positions);
                let distances = ground_and_ceiling::look(map, &start.state, &rotors, pushing);
                ground_and_ceiling::gains(&distances, &rotors)
            }
        };
        let (motors, escs) = match start.motors {
            StartingMotors::PoweringUp => ([Motor::STOPPED; 4], [Esc::powering_up(); 4]),
            StartingMotors::Stopped => ([Motor::STOPPED; 4], [Esc::ready(); 4]),
            StartingMotors::Settled => {
                let most = model.steady_speed(1.0, battery.voltage(), SpinDirection::Normal);
                let speed = settled_speed(
                    &airframe(&parameters, &positions),
                    &model,
                    &start.state,
                    world,
                    most,
                    &surfaces([true; 4]),
                );
                let motor = Motor::steady(&model, speed, battery.voltage());
                ([motor; 4], [Esc::running(); 4])
            }
        };
        let speeds = motors.map(|motor| motor.speed);
        let thrusts = match start.mount {
            Mount::ThrustStand => speeds.map(|speed| model.thrust(speed)),
            Mount::Free => {
                let state = &start.state;
                let mut push = air::push(
                    &airframe(&parameters, &positions),
                    &model,
                    state.attitude.world_to_body(state.velocity),
                    state.rotation,
                    speeds,
                    world.air_density,
                    world.gravity,
                );
                let gains = surfaces(speeds.map(|speed| speed > 0.0));
                ground_and_ceiling::add_lift(&mut push, &gains, &positions);
                push.thrusts
            }
        };
        let parts = parameters.shape.parts();
        let reach = parts
            .iter()
            .map(|part| part.reach)
            .fold(0.0, opendrone_maths::functions::max);
        Ok(QuadBody {
            parameters,
            inertia_inverse,
            mount: start.mount,
            state: start.state,
            motors,
            escs,
            battery,
            positions,
            turning,
            thrusts,
            air_density: world.air_density,
            parts,
            reach,
            contacts: Vec::new(),
        })
    }

    pub fn state(&self) -> &QuadState {
        &self.state
    }

    pub fn parameters(&self) -> &QuadParameters {
        &self.parameters
    }

    pub fn mount(&self) -> Mount {
        self.mount
    }

    /// What each motor reports after the last step, in Betaflight's motor
    /// order.
    pub fn motors(&self) -> [MotorOutput; 4] {
        let model = self.model();
        let volts = self.battery.voltage();
        core::array::from_fn(|k| {
            let motor = &self.motors[k];
            MotorOutput {
                speed: motor.speed,
                thrust: self.thrusts[k],
                torque: model.drag_torque(motor.speed),
                current: motor.current,
                supply_current: if volts > 0.0 {
                    motor.power / volts
                } else {
                    0.0
                },
                drive: motor.drive,
                esc: self.escs[k].state(),
            }
        })
    }

    /// What the battery reports after the last step.
    pub fn battery(&self) -> BatteryOutput {
        self.battery.output(&self.parameters.battery)
    }

    /// True once every ESC has played its ready beep: none is still starting
    /// up after power-up, so each answers its commands.
    pub fn escs_ready(&self) -> bool {
        self.escs
            .iter()
            .all(|esc| !matches!(esc.state(), EscState::StartingUp(_)))
    }

    /// Every place where the Map pushed the Quad during the last step, in a
    /// fixed order: by part, then by Map shape. On the thrust stand, none.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    fn model(&self) -> Model {
        Model::new(
            &self.parameters.motors,
            &self.parameters.props,
            self.air_density,
        )
    }

    /// Moves the Quad on by one fixed step of `dt` seconds, with these motor
    /// commands, through the Map's world values and solid parts (see the
    /// crate's "How one step moves the Quad").
    pub fn step(&mut self, world: &World, map: &MapCollision, commands: &MotorCommands, dt: f64) {
        self.air_density = world.air_density;
        let model = self.model();

        // 1–3: the ESCs, the motors and the battery.
        let volts = self.battery.voltage();
        // The rotors' spin reaction (step 6) counts only what the motors
        // themselves do to their rotors in this step: from here, after the
        // last step's collision stage, to just after the motors' own step.
        let speeds_before = self.motors.map(|motor| motor.speed);
        let mut power = 0.0;
        for ((motor, esc), command) in self.motors.iter_mut().zip(&mut self.escs).zip(commands.0) {
            let drive = esc.step(
                command,
                motor.speed,
                &self.parameters.esc,
                self.parameters.motors.poles,
                dt,
            );
            motor.step(&model, drive, volts, dt);
            power += motor.power;
        }
        self.battery.step(&self.parameters.battery, power, dt);

        if self.mount == Mount::ThrustStand {
            // Held still, in still air: each rotor gives its thrust-stand
            // thrust.
            self.thrusts = self.motors.map(|motor| model.thrust(motor.speed));
            return;
        }

        // 4–7: the free move, with the rotors' thrust and torques and the
        // air. 8: the collision stage starts from the speeds the free move
        // gave, so their push is in them, and redoes the move from where the
        // step started.
        let before = self.state;
        // Each rotor that pushes air looks down and up at the Map from where
        // the step starts (step 4).
        let rotors = rotors(&self.parameters, &self.positions);
        let pushing = self.motors.map(|motor| motor.speed > 0.0);
        let distances = ground_and_ceiling::look(map, &self.state, &rotors, pushing);
        let gains = ground_and_ceiling::gains(&distances, &rotors);
        self.move_freely(world, &model, speeds_before, &gains, dt);
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

    /// Steps 4 to 7: the move as if nothing were in the way, under gravity,
    /// the rotors' thrust and torques, and the air, with each rotor's thrust
    /// gain from the surfaces near it. `speeds_before` are the motors' speeds
    /// before this step's.
    fn move_freely(
        &mut self,
        world: &World,
        model: &Model,
        speeds_before: [f64; 4],
        gains: &[f64; 4],
        dt: f64,
    ) {
        let speeds = self.motors.map(|motor| motor.speed);
        let state = &mut self.state;
        let mut push = air::push(
            &airframe(&self.parameters, &self.positions),
            model,
            state.attitude.world_to_body(state.velocity),
            state.rotation,
            speeds,
            world.air_density,
            world.gravity,
        );
        ground_and_ceiling::add_lift(&mut push, gains, &self.positions);
        self.thrusts = push.thrusts;

        // Each prop's drag torque, and the rotors' own spin (step 6).
        let rotor_inertia = self.parameters.props.rotor_inertia;
        let mut torque = push.torque;
        let mut spin = 0.0;
        let mut spin_change = 0.0;
        for k in 0..4 {
            let turning = self.turning[k];
            torque += Vec3::new(0.0, 0.0, -turning * model.drag_torque(speeds[k]));
            spin += turning * rotor_inertia * speeds[k];
            spin_change += turning * rotor_inertia * (speeds[k] - speeds_before[k]) / dt;
        }
        let spin = Vec3::new(0.0, 0.0, spin);

        let gravity = Vec3::new(0.0, 0.0, -world.gravity);
        let pushed = state
            .attitude
            .body_to_world(push.force / self.parameters.mass);
        let acceleration = gravity + pushed;
        state.velocity += acceleration * dt;
        state.position += state.velocity * dt;

        let inertia = self.parameters.inertia;
        let w = state.rotation;
        let angular_acceleration = self.inertia_inverse
            * (torque - Vec3::new(0.0, 0.0, spin_change) - w.cross(inertia * w + spin));
        state.rotation += angular_acceleration * dt;
        state.attitude = state.attitude.turned_by(state.rotation * dt);
    }

    /// True when every number in its state, its motors and its battery is a
    /// real number.
    pub fn is_finite(&self) -> bool {
        let battery = self.battery();
        self.state.is_finite()
            && self.motors().iter().all(|m| {
                [
                    m.speed,
                    m.thrust,
                    m.torque,
                    m.current,
                    m.supply_current,
                    m.drive,
                ]
                .iter()
                .all(|v| v.is_finite())
            })
            && [
                battery.voltage,
                battery.current,
                battery.charge_used,
                battery.sag,
            ]
            .iter()
            .all(|v| v.is_finite())
    }

    /// Feeds the Quad's whole state into a fingerprint: where it is and how
    /// it moves, each motor and ESC, and the battery.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.state.write_fingerprint(f);
        for (motor, esc) in self.motors.iter().zip(&self.escs) {
            f.write_f64s(&[motor.speed, motor.drive, motor.current, motor.power]);
            esc.write_fingerprint(f);
        }
        self.battery.write_fingerprint(f);
    }
}

/// What ground and ceiling effect need to know about a Quad.
fn rotors<'a>(
    parameters: &'a QuadParameters,
    positions: &'a [Vec3; 4],
) -> ground_and_ceiling::Rotors<'a> {
    ground_and_ceiling::Rotors {
        positions,
        radius: parameters.props.diameter / 2.0,
        diagonal: parameters.rotors.diagonal,
        strengths: &parameters.ground_and_ceiling,
    }
}

/// What the air needs to know about a Quad.
fn airframe<'a>(parameters: &'a QuadParameters, positions: &'a [Vec3; 4]) -> air::Airframe<'a> {
    let diameter = parameters.props.diameter;
    air::Airframe {
        mass: parameters.mass,
        drag: &parameters.drag,
        positions,
        disc_area: core::f64::consts::PI * diameter * diameter / 4.0,
    }
}

/// The speed "settled" motors spin at (see [`StartingMotors::Settled`]): the
/// same on all four, whose thrust's upward part, with the air's push at the
/// starting motion and the `gains` of the surfaces near the rotors, carries
/// the weight; at most `most`, the speed full drive holds; none upside down
/// or past its side.
fn settled_speed(
    airframe: &air::Airframe,
    model: &Model,
    state: &QuadState,
    world: &World,
    most: f64,
    gains: &[f64; 4],
) -> f64 {
    if state.attitude.body_to_world(Vec3::new(0.0, 0.0, 1.0)).z <= 0.0 {
        return 0.0;
    }
    let velocity = state.attitude.world_to_body(state.velocity);
    // How much more than the weight the thrust and the air hold up, in
    // newtons, with every rotor at `speed`.
    let held_up = |speed: f64| {
        let mut push = air::push(
            airframe,
            model,
            velocity,
            state.rotation,
            [speed; 4],
            world.air_density,
            world.gravity,
        );
        ground_and_ceiling::add_lift(&mut push, gains, airframe.positions);
        state.attitude.body_to_world(push.force).z - airframe.mass * world.gravity
    };
    // With the motors stopped, the air alone (falling fast) may hold it up.
    let stopped = held_up(0.0);
    if stopped.is_nan() || stopped >= 0.0 {
        return 0.0;
    }
    let flat_out = held_up(most);
    if flat_out.is_nan() || flat_out <= 0.0 {
        return most;
    }
    // Halve the gap between a speed too slow and one too fast until no
    // number lies between them, then take the closer of the two.
    let (mut slow, mut fast) = (0.0, most);
    loop {
        let middle = slow + (fast - slow) / 2.0;
        if middle <= slow || middle >= fast {
            break;
        }
        if held_up(middle) < 0.0 {
            slow = middle;
        } else {
            fast = middle;
        }
    }
    if -held_up(slow) <= held_up(fast) {
        slow
    } else {
        fast
    }
}
