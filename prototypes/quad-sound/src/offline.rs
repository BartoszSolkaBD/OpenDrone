//! PROTOTYPE (#34). Things that run without the window:
//! - `render`: the same Firewheel graph, run offline into WAV files the maintainer can listen to.
//! - `bench`: CPU cost (offline, many seconds) and the live stream's block size and output delay
//!   on this machine's sound device, at several requested block sizes, with the master at 0.
//! - `nodevice`: the app's no-sound-device path, headless.

use crate::audio::Engine;
use crate::map::{MapGeom, MapKind};
use crate::paths::PathKind;
use crate::quads::{QuadKind, Tuning};
use crate::script::{ScriptKind, ScriptRun};
use crate::sim::{DT, Sim, SimEvent};
use crate::synth::{load_f, SoundFrame};
use firewheel::{ActivateInfo, backend::BackendProcessInfo, node::StreamStatus};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub struct Scene {
    pub name: &'static str,
    pub quad: QuadKind,
    pub map: MapKind,
    pub stand: bool,
    pub script: ScriptKind,
    pub secs: f32,
    pub clip: bool,
    pub what: &'static str,
}

pub fn scenes() -> Vec<Scene> {
    use QuadKind::*;
    use ScriptKind as S;
    let s = |name, quad, map, stand, script, secs, what| Scene { name, quad, map, stand, script, secs, clip: false, what };
    let mut v = vec![
        s("01-whoop-powerup-onquad", Whoop65, MapKind::Bando, false, S::PowerUp, 7.0, "Reset: Bluejay start-up melody and ready beeps, arm, idle, disarm"),
        s("02-whoop-hover-onquad", Whoop65, MapKind::Bando, false, S::Hover, 10.0, "take off, hover with small corrections, land"),
        s("03-whoop-punchout-onquad", Whoop65, MapKind::Bando, false, S::PunchOut, 9.0, "hover, full throttle 1.6 s, chop, catch"),
        s("04-whoop-punchout-standing", Whoop65, MapKind::Bando, true, S::PunchOut, 9.0, "the same flight heard from the Launch Spot"),
        s("05-five-powerup-onquad", Freestyle5, MapKind::SkatePark, false, S::PowerUp, 7.0, "Reset with the 5\": ESC melody plus Betaflight's buzzer, arm and disarm beeps"),
        s("06-five-hover-onquad", Freestyle5, MapKind::SkatePark, false, S::Hover, 10.0, "take off, hover, land"),
        s("07-five-punchout-onquad", Freestyle5, MapKind::SkatePark, false, S::PunchOut, 9.0, "hover, full throttle 1.6 s, chop, catch"),
        s("08-five-punchout-standing", Freestyle5, MapKind::SkatePark, true, S::PunchOut, 9.0, "the same flight heard from the Launch Spot"),
        s("10-five-flip-onquad", Freestyle5, MapKind::SkatePark, false, S::Flip, 5.0, "punch, acro roll flip, catch"),
        s("11-five-propwash-onquad", Freestyle5, MapKind::SkatePark, false, S::PropWash, 6.0, "chop and fall into your own air: motors warble"),
        s("12-five-flyby-standing", Freestyle5, MapKind::SkatePark, true, S::Path(PathKind::FlyBy), 7.0, "fly-by at 26 m/s, 8 m in front: Doppler"),
        s("13-five-around-tower-standing", Freestyle5, MapKind::Bando, true, S::Path(PathKind::AroundTower), 16.0, "round the Bando tower: muffled behind it"),
        s("14-five-outandback-standing", Freestyle5, MapKind::SkatePark, true, S::Path(PathKind::OutAndBack), 12.0, "out to 60 m and back: fade, delay"),
        s("15-five-propstrike-onquad", Freestyle5, MapKind::SkatePark, false, S::PropStrike, 6.0, "two Prop Strikes: ticks, motor recovers"),
        s("16-five-jam-onquad", Freestyle5, MapKind::SkatePark, false, S::Jam, 7.5, "jammed prop: restarts fail, three falling ESC tones"),
        s("17-five-crash-onquad", Freestyle5, MapKind::SkatePark, false, S::Crash, 5.0, "dive into the ground: hit clip"),
        s("18-five-failsafe-onquad", Freestyle5, MapKind::SkatePark, false, S::Failsafe, 7.0, "link lost, drop after 1.5 s, RX_LOST beeps until the link returns"),
        s("19-five-lowbattery-onquad", Freestyle5, MapKind::SkatePark, false, S::LowBattery, 8.5, "pack sags: LOW BATTERY then LAND NOW beeps"),
        s("20-whoop-crash-onquad", Whoop65, MapKind::Bando, false, S::Crash, 5.0, "whoop dives into the ground: hit clip"),
        s("21-five-intobowl-standing", Freestyle5, MapKind::SkatePark, true, S::Path(PathKind::IntoBowl), 7.0, "down into the Skate Park's pool bowl: muffled by its walls"),
        s("23-whoop-beacon-onquad", Whoop65, MapKind::Bando, false, S::Beacon, 8.0, "10 idle minutes skipped: Bluejay's beacon every 3.1 s"),
        s("24-five-pausemenu-onquad", Freestyle5, MapKind::SkatePark, false, S::PauseMenu, 7.0, "Pause Menu at 2.5 s: Quad silent, background dips, menu click; Resume at 5 s"),
        s("22-whoop-flyby-standing", Whoop65, MapKind::SkatePark, true, S::Path(PathKind::FlyBy), 10.0, "whoop fly-by at 11 m/s, 8 m in front"),
    ];
    v.push(Scene { clip: true, ..s("09-five-punchout-onquad-clipping", Freestyle5, MapKind::SkatePark, false, S::PunchOut, 9.0, "as 07, with clipping on") });
    v.sort_by_key(|s| s.name);
    v
}

