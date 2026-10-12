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
//! - a built-in Modes table with Arm on AUX1 and Crash Flip on AUX3, and
//!   Betaflight's arming checks (throttle low, tilt within `small_angle`
//!   unless the Crash Flip switch is on, the ESCs' ready beep in place of
//!   Betaflight's boot grace, the Radio Link and Failsafe, and the Arm switch
//!   going off before it can arm again) ([ADR-0017], [`arming`]);
//! - the Radio Link's loss and Failsafe, flying DROP ([`failsafe`]);
//! - the PID loop with Betaflight's scaling, I limit and PID-sum limits;
//! - the gyro's, the D term's and yaw P's low-pass filters, static or
//!   following the throttle (`gyro_filter`, `pid`, `filter`);
//! - Betaflight's detection of the Packet Rate from the frames' timing, and
//!   RC smoothing tuned to it (`rc_smoothing`);
//! - feedforward, with boost, averaging, jitter attenuation, the max-rate
//!   limit, transition and yaw hold (`feedforward`);
//! - the Legacy mixer with Airmode always on, motor idle and the motor output
//!   limit;
//! - Crash Flip, which spins the motors backwards straight from the sticks
//!   (#26 §3, [`arming`], `mixer::crash_flip`), and the DShot commands that
//!   tell the ESCs which way to spin on every arm and disarm
//!   ([`dshot_command`]);
//! - yaw spin recovery (#26 §4, `yaw_spin`) and runaway takeoff prevention
//!   (`runaway`);
//! - the Betaflight CLI translator ([`cli`]): a quad's `diff all` imported as
//!   a 2026.6 Tune, and the pilot's pasted Rates and `aux` switches.
//!
//! The rest of the loop shaping (#50), Angle and Horizon (#51), the OSD
//! paste and the OSD logic (#59) arrive with their tickets.
//!
//! # Power-up
//!
//! [`FlightController::new`] powers it up "fresh", as Reset does: I empty,
//! the link settled, and Betaflight's own waits skipped (its 1.25 s gyro
//! calibration, its 5 s arming grace and its 5 s wait before Failsafe
//! watches the link). In their place, arming waits for the ESCs' ready beep
//! (`BOOTGRACE`, #32 §4). Its filters are settled too: the detected Packet
//! Rate is the Radio Link's, with RC smoothing and feedforward tuned to it,
//! and each low-pass starts as if its first input had held since power-up.
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
//!    `airmode_start_throttle_percent` after arming; runaway takeoff
//!    prevention adds up stable flight; the Arm switch arms or (after more
//!    than three frames off) disarms, and the Crash Flip switch is read as
//!    it arms; the sticks become commands, and the Rates turn them into each
//!    axis's setpoint; the arming checks run, and the Crash Flip switch going
//!    off disarms.
//!    With the Channels, the frame's gap is timed for the detected Packet
//!    Rate.
//! 4. The gyro reading passes through its low-passes.
//! 5. If the Channels were worked out again, the detected Packet Rate moves
//!    on (retuning RC smoothing and feedforward every third frame that
//!    counts) and feedforward is worked out for the frame. Then RC smoothing
//!    takes a step on the setpoints, the throttle and feedforward.
//! 6. Yaw spin recovery watches the filtered gyro's yaw rate.
//! 7. The PID loop compares each smoothed setpoint with the filtered gyro;
//!    then runaway takeoff prevention checks the PID sums.
//! 8. The mixer turns the PID sums and the smoothed throttle into four motor
//!    commands, or in Crash Flip the sticks do; a spin direction command
//!    queued by arming or disarming goes out in their place
//!    ([`dshot_command`]). Outside Crash Flip, the mixer's throttle then
//!    moves the dynamic low-passes for the next loop.
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
pub mod dshot_command;
pub mod failsafe;
mod feedforward;
mod filter;
mod gyro_filter;
mod mixer;
mod pid;
pub mod rates;
mod rc_smoothing;
mod receiver;
mod runaway;
pub mod tune;
mod yaw_spin;

