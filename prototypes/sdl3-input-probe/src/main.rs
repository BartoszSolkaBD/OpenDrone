//! PROTOTYPE (throwaway) for OpenDrone issue #18:
//! "Does an SDL3 input thread deliver full-rate, unfiltered samples from the DualSense and
//! Pocket alongside Bevy?"
//!
//! A minimal Bevy app owns the macOS main thread and keeps a window rendering. SDL 3.4 runs its
//! joystick/gamepad loop on a separate plain thread, records every raw change with SDL's
//! timestamp, and a guided script tells the pilot what to do with the sticks. At the end it
//! writes `summary.md`, `stats.json` and `events.csv` into `results/<label>-<time>/`.
//!
//! Run: `cargo run --release -- --label pocket-usb` (or `./run.sh pocket-usb` to launch it as
//! its own .app, which is the honest test for macOS permission prompts).

mod input_thread;
mod report;

use bevy::log::{Level, LogPlugin};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use input_thread::{Live, ThreadConfig, ThreadResult};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const QUESTION: &str = "Does an SDL3 input thread deliver full-rate, unfiltered samples from the DualSense and Pocket alongside Bevy?";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Phase {
    Connect = 0,
    Rest = 1,
    Circles = 2,
    Yaw = 3,
    Extremes = 4,
    Switches = 5,
    Unfocused = 6,
    Sensors = 7,
    Done = 8,
    GetReady = 9,
}

impl Phase {
    pub const ALL: [Phase; 10] = [
        Phase::Connect,
        Phase::Rest,
        Phase::Circles,
        Phase::Yaw,
        Phase::Extremes,
        Phase::Switches,
        Phase::Unfocused,
        Phase::Sensors,
        Phase::Done,
        Phase::GetReady,
    ];
    pub fn from_u8(v: u8) -> Phase {
        Phase::ALL.into_iter().find(|p| *p as u8 == v).unwrap_or(Phase::GetReady)
    }
    pub fn key(self) -> &'static str {
        match self {
            Phase::Connect => "connect",
            Phase::Rest => "rest",
            Phase::Circles => "circles",
            Phase::Yaw => "yaw",
            Phase::Extremes => "extremes",
            Phase::Switches => "switches",
            Phase::Unfocused => "unfocused",
            Phase::Sensors => "sensors",
            Phase::Done => "done",
            Phase::GetReady => "get-ready",
        }
    }
    fn prompt(self) -> &'static str {
        match self {
            Phase::Connect => "Click once on this window so it is in front.\nPlug in or switch on the Input Device now. The probe starts by itself once it sees one.\n(Press SPACE to start without a device. ESC at any time stops early and still saves results.)",
            Phase::Rest => "HANDS OFF. Let both sticks rest at centre (throttle can stay where it is).",
            Phase::Circles => "Move BOTH sticks in FAST, continuous, full circles. Keep going until the timer ends.",
            Phase::Yaw => "Move ONLY the yaw stick: left stick, left-right (Mode 2). Back and forth, fast. Nothing else.",
            Phase::Extremes => "SLOWLY trace the full edge of each stick (all corners), twice round.",
            Phase::Switches => "Flip EVERY switch through every position, turn the pot/knob,\npress every button, squeeze both triggers.",
            Phase::Unfocused => "CLICK ON THE TERMINAL WINDOW (so this window loses focus),\nthen keep moving both sticks in fast circles.",
            Phase::Sensors => "Motion sensors are now ON. Move BOTH sticks in FAST, continuous circles again.",
            Phase::Done => "Done. Writing results...",
            Phase::GetReady => "",
        }
    }
    fn seconds(self, quick: bool) -> f32 {
        if quick {
            return 1.5;
        }
        match self {
            Phase::Rest => 5.0,
            Phase::Circles => 12.0,
            Phase::Yaw => 6.0,
            Phase::Extremes => 12.0,
            Phase::Switches => 15.0,
            Phase::Unfocused => 10.0,
            Phase::Sensors => 12.0,
            _ => 0.0,
        }
    }
}

const STEPS: [Phase; 9] = [
    Phase::Connect,
    Phase::Rest,
    Phase::Circles,
    Phase::Yaw,
    Phase::Extremes,
    Phase::Switches,
    Phase::Unfocused,
    Phase::Sensors,
    Phase::Done,
];

struct Args {
    label: String,
    quick: bool,
    gilrs: bool,
    poll_us: u64,
    qos: bool,
    selftest: bool,
    out_root: PathBuf,
}