pub fn maps() -> HashMap<MapKind, Arc<MapGeom>> {
    let mut m = HashMap::new();
    for k in [MapKind::SkatePark, MapKind::Bando] {
        match MapGeom::load(k) {
            Ok(g) => {
                eprintln!(
                    "[map] {}: {} parts, {} triangles, launch {:?} facing {:.0}° ({:?})",
                    k.label(),
                    g.objects.len(),
                    g.tri_count,
                    g.launch,
                    g.launch_yaw.to_degrees(),
                    g.launch_names
                );
                if let Some(o) = g.main_feature() {
                    eprintln!("[map]   main feature: {} {:?}..{:?}", o.name, o.min, o.max);
                }
                if std::env::var("T34_OBJECTS").is_ok() {
                    let mut v: Vec<_> = g.objects.iter().collect();
                    v.sort_by(|a, b| ((b.max - b.min).length()).partial_cmp(&(a.max - a.min).length()).unwrap());
                    for o in v.iter().take(25) {
                        eprintln!("[map]   {:30} {:?}..{:?}", o.name, o.min, o.max);
                    }
                }
                m.insert(k, Arc::new(g));
            }
            Err(e) => panic!("map {}: {e}", k.label()),
        }
    }
    m
}

pub fn background_for(tuning: &Tuning, map: MapKind) -> String {
    match map {
        MapKind::SkatePark => tuning.clips.skate_park_background.clone(),
        MapKind::Bando => tuning.clips.bando_background.clone(),
    }
}

pub fn hit_for(tuning: &Tuning, quad: QuadKind) -> String {
    match quad {
        QuadKind::Whoop65 => tuning.clips.whoop_hit.clone(),
        QuadKind::Freestyle5 => tuning.clips.five_hit.clone(),
    }
}

pub fn hit_gain(tuning: &Tuning, quad: QuadKind, speed: f32) -> f32 {
    tuning.block(quad).hit_level * (speed / 5.0).clamp(0.2, 2.0)
}

