//! What an Expectation can measure: the words a Scenario's `what` may use.

use opendrone_maths::{DEGREE, PilotRates, functions};
use opendrone_pack::units::Dimension;
use opendrone_sim::{Channel, DebugRecord, QuadOutput, RadioLinkOutput};

/// What one step gave, to measure: the Quad's output (none where the Flight
/// Controller runs alone), what our Flight Controller's loop did (none
/// where the motors are scripted, and before the first loop), and what the
/// Radio Link did (none where the motors are scripted, and at the start).
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub quad: Option<QuadOutput>,
    pub flight_controller: Option<DebugRecord>,
    pub radio_link: Option<RadioLinkOutput>,
}

/// One of the four sticks, as a Channel the Radio Link carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stick {
    Roll,
    Pitch,
    Yaw,
    Throttle,
}

/// One of the three rotation axes, as pilots name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Roll,
    Pitch,
    Yaw,
}

/// One of the PID loop's terms, or their sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    P,
    I,
    D,
    Sum,
}

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
    /// The same for the speed east. East is positive.
    AccelerationEast,
    /// The same for the speed north. North is positive.
    AccelerationNorth,
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
    /// The rotation speed the Rates ask for: our Flight Controller's
    /// setpoint, in the pilot's directions (rolling right, pitching nose up,
    /// yawing nose right positive).
    Setpoint(Axis),
    /// One of the PID loop's terms on Betaflight's scale (1000 is the whole
    /// motor range), in the pilot's directions: a positive pitch term pushes
    /// the nose up.
    PidTerm(Axis, Term),
    /// The DShot value our Flight Controller sends a motor: 0 is "stop", 48
    /// to 2047 the throttle.
    MotorDshot(usize),
    /// The throttle our Flight Controller's mixer starts from, from 0% to
    /// 100%, before Airmode moves it: what Betaflight's Blackbox logs as the
    /// mixer's throttle.
    MixerThrottle,
    /// How many reports from the Input Device reached the Radio Link since
    /// its last frame: the fresh ones the frame carries, the newest winning
    /// (see `opendrone_sim::Frame::reports`). Read on each step a frame
    /// leaves.
    ReportsInTheFrame,
    /// How long before a frame left the newest fresh report it carries
    /// arrived. Read on each step a frame carrying a fresh report leaves.
    FrameAge,
    /// A stick as the last Radio Link frame carried it, in percent as a
    /// Timeline writes sticks: roll, pitch and yaw from −100% to 100%, the
    /// throttle from 0% to 100%, read through ELRS's µs (988 µs + 0.625 µs a
    /// step from step 172).
    Channel(Stick),
    /// Something that happens: measured as how long after a stretch's start
    /// it first does (the "when something happens" Expectation).
    Happens(Event),
}

/// Something that happens between two steps, for the "when something
/// happens" Expectation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Our Flight Controller arms.
    Arms,
    /// Our Flight Controller disarms.
    Disarms,
    /// One of Betaflight's arming-disabled flags is raised (`true`) or
    /// cleared (`false`): its place in `ArmingBlocks::NAMES`.
    Blocks { flag: usize, raised: bool },
    /// Failsafe's stage 2 starts: with DROP, the Quad disarms.
    FailsafeStarts,
    /// Failsafe ends: the link has been back long enough.
    FailsafeEnds,
    /// Every ESC has played its ready beep.
    EscsReady,
    /// The Radio Link counts the Flying Input Device as lost: unplugged, or
    /// silent too long.
    InputDeviceLost,
    /// The Radio Link counts the Flying Input Device as back.
    InputDeviceBack,
}

