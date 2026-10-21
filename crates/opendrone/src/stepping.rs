//! Stepping the Simulation inside Bevy's fixed timestep (#2 §1.2).
//!
//! Bevy's fixed timestep ticks at 1 kHz. Each tick runs 8 Simulation steps,
//! all inside one system, so nothing else runs between them and their order
//! never changes. Before each frame Bevy runs as many ticks as the time that
//! passed calls for, one after another (catch-up stepping): a step is never
//! skipped or stretched, whatever the frame rate. When a frame takes longer
//! than a quarter of a second, Bevy lets the extra time go, so on overload
//! the Simulation slows down instead. Pause stops Bevy's clock, so no tick
//! runs.
//!
//! The picture is drawn between the last two ticks' states, by how far the
//! game's clock is into the next tick ([`Flight::drawn_between`]).

use bevy::prelude::*;
use opendrone_maths::{Fingerprint, Fingerprinter};
use opendrone_scenario::ScenarioInputs;
use opendrone_sim::{FlightInput, InputDeviceFacts, QuadState, Simulation, SimulationTime};

/// Bevy's fixed timestep: 1,000 ticks a second.
pub const TICK_HZ: u32 = 1000;

/// The most Flight Inputs Save Flight's recorder keeps: a million, about
/// 24 MB. That is the first 16 minutes of a flight on a 1 kHz Input Device,
/// or over an hour on a 250 Hz one. A saved flight has to start where the
/// Simulation did (Reset and arming don't start it afresh), so the recorder
/// can't drop the old entries and keep the new: past this, it lets go of
/// everything it held, keeps nothing more, and Save Flight says the flight is
/// too long to save.
pub const MAX_KEPT_INPUTS: usize = 1_000_000;

/// The flight: the Simulation and what steps it.
///
/// It lives on the main thread (a non-send resource): the Simulation holds its
/// Flight Controller seam as a plain trait object.
pub struct Flight {
    simulation: Simulation,
    steps_per_tick: u64,
    /// A Scenario's Flight Inputs, for the reference check; the pilot's
    /// arrive through [`Flight::input`] instead.
    script: Option<ScenarioInputs>,
    /// The fingerprint at the start and after every step, when asked for.
    fingerprints: Option<Vec<Fingerprint>>,
    /// What Save Flight writes out: the Flight Inputs the Simulation took,
    /// and the run's fingerprint so far. Only a flight the pilot flies keeps
    /// it.
    kept: Option<Kept>,
    /// The Quad's state after the tick before the last, and after the last.
    before: QuadState,
    after: QuadState,
    /// When each Reset that hasn't arrived yet will: the Simulation Time
    /// it's stamped with. It can be a frame ahead of the Simulation, so the
    /// Reset arrives in a later tick than the one after it is handed in.
    resets_due: Vec<SimulationTime>,
    /// The last tick moved the Quad at once (Reset): the picture shows no
    /// motion across it.
    jumped: bool,
}

impl Flight {
    /// A flight of `simulation`, stepped `physics rate ÷ 1000` steps a tick:
    /// 8 at the alpha's 8 kHz. The physics rate must be a whole number of
    /// kHz.
    pub fn new(simulation: Simulation) -> Result<Flight, String> {
        let hz = simulation.physics_rate().hz();
        if !hz.is_multiple_of(TICK_HZ) {
            return Err(format!(
                "the physics rate, {hz} Hz, isn't a whole number of steps for each of Bevy's {TICK_HZ} ticks a second"
            ));
        }
        let state = *simulation.quad_state(0);
        Ok(Flight {
            steps_per_tick: u64::from(hz / TICK_HZ),
            simulation,
            script: None,
            fingerprints: None,
            kept: None,
            before: state,
            after: state,
            resets_due: Vec::new(),
            jumped: false,
        })
    }

    /// A Scenario's flight: its Simulation, with its Flight Inputs handed in
    /// at each step, until its last moment, keeping the fingerprint after
    /// every step.
    pub fn scripted(simulation: Simulation, script: ScenarioInputs) -> Result<Flight, String> {
        let mut flight = Flight::new(simulation)?;
        flight.fingerprints = Some(vec![flight.simulation.fingerprint()]);
        flight.script = Some(script);
        Ok(flight)
    }

