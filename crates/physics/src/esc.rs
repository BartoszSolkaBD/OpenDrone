//! Each motor's ESC, copying Bluejay v0.21.0's behaviour (#26 §5, #32 §4).
//!
//! Bluejay is the ESC firmware on the Meteor65 Pro. Nothing here is copied
//! from its code (it is GPL); its behaviour and timing are reimplemented from
//! reading `src/Bluejay.asm`, `src/Modules/Fx.asm`, `src/Modules/Isrs.asm`
//! and `src/Modules/Power.asm` at the v0.21.0 tag. The line numbers below are
//! that tag's.
//!
//! # Power-up
//!
//! A freshly powered ESC ignores every command until it has played its
//! start-up tones and its ready beep, about 1.66 s:
//!
//! | Step | Lasts | Bluejay |
//! |---|---|---|
//! | Power-on wait | 100.3 ms | `wait100ms`, Bluejay.asm L498 |
//! | Start-up melody | 1041.5 ms | `play_beep_melody`, L499; the default melody, L406–407 |
//! | Pause | 100.3 ms | `wait100ms`, "wait for flight controller to get ready", L502 |
//! | Looking for the signal | 201.7 ms | `setup_dshot` (L545): `wait1ms` (L562), then the DShot300 and DShot600 tests, 100.3 ms each (L609–613, L623–627) |
//! | "Signal found" beep | 60.9 ms | `beep_f1_short`, L643 |
//! | Waiting for zero throttle | until 10 counts | `arming_wait`, L647–651 |
//! | Ready beep | 66.5 ms | `beep_f2_short`, L654; then `wait_for_start`, L658 |
//!
//! - **Timing.** Bluejay's "millisecond" is 24,581 cycles of its 24.5 MHz
//!   clock (`wait_ms`, Fx.asm L70–86), so `wait100ms` is 100.33 ms. A tone of
//!   `Temp4` pulses of loop length `Temp3` lasts `Temp4` × (24.72 × `Temp3` +
//!   399.3) µs, the ESC Configurator's own formula, which a cycle count of
//!   Fx.asm's `beep` (L131–183) matches to 0.15%. The default melody's notes
//!   and rests add up to 1041.5 ms; `beep_f1_short` is 30 pulses at loop
//!   length 66 (60.9 ms) and `beep_f2_short` 44 at 45 (66.5 ms).
//! - **The signal.** The Flight Controller is taken to send DShot600,
//!   Betaflight 2026.6's default `motor_pwm_protocol`, so the ESC finds it in
//!   its second test. (On DShot300 it would find it 100 ms sooner; the ready
//!   beep comes at the same moment either way.)
//! - **Zero throttle.** Bluejay's Timer2 starts at `setup_dshot` (L553) and
//!   overflows every 65,536 × 12 cycles, 32.10 ms. Each overflow while the
//!   throttle is zero adds one to `Rcp_Stop_Cnt` (Isrs.asm L538–549), and any
//!   frame above zero sets it back to nought (Isrs.asm L338). At 10 the
//!   ESC beeps ready. Interrupts are off during a beep, so overflows that
//!   fall inside one count once, at its end. So a Flight Controller that holds
//!   the throttle at zero from the start gets its ready beep at about 1.595 s,
//!   and the ESC is ready at about 1.662 s.
//!
//! # Starting a stopped motor
//!
//! - **Start wait.** On the first command above zero, a ready ESC waits
//!   (`wait100ms`, "wait to see if start pulse was glitch", L726–730), then
//!   starts the motor only if the command is still above zero. The Quad
//!   definition's `start_wait` holds the length.
//! - **Start-up power.** Every DShot frame's drive is capped at `Pwm_Limit`
//!   (Isrs.asm L347–372), which `motor_start` sets to Startup Power Max
//!   (`Pwm_Limit_Beg`, L762–768), the Quad definition's
//!   `startup_power_limit`. It stays there through two phases, counted in
//!   electrical turns at `run6`, once a turn (L894–955):
//!   - the start-up phase, 24 commutations, four turns (`Startup_Cnt`,
//!     L910–916 and Timing.asm L790–791), which clears only its own flag
//!     (L921–923);
//!   - the initial-run phase, a countdown of 12 (`Initial_Run_Rot_Cntd`, set
//!     at L808), taken one a turn from the turn the start-up phase ends
//!     (L925–935).
//!
//!   The countdown reaches nought on the 15th turn, and only then is the
//!   limit lifted (`initial_run_phase_done`, "lift startup power
//!   restrictions", L942–951). So the drive is capped for 15 electrical turns,
//!   then the motor runs as commanded.
//! - **Stopping.** At a zero command a running motor is braked: Bluejay always
//!   drives "damped light", which brakes at zero drive (L54). Below its
//!   minimum speed, about 1,330 electrical RPM, it powers off (L983–987) and
//!   is ready again.
//!
//! Not modelled yet: Bluejay's low-speed power limit after the start-up phase
//! (`Pgm_Rpm_Power_Slope`; the measured spin-up times already include whatever
//! a real ESC does), the beacon after 10 idle minutes, signal loss, and
//! stalled motors' restarts, which the Prop Strike ticket (#45) adds with the
//! state "stopped after failed restarts".