struct Offline {
    // The processor must drop before the context, or the context's Drop waits for it.
    proc: firewheel::processor::FirewheelProcessor,
    engine: Engine,
    writers: [triple_buffer::Input<SoundFrame>; 2],
    block: usize,
    started: Instant,
    pub cpu: Duration,
    pub frames: u64,
}

impl Offline {
    fn new(tuning: &Tuning, quad: QuadKind, sr: u32, block: usize) -> Self {
        let (mut engine, writers) = Engine::new(tuning, quad);
        let proc = engine
            .cx
            .activate(ActivateInfo {
                sample_rate: NonZeroU32::new(sr).unwrap(),
                max_block_frames: NonZeroU32::new(block as u32).unwrap(),
                num_stream_in_channels: 0,
                num_stream_out_channels: 2,
                input_to_output_latency_seconds: 0.0,
            })
            .expect("activate offline");
        engine.sr = sr;
        engine.update();
        Offline { engine, proc, writers, block, started: Instant::now(), cpu: Duration::ZERO, frames: 0 }
    }

    fn publish(&mut self, f: SoundFrame) {
        self.writers[0].write(f);
        self.writers[1].write(f);
    }

    fn process_block(&mut self, out: &mut Vec<f32>) {
        self.engine.update();
        let n = self.block;
        let mut buf = vec![0.0f32; n * 2];
        let input: Vec<f32> = Vec::new();
        let t0 = Instant::now();
        self.proc.process(
            &audioadapter_buffers::direct::InterleavedSlice::new(&input[..], 0, n).unwrap(),
            &mut audioadapter_buffers::direct::InterleavedSlice::new_mut(&mut buf[..], 2, n).unwrap(),
            BackendProcessInfo {
                frames: n,
                process_timestamp: Some(t0),
                duration_since_stream_start: self.started.elapsed(),
                input_stream_status: StreamStatus::empty(),
                output_stream_status: StreamStatus::empty(),
                dropped_frames: 0,
                process_to_playback_delay: None,
            },
        );
        self.cpu += t0.elapsed();
        self.frames += n as u64;
        out.extend_from_slice(&buf);
    }
}