    /// A flight the pilot flies with the Input Device whose facts are given,
    /// keeping what Save Flight needs to write it out as a Scenario: every
    /// Flight Input it takes, up to [`MAX_KEPT_INPUTS`]. That costs next to
    /// nothing.
    pub fn keeping(simulation: Simulation, device: InputDeviceFacts) -> Result<Flight, String> {
        Flight::keeping_at_most(simulation, device, MAX_KEPT_INPUTS)
    }

    /// As [`Flight::keeping`], keeping at most `most` Flight Inputs rather
    /// than [`MAX_KEPT_INPUTS`].
    pub fn keeping_at_most(
        simulation: Simulation,
        device: InputDeviceFacts,
        most: usize,
    ) -> Result<Flight, String> {
        let mut flight = Flight::new(simulation)?;
        flight.kept = Some(Kept {
            device,
            inputs: Vec::new(),
            most,
            outgrown_at: None,
            run: None,
        });
        Ok(flight)
    }

    /// Also keeps the fingerprint of the whole run, from the start and after
    /// every step, as the Scenario runner works it out. Checks use it to show
    /// a saved flight gives the runner's run fingerprint. The game never
    /// does: fingerprinting each step costs more than taking it.
    pub fn checking_every_step(mut self) -> Flight {
        if let Some(kept) = &mut self.kept {
            let mut run = Fingerprinter::new();
            run.write_fingerprint(self.simulation.fingerprint());
            kept.run = Some(run);
        }
        self
    }

    /// What Save Flight writes out, or `None` for a flight that isn't the
    /// pilot's, or that hasn't started (no Flying Input Device yet).
    pub fn kept(&self) -> Option<KeptFlight<'_>> {
        let kept = self.kept.as_ref()?;
        Some(KeptFlight {
            device: kept.device,
            inputs: &kept.inputs,
            outgrown_at: kept.outgrown_at,
            steps: self.simulation.time().ticks(),
            state_fingerprint: self.simulation.fingerprint(),
            run_fingerprint: kept.run.as_ref().map(Fingerprinter::finish),
        })
    }

    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }

    /// How many Simulation steps each tick runs.
    pub fn steps_per_tick(&self) -> u64 {
        self.steps_per_tick
    }

    /// A Flight Input for the Quad, stamped with the Simulation Time it
    /// arrives at. Simulation Time that has already been stepped can't
    /// change: an input stamped earlier arrives at the next step.
    pub fn input(&mut self, at: SimulationTime, input: FlightInput) {
        let at = at.max(self.simulation.time());
        if let Some(kept) = &mut self.kept {
            // In the order the Simulation takes them: by moment, and in the
            // order given within one.
            if kept.outgrown_at.is_none() {
                if kept.inputs.len() == kept.most {
                    // Too long to save from its start: let go of it all.
                    kept.inputs = Vec::new();
                    kept.outgrown_at = Some(at);
                } else {
                    let place = kept.inputs.partition_point(|(time, _)| *time <= at);
                    kept.inputs.insert(place, (at, input));
                }
            }
        }
        if input == FlightInput::Reset {
            self.resets_due.push(at);
        }
        self.simulation.flight_input(0, at, input);
    }

    /// One of Bevy's ticks: the Simulation's steps for it, one after another,
    /// each with the Flight Inputs of its own moment. A Scenario's flight
    /// stops at its last moment.
    pub fn tick(&mut self) {
        self.before = self.after;
        for _ in 0..self.steps_per_tick {
            if self.finished() {
                break;
            }
            let now = self.simulation.time();
            if let Some(script) = &mut self.script {
                for input in script.at(now, self.simulation.quad_state(0)) {
                    if input == FlightInput::Reset {
                        self.resets_due.push(now);
                    }
                    self.simulation.flight_input(0, now, input);
                }
            }
            self.simulation.step();
            let run = self.kept.as_mut().and_then(|kept| kept.run.as_mut());
            if self.fingerprints.is_some() || run.is_some() {
                let fingerprint = self.simulation.fingerprint();
                if let Some(fingerprints) = &mut self.fingerprints {
                    fingerprints.push(fingerprint);
                }
                if let Some(run) = run {
                    run.write_fingerprint(fingerprint);
                }
            }
        }
        self.after = *self.simulation.quad_state(0);
        // Reset moves the Quad to its Launch Spot at once: no motion to draw
        // across it. It arrived in this tick if its moment has been stepped
        // (the step at a Reset's moment is the one that takes it).
        let stepped_to = self.simulation.time();
        let before_reset = self.resets_due.len();
        self.resets_due.retain(|at| *at >= stepped_to);
        self.jumped = self.resets_due.len() < before_reset;
        if self.jumped {
            self.before = self.after;
        }
    }

    /// True when a Scenario's flight has reached its last moment. The
    /// pilot's flight never ends.
    pub fn finished(&self) -> bool {
        self.script
            .as_ref()
            .is_some_and(|script| self.simulation.time() >= script.length())
    }

    /// The fingerprint at the start and after every step, for a Scenario's
    /// flight.
    pub fn fingerprints(&self) -> Option<&[Fingerprint]> {
        self.fingerprints.as_deref()
    }

    /// The fingerprint of the whole run: every step's fingerprint, in order,
    /// as the Scenario runner works out a run's fingerprint.
    pub fn run_fingerprint(&self) -> Option<Fingerprint> {
        let fingerprints = self.fingerprints.as_ref()?;
        let mut f = Fingerprinter::new();
        for step in fingerprints {
            f.write_fingerprint(*step);
        }
        Some(f.finish())
    }

    /// Whether the last tick moved the Quad at once, as Reset does: there is
    /// no motion to draw across it, and nothing of the old place to show late.
    pub fn jumped_in_last_tick(&self) -> bool {
        self.jumped
    }

    /// The last two ticks' states, to draw between.
    pub fn drawn_between(&self) -> (&QuadState, &QuadState) {
        (&self.before, &self.after)
    }
}

