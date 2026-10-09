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
//! # Stalls and restarts
//!
//! A motor can stall: a Prop Strike can stop it at once, or a prop jammed
//! against the Map can hold it still (#26 §1, ADR-0012).
//!
//! - **A stall.** Bluejay watches the motor's back-voltage. A running motor
//!   that falls below the minimum speed while the command is above zero, or
//!   is turned backwards (its back-voltage then crosses zero in the wrong
//!   order), has stalled (`run6_check_speed`, L983–987, and the zero-cross
//!   timeouts, `evaluate_comparator_integrity`, Timing.asm L771–788). The
//!   ESC switches it off (`exit_run_mode`, L1037–1041) and, the command
//!   being above zero, treats it as a stall (L1067–1071).
//! - **A restart.** It waits 100 ms with the motor off ("wait for a bit
//!   between stall restarts", `wait100ms`, L1084; the Quad definition's
//!   `start_wait`) and starts it again exactly as from a standstill
//!   (`ljmp motor_start`, L1086): the drive capped at the start-up power
//!   limit for 15 electrical turns. A zero command by the end of the wait
//!   leaves the motor stopped and the ESC ready (the start-up phase leaves
//!   run mode at zero throttle, L918–919, and that stop is no stall,
//!   L1067).
//! - **A failed start.** While it starts a motor, Bluejay steps the motor's
//!   field on when the rotor's back-voltage says it has turned (its zero
//!   cross), or, when the rotor doesn't answer, when its wait for that times
//!   out (Timing.asm L533–575 and L640–652); it never leaves run mode on a
//!   timeout during the start-up and initial-run phases (L771–776). So a
//!   jammed motor still gets through its 15 capped electrical turns, at the
//!   timeouts' pace. Then it waits for one more zero cross, its drive still
//!   capped (the low-speed limit `Pwm_Limit_By_Rpm` is still Startup Power
//!   Max, Power.asm L65–96), and that wait's timeout ends the try
//!   (`exit_run_mode_on_timeout`, L1033–1036). Here a start's turns go on at
//!   the rotor's pace or the timeouts', whichever is faster; when its 15
//!   turns are done, a rotor turning the right way at the minimum speed or
//!   faster runs on, and a slower one has the closing wait to get there
//!   before the start has failed. Worked from the timeouts, a jammed try
//!   takes 1.711 s, on an assumption about Bluejay's comparator spelt out
//!   after the list:
//!   - Bluejay's commutation timers count 12 cycles of its 24.5 MHz clock
//!     (L553–554), 0.49 µs. While the rotor doesn't answer, its estimate of
//!     four commutations' time (`Comm_Period4x`) sits at its slowest, 0xFFFF
//!     counts (Timing.asm L94–117, L178–217), so 15 electrical degrees is a
//!     sixteenth of that less 2 counts: 4,093 counts (Timing.asm L226–240).
//!   - In the start-up phase (four turns, 24 commutations) each commutation
//!     waits 15° of advance (the other waits are cut to 16 counts, Timing.asm
//!     L463–470), then a zero-cross wait of a quarter of the estimate plus
//!     0x4000 counts (Timing.asm L542–557), which times out twice (L540,
//!     L645): 69,659 counts, 34.1 ms.
//!   - In the initial-run phase (the other 11 turns, 66 commutations) each
//!     waits 15°, 15° and 7.5°, then a zero-cross wait of a quarter of the
//!     estimate, once: 26,615 counts, 13.0 ms.
//!   - The closing wait, for one zero cross after the 15 turns. The model
//!     takes the comparator to read the level opposite to the one Bluejay
//!     waits for. That first reading clears the demag flag that every wait
//!     starts with and sets a timeout of 65,280 counts, 32.0 ms (Timing.asm
//!     L599, L656–658, L671–673, L693–734); while it keeps reading so,
//!     nothing can end the wait but that timeout, which, with the flag clear,
//!     leaves run mode (L771–788). Had it read the level Bluejay waits for
//!     instead, the demag flag would stay set, the first wait's timeout
//!     (16,383 counts) wouldn't leave run mode (L777), and the next wait, for
//!     the other level, would end the same way: one commutation (26,615
//!     counts, 13.0 ms) later, 1.724 s in all.
//!
//!   **The assumption.** The working, like the model, takes a jammed rotor
//!   to show Bluejay none of the levels it waits for, at any start-up or
//!   initial-run step, so every one of those 90 waits times out. Nothing in
//!   the physics says what the comparator reads with no back-voltage, and a
//!   real one may do otherwise. A level fixed by its own offset is the one
//!   Bluejay waits for at half the steps (each phase is waited on for high
//!   at one step and for low at another), and in the start phases a good
//!   reading ends the wait at once (L608–609, L660–669), so a try would
//!   take about 1.0 s. A comparator showing the awaited level at every step
//!   could even carry the start on to `run6`, which clears the count of
//!   failed starts (Bluejay.asm L957–960), so Bluejay might never give up.
//!   The Scenarios check the moments around a jammed try with room for any
//!   try from about 1.0 s to 1.724 s.
//!
//!   Bluejay builds for 48 MHz chips run the start-up phase's waits about a
//!   quarter faster and the closing wait twice as fast, about 1.5 s a try;
//!   these numbers are for the 24.5 MHz chips' timing.
//! - **Giving up.** Bluejay counts failed starts in a row
//!   (`Startup_Stall_Cnt`: up one at each failed start, L1033–1036, back to
//!   nought once the motor runs properly, L957–959, and at every stop at zero
//!   throttle, L1122–1124). A stall when the count has reached the Quad
//!   definition's `restart_tries` (3, Bluejay's own limit: L1073–1077) is the
//!   last: the ESC plays its "motor stalled" beeps (`beep_motor_stalled`,
//!   L1104; Fx.asm L197–201, three tones of 123, 116 and 108 ms by the
//!   ESC Configurator's tone formula) and goes back to waiting for arming
//!   (`arming_begin`, L1107 and L633–656): the "signal found" beep, then it
//!   waits for the throttle to be zero for ten counts of its 32 ms timer, as
//!   at power-up, then the ready beep. So the motor stays off, "stopped
//!   after failed restarts", until the Flight Controller sends zero throttle:
//!   until it disarms. Betaflight sends zero to every motor while disarmed,
//!   and an armed Quad never sends less than its idle, so zero throttle is
//!   how the Flight Controller's disarm reaches the ESC; nothing else is
//!   needed. (Bluejay's Timer2 runs all the time; its count starts afresh
//!   here when the ESC gives up.)
//! - **How many restarts.** A running motor that stalls hasn't failed a
//!   start, so it gets 3 restarts before the ESC gives up. A motor jammed
//!   from a standstill fails its first start, so it gets 2.
//!
//! Not modelled yet: Bluejay's low-speed power limit after the start-up phase
//! (`Pgm_Rpm_Power_Slope`; the measured spin-up times already include whatever
//! a real ESC does), the beacon after 10 idle minutes, signal loss, a stall
//! found by a zero-cross timeout above the minimum speed (a motor slowed
//! suddenly but not stopped), Bluejay's own direction change while running
//! (Crash Flip doesn't need it: Betaflight sends a spin direction command
//! only once every motor has been sent "stop"), and the start-up power floor and
//! stall boost: during the initial-run phase Bluejay raises a command to at
//! least Startup Power Min (21 of 2047) and adds 40 of 2047 for each failed
//! start in a row before the start-up cap (Isrs.asm L286–318), which matters
//! only for commands below the cap.