use opendrone_maths::{Attitude, DEGREE, Fingerprinter, PilotRates, Vec3};

use arming::Arming;
use dshot_command::DshotCommands;
use failsafe::{Failsafe, Receiver};
use feedforward::{Feedforward, FeedforwardFrame};
use gyro_filter::{DynamicThrottle, GyroFilters};
use mixer::CrashFlipInputs;
use pid::Pid;
use rc_smoothing::{PacketRate, RcSmoothing};
use receiver::RcCommand;
use runaway::Runaway;
use yaw_spin::YawSpin;

pub use arming::ArmingBlocks;
pub use channels::{Channel, Channels};
pub use failsafe::FailsafePhase;
pub use mixer::{DSHOT_HIGHEST, DSHOT_LOWEST};
pub use pid::Terms;
pub use rates::{Axis, AxisRates, Rates, RatesType, ThrottleLimitType};
pub use tune::{
    FailsafeProcedure, FeedforwardAveraging, Gains, LowpassType, MixerType, MotorProtocol, Tune,
    TuneProblems, YawSpinRecovery,
};

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

/// What the Flight Controller sends one motor's ESC: a DShot value (0 is
/// "stop", 1 to 47 are commands, which Bluejay takes as no throttle, and 48
/// to 2047 is the throttle) and the spin direction the ESC has taken
/// ([`dshot_command`]).
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
    /// nose down, yaw nose left positive. This is the newest frame's, before
    /// any smoothing (Betaflight's `rawSetpoint`).
    pub setpoint: [f64; 3],
    /// Each axis's setpoint as the PID loop uses it: after RC smoothing,
    /// when it's on (`getSetpointRate`). In the same axes and °/s.
    pub smoothed_setpoint: [f64; 3],
    /// The gyro as the sensor reads it, in Betaflight's axes and °/s
    /// (Blackbox's `gyroUnfilt`).
    pub gyro: [f64; 3],
    /// The gyro after its low-passes, which every part of the Flight
    /// Controller reads, in the same axes and °/s (Blackbox's `gyroADC`).
    pub filtered_gyro: [f64; 3],
    /// The Packet Rate the Flight Controller has detected from the frames'
    /// timing, in Hz (Betaflight's `rc_smoothing_info`, "Detected Rx
    /// frequency").
    pub detected_packet_rate: f64,
    /// RC smoothing's cutoffs in use, in whole Hz: the setpoints' and
    /// feedforward's, then the throttle's (`rc_smoothing_info`); `None` with
    /// RC smoothing off.
    pub rc_smoothing_cutoffs: Option<[u16; 2]>,
    /// Each axis's P, I, D and F terms and their sum, on Betaflight's scale
    /// (1000 is the whole motor range). I is the one the sum used.
    pub terms: [Terms; 3],
    /// The mixer's throttle, from 0 to 1, after RC smoothing.
    pub throttle: f64,
    /// The four motor commands, in Betaflight's motor order.
    pub motors: [MotorCommand; 4],
    /// In Crash Flip: armed with the Crash Flip switch on
    /// (`crashFlipModeActive`).
    pub crash_flip: bool,
    /// In yaw spin recovery (`yawSpinDetected`).
    pub yaw_spin_recovery: bool,
    /// The flight has counted as stable since power-up, so runaway takeoff
    /// prevention has switched itself off (`runawayTakeoffCheckDisabled`).
    pub flight_stable: bool,
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
    /// Each axis's setpoint from the newest frame, before smoothing.
    setpoint: [f64; 3],
    /// The detected Packet Rate, RC smoothing and feedforward.
    packet_rate: PacketRate,
    rc_smoothing: RcSmoothing,
    feedforward: Feedforward,
    /// The gyro's low-passes, and when the dynamic ones last moved.
    gyro_filters: GyroFilters,
    dynamic_throttle: DynamicThrottle,
    arming: Arming,
    /// Airmode's start: the throttle passed its threshold since arming.
    throttle_raised: bool,
    /// Whether the PID loop works, and whether I is emptied each loop
    /// (`pidStabilisationState`, `pidSetItermReset`).
    stabilising: bool,
    reset_iterm: bool,
    pid: Pid,
    yaw_spin: YawSpin,
    runaway: Runaway,
    /// The tilt's cosine when Crash Flip first drove the motors, for
    /// `crashflip_rate`'s fade; `None` outside Crash Flip.
    tilt_at_start: Option<f64>,
    dshot: DshotCommands,
    last: DebugRecord,
}