use opendrone_maths::{Fingerprinter, functions};

use crate::commands::{MotorCommand, SpinDirection};

/// One Bluejay "millisecond": 24,581 cycles of its 24.5 MHz clock.
const BLUEJAY_MS: f64 = 24_581.0 / 24_500_000.0;
/// `wait100ms`.
const WAIT_100MS: f64 = 100.0 * BLUEJAY_MS;
/// The default start-up melody, notes and rests.
const MELODY: f64 = 1.041_49;
/// `wait1ms`, then the DShot300 and DShot600 tests.
const LOOKING_FOR_SIGNAL: f64 = BLUEJAY_MS + 2.0 * WAIT_100MS;
/// `beep_f1_short`: 30 pulses at loop length 66.
const SIGNAL_FOUND_BEEP: f64 = 30.0 * (24.72 * 66.0 + 399.3) * 1e-6;
/// `beep_f2_short`: 44 pulses at loop length 45.
const READY_BEEP: f64 = 44.0 * (24.72 * 45.0 + 399.3) * 1e-6;
/// How often Timer2 overflows before arming: 65,536 × 12 cycles at 24.5 MHz.
const TIMER2: f64 = 65_536.0 * 12.0 / 24_500_000.0;
/// Timer2 overflows at zero throttle before the ready beep.
const ZERO_THROTTLE_COUNTS: u32 = 10;
/// Electrical turns with the drive capped at the start-up power limit: four
/// for the start-up phase, and 11 more until the initial-run countdown of 12,
/// which starts on the fourth, reaches nought.
const CAPPED_ELECTRICAL_TURNS: f64 = 15.0;
/// Bluejay's minimum running speed, in electrical RPM.
const MINIMUM_ELECTRICAL_RPM: f64 = 1330.0;

/// What an ESC is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscState {
    /// Just powered: playing its start-up tones, ignoring every command.
    StartingUp(StartUpStep),
    /// Powered up and armed, its motor stopped, waiting for a command above
    /// zero.
    Ready,
    /// A command above zero arrived; it waits the start wait to see it wasn't
    /// a glitch.
    StartWait,
    /// Starting its motor, its drive held under the start-up power limit.
    Starting,
    /// Driving its motor as commanded, braking it at zero.
    Running,
}

/// The steps of an ESC's power-up, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartUpStep {
    /// Silence while the power settles.
    PowerOn,
    /// The start-up melody.
    Melody,
    /// Silence, for the Flight Controller to get ready.
    Pause,
    /// Testing the signal for each DShot speed.
    LookingForSignal,
    /// The "signal found" beep.
    SignalFound,
    /// Silence until the throttle has been zero for ten counts of its timer.
    WaitingForZeroThrottle,
    /// The ready beep.
    ReadyBeep,
}