use core::f64::consts::TAU;

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
const SIGNAL_FOUND_BEEP: f64 = tone(30.0, 66.0);
/// `beep_f2_short`: 44 pulses at loop length 45.
const READY_BEEP: f64 = tone(44.0, 45.0);
/// `beep_motor_stalled`: `beep_f3` (92 pulses at loop length 38), `beep_f2`
/// (77 at 45) and `beep_f1` (53 at 66), one after another.
const STALLED_BEEPS: f64 = tone(92.0, 38.0) + tone(77.0, 45.0) + tone(53.0, 66.0);
/// How often Timer2 overflows before arming: 65,536 × 12 cycles at 24.5 MHz.
const TIMER2: f64 = 65_536.0 * 12.0 / 24_500_000.0;
/// Timer2 overflows at zero throttle before the ready beep.
const ZERO_THROTTLE_COUNTS: u32 = 10;
/// Electrical turns with the drive capped at the start-up power limit: four
/// for the start-up phase, and 11 more until the initial-run countdown of 12,
/// which starts on the fourth, reaches nought.
const CAPPED_ELECTRICAL_TURNS: f64 = 15.0;
/// Electrical turns in the start-up phase: 24 commutations.
const START_UP_PHASE_TURNS: f64 = 4.0;
/// Bluejay's minimum running speed, in electrical RPM.
const MINIMUM_ELECTRICAL_RPM: f64 = 1330.0;
/// One count of Bluejay's commutation timers: 12 cycles of its 24.5 MHz
/// clock.
const TIMER_COUNT: f64 = 12.0 / 24_500_000.0;
/// One commutation in the start-up phase when the rotor doesn't answer: 15°
/// of advance (4,093 counts), the two short waits (16 counts each) and the
/// zero-cross wait timing out twice (2 × 32,767 counts). See the module's
/// "Stalls and restarts".
const START_UP_COMMUTATION: f64 = 69_659.0 * TIMER_COUNT;
/// One commutation in the initial-run phase when the rotor doesn't answer:
/// 15°, 15° and 7.5° of waits (10,232 counts) and the zero-cross wait timing
/// out once (16,383 counts).
const INITIAL_RUN_COMMUTATION: f64 = 26_615.0 * TIMER_COUNT;
/// The wait for a zero cross after the 15 capped turns, when the rotor
/// doesn't answer: a comparator reading the wrong level sets this timeout,
/// 65,280 counts. See the module's "Stalls and restarts".
const CLOSING_WAIT: f64 = 65_280.0 * TIMER_COUNT;

