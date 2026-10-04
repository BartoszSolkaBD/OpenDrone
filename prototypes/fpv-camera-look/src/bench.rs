//! PROTOTYPE (#28). Scripted runs: the GPU benchmark (`--bench`), the latency test (`--latency`)
//! and the screenshot set (`--screenshots`). Each one writes a report and quits.

use crate::map::MapState;
use crate::render::LatencySample;
use crate::rig::{self, Control, Rig};
use crate::settings::{BreakupLevel, Look, MapKind, Tuning};
use crate::{stats, Bypass, LaunchArgs, MapRequest, Measurements};
use bevy::prelude::*;
use bevy::render::view::screenshot::save_to_disk;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Step {
    HidePanel(bool),
    SetMap(MapKind),
    WaitBuilt,
    Wait(f32),
    SetBypass(bool),
    SetLook(Look),
    SetScale(f32),
    SetBreakup(BreakupLevel),
    SetFov(f32),
    PlayFirstPath,
    SeekPath { which: usize, frac: f32 },
    Measure { label: String, secs: f32 },
    Screenshot(String),
    FreeFlyAtLaunch,
    Synthetic(bool),
    SetDebug(f32),
    SetRepeat(u32),
    SetAutoExposure(bool),
    SetVtx(f32),
    SetLensCurve(f32),
    SetAspect(bool),
    Profile,
    Report,
    Exit,
}

#[derive(Clone, Debug)]
pub struct BenchRow {
    pub map: MapKind,
    pub label: String,
    pub source: UVec2,
    pub frames: usize,
    pub cpu: (f32, f32, f32),
    pub total: (f32, f32, f32),
    pub scene: (f32, f32, f32),
    pub mips: (f32, f32, f32),
    pub look: (f32, f32, f32),
    pub camera: (f32, f32, f32),
    pub tail: (f32, f32, f32),
    pub clock_check: f32,
    /// Mean frame time = 1 / frame rate (the honest throughput number; medians hide the waits).
    pub mean_ms: f32,
}

#[derive(Resource, Default)]
pub struct Script {
    pub steps: VecDeque<Step>,
    pub kind: &'static str,
    wait_until: f64,
    measuring: Option<(String, f64, u64, usize)>,
    pub rows: Vec<BenchRow>,
    pub shots: Vec<String>,
    pub started: f64,
    pub profile_md: String,
}

/// A thread that "presses" a stick at random moments, so latency includes waiting for the frame.
#[derive(Resource, Clone)]
pub struct SyntheticSteps {
    pub active: Arc<AtomicBool>,
    pub latest: Arc<AtomicU64>,
    pending: Option<u64>,
    flip: bool,
}

impl Default for SyntheticSteps {
    fn default() -> Self {
        let s = Self { active: Arc::new(AtomicBool::new(false)), latest: Arc::new(AtomicU64::new(0)), pending: None, flip: false };
        let (a, l) = (s.active.clone(), s.latest.clone());
        std::thread::Builder::new()
            .name("synthetic-steps".into())
            .spawn(move || {
                let mut x: u64 = 0x2545F4914F6CDD1D;
                loop {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    let ms = 150 + (x % 200);
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    if a.load(Ordering::Relaxed) {
                        l.store(crate::render::mach_ns(), Ordering::Release);
                    }
                }
            })
            .ok();
        s
    }
}

impl SyntheticSteps {
    pub fn take_pending(&mut self) -> Option<u64> {
        self.pending.take()
    }
}

