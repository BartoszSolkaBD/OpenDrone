//! Input Tracks: recorded Flight Inputs a Scenario plays back, from a CSV
//! file beside it (#11 §2).
//!
//! The file starts with comment lines (`#`) saying where it came from, then
//! this header, then one row for each Flight Input, in time order:
//!
//! ```text
//! time (s),roll (µs),pitch (µs),throttle (µs),yaw (µs),arm (µs),flight mode (µs),crash flip (µs),other flight input
//! 0,1500,1500,988,1500,988,988,988,
//! 0.004,1503.5,,,,,,,
//! 0.008,,,,,,,,
//! 1.5,,,,,,,,input device lost
//! ```
//!
//! - **`time`** is Simulation Time from the Scenario's start, in seconds, a
//!   whole number of physics steps at the Scenario's physics rate.
//! - **The Channels** are in microseconds, as the Input Device's Calibration
//!   and channel mapping give them, at full resolution. Each is rounded to
//!   the nearest step an ELRS receiver outputs as it enters the Simulation,
//!   as the game rounds a live device's Channels (ADR-0007). A row gives
//!   only the Channels that changed; an empty cell holds the last value.
//!   The first row, at 0 s, gives every Channel.
//! - **A row with no values and no other Flight Input** is a report from the device that
//!   changed nothing. A device that reports at rest sends one every report,
//!   and those are what keep it from counting as lost; a device that reports
//!   only changes has none.
//! - **`other flight input`** is empty, `input device lost` (the computer
//!   reported the Flying Input Device removed), `input device back` or
//!   `reset`: the Flight Inputs that aren't Channels. A row with one gives no
//!   Channels.
//!
//! So a row comes only when something happens: a value changes, the device
//! reports, or another Flight Input comes.

use opendrone_pack::{Problem, Problems};
use opendrone_sim::{Channel, Channels, FlightInput, MotorCommands, PhysicsRate, SimulationTime};

/// The header every Input Track starts with, after its comments.
pub const INPUT_TRACK_HEADER: &str = "time (s),roll (µs),pitch (µs),throttle (µs),yaw (µs),arm (µs),flight mode (µs),crash flip (µs),other flight input";

/// The words of the `other flight input` column.
const LOST: &str = "input device lost";
const BACK: &str = "input device back";
const RESET: &str = "reset";

/// What an Input Track may say, from the Scenario that plays it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TrackRules {
    pub rate: PhysicsRate,
    /// Whether `reset` may appear: only in a Flight Scenario that starts as
    /// Reset leaves the Quad.
    pub reset: bool,
}

/// Reads an Input Track, naming it `file` in problems: every Flight Input in
/// it, at its step, in order. Every problem is listed at once.
pub(crate) fn read_input_track(
    file: &str,
    text: &str,
    rules: TrackRules,
) -> Result<Vec<(SimulationTime, FlightInput)>, Problems> {
    let mut problems = Problems::new();
    let problem = |line: usize, sentence: String| Problem::of(file, line, sentence);
    let mut rows = text
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'));
    match rows.next() {
        Some((_, header)) if header.replace("us)", "µs)") == INPUT_TRACK_HEADER => {}
        Some((line, header)) => {
            problems.push(problem(
                line,
                format!(
                    "an Input Track's first line after its comments is its header, \"{INPUT_TRACK_HEADER}\", not \"{header}\""
                ),
            ));
            return Err(problems);
        }
        None => {
            return Err(Problems::of_file(
                file,
                format!(
                    "an Input Track needs its header, \"{INPUT_TRACK_HEADER}\", and a row at 0 s"
                ),
            ));
        }
    }
    let hz = f64::from(rules.rate.hz());
    let mut inputs = Vec::new();
    let mut channels: Option<Channels> = None;
    let mut last_tick = 0;
    let mut lost = false;
    for (line, row) in rows {
        let Some(read) = read_row(row, hz, rules, line, file, &mut problems) else {
            if channels.is_none() {
                // Nothing after it can be read without the first row.
                return Err(problems);
            }
            continue;
        };
        let Row {
            tick,
            values,
            other_input,
        } = read;
        let at = SimulationTime::from_ticks(tick);
        let current = match channels {
            Some(current) => current,
            None => match (tick, other_input, values) {
                (
                    0,
                    "",
                    [
                        Some(roll),
                        Some(pitch),
                        Some(throttle),
                        Some(yaw),
                        Some(arm),
                        Some(flight_mode),
                        Some(crash_flip),
                    ],
                ) => Channels {
                    roll,
                    pitch,
                    throttle,
                    yaw,
                    arm,
                    flight_mode,
                    crash_flip,
                },
                _ => {
                    problems.push(Problem::of(
                        file,
                        line,
                        "an Input Track's first row is at 0 s and gives every Channel: roll, pitch, throttle, yaw, arm, flight mode and crash flip",
                    ));
                    return Err(problems);
                }
            },
        };
        if tick < last_tick {
            problems.push(Problem::of(
                file,
                line,
                format!(
                    "rows go in time order, but {} s comes after {} s",
                    tick as f64 / hz,
                    last_tick as f64 / hz
                ),
            ));
            continue;
        }
        last_tick = tick;
        let seconds = tick as f64 / hz;
        match other_input {
            "" => {
                let [roll, pitch, throttle, yaw, arm, flight_mode, crash_flip] = values;
                let next = Channels {
                    roll: roll.unwrap_or(current.roll),
                    pitch: pitch.unwrap_or(current.pitch),
                    throttle: throttle.unwrap_or(current.throttle),
                    yaw: yaw.unwrap_or(current.yaw),
                    arm: arm.unwrap_or(current.arm),
                    flight_mode: flight_mode.unwrap_or(current.flight_mode),
                    crash_flip: crash_flip.unwrap_or(current.crash_flip),
                };
                channels = Some(next);
                inputs.push((at, FlightInput::Channels(next)));
            }
            LOST if lost => problems.push(Problem::of(
                file,
                line,
                format!(
                    "the Flying Input Device is lost at {seconds} s, but it was already lost: it must come back first"
                ),
            )),
            LOST => {
                lost = true;
                inputs.push((at, FlightInput::InputDeviceLost));
            }
            BACK if !lost => problems.push(Problem::of(
                file,
                line,
                format!("the Flying Input Device is back at {seconds} s, but it wasn't lost"),
            )),
            BACK => {
                lost = false;
                inputs.push((at, FlightInput::InputDeviceBack));
            }
            _ => inputs.push((at, FlightInput::Reset)),
        }
    }
    if channels.is_none() {
        problems.push(Problem::of(
            file,
            0,
            "an Input Track needs a row at 0 s that gives every Channel",
        ));
    }
    problems.or(inputs)
}