/// A pilot's flight, kept for Save Flight.
struct Kept {
    device: InputDeviceFacts,
    inputs: Vec<(SimulationTime, FlightInput)>,
    /// The most Flight Inputs it keeps.
    most: usize,
    /// When the flight outgrew `most` inputs, the Simulation Time of the one
    /// that didn't fit: `inputs` was let go then, and stays empty.
    outgrown_at: Option<SimulationTime>,
    /// The run's fingerprint so far, when asked for
    /// ([`Flight::checking_every_step`]).
    run: Option<Fingerprinter>,
}

/// What a pilot's flight has been, so far, for Save Flight to write out as a
/// Scenario and its Input Track.
#[derive(Clone, Copy, Debug)]
pub struct KeptFlight<'f> {
    /// The facts of the Flying Input Device the flight was set up with.
    pub device: InputDeviceFacts,
    /// Every Flight Input the Simulation was handed, in the order it takes
    /// them, each stamped with the Simulation Time it arrives at. The ones
    /// stamped at or after [`KeptFlight::steps`] haven't arrived yet.
    pub inputs: &'f [(SimulationTime, FlightInput)],
    /// When the flight grew past [`MAX_KEPT_INPUTS`] Flight Inputs, the
    /// Simulation Time of the one that didn't fit. The recorder let go of
    /// every input then, so `inputs` is empty and the flight can't be saved.
    pub outgrown_at: Option<SimulationTime>,
    /// How many steps the Simulation has taken.
    pub steps: u64,
    /// The fingerprint of the Simulation's whole state now, after
    /// [`KeptFlight::steps`] steps.
    pub state_fingerprint: Fingerprint,
    /// The fingerprint of the whole run so far, from the start and after
    /// every step, for a flight that asked for it
    /// ([`Flight::checking_every_step`]).
    pub run_fingerprint: Option<Fingerprint>,
}

/// Bevy's fixed timestep at 1 kHz, running the flight's steps in one system.
pub struct SteppingPlugin;

impl Plugin for SteppingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
            .add_systems(FixedUpdate, step_the_flight);
    }
}

pub fn step_the_flight(flight: Option<NonSendMut<Flight>>) {
    if let Some(mut flight) = flight {
        flight.tick();
    }
}
