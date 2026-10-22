//! Running a Scenario through the Simulation (or, for a Flight Controller
//! Scenario, through the Flight Controller alone) and measuring its
//! Expectations.

use opendrone_flight_controller::{FlightController, Tune};
use opendrone_maths::{Attitude, Fingerprint, Fingerprinter, Vec3, functions};
use opendrone_pack::{MapDefinition, QuadDefinition};
use opendrone_pack::{Packs, Problem, Problems};
use opendrone_sim::{
    Channel, Channels, FlightControllerSeam, FlightInput, InputDeviceFacts, MapShapeProblem, Mount,
    OurFlightController, PacketRate, PhysicsRate, QuadSetUp, QuadState, RadioLink, RadioLinkOutput,
    ScriptedMotors, SensorReadings, SetUp, SetUpError, SetUpProblem, Simulation, SimulationTime,
    sensor_readings,
};
use opendrone_test_pilot::{Jobs, TestPilot};

use crate::measure::{Sample, angle_near};
use crate::osd::OsdRun;
use crate::read::{
    BasisKind, Case, Comparison, Expectation, Expecting, FlightMode, InputDevice, Inputs, Kind,
    Named, PilotChanges, PilotEntry, Scenario, Start, Statistic, Stick, When,
};

/// What one run of a Scenario measured.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub expectations: Vec<Measured>,
    /// The automatic check every Scenario gets: where the first broken number
    /// (not a number, or endless) appeared, if one did.
    pub first_broken_number: Option<String>,
    /// The whole state's fingerprint at the start (0) and after every step;
    /// for a table of cases, after each case.
    pub step_fingerprints: Vec<Fingerprint>,
    /// True when the steps are a table of cases, each its own run.
    pub steps_are_cases: bool,
    /// What the Simulation received from the Quad and the Map (no Map when
    /// the Flight Controller runs alone).
    pub quad: Received,
    pub map: Option<Received>,
    pub physics_rate: u32,
}

/// A Quad or Map's id and the fingerprint of what the Simulation received
/// from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Received {
    pub id: String,
    pub fingerprint: Fingerprint,
}

/// One Expectation's measured value.
#[derive(Clone, Debug)]
pub struct Measured {
    pub description: String,
    pub basis: BasisKind,
    /// The expected value as our tools write it.
    pub expected: String,
    /// The measured value to 3 significant figures, in the expected value's
    /// unit, or why nothing could be measured.
    pub measured: String,
    pub passed: bool,
    /// The line its `[[expect]]` starts on.
    pub line: usize,
}

impl Outcome {
    /// True when every Expectation passed and no number broke.
    pub fn passed(&self) -> bool {
        self.first_broken_number.is_none() && self.expectations.iter().all(|e| e.passed)
    }

    /// The fingerprint of the whole run: every step's fingerprint, in order.
    pub fn run_fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        for step in &self.step_fingerprints {
            f.write_fingerprint(*step);
        }
        f.finish()
    }
}

/// Runs a Scenario: builds the Simulation (or the Flight Controller alone)
/// from its starting state and the Packs, steps it at the physics rate up to
/// the last moment the Scenario mentions, and measures every Expectation on
/// the way.
pub fn run(scenario: &Scenario, packs: &Packs) -> Result<Outcome, Problems> {
    let start = &scenario.start;
    // Only the runner's Expectations need the recorded flight; the game
    // flies the Scenario without it.
    if let Some(recorded) = &scenario.recorded_flight
        && !recorded.read
    {
        return Err(Problems(vec![Problem {
            file: scenario.file.clone(),
            line: recorded.line,
            sentence: format!(
                "the recorded flight \"{}\" hasn't been read: the runner reads it from beside the Scenario",
                recorded.file
            ),
        }]));
    }
    let Found { quad, map, tune } = find(scenario, packs)?;
    let quad_received = Received {
        id: quad.id.clone(),
        fingerprint: quad.fingerprint(),
    };
    let map_received = map.as_ref().map(|map| Received {
        id: map.id.clone(),
        fingerprint: map.fingerprint(),
    });

    if start.kind == Kind::FlightController {
        let tune = tune.expect("a Flight Controller Scenario reads the Tune");
        return Ok(run_alone(scenario, &tune, &quad.name, quad_received));
    }
    let map = map.expect("every kind but the Flight Controller's names a Map");

    // This run, measuring every Expectation.
    let mut tallies: Vec<Tally> = scenario
        .expectations
        .iter()
        .map(|e| Tally::new(e, e.when, recorded_values(scenario, e)))
        .collect();
    let this_run = Plan::this_run(scenario);
    // The OSD, worked out beside the run, where the Scenario expects OSD
    // text; the reader keeps `[osd]` to Scenarios that fly our Flight
    // Controller.
    let mut osd = scenario
        .osd
        .as_ref()
        .zip(tune.as_ref())
        .map(|(section, tune)| OsdRun::new(section, &quad.name, tune));
    let (step_fingerprints, first_broken_number) = simulate(
        scenario,
        &quad,
        &map,
        tune.as_ref(),
        &this_run,
        &mut tallies,
        osd.as_mut(),
    )?;

    // Each other run, measuring the Expectations that compare with it.
    let mut others: Vec<Option<Result<f64, String>>> = vec![None; tallies.len()];
    for (index, other) in scenario.other_runs.iter().enumerate() {
        let (numbers, mut other_tallies): (Vec<usize>, Vec<Tally>) = scenario
            .expectations
            .iter()
            .enumerate()
            .filter_map(|(n, e)| {
                let compared = e.compared.filter(|c| c.run == index)?;
                Some((n, Tally::new(e, compared.when, None)))
            })
            .unzip();
        let plan = Plan {
            physics_rate: other.physics_rate,
            random_seed: other.random_seed.unwrap_or(start.random_seed),
            battery: other
                .battery
                .or(start.battery)
                .expect("the reader requires `battery` for every kind that runs the physics"),
            inputs: &other.inputs,
            length: other.length,
        };
        simulate(
            scenario,
            &quad,
            &map,
            tune.as_ref(),
            &plan,
            &mut other_tallies,
            None,
        )?;
        for (n, tally) in numbers.into_iter().zip(&other_tallies) {
            others[n] = Some(tally.value());
        }
    }

    let mut expectations: Vec<Measured> = tallies
        .into_iter()
        .zip(others)
        .map(|(tally, other)| tally.finish(other))
        .collect();
    expectations.extend(osd.map(OsdRun::finish).unwrap_or_default());
    Ok(Outcome {
        expectations,
        first_broken_number,
        step_fingerprints,
        steps_are_cases: false,
        quad: quad_received,
        map: map_received,
        physics_rate: start.physics_rate.hz(),
    })
}