/// One row, read.
struct Row<'r> {
    tick: u64,
    /// Roll, pitch, throttle, yaw, arm, flight mode and crash flip, where
    /// given.
    values: [Option<Channel>; 7],
    /// Empty, or one of the words of the `other flight input` column.
    other_input: &'r str,
}

/// Reads one row, or says what's wrong with it.
fn read_row<'r>(
    row: &'r str,
    hz: f64,
    rules: TrackRules,
    line: usize,
    file: &str,
    problems: &mut Problems,
) -> Option<Row<'r>> {
    let mut problem = |sentence: String| problems.push(Problem::of(file, line, sentence));
    let cells: Vec<&str> = row.split(',').map(str::trim).collect();
    if cells.len() != 9 {
        problem(format!(
            "a row has 9 cells, as the header does (the time, seven Channels and another Flight Input), not {}",
            cells.len()
        ));
        return None;
    }
    let tick = moment(cells[0], hz);
    if tick.is_none() {
        problem(format!(
            "\"{}\" isn't a time OpenDrone can use here: write seconds from the Scenario's start, such as 0.004, each a whole number of physics steps (at {} Hz one step is {} s)",
            cells[0],
            rules.rate.hz(),
            1.0 / hz
        ));
    }
    let mut values: [Option<Channel>; 7] = [None; 7];
    let mut fine = true;
    for (value, cell) in values.iter_mut().zip(&cells[1..8]) {
        if cell.is_empty() {
            continue;
        }
        match micros(cell) {
            Ok(channel) => *value = Some(channel),
            Err(sentence) => {
                problem(sentence);
                fine = false;
            }
        }
    }
    let other_input = cells[8];
    match other_input {
        "" | LOST | BACK => {}
        RESET if rules.reset => {}
        RESET => {
            problem("Reset puts the Quad back where the Scenario starts, as its Launch Spot, so only a Flight Scenario that starts as Reset leaves the Quad (landed and still, disarmed, with its motors \"powering up\") has `reset`".to_string());
            fine = false;
        }
        other => {
            problem(format!(
                "`other flight input` is empty, \"{LOST}\", \"{BACK}\" or \"{RESET}\", not \"{other}\""
            ));
            fine = false;
        }
    }
    if !other_input.is_empty() && values.iter().any(Option::is_some) {
        problem(
            "a row is one Flight Input: Channels, or another Flight Input, not both".to_string(),
        );
        fine = false;
    }
    let tick = tick?;
    fine.then_some(Row {
        tick,
        values,
        other_input,
    })
}

/// The header of a motor Input Track, after its comments: the recorded motor
/// commands a Physics or Thrust Stand Scenario plays back, such as a real
/// flight log's.
pub const MOTOR_TRACK_HEADER: &str = "time (s),motor 1 (%),motor 2 (%),motor 3 (%),motor 4 (%)";

