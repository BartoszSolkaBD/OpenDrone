//! Our Betaflight-style Flight Controller, plus the Betaflight CLI translator
//! (`diff all` import, Rates paste, `aux` paste), which takes text in and
//! opens no files, and Betaflight's OSD logic, which runs beside the
//! Simulation ([ADR-0022]).
//!
//! It uses only `opendrone-maths`, and never the physics: the two meet only
//! inside `opendrone-sim`.
//!
//! # What it is so far
//!
//! It copies Betaflight 2026.6 ([ADR-0008]), reimplemented from the
//! behaviour of Betaflight 2026.6.2's source and never copied from it (the
//! Betaflight research §8.4). So far it has:
//!
//! - the pilot's [`Rates`] in all five types, with the rate limit
//!   ([`rates`]);
//! - the [`Tune`]'s settings, read from its `set` lines ([`tune`]);
//! - what Betaflight makes of each Radio Link frame: the [`Channels`] in µs,
//!   the stick commands with the deadband, and the throttle through its
//!   curve;
//! - a built-in Modes table with Arm on AUX1, and the basic arming checks
//!   (throttle low, tilt within `small_angle`, the Arm switch going off
//!   before it can arm again) ([ADR-0017]);
//! - the PID loop with Betaflight's scaling, I limit and PID-sum limits;
//! - the Legacy mixer with Airmode always on, motor idle and the motor output
//!   limit;
//! - the Betaflight CLI translator ([`cli`]): a quad's `diff all` imported as
//!   a 2026.6 Tune, and the pilot's pasted Rates and `aux` switches.
//!
//! The filters, RC smoothing and feedforward (#49), the rest of the loop
//! shaping (#50), Angle and Horizon (#51), the rest of arming, Failsafe and
//! power-up (#52), Crash Flip and yaw spin recovery (#54), the OSD paste and
//! the OSD logic (#59) arrive with their tickets.
//!
//! # One loop
//!
//! [`FlightController::step`] runs one loop, once per physics step:
//!
//! 1. If a Radio Link frame arrived, Betaflight's receiver work runs: Airmode
//!    starts once the throttle first passes `airmode_start_throttle_percent`
//!    after arming; the Arm switch arms or (after more than three frames
//!    off) disarms; the sticks become commands, and the Rates turn them into
//!    each axis's setpoint.
//! 2. The PID loop compares each setpoint with the gyro.
//! 3. The mixer turns the PID sums and the throttle into four motor
//!    commands.
//!
//! # Directions
//!
//! Inside, it works in Betaflight's own axes, in °/s: roll positive rolling
//! right, pitch positive nose down and yaw positive nose left. Those are the
//! body's rotation about its forward, left and up axes, so the gyro reading
//! in body axes (forward, left, up) is Betaflight's gyro as it stands. Its
//! [`DebugRecord`] gives setpoints and gyro both ways.
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
//! [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md
//! [ADR-0017]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md
//! [ADR-0022]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0022-osd-worked-out-beside-the-simulation.md

mod arming;
mod channels;
pub mod cli;
mod mixer;
mod pid;
pub mod rates;
mod receiver;
pub mod tune;

use opendrone_maths::{Attitude, DEGREE, Fingerprinter, PilotRates, Vec3};

use arming::Arming;
use pid::Pid;
use receiver::{RcCommand, RcData};

pub use arming::ArmingBlocks;
pub use channels::{Channel, Channels};
pub use mixer::{DSHOT_HIGHEST, DSHOT_LOWEST};
pub use pid::Terms;
pub use rates::{Axis, AxisRates, Rates, RatesType, ThrottleLimitType};
pub use tune::{Gains, MixerType, MotorProtocol, Tune, TuneProblems};

/// What a real board's sensors read, which is all the Flight Controller
/// knows of the Quad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensorReadings {
    /// The gyro: the rotation in body axes (forward, left, up), in radians
    /// per second.
    pub gyro: Vec3,
    /// The true attitude, as Angle and Horizon level against it and arming
    /// checks the tilt.
    pub attitude: Attitude,
}