/// Run one scene through the sim and the graph; returns interleaved stereo and the CPU time.
fn run_scene(scene: &Scene, tuning: &Tuning, maps: &HashMap<MapKind, Arc<MapGeom>>, sr: u32) -> (Vec<f32>, Duration, f32) {
    let mut tuning = tuning.clone();
    tuning.listener.where_you_stand = scene.stand;
    tuning.listener.clip_on = scene.clip || tuning.listener.clip_on && !scene.name.contains("onquad") || (scene.clip);
    if !scene.clip && scene.name.contains("onquad") {
        tuning.listener.clip_on = false;
    }
    let block = 256;
    let t_setup = Instant::now();
    let mut off = Offline::new(&tuning, scene.quad, sr, block);
    let bg = background_for(&tuning, scene.map);
    off.engine.set_background(&bg, &tuning);
    if std::env::var("T34_TIMING").is_ok() {
        eprintln!("[timing] setup + background decode {:.2} s", t_setup.elapsed().as_secs_f64());
    }
    let mut sim = Sim::new(scene.quad, maps[&scene.map].clone());
    let mut run = ScriptRun::new(scene.script);
    let mut out = Vec::new();
    let block_s = block as f64 / sr as f64;
    let mut sim_time = 0.0f64;
    let mut t = 0.0f64;
    let hit = hit_for(&tuning, scene.quad);
    let trace = std::env::var("T34_TRACE").is_ok();
    let mut was_paused = false;
    while t < scene.secs as f64 {
        t += block_s;
        while sim_time < t {
            run.step(&mut sim, DT);
            sim.step();
            sim_time += DT as f64;
        }
        for e in sim.events.drain(..) {
            match e {
                SimEvent::Hit { speed } => off.engine.play_hit(&hit, hit_gain(&tuning, scene.quad, speed)),
            }
        }
        if sim.paused != was_paused {
            // The Pause Menu: background dips, a menu click (the Quad fades in the voice node).
            was_paused = sim.paused;
            off.engine.set_paused(was_paused, &tuning);
            let click = if was_paused { tuning.clips.menu_click.clone() } else { tuning.clips.menu_back.clone() };
            off.engine.play_menu(&click);
        }
        let f = sim.frame();
        off.publish(f);
        let n0 = out.len();
        off.process_block(&mut out);
        if trace && ((t * 10.0) as u64 != ((t - block_s) * 10.0) as u64) {
            let rms = (out[n0..].iter().map(|x| x * x).sum::<f32>() / (out.len() - n0) as f32).sqrt();
            let rpm: Vec<String> = sim.omega.iter().map(|w| format!("{:5.0}", w * 60.0 / std::f32::consts::TAU)).collect();
            let (_, _, roll) = sim.att.to_euler(glam::EulerRot::ZYX);
            let states: String = (0..4).map(|i| sim.motor_state_label(i).chars().next().unwrap()).collect();
            eprintln!(
                "t={:5.2} armed={} esc={} block={:9} h={:5.2} v={:5.1} roll={:5.0} rpm=[{}] {} I=[{:4.1} {:4.1}] rub={:.1} esc_hz={:4.0} buzz={} {:18} walls={} d={:5.1} delay={:5.1}ms dop={:.3} rms={:.3} gr={:.1}",
                t,
                sim.armed as u8,
                sim.esc_ready as u8,
                sim.arm_block,
                sim.pos.z - sim.map.launch.z,
                sim.airspeed,
                roll.to_degrees(),
                rpm.join(" "),
                states,
                sim.current[0],
                sim.current[1],
                sim.rub.iter().cloned().fold(0.0, f32::max),
                f.esc_hz.iter().cloned().fold(0.0, f32::max),
                f.buzzer as u8,
                sim.beeper_label(),
                sim.walls,
                load_f(&off.engine.stats.distance_m),
                load_f(&off.engine.stats.delay_ms),
                load_f(&off.engine.stats.doppler),
                rms,
                load_f(&off.engine.stats.gain_reduction_db)
            );
        }
    }
    let peak = out.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    (out, off.cpu, peak)
}

pub fn render(names: &[String], tuning: &Tuning, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).unwrap();
    let maps = maps();
    let sr = 48000;
    let mut index = String::from("# Rendered sounds (PROTOTYPE, #34)\n\nMade offline by `./run.sh render` with the tuning in effect, through the same Firewheel graph the app plays live. 48 kHz, 16-bit stereo WAV here (not committed); AAC 256 kbps copies with the same names in `m4a/` (committed, so they can be played from the branch). Peak is of the whole file (1.0 = full scale). Use headphones or decent speakers; a laptop speaker loses the low end.\n\n| File | Quad | Listening Position | What happens | Peak |\n|---|---|---|---|---|\n");
    let mut total_cpu = Duration::ZERO;
    let mut total_audio = 0.0;
    for scene in scenes() {
        if !names.is_empty() && !names.iter().any(|n| scene.name.contains(n.as_str())) {
            continue;
        }
        let (out, cpu, peak) = run_scene(&scene, tuning, &maps, sr);
        let path = out_dir.join(format!("{}.wav", scene.name));
        let spec = hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in &out {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
        }
        w.finalize().unwrap();
        total_cpu += cpu;
        total_audio += scene.secs as f64;
        eprintln!(
            "[render] {:<34} {:>5.1} s  peak {:>5.2}{}  sound thread {:.2}% of one core",
            scene.name,
            scene.secs,
            peak,
            if peak > 1.0 { " (CLIPS)" } else { "" },
            cpu.as_secs_f64() / scene.secs as f64 * 100.0
        );
        index.push_str(&format!(
            "| `{}.wav` | {} | {} | {} | {:.2} |\n",
            scene.name,
            scene.quad.label(),
            if scene.stand { "Where you stand" } else if scene.clip { "On the Quad + clipping" } else { "On the Quad" },
            scene.what,
            peak
        ));
    }
    index.push_str(&format!(
        "\nWhole graph, offline: {:.2}% of one M4 core on average ({:.1} s of sound in {:.3} s).\n",
        total_cpu.as_secs_f64() / total_audio * 100.0,
        total_audio,
        total_cpu.as_secs_f64()
    ));
    std::fs::write(out_dir.join("README.md"), index).unwrap();
}

