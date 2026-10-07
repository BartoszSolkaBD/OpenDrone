//! Running a Scenario through the Simulation and measuring its Expectations.

use opendrone_maths::{Fingerprint, Fingerprinter};
use opendrone_pack::{Packs, Problem, Problems};
use opendrone_sim::{QuadSetUp, QuadState, ScriptedMotors, SetUp, SetUpProblem, Simulation};

use crate::measure::angle_near;
use crate::read::{BasisKind, Expectation, Scenario, Statistic, When};

/// What one run of a Scenario measured.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub expectations: Vec<Measured>,
    /// The automatic check every Scenario gets: where the first broken number
    /// (not a number, or endless) appeared, if one did.
    pub first_broken_number: Option<String>,
    /// The whole state's fingerprint at the start (0) and after every step.
    pub step_fingerprints: Vec<Fingerprint>,
    /// What the Simulation received from the Quad and the Map.
    pub quad: (String, Fingerprint),
    pub map: (String, Fingerprint),
    pub physics_rate: u32,
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

/// Runs a Scenario: builds the Simulation from its starting state and the
/// Packs, steps it at the physics rate up to the last moment the Scenario
/// mentions, and measures every Expectation on the way.
pub fn run(scenario: &Scenario, packs: &Packs) -> Result<Outcome, Problems> {
    let start = &scenario.start;
    let mut problems = Problems::new();
    let named = |what: &str, (id, line): &(String, usize), found: Problems| {
        let mut all = Problems(vec![Problem {
            file: scenario.file.clone(),
            line: *line,
            sentence: format!("can't use the {what} \"{id}\":"),
        }]);
        all.extend(found);
        all
    };
    let quad = packs
        .quad(&start.quad.0)
        .map_err(|found| problems.extend(named("Quad", &start.quad, found)))
        .ok();
    let map = packs
        .map(&start.map.0)
        .map_err(|found| problems.extend(named("Map", &start.map, found)))
        .ok();
    let (Some(quad), Some(map)) = (quad, map) else {
        return Err(problems);
    };

    let set_up = SetUp {
        physics_rate: start.physics_rate,
        world: map.world,
        random_seed: start.random_seed,
        quads: vec![QuadSetUp {
            parameters: quad.parameters,
            start: QuadState {
                position: start.position,
                velocity: start.velocity,
                attitude: start.attitude,
                rotation: start.rotation,
            },
            flight_controller: Box::new(ScriptedMotors::new(scenario.motor_script.clone())),
        }],
    };
    let mut sim = Simulation::new(set_up).map_err(|error| {
        let sentence = match error.problem {
            SetUpProblem::MassNotAboveZero => "its mass must be above zero",
            SetUpProblem::InertiaHasNoInverse => "its inertia can't be turned around (no inverse)",
        };
        Problems(vec![Problem {
            file: scenario.file.clone(),
            line: start.quad.1,
            sentence: format!("the Quad \"{}\" can't fly: {sentence}", start.quad.0),
        }])
    })?;

    let step = start.physics_rate.step_length();
    let mut tallies: Vec<Tally> = scenario.expectations.iter().map(Tally::new).collect();
    let mut step_fingerprints = Vec::with_capacity(scenario.length.ticks() as usize + 1);
    let mut first_broken_number = None;
    let mut before: Option<QuadState> = None;
    for tick in 0..=scenario.length.ticks() {
        if tick > 0 {
            sim.step();
        }
        let now = *sim.quad_state(0);
        step_fingerprints.push(sim.fingerprint());
        if first_broken_number.is_none() && !sim.is_finite() {
            first_broken_number = Some(format!(
                "first at {} s (step {tick}): the Quad's state holds a number that isn't a real number",
                sim.time().seconds(start.physics_rate)
            ));
        }
        for tally in &mut tallies {
            tally.see(tick, before.as_ref(), &now, step);
        }
        before = Some(now);
    }

    Ok(Outcome {
        expectations: tallies.into_iter().map(Tally::finish).collect(),
        first_broken_number,
        step_fingerprints,
        quad: (quad.id.clone(), quad.fingerprint()),
        map: (map.id.clone(), map.fingerprint()),
        physics_rate: start.physics_rate.hz(),
    })
}

/// One Expectation's measurement, built up step by step.
struct Tally<'s> {
    expectation: &'s Expectation,
    count: u64,
    sum: f64,
    lowest: f64,
    highest: f64,
    last: Option<f64>,
}

impl<'s> Tally<'s> {
    fn new(expectation: &'s Expectation) -> Tally<'s> {
        Tally {
            expectation,
            count: 0,
            sum: 0.0,
            lowest: f64::INFINITY,
            highest: f64::NEG_INFINITY,
            last: None,
        }
    }

    fn see(&mut self, tick: u64, before: Option<&QuadState>, now: &QuadState, step: f64) {
        let wanted = match self.expectation.when {
            When::At(at) => tick == at,
            When::Over { from, to, .. } => from < tick && tick <= to,
        };
        if !wanted {
            return;
        }
        let Some(value) = self.expectation.measure.read(before, now, step) else {
            return;
        };
        self.count += 1;
        self.sum += value;
        // Written out, not with f64::min and f64::max, which may pick either
        // zero when given +0 and -0.
        if value < self.lowest {
            self.lowest = value;
        }
        if value > self.highest {
            self.highest = value;
        }
        self.last = Some(value);
    }

    fn finish(self) -> Measured {
        let e = self.expectation;
        let value = match e.when {
            When::At(_) => self.last,
            When::Over { statistic, .. } if self.count > 0 => Some(match statistic {
                Statistic::Mean => self.sum / self.count as f64,
                Statistic::Lowest => self.lowest,
                Statistic::Highest => self.highest,
                Statistic::Final => self.last.unwrap_or(f64::NAN),
            }),
            When::Over { .. } => None,
        };
        let value = value.map(|value| {
            if e.measure.is_an_angle() {
                angle_near(value, e.expected.centre())
            } else {
                value
            }
        });
        let (measured, passed) = match value {
            Some(value) => (
                e.expected.unit().write(value),
                value.is_finite() && e.expected.accepts(value),
            ),
            None => ("nothing measured".to_string(), false),
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