impl FlightController {
    /// A "fresh" Flight Controller, as right after power-up with Betaflight's
    /// own waits skipped: I empty, no frame yet but the link settled, and the
    /// D term's memory starting from the current sensor readings. `loop_hz`
    /// is how many loops a second it runs, one per physics step;
    /// `packet_rate_hz` is the Radio Link's Packet Rate, which it has
    /// detected and tuned RC smoothing and feedforward to. `armed` starts it
    /// armed, as a mid-air start does; disarmed, `BOOTGRACE` holds until the
    /// sensor readings say the ESCs are ready.
    pub fn new(
        tune: Tune,
        rates: Rates,
        loop_hz: u32,
        packet_rate_hz: u32,
        armed: bool,
        readings: &SensorReadings,
    ) -> FlightController {
        let gyro = degrees(readings.gyro);
        let pid = Pid::new(&tune, loop_hz, gyro);
        let packet_rate = PacketRate::settled(packet_rate_hz);
        let rc_smoothing = RcSmoothing::new(&tune, loop_hz, packet_rate.detected_hz);
        let feedforward = Feedforward::new(&tune, &rates, packet_rate.detected_hz);
        let gyro_filters = GyroFilters::new(&tune, loop_hz);
        let throttle_lookup = receiver::throttle_lookup(&rates);
        let arming = Arming::new(armed);
        let receiver = Receiver::at_power_up(&tune);
        let failsafe = Failsafe::at_power_up(&tune);
        let yaw_spin = YawSpin::new(&tune, &rates);
        FlightController {
            last: DebugRecord {
                armed,
                arming_blocks: arming.blocks,
                failsafe: failsafe_readings(&failsafe, &receiver),
                setpoint: [0.0; 3],
                smoothed_setpoint: [0.0; 3],
                gyro,
                filtered_gyro: gyro,
                detected_packet_rate: packet_rate.detected_hz,
                rc_smoothing_cutoffs: cutoffs(&rc_smoothing),
                terms: [Terms::default(); 3],
                throttle: 0.0,
                motors: [stopped(); 4],
                crash_flip: false,
                yaw_spin_recovery: false,
                flight_stable: false,
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
            packet_rate,
            rc_smoothing,
            feedforward,
            gyro_filters,
            dynamic_throttle: DynamicThrottle::new(),
            arming,
            throttle_raised: false,
            stabilising: false,
            reset_iterm: true,
            pid,
            yaw_spin,
            runaway: Runaway::default(),
            tilt_at_start: None,
            dshot: DshotCommands::new(loop_hz),
        }
    }

    /// The yaw rate at which yaw spin recovery starts, in °/s, as the Tune
    /// and the Rates set it; `None` with it off.
    pub fn yaw_spin_threshold(&self) -> Option<i64> {
        self.yaw_spin.threshold()
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
        let before = (self.arming.armed, self.arming.crash_flip);
        let changed = self.receiver.look(now_us, frame);
        self.failsafe.check(now_ms, &mut self.arming);
        if changed {
            self.receive(now_us, readings);
        }
        let unfiltered = degrees(readings.gyro);
        let gyro = self.gyro_filters.apply(unfiltered);
        self.process_sticks(changed);
        let smoothed = self.rc_smoothing.smoothed;
        let smoothed_setpoint = [smoothed[0], smoothed[1], smoothed[2]];
        self.yaw_spin.check(gyro[2], now_us);
        self.pid.step(
            smoothed_setpoint,
            gyro,
            self.rc_smoothing.feedforward_smoothed,
            self.stabilising,
            self.reset_iterm,
            self.yaw_spin.active,
        );
        let sums = self.pid.terms.map(|terms| terms.sum);
        if self
            .runaway
            .loop_check(&self.tune, self.runaway_moment(now_us), sums, gyro)
        {
            self.arming.blocks.runaway = true;
            self.arming.disarm();
        }
        // Arming and disarming tell the ESCs which way to spin; so does Crash
        // Flip ending while armed (`crashflip_auto_rearm`).
        match (before, (self.arming.armed, self.arming.crash_flip)) {
            ((false, _), (true, crash_flip)) => self.dshot.spin(if crash_flip {
                SpinDirection::Reversed
            } else {
                SpinDirection::Normal
            }),
            ((true, _), (false, _)) | ((true, true), (true, false)) => {
                self.dshot.spin(SpinDirection::Normal);
            }
            _ => {}
        }
        let mixed = mixer::mix(
            &self.tune,
            &self.rates,
            sums,
            smoothed[3],
            self.arming.armed,
            self.yaw_spin.active,
        );
        let dshot = if self.arming.crash_flip {
            let deflection = self.command.map_or([0.0; 3], |command| {
                Axis::ALL.map(|axis| command.deflection(&self.tune, axis))
            });
            let inputs = CrashFlipInputs {
                deflection,
                gyro,
                cos_tilt: readings.attitude.body_to_world(Vec3::new(0.0, 0.0, 1.0)).z,
            };
            mixer::crash_flip(&self.tune, inputs, &mut self.tilt_at_start)
        } else {
            self.tilt_at_start = None;
            // Betaflight's mixer moves the dynamic low-passes, except in
            // Crash Flip, which skips the rest of the mixer.
            if let Some(throttle) =
                self.dynamic_throttle
                    .after_mixing(now_us, mixed.throttle, &mut self.gyro_filters)
            {
                self.pid.follow(throttle);
            }
            mixed.dshot
        };
        let (sent, direction) = self.dshot.output(dshot);
        let motors = sent.map(|dshot| MotorCommand { dshot, direction });
        self.last = DebugRecord {
            armed: self.arming.armed,
            arming_blocks: self.arming.blocks,
            failsafe: failsafe_readings(&self.failsafe, &self.receiver),
            setpoint: self.setpoint,
            smoothed_setpoint,
            gyro: unfiltered,
            filtered_gyro: gyro,
            detected_packet_rate: self.packet_rate.detected_hz,
            rc_smoothing_cutoffs: cutoffs(&self.rc_smoothing),
            terms: self.pid.terms,
            throttle: mixed.throttle,
            motors,
            crash_flip: self.arming.crash_flip,
            yaw_spin_recovery: self.yaw_spin.active,
            flight_stable: self.runaway.done,
        };
        self.loops += 1;
        motors
    }

    /// Betaflight's work on the Channels, in its order: they are worked out
    /// again, and Failsafe told whether they were good; Airmode's start and
    /// the PID loop's state read the arming as it stood, and runaway takeoff
    /// prevention adds up stable flight; then the Arm and Crash Flip
    /// switches; then the stick commands and the setpoints; then the arming
    /// checks.
    fn receive(&mut self, now_us: u64, readings: &SensorReadings) {
        let now_ms = now_us / 1000;
        let good = self
            .receiver
            .apply(now_ms, &self.tune, self.failsafe.active);
        if good {
            self.failsafe.good_channels(now_ms, &mut self.arming.blocks);
        } else {
            self.failsafe.bad_channels(now_ms, &mut self.arming.blocks);
        }
        self.packet_rate.frame_check(now_us, self.receiver.signal);
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
        // The sticks as the frame before left them, as Betaflight's
        // `processRx` reads them.
        let deflection = self
            .command
            .map(|command| Axis::ALL.map(|axis| command.deflection(&self.tune, axis)));
        self.runaway.frame(
            &self.tune,
            self.runaway_moment(now_us),
            rc.throttle_percent(&self.tune),
            deflection,
            self.pid.terms.map(|terms| terms.sum),
        );

        let was_armed = self.arming.armed;
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
        if !was_armed && self.arming.armed {
            self.runaway.arm();
        }

        let command = RcCommand::new(&rc, &self.tune, &self.throttle_lookup);
        for axis in Axis::ALL {
            let deflection = command.deflection(&self.tune, axis);
            self.setpoint[axis.index()] = self.rates.setpoint(axis, deflection);
        }
        self.command = Some(command);
    }

    /// Betaflight's work on the sticks each loop (`processRcCommand`): when
    /// the Channels were worked out again, the detected Packet Rate moves on,
    /// retuning RC smoothing and feedforward every third frame that counts,
    /// and feedforward is worked out for the frame; then RC smoothing takes a
    /// step.
    fn process_sticks(&mut self, changed: bool) {
        let newest = match (changed, self.command) {
            (true, Some(command)) => {
                if self.packet_rate.retune(self.receiver.signal) {
                    let detected = self.packet_rate.detected_hz;
                    self.feedforward.retune(detected);
                    self.rc_smoothing.set_cutoffs(detected);
                }
                self.feedforward.frame(&FeedforwardFrame {
                    sticks: command.sticks,
                    setpoints: self.setpoint,
                    deflections: Axis::ALL.map(|axis| command.deflection(&self.tune, axis).abs()),
                    gap: self.packet_rate.gap_us as f64 * 1e-6,
                    rate_hz: self.packet_rate.rate_hz,
                });
                Some([
                    self.setpoint[0],
                    self.setpoint[1],
                    self.setpoint[2],
                    command.throttle as f64,
                ])
            }
            _ => None,
        };
        self.rc_smoothing.apply(newest, self.feedforward.raw);
    }

    /// Where runaway takeoff prevention finds the Flight Controller now.
    fn runaway_moment(&self, now_us: u64) -> runaway::Moment {
        runaway::Moment {
            armed: self.arming.armed,
            crash_flip: self.arming.crash_flip,
            now_us,
        }
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
        self.packet_rate.write_fingerprint(f);
        self.rc_smoothing.write_fingerprint(f);
        self.feedforward.write_fingerprint(f);
        self.gyro_filters.write_fingerprint(f);
        self.dynamic_throttle.write_fingerprint(f);
        self.arming.write_fingerprint(f);
        for flag in [self.throttle_raised, self.stabilising, self.reset_iterm] {
            f.write_u64(u64::from(flag));
        }
        self.pid.write_fingerprint(f);
        self.yaw_spin.write_fingerprint(f);
        self.runaway.write_fingerprint(f);
        match self.tilt_at_start {
            None => f.write_u64(0),
            Some(tilt) => {
                f.write_u64(1);
                f.write_f64(tilt);
            }
        }
        self.dshot.write_fingerprint(f);
        for motor in self.last.motors {
            f.write_u64(u64::from(motor.dshot));
            f.write_u64(match motor.direction {
                SpinDirection::Normal => 0,
                SpinDirection::Reversed => 1,
            });
        }
    }
}

/// RC smoothing's cutoffs for the [`DebugRecord`].
fn cutoffs(rc_smoothing: &RcSmoothing) -> Option<[u16; 2]> {
    rc_smoothing.on().then_some([
        rc_smoothing.setpoint_cutoff_hz,
        rc_smoothing.throttle_cutoff_hz,
    ])
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
