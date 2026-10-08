//! The Input Tracks in `scenarios/recorded/` are the recorded device traces
//! in `tests/traces/`, played through the built-in Pack's Input Device
//! profiles exactly as the game hands a live device's Channels to the
//! Simulation (#56). This check makes each track afresh and compares it with
//! the committed file, so a track can't drift from its trace or from the
//! input layer.
//!
//! To write them again, after a change to a trace or to how Channels are
//! made: `OPENDRONE_WRITE_INPUT_TRACKS=1 cargo test -p opendrone-input --test
//! input_tracks`, then run the Scenarios.
//!
//! The format is the Scenario runner's (`opendrone-scenario`, `track.rs`,
//! and `docs/verification/reading-a-scenario.md`): a row for each Flight
//! Input, giving only the Channels that changed, in µs; an empty row for a
//! report from a device that reports at rest that changed nothing; and the
//! device unplugged and plugged back in as events. A device that reports at
//! rest going silent is no event: the Simulation counts the silence itself.
//! Times are Simulation Time at 8 kHz from the trace's first poll, each
//! report entering at the first step at or after the input thread read it.

mod common;

use std::fmt::Write as _;
use std::time::Duration;

use common::Trace;
use opendrone_input::{InputEvent, Inputs, Lost};

/// Each Input Track: its file in `scenarios/recorded/`, the trace it comes
/// from, and what it shows.
const TRACKS: [(&str, &str, &str); 4] = [
    (
        "dualsense-at-rest.csv",
        "dualsense-at-rest-27.csv",
        "The DualSense over USB on the desk, untouched, its motion sensors on: a report every 4 ms that changes nothing, its heartbeat.",
    ),
    (
        "dualsense-fast-circles.csv",
        "dualsense-let-go-27.csv",
        "The DualSense over USB: the end of fast circles on both sticks, then hands off. Its motion sensors were off, so it has no heartbeat and reports only changes.",
    ),
    (
        "pocket-slow-circles.csv",
        "pocket-adc-filter-off-30.csv",
        "The Radiomaster Pocket over USB, RF off, its ADC filter Off: two seconds of slow circles.",
    ),
    (
        "pocket-rest-and-unplug.csv",
        "pocket-rf-off-18.csv",
        "The Radiomaster Pocket over USB, RF off: plugged in and left alone for 11 s, then fast circles, switches and buttons, then unplugged.",
    ),
];

/// The physics rate the tracks are written at.
const PHYSICS_HZ: u128 = 8000;

#[test]
fn the_input_tracks_are_the_recorded_traces_through_the_built_in_profiles() {
    let write = std::env::var_os("OPENDRONE_WRITE_INPUT_TRACKS").is_some();
    let folder = common::repo().join("scenarios/recorded");
    let mut stale = Vec::new();
    for (name, trace, about) in TRACKS {
        let fresh = input_track(trace, about);
        let path = folder.join(name);
        if write {
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(&path, &fresh).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(fresh.as_str()) {
            stale.push(name);
        }
    }
    assert!(
        stale.is_empty(),
        "out of date in scenarios/recorded/: {stale:?}. Write them again with `OPENDRONE_WRITE_INPUT_TRACKS=1 cargo test -p opendrone-input --test input_tracks`"
    );
}

/// One trace as an Input Track.
fn input_track(file: &str, about: &str) -> String {
    let trace = Trace::read(file);
    let mut inputs = Inputs::new(common::built_in_profiles());
    let batches = trace.batches();
    let start = batches.first().map_or(Duration::ZERO, |b| b.at);
    let heartbeat = trace.device.heartbeat;
    let mut text = String::new();
    for line in about.split(". ") {
        let line = line.trim_end_matches('.');
        writeln!(text, "# {line}.").unwrap();
    }
    writeln!(
        text,
        "# Made from crates/input/tests/traces/{file} (its header says which run and stretch), through the"
    )
    .unwrap();
    writeln!(
        text,
        "# built-in Pack's Input Device profile, uncalibrated, by crates/input/tests/input_tracks.rs."
    )
    .unwrap();
    writeln!(
        text,
        "# Device: {}; heartbeat: {}.",
        trace.device.name,
        if heartbeat { "on" } else { "off" }
    )
    .unwrap();
    writeln!(
        text,
        "# Times are Simulation Time at 8 kHz from the trace's first poll; Channels in µs."
    )
    .unwrap();
    writeln!(
        text,
        "time (s),roll (µs),pitch (µs),throttle (µs),yaw (µs),arm (µs),flight mode (µs),crash flip (µs),event"
    )
    .unwrap();
    let mut written: Option<[String; 7]> = None;
    let mut unplugged = false;
    for batch in batches {
        let at = batch.at - start;
        let events = inputs.take(batch);
        // The first step at or after the poll that read it.
        let ticks = (at.as_nanos() * PHYSICS_HZ).div_ceil(1_000_000_000);
        let time = seconds(ticks);
        let mut changed: Option<[String; 7]> = None;
        for event in &events {
            match event {
                InputEvent::Channels { channels, .. } => {
                    changed = Some(
                        [
                            Some(channels.roll),
                            Some(channels.pitch),
                            Some(channels.throttle),
                            Some(channels.yaw),
                            channels.arm,
                            channels.flight_mode,
                            channels.crash_flip,
                        ]
                        // A switch with no source is low: Arm off, Acro,
                        // Crash Flip off, as the game sets them.
                        .map(|us| micros(us.unwrap_or(988.0))),
                    );
                }
                InputEvent::Lost {
                    why: Lost::Unplugged,
                    ..
                } => {
                    unplugged = true;
                    writeln!(text, "{time},,,,,,,,input device lost").unwrap();
                }
                InputEvent::Back { .. } if unplugged => {
                    unplugged = false;
                    writeln!(text, "{time},,,,,,,,input device back").unwrap();
                }
                _ => {}
            }
        }
        match (changed, &written) {
            (Some(values), None) => {
                writeln!(text, "{time},{},", values.join(",")).unwrap();
                written = Some(values);
            }
            (Some(values), Some(before)) => {
                let cells: Vec<&str> = values
                    .iter()
                    .zip(before)
                    .map(|(now, was)| if now == was { "" } else { now.as_str() })
                    .collect();
                if cells.iter().any(|c| !c.is_empty()) || (heartbeat && !unplugged) {
                    writeln!(text, "{time},{},", cells.join(",")).unwrap();
                }
                written = Some(values);
            }
            // A report that changed nothing: a Flight Input only from a
            // device that reports at rest.
            (None, Some(_)) if heartbeat && !unplugged => {
                writeln!(text, "{time},,,,,,,,").unwrap();
            }
            _ => {}
        }
    }
    text
}

/// Steps at 8 kHz as seconds, written plainly: exact, since a step is
/// 0.000125 s.
fn seconds(ticks: u128) -> String {
    let whole = ticks / PHYSICS_HZ;
    let rest = ticks % PHYSICS_HZ;
    if rest == 0 {
        format!("{whole}")
    } else {
        let micros = rest * 125;
        let text = format!("{whole}.{micros:06}");
        text.trim_end_matches('0').to_string()
    }
}

/// µs to three decimals, without trailing zeros.
fn micros(us: f64) -> String {
    let text = format!("{us:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}