/// Which way a motor is told to spin. Crash Flip (#54) spins them backwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinDirection {
    Normal,
    Reversed,
}

/// What the Flight Controller sends one motor's ESC: a DShot throttle value
/// (0 is "stop"; 48 to 2047 is the throttle) and the spin direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotorCommand {
    pub dshot: u16,
    pub direction: SpinDirection,
}

/// What one loop did, for the flight log, the OSD and Scenarios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugRecord {
    pub armed: bool,
    /// Why arming is refused right now (all clear while armed).
    pub arming_blocks: ArmingBlocks,
    /// Each axis's setpoint, in Betaflight's axes and °/s: roll right, pitch
    /// nose down, yaw nose left positive.
    pub setpoint: [f64; 3],
    /// The gyro, in Betaflight's axes and °/s.
    pub gyro: [f64; 3],
    /// Each axis's P, I, D and F terms and their sum, on Betaflight's scale
    /// (1000 is the whole motor range). I is the one the sum used.
    pub terms: [Terms; 3],
    /// The mixer's throttle, from 0 to 1.
    pub throttle: f64,
    /// The four motor commands, in Betaflight's motor order.
    pub motors: [MotorCommand; 4],
}

impl DebugRecord {
    /// The setpoints in the words pilots use, in °/s: rolling right, pitching
    /// nose up and yawing nose right are positive.
    pub fn pilot_setpoint(&self) -> PilotRates {
        PilotRates::from_body(Vec3::new(
            self.setpoint[0],
            self.setpoint[1],
            self.setpoint[2],
        ))
    }
}

/// Our Flight Controller for one Quad.
#[derive(Clone, Debug)]
pub struct FlightController {
    tune: Tune,
    rates: Rates,
    throttle_lookup: [i64; 12],
    /// The last frame's Channels, in µs, or `None` before the first.
    rc: Option<RcData>,
    command: Option<RcCommand>,
    setpoint: [f64; 3],
    arming: Arming,
    /// Airmode's start: the throttle passed its threshold since arming.
    throttle_raised: bool,
    /// Whether the PID loop works, and whether I is emptied each loop
    /// (`pidStabilisationState`, `pidSetItermReset`).
    stabilising: bool,
    reset_iterm: bool,
    pid: Pid,
    last: DebugRecord,
}

impl FlightController {
    /// A "fresh" Flight Controller, as right after power-up with Betaflight's
    /// own waits skipped: I empty, no frame yet, and the D term's memory
    /// starting from the current sensor readings. `loop_hz` is how many loops
    /// a second it runs, one per physics step. `armed` starts it armed, as a
    /// mid-air start does.
    pub fn new(
        tune: Tune,
        rates: Rates,
        loop_hz: u32,
        armed: bool,
        readings: &SensorReadings,
    ) -> FlightController {
        let gyro = degrees(readings.gyro);
        let pid = Pid::new(&tune, loop_hz, gyro);
        let throttle_lookup = receiver::throttle_lookup(&rates);
        let arming = Arming::new(armed);
        FlightController {
            last: DebugRecord {
                armed,
                arming_blocks: arming.blocks,
                setpoint: [0.0; 3],
                gyro,
                terms: [Terms::default(); 3],
                throttle: 0.0,
                motors: [stopped(); 4],
            },
            tune,
            rates,
            throttle_lookup,
            rc: None,
            command: None,
            setpoint: [0.0; 3],
            arming,
            throttle_raised: false,
            stabilising: false,
            reset_iterm: true,
            pid,
        }
    }

    pub fn tune(&self) -> &Tune {
        &self.tune
    }

    pub fn rates(&self) -> &Rates {
        &self.rates
    }

    pub fn armed(&self) -> bool {
        self.arming.armed
    }

    /// What the last loop did (at the start: nothing yet, motors stopped).
    pub fn debug(&self) -> &DebugRecord {
        &self.last
    }