impl EscState {
    /// A plain name, such as "starting up (melody)".
    pub fn words(self) -> &'static str {
        match self {
            EscState::StartingUp(step) => match step {
                StartUpStep::PowerOn => "starting up (power-on wait)",
                StartUpStep::Melody => "starting up (melody)",
                StartUpStep::Pause => "starting up (pause)",
                StartUpStep::LookingForSignal => "starting up (looking for the signal)",
                StartUpStep::SignalFound => "starting up (\"signal found\" beep)",
                StartUpStep::WaitingForZeroThrottle => "starting up (waiting for zero throttle)",
                StartUpStep::ReadyBeep => "starting up (ready beep)",
            },
            EscState::Ready => "ready",
            EscState::StartWait => "start wait",
            EscState::Starting => "starting",
            EscState::Running => "running",
        }
    }

    fn code(self) -> u64 {
        match self {
            EscState::StartingUp(step) => match step {
                StartUpStep::PowerOn => 0,
                StartUpStep::Melody => 1,
                StartUpStep::Pause => 2,
                StartUpStep::LookingForSignal => 3,
                StartUpStep::SignalFound => 4,
                StartUpStep::WaitingForZeroThrottle => 5,
                StartUpStep::ReadyBeep => 6,
            },
            EscState::Ready => 7,
            EscState::StartWait => 8,
            EscState::Starting => 9,
            EscState::Running => 10,
        }
    }
}

/// The ESC's numbers from the Quad definition (#26 §5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EscParameters {
    /// How long a ready ESC waits before starting a stopped motor, in
    /// seconds.
    pub start_wait: f64,
    /// The highest drive while a motor starts, as a share of full drive.
    pub startup_power_limit: f64,
    /// How many times it restarts a stalled motor before giving up. Read by
    /// the Prop Strike ticket (#45).
    pub restart_tries: u32,
}

/// What the ESC does to its motor during a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drive {
    /// Every switch off: the motor turns freely.
    Off,
    /// Driving at up to this share of the battery's voltage, this way.
    On {
        throttle: f64,
        direction: SpinDirection,
    },
}

/// One ESC's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Esc {
    state: EscState,
    /// Seconds in the current state or step.
    in_state: f64,
    /// Seconds since Timer2 last overflowed, once it runs.
    timer: Option<f64>,
    /// Timer2 overflows at zero throttle (`Rcp_Stop_Cnt`).
    zero_counts: u32,
    /// An overflow fell inside a beep, to count at its end.
    overflow_pending: bool,
    /// How far the motor has turned since its start-up began, in radians.
    turned: f64,
}

impl Esc {
    /// Just powered up: about to play its start-up tones.
    pub(crate) fn powering_up() -> Esc {
        Esc::in_state(EscState::StartingUp(StartUpStep::PowerOn))
    }

    /// Powered up and ready, its motor stopped.
    pub(crate) fn ready() -> Esc {
        Esc::in_state(EscState::Ready)
    }

    /// Already running its motor.
    pub(crate) fn running() -> Esc {
        Esc::in_state(EscState::Running)
    }

    fn in_state(state: EscState) -> Esc {
        Esc {
            state,
            in_state: 0.0,
            timer: None,
            zero_counts: 0,
            overflow_pending: false,
            turned: 0.0,
        }
    }

    pub(crate) fn state(&self) -> EscState {
        self.state
    }

    /// Moves on by one step of `dt` seconds with this step's command, given
    /// the motor's speed at the step's start (rad/s, positive the normal
    /// way), and says how it drives the motor during the step.
    pub(crate) fn step(
        &mut self,
        command: MotorCommand,
        speed: f64,
        parameters: &EscParameters,
        poles: u32,
        dt: f64,
    ) -> Drive {
        let zero = command.throttle <= 0.0;
        self.in_state += dt;
        self.run_timer(zero, dt);
        let pole_pairs = f64::from(poles.max(2) / 2);

        match self.state {
            EscState::StartingUp(step) => self.start_up(step, zero),
            EscState::Ready => {
                if !zero {
                    self.enter(EscState::StartWait);
                }
            }
            EscState::StartWait => {
                if self.in_state >= parameters.start_wait - dt / 2.0 {
                    self.enter(if zero {
                        EscState::Ready
                    } else {
                        EscState::Starting
                    });
                }
            }
            EscState::Starting => {
                self.turned += speed.abs() * dt;
                if zero {
                    self.enter(EscState::Ready);
                } else if self.turned * pole_pairs
                    >= CAPPED_ELECTRICAL_TURNS * 2.0 * core::f64::consts::PI
                {
                    self.enter(EscState::Running);
                }
            }
            EscState::Running => {
                let minimum =
                    MINIMUM_ELECTRICAL_RPM * 2.0 * core::f64::consts::PI / 60.0 / pole_pairs;
                if zero && speed.abs() < minimum {
                    self.enter(EscState::Ready);
                }
            }
        }

        match self.state {
            EscState::Starting => Drive::On {
                throttle: functions::min(command.throttle, parameters.startup_power_limit),
                direction: command.direction,
            },
            EscState::Running => Drive::On {
                throttle: functions::max(command.throttle, 0.0),
                direction: command.direction,
            },
            _ => Drive::Off,
        }
    }