pub fn start_script(mut script: ResMut<Script>, args: Res<LaunchArgs>, time: Res<Time<Real>>) {
    script.started = time.elapsed_secs_f64();
    let mut s = VecDeque::new();
    if args.bench || args.bench_quick {
        script.kind = "bench";
        s.push_back(Step::HidePanel(true));
        // Quick: Bando only, the runs that give the camera's cost (about 45 s).
        let maps: &[MapKind] = if args.bench_quick { &[MapKind::Bando] } else { &[MapKind::Bando, MapKind::SkatePark] };
        for &map in maps {
            s.push_back(Step::SetMap(map));
            s.push_back(Step::WaitBuilt);
            s.push_back(Step::SetBreakup(BreakupLevel::Realistic));
            s.push_back(Step::SetFov(160.0));
            s.push_back(Step::Wait(1.0));
            // (label, bypass, look, render scale, merged pass drawn N times, auto-exposure)
            let configs: [(&str, bool, Look, f32, u32, bool); 8] = [
                ("A. Plain Bevy camera straight to the output, flat 110 deg, NO camera effects", true, Look::Analog, 1.0, 1, true),
                ("B. Analog, Map drawn at 1.0x", false, Look::Analog, 1.0, 1, true),
                ("C. Analog, auto-exposure OFF (A/B for its cost)", false, Look::Analog, 1.0, 1, false),
                ("D. Analog, merged pass drawn 5x (cost probe)", false, Look::Analog, 1.0, 5, true),
                ("E. Digital, Map drawn at 1.0x", false, Look::Digital, 1.0, 1, true),
                ("F. Digital, merged pass drawn 5x (cost probe)", false, Look::Digital, 1.0, 5, true),
                ("G. Digital, Map drawn at 1.5x", false, Look::Digital, 1.5, 1, true),
                ("H. Digital, Map drawn at 2.0x", false, Look::Digital, 2.0, 1, true),
            ];
            for (label, bypass, look, scale, reps, ae) in configs {
                if args.bench_quick && (label.starts_with("C.") || label.starts_with("G.") || label.starts_with("H.")) {
                    continue;
                }
                s.push_back(Step::SetBypass(bypass));
                s.push_back(Step::SetLook(look));
                s.push_back(Step::SetScale(scale));
                s.push_back(Step::SetRepeat(reps));
                s.push_back(Step::SetAutoExposure(ae));
                s.push_back(Step::PlayFirstPath);
                s.push_back(Step::Wait(1.0));
                s.push_back(Step::WaitBuilt);
                s.push_back(Step::Wait(0.5));
                s.push_back(Step::Measure { label: label.to_string(), secs: 6.0 });
            }
            s.push_back(Step::SetBypass(false));
            s.push_back(Step::SetRepeat(1));
            s.push_back(Step::SetAutoExposure(true));
        }
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    } else if args.latency_test {
        script.kind = "latency";
        s.push_back(Step::HidePanel(true));
        s.push_back(Step::WaitBuilt);
        s.push_back(Step::FreeFlyAtLaunch);
        s.push_back(Step::Wait(1.5));
        s.push_back(Step::Synthetic(true));
        s.push_back(Step::Measure { label: "latency".into(), secs: 12.0 });
        s.push_back(Step::Synthetic(false));
        s.push_back(Step::Wait(0.5));
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    } else if args.look_shots {
        // A short set for checking the look after a change (about 20 s).
        script.kind = "screenshots";
        s.push_back(Step::HidePanel(true));
        s.push_back(Step::WaitBuilt);
        for f in [0.12f32, 0.62] {
            s.push_back(Step::SeekPath { which: 0, frac: f });
            for look in [Look::Analog, Look::Digital] {
                s.push_back(Step::SetLook(look));
                s.push_back(Step::Wait(2.5));
                s.push_back(Step::Screenshot(format!(
                    "r2-bando-p0-{:02}-{}",
                    (f * 100.0) as u32,
                    if look == Look::Analog { "analog" } else { "digital" }
                )));
            }
        }
        s.push_back(Step::SeekPath { which: 1, frac: 0.62 });
        s.push_back(Step::SetLook(Look::Analog));
        for (mw, tag) in [(6.0, "noisy"), (2.0, "heavy")] {
            s.push_back(Step::SetVtx(mw));
            s.push_back(Step::Wait(2.0));
            s.push_back(Step::Screenshot(format!("r2-breakup-{tag}-analog")));
        }
        s.push_back(Step::SetVtx(25.0));
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    } else if args.signal_profile {
        script.kind = "signal-profile";
        s.push_back(Step::HidePanel(true));
        for map in [MapKind::Bando, MapKind::SkatePark] {
            s.push_back(Step::SetMap(map));
            s.push_back(Step::WaitBuilt);
            s.push_back(Step::Profile);
        }
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    } else if args.debug_shots {
        script.kind = "screenshots";
        s.push_back(Step::HidePanel(true));
        s.push_back(Step::WaitBuilt);
        s.push_back(Step::SeekPath { which: 0, frac: 0.12 });
        for (d, name) in [(1.0, "debug-warp"), (2.0, "debug-source"), (0.0, "debug-normal")] {
            s.push_back(Step::SetDebug(d));
            s.push_back(Step::Wait(1.0));
            s.push_back(Step::Screenshot(name.into()));
        }
        // Lens extremes and the panel itself.
        s.push_back(Step::SetLook(Look::Digital));
        for (fov, name) in [(90.0, "lens-fov090"), (170.0, "lens-fov170"), (160.0, "lens-fov160")] {
            s.push_back(Step::SetFov(fov));
            s.push_back(Step::Wait(1.0));
            s.push_back(Step::Screenshot(name.into()));
        }
        s.push_back(Step::SetLensCurve(0.5));
        s.push_back(Step::Wait(1.0));
        s.push_back(Step::Screenshot("lens-fov160-curve05".into()));
        s.push_back(Step::SetLensCurve(0.0));
        s.push_back(Step::SetAspect(true));
        s.push_back(Step::Wait(1.0));
        s.push_back(Step::Screenshot("lens-fov160-16x9".into()));
        s.push_back(Step::SetAspect(false));
        s.push_back(Step::HidePanel(false));
        s.push_back(Step::Wait(1.0));
        s.push_back(Step::Screenshot("panel".into()));
        s.push_back(Step::Wait(1.0));
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    } else if args.screenshots {
        script.kind = "screenshots";
        s.push_back(Step::HidePanel(true));
        let plan: [(MapKind, usize, &[f32]); 3] = [
            (MapKind::Bando, 0, &[0.12, 0.3, 0.45, 0.62, 0.8]),
            (MapKind::Bando, 1, &[0.33, 0.47, 0.62, 0.8]),
            (MapKind::SkatePark, 0, &[0.1, 0.25, 0.45, 0.6, 0.75]),
        ];
        for (map, which, fracs) in plan {
            s.push_back(Step::SetMap(map));
            s.push_back(Step::WaitBuilt);
            s.push_back(Step::SetBreakup(BreakupLevel::Realistic));
            for f in fracs {
                s.push_back(Step::SeekPath { which, frac: *f });
                for look in [Look::Analog, Look::Digital] {
                    s.push_back(Step::SetLook(look));
                    s.push_back(Step::Wait(1.6));
                    let name = format!(
                        "{}-p{}-{:02}-{}",
                        if map == MapKind::Bando { "bando" } else { "skatepark" },
                        which,
                        (f * 100.0) as u32,
                        if look == Look::Analog { "analog" } else { "digital" }
                    );
                    s.push_back(Step::Screenshot(name));
                }
            }
        }
        // Breakup check: the Bando's south-west room, with the whoop's VTX turned down so the
        // Video Signal is weak (a stand-in for "deeper inside"), on Realistic.
        s.push_back(Step::SetMap(MapKind::Bando));
        s.push_back(Step::WaitBuilt);
        s.push_back(Step::SeekPath { which: 1, frac: 0.62 });
        for (mw, tag) in [(3.0, "medium"), (0.8, "heavy"), (0.2, "lost")] {
            s.push_back(Step::SetVtx(mw));
            for look in [Look::Analog, Look::Digital] {
                s.push_back(Step::SetLook(look));
                s.push_back(Step::Wait(2.0));
                s.push_back(Step::Screenshot(format!(
                    "breakup-{tag}-{}",
                    if look == Look::Analog { "analog" } else { "digital" }
                )));
            }
        }
        s.push_back(Step::SetVtx(25.0));
        s.push_back(Step::Wait(1.0));
        s.push_back(Step::Report);
        s.push_back(Step::Exit);
    }
    script.steps = s;
}