/// A Scenario's Simulation, set up exactly as [`run`] sets up its run, with
/// the Flight Inputs its Timeline or Input Track gives, for a caller that
/// steps the Simulation itself. The game's reference check flies one through
/// the game's own stepping: its fingerprints must match the runner's
/// (ADR-0003, #62).
///
/// Only the Scenario's own run: not the other runs its Expectations compare
/// with. A Flight Controller Scenario has no Simulation, so it can't be set
/// up this way.
pub fn simulation(
    scenario: &Scenario,
    packs: &Packs,
) -> Result<(Simulation, ScenarioInputs), Problems> {
    let Found { quad, map, tune } = find(scenario, packs)?;
    let Some(map) = map.filter(|_| scenario.start.kind != Kind::FlightController) else {
        // No line: the refusal is about the whole Scenario, not one line of it.
        return Err(Problems::of_file(
            scenario.file.clone(),
            "a Flight Controller Scenario runs the Flight Controller alone, with no Simulation to set up",
        ));
    };
    let plan = Plan::this_run(scenario);
    let (simulation, feed) = set_up(scenario, &quad, &map, tune.as_ref(), &plan)?;
    Ok((
        simulation,
        ScenarioInputs {
            feed,
            length: scenario.length,
        },
    ))
}

/// The Flight Inputs of a Scenario's run, step by step ([`simulation`]).
pub struct ScenarioInputs {
    feed: Feed,
    length: SimulationTime,
}

impl ScenarioInputs {
    /// How long the run lasts: it ends once the Simulation has stepped to
    /// this moment.
    pub fn length(&self) -> SimulationTime {
        self.length
    }

    /// The Flight Inputs that arrive at the step that starts at `now`, in
    /// order, each to be stamped with `now`. Call it once for each step, in
    /// order, with the Quad's state at the step's start: the Test Pilot reads
    /// it, as in the runner.
    pub fn at(&mut self, now: SimulationTime, state: &QuadState) -> Vec<FlightInput> {
        self.feed.inputs_at(now.ticks(), Some(state))
    }
}

/// What a Scenario flies with, from the Packs: its Quad, its Map (none when
/// the Flight Controller runs alone), and the Tune our Flight Controller
/// reads (none for scripted motors).
struct Found {
    quad: QuadDefinition,
    map: Option<MapDefinition>,
    tune: Option<Tune>,
}

