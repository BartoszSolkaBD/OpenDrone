//! What an Expectation can measure: the words a Scenario's `what` may use.

use opendrone_maths::{PilotRates, functions};
use opendrone_pack::units::Dimension;
use opendrone_sim::QuadOutput;

/// One measurable quantity of the Quad, read from its output after a step.
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
    /// East is positive.
    SpeedEast,
    /// North is positive.
    SpeedNorth,
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
    /// A motor's speed, positive the normal way. Motors count from 0 here
    /// and from 1 in Scenarios, in Betaflight's order.
    MotorSpeed(usize),
    /// A motor's thrust along the Quad's up axis.
    MotorThrust(usize),
    /// The air's drag torque on a motor's prop.
    MotorTorque(usize),
    /// The current through a motor.
    MotorCurrent(usize),
    /// The share of the battery's voltage a motor's ESC puts across it.
    MotorDrive(usize),
    /// All four motors' thrust.
    TotalThrust,
    /// At the battery's terminals, past its connector.
    BatteryVoltage,
    /// Drawn from the battery.
    BatteryCurrent,
    /// Drawn from the battery since the start.
    BatteryChargeUsed,
    /// How far the battery's voltage sits below its resting voltage at its
    /// charge.
    BatterySag,
}

const ALL: [(&str, Measure); 40] = [
    ("height", Measure::Height),
    ("distance east", Measure::DistanceEast),
    ("distance north", Measure::DistanceNorth),
    ("vertical speed", Measure::VerticalSpeed),
    ("speed east", Measure::SpeedEast),
    ("speed north", Measure::SpeedNorth),
    ("horizontal speed", Measure::HorizontalSpeed),
    ("speed", Measure::Speed),
    ("vertical acceleration", Measure::VerticalAcceleration),
    ("roll rate", Measure::RollRate),
    ("pitch rate", Measure::PitchRate),
    ("yaw rate", Measure::YawRate),
    ("roll", Measure::Roll),
    ("pitch", Measure::Pitch),
    ("heading", Measure::Heading),
    ("motor 1 speed", Measure::MotorSpeed(0)),
    ("motor 2 speed", Measure::MotorSpeed(1)),
    ("motor 3 speed", Measure::MotorSpeed(2)),
    ("motor 4 speed", Measure::MotorSpeed(3)),
    ("motor 1 thrust", Measure::MotorThrust(0)),
    ("motor 2 thrust", Measure::MotorThrust(1)),
    ("motor 3 thrust", Measure::MotorThrust(2)),
    ("motor 4 thrust", Measure::MotorThrust(3)),
    ("motor 1 torque", Measure::MotorTorque(0)),
    ("motor 2 torque", Measure::MotorTorque(1)),
    ("motor 3 torque", Measure::MotorTorque(2)),
    ("motor 4 torque", Measure::MotorTorque(3)),
    ("motor 1 current", Measure::MotorCurrent(0)),
    ("motor 2 current", Measure::MotorCurrent(1)),
    ("motor 3 current", Measure::MotorCurrent(2)),
    ("motor 4 current", Measure::MotorCurrent(3)),
    ("motor 1 drive", Measure::MotorDrive(0)),
    ("motor 2 drive", Measure::MotorDrive(1)),
    ("motor 3 drive", Measure::MotorDrive(2)),
    ("motor 4 drive", Measure::MotorDrive(3)),
    ("total thrust", Measure::TotalThrust),
    ("battery voltage", Measure::BatteryVoltage),
    ("battery current", Measure::BatteryCurrent),
    ("battery charge used", Measure::BatteryChargeUsed),
    ("battery sag", Measure::BatterySag),
];

impl Measure {
    /// The measure a Scenario's `what` names.
    pub fn named(name: &str) -> Option<Measure> {
        ALL.iter().find(|(n, _)| *n == name).map(|(_, m)| *m)
    }