pub fn synthetic_steps(mut steps: ResMut<SyntheticSteps>, mut rig: ResMut<Rig>) {
    let t = steps.latest.swap(0, Ordering::AcqRel);
    if t != 0 {
        steps.pending = Some(t);
        // Make the frame visibly different: yaw the camera 10 degrees one way or the other.
        steps.flip = !steps.flip;
        rig.heading += if steps.flip { 0.17 } else { -0.17 };
    }
}

fn results_dir() -> std::path::PathBuf {
    crate::settings::proto_dir().join("results")
}

#[allow(clippy::too_many_arguments)]
pub fn run_script(
    mut script: ResMut<Script>,
    time: Res<Time<Real>>,
    mut tuning: ResMut<Tuning>,
    mut rig: ResMut<Rig>,
    map: Res<MapState>,
    mut req: ResMut<MapRequest>,
    mut bypass: ResMut<Bypass>,
    mut ui: ResMut<crate::ui::UiState>,
    meas: Res<Measurements>,
    steps: Option<Res<SyntheticSteps>>,
    args: Res<LaunchArgs>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut debug: ResMut<crate::DebugView>,
    mut probe: ResMut<crate::CostProbe>,
    output: Res<crate::Output>,
) {
    let now = time.elapsed_secs_f64();
    if now < script.wait_until {
        return;
    }
    if let Some((label, end, start_frame, cpu_start)) = script.measuring.clone() {
        if now < end {
            return;
        }
        let gpu: Vec<_> = meas.gpu.iter().filter(|g| g.frame_id >= start_frame).copied().collect();
        let cpu: Vec<f32> = meas.cpu_ms.iter().rev().take(meas.frame.saturating_sub(cpu_start as u64) as usize).copied().collect();
        let st = |f: &dyn Fn(&crate::render::FrameGpu) -> f32| stats(gpu.iter().map(f).collect());
        let mean_ms = cpu.iter().sum::<f32>() / cpu.len().max(1) as f32;
        script.rows.push(BenchRow {
            map: map.current.unwrap_or_default(),
            label,
            source: meas.source_size,
            frames: cpu.len(),
            mean_ms,
            cpu: stats(cpu),
            total: st(&|g| g.total),
            scene: st(&|g| g.scene),
            mips: st(&|g| g.mips),
            look: st(&|g| g.look),
            camera: st(&|g| g.mips + g.look),
            tail: st(&|g| g.tail),
            clock_check: stats(gpu.iter().map(|g| g.end_minus_submit).collect()).0,
        });
        script.measuring = None;
    }
    let Some(step) = script.steps.pop_front() else { return };
    match step {
        Step::HidePanel(h) => ui.hidden = h,
        Step::SetMap(m) => req.0 = Some(m),
        Step::WaitBuilt => {
            if !map.built || req.0.is_some() || !meas.pipelines_settled {
                script.steps.push_front(Step::WaitBuilt);
            }
        }
        Step::Wait(s) => script.wait_until = now + s as f64,
        Step::SetBypass(b) => bypass.0 = b,
        Step::SetLook(l) => tuning.look = l,
        Step::SetScale(s) => {
            tuning.analog.source_scale = s;
            tuning.digital.source_scale = s;
        }
        Step::SetBreakup(b) => tuning.breakup = b,
        Step::SetFov(f) => tuning.fov_deg = f,
        Step::PlayFirstPath => {
            let idx = crate::paths::paths_for(map.current.unwrap_or_default())[0];
            rig.control = Control::Path;
            rig.path_loop = true;
            rig::select_path(&mut rig, idx);
            rig.path_playing = true;
        }
        Step::SeekPath { which, frac } => {
            let idx = crate::paths::paths_for(map.current.unwrap_or_default())[which];
            rig.control = Control::Path;
            rig.path_loop = false;
            rig::select_path(&mut rig, idx);
            let len = rig.path.as_ref().map(|p| p.length()).unwrap_or(0.0);
            let q = tuning.quad;
            while rig.path_d < len * frac {
                rig::advance_path(&mut rig, 1.0 / 120.0, q);
            }
            rig.path_playing = false;
        }
        Step::Measure { label, secs } => {
            script.measuring = Some((label, now + secs as f64, meas.frame + 1, meas.frame as usize));
        }
        Step::Screenshot(name) => {
            let dir = results_dir().join("screenshots");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("{name}.png"));
            script.shots.push(path.display().to_string());
            commands.spawn(crate::screenshot_of(&output)).observe(save_to_disk(path));
            script.wait_until = now + 0.4;
        }
        Step::FreeFlyAtLaunch => {
            rig.control = Control::FreeFly;
            rig::reset_to_launch(&mut rig, &map);
            rig.pos += Vec3::Y * 1.2;
        }
        Step::Synthetic(on) => {
            if let Some(s) = steps.as_ref() {
                s.active.store(on, Ordering::Release);
            }
        }
        Step::SetDebug(d) => debug.0 = d,
        Step::Profile => {
            let md = crate::profile::report(&map, &tuning);
            script.profile_md.push_str(&md);
            script.profile_md.push('\n');
        }
        Step::SetRepeat(n) => probe.0 = n,
        Step::SetVtx(mw) => tuning.signal.vtx_mw_whoop = mw,
        Step::SetLensCurve(k) => {
            tuning.analog.lens_curve = k;
            tuning.digital.lens_curve = k;
        }
        Step::SetAspect(wide) => {
            tuning.aspect = if wide { crate::settings::Aspect::SixteenNine } else { crate::settings::Aspect::FourThree }
        }
        Step::SetAutoExposure(on) => {
            tuning.analog.exposure.enabled = on;
            tuning.digital.exposure.enabled = on;
        }
        Step::Report => {
            let recent: Vec<f32> = meas.cpu_ms.iter().rev().take(2000).copied().collect();
            write_report(&script, &meas.latency, &recent, &args, &tuning)
        }
        Step::Exit => {
            exit.write(AppExit::Success);
        }
    }
}

