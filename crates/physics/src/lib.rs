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
//! - the thrust stand: a Quad held still, its motors, ESCs and battery working
//!   as in flight ([`Mount::ThrustStand`]).
//!
//! The other effects arrive with their own tickets and Scenarios: drag, the
//! rest of the air and the rotors' own spin effects (#42), collisions (#43),
//! Prop Strikes and stalled motors' restarts (#45), and so on.
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
//! 4. The forces on the Quad give its acceleration: the Map's gravity and each
//!    rotor's thrust, along the body's up axis.
//! 5. Semi-implicit Euler: the speed changes first, then the position moves
//!    with the new speed.
//! 6. The rotation changes by Euler's equation for a rigid body,
//!    `J·dω/dt = M − ω × J·ω`, with `J` the inertia, `ω` the rotation in body
//!    axes and `M` the torques: each rotor's thrust about the centre of mass,
//!    and each prop's drag torque, which twists the frame against the prop's
//!    spin. The `ω × J·ω` part is the gyroscopic coupling between the axes.
//! 7. The attitude turns by the new rotation over the step, with the
//!    exponential map, which is exact for a steady rotation and never drifts
//!    (flight-dynamics research §2.3).
//!
//! On the thrust stand, steps 4 to 7 are skipped: the Quad doesn't move.
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
pub use commands::{MotorCommand, MotorCommands, SpinDirection};
pub use esc::{EscParameters, EscState, StartUpStep};
pub use motor::{MotorParameters, PropParameters};
pub use rotors::{PropDirection, RotorLayout};

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
    /// Spinning at the speed whose thrust carries the weight along the
    /// motors' axis, with the ESCs running (ADR-0002): the same speed on
    /// every motor, `m·g·cos(tilt) / 4` of thrust each, and none upside down.
    /// Level, that holds the stated motion; tilted, nothing but drag could,
    /// and drag arrives with the air ticket (#42).
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
}

/// What one motor reports after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorOutput {
    /// In rad/s, positive the normal way.
    pub speed: f64,
    /// Along the body's up axis, in newtons: negative spinning backwards.
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
    /// The air density the motors were last worked out in.
    air_density: f64,
}

impl QuadBody {
    pub fn new(
        parameters: QuadParameters,
        start: QuadStart,
        world: &World,
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

        let battery = Battery::new(&parameters.battery, start.battery);
        let model = Model::new(&parameters.motors, &parameters.props, world.air_density);
        let (motors, escs) = match start.motors {
            StartingMotors::PoweringUp => ([Motor::STOPPED; 4], [Esc::powering_up(); 4]),
            StartingMotors::Stopped => ([Motor::STOPPED; 4], [Esc::ready(); 4]),
            StartingMotors::Settled => {
                let up = start.state.attitude.body_to_world(Vec3::new(0.0, 0.0, 1.0));
                let along_axis = opendrone_maths::functions::max(up.z, 0.0);
                let thrust = parameters.mass * world.gravity * along_axis / 4.0;
                let speed = model.settled_speed(thrust);
                let motor = Motor::steady(&model, speed, battery.voltage());
                ([motor; 4], [Esc::running(); 4])
            }
        };
        let positions = parameters.rotors.positions();
        let turning = parameters.rotors.turning();
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
            air_density: world.air_density,
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
                thrust: model.thrust(motor.speed),
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

    fn model(&self) -> Model {
        Model::new(
            &self.parameters.motors,
            &self.parameters.props,
            self.air_density,
        )
    }

    /// Moves the Quad on by one fixed step of `dt` seconds, with these motor
    /// commands (see the crate's "How one step moves the Quad").
    pub fn step(&mut self, world: &World, commands: &MotorCommands, dt: f64) {
        self.air_density = world.air_density;
        let model = self.model();

        // 1–3: the ESCs, the motors and the battery.
        let volts = self.battery.voltage();
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
            return;
        }

        // 4–7: the rigid body.
        let state = &mut self.state;
        let mut thrust = 0.0;
        let mut torque = Vec3::ZERO;
        for k in 0..4 {
            let speed = self.motors[k].speed;
            let force = model.thrust(speed);
            thrust += force;
            torque += self.positions[k].cross(Vec3::new(0.0, 0.0, force));
            torque += Vec3::new(0.0, 0.0, -self.turning[k] * model.drag_torque(speed));
        }

        let gravity = Vec3::new(0.0, 0.0, -world.gravity);
        let lift = state
            .attitude
            .body_to_world(Vec3::new(0.0, 0.0, thrust / self.parameters.mass));
        let acceleration = gravity + lift;
        state.velocity += acceleration * dt;
        state.position += state.velocity * dt;

        let inertia = self.parameters.inertia;
        let w = state.rotation;
        let angular_acceleration = self.inertia_inverse * (torque - w.cross(inertia * w));
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