/// CPU cost offline, then the live stream's numbers on this machine's default output device.
pub fn bench(tuning: &Tuning) -> String {
    let maps = maps();
    let mut report = String::new();
    report.push_str("# Sound bench (PROTOTYPE, #34)\n\n");
    report.push_str(&format!("Machine: {}\n\n", machine()));
    // 1. Offline CPU: 60 s of punch-outs per Quad, both Listening Positions.
    report.push_str("## CPU cost, offline (whole graph: 4 motor voices, listener, volumes, samplers)\n\n| Quad | Listening Position | % of one core |\n|---|---|---|\n");
    for quad in [QuadKind::Whoop65, QuadKind::Freestyle5] {
        for stand in [false, true] {
            let scene = Scene { name: "bench", quad, map: MapKind::Bando, stand, script: ScriptKind::PunchOut, secs: 60.0, clip: false, what: "" };
            let (_, cpu, _) = run_scene(&scene, tuning, &maps, 48000);
            report.push_str(&format!(
                "| {} | {} | {:.2}% |\n",
                quad.label(),
                if stand { "Where you stand" } else { "On the Quad" },
                cpu.as_secs_f64() / 60.0 * 100.0
            ));
        }
    }
    // 2. Live: block size and the OS-reported output delay at several requested block sizes.
    report.push_str("\n## Live stream on the default output device (master volume 0, so nothing is heard)\n\n");
    report.push_str("Output delay is what CoreAudio reports through cpal: the device buffer plus the device's latency and safety offset. It is not measured acoustically (the Mac mini has no microphone). A new tick can wait up to one block before the next callback reads it, so the worst case adds one block; the average adds half. The game adds its own frame-to-sound hand-off on top (not in this prototype: here the simulation thread publishes every 1 ms).\n\n");
    report.push_str("| Requested block | Block in use | Sample rate | Output delay (callback to speaker) | Worst case tick to speaker (+ one block wait) | Tick age at read | Peak CPU in a block | Device |\n|---|---|---|---|---|---|---|---|\n");
    let mut t = tuning.clone();
    t.volumes.master = 0.0;
    for req in [Some(64u32), Some(128), Some(256), Some(512), Some(1024), None] {
        match live_probe(&t, &maps, req) {
            Ok(r) => report.push_str(&r),
            Err(e) => report.push_str(&format!("| {:?} | failed: {e} | | | | | | |\n", req)),
        }
    }
    report
}