/// Finds a Scenario's Quad, Map and Tune in the Packs, or says why it can't.
fn find(scenario: &Scenario, packs: &Packs) -> Result<Found, Problems> {
    let start = &scenario.start;
    let mut problems = Problems::new();
    let named = |what: &str, named: &Named, found: Problems| {
        let mut all = Problems(vec![Problem {
            file: scenario.file.clone(),
            line: named.line,
            sentence: format!("can't use the {what} \"{}\":", named.id),
        }]);
        all.extend(found);
        all
    };
    let quad = packs
        .quad(&start.quad.id)
        .map_err(|found| problems.extend(named("Quad", &start.quad, found)))
        .ok();
    let map = match &start.map {
        None => None,
        Some(map) => match packs.map(&map.id) {
            Ok(found) => Some(found),
            Err(found) => {
                problems.extend(named("Map", map, found));
                return Err(problems);
            }
        },
    };
    let Some(quad) = quad else {
        return Err(problems);
    };
    if let Inputs::Track(track) = &scenario.inputs
        && !track.read
    {
        return Err(Problems(vec![Problem {
            file: scenario.file.clone(),
            line: track.line,
            sentence: format!(
                "the Input Track \"{}\" hasn't been read: the runner reads it from beside the Scenario",
                track.file
            ),
        }]));
    }
    let unread = |line: usize, sentence: String| {
        Err(Problems(vec![Problem {
            file: scenario.file.clone(),
            line,
            sentence,
        }]))
    };
    if let Inputs::MotorTrack(track) = &scenario.inputs {
        return unread(
            track.line,
            format!(
                "the Input Track \"{}\" hasn't been read: the runner reads it from beside the Scenario",
                track.file
            ),
        );
    }
    // Scripted motors need no Flight Controller; a pilot's inputs fly ours,
    // on the thrust stand too.
    let tune = match (start.kind, &scenario.inputs) {
        (Kind::Physics, _) | (Kind::ThrustStand, Inputs::Motors(_)) => None,
        _ => match &quad.flight_controller {
            Ok(tune) => Some(tune.clone()),
            Err(missing) => {
                return Err(Problems(vec![Problem {
                    file: scenario.file.clone(),
                    line: start.quad.line,
                    sentence: format!(
                        "the Quad \"{}\" can't fly with our Flight Controller yet: its Tune doesn't spell out {} (ADR-0015)",
                        start.quad.id,
                        missing
                            .iter()
                            .map(|m| format!("`{m}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }]));
            }
        },
    };
    Ok(Found { quad, map, tune })
}

/// One run of a Scenario: its physics rate, random seed, battery and
/// Timeline.
struct Plan<'s> {
    physics_rate: PhysicsRate,
    random_seed: u64,
    battery: f64,
    inputs: &'s Inputs,
    length: SimulationTime,
}

impl<'s> Plan<'s> {
    /// The Scenario's own run, as its starting state and Timeline give it.
    fn this_run(scenario: &'s Scenario) -> Plan<'s> {
        let start = &scenario.start;
        Plan {
            physics_rate: start.physics_rate,
            random_seed: start.random_seed,
            battery: start
                .battery
                .expect("the reader requires `battery` for every kind that runs the physics"),
            inputs: &scenario.inputs,
            length: scenario.length,
        }
    }
}

/// Builds the Simulation for one run and steps it to the run's end, showing
/// every tally the output after each step, and the OSD, if one is worked out
/// beside the run, each Flight Controller loop. Gives back the whole state's
/// fingerprint after every step, and where a broken number first appeared.
fn simulate(
    scenario: &Scenario,
    quad: &QuadDefinition,
    map: &MapDefinition,
    tune: Option<&Tune>,
    plan: &Plan<'_>,
    tallies: &mut [Tally<'_>],
    mut osd: Option<&mut OsdRun<'_>>,
) -> Result<(Vec<Fingerprint>, Option<String>), Problems> {
    let (mut sim, mut feed) = set_up(scenario, quad, map, tune, plan)?;
    // Where the motors are scripted, no sticks reach a Flight Controller.
    let flies = !matches!(plan.inputs, Inputs::Motors(_));
    let step = plan.physics_rate.step_length();
    let mut step_fingerprints = Vec::with_capacity(plan.length.ticks() as usize + 1);
    let mut first_broken_number = None;
    let mut before: Option<Sample> = None;
    for tick in 0..=plan.length.ticks() {
        if tick > 0 {
            // The pilot's Flight Inputs for the step that starts now enter the
            // Simulation: from a Timeline, the Flying Input Device lost or
            // back, Reset, and the Channels whenever they change; from an
            // Input Track, each as it was recorded.
            for input in feed.inputs_at(tick - 1, Some(sim.quad_state(0))) {
                sim.flight_input(0, SimulationTime::from_ticks(tick - 1), input);
            }
            sim.step();
        }
        let output = sim.quad_output(0);
        let now = Sample {
            quad: Some(output),
            flight_controller: output.flight_controller,
            radio_link: (flies && tick > 0).then_some(output.radio_link),
            gravity: map.world.gravity,
        };
        step_fingerprints.push(sim.fingerprint());
        let finite = sim.is_finite() && output.flight_controller.is_none_or(|r| finite_record(&r));
        if first_broken_number.is_none() && !finite {
            first_broken_number = Some(format!(
                "first at {} s (step {tick}): the Quad's state holds a number that isn't a real number",
                sim.time().seconds(plan.physics_rate)
            ));
        }
        for tally in tallies.iter_mut() {
            tally.see(tick, before.as_ref(), &now, step);
        }
        if let Some(osd) = osd.as_mut() {
            let record = if tick > 0 {
                output.flight_controller
            } else {
                None
            };
            osd.see(tick, record.as_ref(), step);
        }
        before = Some(now);
    }
    Ok((step_fingerprints, first_broken_number))
}

/// Builds the Simulation for one run, with its pilot's Flight Inputs.
fn set_up(
    scenario: &Scenario,
    quad: &QuadDefinition,
    map: &MapDefinition,
    tune: Option<&Tune>,
    plan: &Plan<'_>,
) -> Result<(Simulation, Feed), Problems> {
    let start = &scenario.start;
    let state = QuadState {
        position: start.position,
        velocity: start.velocity,
        attitude: start.attitude,
        rotation: start.rotation,
    };
    let flight_controller: Box<dyn FlightControllerSeam> = match (plan.inputs, tune) {
        (Inputs::Motors(timeline), _) => Box::new(ScriptedMotors::new(timeline.clone())),
        (Inputs::Pilot(_) | Inputs::Track(_), Some(tune)) => Box::new(OurFlightController::new(
            tune.clone(),
            start.rates.clone(),
            plan.physics_rate,
            packet_rate(start),
            start.armed,
            start.assists.auto_arm,
            &sensor_readings(&state, false, quad.parameters.gyro_range),
        )),
        (Inputs::Pilot(_) | Inputs::Track(_), None) => {
            unreachable!("a pilot's inputs fly our Flight Controller")
        }
        (Inputs::MotorTrack(_), _) => {
            unreachable!("`find` refuses a motor Input Track that hasn't been read")
        }
    };
    // Where the motors are scripted, no sticks reach a Flight Controller.
    let flies = !matches!(plan.inputs, Inputs::Motors(_));
    let feed = Feed::new(plan.inputs, start, plan.physics_rate);
    let set_up = SetUp {
        physics_rate: plan.physics_rate,
        world: map.world,
        map: map.shapes.clone(),
        random_seed: plan.random_seed,
        quads: vec![QuadSetUp {
            parameters: quad.parameters.clone(),
            start: state,
            // A Scenario's Launch Spot is where it starts: only a Scenario
            // that starts as Reset leaves the Quad, landed and still, may
            // press Reset (the reader checks), and Reset puts it back there.
            launch_spot: state,
            motors: start
                .motors
                .expect("every kind but the Flight Controller's says how its motors start"),
            battery: plan.battery,
            mount: mount(start),
            packet_rate: packet_rate(start),
            input_device: feed.device(),
            input_smoothing: flies && start.assists.input_smoothing,
            endless_battery: flies && start.assists.endless_battery,
            flight_controller,
        }],
    };
    let sim = Simulation::new(set_up).map_err(|error| {
        let (line, sentence) = match error {
            SetUpError::Quad { problem, .. } => {
                let why = match problem {
                    SetUpProblem::MassNotAboveZero => "its mass must be above zero".to_string(),
                    SetUpProblem::InertiaHasNoInverse => {
                        "its inertia can't be turned around (no inverse)".to_string()
                    }
                    SetUpProblem::NotAboveZero(what) => format!("{what} must be above zero"),
                    SetUpProblem::NoVoltageCurve => {
                        "its battery's voltage curve has no points".to_string()
                    }
                    SetUpProblem::PropWashOutOfRange => {
                        "its Prop Wash strength must be from 0% to 100% and its flicker 0 Hz or more".to_string()
                    }
                    SetUpProblem::ShapeCantBeBuilt => {
                        "its collision shape needs every size above zero, a bounce from 0 to 1, and a friction and a prop grip of 0 or more".to_string()
                    }
                    SetUpProblem::GroundOrCeilingEffectCantWork => {
                        "its ground_effect_body and ceiling_effect_asymmetry must be 0 or more, and ground_effect_body small enough for its rotors' layout that close to a floor its rotors still push air down".to_string()
                    }
                };
                (
                    start.quad.line,
                    format!("the Quad \"{}\" can't fly: {why}", start.quad.id),
                )
            }
            SetUpError::MapShape { shape, problem } => {
                let why = match problem {
                    MapShapeProblem::NotARealNumber => "a number in it isn't a real number",
                    MapShapeProblem::BoxHasNoSize => "a box needs a size above zero each way",
                    MapShapeProblem::ConvexHasNoVolume => {
                        "a convex shape needs at least four triangles that enclose some volume"
                    }
                    MapShapeProblem::ConvexIsNotClosed => {
                        "a convex shape's triangles must close it: every edge shared by two triangles running opposite ways, and every corner used"
                    }
                    MapShapeProblem::ConvexIsNotConvex => {
                        "a convex shape's triangles must face outward with every corner behind each triangle's plane"
                    }
                    MapShapeProblem::MeshHasNoTriangles => "a triangle mesh needs a triangle",
                    MapShapeProblem::MeshCornerMissing => {
                        "a triangle names a corner the mesh doesn't have"
                    }
                };
                (
                    start.map.as_ref().map_or(0, |named| named.line),
                    format!(
                        "the Map \"{}\" can't be flown: its shape {shape} (counting from 0) isn't solid: {why}",
                        map.id
                    ),
                )
            }
        };
        Problems(vec![Problem {
            file: scenario.file.clone(),
            line,
            sentence,
        }])
    })?;
    Ok((sim, feed))
}

/// Runs a Flight Controller Scenario: the Flight Controller alone, fed its
/// Timeline, or each case of its table on a fresh Flight Controller.
fn run_alone(scenario: &Scenario, tune: &Tune, quad_name: &str, quad: Received) -> Outcome {
    let start = &scenario.start;
    let mut first_broken_number = None;
    let mut step_fingerprints = Vec::new();
    let measured = if scenario.cases.is_empty() {
        let mut tallies: Vec<Tally> = scenario
            .expectations
            .iter()
            .map(|e| Tally::new(e, e.when, recorded_values(scenario, e)))
            .collect();
        let mut feed = Feed::new(&scenario.inputs, start, start.physics_rate);
        let mut osd = scenario
            .osd
            .as_ref()
            .map(|section| OsdRun::new(section, quad_name, tune));
        let (fingerprints, broken) = loop_alone(
            start,
            tune,
            &mut feed,
            scenario.length,
            &mut tallies,
            osd.as_mut(),
        );
        step_fingerprints = fingerprints;
        first_broken_number = broken;
        let mut measured: Vec<Measured> = tallies.into_iter().map(|t| t.finish(None)).collect();
        measured.extend(osd.map(OsdRun::finish).unwrap_or_default());
        measured
    } else {
        let mut measured = Vec::new();
        for case in &scenario.cases {
            let mut tallies: Vec<Tally> = case
                .expectations
                .iter()
                .map(|e| Tally::new(e, e.when, None))
                .collect();
            let timeline = Inputs::Pilot(case_timeline(case));
            let mut feed = Feed::new(&timeline, start, start.physics_rate);
            let (fingerprints, broken) = loop_alone(
                start,
                tune,
                &mut feed,
                SimulationTime::from_ticks(1),
                &mut tallies,
                None,
            );
            if first_broken_number.is_none()
                && let Some(broken) = broken
            {
                first_broken_number = Some(format!("in the case on line {}, {broken}", case.line));
            }
            step_fingerprints.push(*fingerprints.last().expect("a case runs one loop"));
            measured.extend(tallies.into_iter().map(|t| t.finish(None)));
        }
        measured
    };
    Outcome {
        expectations: measured,
        first_broken_number,
        step_fingerprints,
        steps_are_cases: !scenario.cases.is_empty(),
        quad,
        map: None,
        physics_rate: start.physics_rate.hz(),
    }
}

/// A case as a Timeline of one moment at the start.
fn case_timeline(case: &Case) -> Vec<PilotEntry> {
    vec![PilotEntry {
        at: SimulationTime::START,
        changes: case.inputs,
    }]
}

/// The Flight Controller alone, one loop per physics step for `length`
/// steps, with Radio Link frames at the Packet Rate as a receiver hands them
/// over. Gives back the fingerprint of its whole state at the start and after
/// every loop, and where a broken number first appeared.
fn loop_alone(
    start: &Start,
    tune: &Tune,
    feed: &mut Feed,
    length: SimulationTime,
    tallies: &mut [Tally<'_>],
    mut osd: Option<&mut OsdRun<'_>>,
) -> (Vec<Fingerprint>, Option<String>) {
    let rate = start.physics_rate;
    let mut flight_controller = FlightController::new(
        tune.clone(),
        start.rates.clone(),
        rate.hz(),
        packet_rate(start).hz(),
        start.armed,
        &feed.readings(0),
    );
    let mut link = RadioLink::new(packet_rate(start), rate, feed.device());
    let mut output = RadioLinkOutput::default();
    let fingerprint = |fc: &FlightController, link: &RadioLink| {
        let mut f = Fingerprinter::new();
        fc.write_fingerprint(&mut f);
        link.write_fingerprint(&mut f);
        f.finish()
    };
    let mut fingerprints = vec![fingerprint(&flight_controller, &link)];
    let mut first_broken_number = None;
    let mut before: Option<Sample> = None;
    for tick in 0..=length.ticks() {
        let mut now = Sample {
            quad: None,
            flight_controller: None,
            radio_link: None,
            gravity: 0.0,
        };
        if tick > 0 {
            let t = tick - 1;
            let at = SimulationTime::from_ticks(t);
            for input in feed.inputs_at(t, None) {
                match input {
                    FlightInput::Channels(channels) => link.hear(at, channels),
                    FlightInput::InputDeviceLost => link.lose(at),
                    FlightInput::InputDeviceBack => link.back(at),
                    // The reader keeps Reset to Flight Scenarios.
                    FlightInput::Reset => {}
                }
            }
            let frame = link.frame(at);
            output = output.after(frame, &link);
            let channels = frame.map(|frame| frame.channels);
            flight_controller.step(&feed.readings(t), channels.as_ref());
            now.flight_controller = Some(*flight_controller.debug());
            now.radio_link = Some(output);
            fingerprints.push(fingerprint(&flight_controller, &link));
            if first_broken_number.is_none() && !finite_record(flight_controller.debug()) {
                first_broken_number = Some(format!(
                    "first at {} s (step {tick}): the Flight Controller holds a number that isn't a real number",
                    SimulationTime::from_ticks(tick).seconds(rate)
                ));
            }
        }
        for tally in tallies.iter_mut() {
            tally.see(tick, before.as_ref(), &now, rate.step_length());
        }
        if let Some(osd) = osd.as_mut() {
            osd.see(tick, now.flight_controller.as_ref(), rate.step_length());
        }
        before = Some(now);
    }
    (fingerprints, first_broken_number)
}

/// True when every number in a Flight Controller loop's record is a real
/// number.
fn finite_record(record: &opendrone_sim::DebugRecord) -> bool {
    let terms = record
        .terms
        .iter()
        .flat_map(|t| [t.p, t.i, t.d, t.f, t.sum]);
    record
        .setpoint
        .iter()
        .chain(&record.gyro)
        .copied()
        .chain(terms)
        .chain([record.throttle])
        .all(f64::is_finite)
}

/// The pilot's Flight Inputs, step by step, and for the Flight Controller
/// alone the sensor readings.
enum Feed {
    /// From a Timeline: the Flying Input Device lost or back, Reset, and the
    /// Channels whenever they change. Scripted sticks have no Input Device.
    ///
    /// A Flight Scenario's Timeline may also hand sticks to the Test Pilot,
    /// which reads the Quad's state and moves only those sticks.
    Timeline {
        track: Box<PilotTrack>,
        test_pilot: TestPilot,
        last: Option<Channels>,
    },
    /// From an Input Track: each Flight Input as recorded, from the device
    /// whose facts it gives. The sensor readings hold `[start]`'s.
    Recorded {
        inputs: Vec<(SimulationTime, FlightInput)>,
        next: usize,
        device: InputDeviceFacts,
        readings: SensorReadings,
    },
    /// Scripted motors: no pilot.
    None,
}

impl Feed {
    fn new(inputs: &Inputs, start: &Start, rate: PhysicsRate) -> Feed {
        match inputs {
            Inputs::Pilot(entries) => Feed::Timeline {
                track: Box::new(PilotTrack::new(entries, start)),
                test_pilot: TestPilot::new(rate.hz()),
                last: None,
            },
            Inputs::Track(track) => Feed::Recorded {
                inputs: track.inputs.clone(),
                next: 0,
                device: track.device,
                readings: SensorReadings::still(start.rotation, start.attitude, true),
            },
            Inputs::Motors(_) | Inputs::MotorTrack(_) => Feed::None,
        }
    }

    /// The Input Device's facts, for the Radio Link: none for scripted
    /// sticks, which get plain regular frames.
    fn device(&self) -> Option<InputDeviceFacts> {
        match self {
            Feed::Recorded { device, .. } => Some(*device),
            Feed::Timeline { .. } | Feed::None => None,
        }
    }

    /// The Flight Inputs that arrive at a step, in order. Call it once for
    /// each step, in order. `state` is the Quad's, as the Test Pilot sees it
    /// at the step's start; the Flight Controller alone has none.
    fn inputs_at(&mut self, tick: u64, state: Option<&QuadState>) -> Vec<FlightInput> {
        match self {
            Feed::Timeline {
                track,
                test_pilot,
                last,
            } => {
                let mut inputs: Vec<FlightInput> = track.events_at(tick).collect();
                let mut channels = track.channels(tick);
                for jobs in track.jobs_at(tick) {
                    test_pilot.set_jobs(jobs, &channels);
                }
                if let Some(state) = state {
                    channels = test_pilot.fly(state, channels);
                }
                if *last != Some(channels) {
                    inputs.push(FlightInput::Channels(channels));
                    *last = Some(channels);
                }
                inputs
            }
            Feed::Recorded { inputs, next, .. } => {
                let mut arrived = Vec::new();
                while let Some((at, input)) = inputs.get(*next)
                    && at.ticks() <= tick
                {
                    arrived.push(*input);
                    *next += 1;
                }
                arrived
            }
            Feed::None => Vec::new(),
        }
    }

    /// The sensor readings at a step, for the Flight Controller alone.
    fn readings(&self, tick: u64) -> SensorReadings {
        match self {
            Feed::Timeline { track, .. } => track.readings(tick),
            Feed::Recorded { readings, .. } => *readings,
            Feed::None => SensorReadings::still(Vec3::ZERO, Attitude::BODY_IS_WORLD, true),
        }
    }
}

/// The pilot's Timeline, ready to read at any step: each stick's values (with
/// ramps between), the Arm and Crash Flip switches and, for the Flight
/// Controller alone, the sensor readings.
struct PilotTrack {
    sticks: [Vec<(u64, Stick)>; 4],
    arm: Vec<(u64, bool)>,
    crash_flip: Vec<(u64, bool)>,
    rotation: Vec<(u64, Vec3)>,
    attitude: Vec<(u64, Attitude)>,
    /// The Flight Inputs besides the Channels, each at its step, in order:
    /// the Flying Input Device lost or back, and Reset.
    events: Vec<(u64, FlightInput)>,
    /// The Test Pilot's jobs, each from its step on.
    jobs: Vec<(u64, Jobs)>,
    /// AUX2, from the start's Flight Mode: no Flight Mode switch is bound in
    /// a Scenario, so the pilot's Flight Mode setting drives it (ADR-0017).
    flight_mode: Channel,
}

impl PilotTrack {
    fn new(entries: &[PilotEntry], start: &Start) -> PilotTrack {
        let mut track = PilotTrack {
            sticks: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            arm: Vec::new(),
            crash_flip: Vec::new(),
            rotation: vec![(0, start.rotation)],
            attitude: vec![(0, start.attitude)],
            events: Vec::new(),
            jobs: Vec::new(),
            flight_mode: match start.flight_mode {
                FlightMode::Acro => Channel::LOW,
                FlightMode::Horizon => Channel::CENTRE,
                FlightMode::Angle => Channel::HIGH,
            },
        };
        for entry in entries {
            let tick = entry.at.ticks();
            let PilotChanges {
                roll,
                pitch,
                yaw,
                throttle,
                arm,
                crash_flip,
                rotation,
                attitude,
                input_device,
                reset,
                test_pilot,
            } = entry.changes;
            if let Some(jobs) = test_pilot {
                track.jobs.push((tick, jobs));
            }
            match input_device {
                Some(InputDevice::Lost) => track.events.push((tick, FlightInput::InputDeviceLost)),
                Some(InputDevice::Back) => track.events.push((tick, FlightInput::InputDeviceBack)),
                None => {}
            }
            if reset {
                track.events.push((tick, FlightInput::Reset));
            }
            for (keys, stick) in track.sticks.iter_mut().zip([roll, pitch, yaw, throttle]) {
                if let Some(stick) = stick {
                    keys.push((tick, stick));
                }
            }
            if let Some(arm) = arm {
                track.arm.push((tick, arm));
            }
            if let Some(crash_flip) = crash_flip {
                track.crash_flip.push((tick, crash_flip));
            }
            if let Some(rotation) = rotation {
                track.rotation.push((tick, rotation));
            }
            if let Some(attitude) = attitude {
                track.attitude.push((tick, attitude));
            }
        }
        track
    }

    /// The Channels at a step: each stick rounded to the nearest whole step
    /// a receiver outputs.
    fn channels(&self, tick: u64) -> Channels {
        let share = |k: usize| stick_at(&self.sticks[k], tick);
        Channels {
            roll: Channel::from_stick(share(0)),
            pitch: Channel::from_stick(share(1)),
            yaw: Channel::from_stick(share(2)),
            throttle: Channel::from_throttle(share(3)),
            arm: Channel::from_switch(held(&self.arm, tick).unwrap_or(false)),
            flight_mode: self.flight_mode,
            crash_flip: Channel::from_switch(held(&self.crash_flip, tick).unwrap_or(false)),
        }
    }

    /// The Flight Inputs besides the Channels that arrive at a step, in the
    /// file's order.
    fn events_at(&self, tick: u64) -> impl Iterator<Item = FlightInput> + '_ {
        self.events
            .iter()
            .filter(move |(at, _)| *at == tick)
            .map(|(_, input)| *input)
    }

    /// The Test Pilot's jobs set at a step, in the file's order.
    fn jobs_at(&self, tick: u64) -> impl Iterator<Item = Jobs> + '_ {
        self.jobs
            .iter()
            .filter(move |(at, _)| *at == tick)
            .map(|(_, jobs)| *jobs)
    }

    /// The sensor readings at a step, for the Flight Controller alone. It
    /// has no ESCs to wait for: a Flight Controller Scenario's ESCs count as
    /// ready. It has no battery and no motors, so its Flight Controller finds
    /// no battery and reads no RPM.
    fn readings(&self, tick: u64) -> SensorReadings {
        SensorReadings::still(
            held(&self.rotation, tick).unwrap_or(Vec3::ZERO),
            held(&self.attitude, tick).unwrap_or(Attitude::BODY_IS_WORLD),
            true,
        )
    }
}

/// The last value set at or before `tick`.
fn held<T: Copy>(keys: &[(u64, T)], tick: u64) -> Option<T> {
    let place = keys.partition_point(|(at, _)| *at <= tick);
    place.checked_sub(1).map(|i| keys[i].1)
}

/// A stick's share at `tick`: the last value set, or, while a later moment
/// ramps to its value, a straight line from the last value to that one.
fn stick_at(keys: &[(u64, Stick)], tick: u64) -> f64 {
    let place = keys.partition_point(|(at, _)| *at <= tick);
    let Some((from_tick, from)) = place.checked_sub(1).map(|i| keys[i]) else {
        return 0.0;
    };
    match keys.get(place) {
        Some((to_tick, to)) if to.ramp => {
            let along = (tick - from_tick) as f64 / (to_tick - from_tick) as f64;
            from.share + (to.share - from.share) * along
        }
        _ => from.share,
    }
}

/// A Thrust Stand Scenario holds the Quad still; every other kind lets it
/// fly.
fn mount(start: &Start) -> Mount {
    match start.kind {
        Kind::ThrustStand => Mount::ThrustStand,
        Kind::Flight | Kind::FlightController | Kind::Physics => Mount::Free,
    }
}

fn packet_rate(start: &Start) -> PacketRate {
    PacketRate::from_hz(start.packet_rate)
        .expect("the reader takes only Packet Rates a pilot can pick")
}

/// How close to the far side, or to a whole turn, an angle counts as there:
/// rounding, in radians.
const ROUNDING: f64 = 1e-9;

/// One Expectation's measurement, built up step by step.
struct Tally<'s> {
    expectation: &'s Expectation,
    /// When to measure, in this run's steps.
    when: When,
    count: u64,
    sum: f64,
    lowest: f64,
    highest: f64,
    last: Option<f64>,
    /// True once an angle jumped, or reached half a turn from the expected
    /// value, during the stretch (see `see`).
    no_single_answer: bool,
    /// How many of the samples were roll or heading read with the nose
    /// straight up or down (see `see`).
    straight_up_or_down: u64,
    /// For something that happens: how long after the stretch's start it
    /// first did, in seconds.
    first: Option<f64>,
    /// For a gap from a recorded flight: the recorded values, in SI units,
    /// each at its step, in time order.
    recorded: Option<&'s [(u64, f64)]>,
    /// For a gap: the sum of the gaps' squares.
    sum_of_squares: f64,
}

/// The recorded values an Expectation that matches a recorded flight
/// compares with: its measure's column. `None` for any other Expectation.
fn recorded_values<'s>(scenario: &'s Scenario, e: &Expectation) -> Option<&'s [(u64, f64)]> {
    match e.when {
        When::Over { statistic, .. } if statistic.is_a_gap() => scenario
            .recorded_flight
            .as_ref()?
            .column(e.measure)
            .map(|column| column.values.as_slice()),
        _ => None,
    }
}

impl<'s> Tally<'s> {
    fn new(
        expectation: &'s Expectation,
        when: When,
        recorded: Option<&'s [(u64, f64)]>,
    ) -> Tally<'s> {
        Tally {
            expectation,
            when,
            count: 0,
            sum: 0.0,
            lowest: f64::INFINITY,
            highest: f64::NEG_INFINITY,
            last: None,
            no_single_answer: false,
            straight_up_or_down: 0,
            first: None,
            recorded,
            sum_of_squares: 0.0,
        }
    }

    fn see(&mut self, tick: u64, before: Option<&Sample>, now: &Sample, step: f64) {
        let wanted = match self.when {
            When::At(at) => tick == at,
            When::Over { from, to, .. } => from < tick && tick <= to,
        };
        if !wanted {
            return;
        }
        let Some(mut value) = self.expectation.measure.read(before, now, step) else {
            return;
        };
        // Matching a recorded flight: only at the recorded moments, the gap
        // between this run and the recording.
        if let When::Over { statistic, .. } = self.when
            && statistic.is_a_gap()
        {
            let Some(recorded) = self.recorded else {
                return;
            };
            let Ok(at) = recorded.binary_search_by_key(&tick, |(t, _)| *t) else {
                return;
            };
            let gap = value - recorded[at].1;
            self.count += 1;
            self.sum += gap;
            self.sum_of_squares += gap * gap;
            return;
        }
        // Something that happens reads 1 on the step it happens.
        if let When::Over {
            from,
            statistic: Statistic::First,
            ..
        } = self.when
        {
            if value == 1.0 && self.first.is_none() {
                self.first = Some((tick - from) as f64 * step);
            }
            return;
        }
        // Every angle is taken the short way round from the expected value
        // before it counts, so a heading that crosses north (359.5° to 1.5°)
        // or a roll that crosses upside down (179.5° to -178.5°) has the
        // right lowest, highest, mean and final value.
        //
        // That only works while the angle stays strictly within half a turn
        // of the expected value and moves smoothly. Otherwise the lowest,
        // highest and mean would come out near whatever was expected, so
        // those three then fail (`final` and values at a moment don't). Three
        // things break it:
        //
        // - The angle sweeps past the side opposite the expected value, as in
        //   a flip or a pirouette: the taken-round values jump by a whole
        //   turn between two steps.
        // - The nose passes straight up or down: roll and heading jump by
        //   half a turn, because pitch only reads from -90° to 90°.
        // - The angle reaches the far side exactly, so the turn round to it
        //   is as long one way as the other: a whole turn can then end
        //   there without a jump.
        //
        // A jump of a quarter turn or more counts: a real turn that fast in
        // one step would be 720,000 °/s at 8 kHz. The far side counts to
        // within rounding.
        if self.expectation.measure.is_an_angle()
            && let Expecting::Value(expected) = &self.expectation.expected
        {
            let centre = expected.centre();
            value = angle_near(value, centre);
            let jumped = self
                .last
                .is_some_and(|last| (value - last).abs() >= core::f64::consts::FRAC_PI_2);
            let at_the_far_side = (value - centre).abs() >= core::f64::consts::PI - ROUNDING;
            if jumped || at_the_far_side {
                self.no_single_answer = true;
            }
        }
        // Within about 0.00000006° of straight up or down, roll reads 0° and
        // heading carries the whole turn. A Quad that leaves that band slowly
        // and sideways can step from one reading to the other by less than a
        // quarter turn, so the jump check misses it, yet the two readings
        // don't belong in one lowest, highest or mean. So a stretch that
        // mixes them has none of the three. A stretch wholly inside the band,
        // like a spin about the nose pointing straight up, keeps them.
        if self.expectation.measure.read_straight_up_or_down(now) {
            self.straight_up_or_down += 1;
        }
        self.count += 1;
        self.sum += value;
        // Not std's f64::min and f64::max, which may pick either zero when
        // given +0 and -0.
        self.lowest = functions::min(self.lowest, value);
        self.highest = functions::max(self.highest, value);
        self.last = Some(value);
    }

    /// The value measured, in SI units, or why there is none.
    fn value(&self) -> Result<f64, String> {
        let e = self.expectation;
        // A whole turn between the lowest and the highest can only happen if
        // the angle reached the far side, which `see` already caught; it is
        // checked again here so no way round it is left.
        let whole_turn = e.measure.is_an_angle()
            && self.highest - self.lowest >= 2.0 * core::f64::consts::PI - ROUNDING;
        let partly_straight_up_or_down =
            self.straight_up_or_down > 0 && self.straight_up_or_down < self.count;
        if let When::Over { statistic, .. } = self.when
            && statistic != Statistic::Final
        {
            let (what, word) = (e.measure.name(), statistic.word());
            let why = if self.no_single_answer || whole_turn {
                Some(format!(
                    "the {what} jumped, or reached half a turn from the expected value, during the stretch, as it does in flips and when the nose passes straight up or down, so its {word}"
                ))
            } else if partly_straight_up_or_down {
                Some(format!(
                    "for part of the stretch, and not all of it, the nose was within 0.00000006° of straight up or down, where roll reads 0° and heading carries the whole turn, so the {what}'s {word}"
                ))
            } else {
                None
            };
            if let Some(why) = why {
                return Err(format!(
                    "none: {why} has no single answer; check it at moments, over a shorter stretch, or check its rate"
                ));
            }
        }
        let value = match self.when {
            When::At(_) => self.last,
            When::Over {
                statistic: Statistic::First,
                ..
            } => return self.first.ok_or_else(|| "never".to_string()),
            When::Over { statistic, .. } if self.count > 0 => Some(match statistic {
                Statistic::Mean | Statistic::MeanGap => self.sum / self.count as f64,
                Statistic::RmsGap => (self.sum_of_squares / self.count as f64).sqrt(),
                Statistic::Lowest => self.lowest,
                Statistic::Highest => self.highest,
                Statistic::Final | Statistic::First => self.last.unwrap_or(f64::NAN),
            }),
            When::Over { .. } => None,
        };
        value.ok_or_else(|| "nothing measured".to_string())
    }

    /// The Expectation's measured value and verdict. `other` is the other
    /// run's value, for an Expectation that compares with one.
    fn finish(self, other: Option<Result<f64, String>>) -> Measured {
        let e = self.expectation;
        let expected = match &e.expected {
            Expecting::Value(expected) => expected,
            // Something that should never happen in its stretch.
            Expecting::Never => {
                let (measured, passed) = match self.first {
                    None => ("never".to_string(), true),
                    Some(after) => (
                        format!(
                            "after {} s",
                            opendrone_pack::units::significant_figures(after, 3)
                        ),
                        false,
                    ),
                };
                return Measured {
                    description: e.description.clone(),
                    basis: e.basis.kind,
                    expected: e.expected.text(),
                    measured,
                    passed,
                    line: e.line,
                };
            }
        };
        let value = match (e.compared, self.value(), other) {
            (None, value, _) => value,
            (Some(_), Err(why), _) => Err(why),
            (Some(_), Ok(_), None) => Err("nothing measured in the other run".to_string()),
            (Some(_), Ok(_), Some(Err(why))) => Err(format!("in the other run, {why}")),
            (Some(compared), Ok(this), Some(Ok(that))) => match compared.how {
                Comparison::Difference => Ok(this - that),
                Comparison::Ratio if that != 0.0 => Ok(this / that),
                Comparison::Ratio => Err(format!(
                    "none: the other run measured {}, and a share of nothing has no answer",
                    expected.unit().write(that)
                )),
                Comparison::Gap => Ok((this - that).abs()),
            },
        };
        let (measured, passed) = match value {
            Ok(value) => (
                expected.unit().write(value),
                value.is_finite() && expected.accepts(value),
            ),
            Err(why) => (why, false),
        };
        Measured {
            description: e.description.clone(),
            basis: e.basis.kind,
            expected: e.expected.text(),
            measured,
            passed,
            line: e.line,
        }
    }
}