/// Every event's words, as a Scenario's `what` names it.
const EVENTS: [(&str, Event); 21] = [
    ("the Quad arms", Event::Arms),
    ("the Quad disarms", Event::Disarms),
    ("Failsafe's stage 2 starts", Event::FailsafeStarts),
    ("Failsafe ends", Event::FailsafeEnds),
    ("the ESCs are ready", Event::EscsReady),
    (
        "FAILSAFE blocks arming",
        Event::Blocks {
            flag: 0,
            raised: true,
        },
    ),
    (
        "FAILSAFE stops blocking arming",
        Event::Blocks {
            flag: 0,
            raised: false,
        },
    ),
    (
        "RXLOSS blocks arming",
        Event::Blocks {
            flag: 1,
            raised: true,
        },
    ),
    (
        "RXLOSS stops blocking arming",
        Event::Blocks {
            flag: 1,
            raised: false,
        },
    ),
    (
        "NOT_DISARMED blocks arming",
        Event::Blocks {
            flag: 2,
            raised: true,
        },
    ),
    (
        "NOT_DISARMED stops blocking arming",
        Event::Blocks {
            flag: 2,
            raised: false,
        },
    ),
    (
        "THROTTLE blocks arming",
        Event::Blocks {
            flag: 3,
            raised: true,
        },
    ),
    (
        "THROTTLE stops blocking arming",
        Event::Blocks {
            flag: 3,
            raised: false,
        },
    ),
    (
        "ANGLE blocks arming",
        Event::Blocks {
            flag: 4,
            raised: true,
        },
    ),
    (
        "ANGLE stops blocking arming",
        Event::Blocks {
            flag: 4,
            raised: false,
        },
    ),
    (
        "BOOTGRACE blocks arming",
        Event::Blocks {
            flag: 5,
            raised: true,
        },
    ),
    (
        "BOOTGRACE stops blocking arming",
        Event::Blocks {
            flag: 5,
            raised: false,
        },
    ),
    (
        "ARM_SWITCH blocks arming",
        Event::Blocks {
            flag: 6,
            raised: true,
        },
    ),
    (
        "ARM_SWITCH stops blocking arming",
        Event::Blocks {
            flag: 6,
            raised: false,
        },
    ),
    ("the Flying Input Device is lost", Event::InputDeviceLost),
    ("the Flying Input Device is back", Event::InputDeviceBack),
];

impl Event {
    /// Its words, such as "the Quad disarms".
    pub fn words(self) -> &'static str {
        EVENTS
            .iter()
            .find(|(_, e)| *e == self)
            .map_or("", |(words, _)| words)
    }

    /// True for what our Flight Controller does; false for the ESCs and the
    /// Radio Link.
    fn of_the_flight_controller(self) -> bool {
        !matches!(
            self,
            Event::EscsReady | Event::InputDeviceLost | Event::InputDeviceBack
        )
    }

    /// True for what the Radio Link does.
    fn of_the_radio_link(self) -> bool {
        matches!(self, Event::InputDeviceLost | Event::InputDeviceBack)
    }

    /// Whether it happened between the step before and this one: `None` when
    /// a step has nothing to tell it by, such as before the Flight
    /// Controller's first loop.
    pub fn happened(self, before: &Sample, now: &Sample) -> Option<bool> {
        if self == Event::EscsReady {
            let ready = |sample: &Sample| {
                sample.quad.map(|quad| {
                    quad.motors
                        .iter()
                        .all(|motor| !matches!(motor.esc, opendrone_sim::EscState::StartingUp(_)))
                })
            };
            return Some(!ready(before)? && ready(now)?);
        }
        if self.of_the_radio_link() {
            let lost = |sample: &Sample| sample.radio_link.map(|link| link.lost);
            let (was, is) = (lost(before)?, lost(now)?);
            return Some(match self {
                Event::InputDeviceLost => !was && is,
                _ => was && !is,
            });
        }
        let (before, now) = (before.flight_controller?, now.flight_controller?);
        Some(match self {
            Event::Arms => !before.armed && now.armed,
            Event::Disarms => before.armed && !now.armed,
            Event::FailsafeStarts => !before.failsafe.active && now.failsafe.active,
            Event::FailsafeEnds => before.failsafe.active && !now.failsafe.active,
            Event::Blocks { flag, raised } => {
                let was = before.arming_blocks.flags()[flag];
                let is = now.arming_blocks.flags()[flag];
                was != raised && is == raised
            }
            Event::EscsReady | Event::InputDeviceLost | Event::InputDeviceBack => false,
        })
    }
}