/// How long a beep of `pulses` pulses at loop length `length` lasts, in
/// seconds: the ESC Configurator's tone formula, which a cycle count of
/// Fx.asm's `beep` (L131–183) matches to 0.15%.
const fn tone(pulses: f64, length: f64) -> f64 {
    pulses * (24.72 * length + 399.3) * 1e-6
}

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
    /// Its motor stalled: it switched the motor off and waits the start wait
    /// before restarting it.
    RestartWait,
    /// Restarting its stalled motor as it starts a stopped one, its drive held
    /// under the start-up power limit.
    Restarting,
    /// It gave up after the Quad definition's `restart_tries` failed starts in
    /// a row: the motor stays off until the throttle has been zero for ten
    /// counts of its timer, that is until the Flight Controller disarms, and
    /// the ESC has beeped ready again.
    StoppedAfterFailedRestarts(StoppedStep),
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

/// The steps after an ESC gives up on a stalled motor, in order: Bluejay's
/// "motor stalled" beeps, then the end of its power-up again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoppedStep {
    /// Three falling tones.
    StalledBeeps,
    /// The "signal found" beep.
    SignalFound,
    /// Silence until the throttle has been zero for ten counts of its timer:
    /// until the Flight Controller disarms.
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
            EscState::RestartWait => "restart wait",
            EscState::Restarting => "restarting",
            EscState::StoppedAfterFailedRestarts(step) => match step {
                StoppedStep::StalledBeeps => {
                    "stopped after failed restarts (\"motor stalled\" beeps)"
                }
                StoppedStep::SignalFound => "stopped after failed restarts (\"signal found\" beep)",
                StoppedStep::WaitingForZeroThrottle => {
                    "stopped after failed restarts (waiting for zero throttle)"
                }
                StoppedStep::ReadyBeep => "stopped after failed restarts (ready beep)",
            },
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
            EscState::RestartWait => 11,
            EscState::Restarting => 12,
            EscState::StoppedAfterFailedRestarts(step) => match step {
                StoppedStep::StalledBeeps => 13,
                StoppedStep::SignalFound => 14,
                StoppedStep::WaitingForZeroThrottle => 15,
                StoppedStep::ReadyBeep => 16,
            },
        }
    }

    /// True while Bluejay beeps, with its interrupts off: it reads no
    /// command, and a Timer2 overflow counts only once the beep ends.
    fn beeping(self) -> bool {
        matches!(
            self,
            EscState::StartingUp(StartUpStep::SignalFound | StartUpStep::ReadyBeep)
                | EscState::StoppedAfterFailedRestarts(
                    StoppedStep::StalledBeeps | StoppedStep::SignalFound | StoppedStep::ReadyBeep
                )
        )
    }
}

