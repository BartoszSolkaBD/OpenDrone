//! Save Flight: a developer Action that writes the flight so far as a Scenario
//! and its Input Track, so a flight the maintainer cares about becomes a test
//! (#11 §4). It has no menu entry and no default key.
//!
//! The Scenario starts landed at the Launch Spot, as Reset leaves the Quad,
//! with its starting state spelled out from the flight's own settings
//! (ADR-0002). The Input Track holds every Flight Input the game handed the
//! Simulation, each at its Simulation Time, so the headless Scenario runner
//! flies exactly the flight the game flew: the whole state after its last
//! step has the fingerprint the game's had (ADR-0001). The Scenario says that
//! fingerprint, so what the runner measures can be checked against it. The
//! game keeps only that last one: fingerprinting every step, to give the
//! run's fingerprint, would cost more than the step ([`Flight::kept`]), so
//! the readable checks do that instead.
//!
//! **From when.** The brief says "since the last Reset or arming", but a
//! flight can only be flown again to the same fingerprint from a start that is
//! really a fresh start. Reset puts the Quad back on its Launch Spot with a
//! fresh Flight Controller, but the Simulation Time, the Radio Link's beat
//! and the air's flicker go on through it (the Simulation's Reset), and
//! arming changes none of the Quad's state. So the saved flight is the whole
//! flight since the Simulation started, Resets in it as `reset` entries in
//! the `other flight input` column. The Scenario's comment says when the last
//! Reset was and when the Arm switch went on, so Expectations can name the
//! stretch since then.
//!
//! **How long.** The recorder keeps at most [`MAX_KEPT_INPUTS`] Flight
//! Inputs, so a long session doesn't use more and more memory. Because a saved
//! flight has to start where the Simulation did, a flight past that has
//! nothing it could save from, and Save Flight says it is too long.
//!
//! **Expectations are added afterwards**, but the Scenario reader refuses a
//! Scenario with none. So the Scenario carries one honest Observed
//! Expectation, the Quad's height at the end of the flight, which also ends
//! the run where the flight was saved. The maintainer replaces it.

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_input::profile::FlightMode;
use opendrone_maths::Fingerprint;
use opendrone_scenario::{INPUT_TRACK_HEADER, SCENARIO_FORMAT};
use opendrone_sim::{Channel, Channels, FlightInput, SimulationTime};

use crate::set_up::FlightPlan;
use crate::settings::Settings;
use crate::stepping::{Flight, MAX_KEPT_INPUTS};

/// How far from the written height the saved Expectation lets the Quad be:
/// wide enough that the height written to four decimals holds.
const HEIGHT_WITHIN_M: &str = "0.001 m";

/// A saved flight's two files, as text.
#[derive(Clone, Debug)]
pub struct FlightFiles {
    /// The Scenario file's text.
    pub scenario: String,
    /// The Input Track file's text.
    pub track: String,
    /// How many Simulation steps the flight lasted.
    pub steps: u64,
    /// The game's fingerprint of the Simulation's whole state after the last
    /// step, which the runner's fingerprint after that step must equal.
    pub state_fingerprint: Fingerprint,
    /// The game's fingerprint of the whole run, from the start and after
    /// every step, which the runner's Results show too: only for a flight
    /// that kept it ([`Flight::checking_every_step`]).
    pub run_fingerprint: Option<Fingerprint>,
}

/// A flight written to disk.
#[derive(Clone, Debug)]
pub struct SavedFlight {
    pub scenario: PathBuf,
    pub track: PathBuf,
    pub steps: u64,
    pub seconds: f64,
    pub state_fingerprint: Fingerprint,
}

/// Writes the flight so far to `folder` as a Scenario and its Input Track,
/// under names that aren't taken yet, such as `whoop-65-saved-flight-1.toml`
/// and `whoop-65-saved-flight-1.csv`.
pub fn save_flight(
    plan: &FlightPlan,
    flight: &Flight,
    folder: &Path,
) -> Result<SavedFlight, String> {
    fs::create_dir_all(folder)
        .map_err(|e| format!("can't make the folder {}: {e}", folder.display()))?;
    let slug = plan.quad.id.rsplit('/').next().unwrap_or(&plan.quad.id);
    let name = (1..)
        .map(|n| format!("{slug}-saved-flight-{n}"))
        .find(|name| {
            !folder.join(format!("{name}.toml")).exists()
                && !folder.join(format!("{name}.csv")).exists()
        })
        .expect("a free name is found");
    let track_name = format!("{name}.csv");
    let files = flight_files(plan, flight, &track_name)?;
    let track = folder.join(&track_name);
    let scenario = folder.join(format!("{name}.toml"));
    fs::write(&track, &files.track).map_err(|e| format!("can't write {}: {e}", track.display()))?;
    fs::write(&scenario, &files.scenario)
        .map_err(|e| format!("can't write {}: {e}", scenario.display()))?;
    let rate = plan.settings.physics_rate;
    Ok(SavedFlight {
        scenario,
        track,
        steps: files.steps,
        seconds: SimulationTime::from_ticks(files.steps).seconds(rate),
        state_fingerprint: files.state_fingerprint,
    })
}

