//! What an Expectation can measure: the words a Scenario's `what` may use.

use opendrone_maths::{PilotRates, functions};
use opendrone_pack::units::Dimension;
use opendrone_sim::QuadState;

/// One measurable quantity of the Quad, read from its state after a step.
///
/// The measurements use only `opendrone-maths` for anything beyond `+ - * /`
/// and square roots, so the measured values are the same on every computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Up from the Map's origin.
    Height,
    DistanceEast,
    DistanceNorth,
    /// Up is positive.
    VerticalSpeed,
    /// Over the ground, in any direction.
    HorizontalSpeed,
    /// In any direction.
    Speed,
    /// How much the vertical speed changed over the last step, divided by the
    /// step's length. Up is positive.
    VerticalAcceleration,
    /// Rolling right is positive.
    RollRate,
    /// Nose up is positive.
    PitchRate,
    /// Nose right is positive.
    YawRate,
    /// Right side down is positive, from -180° to 180°.
    Roll,
    /// Nose up is positive, from -90° to 90°.
    Pitch,
    /// Where the nose points, as on a compass, from 0° to 360°. With the nose
    /// straight up or down, roll reads 0° and heading carries the whole turn
    /// (see `opendrone_maths::PilotAngles`).
    Heading,
}

const ALL: [(&str, Measure); 13] = [
    ("height", Measure::Height),
    ("distance east", Measure::DistanceEast),
    ("distance north", Measure::DistanceNorth),
    ("vertical speed", Measure::VerticalSpeed),
    ("horizontal speed", Measure::HorizontalSpeed),
    ("speed", Measure::Speed),
    ("vertical acceleration", Measure::VerticalAcceleration),
    ("roll rate", Measure::RollRate),
    ("pitch rate", Measure::PitchRate),
    ("yaw rate", Measure::YawRate),
    ("roll", Measure::Roll),
    ("pitch", Measure::Pitch),
    ("heading", Measure::Heading),
];

impl Measure {
    /// The measure a Scenario's `what` names.
    pub fn named(name: &str) -> Option<Measure> {
        ALL.iter().find(|(n, _)| *n == name).map(|(_, m)| *m)
    }

    /// Every name the runner can measure.
    pub fn names() -> Vec<&'static str> {
        ALL.iter().map(|(name, _)| *name).collect()
    }

    pub fn name(self) -> &'static str {
        ALL.iter()
            .find(|(_, m)| *m == self)
            .map_or("", |(name, _)| name)
    }

    pub fn dimension(self) -> Dimension {
        match self {
            Measure::Height | Measure::DistanceEast | Measure::DistanceNorth => Dimension::LENGTH,
            Measure::VerticalSpeed | Measure::HorizontalSpeed | Measure::Speed => Dimension::SPEED,
            Measure::VerticalAcceleration => Dimension::ACCELERATION,
            Measure::RollRate | Measure::PitchRate | Measure::YawRate => Dimension::ROTATION_SPEED,
            Measure::Roll | Measure::Pitch | Measure::Heading => Dimension::ANGLE,
        }
    }

    /// True for angles, which are compared the short way round the circle.
    pub fn is_an_angle(self) -> bool {
        self.dimension() == Dimension::ANGLE
    }

    /// True when it compares a step with the one before.
    pub fn needs_a_step_before(self) -> bool {
        self == Measure::VerticalAcceleration
    }

    /// True when this is roll or heading and the nose is within about
    /// 0.00000006° of straight up or down, so roll reads 0° and heading
    /// carries the whole turn (see
    /// `opendrone_maths::Attitude::is_straight_up_or_down`).
    pub fn read_straight_up_or_down(self, now: &QuadState) -> bool {
        matches!(self, Measure::Roll | Measure::Heading) && now.attitude.is_straight_up_or_down()
    }

    /// The value after a step, in SI units, given the state before the step
    /// (`None` at the start) and the step's length in seconds.
    pub fn read(self, before: Option<&QuadState>, now: &QuadState, step: f64) -> Option<f64> {
        let v = now.velocity;
        let rates = || PilotRates::from_body(now.rotation);
        let angles = || now.attitude.pilot_angles();
        Some(match self {
            Measure::Height => now.position.z,
            Measure::DistanceEast => now.position.x,
            Measure::DistanceNorth => now.position.y,
            Measure::VerticalSpeed => v.z,
            Measure::HorizontalSpeed => (v.x * v.x + v.y * v.y).sqrt(),
            Measure::Speed => v.length(),
            Measure::VerticalAcceleration => (v.z - before?.velocity.z) / step,
            Measure::RollRate => rates().roll,
            Measure::PitchRate => rates().pitch,
            Measure::YawRate => rates().yaw,
            Measure::Roll => angles().roll,
            Measure::Pitch => angles().pitch,
            Measure::Heading => angles().heading,
        })
    }
}

/// `angle` (radians) moved by whole turns to lie within half a turn of
/// `near`, so 359.9° and 0.1° count as 0.2° apart.
pub fn angle_near(angle: f64, near: f64) -> f64 {
    let turn = 2.0 * core::f64::consts::PI;
    let mut difference = functions::fmod(angle - near, turn);
    if difference > turn / 2.0 {
        difference -= turn;
    } else if difference < -turn / 2.0 {
        difference += turn;
    }
    near + difference
}