    /// Every name the runner can measure, with the four motors' measures
    /// written once each, as "motor N speed".
    pub fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = ALL
            .iter()
            .filter(|(name, _)| !name.starts_with("motor "))
            .map(|(name, _)| *name)
            .collect();
        let at = names.iter().position(|n| *n == "total thrust").unwrap_or(0);
        for (k, motor) in [
            "motor N speed (N from 1 to 4, in Betaflight's motor order)",
            "motor N thrust",
            "motor N torque",
            "motor N current",
            "motor N drive",
        ]
        .into_iter()
        .enumerate()
        {
            names.insert(at + k, motor);
        }
        names
    }

    pub fn name(self) -> &'static str {
        ALL.iter()
            .find(|(_, m)| *m == self)
            .map_or("", |(name, _)| name)
    }

    pub fn dimension(self) -> Dimension {
        match self {
            Measure::Height | Measure::DistanceEast | Measure::DistanceNorth => Dimension::LENGTH,
            Measure::VerticalSpeed
            | Measure::SpeedEast
            | Measure::SpeedNorth
            | Measure::HorizontalSpeed
            | Measure::Speed => Dimension::SPEED,
            Measure::VerticalAcceleration => Dimension::ACCELERATION,
            Measure::RollRate | Measure::PitchRate | Measure::YawRate => Dimension::ROTATION_SPEED,
            Measure::Roll | Measure::Pitch | Measure::Heading => Dimension::ANGLE,
            Measure::MotorSpeed(_) => Dimension::ROTATION_SPEED,
            Measure::MotorThrust(_) | Measure::TotalThrust => Dimension::FORCE,
            Measure::MotorTorque(_) => Dimension::TORQUE,
            Measure::MotorCurrent(_) | Measure::BatteryCurrent => Dimension::CURRENT,
            Measure::MotorDrive(_) => Dimension::PERCENT,
            Measure::BatteryVoltage | Measure::BatterySag => Dimension::VOLTAGE,
            Measure::BatteryChargeUsed => Dimension::CHARGE,
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
    pub fn read_straight_up_or_down(self, now: &QuadOutput) -> bool {
        matches!(self, Measure::Roll | Measure::Heading)
            && now.state.attitude.is_straight_up_or_down()
    }

    /// The value after a step, in SI units, given the output before the step
    /// (`None` at the start) and the step's length in seconds.
    pub fn read(self, before: Option<&QuadOutput>, now: &QuadOutput, step: f64) -> Option<f64> {
        let state = &now.state;
        let v = state.velocity;
        let rates = || PilotRates::from_body(state.rotation);
        let angles = || state.attitude.pilot_angles();
        Some(match self {
            Measure::Height => state.position.z,
            Measure::DistanceEast => state.position.x,
            Measure::DistanceNorth => state.position.y,
            Measure::VerticalSpeed => v.z,
            Measure::SpeedEast => v.x,
            Measure::SpeedNorth => v.y,
            Measure::HorizontalSpeed => (v.x * v.x + v.y * v.y).sqrt(),
            Measure::Speed => v.length(),
            Measure::VerticalAcceleration => (v.z - before?.state.velocity.z) / step,
            Measure::RollRate => rates().roll,
            Measure::PitchRate => rates().pitch,
            Measure::YawRate => rates().yaw,
            Measure::Roll => angles().roll,
            Measure::Pitch => angles().pitch,
            Measure::Heading => angles().heading,
            Measure::MotorSpeed(k) => now.motors[k].speed,
            Measure::MotorThrust(k) => now.motors[k].thrust,
            Measure::MotorTorque(k) => now.motors[k].torque,
            Measure::MotorCurrent(k) => now.motors[k].current,
            Measure::MotorDrive(k) => now.motors[k].drive,
            Measure::TotalThrust => now.motors.iter().map(|m| m.thrust).sum(),
            Measure::BatteryVoltage => now.battery.voltage,
            Measure::BatteryCurrent => now.battery.current,
            Measure::BatteryChargeUsed => now.battery.charge_used,
            Measure::BatterySag => now.battery.sag,
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
