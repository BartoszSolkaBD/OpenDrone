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
//! - Flight Inputs, the one way in: [`Simulation::flight_input`] takes them,
//!   stamped with Simulation Time ([`FlightInput`]): Channels, the Flying
//!   Input Device lost or back, and Reset.
//! - The Radio Link ([`RadioLink`]): each Quad's Channels reach its Flight
//!   Controller in frames at the pilot's [`PacketRate`], like an ELRS link on
//!   CRSF (ADR-0007). When the Packet Rate divides the Flying Input Device's
//!   Report Rate, the frames follow the device's report beat, learnt from
//!   the Flight Inputs' stamps (ADR-0020); otherwise, and for scripted
//!   sticks, they come on the link's own clock. The device's facts come with
//!   the set-up ([`QuadSetUp::input_device`]). While the device is lost
//!   (unplugged, or, if it reports at rest, silent for 1 s), it sends no
//!   frames, and the Flight Controller's Failsafe follows.
//! - Input smoothing, the Assist that smooths roll, pitch and yaw with a
//!   gentle 15 Hz low-pass before the Radio Link ([`InputSmoothing`]), set
//!   with [`QuadSetUp::input_smoothing`].
//! - Reset: the Quad back on its Launch Spot ([`QuadSetUp::launch_spot`]),
//!   landed and disarmed, powered up fresh as a new battery does: a full
//!   charge, a fresh Flight Controller, and ESCs playing their start-up
//!   tones, so arming waits for their ready beep, about 1.7 s. The Radio Link
//!   belongs to the pilot's radio, so it keeps its beat.
//! - Auto-arm, the sim-only Assist that arms on the first throttle raise
//!   ([`auto_arm`]), set with [`OurFlightController::new`].
//! - [`FlightControllerSeam`]: what turns sensor readings and Radio Link
//!   frames into the four motor commands each tick, one Flight Controller
//!   loop per physics step. Our Flight Controller plugs in as
//!   [`OurFlightController`], the [`ScriptedMotors`] stand-in for Physics and
//!   Thrust Stand Scenarios, and later perhaps a SITL bridge in its own
//!   crate.
//! - The thrust-stand set-up: a Quad set up with [`Mount::ThrustStand`] is
//!   held still while its motors, ESCs and battery work as in flight.
//! - [`Simulation::quad_output`]: each tick's output per Quad: where it is and
//!   how it moves, each motor's speed, thrust, torque and current, each ESC's
//!   state, the battery's voltage, current and charge, the motor commands,
//!   and what our Flight Controller's loop did.
//! - [`Simulation::fingerprint`]: a fingerprint of the whole state, the same on
//!   every computer, for the repeat and agreement checks.
//! - After each tick, every Quad's state and its contacts with the Map
//!   ([`Simulation::contacts`]).
//! - The read-only line question ([`Simulation::line_question`]): which Map
//!   surfaces a straight line passes through, and where it goes in and comes
//!   out, for the Video Signal and the Where-you-stand sound. It never
//!   changes the Simulation.
//!
//! Each tick, for every Quad in order: the Flight Inputs stamped with this
//! moment arrive (Reset powering the Quad up there and then); the Radio Link
//! sends a frame if one is due, its sticks as Input smoothing has them; Input
//! smoothing moves on by a step; the Flight Controller seam reads the sensors
//! (the state at the tick's start, and whether the ESCs have beeped ready)
//! and the frame and gives the motor commands; the physics moves the Quad on
//! by one step with them.
//!
//! A crash never disarms or resets the Quad on its own: only the pilot's
//! Arm switch, Failsafe or Reset do.
//!
//! Endless Battery (#57) and the rest of the session state arrive with their
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

pub mod auto_arm;
mod flight_controller;
mod input_smoothing;
mod motors;
mod radio_link;
mod time;

use opendrone_maths::{Fingerprint, Fingerprinter, Vec3};
use opendrone_physics::{MapCollision, QuadBody, QuadStart};

