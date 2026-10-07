//! Running a Scenario through the Simulation and measuring its Expectations.

use opendrone_maths::{Fingerprint, Fingerprinter, functions};
use opendrone_pack::{Packs, Problem, Problems};
use opendrone_sim::{QuadSetUp, QuadState, ScriptedMotors, SetUp, SetUpProblem, Simulation};

use crate::measure::angle_near;
use crate::read::{BasisKind, Expectation, Named, Scenario, Statistic, When};

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
    pub quad: Received,
    pub map: Received,
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

/// Runs a Scenario: builds the Simulation from its starting state and the
/// Packs, steps it at the physics rate up to the last moment the Scenario
/// mentions, and measures every Expectation on the way.
pub fn run(scenario: &Scenario, packs: &Packs) -> Result<Outcome, Problems> {
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
    let map = packs
        .map(&start.map.id)
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
            flight_controller: Box::new(ScriptedMotors::new(scenario.timeline.clone())),
        }],
    };
    let mut sim = Simulation::new(set_up).map_err(|error| {
        let sentence = match error.problem {
            SetUpProblem::MassNotAboveZero => "its mass must be above zero",
            SetUpProblem::InertiaHasNoInverse => "its inertia can't be turned around (no inverse)",
        };
        Problems(vec![Problem {
            file: scenario.file.clone(),
            line: start.quad.line,
            sentence: format!("the Quad \"{}\" can't fly: {sentence}", start.quad.id),
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
        quad: Received {
            id: quad.id.clone(),
            fingerprint: quad.fingerprint(),
        },
        map: Received {
            id: map.id.clone(),
            fingerprint: map.fingerprint(),
        },
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
    /// True once an angle jumped between two steps during the stretch (see
    /// `see`).
    jumped: bool,
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
            jumped: false,
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
        let Some(mut value) = self.expectation.measure.read(before, now, step) else {
            return;
        };
        // Every angle is taken the short way round from the expected value
        // before it counts, so a heading that crosses north (359.5° to 1.5°)
        // or a roll that crosses upside down (179.5° to -178.5°) has the
        // right lowest, highest, mean and final value.
        //
        // That only works while the angle moves smoothly near the expected
        // value. Two things break it, and both show as a jump between two
        // steps:
        //
        // - The angle sweeps past the side opposite the expected value, as in
        //   a flip or a pirouette: the taken-round values jump by a whole
        //   turn.
        // - The nose passes straight up or down: roll and heading jump by
        //   half a turn, because pitch only reads from -90° to 90°.
        //
        // Either way the lowest, highest and mean would come out near
        // whatever was expected, so those three then fail. A jump of a
        // quarter turn or more counts: a real turn that fast in one step
        // would be 720,000 °/s at 8 kHz.
        if self.expectation.measure.is_an_angle() {
            value = angle_near(value, self.expectation.expected.centre());
            if let Some(last) = self.last
                && (value - last).abs() >= core::f64::consts::FRAC_PI_2
            {
                self.jumped = true;
            }
        }
        self.count += 1;
        self.sum += value;
        // Not std's f64::min and f64::max, which may pick either zero when
        // given +0 and -0.
        self.lowest = functions::min(self.lowest, value);
        self.highest = functions::max(self.highest, value);
        self.last = Some(value);
    }

    fn finish(self) -> Measured {
        let e = self.expectation;
        if let When::Over { statistic, .. } = e.when
            && statistic != Statistic::Final
            && self.jumped
        {
            return Measured {
                description: e.description.clone(),
                basis: e.basis.kind,
                expected: e.expected.text(),
                measured: format!(
                    "none: the {} jumped, or went more than half a turn from the expected value, during the stretch, as it does in flips and when the nose passes straight up or down, so its {} has no single answer; check it at moments, over a shorter stretch, or check its rate",
                    e.measure.name(),
                    statistic.word()
                ),
                passed: false,
                line: e.line,
            };
        }
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