    /// Bluejay's Timer2 and its count of zero-throttle overflows.
    fn run_timer(&mut self, zero: bool, dt: f64) {
        let Some(since) = self.timer.as_mut() else {
            return;
        };
        let beeping = matches!(
            self.state,
            EscState::StartingUp(StartUpStep::SignalFound | StartUpStep::ReadyBeep)
        );
        // A frame above zero sets the count back, except during a beep, when
        // no frame is read.
        if !zero && !beeping {
            self.zero_counts = 0;
        }
        *since += dt;
        while *since >= TIMER2 {
            *since -= TIMER2;
            if beeping {
                self.overflow_pending = true;
            } else if zero {
                self.zero_counts = self.zero_counts.saturating_add(1);
            }
        }
    }

    /// The power-up's steps, each after the one before.
    fn start_up(&mut self, step: StartUpStep, zero: bool) {
        let next = |esc: &mut Esc, length: f64, next: StartUpStep| {
            if esc.in_state >= length {
                esc.in_state -= length;
                esc.state = EscState::StartingUp(next);
            }
        };
        match step {
            StartUpStep::PowerOn => next(self, WAIT_100MS, StartUpStep::Melody),
            StartUpStep::Melody => next(self, MELODY, StartUpStep::Pause),
            StartUpStep::Pause => {
                next(self, WAIT_100MS, StartUpStep::LookingForSignal);
                if self.state == EscState::StartingUp(StartUpStep::LookingForSignal) {
                    // `setup_dshot` starts Timer2.
                    self.timer = Some(self.in_state);
                }
            }
            StartUpStep::LookingForSignal => {
                next(self, LOOKING_FOR_SIGNAL, StartUpStep::SignalFound);
            }
            StartUpStep::SignalFound => {
                next(self, SIGNAL_FOUND_BEEP, StartUpStep::WaitingForZeroThrottle);
                if self.state == EscState::StartingUp(StartUpStep::WaitingForZeroThrottle) {
                    self.count_pending(zero);
                }
            }
            StartUpStep::WaitingForZeroThrottle => {
                if self.zero_counts >= ZERO_THROTTLE_COUNTS {
                    self.in_state = 0.0;
                    self.state = EscState::StartingUp(StartUpStep::ReadyBeep);
                }
            }
            StartUpStep::ReadyBeep => {
                if self.in_state >= READY_BEEP {
                    self.count_pending(zero);
                    self.timer = None;
                    self.enter(EscState::Ready);
                }
            }
        }
    }

    /// At a beep's end, an overflow that fell inside it counts once.
    fn count_pending(&mut self, zero: bool) {
        if self.overflow_pending && zero {
            self.zero_counts = self.zero_counts.saturating_add(1);
        }
        self.overflow_pending = false;
    }

    fn enter(&mut self, state: EscState) {
        self.state = state;
        self.in_state = 0.0;
        self.turned = 0.0;
    }

    /// Feeds everything it remembers into a fingerprint.
    pub(crate) fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.state.code());
        f.write_f64(self.in_state);
        match self.timer {
            Some(since) => {
                f.write_u64(1);
                f.write_f64(since);
            }
            None => f.write_u64(0),
        }
        f.write_u64(u64::from(self.zero_counts));
        f.write_u64(u64::from(self.overflow_pending));
        f.write_f64(self.turned);
    }
}