    /// One loop: `frame` is the Radio Link frame that arrived since the last
    /// loop, if one did. Gives the four motor commands.
    pub fn step(
        &mut self,
        readings: &SensorReadings,
        frame: Option<&Channels>,
    ) -> [MotorCommand; 4] {
        if let Some(channels) = frame {
            self.receive(channels, readings);
        }
        let gyro = degrees(readings.gyro);
        self.pid
            .step(self.setpoint, gyro, self.stabilising, self.reset_iterm);
        let sums = self.pid.terms.map(|terms| terms.sum);
        let throttle_command = self.command.map_or(receiver::RANGE_MIN, |c| c.throttle);
        let mixed = mixer::mix(
            &self.tune,
            &self.rates,
            sums,
            throttle_command,
            self.arming.armed,
        );
        let motors = mixed.dshot.map(|dshot| MotorCommand {
            dshot,
            direction: SpinDirection::Normal,
        });
        self.last = DebugRecord {
            armed: self.arming.armed,
            arming_blocks: self.arming.blocks,
            setpoint: self.setpoint,
            gyro,
            terms: self.pid.terms,
            throttle: mixed.throttle,
            motors,
        };
        motors
    }

    /// Betaflight's work on a new frame, in its order: Airmode's start and
    /// the PID loop's state read the arming as it stood; then the Arm switch;
    /// then the stick commands and the setpoints.
    fn receive(&mut self, channels: &Channels, readings: &SensorReadings) {
        let rc = RcData::from_channels(channels);
        let throttle_active = !rc.throttle_low(&self.tune);
        let airmode_active = if self.arming.armed {
            if rc.throttle_percent(&self.tune)
                >= i64::from(self.tune.airmode_start_throttle_percent)
            {
                self.throttle_raised = true;
            }
            self.throttle_raised
        } else {
            self.throttle_raised = false;
            false
        };
        if self.arming.armed && (airmode_active || throttle_active) {
            self.reset_iterm = false;
            self.stabilising = true;
        } else {
            self.reset_iterm = true;
            self.stabilising = self.tune.pid_at_min_throttle;
        }

        self.arming.frame(&rc, &self.tune, readings.attitude);

        let command = RcCommand::new(&rc, &self.tune, &self.throttle_lookup);
        for axis in Axis::ALL {
            let deflection = command.deflection(&self.tune, axis);
            self.setpoint[axis.index()] = self.rates.setpoint(axis, deflection);
        }
        self.rc = Some(rc);
        self.command = Some(command);
    }

    /// Feeds everything it remembers into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.tune.write_fingerprint(f);
        self.rates.write_fingerprint(f);
        match self.rc {
            None => f.write_u64(0),
            Some(rc) => {
                f.write_u64(1);
                f.write_f64s(&[rc.roll, rc.pitch, rc.yaw, rc.throttle]);
                f.write_f64s(&rc.aux);
            }
        }
        if let Some(command) = self.command {
            f.write_f64s(&command.sticks);
            f.write_u64(command.throttle as u64);
        }
        f.write_f64s(&self.setpoint);
        self.arming.write_fingerprint(f);
        for flag in [self.throttle_raised, self.stabilising, self.reset_iterm] {
            f.write_u64(u64::from(flag));
        }
        self.pid.write_fingerprint(f);
        for motor in self.last.motors {
            f.write_u64(u64::from(motor.dshot));
        }
    }
}

/// A body rotation in rad/s as Betaflight's gyro in °/s.
fn degrees(rotation: Vec3) -> [f64; 3] {
    [
        rotation.x / DEGREE,
        rotation.y / DEGREE,
        rotation.z / DEGREE,
    ]
}

fn stopped() -> MotorCommand {
    MotorCommand {
        dshot: 0,
        direction: SpinDirection::Normal,
    }
}

/// Betaflight's `constrainf`: `low` below it, `high` above it, and never a
/// panic, whatever the numbers (std's `clamp` panics when `low` is above
/// `high` or either is not a number).
pub(crate) fn constrain(value: f64, low: f64, high: f64) -> f64 {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}
