//! Input Tracks: recorded Flight Inputs a Scenario plays back, from a CSV
//! file beside it (#11 §2).
//!
//! The file starts with comment lines (`#`) saying where it came from, then
//! this header, then one row for each Flight Input, in time order:
//!
//! ```text
//! time (s),roll (µs),pitch (µs),throttle (µs),yaw (µs),arm (µs),flight mode (µs),crash flip (µs),event
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
//! - **A row with no values and no event** is a report from the device that
//!   changed nothing. A device that reports at rest sends one every report,
//!   and those are what keep it from counting as lost; a device that reports
//!   only changes has none.
//! - **`event`** is empty, `input device lost` (the computer reported the
//!   Flying Input Device removed), `input device back` or `reset`. A row with
//!   an event gives no Channels.
//!
//! So a row comes only when something happens: a value changes, the device
//! reports, or an event.

use opendrone_pack::{Problem, Problems};
use opendrone_sim::{Channel, Channels, FlightInput, PhysicsRate, SimulationTime};

/// The header every Input Track starts with, after its comments.
pub const INPUT_TRACK_HEADER: &str = "time (s),roll (µs),pitch (µs),throttle (µs),yaw (µs),arm (µs),flight mode (µs),crash flip (µs),event";

/// The words of the `event` column.
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
            event,
        } = read;
        let at = SimulationTime::from_ticks(tick);
        let current = match channels {
            Some(current) => current,
            None => match (tick, event, values) {
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
        match event {
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
    /// Empty, or one of the event words.
    event: &'r str,
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
            "a row has 9 cells, as the header does (the time, seven Channels and an event), not {}",
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
    let event = cells[8];
    match event {
        "" | LOST | BACK => {}
        RESET if rules.reset => {}
        RESET => {
            problem("Reset puts the Quad back where the Scenario starts, as its Launch Spot, so only a Flight Scenario that starts as Reset leaves the Quad (landed and still, disarmed, with its motors \"powering up\") has `reset`".to_string());
            fine = false;
        }
        other => {
            problem(format!(
                "`event` is empty, \"{LOST}\", \"{BACK}\" or \"{RESET}\", not \"{other}\""
            ));
            fine = false;
        }
    }
    if !event.is_empty() && values.iter().any(Option::is_some) {
        problem("a row is one Flight Input: Channels, or an event, not both".to_string());
        fine = false;
    }
    let tick = tick?;
    fine.then_some(Row {
        tick,
        values,
        event,
    })
}

/// Seconds as a whole number of steps at `hz` steps a second.
fn moment(text: &str, hz: f64) -> Option<u64> {
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