const ALL: [(&str, Measure); 68] = [
    ("height", Measure::Height),
    ("distance east", Measure::DistanceEast),
    ("distance north", Measure::DistanceNorth),
    ("vertical speed", Measure::VerticalSpeed),
    ("speed east", Measure::SpeedEast),
    ("speed north", Measure::SpeedNorth),
    ("horizontal speed", Measure::HorizontalSpeed),
    ("speed", Measure::Speed),
    ("vertical acceleration", Measure::VerticalAcceleration),
    ("acceleration east", Measure::AccelerationEast),
    ("acceleration north", Measure::AccelerationNorth),
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
    ("roll setpoint", Measure::Setpoint(Axis::Roll)),
    ("pitch setpoint", Measure::Setpoint(Axis::Pitch)),
    ("yaw setpoint", Measure::Setpoint(Axis::Yaw)),
    ("roll P term", Measure::PidTerm(Axis::Roll, Term::P)),
    ("roll I term", Measure::PidTerm(Axis::Roll, Term::I)),
    ("roll D term", Measure::PidTerm(Axis::Roll, Term::D)),
    ("roll PID sum", Measure::PidTerm(Axis::Roll, Term::Sum)),
    ("pitch P term", Measure::PidTerm(Axis::Pitch, Term::P)),
    ("pitch I term", Measure::PidTerm(Axis::Pitch, Term::I)),
    ("pitch D term", Measure::PidTerm(Axis::Pitch, Term::D)),
    ("pitch PID sum", Measure::PidTerm(Axis::Pitch, Term::Sum)),
    ("yaw P term", Measure::PidTerm(Axis::Yaw, Term::P)),
    ("yaw I term", Measure::PidTerm(Axis::Yaw, Term::I)),
    ("yaw D term", Measure::PidTerm(Axis::Yaw, Term::D)),
    ("yaw PID sum", Measure::PidTerm(Axis::Yaw, Term::Sum)),
    ("motor 1 DShot", Measure::MotorDshot(0)),
    ("motor 2 DShot", Measure::MotorDshot(1)),
    ("motor 3 DShot", Measure::MotorDshot(2)),
    ("motor 4 DShot", Measure::MotorDshot(3)),
    ("mixer throttle", Measure::MixerThrottle),
    ("reports in the frame", Measure::ReportsInTheFrame),
    ("frame age", Measure::FrameAge),
    ("roll channel", Measure::Channel(Stick::Roll)),
    ("pitch channel", Measure::Channel(Stick::Pitch)),
    ("yaw channel", Measure::Channel(Stick::Yaw)),
    ("throttle channel", Measure::Channel(Stick::Throttle)),
];

