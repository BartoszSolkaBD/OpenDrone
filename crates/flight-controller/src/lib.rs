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
//! - a built-in Modes table with Arm on AUX1, and Betaflight's arming checks
//!   (throttle low, tilt within `small_angle`, the ESCs' ready beep in place
//!   of Betaflight's boot grace, the Radio Link and Failsafe, and the Arm
//!   switch going off before it can arm again) ([ADR-0017], [`arming`]);
//! - the Radio Link's loss and Failsafe, flying DROP ([`failsafe`]);
//! - the PID loop with Betaflight's scaling, I limit and PID-sum limits;
//! - the Legacy mixer with Airmode always on, motor idle and the motor output
//!   limit;
//! - the Betaflight CLI translator ([`cli`]): a quad's `diff all` imported as
//!   a 2026.6 Tune, and the pilot's pasted Rates and `aux` switches.
//!
//! The filters, RC smoothing and feedforward (#49), the rest of the loop
//! shaping (#50), Angle and Horizon (#51), Crash Flip and yaw spin recovery
//! (#54), the OSD paste and the OSD logic (#59) arrive with their tickets.
//!
//! # Power-up
//!
//! [`FlightController::new`] powers it up "fresh", as Reset does: I empty,
//! the link settled, and Betaflight's own waits skipped (its 1.25 s gyro
//! calibration, its 5 s arming grace and its 5 s wait before Failsafe
//! watches the link). In their place, arming waits for the ESCs' ready beep
//! (`BOOTGRACE`, #32 §4).
//!
//! # One loop
//!
//! [`FlightController::step`] runs one loop, once per physics step, its
//! clock counting the loops since power-up:
//!
//! 1. Betaflight's receiver looks for a frame: one arrived, or none has for
//!    150 ms (then again every 50 ms), and the Channels are worked out
//!    again, held or set to Failsafe's values while the link is lost
//!    ([`failsafe`]).
//! 2. Whenever more than 10 ms have passed since the last time, Failsafe is
//!    checked: past `failsafe_delay` without good Channels, DROP disarms.
//! 3. If the Channels were worked out again, Betaflight's receiver work
//!    runs: Airmode starts once the throttle first passes
//!    `airmode_start_throttle_percent` after arming; the Arm switch arms or
//!    (after more than three frames off) disarms; the sticks become
//!    commands, and the Rates turn them into each axis's setpoint; the
//!    arming checks run.
//! 4. The PID loop compares each setpoint with the gyro.
//! 5. The mixer turns the PID sums and the throttle into four motor
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

pub mod arming;
mod channels;
pub mod cli;
pub mod failsafe;
mod mixer;
mod pid;
pub mod rates;
mod receiver;
pub mod tune;

use opendrone_maths::{Attitude, DEGREE, Fingerprinter, PilotRates, Vec3};

use arming::Arming;
use failsafe::{Failsafe, Receiver};
use pid::Pid;
use receiver::RcCommand;

pub use arming::ArmingBlocks;
pub use channels::{Channel, Channels};
pub use failsafe::FailsafePhase;
pub use mixer::{DSHOT_HIGHEST, DSHOT_LOWEST};
pub use pid::Terms;
pub use rates::{Axis, AxisRates, Rates, RatesType, ThrottleLimitType};
pub use tune::{FailsafeProcedure, Gains, MixerType, MotorProtocol, Tune, TuneProblems};

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
    /// Whether every ESC has played its ready beep and answers commands.
    /// Betaflight's own boot grace is a fixed 5 s; OpenDrone's Flight
    /// Controller holds `BOOTGRACE` until this instead, so arming waits for
    /// the ESCs' start-up after power-up, about 1.7 s (#32 §4). Running ESCs
    /// (a mid-air start) are ready.
    pub escs_ready: bool,
}

