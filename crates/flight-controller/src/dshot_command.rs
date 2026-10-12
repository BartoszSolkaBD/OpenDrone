//! The DShot commands that tell the ESCs which way to spin, reimplemented
//! from Betaflight 2026.6.2's `dshotCommandWrite`,
//! `dshotCommandOutputIsEnabled` and `dshotCommandIsProcessing`
//! (`src/main/drivers/dshot_command.c`) and the DShot output that sends them
//! (`pwmWriteDshotInt`, `pwmCompleteDshotMotorUpdate`); the Betaflight
//! research §4.1.
//!
//! On every arm Betaflight queues "spin direction normal" (DShot command 20)
//! for all four ESCs, or "spin direction reversed" (21) when the Crash Flip
//! switch is on, and on every disarm "normal" again (`setMotorSpinDirection`,
//! `core.c`). A command waits until every motor was last sent 0 ("stop"),
//! then:
//!
//! 1. 10 ms with no DShot frames at all (`DSHOT_INITIAL_DELAY_US`);
//! 2. the command, sent 10 times, 1 ms apart, with no frames in between
//!    (`DSHOT_COMMAND_DELAY_US`);
//! 3. 1 ms more with no frames.
//!
//! Every wait counts whole loops, rounded up (`dshotCommandCyclesFromTime`):
//! at 8 kHz the first command goes out on the 81st loop and the motors'
//! throttle on the 171st, 21.25 ms after the loop that armed. While no frame
//! is sent, an ESC holds the last one it got: "stop", or the command, which
//! Bluejay takes as no throttle too.
//!
//! Bluejay takes a direction command once it has read it seven times in a
//! row (`DShot_Cmd_Cnt` reaching 6, `src/Modules/DShot.asm` and `Isrs.asm` in
//! v0.21.0). The motor commands carry the direction the ESCs have taken: it
//! changes on the command's seventh frame. The motors are stopped through all
//! of it, so the moment changes nothing in the air.

use opendrone_maths::Fingerprinter;

use crate::SpinDirection;

/// How many commands the queue holds (`DSHOT_MAX_COMMANDS`): any more are
/// dropped.
const MOST_QUEUED: usize = 3;
/// The wait before a command's first frame, in µs.
const INITIAL_DELAY_US: u64 = 10_000;
/// The wait after each frame of a command, in µs.
const COMMAND_DELAY_US: u64 = 1_000;
/// How many times a direction command is sent.
const REPEATS: u8 = 10;
/// How many of them in a row Bluejay reads before it takes one: its count
/// starts at 0 on the first and takes it at 6.
const BLUEJAY_READS: u8 = 7;

/// DShot command 20, "spin direction normal", and 21, "reversed"
/// (`DSHOT_CMD_SPIN_DIRECTION_NORMAL`, `_REVERSED`).
fn code(direction: SpinDirection) -> u16 {
    match direction {
        SpinDirection::Normal => 20,
        SpinDirection::Reversed => 21,
    }
}

/// Where a queued command stands (`dshotCommandState_e`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Waiting for every motor to have been sent "stop".
    IdleWait,
    /// The wait before its first frame.
    StartDelay,
    /// Being sent.
    Active,
    /// The wait after its last frame.
    PostDelay,
}

/// One queued command (`dshotCommandControl_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Command {
    direction: SpinDirection,
    state: State,
    /// Loops left to wait.
    wait: u64,
    /// Frames left to send.
    repeats: u8,
}

/// The DShot output: the queue of direction commands and what each ESC last
/// got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DshotCommands {
    /// The loop's length in whole µs (`dshotCommandPidLoopTimeUs`).
    loop_us: u64,
    queue: Vec<Command>,
    /// The value each motor's output was last given, sent or not
    /// (`protocolControl.value`): "idle" means 0.
    written: [u16; 4],
    /// The last value each ESC got on the wire.
    sent: [u16; 4],
    /// The direction the ESCs have taken.
    direction: SpinDirection,
    /// The command Bluejay is counting, and how many times in a row it has
    /// read it.
    counting: Option<(u16, u8)>,
}

impl DshotCommands {
    /// At power-up: nothing queued, every ESC stopped and spinning the
    /// normal way. `loop_hz` is how many loops a second the Flight Controller
    /// runs.
    pub fn new(loop_hz: u32) -> DshotCommands {
        DshotCommands {
            loop_us: (1_000_000 / u64::from(loop_hz.max(1))).max(1),
            queue: Vec::new(),
            written: [0; 4],
            sent: [0; 4],
            direction: SpinDirection::Normal,
            counting: None,
        }
    }