impl Measure {
    /// The measure a Scenario's `what` names: a quantity, or something that
    /// happens.
    pub fn named(name: &str) -> Option<Measure> {
        ALL.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, m)| *m)
            .or_else(|| {
                EVENTS
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, e)| Measure::Happens(*e))
            })
    }

    /// Every event the runner can see happen, as `what` names it.
    pub fn events() -> Vec<&'static str> {
        EVENTS.iter().map(|(words, _)| *words).collect()
    }

    /// The event, for something that happens.
    pub fn event(self) -> Option<Event> {
        match self {
            Measure::Happens(event) => Some(event),
            _ => None,
        }
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
        let at = names
            .iter()
            .position(|n| *n == "mixer throttle")
            .map_or(names.len(), |i| i + 1);
        names.insert(at, "motor N DShot");
        names
    }

    pub fn name(self) -> &'static str {
        if let Measure::Happens(event) = self {
            return event.words();
        }
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
            Measure::VerticalAcceleration
            | Measure::AccelerationEast
            | Measure::AccelerationNorth => Dimension::ACCELERATION,
            Measure::RollRate | Measure::PitchRate | Measure::YawRate => Dimension::ROTATION_SPEED,
            Measure::Roll | Measure::Pitch | Measure::Heading => Dimension::ANGLE,
            Measure::MotorSpeed(_) => Dimension::ROTATION_SPEED,
            Measure::MotorThrust(_) | Measure::TotalThrust => Dimension::FORCE,
            Measure::MotorTorque(_) => Dimension::TORQUE,
            Measure::MotorCurrent(_) | Measure::BatteryCurrent => Dimension::CURRENT,
            Measure::MotorDrive(_) => Dimension::PERCENT,
            Measure::BatteryVoltage | Measure::BatterySag => Dimension::VOLTAGE,
            Measure::BatteryChargeUsed => Dimension::CHARGE,
            Measure::Setpoint(_) => Dimension::ROTATION_SPEED,
            Measure::PidTerm(..) | Measure::MotorDshot(_) => Dimension::NONE,
            Measure::MixerThrottle => Dimension::PERCENT,
            Measure::ReportsInTheFrame => Dimension::NONE,
            Measure::FrameAge => Dimension::TIME,
            Measure::Channel(_) => Dimension::PERCENT,
            Measure::Happens(_) => Dimension::TIME,
        }
    }

    /// True for what our Flight Controller's loop does: the only things a
    /// Flight Controller Scenario measures, and nothing a Scenario with
    /// scripted motors can.
    pub fn of_the_flight_controller(self) -> bool {
        match self {
            Measure::Setpoint(_)
            | Measure::PidTerm(..)
            | Measure::MotorDshot(_)
            | Measure::MixerThrottle => true,
            Measure::Happens(event) => event.of_the_flight_controller(),
            _ => false,
        }
    }

    /// True for what the Radio Link does: measured in Flight and Flight
    /// Controller Scenarios, where the pilot's sticks reach a Flight
    /// Controller, and never where the motors are scripted.
    pub fn of_the_radio_link(self) -> bool {
        match self {
            Measure::ReportsInTheFrame | Measure::FrameAge | Measure::Channel(_) => true,
            Measure::Happens(event) => event.of_the_radio_link(),
            _ => false,
        }
    }

    /// True for angles, which are compared the short way round the circle.
    pub fn is_an_angle(self) -> bool {
        self.dimension() == Dimension::ANGLE
    }

    /// True when it compares a step with the one before.
    pub fn needs_a_step_before(self) -> bool {
        matches!(
            self,
            Measure::VerticalAcceleration | Measure::AccelerationEast | Measure::AccelerationNorth
        )
    }

    /// True when this is roll or heading and the nose is within about
    /// 0.00000006° of straight up or down, so roll reads 0° and heading
    /// carries the whole turn (see
    /// `opendrone_maths::Attitude::is_straight_up_or_down`).
    pub fn read_straight_up_or_down(self, now: &Sample) -> bool {
        matches!(self, Measure::Roll | Measure::Heading)
            && now
                .quad
                .is_some_and(|quad| quad.state.attitude.is_straight_up_or_down())
    }

    /// The value after a step, in SI units, given what the step before gave
    /// (`None` at the start) and the step's length in seconds. `None` when
    /// this step has nothing to measure it by.
    pub fn read(self, before: Option<&Sample>, now: &Sample, step: f64) -> Option<f64> {
        if let Measure::Happens(event) = self {
            // 1 when it happened over the step, 0 when it didn't.
            return event
                .happened(before?, now)
                .map(|happened| f64::from(u8::from(happened)));
        }
        if self.of_the_flight_controller() {
            return self.read_flight_controller(now.flight_controller.as_ref()?);
        }
        if self.of_the_radio_link() {
            let link = now.radio_link?;
            return Some(match self {
                Measure::ReportsInTheFrame => f64::from(link.frame?.reports),
                Measure::FrameAge => {
                    let frame = link.frame.filter(|frame| frame.reports > 0)?;
                    frame.age as f64 * step
                }
                Measure::Channel(stick) => {
                    let channels = link.last_frame?;
                    match stick {
                        Stick::Roll => stick_share(channels.roll),
                        Stick::Pitch => stick_share(channels.pitch),
                        Stick::Yaw => stick_share(channels.yaw),
                        Stick::Throttle => (elrs_micros(channels.throttle) - 988.0) / 1024.0,
                    }
                }
                _ => return None,
            });
        }
        let before = before.and_then(|b| b.quad.as_ref());
        let now = now.quad.as_ref()?;
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
            Measure::AccelerationEast => (v.x - before?.state.velocity.x) / step,
            Measure::AccelerationNorth => (v.y - before?.state.velocity.y) / step,
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
            Measure::Setpoint(_)
            | Measure::PidTerm(..)
            | Measure::MotorDshot(_)
            | Measure::MixerThrottle
            | Measure::ReportsInTheFrame
            | Measure::FrameAge
            | Measure::Channel(_)
            | Measure::Happens(_) => return None,
        })
    }

    fn read_flight_controller(self, record: &DebugRecord) -> Option<f64> {
        // Betaflight's axes turn into the pilot's: pitch and yaw turn round.
        let pilot = |axis: Axis, value: f64| match axis {
            Axis::Roll => value,
            Axis::Pitch | Axis::Yaw => -value,
        };
        let index = |axis: Axis| match axis {
            Axis::Roll => 0,
            Axis::Pitch => 1,
            Axis::Yaw => 2,
        };
        Some(match self {
            Measure::Setpoint(axis) => pilot(axis, record.setpoint[index(axis)]) * DEGREE,
            Measure::PidTerm(axis, term) => {
                let terms = record.terms[index(axis)];
                pilot(
                    axis,
                    match term {
                        Term::P => terms.p,
                        Term::I => terms.i,
                        Term::D => terms.d,
                        Term::Sum => terms.sum,
                    },
                )
            }
            Measure::MotorDshot(k) => f64::from(record.motors[k].dshot),
            Measure::MixerThrottle => record.throttle,
            _ => return None,
        })
    }
}

/// A Channel in µs as ELRS means it: 988 µs at step 172 (−100%), 1500 µs at
/// 992 (centre, 1500.31 µs to be exact) and 2012 µs at 1811 (+100%).
fn elrs_micros(channel: Channel) -> f64 {
    988.0 + (f64::from(channel.step()) - 172.0) * 1024.0 / 1639.0
}

/// A roll, pitch or yaw Channel as a stick from −1 to +1.
fn stick_share(channel: Channel) -> f64 {
    (elrs_micros(channel) - 1500.0) / 512.0
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