/// The ESC's numbers from the Quad definition (#26 §5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EscParameters {
    /// How long a ready ESC waits before starting a stopped motor, and how
    /// long it waits with a stalled motor off before restarting it, in
    /// seconds.
    pub start_wait: f64,
    /// The highest drive while a motor starts, as a share of full drive.
    pub startup_power_limit: f64,
    /// How many starts in a row may fail before the ESC gives up on its
    /// motor: after a running motor stalls, that many restarts; a motor
    /// jammed from a standstill fails its first start too, so it gets one
    /// fewer.
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
    /// Seconds since Timer2 last overflowed, while its count of zero
    /// throttle matters.
    timer: Option<f64>,
    /// Timer2 overflows at zero throttle (`Rcp_Stop_Cnt`).
    zero_counts: u32,
    /// An overflow fell inside a beep, to count at its end.
    overflow_pending: bool,
    /// How many electrical turns the current start has stepped the motor's
    /// field through.
    start_turns: f64,
    /// Seconds the current start has waited for a zero cross since its 15
    /// capped turns were done.
    closing: f64,
    /// Failed starts in a row (`Startup_Stall_Cnt`).
    failed_starts: u32,
    /// Restarts since the motor last ran properly or stopped at zero
    /// throttle.
    restarts: u32,
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
            start_turns: 0.0,
            closing: 0.0,
            failed_starts: 0,
            restarts: 0,
        }
    }

    pub(crate) fn state(&self) -> EscState {
        self.state
    }

    /// How many times it has restarted its motor since the motor last ran
    /// properly or stopped at zero throttle.
    pub(crate) fn restarts(&self) -> u32 {
        self.restarts
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
        let minimum = MINIMUM_ELECTRICAL_RPM * TAU / 60.0 / pole_pairs;
        // How fast it turns the way it's told to: turning the other way, its
        // back-voltage crosses zero in the wrong order, and Bluejay can't run
        // it.
        let forward = match command.direction {
            SpinDirection::Normal => speed,
            SpinDirection::Reversed => -speed,
        };
        let too_slow = forward < minimum;

        match self.state {
            EscState::StartingUp(step) => self.start_up(step, zero),
            EscState::Ready => {
                if !zero {
                    self.enter(EscState::StartWait);
                }
            }
            EscState::StartWait | EscState::RestartWait => {
                if self.in_state >= parameters.start_wait - dt / 2.0 {
                    if zero {
                        self.stop();
                    } else if self.state == EscState::StartWait {
                        self.enter(EscState::Starting);
                    } else {
                        self.restarts = self.restarts.saturating_add(1);
                        self.enter(EscState::Restarting);
                    }
                }
            }
            EscState::Starting | EscState::Restarting => {
                if zero {
                    self.stop();
                } else {
                    // The field steps on at the rotor's pace, or at the
                    // zero-cross timeouts' when the rotor lags.
                    let commutation = if self.start_turns < START_UP_PHASE_TURNS {
                        START_UP_COMMUTATION
                    } else {
                        INITIAL_RUN_COMMUTATION
                    };
                    let rotor = functions::max(forward, 0.0) * pole_pairs / TAU * dt;
                    self.start_turns += functions::max(rotor, dt / (6.0 * commutation));
                    if self.start_turns >= CAPPED_ELECTRICAL_TURNS {
                        if !too_slow {
                            self.failed_starts = 0;
                            self.restarts = 0;
                            self.enter(EscState::Running);
                        } else {
                            // Still capped, waiting for a zero cross that
                            // doesn't come.
                            self.closing += dt;
                            if self.closing >= CLOSING_WAIT - dt / 2.0 {
                                self.failed_starts = self.failed_starts.saturating_add(1);
                                self.stalled(parameters);
                            }
                        }
                    }
                }
            }
            EscState::Running => {
                if zero {
                    if speed.abs() < minimum {
                        self.stop();
                    }
                } else if too_slow {
                    self.stalled(parameters);
                }
            }
            EscState::StoppedAfterFailedRestarts(step) => self.after_giving_up(step, zero),
        }

        match self.state {
            EscState::Starting | EscState::Restarting => Drive::On {
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

    /// Its motor stalled with the command above zero: it restarts it, or
    /// gives up once `restart_tries` starts in a row have failed.
    fn stalled(&mut self, parameters: &EscParameters) {
        if self.failed_starts >= parameters.restart_tries {
            // `arming_begin` clears the count; Timer2's count of zero
            // throttle starts afresh.
            self.failed_starts = 0;
            self.timer = Some(0.0);
            self.zero_counts = 0;
            self.overflow_pending = false;
            self.enter(EscState::StoppedAfterFailedRestarts(
                StoppedStep::StalledBeeps,
            ));
        } else {
            self.enter(EscState::RestartWait);
        }
    }

    /// A stop at zero throttle: ready, with the counts cleared.
    fn stop(&mut self) {
        self.failed_starts = 0;
        self.restarts = 0;
        self.enter(EscState::Ready);
    }

    /// Bluejay's Timer2 and its count of zero-throttle overflows.
    fn run_timer(&mut self, zero: bool, dt: f64) {
        let Some(since) = self.timer.as_mut() else {
            return;
        };
        let beeping = self.state.beeping();
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

    /// After giving up: the "motor stalled" beeps, then the end of the
    /// power-up again, each step after the one before.
    fn after_giving_up(&mut self, step: StoppedStep, zero: bool) {
        let next = |esc: &mut Esc, length: f64, next: StoppedStep| {
            if esc.in_state >= length {
                esc.in_state -= length;
                esc.state = EscState::StoppedAfterFailedRestarts(next);
                // Interrupts come back on between the beeps.
                esc.count_pending(zero);
            }
        };
        match step {
            StoppedStep::StalledBeeps => next(self, STALLED_BEEPS, StoppedStep::SignalFound),
            StoppedStep::SignalFound => {
                next(self, SIGNAL_FOUND_BEEP, StoppedStep::WaitingForZeroThrottle);
            }
            StoppedStep::WaitingForZeroThrottle => {
                if self.zero_counts >= ZERO_THROTTLE_COUNTS {
                    self.in_state = 0.0;
                    self.state = EscState::StoppedAfterFailedRestarts(StoppedStep::ReadyBeep);
                }
            }
            StoppedStep::ReadyBeep => {
                if self.in_state >= READY_BEEP {
                    self.count_pending(zero);
                    self.timer = None;
                    self.stop();
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
        self.start_turns = 0.0;
        self.closing = 0.0;
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
        f.write_f64(self.start_turns);
        f.write_f64(self.closing);
        f.write_u64(u64::from(self.failed_starts));
        f.write_u64(u64::from(self.restarts));
    }
}