fn machine() -> String {
    let out = std::process::Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn live_probe(tuning: &Tuning, maps: &HashMap<MapKind, Arc<MapGeom>>, block: Option<u32>) -> Result<String, String> {
    use firewheel::cpal::{CpalConfig, CpalOutputConfig, CpalStream};
    let quad = QuadKind::Freestyle5;
    let (mut engine, mut writers) = Engine::new(tuning, quad);
    // Note: turning on Firewheel 0.14's per-node profiling here (profile_nodes) panicked on the
    // audio thread (profiling.rs:226, index out of bounds), so only the overall figure is used.
    let cfg = CpalConfig { output: CpalOutputConfig { desired_block_frames: block, ..Default::default() }, input: None };
    let mut stream = CpalStream::new(&mut engine.cx, cfg).map_err(|e| e.to_string())?;
    engine.sr = stream.info().sample_rate.get();
    let device = stream
        .info()
        .out_device_id
        .as_ref()
        .and_then(|id| {
            firewheel::cpal::default_host_enumerator().output_devices().into_iter().find(|d| &d.id == id).and_then(|d| d.name)
        })
        .unwrap_or_else(|| "default".into());
    let mut sim = Sim::new(quad, maps[&MapKind::SkatePark].clone());
    let mut run = ScriptRun::new(ScriptKind::PunchOut);
    let start = Instant::now();
    let mut sim_t = 0.0f64;
    let mut peak_cpu = 0.0f64;
    let mut ages = Vec::new();
    let mut delays = Vec::new();
    while start.elapsed() < Duration::from_secs(4) {
        let now = start.elapsed().as_secs_f64();
        while sim_t < now {
            run.step(&mut sim, DT);
            sim.step();
            sim_t += DT as f64;
            if (sim_t * 2000.0) as u64 % 2 == 0 {
                let f = sim.frame();
                writers[0].write(f);
                writers[1].write(f);
            }
        }
        engine.update();
        let p = engine.cx.profiling_data();
        peak_cpu = peak_cpu.max(p.overall_cpu_usage);
        if start.elapsed() > Duration::from_millis(500) {
            ages.push(engine.stats.frame_age_us.load(Ordering::Relaxed) as f32 / 1000.0);
            delays.push(engine.stats.playback_delay_us.load(Ordering::Relaxed) as f32 / 1000.0);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let _ = stream.poll_status().count();
    let bf = engine.stats.block_frames.load(Ordering::Relaxed);
    let sr = engine.stats.sample_rate.load(Ordering::Relaxed).max(1);
    let med = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let delay = med(&mut delays);
    let age = med(&mut ages);
    let block_ms = bf as f32 / sr as f32 * 1000.0;
    drop(stream);
    let _ = load_f;
    Ok(format!(
        "| {} | {} frames ({:.1} ms) | {} Hz | {:.1} ms | {:.1} ms | {:.2} ms | {:.1}% | {} |\n",
        block.map(|b| b.to_string()).unwrap_or("device default".into()),
        bf,
        block_ms,
        sr,
        delay,
        delay + block_ms,
        age,
        peak_cpu * 100.0,
        device
    ))
}

/// The no-sound-device path, headless: the sim runs, the engine is never activated, every sound
/// call is skipped, and the app says so once.
pub fn nodevice(tuning: &Tuning) {
    let maps = maps();
    let (mut engine, mut writers) = Engine::new(tuning, QuadKind::Freestyle5);
    println!("No sound device: running silent. (said once, as the Hub would)");
    let mut sim = Sim::new(QuadKind::Freestyle5, maps[&MapKind::Bando].clone());
    let mut run = ScriptRun::new(ScriptKind::Crash);
    let start = Instant::now();
    let mut steps = 0u64;
    let mut hits = 0;
    while start.elapsed() < Duration::from_secs(4) {
        for _ in 0..20 {
            run.step(&mut sim, DT);
            sim.step();
            steps += 1;
        }
        for e in sim.events.drain(..) {
            if let SimEvent::Hit { speed } = e {
                hits += 1;
                engine.play_hit(&hit_for(tuning, QuadKind::Freestyle5), speed);
            }
        }
        let f = sim.frame();
        writers[0].write(f);
        writers[1].write(f);
        engine.apply_tuning(tuning, QuadKind::Freestyle5);
        engine.play_menu(&tuning.clips.menu_click);
        engine.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "ok: {} sim steps ({:.1} s of flight), {} hit(s) skipped, engine active = {}, no panic",
        steps,
        steps as f64 * DT as f64,
        hits,
        engine.active()
    );
}

pub fn default_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("renders")
}