/// Reads a motor Input Track, naming it `file` in problems: every change of
/// the four motor commands, at its step, in order. Every problem is listed at
/// once.
///
/// ```text
/// time (s),motor 1 (%),motor 2 (%),motor 3 (%),motor 4 (%)
/// 0,23.04,25.12,24.9,22.71
/// 0.002,23.1,,,
/// ```
///
/// - **`time`** is Simulation Time from the Scenario's start, in seconds, a
///   whole number of physics steps.
/// - **The motor commands** are in Betaflight's motor order, each from 0% to
///   100%: the ESC's drive, as in a Timeline's `motors`. A row gives only the
///   commands that changed; an empty cell holds the last value. The first
///   row, at 0 s, gives all four.
pub(crate) fn read_motor_track(
    file: &str,
    text: &str,
    rate: PhysicsRate,
) -> Result<Vec<(SimulationTime, MotorCommands)>, Problems> {
    let mut problems = Problems::new();
    let mut rows = text
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'));
    match rows.next() {
        Some((_, header)) if header == MOTOR_TRACK_HEADER => {}
        Some((line, header)) => {
            return Err(Problems(vec![Problem::of(
                file,
                line,
                format!(
                    "a motor Input Track's first line after its comments is its header, \"{MOTOR_TRACK_HEADER}\", not \"{header}\""
                ),
            )]));
        }
        None => {
            return Err(Problems::of_file(
                file,
                format!(
                    "a motor Input Track needs its header, \"{MOTOR_TRACK_HEADER}\", and a row at 0 s"
                ),
            ));
        }
    }
    let hz = f64::from(rate.hz());
    let mut timeline: Vec<(SimulationTime, MotorCommands)> = Vec::new();
    let mut current: Option<[f64; 4]> = None;
    let mut last_tick = 0;
    for (line, row) in rows {
        let mut problem = |sentence: String| problems.push(Problem::of(file, line, sentence));
        let cells: Vec<&str> = row.split(',').map(str::trim).collect();
        if cells.len() != 5 {
            problem(format!(
                "a row has 5 cells, as the header does (the time and four motor commands), not {}",
                cells.len()
            ));
            continue;
        }
        let Some(tick) = moment(cells[0], hz) else {
            problem(format!(
                "\"{}\" isn't a time OpenDrone can use here: write seconds from the Scenario's start, such as 0.002, each a whole number of physics steps (at {} Hz one step is {} s)",
                cells[0],
                rate.hz(),
                1.0 / hz
            ));
            continue;
        };
        let mut values: [Option<f64>; 4] = [None; 4];
        let mut fine = true;
        for (value, cell) in values.iter_mut().zip(&cells[1..]) {
            if cell.is_empty() {
                continue;
            }
            match percent(cell) {
                Some(share) => *value = Some(share),
                None => {
                    problem(format!(
                        "\"{cell}\" isn't a motor command OpenDrone can read: write a percent from 0 to 100, such as 23.04"
                    ));
                    fine = false;
                }
            }
        }
        if !fine {
            continue;
        }
        let next = match current {
            None => match (tick, values) {
                (0, [Some(a), Some(b), Some(c), Some(d)]) => [a, b, c, d],
                _ => {
                    problem(
                        "a motor Input Track's first row is at 0 s and gives all four motor commands"
                            .to_string(),
                    );
                    return Err(problems);
                }
            },
            Some(held) => {
                let mut next = held;
                for (n, value) in next.iter_mut().zip(values) {
                    if let Some(value) = value {
                        *n = value;
                    }
                }
                next
            }
        };
        if current.is_some() && tick <= last_tick {
            problem(format!(
                "rows go in time order, one row a moment, but {} s comes after {} s",
                tick as f64 / hz,
                last_tick as f64 / hz
            ));
            continue;
        }
        last_tick = tick;
        current = Some(next);
        let mut commands = MotorCommands::STOPPED;
        for (command, share) in commands.0.iter_mut().zip(next) {
            command.throttle = share;
        }
        timeline.push((SimulationTime::from_ticks(tick), commands));
    }
    if current.is_none() && problems.is_empty() {
        problems.push(Problem::of(
            file,
            0,
            "a motor Input Track needs a row at 0 s that gives all four motor commands",
        ));
    }
    problems.or(timeline)
}

/// A motor command in percent, from 0 to 100, as a share from 0 to 1.
fn percent(text: &str) -> Option<f64> {
    if !text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    (0.0..=100.0).contains(&value).then_some(value / 100.0)
}

/// Seconds as a whole number of steps at `hz` steps a second.
pub(crate) fn moment(text: &str, hz: f64) -> Option<u64> {
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let seconds: f64 = text.parse().ok()?;
    let steps = seconds * hz;
    let whole = steps.round();
    ((steps - whole).abs() <= 1e-6).then_some(whole as u64)
}

/// A Channel in µs, rounded to the nearest step a receiver outputs.
fn micros(text: &str) -> Result<Channel, String> {
    let plain = !text.is_empty()
        && text
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'));
    let micros: f64 = match text.parse() {
        Ok(micros) if plain => micros,
        _ => {
            return Err(format!(
                "\"{text}\" isn't a Channel OpenDrone can read: write it in µs with a decimal point, such as 1503.5"
            ));
        }
    };
    // The receiver's step before it is kept within 11 bits.
    let step = (172.0 + (micros - 988.0) * 1639.0 / 1024.0).round();
    if !(0.0..=2047.0).contains(&step) {
        return Err(format!(
            "{text} µs is beyond what a receiver's Channel holds: ELRS's 11 bits run from about 881 to 2159 µs"
        ));
    }
    Ok(Channel::from_micros(micros))
}