fn parse_args() -> Args {
    let mut a = Args {
        label: "run".into(),
        quick: false,
        gilrs: true,
        poll_us: 250,
        qos: true,
        selftest: false,
        out_root: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/results")),
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--label" => a.label = it.next().expect("--label needs a value"),
            "--quick" => a.quick = true,
            "--no-gilrs" => a.gilrs = false,
            "--no-qos" => a.qos = false,
            "--selftest" => a.selftest = true,
            "--poll-us" => a.poll_us = it.next().and_then(|v| v.parse().ok()).expect("--poll-us needs a number"),
            "--out" => a.out_root = PathBuf::from(it.next().expect("--out needs a path")),
            "-h" | "--help" => {
                println!("sdl3-input-probe [--label NAME] [--quick] [--no-gilrs] [--no-qos] [--selftest] [--poll-us N] [--out DIR]");
                std::process::exit(0);
            }
            other if other.starts_with("-psn_") => {} // macOS Finder launch argument
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

// ---------- macOS permission status (reads only; never prompts) ----------

#[cfg(target_os = "macos")]
fn input_monitoring_status() -> String {
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOHIDCheckAccess(request_type: u32) -> u32;
    }
    // kIOHIDRequestTypeListenEvent = 1; kIOHIDAccessTypeGranted = 0, Denied = 1, Unknown = 2
    match unsafe { IOHIDCheckAccess(1) } {
        0 => "Granted".into(),
        1 => "Denied".into(),
        2 => "Unknown (never asked)".into(),
        x => format!("code {x}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn input_monitoring_status() -> String {
    "n/a (not macOS)".into()
}

fn os_description() -> String {
    if cfg!(target_os = "macos") {
        let v = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        format!("macOS {v} ({})", std::env::consts::ARCH)
    } else {
        format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn utc_now() -> (String, String) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    // Howard Hinnant's civil-from-days.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let (hh, mm, ss) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    (
        format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}"),
        format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"),
    )
}

// ---------- Bevy side ----------

#[derive(Resource)]
struct Shared {
    t0: Instant,
    phase: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
    enable_sensors: Arc<AtomicBool>,
    live: Arc<Mutex<Live>>,
    thread: Mutex<Option<JoinHandle<ThreadResult>>>,
    poll_us: u64,
    qos: bool,
    selftest: bool,
}

#[derive(Resource)]
struct Run {
    label: String,
    quick: bool,
    gilrs: bool,
    out_root: PathBuf,
    started_utc: String,
    stamp: String,
    hid_start: String,
    app_bundle: bool,
}

#[derive(Resource)]
struct Guide {
    idx: usize,
    step_started: Instant,
    lead_in: bool,
    announced: (usize, bool),
    device_seen_at: Option<Instant>,
    done_at: Option<Instant>,
    result_line: String,
}

#[derive(Resource, Default)]
struct Frames(BTreeMap<String, report::FramePhase>);

#[derive(Component)]
struct Hud;

fn main() {
    let args = parse_args();
    let (started_utc, stamp) = utc_now();
    let app_bundle = std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false);
    let hid_start = input_monitoring_status();

    println!("PROTOTYPE sdl3-input-probe for issue #18");
    println!("Question: {QUESTION}");
    println!(
        "Run label: {} | quick: {} | Bevy gilrs: {} | poll sleep: {} us | Input Monitoring at start: {}",
        args.label, args.quick, args.gilrs, args.poll_us, hid_start
    );

    let shared = Shared {
        t0: Instant::now(),
        phase: Arc::new(AtomicU8::new(Phase::Connect as u8)),
        stop: Arc::new(AtomicBool::new(false)),
        enable_sensors: Arc::new(AtomicBool::new(false)),
        live: Arc::new(Mutex::new(Live::default())),
        thread: Mutex::new(None),
        poll_us: args.poll_us,
        qos: args.qos,
        selftest: args.selftest,
    };
    if args.selftest {
        println!("SELF-TEST: a virtual joystick feeds synthetic values. Nothing in this run is a hardware measurement.");
    }

    let mut plugins = DefaultPlugins
        .set(LogPlugin { level: Level::WARN, ..default() })
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("SDL3 Input Probe (PROTOTYPE) - {}", args.label),
                resolution: (1200u32, 760u32).into(),
                ..default()
            }),
            close_when_requested: false,
            ..default()
        })
        .build();
    if !args.gilrs {
        plugins = plugins.disable::<bevy::gilrs::GilrsPlugin>();
    }

    App::new()
        .add_plugins(plugins)
        .insert_resource(ClearColor(Color::srgb(0.07, 0.08, 0.10)))
        .insert_resource(shared)
        .insert_resource(Run {
            label: args.label,
            quick: args.quick,
            gilrs: args.gilrs,
            out_root: args.out_root,
            started_utc,
            stamp,
            hid_start,
            app_bundle,
        })
        .insert_resource(Guide {
            idx: 0,
            step_started: Instant::now(),
            lead_in: false,
            announced: (usize::MAX, false),
            device_seen_at: None,
            done_at: None,
            result_line: String::new(),
        })
        .init_resource::<Frames>()
        .add_systems(Startup, (setup, start_input_thread))
        .add_systems(Update, (guide, hud, frame_stats))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont::from_font_size(17.0),
        TextColor(Color::srgb(0.92, 0.93, 0.95)),
        Node { position_type: PositionType::Absolute, top: Val::Px(16.0), left: Val::Px(18.0), ..default() },
    ));
}

