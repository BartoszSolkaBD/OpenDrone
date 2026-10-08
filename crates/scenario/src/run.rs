//! Running a Scenario through the Simulation and measuring its Expectations.

use opendrone_maths::{Fingerprint, Fingerprinter, functions};
use opendrone_pack::{MapDefinition, QuadDefinition};
use opendrone_pack::{Packs, Problem, Problems};
use opendrone_sim::{
    MapShapeProblem, MotorCommands, Mount, PhysicsRate, QuadOutput, QuadSetUp, QuadState,
    ScriptedMotors, SetUp, SetUpError, SetUpProblem, Simulation, SimulationTime,
};

use crate::measure::angle_near;
use crate::read::{
    BasisKind, Comparison, Expectation, Kind, Named, Scenario, Start, Statistic, When,
};

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

    // This run, measuring every Expectation.
    let mut tallies: Vec<Tally> = scenario
        .expectations
        .iter()
        .map(|e| Tally::new(e, e.when))
        .collect();
    let this_run = Plan {
        physics_rate: start.physics_rate,
        battery: start.battery,
        timeline: &scenario.timeline,
        length: scenario.length,
    };
    let (step_fingerprints, first_broken_number) =
        simulate(scenario, &quad, &map, &this_run, &mut tallies)?;

    // Each other run, measuring the Expectations that compare with it.
    let mut others: Vec<Option<Result<f64, String>>> = vec![None; tallies.len()];
    for (index, other) in scenario.other_runs.iter().enumerate() {
        let (numbers, mut other_tallies): (Vec<usize>, Vec<Tally>) = scenario
            .expectations
            .iter()
            .enumerate()
            .filter_map(|(n, e)| {
                let compared = e.compared.filter(|c| c.run == index)?;
                Some((n, Tally::new(e, compared.when)))
            })
            .unzip();
        let plan = Plan {
            physics_rate: other.physics_rate,
            battery: other.battery.unwrap_or(start.battery),
            timeline: &other.timeline,
            length: other.length,
        };
        simulate(scenario, &quad, &map, &plan, &mut other_tallies)?;
        for (n, tally) in numbers.into_iter().zip(&other_tallies) {
            others[n] = Some(tally.value());
        }
    }

    Ok(Outcome {
        expectations: tallies
            .into_iter()
            .zip(others)
            .map(|(tally, other)| tally.finish(other))
            .collect(),
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

/// One run of a Scenario: its physics rate, battery and Timeline.
struct Plan<'s> {
    physics_rate: PhysicsRate,
    battery: f64,
    timeline: &'s [(SimulationTime, MotorCommands)],
    length: SimulationTime,
}

/// Builds the Simulation for one run and steps it to the run's end, showing
/// every tally the output after each step. Gives back the whole state's
/// fingerprint after every step, and where a broken number first appeared.
fn simulate(
    scenario: &Scenario,
    quad: &QuadDefinition,
    map: &MapDefinition,
    plan: &Plan<'_>,
    tallies: &mut [Tally<'_>],
) -> Result<(Vec<Fingerprint>, Option<String>), Problems> {
    let start = &scenario.start;
    let set_up = SetUp {
        physics_rate: plan.physics_rate,
        world: map.world,
        map: map.shapes.clone(),
        random_seed: start.random_seed,
        quads: vec![QuadSetUp {
            parameters: quad.parameters.clone(),
            start: QuadState {
                position: start.position,
                velocity: start.velocity,
                attitude: start.attitude,
                rotation: start.rotation,
            },
            motors: start.motors,
            battery: plan.battery,
            mount: mount(start),
            flight_controller: Box::new(ScriptedMotors::new(plan.timeline.to_vec())),
        }],
    };
    let mut sim = Simulation::new(set_up).map_err(|error| {
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
                    SetUpProblem::ShapeCantBeBuilt => {
                        "its collision shape needs every size above zero, a bounce from 0 to 1 and a friction of 0 or more".to_string()
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
                        "a convex shape needs four corners that aren't all in one plane"
                    }
                    MapShapeProblem::MeshHasNoTriangles => "a triangle mesh needs a triangle",
                    MapShapeProblem::MeshCornerMissing => {
                        "a triangle names a corner the mesh doesn't have"
                    }
                };
                (
                    start.map.line,
                    format!(
                        "the Map \"{}\" can't be flown: its shape {shape} (counting from 0) isn't solid: {why}",
                        start.map.id
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

    let step = plan.physics_rate.step_length();
    let mut step_fingerprints = Vec::with_capacity(plan.length.ticks() as usize + 1);
    let mut first_broken_number = None;
    let mut before: Option<QuadOutput> = None;
    for tick in 0..=plan.length.ticks() {
        if tick > 0 {
            sim.step();
        }
        let now = sim.quad_output(0);
        step_fingerprints.push(sim.fingerprint());
        if first_broken_number.is_none() && !sim.is_finite() {
            first_broken_number = Some(format!(
                "first at {} s (step {tick}): the Quad's state holds a number that isn't a real number",
                sim.time().seconds(plan.physics_rate)
            ));
        }
        for tally in tallies.iter_mut() {
            tally.see(tick, before.as_ref(), &now, step);
        }
        before = Some(now);
    }
    Ok((step_fingerprints, first_broken_number))
}

/// A Thrust Stand Scenario holds the Quad still; every other kind lets it
/// fly.
fn mount(start: &Start) -> Mount {
    match start.kind {
        Kind::ThrustStand => Mount::ThrustStand,
        Kind::Flight | Kind::FlightController | Kind::Physics => Mount::Free,
    }
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
}

impl<'s> Tally<'s> {
    fn new(expectation: &'s Expectation, when: When) -> Tally<'s> {
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
        }
    }

    fn see(&mut self, tick: u64, before: Option<&QuadOutput>, now: &QuadOutput, step: f64) {
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
        if self.expectation.measure.is_an_angle() {
            let centre = self.expectation.expected.centre();
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
            When::Over { statistic, .. } if self.count > 0 => Some(match statistic {
                Statistic::Mean => self.sum / self.count as f64,
                Statistic::Lowest => self.lowest,
                Statistic::Highest => self.highest,
                Statistic::Final => self.last.unwrap_or(f64::NAN),
            }),
            When::Over { .. } => None,
        };
        value.ok_or_else(|| "nothing measured".to_string())
    }

    /// The Expectation's measured value and verdict. `other` is the other
    /// run's value, for an Expectation that compares with one.
    fn finish(self, other: Option<Result<f64, String>>) -> Measured {
        let e = self.expectation;
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
                    e.expected.unit().write(that)
                )),
            },
        };
        let (measured, passed) = match value {
            Ok(value) => (
                e.expected.unit().write(value),
                value.is_finite() && e.expected.accepts(value),
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