/// The flight so far as a Scenario and an Input Track named `track_file`
/// (a file beside the Scenario).
pub fn flight_files(
    plan: &FlightPlan,
    flight: &Flight,
    track_file: &str,
) -> Result<FlightFiles, String> {
    let kept = flight.kept().ok_or_else(|| {
        "there is no flight to save yet: move a stick on an Input Device to start one".to_string()
    })?;
    if let Some(at) = kept.outgrown_at {
        return Err(format!(
            "the flight is too long to save: it passed {MAX_KEPT_INPUTS} Flight Inputs at {} s, and a saved flight has to start where the Simulation did, so the game stopped keeping them",
            seconds_short(at.ticks(), plan.settings.physics_rate.hz())
        ));
    }
    if kept.steps == 0 {
        return Err("the flight hasn't taken a step yet".to_string());
    }
    let settings = &plan.settings;
    let hz = settings.physics_rate.hz();

    // What the Simulation has taken: every input stamped before the step it
    // is at. Later ones are still waiting, and change nothing yet.
    let taken: Vec<&(SimulationTime, FlightInput)> = kept
        .inputs
        .iter()
        .filter(|(at, _)| at.ticks() < kept.steps)
        .collect();
    let track = input_track(&taken, hz)?;

    let seconds = seconds_text(kept.steps, hz);
    let end = flight.simulation().quad_state(0).position.z;
    let height = format!("{end:.4}");
    let history = history(&taken, hz);
    let launch = plan.launch_spot();
    let device = kept.device;
    let report_rate = device
        .report_rate
        .map_or_else(|| "unknown".to_string(), |r| format!("{} Hz", r.hz()));
    let name = format!(
        "A saved flight of the {} for {} s",
        plan.quad.name,
        seconds_short(kept.steps, hz)
    );
    let on_off = |on: bool| if on { "on" } else { "off" };

    let mut text = String::new();
    text.push_str(&format!(
        "\
# Saved with Save Flight, the game's developer Action: the whole flight from
# the moment it started to {seconds} s, as the game flew it. It starts landed
# at the Launch Spot as Reset leaves the Quad, and its Input Track holds every
# Flight Input the game handed the Simulation, so the headless runner flies
# it exactly as the game did. In the game the whole state's fingerprint after
# step {steps} was {fingerprint}: the line for step {steps} in the file
# `cargo scenarios check --fingerprints <folder>` writes for this Scenario must
# say the same.
{history}#
# The one Expectation below only pins where the Quad ended, which also ends the
# run where the flight was saved. Replace it with what this flight should
# prove, each with its Basis.
format = {format}
name   = \"{name}\"

[start]
kind              = \"flight\"
quad              = \"{quad}\"
map               = \"{map}\"
position          = \"{east} m east, {north} m north, {up} m up\"
attitude          = \"level, heading 0°\"
speed             = \"0 m/s\"
rotation          = \"0 °/s\"
armed             = false
motors            = \"powering up\"
flight_controller = \"fresh\"
battery           = \"100%\"
flight_mode       = \"{flight_mode}\"
assists           = {{ input_smoothing = \"{smoothing}\", endless_battery = \"{endless}\", auto_arm = \"{auto_arm}\" }}
radio_link        = \"{packet} Hz\"
physics_rate      = \"{physics}\"
random_seed       = {seed}

",
        steps = kept.steps,
        fingerprint = kept.state_fingerprint,
        format = SCENARIO_FORMAT,
        name = escaped(&name),
        quad = plan.quad.id,
        map = plan.map.id,
        east = launch.position.x,
        north = launch.position.y,
        up = launch.position.z,
        flight_mode = flight_mode_word(settings.flight_mode),
        smoothing = on_off(settings.input_smoothing),
        endless = on_off(settings.endless_battery),
        auto_arm = on_off(settings.auto_arm),
        packet = settings.packet_rate.hz(),
        physics = rate_words(hz),
        seed = settings.random_seed,
    ));
    text.push_str(&rates_section(settings));
    text.push_str(&format!(
        "
[inputs]
input_track  = \"{track_file}\"
input_device = {{ report_rate = \"{report_rate}\", reports_at_rest = {at_rest} }}

[[expect]]
what  = \"height\"
at    = \"{seconds} s\"
value = \"{height} m ± {HEIGHT_WITHIN_M}\"
basis = \"observed: where the Quad was when the flight was saved; a placeholder until this flight's own Expectations are written\"
",
        at_rest = device.reports_at_rest,
    ));
    Ok(FlightFiles {
        scenario: text,
        track,
        steps: kept.steps,
        state_fingerprint: kept.state_fingerprint,
        run_fingerprint: kept.run_fingerprint,
    })
}

/// The Input Track's text: a row for every Flight Input, Channels as the
/// ones that changed, as the format has it (`opendrone_scenario`'s `track`).
fn input_track(inputs: &[&(SimulationTime, FlightInput)], hz: u32) -> Result<String, String> {
    let mut text = String::from(
        "\
# Saved with Save Flight, the game's developer Action: the Flight Inputs of one flight.
# Times are Simulation Time from the flight's start; Channels in µs.
",
    );
    text.push_str(INPUT_TRACK_HEADER);
    text.push('\n');
    let mut last: Option<Channels> = None;
    for (index, &&(at, input)) in inputs.iter().enumerate() {
        let time = seconds_text(at.ticks(), hz);
        match input {
            FlightInput::Channels(channels) => {
                let cells = match last {
                    None if index == 0 && at == SimulationTime::START => {
                        all_channels(&channels).map(Some)
                    }
                    None => {
                        return Err("the flight's first Flight Input should be its Channels at 0 s, but it isn't".to_string());
                    }
                    Some(before) => changed_channels(&before, &channels),
                };
                let cells: Vec<String> = cells.into_iter().map(Option::unwrap_or_default).collect();
                text.push_str(&format!("{time},{},\n", cells.join(",")));
                last = Some(channels);
            }
            _ if last.is_none() => {
                return Err(
                    "the flight's first Flight Input should be its Channels at 0 s, but it isn't"
                        .to_string(),
                );
            }
            FlightInput::InputDeviceLost => {
                text.push_str(&format!("{time},,,,,,,,input device lost\n"))
            }
            FlightInput::InputDeviceBack => {
                text.push_str(&format!("{time},,,,,,,,input device back\n"))
            }
            FlightInput::Reset => text.push_str(&format!("{time},,,,,,,,reset\n")),
        }
    }
    Ok(text)
}

/// All seven Channels, in the Input Track's column order.
fn all_channels(channels: &Channels) -> [String; 7] {
    [
        micros_text(channels.roll),
        micros_text(channels.pitch),
        micros_text(channels.throttle),
        micros_text(channels.yaw),
        micros_text(channels.arm),
        micros_text(channels.flight_mode),
        micros_text(channels.crash_flip),
    ]
}

/// The Channels that differ from `before`, in the Input Track's column
/// order, and nothing for the ones that don't.
fn changed_channels(before: &Channels, now: &Channels) -> [Option<String>; 7] {
    let (was, is) = (all_channels(before), all_channels(now));
    let mut cells: [Option<String>; 7] = Default::default();
    for ((cell, was), is) in cells.iter_mut().zip(was).zip(is) {
        // A Channel is a whole receiver step, and its text keeps the step, so
        // equal text is an equal Channel.
        if was != is {
            *cell = Some(is);
        }
    }
    cells
}

/// A Channel in µs as the Input Track writes it: the value an Input Device's
/// Calibration gives, which [`Channel::from_micros`] turns back into this
/// receiver step. (That is not the µs Betaflight reads from the step,
/// [`Channel::micros`].)
fn micros_text(channel: Channel) -> String {
    let micros = 988.0 + (f64::from(channel.step()) - 172.0) * 1024.0 / 1639.0;
    trimmed(format!("{micros:.3}"))
}

/// When the Resets were and when the Arm switch went on and off, as comment
/// lines for the Scenario, ending with a newline: the stretches Expectations
/// may want to name.
fn history(inputs: &[&(SimulationTime, FlightInput)], hz: u32) -> String {
    let mut resets = Vec::new();
    let mut arm_changes: Vec<(String, bool)> = Vec::new();
    let mut armed_switch = false;
    for &&(at, input) in inputs {
        match input {
            FlightInput::Reset => resets.push(seconds_text(at.ticks(), hz)),
            FlightInput::Channels(channels) => {
                let on = channels.arm.micros() > 1500.0;
                if on != armed_switch {
                    armed_switch = on;
                    arm_changes.push((seconds_text(at.ticks(), hz), on));
                }
            }
            FlightInput::InputDeviceLost | FlightInput::InputDeviceBack => {}
        }
    }
    let mut lines = String::new();
    lines.push_str("#\n");
    lines.push_str(&if resets.is_empty() {
        "# Reset: never pressed, so the flight is one stretch from the start.\n".to_string()
    } else {
        format!("# Reset pressed at {} s.\n", resets.join(" s, "))
    });
    lines.push_str(&if arm_changes.is_empty() {
        "# The Arm switch never went on.\n".to_string()
    } else {
        let words: Vec<String> = arm_changes
            .iter()
            .map(|(time, on)| format!("{} at {time} s", if *on { "on" } else { "off" }))
            .collect();
        format!("# The Arm switch went {}.\n", words.join(", "))
    });
    lines
}

/// `[start.rates]`: every field of the Rates, as the Betaflight App shows
/// it, with a blank line before it.
fn rates_section(settings: &Settings) -> String {
    use opendrone_flight_controller::{RatesType, ThrottleLimitType};
    let rates = &settings.rates;
    let (type_name, labels): (&str, [(&str, Shown); 3]) = match rates.rates_type {
        RatesType::Betaflight => (
            "Betaflight",
            [
                ("rc rate", Shown::Hundredths),
                ("rate", Shown::Hundredths),
                ("rc expo", Shown::Hundredths),
            ],
        ),
        RatesType::Raceflight => (
            "Raceflight",
            [
                ("rate", Shown::TensOfDegrees),
                ("acro+", Shown::Percent),
                ("expo", Shown::Percent),
            ],
        ),
        RatesType::Kiss => (
            "KISS",
            [
                ("rc rate", Shown::Hundredths),
                ("rate", Shown::Hundredths),
                ("rc curve", Shown::Hundredths),
            ],
        ),
        RatesType::Actual => (
            "Actual",
            [
                ("center sensitivity", Shown::TensOfDegrees),
                ("max rate", Shown::TensOfDegrees),
                ("expo", Shown::Hundredths),
            ],
        ),
        RatesType::Quick => (
            "Quick",
            [
                ("rc rate", Shown::Hundredths),
                ("max rate", Shown::TensOfDegrees),
                ("expo", Shown::Hundredths),
            ],
        ),
    };
    let axis = |axis: opendrone_flight_controller::AxisRates| {
        [axis.rc_rate, axis.srate, axis.expo]
            .iter()
            .zip(labels)
            .map(|(&stored, (label, shown))| format!("{label} {}", shown.show(stored)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let [roll, pitch, yaw] = rates.rate_limit;
    let rate_limit = if roll == pitch && pitch == yaw {
        format!("{roll} °/s")
    } else {
        format!("roll {roll} °/s, pitch {pitch} °/s, yaw {yaw} °/s")
    };
    let limit = match rates.throttle_limit_type {
        ThrottleLimitType::Off => "off".to_string(),
        ThrottleLimitType::Scale => format!("scale {}%", rates.throttle_limit_percent),
        ThrottleLimitType::Clip => format!("clip {}%", rates.throttle_limit_percent),
    };
    let hundredths = |stored: u8| Shown::Hundredths.show(stored);
    format!(
        "\
[start.rates]
type               = \"{type_name}\"
roll               = \"{}\"
pitch              = \"{}\"
yaw                = \"{}\"
rate_limit         = \"{rate_limit}\"
throttle           = \"mid {}, hover {}, expo {}, limit {limit}\"
quickrates_rc_expo = \"{}\"
",
        axis(rates.roll),
        axis(rates.pitch),
        axis(rates.yaw),
        hundredths(rates.thr_mid),
        hundredths(rates.thr_hover),
        hundredths(rates.thr_expo),
        if rates.quickrates_rc_expo {
            "on"
        } else {
            "off"
        },
    )
}

/// How the Betaflight App shows one stored rate number.
#[derive(Clone, Copy)]
enum Shown {
    /// The stored number ÷ 100, with two decimals.
    Hundredths,
    /// The stored number × 10, in °/s.
    TensOfDegrees,
    /// The stored number, in %.
    Percent,
}

impl Shown {
    fn show(self, stored: u8) -> String {
        match self {
            Shown::Hundredths => format!("{}.{:02}", stored / 100, stored % 100),
            Shown::TensOfDegrees => format!("{} °/s", u32::from(stored) * 10),
            Shown::Percent => format!("{stored}%"),
        }
    }
}

fn flight_mode_word(mode: FlightMode) -> &'static str {
    match mode {
        FlightMode::Acro => "Acro",
        FlightMode::Horizon => "Horizon",
        FlightMode::Angle => "Angle",
    }
}

/// A physics rate as the Scenario format writes it: "8 kHz".
fn rate_words(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{hz} Hz")
    }
}

/// A time in seconds from steps, written exactly enough that it's a whole
/// number of steps again, such as `0.001125`.
fn seconds_text(ticks: u64, hz: u32) -> String {
    trimmed(format!("{:.12}", ticks as f64 / f64::from(hz)))
}

/// A time in seconds for people, to a millisecond.
fn seconds_short(ticks: u64, hz: u32) -> String {
    trimmed(format!("{:.3}", ticks as f64 / f64::from(hz)))
}

/// A number's text without zeros after its decimal point, and without the
/// point when nothing follows it.
fn trimmed(text: String) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// A TOML basic string's inside.
fn escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