/// Failsafe as one loop left it, for the OSD and Scenarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailsafeReadings {
    /// Betaflight's Failsafe phase.
    pub phase: FailsafePhase,
    /// Stage 2 is on (`failsafeIsActive`): from the drop until Failsafe
    /// ends.
    pub active: bool,
    /// Frames are arriving (`isRxReceivingSignal`): false from 150 ms
    /// without one until the next arrives.
    pub signal: bool,
    /// Failsafe counts the link as up: false from `failsafe_delay` without
    /// good Channels until they have kept arriving for
    /// `failsafe_recovery_delay`.
    pub link_up: bool,
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
    /// Why arming is refused right now: Betaflight's arming-disabled flags
    /// as they stand (see [`ArmingBlocks`] for which change while armed).
    pub arming_blocks: ArmingBlocks,
    pub failsafe: FailsafeReadings,
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
    /// Loops a second, one per physics step.
    loop_hz: u32,
    /// Loops run since power-up: the Flight Controller's own clock.
    loops: u64,
    /// The receiver: whether frames arrive, and the Channels in use.
    receiver: Receiver,
    failsafe: Failsafe,
    /// The stick commands, or `None` before the Channels were first worked
    /// out.
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
    /// own waits skipped: I empty, no frame yet but the link settled, and the
    /// D term's memory starting from the current sensor readings. `loop_hz`
    /// is how many loops a second it runs, one per physics step. `armed`
    /// starts it armed, as a mid-air start does; disarmed, `BOOTGRACE` holds
    /// until the sensor readings say the ESCs are ready.
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
        let receiver = Receiver::at_power_up(&tune);
        let failsafe = Failsafe::at_power_up(&tune);
        FlightController {
            last: DebugRecord {
                armed,
                arming_blocks: arming.blocks,
                failsafe: failsafe_readings(&failsafe, &receiver),
                setpoint: [0.0; 3],
                gyro,
                terms: [Terms::default(); 3],
                throttle: 0.0,
                motors: [stopped(); 4],
            },
            tune,
            rates,
            throttle_lookup,
            loop_hz: loop_hz.max(1),
            loops: 0,
            receiver,
            failsafe,
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
        // The loop's moment, as Betaflight's `micros()` and `millis()` would
        // read it: whole µs and ms since power-up.
        let now_us = self.loops * 1_000_000 / u64::from(self.loop_hz);
        let now_ms = now_us / 1000;
        let changed = self.receiver.look(now_us, frame);
        self.failsafe.check(now_ms, &mut self.arming);
        if changed {
            self.receive(now_ms, readings);
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
            failsafe: failsafe_readings(&self.failsafe, &self.receiver),
            setpoint: self.setpoint,
            gyro,
            terms: self.pid.terms,
            throttle: mixed.throttle,
            motors,
        };
        self.loops += 1;
        motors
    }

    /// Betaflight's work on the Channels, in its order: they are worked out
    /// again, and Failsafe told whether they were good; Airmode's start and
    /// the PID loop's state read the arming as it stood; then the Arm switch;
    /// then the stick commands and the setpoints; then the arming checks.
    fn receive(&mut self, now_ms: u64, readings: &SensorReadings) {
        let good = self
            .receiver
            .apply(now_ms, &self.tune, self.failsafe.active);
        if good {
            self.failsafe.good_channels(now_ms, &mut self.arming.blocks);
        } else {
            self.failsafe.bad_channels(now_ms, &mut self.arming.blocks);
        }
        let rc = self.receiver.data;
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

        self.arming.frame(
            &rc,
            &self.tune,
            arming::Readings {
                attitude: readings.attitude,
                escs_ready: readings.escs_ready,
                signal: self.receiver.signal,
                link_up: self.failsafe.link_up,
            },
        );

        let command = RcCommand::new(&rc, &self.tune, &self.throttle_lookup);
        for axis in Axis::ALL {
            let deflection = command.deflection(&self.tune, axis);
            self.setpoint[axis.index()] = self.rates.setpoint(axis, deflection);
        }
        self.command = Some(command);
    }

    /// Feeds everything it remembers into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.tune.write_fingerprint(f);
        self.rates.write_fingerprint(f);
        f.write_u64(u64::from(self.loop_hz));
        f.write_u64(self.loops);
        self.receiver.write_fingerprint(f);
        self.failsafe.write_fingerprint(f);
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

fn failsafe_readings(failsafe: &Failsafe, receiver: &Receiver) -> FailsafeReadings {
    FailsafeReadings {
        phase: failsafe.phase,
        active: failsafe.active,
        signal: receiver.signal,
        link_up: failsafe.link_up,
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