pub use flight_controller::{OurFlightController, sensor_readings};
pub use input_smoothing::{INPUT_SMOOTHING_HZ, InputSmoothing};
pub use motors::{FlightControllerSeam, ScriptedMotors};
pub use opendrone_flight_controller::{
    ArmingBlocks, Channel, Channels, DebugRecord, FailsafePhase, FailsafeReadings, Rates,
    SensorReadings, Terms, Tune,
};
pub use opendrone_physics::{
    BatteryOutput, BatteryParameters, Drag, EscParameters, EscState, MotorCommand, MotorCommands,
    MotorOutput, MotorParameters, Mount, PropDirection, PropParameters, QuadParameters, QuadState,
    RotorLayout, SetUpProblem, SpinDirection, StartUpStep, StartingMotors, World,
};
pub use opendrone_physics::{
    Contact, DuctRings, LineCrossing, MapShape, MapShapeProblem, QuadPart, QuadShape,
};
pub use radio_link::{
    FlightInput, Frame, InputDeviceFacts, LOCK_MARGIN_MICROS, PacketRate, RadioLink, ReportRate,
    SILENT_FOR_SECONDS,
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
    /// Where Reset puts this Quad: resting on the Map's Launch Spot, still.
    pub launch_spot: QuadState,
    /// How its motors and their ESCs start.
    pub motors: StartingMotors,
    /// The battery's charge, as a share of its capacity (0 to 1).
    pub battery: f64,
    /// Free to fly, or held on the thrust stand.
    pub mount: Mount,
    /// The pilot's Packet Rate: how often the Radio Link carries this Quad's
    /// Channels to its Flight Controller.
    pub packet_rate: PacketRate,
    /// The Flying Input Device's facts, from its Input Device profile: its
    /// Report Rate and whether it reports at rest. `None` for scripted
    /// sticks, such as a Scenario's Timeline, which get plain regular frames.
    pub input_device: Option<InputDeviceFacts>,
    /// Input smoothing, the Assist, on or off.
    pub input_smoothing: bool,
    /// What plugs into the Flight Controller seam for this Quad.
    pub flight_controller: Box<dyn FlightControllerSeam>,
}

/// One Quad's output after a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadOutput {
    /// Where it is and how it moves.
    pub state: QuadState,
    /// Each motor, in Betaflight's motor order, with its ESC's state.
    pub motors: [MotorOutput; 4],
    pub battery: BatteryOutput,
    /// The motor commands the Flight Controller seam gave on the tick (all
    /// stopped before the first).
    pub commands: MotorCommands,
    /// What our Flight Controller's loop did on the tick; `None` before the
    /// first and for anything else in the seam.
    pub flight_controller: Option<DebugRecord>,
    /// What the Radio Link did on the tick.
    pub radio_link: RadioLinkOutput,
}

/// What a Radio Link did on a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioLinkOutput {
    /// The frame that left on the tick, if one did, with its Channels as the
    /// Flight Controller got them (Input smoothing's, if on).
    pub frame: Option<Frame>,
    /// The Channels of the last frame to leave, on this tick or before;
    /// `None` before the first.
    pub last_frame: Option<Channels>,
    /// Whether the Flying Input Device counts as lost: unplugged, or silent
    /// for too long.
    pub lost: bool,
    /// Whether the frames follow the device's report beat (ADR-0020).
    pub locked: bool,
}

impl RadioLinkOutput {
    /// The output after a tick on which `frame` left, or none did.
    pub fn after(&self, frame: Option<Frame>, link: &RadioLink) -> RadioLinkOutput {
        RadioLinkOutput {
            frame,
            last_frame: frame.map(|f| f.channels).or(self.last_frame),
            lost: link.is_lost(),
            locked: link.is_locked(),
        }
    }
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
    /// Where Reset puts it.
    launch_spot: QuadState,
    flight_controller: Box<dyn FlightControllerSeam>,
    motor_commands: MotorCommands,
    radio_link: RadioLink,
    /// Input smoothing, when the pilot has it on.
    input_smoothing: Option<InputSmoothing>,
    /// What the Radio Link did on the last tick.
    radio_link_output: RadioLinkOutput,
    /// Flight Inputs not yet arrived, in the order of their moments.
    inputs: Vec<(SimulationTime, FlightInput)>,
    /// Whether the seam has run a loop yet.
    stepped: bool,
}

impl SimulatedQuad {
    /// Reset: the Quad on its Launch Spot, still, with a full battery and its
    /// ESCs just powered, and its Flight Controller powered up fresh. Its
    /// Radio Link and the Flight Inputs still to arrive are the pilot's, so
    /// they go on.
    fn reset(&mut self, world: &World) {
        let start = QuadStart {
            state: self.launch_spot,
            motors: StartingMotors::PoweringUp,
            battery: 1.0,
            mount: self.body.mount(),
        };
        // The same parameters were accepted at set-up, so they are accepted
        // again; if they weren't, the Quad would stay as it was.
        if let Ok(body) = QuadBody::new(self.body.parameters().clone(), start, world) {
            self.body = body;
        }
        self.motor_commands = MotorCommands::STOPPED;
        let readings = sensor_readings(self.body.state(), self.body.escs_ready());
        self.flight_controller.power_up(&readings);
    }
}

