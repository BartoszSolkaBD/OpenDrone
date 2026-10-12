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
use opendrone_sim::{FlightInput, QuadState, Simulation, SimulationTime};

/// Bevy's fixed timestep: 1,000 ticks a second.
pub const TICK_HZ: u32 = 1000;

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
    /// The Quad's state after the tick before the last, and after the last.
    before: QuadState,
    after: QuadState,
    /// A Reset arrives during the next tick: draw no motion across it.
    reset_in_last_tick: bool,
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
            before: state,
            after: state,
            reset_in_last_tick: false,
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
        self.reset_in_last_tick |= input == FlightInput::Reset;
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
                    self.reset_in_last_tick |= input == FlightInput::Reset;
                    self.simulation.flight_input(0, now, input);
                }
            }
            self.simulation.step();
            if let Some(fingerprints) = &mut self.fingerprints {
                fingerprints.push(self.simulation.fingerprint());
            }
        }
        self.after = *self.simulation.quad_state(0);
        // Reset moves the Quad to its Launch Spot at once: no motion to draw
        // across it.
        if std::mem::take(&mut self.reset_in_last_tick) {
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

    /// The last two ticks' states, to draw between.
    pub fn drawn_between(&self) -> (&QuadState, &QuadState) {
        (&self.before, &self.after)
    }
}

/// Bevy's fixed timestep at 1 kHz, running the flight's steps in one system.
pub struct SteppingPlugin;

impl Plugin for SteppingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
            .add_systems(FixedUpdate, step_the_flight);
    }
}

fn step_the_flight(flight: Option<NonSendMut<Flight>>) {
    if let Some(mut flight) = flight {
        flight.tick();
    }
}