fn f3(v: (f32, f32, f32)) -> String {
    format!("{:.2} / {:.2} / {:.2}", v.0, v.1, v.2)
}

fn write_report(script: &Script, latency: &[LatencySample], meas_cpu: &[f32], args: &LaunchArgs, tuning: &Tuning) {
    let dir = results_dir();
    let _ = std::fs::create_dir_all(&dir);
    let stamp = crate::now_stamp();
    let mut md = String::new();
    let pipe = if args.pipelined { "ON" } else { "OFF" };
    let _ = writeln!(md, "# PROTOTYPE #28 {} run, {stamp}\n", script.kind);
    let _ = writeln!(md, "- Machine: Mac mini M4 (10-core GPU), macOS, Bevy 0.19.1, Metal. Window 2560x1440, VSync off (AutoNoVsync).");
    let _ = writeln!(md, "- Pipelined rendering: **{pipe}**. Frames allowed in flight on the GPU: **{}**. Quad: {}. FOV {} deg, Camera Tilt {} deg, Breakup {}.", args.frames_in_flight, tuning.quad.label(), tuning.fov_deg, tuning.tilt_deg, tuning.breakup.label());
    let _ = writeln!(
        md,
        "- Output: {}.",
        if args.offscreen {
            "OFFSCREEN 2560x1440 image, no window (no present, no compositor): the GPU work is the same except the final copy to the screen"
        } else {
            "the window at its native size"
        }
    );
    let _ = writeln!(md, "- Values are median / p95 / p99 in ms. Frame time = wall clock between frames (at most 3 frames in flight, VSync off). GPU times are Metal timestamps at the END of each pass: mip chain = Map render done -> last mip done; merged pass = last mip done -> merged pass done.\n");
    match script.kind {
        "bench" => {
            let _ = writeln!(md, "| Map | Run | Map drawn at (px) | Frames | **Mean frame ms (fps)** | Frame time median / p95 / p99 | Mip chain | Merged pass | **Camera steps (mips + merged)** |");
            let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");
            for r in &script.rows {
                let _ = writeln!(
                    md,
                    "| {} | {} | {}x{} | {} | **{:.2} ({:.0})** | {} | {} | {} | **{}** |",
                    r.map.label(),
                    r.label,
                    r.source.x,
                    r.source.y,
                    r.frames,
                    r.mean_ms,
                    1000.0 / r.mean_ms,
                    f3(r.cpu),
                    f3(r.mips),
                    f3(r.look),
                    f3(r.camera),
                );
            }
            let _ = writeln!(md, "\nHow to read it (use the MEAN frame time; medians hide the waits for the GPU): B minus A is everything the FPV camera adds over a plain Bevy camera (including drawing a wider view); B minus C is auto-exposure; (D minus B) / 4 is one Analog merged pass and (F minus E) / 4 one Digital merged pass, measured on the whole frame instead of by timestamps.");
            let mut derived = String::new();
            for map in [MapKind::Bando, MapKind::SkatePark] {
                let get = |p: &str| script.rows.iter().find(|r| r.map == map && r.label.starts_with(p)).map(|r| r.mean_ms);
                if let (Some(a), Some(b), None, Some(d), Some(e), Some(f)) =
                    (get("A."), get("B."), get("C."), get("D."), get("E."), get("F."))
                {
                    let _ = writeln!(
                        derived,
                        "- {}: Analog adds {:.2} ms over a plain camera, Digital {:.2} ms; one Analog merged pass {:.2} ms, one Digital merged pass {:.2} ms.",
                        map.label(),
                        b - a,
                        e - a,
                        (d - b) / 4.0,
                        (f - e) / 4.0
                    );
                }
                if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f), Some(g), Some(h)) =
                    (get("A."), get("B."), get("C."), get("D."), get("E."), get("F."), get("G."), get("H."))
                {
                    let _ = writeln!(
                        derived,
                        "- {}: Analog adds {:.2} ms over a plain camera, Digital {:.2} ms; auto-exposure {:.2} ms; one Analog merged pass {:.2} ms, one Digital merged pass {:.2} ms; drawing the Map at 1.5x adds {:.2} ms, at 2.0x {:.2} ms.",
                        map.label(),
                        b - a,
                        e - a,
                        b - c,
                        (d - b) / 4.0,
                        (f - e) / 4.0,
                        g - e,
                        h - e
                    );
                }
            }
            let _ = writeln!(md, "\n{derived}");
            let _ = writeln!(md, "Clock check (merged pass end minus CPU submit, median ms per run; small positive = GPU and CPU clocks agree): {}", script.rows.iter().map(|r| format!("{:.2}", r.clock_check)).collect::<Vec<_>>().join(", "));
        }
        "latency" => {
            let pres: Vec<f32> = latency.iter().map(|l| l.to_present_ms).collect();
            let gpu: Vec<f32> = latency.iter().filter_map(|l| l.to_gpu_done_ms).collect();
            let (p50, p95, p99) = stats(pres.clone());
            let (g50, g95, g99) = stats(gpu.clone());
            let _ = writeln!(md, "Synthetic stick steps at random moments (150-350 ms apart), 12 s, camera hovering at the Launch Spot.\n");
            let _ = writeln!(md, "| Measured from the input sample to... | Samples | median ms | p95 ms | p99 ms |");
            let _ = writeln!(md, "|---|---|---|---|---|");
            let frame = stats(meas_cpu.to_vec());
            let _ = writeln!(md, "| the frame that used it was handed to the GPU and presented (CPU side) | {} | {p50:.2} | {p95:.2} | {p99:.2} |", pres.len());
            let _ = writeln!(md, "| the CPU saw the GPU finish that frame (upper bound) | {} | {g50:.2} | {g95:.2} | {g99:.2} |", gpu.len());
            let _ = writeln!(md, "\nFrame time during the test: median {:.2} ms, p95 {:.2} ms ({:.0} fps). The step is applied in the next frame's update, so the expected cost of pipelining is about one extra frame.", frame.0, frame.1, 1000.0 / frame.0);
            let _ = writeln!(md, "\nNot included: USB and radio, the Radio Link emulation (#21), the physics step, the compositor and the display's scan-out (about half to one refresh, 3-6 ms at 165 Hz).");
            let raw: Vec<String> = latency.iter().map(|l| format!("{:.2}/{}", l.to_present_ms, l.to_gpu_done_ms.map(|v| format!("{v:.2}")).unwrap_or("-".into()))).collect();
            let _ = writeln!(md, "\nRaw (present/gpu-done ms): {}", raw.join(" "));
        }
        "signal-profile" => {
            let _ = writeln!(md, "Video Signal along the preset paths, simulated without rendering (no flutter), every 0.2 m at the path's speed.\n");
            md.push_str(&script.profile_md);
        }
        "screenshots" => {
            for s in &script.shots {
                let _ = writeln!(md, "- {s}");
            }
        }
        _ => {}
    }
    let name = match script.kind {
        "latency" => format!("latency-pipelined-{}-inflight{}-{stamp}.md", if args.pipelined { "on" } else { "off" }, args.frames_in_flight),
        k => format!("{k}-{stamp}.md"),
    };
    let path = dir.join(name);
    let _ = std::fs::write(&path, &md);
    println!("{md}");
    println!("report written to {}", path.display());
}