impl Simulation {
    pub fn new(set_up: SetUp) -> Result<Simulation, SetUpError> {
        let map = MapCollision::new(&set_up.map).map_err(|error| SetUpError::MapShape {
            shape: error.shape,
            problem: error.problem,
        })?;
        let mut quads = Vec::with_capacity(set_up.quads.len());
        for (index, quad) in set_up.quads.into_iter().enumerate() {
            let start = QuadStart {
                state: quad.start,
                motors: quad.motors,
                battery: quad.battery,
                mount: quad.mount,
            };
            let body = QuadBody::new(quad.parameters, start, &set_up.world).map_err(|problem| {
                SetUpError::Quad {
                    quad: index,
                    problem,
                }
            })?;
            quads.push(SimulatedQuad {
                body,
                launch_spot: quad.launch_spot,
                flight_controller: quad.flight_controller,
                motor_commands: MotorCommands::STOPPED,
                radio_link: RadioLink::new(
                    quad.packet_rate,
                    set_up.physics_rate,
                    quad.input_device,
                ),
                input_smoothing: quad
                    .input_smoothing
                    .then(|| InputSmoothing::new(set_up.physics_rate)),
                radio_link_output: RadioLinkOutput::default(),
                inputs: Vec::new(),
                stepped: false,
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

    /// The one way in: a Flight Input for a Quad (counting from 0 in the
    /// set-up's order), stamped with the Simulation Time it arrives at. It
    /// takes effect at the start of the tick at that moment, or of the next
    /// tick if that moment has passed. Inputs stamped with the same moment
    /// arrive in the order they were given.
    pub fn flight_input(&mut self, quad: usize, at: SimulationTime, input: FlightInput) {
        let inputs = &mut self.quads[quad].inputs;
        let place = inputs.partition_point(|(time, _)| *time <= at);
        inputs.insert(place, (at, input));
    }

    /// One tick: for every Quad in order, the Flight Inputs stamped up to now
    /// arrive, the Radio Link sends a frame if one is due (its sticks as Input
    /// smoothing has them, if on), Input smoothing moves on by a step, the
    /// Flight Controller seam reads the sensors and the frame and gives the
    /// motor commands, then the physics moves the Quad on by one step with
    /// them, colliding with the Map. Then Simulation Time moves on by one
    /// step.
    pub fn step(&mut self) {
        let dt = self.physics_rate.step_length();
        for quad in &mut self.quads {
            let arrived = quad.inputs.partition_point(|(time, _)| *time <= self.time);
            let inputs: Vec<(SimulationTime, FlightInput)> = quad.inputs.drain(..arrived).collect();
            for (at, input) in inputs {
                match input {
                    FlightInput::Channels(channels) => {
                        quad.radio_link.hear(at, channels);
                        if let Some(smoothing) = &mut quad.input_smoothing {
                            smoothing.hear(&channels);
                        }
                    }
                    FlightInput::InputDeviceLost => quad.radio_link.lose(at),
                    FlightInput::InputDeviceBack => quad.radio_link.back(at),
                    FlightInput::Reset => quad.reset(&self.world),
                }
            }
            let mut frame = quad.radio_link.frame(self.time);
            if let Some(smoothing) = &mut quad.input_smoothing {
                if let Some(frame) = &mut frame {
                    frame.channels = smoothing.smooth(frame.channels);
                }
                smoothing.step();
            }
            quad.radio_link_output = quad.radio_link_output.after(frame, &quad.radio_link);
            let channels = frame.map(|frame| frame.channels);
            let readings = sensor_readings(quad.body.state(), quad.body.escs_ready());
            quad.motor_commands =
                quad.flight_controller
                    .step(self.time, &readings, channels.as_ref());
            quad.stepped = true;
            quad.body
                .step(&self.world, &self.map, &quad.motor_commands, dt);
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

    /// A Quad's output after the last tick (at the start before the first):
    /// where it is and how it moves, each motor and its ESC, and the battery.
    pub fn quad_output(&self, quad: usize) -> QuadOutput {
        let quad = &self.quads[quad];
        let body = &quad.body;
        QuadOutput {
            state: *body.state(),
            motors: body.motors(),
            battery: body.battery(),
            commands: quad.motor_commands,
            flight_controller: if quad.stepped {
                quad.flight_controller.debug()
            } else {
                None
            },
            radio_link: quad.radio_link_output,
        }
    }

    /// Every place where the Map pushed a Quad during the last tick, by part
    /// and then by Map shape (none before the first tick, and none on the
    /// thrust stand).
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
            .all(|quad| quad.body.is_finite() && quad.motor_commands.is_finite())
    }

    /// A fingerprint of the whole state, in a fixed order: Simulation Time,
    /// the physics rate, the world, the random seed, then every Quad's state
    /// (with its motors, ESCs and battery), motor commands, Radio Link, Input
    /// smoothing and Flight Controller seam. Two runs, or two computers,
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
            quad.radio_link.write_fingerprint(&mut f);
            match &quad.input_smoothing {
                None => f.write_u64(0),
                Some(smoothing) => {
                    f.write_u64(1);
                    smoothing.write_fingerprint(&mut f);
                }
            }
            quad.flight_controller.write_fingerprint(&mut f);
        }
        f.finish()
    }
}