/// Started from a Bevy system, i.e. after winit has taken the main thread and opened the window.
fn start_input_thread(shared: Res<Shared>) {
    let handle = input_thread::spawn(ThreadConfig {
        poll_sleep_us: shared.poll_us,
        qos: shared.qos,
        t0: shared.t0,
        phase: shared.phase.clone(),
        stop: shared.stop.clone(),
        enable_sensors: shared.enable_sensors.clone(),
        live: shared.live.clone(),
        selftest: shared.selftest,
    });
    *shared.thread.lock().unwrap() = Some(handle);
}

fn lead_in_secs(quick: bool) -> f32 {
    if quick { 0.5 } else { 2.5 }
}

#[allow(clippy::too_many_arguments)]
fn guide(
    mut g: ResMut<Guide>,
    shared: Res<Shared>,
    run: Res<Run>,
    frames: Res<Frames>,
    keys: Res<ButtonInput<KeyCode>>,
    mut close: MessageReader<WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = Instant::now();
    let step = STEPS[g.idx];

    if let Some(t) = g.done_at {
        if now - t > Duration::from_secs_f32(if run.quick { 1.0 } else { 5.0 }) {
            exit.write(AppExit::Success);
        }
        return;
    }

    let abort = keys.just_pressed(KeyCode::Escape) || close.read().count() > 0;
    let mut advance = false;
    let elapsed = (now - g.step_started).as_secs_f32();

    if abort && step != Phase::Done {
        g.idx = STEPS.len() - 1;
        g.step_started = now;
        g.lead_in = false;
        return;
    }

    match step {
        Phase::Connect => {
            shared.phase.store(Phase::Connect as u8, Ordering::Relaxed);
            let has_dev = shared.live.lock().map(|l| !l.devices.is_empty()).unwrap_or(false);
            if has_dev && g.device_seen_at.is_none() {
                g.device_seen_at = Some(now);
            }
            if keys.just_pressed(KeyCode::Space)
                || g.device_seen_at.is_some_and(|t| now - t > Duration::from_secs(if run.quick { 1 } else { 3 }))
                || (run.quick && elapsed > 3.0)
            {
                advance = true;
            }
            if g.announced != (g.idx, false) {
                g.announced = (g.idx, false);
                println!("\n[step 1/{}] {}", STEPS.len() - 1, step.prompt());
            }
        }
        Phase::Done => {
            shared.phase.store(Phase::Done as u8, Ordering::Relaxed);
            shared.stop.store(true, Ordering::Relaxed);
            println!("\n{}", step.prompt());
            let handle = shared.thread.lock().unwrap().take();
            let result = handle.map(|h| h.join().expect("input thread panicked"));
            let hid_end = input_monitoring_status();
            match result {
                Some(result) => {
                    let dir = run.out_root.join(format!("{}-{}", run.label, run.stamp));
                    let meta = report::RunMeta {
                        label: run.label.clone(),
                        started_utc: run.started_utc.clone(),
                        os: os_description(),
                        quick: run.quick,
                        bevy_gilrs_enabled: run.gilrs,
                        launched_as_app_bundle: run.app_bundle,
                        input_monitoring_at_start: run.hid_start.clone(),
                        input_monitoring_at_end: hid_end,
                        duration_s: shared.t0.elapsed().as_secs_f64(),
                    };
                    match report::write_all(&dir, &meta, &result, &frames.0) {
                        Ok(short) => {
                            println!("\n{short}");
                            println!("RESULTS WRITTEN TO: {}", dir.display());
                            println!("  summary: {}", dir.join("summary.md").display());
                            g.result_line = format!("Results written to:\n{}", dir.display());
                        }
                        Err(e) => {
                            eprintln!("could not write results: {e}");
                            g.result_line = format!("Could not write results: {e}");
                        }
                    }
                }
                None => g.result_line = "Input thread was never started.".into(),
            }
            g.done_at = Some(now);
            return;
        }
        _ => {
            let skip = step == Phase::Sensors && !shared.live.lock().map(|l| l.any_sensors).unwrap_or(false);
            if skip {
                advance = true;
            } else if g.lead_in {
                shared.phase.store(Phase::GetReady as u8, Ordering::Relaxed);
                if step == Phase::Sensors {
                    shared.enable_sensors.store(true, Ordering::Relaxed);
                }
                if g.announced != (g.idx, true) {
                    g.announced = (g.idx, true);
                    println!("\n[step {}/{}] GET READY: {}", g.idx + 1, STEPS.len() - 1, step.prompt().replace('\n', " "));
                }
                if elapsed >= lead_in_secs(run.quick) {
                    g.lead_in = false;
                    g.step_started = now;
                }
            } else {
                shared.phase.store(step as u8, Ordering::Relaxed);
                if g.announced != (g.idx, false) {
                    g.announced = (g.idx, false);
                    println!("[step {}/{}] GO ({} s)", g.idx + 1, STEPS.len() - 1, step.seconds(run.quick));
                }
                if elapsed >= step.seconds(run.quick) {
                    advance = true;
                }
            }
        }
    }

    if advance {
        g.idx += 1;
        g.step_started = now;
        g.lead_in = STEPS[g.idx] != Phase::Done;
    }
}

