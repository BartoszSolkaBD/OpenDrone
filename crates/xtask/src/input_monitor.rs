//! `cargo xtask input-monitor`: shows every connected Input Device live, as
//! the game reads it (#61): the input thread on SDL 3.4, the built-in Pack's
//! profiles, and the Channels each device gives, with their stamps on the
//! input thread's clock.
//!
//! It needs real devices to show anything; with none connected it says so
//! plainly and keeps watching. How the maintainer uses it is in
//! `docs/verification/watching-input-devices.md`.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use opendrone_input::channels::Channels;
use opendrone_input::transmitting::Transmitting;
use opendrone_input::{Device, InputEvent, InputThread, Inputs, Kind, Lost};
use opendrone_pack::Packs;

const USAGE: &str = "Usage: cargo xtask input-monitor [--seconds <how long>]";

/// How often the live lines are printed.
const SHOW_EVERY: Duration = Duration::from_millis(500);

/// How often the monitor takes the input thread's batches, as the game would
/// each frame.
const TAKE_EVERY: Duration = Duration::from_millis(5);

pub fn run(args: &[String]) -> ExitCode {
    let run_for = match args {
        [] => None,
        [flag, seconds] if flag == "--seconds" => match seconds.parse::<f64>() {
            Ok(s) if s > 0.0 && s.is_finite() => Some(Duration::from_secs_f64(s)),
            _ => {
                eprintln!("{USAGE}\n--seconds takes a number of seconds above zero.");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let profiles = match crate::packs::repo()
        .and_then(|root| Packs::open(&root.join("packs"), "packs").map_err(|p| p.to_string()))
    {
        Ok(packs) => {
            let problems = packs.problems();
            if !problems.is_empty() {
                println!(
                    "The Pack checker found problems; devices whose profile is broken fall back to \"Any Radio\" or \"Any Gamepad\":\n{problems}"
                );
            }
            packs.input_devices().into_iter().cloned().collect()
        }
        Err(message) => {
            eprintln!("The Packs can't be read: {message}");
            return ExitCode::from(2);
        }
    };
    let thread = match InputThread::start() {
        Ok(thread) => thread,
        Err(reason) => {
            eprintln!("The input thread couldn't start: {reason}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Watching Input Devices through SDL {} on its own thread. Times are seconds on the input thread's clock.{}",
        thread.sdl_version(),
        if run_for.is_some() {
            ""
        } else {
            " Press Ctrl+C to stop."
        }
    );
    let mut inputs = Inputs::new(profiles);
    let started = Instant::now();
    let mut said_none = false;
    let mut last_shown = Instant::now();
    let mut polls_then = thread.polls();
    let mut changes = vec![0u32; 0];
    loop {
        let mut events = Vec::new();
        for batch in thread.batches() {
            events.extend(inputs.take(batch));
        }
        events.extend(inputs.check_silence(thread.now()));
        for event in &events {
            match event {
                InputEvent::Channels { device, .. } => {
                    let index = device.0 as usize;
                    if changes.len() <= index {
                        changes.resize(index + 1, 0);
                    }
                    changes[index] += 1;
                }
                other => println!("{}", happened(&inputs, other)),
            }
        }
        let connected: Vec<&Device> = inputs
            .devices()
            .iter()
            .filter(|d| d.lost() != Some(Lost::Unplugged))
            .collect();
        if connected.is_empty() && !said_none {
            println!(
                "No Input Device is connected. Plug in a Radio or a Gamepad; the monitor keeps watching."
            );
            said_none = true;
        } else if !connected.is_empty() {
            said_none = false;
        }
        if last_shown.elapsed() >= SHOW_EVERY {
            let seconds = last_shown.elapsed().as_secs_f64();
            let polls = thread.polls();
            for device in &connected {
                let count = changes.get(device.id().0 as usize).copied().unwrap_or(0);
                println!("{}", live_line(device, f64::from(count) / seconds));
            }
            if !connected.is_empty() {
                println!(
                    "  input thread: {:.0} polls a second",
                    (polls - polls_then) as f64 / seconds
                );
            }
            changes.iter_mut().for_each(|c| *c = 0);
            polls_then = polls;
            last_shown = Instant::now();
        }
        if run_for.is_some_and(|limit| started.elapsed() >= limit) {
            break;
        }
        std::thread::sleep(TAKE_EVERY);
    }
    println!(
        "Stopped after {:.1} s. The input thread read SDL {:.0} times a second (ADR-0018 asks for about 3,000).",
        thread.now().as_secs_f64(),
        thread.polls() as f64 / thread.now().as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// A device found, lost or back, in one line.
fn happened(inputs: &Inputs, event: &InputEvent) -> String {
    let name = |id: opendrone_input::DeviceId| {
        inputs
            .device(id)
            .map_or_else(|| "?".to_string(), |d| d.info().name.clone())
    };
    match event {
        InputEvent::Found {
            device,
            at,
            name,
            kind,
            profile,
        } => {
            let info = inputs.device(*device).map(Device::info);
            let ids = info.map_or_else(String::new, |i| {
                format!(
                    " (USB {:04X}:{:04X}{})",
                    i.usb_vendor,
                    i.usb_product,
                    if i.sdl_gamepad {
                        ", SDL reads it as a gamepad"
                    } else {
                        ""
                    }
                )
            });
            format!(
                "[{:>9.3} s] Found {name}{ids}: a {}, on the profile {}",
                at.as_secs_f64(),
                kind.word(),
                profile.as_deref().unwrap_or("(none fits)")
            )
        }
        InputEvent::Lost { device, at, why } => format!(
            "[{:>9.3} s] Lost {}: {}",
            at.as_secs_f64(),
            name(*device),
            match why {
                Lost::Unplugged => "unplugged",
                Lost::Silent => "silent for 1 s",
            }
        ),
        InputEvent::Back { device, at } => {
            format!("[{:>9.3} s] Back: {}", at.as_secs_f64(), name(*device))
        }
        InputEvent::Channels { .. } => String::new(),
    }
}

/// One device's Channels now, its raw sticks and buttons, and how often its
/// Channels changed.
fn live_line(device: &Device, changes_a_second: f64) -> String {
    let c = device.channels();
    let state = device.state();
    let raw = match &state.pad {
        Some(pad) => format!(
            "sticks {:?}, triggers {:?}, buttons down {:?}",
            &pad.axes[..4],
            &pad.axes[4..],
            pad.buttons
                .iter()
                .enumerate()
                .filter(|(_, down)| **down)
                .map(|(i, _)| opendrone_input::controls::PadButton::from_index(i)
                    .map_or("?", |b| b.name()))
                .collect::<Vec<_>>()
        ),
        None => format!(
            "axes {:?}, buttons down {:?}",
            state.axes,
            state
                .buttons
                .iter()
                .enumerate()
                .filter(|(_, down)| **down)
                .map(|(i, _)| format!("CH{}", i + 9))
                .collect::<Vec<_>>()
        ),
    };
    let rf = match (device.kind(), device.transmitting()) {
        (Kind::Radio, Transmitting::Yes { closest }) => format!(
            "; seems to transmit (RF on?): changes no closer than {:.1} ms",
            closest.as_secs_f64() * 1000.0
        ),
        (Kind::Radio, Transmitting::No { .. }) => "; not transmitting".to_string(),
        _ => String::new(),
    };
    format!(
        "  {}{}: {}\n    {}; Channels changed {:.0} times a second{}{}",
        device.info().name,
        if device.calibrated() {
            ""
        } else {
            " (not calibrated)"
        },
        channels_text(&c),
        raw,
        changes_a_second,
        if device.lost() == Some(Lost::Silent) {
            "; SILENT"
        } else {
            ""
        },
        rf
    )
}

fn channels_text(c: &Channels) -> String {
    let aux = |v: Option<f64>| v.map_or_else(|| "none".to_string(), |us| format!("{us:.0}"));
    format!(
        "roll {:.1}, pitch {:.1}, throttle {:.1}, yaw {:.1} µs; AUX1 arm {}, AUX2 mode {}, AUX3 crash flip {}",
        c.roll,
        c.pitch,
        c.throttle,
        c.yaw,
        aux(c.arm),
        aux(c.flight_mode),
        aux(c.crash_flip)
    )
}