    /// Queues a spin direction command for all four ESCs
    /// (`dshotCommandWrite`, inline), unless the queue is full.
    pub fn spin(&mut self, direction: SpinDirection) {
        if self.queue.len() >= MOST_QUEUED {
            return;
        }
        let idle = self.all_idle();
        self.queue.push(Command {
            direction,
            state: if idle {
                State::StartDelay
            } else {
                State::IdleWait
            },
            wait: if idle {
                self.loops(INITIAL_DELAY_US)
            } else {
                0
            },
            repeats: REPEATS,
        });
    }

    /// One loop's output: the mixer's four DShot values go out, or a
    /// command's, or nothing at all while a command waits. Gives what each
    /// ESC has last got, and the direction they have taken.
    pub fn output(&mut self, motors: [u16; 4]) -> ([u16; 4], SpinDirection) {
        // `pwmWriteDshotInt`: a command being processed takes the motors'
        // place.
        let values = match self.processing() {
            Some(direction) => [code(direction); 4],
            None => motors,
        };
        self.written = values;
        // `pwmCompleteDshotMotorUpdate`.
        if self.queue.is_empty() || self.output_enabled() {
            self.send(values);
        }
        (self.sent, self.direction)
    }

    /// The command being processed, whose code replaces the motors' values
    /// (`dshotCommandIsProcessing`).
    fn processing(&self) -> Option<SpinDirection> {
        let command = self.queue.first()?;
        let last = self.queue.len() == 1;
        match command.state {
            State::StartDelay | State::Active => Some(command.direction),
            State::PostDelay if !last => Some(command.direction),
            State::IdleWait | State::PostDelay => None,
        }
    }

    /// Whether this loop sends a frame (`dshotCommandOutputIsEnabled`), moving
    /// the queue's first command on.
    fn output_enabled(&mut self) -> bool {
        let idle = self.all_idle();
        let initial = self.loops(INITIAL_DELAY_US);
        let gap = self.loops(COMMAND_DELAY_US);
        let last = self.queue.len() == 1;
        let Some(command) = self.queue.first_mut() else {
            return true;
        };
        match command.state {
            State::IdleWait => {
                if idle {
                    command.state = State::StartDelay;
                    command.wait = initial;
                }
                true
            }
            State::StartDelay | State::Active => {
                if command.wait > 0 {
                    command.wait -= 1;
                    return false;
                }
                command.state = State::Active;
                command.repeats -= 1;
                if command.repeats > 0 {
                    command.wait = gap;
                } else {
                    command.state = State::PostDelay;
                    command.wait = gap;
                    // The loop between two commands counts towards the wait.
                    if !last && command.wait > 0 {
                        command.wait -= 1;
                    }
                }
                true
            }
            State::PostDelay => {
                if command.wait > 0 {
                    command.wait -= 1;
                    return false;
                }
                // `dshotCommandQueueUpdate`: the next command, if any, goes
                // straight to being sent, and this loop sends nothing.
                self.queue.remove(0);
                match self.queue.first_mut() {
                    Some(next) => {
                        next.state = State::Active;
                        next.wait = 0;
                        false
                    }
                    None => true,
                }
            }
        }
    }

    /// A frame goes out to every ESC; Bluejay counts the commands among them.
    fn send(&mut self, values: [u16; 4]) {
        self.sent = values;
        let value = values[0];
        if value == 0 || value >= 48 {
            // "Stop" clears Bluejay's command; a throttle leaves it be.
            if value == 0 {
                self.counting = None;
            }
            return;
        }
        let count = match self.counting {
            Some((counted, count)) if counted == value => count + 1,
            _ => 1,
        };
        if count >= BLUEJAY_READS {
            self.direction = if value == code(SpinDirection::Reversed) {
                SpinDirection::Reversed
            } else {
                SpinDirection::Normal
            };
            self.counting = None;
        } else {
            self.counting = Some((value, count));
        }
    }

    /// Whether every motor's output was last given "stop"
    /// (`allMotorsAreIdle`).
    fn all_idle(&self) -> bool {
        self.written.iter().all(|value| *value == 0)
    }

    /// The fewest whole loops that last at least `us`.
    fn loops(&self, us: u64) -> u64 {
        us.div_ceil(self.loop_us)
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.queue.len() as u64);
        for command in &self.queue {
            f.write_u64(match command.direction {
                SpinDirection::Normal => 0,
                SpinDirection::Reversed => 1,
            });
            f.write_u64(match command.state {
                State::IdleWait => 0,
                State::StartDelay => 1,
                State::Active => 2,
                State::PostDelay => 3,
            });
            f.write_u64(command.wait);
            f.write_u64(u64::from(command.repeats));
        }
        for value in self.written.into_iter().chain(self.sent) {
            f.write_u64(u64::from(value));
        }
        f.write_u64(match self.direction {
            SpinDirection::Normal => 0,
            SpinDirection::Reversed => 1,
        });
        match self.counting {
            None => f.write_u64(0),
            Some((value, count)) => {
                f.write_u64(1);
                f.write_u64(u64::from(value));
                f.write_u64(u64::from(count));
            }
        }
    }
}