fn bar(v: i16) -> String {
    let width = 21usize;
    let pos = (((v as i32 + 32768) as f32 / 65535.0) * (width - 1) as f32).round() as usize;
    (0..width).map(|i| if i == pos { '#' } else if i == width / 2 { '|' } else { '-' }).collect()
}

fn hud(
    g: Res<Guide>,
    shared: Res<Shared>,
    run: Res<Run>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut text: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let live = shared.live.lock().map(|l| l.clone()).unwrap_or_default();
    let step = STEPS[g.idx];
    let elapsed = g.step_started.elapsed().as_secs_f32();
    let focused = windows.single().map(|w| w.focused).unwrap_or(false);

    let mut s = String::new();
    s.push_str(&format!("SDL3 INPUT PROBE (prototype for issue #18)   run: {}\n\n", run.label));
    match step {
        Phase::Done => {
            s.push_str("DONE.\n");
            s.push_str(&g.result_line);
            s.push('\n');
        }
        Phase::Connect => {
            s.push_str(&format!("Step 1 of {}:\n{}\n", STEPS.len() - 1, step.prompt()));
        }
        _ if g.lead_in => {
            let left = (lead_in_secs(run.quick) - elapsed).max(0.0);
            s.push_str(&format!("Step {} of {}: GET READY ({:.0} s)\n{}\n", g.idx + 1, STEPS.len() - 1, left.ceil(), step.prompt()));
        }
        _ => {
            let left = (step.seconds(run.quick) - elapsed).max(0.0);
            s.push_str(&format!(">>> NOW ({:.0} s left) <<<\n{}\n", left.ceil(), step.prompt()));
        }
    }

    s.push_str(&format!(
        "\nWindow: {:.0} fps, {}   |   Input thread: {}  polling {:.0} times/s, longest gap {:.2} ms   |   sensors {}\n",
        1.0 / time.delta_secs().max(1e-6),
        if focused { "focused" } else { "NOT focused" },
        live.init.as_ref().map(|i| if i.sdl_init_ok { format!("SDL {} running", i.sdl_version) } else { format!("SDL FAILED: {}", i.sdl_error) }).unwrap_or("starting".into()),
        live.poll_hz,
        live.poll_max_gap_ms,
        if live.sensors_on { "ON" } else { "off" },
    ));

    if live.devices.is_empty() {
        s.push_str("\nNo Input Device detected yet.\n");
    }
    for d in &live.devices {
        s.push_str(&format!(
            "\n{}   [{}]\n  stick updates/s: {:.0}{}\n",
            d.name,
            d.summary,
            d.updates_per_s,
            if d.sensor_per_s > 0.0 { format!("   gyro reports/s: {:.0}", d.sensor_per_s) } else { String::new() }
        ));
        for (i, v) in d.axes.iter().enumerate() {
            s.push_str(&format!("  axis {i}: {:>6}  {}\n", v, bar(*v)));
        }
        let pressed: Vec<String> =
            d.buttons.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i.to_string()).collect();
        s.push_str(&format!(
            "  buttons ({}): pressed [{}]{}\n",
            d.buttons.len(),
            pressed.join(" "),
            if d.hats.is_empty() { String::new() } else { format!("   hats {:?}", d.hats) }
        ));
    }
    text.0 = s;
}

fn frame_stats(
    shared: Res<Shared>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut frames: ResMut<Frames>,
) {
    let phase = Phase::from_u8(shared.phase.load(Ordering::Relaxed));
    let dt = time.delta_secs_f64();
    let focused = windows.single().map(|w| w.focused).unwrap_or(false);
    let f = frames.0.entry(phase.key().to_string()).or_default();
    f.frames += 1;
    f.seconds += dt;
    f.max_frame_ms = f.max_frame_ms.max(dt * 1000.0);
    if focused {
        f.focused_frames += 1;
    }
}
