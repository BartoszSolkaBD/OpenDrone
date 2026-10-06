//! PROTOTYPE (#34). The window: what to fly and hear on the left, a top-down Map and motor
//! traces in the middle, the tuning panel on the right. The simulation runs on its own thread at
//! 2 kHz and publishes each tick to the sound; this thread only shows and steers it.

use crate::audio::{ClipEntry, Engine};
use crate::input::{self, SdlInput};
use crate::map::{MapGeom, MapKind};
use crate::offline::{background_for, hit_for, hit_gain, maps};
use crate::paths::ALL_PATHS;
use crate::quads::{Knob, QuadDef, QuadKind, Tuning};
use crate::script::{SCRIPTS, ScriptKind, ScriptRun};
use crate::sim::{Cmd, DT, Sim, SimEvent, Throttle};
use crate::synth::{SoundFrame, load_f};
use eframe::egui;
use firewheel::cpal::{CpalConfig, CpalOutputConfig, CpalStream};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Keyboard,
    Device(usize),
}

struct Shared {
    sim: Sim,
    run: Option<ScriptRun>,
    writers: [triple_buffer::Input<SoundFrame>; 2],
    source: Source,
    r2_throttle: bool,
    /// Keyboard sticks, set by the UI thread.
    kb: crate::sim::Controls,
    history: VecDeque<[f32; 4]>,
    steps: u64,
    /// Keyboard only: hold this height (after "Settle mid-air") until a throttle key is used.
    hold: Option<f32>,
}

pub fn run(tuning: Tuning, no_device: bool, selftest: Option<f32>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1560.0, 980.0]).with_title("Quad sound: PROTOTYPE for #34"),
        ..Default::default()
    };
    eframe::run_native("quad-sound", options, Box::new(move |_cc| {
            let mut app = App::new(tuning, no_device);
            if let Some(s) = selftest {
                app.selftest = Some((Instant::now(), s, false));
                app.tuning.volumes.master = 0.0;
                app.set_quad_map(QuadKind::Freestyle5, MapKind::Bando);
                app.start_script(ScriptKind::PunchOut);
            }
            Ok(Box::new(app))
        })).unwrap();
}

struct App {
    tuning: Tuning,
    quad: QuadKind,
    map: MapKind,
    maps: HashMap<MapKind, Arc<MapGeom>>,
    engine: Engine,
    stream: Option<CpalStream>,
    no_device_flag: bool,
    device_note: String,
    notice_shown: bool,
    block_req: Option<u32>,
    retry_at: Instant,
    shared: Arc<Mutex<Shared>>,
    input: SdlInput,
    paused: bool,
    kb_throttle: f32,
    status: String,
    cpu_peak: f64,
    cpu_shown: f64,
    cpu_t: Instant,
    last_tuning: Tuning,
    trail: VecDeque<glam::Vec3>,
    selftest: Option<(Instant, f32, bool)>,
    ui_frames: u64,
}

impl App {
    fn new(tuning: Tuning, no_device: bool) -> Self {
        let maps = maps();
        let quad = QuadKind::Whoop65;
        let map = MapKind::Bando;
        let (engine, writers) = Engine::new(&tuning, quad);
        let sim = Sim::new(quad, maps[&map].clone());
        let shared = Arc::new(Mutex::new(Shared {
            sim,
            run: None,
            writers,
            source: Source::Keyboard,
            r2_throttle: false,
            kb: crate::sim::Controls { angle_mode: true, ..Default::default() },
            history: VecDeque::new(),
            steps: 0,
            hold: None,
        }));
        let input = input::start();
        spawn_sim_thread(shared.clone(), input.clone());
        let mut app = App {
            last_tuning: tuning.clone(),
            tuning,
            quad,
            map,
            maps,
            engine,
            stream: None,
            no_device_flag: no_device,
            device_note: String::new(),
            notice_shown: false,
            block_req: Some(256),
            retry_at: Instant::now(),
            shared,
            input,
            paused: false,
            kb_throttle: 0.0,
            status: String::new(),
            cpu_peak: 0.0,
            cpu_shown: 0.0,
            cpu_t: Instant::now(),
            trail: VecDeque::new(),
            selftest: None,
            ui_frames: 0,
        };
        app.start_stream();
        app
    }

    fn start_stream(&mut self) {
        self.stream = None;
        if self.no_device_flag {
            self.device_note = "No sound device (--no-device): running silent".into();
            return;
        }
        // Wait for the old processor to come back before starting again.
        let t0 = Instant::now();
        while self.engine.cx.is_active() && t0.elapsed() < Duration::from_secs(2) {
            self.engine.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        let cfg = CpalConfig { output: CpalOutputConfig { desired_block_frames: self.block_req, ..Default::default() }, input: None };
        match CpalStream::new(&mut self.engine.cx, cfg) {
            Ok(s) => {
                self.engine.sr = s.info().sample_rate.get();
                let name = s
                    .info()
                    .out_device_id
                    .as_ref()
                    .and_then(|id| {
                        firewheel::cpal::default_host_enumerator().output_devices().into_iter().find(|d| &d.id == id).and_then(|d| d.name)
                    })
                    .unwrap_or_else(|| "default".into());
                self.device_note = format!("Sound: {name}");
                self.stream = Some(s);
                self.engine.update();
                self.engine.apply_tuning(&self.tuning, self.quad);
                let bg = background_for(&self.tuning, self.map);
                self.engine.set_background(&bg, &self.tuning);
                self.engine.cx.set_flags(firewheel::FirewheelFlags { profile_nodes: false, ..Default::default() }).ok();
            }
            Err(e) => {
                self.device_note = format!("No sound device: running silent ({e})");
            }
        }
    }

    fn selftest_report(&mut self, extra: &str) {
        let st = &self.engine.stats;
        println!(
            "selftest: {} | stream {} | {} Hz, block {}, output delay {:.1} ms, tick age {:.2} ms, CPU peak {:.1}% | sim steps {} | ui frames {} | {}",
            self.device_note,
            if self.stream.is_some() { "open" } else { "none" },
            st.sample_rate.load(Ordering::Relaxed),
            st.block_frames.load(Ordering::Relaxed),
            st.playback_delay_us.load(Ordering::Relaxed) as f32 / 1000.0,
            st.frame_age_us.load(Ordering::Relaxed) as f32 / 1000.0,
            self.cpu_shown * 100.0,
            self.shared.lock().unwrap().steps,
            self.ui_frames,
            extra
        );
    }

    fn menu_click(&mut self) {
        let f = self.tuning.clips.menu_click.clone();
        self.engine.play_menu(&f);
    }

    fn set_quad_map(&mut self, quad: QuadKind, map: MapKind) {
        if quad == self.quad && map == self.map {
            return;
        }
        self.quad = quad;
        self.map = map;
        let mut sh = self.shared.lock().unwrap();
        let controls = sh.sim.controls;
        sh.sim = Sim::new(quad, self.maps[&map].clone());
        sh.sim.controls = controls;
        sh.run = None;
        drop(sh);
        self.trail.clear();
        self.engine.apply_tuning(&self.tuning, quad);
        let bg = background_for(&self.tuning, map);
        self.engine.set_background(&bg, &self.tuning);
        self.menu_click();
    }

    fn cmd(&mut self, c: Cmd) {
        self.shared.lock().unwrap().sim.apply(c);
    }

    fn start_script(&mut self, k: ScriptKind) {
        let mut sh = self.shared.lock().unwrap();
        sh.run = Some(ScriptRun::new(k));
    }

    fn pause(&mut self, p: bool) {
        self.paused = p;
        self.cmd(Cmd::Pause(p));
        self.engine.set_paused(p, &self.tuning.clone());
        if p {
            self.menu_click();
        } else {
            let f = self.tuning.clips.menu_back.clone();
            self.engine.play_menu(&f);
        }
    }
}

fn spawn_sim_thread(shared: Arc<Mutex<Shared>>, input: SdlInput) {
    std::thread::Builder::new()
        .name("sim".into())
        .spawn(move || {
            let start = Instant::now();
            let mut sim_t = 0.0f64;
            loop {
                let now = start.elapsed().as_secs_f64();
                {
                    let mut sh = shared.lock().unwrap();
                    // Sticks: a device read straight from the SDL thread's latest values.
                    let dev_sticks = if let Source::Device(i) = sh.source {
                        let inp = input.0.lock().unwrap();
                        inp.devices.get(i).filter(|d| d.connected).map(|d| input::read_sticks(d, sh.r2_throttle))
                    } else {
                        None
                    };
                    let mut steps = 0;
                    while sim_t < now && steps < 200 {
                        let Shared { sim, run, kb, hold, .. } = &mut *sh;
                        if let Some(r) = run {
                            if !r.step(sim, DT) {
                                *run = None;
                            }
                        } else if !sim.paused {
                            if sim.path_kind().is_none() {
                                sim.throttle_mode = match hold {
                                    Some(h) if dev_sticks.is_none() => Throttle::Alt(*h),
                                    _ => Throttle::Stick,
                                };
                            }
                            let arm = sim.controls.arm;
                            if let Some(s) = &dev_sticks {
                                sim.controls.throttle = s.throttle;
                                sim.controls.roll = s.roll;
                                sim.controls.pitch = s.pitch;
                                sim.controls.yaw = s.yaw;
                                sim.controls.arm = s.arm.unwrap_or(arm);
                            } else {
                                sim.controls.throttle = kb.throttle.max(0.0);
                                sim.controls.roll = kb.roll;
                                sim.controls.pitch = kb.pitch;
                                sim.controls.yaw = kb.yaw;
                            }
                        }
                        sh.sim.step();
                        sim_t += DT as f64;
                        steps += 1;
                        sh.steps += 1;
                        if sh.steps % 2 == 0 {
                            let f = sh.sim.frame();
                            sh.writers[0].write(f);
                            sh.writers[1].write(f);
                        }
                        if sh.steps % 20 == 0 {
                            let w = sh.sim.omega;
                            sh.history.push_back(w);
                            if sh.history.len() > 300 {
                                sh.history.pop_front();
                            }
                        }
                    }
                    if steps >= 200 {
                        sim_t = now; // fell behind (window dragged etc.): don't try to catch up
                    }
                }
                std::thread::sleep(Duration::from_micros(500));
            }
        })
        .unwrap();
}

fn knob_ui(ui: &mut egui::Ui, knobs: Vec<Knob<'_>>) {
    let mut group = "";
    for (g, label, v, lo, hi, help) in knobs {
        if g != group {
            group = g;
            ui.add_space(4.0);
            ui.label(egui::RichText::new(g).strong());
        }
        let r = ui.add(egui::Slider::new(v, lo..=hi).text(label));
        if !help.is_empty() {
            r.on_hover_text(help);
        }
    }
}

fn clip_combo(ui: &mut egui::Ui, label: &str, value: &mut String, manifest: &[ClipEntry], group: &str) -> bool {
    let before = value.clone();
    let short = |f: &str| f.rsplit('/').next().unwrap_or(f).trim_end_matches(".ogg").to_string();
    egui::ComboBox::from_label(label).selected_text(short(value)).width(240.0).show_ui(ui, |ui| {
        for c in manifest.iter().filter(|c| c.group == group) {
            ui.selectable_value(value, c.file.clone(), short(&c.file)).on_hover_text(format!(
                "{}\n{} — {}\n{}\n{}",
                c.title, c.author, c.licence, c.source_page, c.notes
            ));
        }
    });
    *value != before
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Runs even while the window is hidden, so sound keeps going in the background.
        ctx.request_repaint_after(Duration::from_millis(15));
        if let Some((t0, secs, requested)) = &mut self.selftest {
            if self.cpu_t.elapsed() > Duration::from_millis(990) {
                eprintln!("selftest: running, t={:.1} s, window shown: {}", t0.elapsed().as_secs_f32(), self.ui_frames > 0);
            }
            if !*requested && t0.elapsed().as_secs_f32() > *secs {
                *requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let give_up = *requested && t0.elapsed().as_secs_f32() > *secs + 3.0;
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
            });
            if give_up && shot.is_none() {
                self.selftest_report("no screenshot: the window was hidden or covered, so eframe drew nothing");
                std::process::exit(0);
            }
            if let Some(img) = shot {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("results/ui-screenshot.png");
                let f = std::fs::File::create(&path).unwrap();
                let mut enc = png::Encoder::new(std::io::BufWriter::new(f), img.size[0] as u32, img.size[1] as u32);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                enc.write_header().unwrap().write_image_data(&bytes).unwrap();
                self.selftest_report(&format!("screenshot {}", path.display()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        // Keep the Firewheel context fed; reopen the device if it went away.
        self.engine.update();
        if let Some(s) = &mut self.stream {
            for e in s.poll_status() {
                eprintln!("[audio] stream: {e}");
            }
            if !s.output_stream_ok() {
                eprintln!("[audio] output stopped (device changed?): reopening the default device");
                self.start_stream();
            }
        } else if !self.no_device_flag && Instant::now() >= self.retry_at {
            self.retry_at = Instant::now() + Duration::from_secs(3);
            self.start_stream();
        }
        let p = self.engine.cx.profiling_data().overall_cpu_usage;
        self.cpu_peak = self.cpu_peak.max(p);
        if self.cpu_t.elapsed() > Duration::from_secs(1) {
            self.cpu_shown = self.cpu_peak;
            self.cpu_peak = 0.0;
            self.cpu_t = Instant::now();
        }

        // Hits from the simulation -> CC0 clips.
        let events: Vec<SimEvent> = self.shared.lock().unwrap().sim.events.drain(..).collect();
        for ev in events {
            let SimEvent::Hit { speed } = ev;
            let f = hit_for(&self.tuning, self.quad);
            let g = hit_gain(&self.tuning, self.quad, speed);
            self.engine.play_hit(&f, g);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ui_frames += 1;
        // Keyboard (fallback; flying with the keyboard is out of the alpha).
        let (w, s, sp, x, l, r, u, d, q, e, ret, back, esc) = ctx.input(|i| {
            use egui::Key::*;
            (
                i.key_down(W),
                i.key_down(S),
                i.key_down(Space),
                i.key_down(X),
                i.key_down(ArrowLeft),
                i.key_down(ArrowRight),
                i.key_down(ArrowUp),
                i.key_down(ArrowDown),
                i.key_down(Q),
                i.key_down(E),
                i.key_pressed(Enter),
                i.key_pressed(Backspace),
                i.key_pressed(Escape),
            )
        });
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        if w {
            self.kb_throttle = (self.kb_throttle + 0.5 * dt).min(1.0);
        }
        if s {
            self.kb_throttle = (self.kb_throttle - 0.5 * dt).max(0.0);
        }
        if esc {
            self.pause(!self.paused);
        }
        if back {
            self.cmd(Cmd::PowerUp);
        }
        {
            let mut sh = self.shared.lock().unwrap();
            if ret {
                let a = !sh.sim.controls.arm;
                sh.sim.controls.arm = a;
            }
            if w || s || sp || x {
                sh.hold = None;
            }
            sh.kb.throttle = if sp { 1.0 } else if x { 0.0 } else { self.kb_throttle };
            sh.kb.roll = (r as i32 - l as i32) as f32 * 0.6;
            sh.kb.pitch = (u as i32 - d as i32) as f32 * 0.6;
            sh.kb.yaw = (e as i32 - q as i32) as f32 * 0.6;
        }


        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("PROTOTYPE (#34): throwaway. The flight model is a rough stand-in, not the game's physics.").color(egui::Color32::from_rgb(230, 170, 60)));
                ui.separator();
                ui.label(&self.device_note);
                ui.separator();
                let st = &self.engine.stats;
                let bf = st.block_frames.load(Ordering::Relaxed);
                let sr = st.sample_rate.load(Ordering::Relaxed).max(1);
                let delay = st.playback_delay_us.load(Ordering::Relaxed) as f32 / 1000.0;
                let age = st.frame_age_us.load(Ordering::Relaxed) as f32 / 1000.0;
                if self.stream.is_some() {
                    ui.label(format!(
                        "{} Hz · block {} ({:.1} ms) · output delay {:.1} ms · tick age {:.1} ms · sound CPU peak {:.1}% of the block",
                        sr,
                        bf,
                        bf as f32 / sr as f32 * 1000.0,
                        delay,
                        age,
                        self.cpu_shown * 100.0
                    ));
                } else {
                    ui.label("no stream");
                }
                egui::ComboBox::from_label("block request")
                    .selected_text(self.block_req.map(|b| b.to_string()).unwrap_or("default".into()))
                    .show_ui(ui, |ui| {
                        for b in [Some(64), Some(128), Some(256), Some(512), Some(1024), None] {
                            if ui.selectable_label(self.block_req == b, b.map(|b| b.to_string()).unwrap_or("default".into())).clicked() {
                                self.block_req = b;
                                self.start_stream();
                            }
                        }
                    });
            });
        });
        if !self.notice_shown && self.stream.is_none() && !self.device_note.is_empty() {
            // "No sound device at start: the game runs silent and says so once on the Hub."
            egui::Window::new("Sound").collapsible(false).show(&ctx, |ui| {
                ui.label(&self.device_note);
                if ui.button("OK").clicked() {
                    self.notice_shown = true;
                }
            });
        }

        egui::Panel::left("left").default_size(390.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.left_panel(ui));
        });
        egui::Panel::right("right").default_size(470.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.right_panel(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.center(ui));

        if self.tuning != self.last_tuning {
            self.engine.apply_tuning(&self.tuning, self.quad);
            let bg = background_for(&self.tuning, self.map);
            self.engine.set_background(&bg, &self.tuning);
            self.last_tuning = self.tuning.clone();
        }
    }
}

impl App {
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Fly and listen");
        let (mut quad, mut map) = (self.quad, self.map);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut quad, QuadKind::Whoop65, "Whoop 65");
            ui.selectable_value(&mut quad, QuadKind::Freestyle5, "Freestyle 5\"");
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut map, MapKind::SkatePark, "Skate Park");
            ui.selectable_value(&mut map, MapKind::Bando, "Bando");
        });
        self.set_quad_map(quad, map);
        ui.horizontal(|ui| {
            ui.label("Listening Position:");
            ui.selectable_value(&mut self.tuning.listener.where_you_stand, false, "On the Quad");
            ui.selectable_value(&mut self.tuning.listener.where_you_stand, true, "Where you stand");
        });
        ui.separator();
        ui.label(egui::RichText::new("Sticks").strong());
        let devices: Vec<(usize, String, bool)> =
            self.input.0.lock().unwrap().devices.iter().enumerate().map(|(i, d)| (i, d.name.clone(), d.connected)).collect();
        {
            let mut sh = self.shared.lock().unwrap();
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut sh.source, Source::Keyboard, "Keyboard");
                for (i, n, c) in &devices {
                    if *c {
                        ui.selectable_value(&mut sh.source, Source::Device(*i), n);
                    }
                }
            });
            ui.checkbox(&mut sh.r2_throttle, "Gamepad: throttle on R2 (else left stick, rests at 50%)");
            let mut arm = sh.sim.controls.arm;
            let mut angle = sh.sim.controls.angle_mode;
            let mut flip = sh.sim.controls.crash_flip;
            ui.horizontal(|ui| {
                ui.checkbox(&mut arm, "Arm (Enter)");
                ui.checkbox(&mut angle, "Angle mode");
                ui.checkbox(&mut flip, "Crash Flip");
            });
            sh.sim.controls.arm = arm;
            sh.sim.controls.angle_mode = angle;
            sh.sim.controls.crash_flip = flip;
        }
        ui.label("Keys: W/S throttle, Space punch, X chop, arrows roll/pitch, Q/E yaw, Enter arm, Backspace Reset, Esc Pause Menu. A Radio's CH5 arms.");
        ui.add(egui::Slider::new(&mut self.kb_throttle, 0.0..=1.0).text("keyboard throttle"));
        ui.separator();
        ui.label(egui::RichText::new("Scripted flights (repeatable)").strong());
        for k in SCRIPTS {
            if ui.button(k.label()).clicked() {
                self.start_script(k);
            }
        }
        ui.label(egui::RichText::new("Where-you-stand passes").strong());
        for p in ALL_PATHS {
            if ui.add_enabled(p.fits(self.map), egui::Button::new(p.label())).clicked() {
                self.tuning.listener.where_you_stand = true;
                self.start_script(ScriptKind::Path(p));
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("Events").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reset (power-up)").clicked() {
                self.cmd(Cmd::PowerUp);
            }
            if ui.button("Settle mid-air").clicked() {
                self.cmd(Cmd::SettleInAir(2.0));
                self.shared.lock().unwrap().hold = Some(2.0);
            }
            if ui.button("Prop Strike").clicked() {
                self.cmd(Cmd::Strike { motor: 1, rub: 0.6, secs: 0.2 });
            }
            if ui.button("Jam a prop").clicked() {
                self.cmd(Cmd::Jam { motor: 2 });
            }
            if ui.button("Hit (5 m/s)").clicked() {
                self.cmd(Cmd::Hit { speed: 5.0 });
            }
            let link = self.shared.lock().unwrap().sim.link_ok;
            if ui.button(if link { "Lose the link" } else { "Link back" }).clicked() {
                self.cmd(Cmd::Link(!link));
            }
            if ui.button("Skip 10 min idle").clicked() {
                self.cmd(Cmd::IdleTenMinutes);
            }
            if ui.button(if self.paused { "Resume" } else { "Pause Menu" }).clicked() {
                let p = !self.paused;
                self.pause(p);
            }
        });
        {
            let mut sh = self.shared.lock().unwrap();
            let mut c = sh.sim.charge;
            ui.add(egui::Slider::new(&mut c, 0.0..=1.0).text("pack charge"));
            if c != sh.sim.charge {
                sh.sim.apply(Cmd::Charge(c));
            }
        }
        ui.separator();
        self.status_ui(ui);
    }

    fn status_ui(&mut self, ui: &mut egui::Ui) {
        let sh = self.shared.lock().unwrap();
        let sim = &sh.sim;
        let def = &sim.def;
        ui.label(egui::RichText::new("State").strong());
        let script = sh.run.as_ref().map(|r| format!("{} ({:.1} s)", r.kind.label(), r.t)).unwrap_or("—".into());
        ui.label(format!("script: {script}"));
        ui.label(format!(
            "armed: {}  {}  ESCs: {}  link: {}{}",
            sim.armed,
            if sim.arm_block.is_empty() { String::new() } else { format!("arming blocked: {}", sim.arm_block) },
            if sim.esc_ready { "ready" } else { "starting up" },
            if sim.link_ok { "ok" } else { "LOST" },
            if sim.failsafe { " FAILSAFE" } else { "" }
        ));
        ui.label(format!("buzzer: {}   battery {:.2} V ({:.2} V/cell)", sim.beeper_label(), sim.v_batt, sim.v_batt / def.cells as f32));
        ui.label(format!(
            "height {:.1} m  speed {:.1} m/s  downwash {:.1} m/s  prop wash {:.0}%",
            sim.pos.z - sim.map.launch.z,
            sim.airspeed,
            sim.downwash,
            sim.wash * 100.0
        ));
        egui::Grid::new("motors").striped(true).show(ui, |ui| {
            ui.label("motor");
            ui.label("RPM");
            ui.label("blade-pass Hz");
            ui.label("current A");
            ui.label("state");
            ui.end_row();
            for i in 0..4 {
                let rpm = sim.omega[i] * 60.0 / std::f32::consts::TAU;
                ui.label(format!("{}", i + 1));
                ui.label(format!("{:.0}", rpm));
                ui.label(format!("{:.0}", rpm / 60.0 * def.blades as f32));
                ui.label(format!("{:.2}", sim.current[i]));
                ui.label(sim.motor_state_label(i));
                ui.end_row();
            }
        });
        let st = &self.engine.stats;
        ui.label(format!(
            "to the pilot: {:.1} m, delay {:.0} ms, Doppler x{:.3}, walls {:.1}",
            load_f(&st.distance_m),
            load_f(&st.delay_ms),
            load_f(&st.doppler),
            load_f(&st.walls)
        ));
        ui.label(format!("compressor gain change {:.1} dB, wind level {:.2}", load_f(&st.gain_reduction_db), load_f(&st.wind)));
        ui.label(
            egui::RichText::new(format!(
                "model: {} kg, {}-blade {:.0} mm props, {:.0} KV, {}S, hover ≈{:.0} RPM, max ≈{:.0} RPM",
                def.mass,
                def.blades,
                def.prop_d * 1000.0,
                def.kv_rpm,
                def.cells,
                (def.mass * 9.81 / 4.0 / def.k_f).sqrt() * 60.0 / std::f32::consts::TAU,
                def.omega_max() * 60.0 / std::f32::consts::TAU
            ))
            .small(),
        );
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tuning");
        ui.horizontal(|ui| {
            if ui.button("Save tuning").clicked() {
                let stamp = std::process::Command::new("date")
                    .arg("+%Y%m%d-%H%M%S")
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default();
                let dir = crate::tuning_dir();
                let a = self.tuning.save(&dir.join("current.toml"));
                let b = self.tuning.save(&dir.join(format!("saved-{stamp}.toml")));
                self.status = match (a, b) {
                    (Ok(()), Ok(())) => format!("saved tuning/current.toml and tuning/saved-{stamp}.toml"),
                    (Err(e), _) | (_, Err(e)) => format!("save failed: {e}"),
                };
            }
            if ui.button(format!("Reset {} to defaults", self.quad.label())).clicked() {
                *self.tuning.block_mut(self.quad) = crate::quads::SoundBlock::for_quad(self.quad);
            }
            if ui.button("Reset listener").clicked() {
                let w = self.tuning.listener.where_you_stand;
                self.tuning.listener = Default::default();
                self.tuning.listener.where_you_stand = w;
            }
        });
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        egui::CollapsingHeader::new("Group volumes (the Sound tab)").default_open(true).show(ui, |ui| {
            let v = &mut self.tuning.volumes;
            ui.add(egui::Slider::new(&mut v.master, 0.0..=1.0).text("Master (0 = off)"));
            ui.add(egui::Slider::new(&mut v.quad, 0.0..=1.0).text("Quad: motors, props, wind, beeps"));
            ui.add(egui::Slider::new(&mut v.crashes, 0.0..=1.0).text("Crashes: Prop Strikes, hits"));
            ui.add(egui::Slider::new(&mut v.background, 0.0..=1.0).text("Background"));
            ui.add(egui::Slider::new(&mut v.menus, 0.0..=1.0).text("Menus"));
            ui.add(egui::Slider::new(&mut v.pause_background, 0.0..=1.0).text("Background while paused"));
        });
        let label = format!("Sound block: {}", self.quad.label());
        egui::CollapsingHeader::new(label).default_open(true).show(ui, |ui| {
            ui.label(egui::RichText::new("Set by ear; no Confidence. Physics numbers are not here on purpose.").small());
            let b = self.tuning.block_mut(self.quad);
            knob_ui(ui, b.knobs());
        });
        egui::CollapsingHeader::new("On the Quad (camera mic)").default_open(true).show(ui, |ui| {
            let l = &mut self.tuning.listener;
            ui.horizontal(|ui| {
                ui.checkbox(&mut l.comp_on, "compressor");
                ui.checkbox(&mut l.clip_on, "clipping (try it)");
            });
            knob_ui(ui, l.knobs_onquad());
        });
        egui::CollapsingHeader::new("Where you stand").default_open(true).show(ui, |ui| {
            let l = &mut self.tuning.listener;
            ui.horizontal(|ui| {
                ui.checkbox(&mut l.delay_on, "travel time (and so Doppler)");
                ui.checkbox(&mut l.wall_muffle_on, "wall muffling");
            });
            knob_ui(ui, l.knobs_stand());
        });
        egui::CollapsingHeader::new("CC0 clips").default_open(true).show(ui, |ui| {
            let m = self.engine.manifest.clone();
            let c = &mut self.tuning.clips;
            let mut preview: Option<(String, bool)> = None;
            ui.horizontal(|ui| {
                clip_combo(ui, "whoop hit", &mut c.whoop_hit, &m, "hits");
                if ui.small_button("▶").clicked() {
                    preview = Some((c.whoop_hit.clone(), true));
                }
            });
            ui.horizontal(|ui| {
                clip_combo(ui, "5\" hit", &mut c.five_hit, &m, "hits");
                if ui.small_button("▶").clicked() {
                    preview = Some((c.five_hit.clone(), true));
                }
            });
            ui.horizontal(|ui| {
                clip_combo(ui, "menu click", &mut c.menu_click, &m, "menus");
                if ui.small_button("▶").clicked() {
                    preview = Some((c.menu_click.clone(), false));
                }
            });
            ui.horizontal(|ui| {
                clip_combo(ui, "menu back", &mut c.menu_back, &m, "menus");
                if ui.small_button("▶").clicked() {
                    preview = Some((c.menu_back.clone(), false));
                }
            });
            clip_combo(ui, "Skate Park background", &mut c.skate_park_background, &m, "background-skate-park");
            ui.add(egui::Slider::new(&mut c.skate_park_level, 0.0..=2.0).text("Skate Park level"));
            clip_combo(ui, "Bando background", &mut c.bando_background, &m, "background-bando");
            ui.add(egui::Slider::new(&mut c.bando_level, 0.0..=2.0).text("Bando level"));
            if let Some((f, hit)) = preview {
                if hit {
                    let g = self.tuning.block(self.quad).hit_level;
                    self.engine.play_hit(&f, g);
                } else {
                    self.engine.play_menu(&f);
                }
            }
        });
    }

    fn center(&mut self, ui: &mut egui::Ui) {
        let (pos, listener, launch, history, walls) = {
            let sh = self.shared.lock().unwrap();
            (sh.sim.pos, sh.sim.listener, sh.sim.map.launch, sh.history.clone(), sh.sim.walls)
        };
        self.trail.push_back(pos);
        if self.trail.len() > 600 {
            self.trail.pop_front();
        }
        let map = self.maps[&self.map].clone();
        ui.label(format!("{} from above: grey = Map parts (darker = taller), green = Launch Spot, blue = you, orange = the Quad", self.map.label()));
        let avail = ui.available_size();
        let h = (avail.y * 0.62).max(200.0);
        let (resp, painter) = ui.allocate_painter(egui::vec2(avail.x, h), egui::Sense::hover());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(28, 30, 34));
        // View the built-up area (skip the 400 m ground planes), plus the pilot and the Quad.
        let near = |o: &&crate::map::Obj| (o.max.x - o.min.x) < 120.0 && (o.max.y - o.min.y) < 120.0;
        let mut min = launch.min(listener).min(pos);
        let mut max = launch.max(listener).max(pos);
        for o in map.objects.iter().filter(near) {
            if (o.min - launch).length() < 90.0 || (o.max - launch).length() < 90.0 {
                min = min.min(o.min);
                max = max.max(o.max);
            }
        }
        let span = (max.x - min.x).max(max.y - min.y).max(1.0) * 1.1;
        let scale = (rect.width().min(rect.height())) / span;
        let c = (min + max) * 0.5;
        let to = |p: glam::Vec3| egui::pos2(rect.center().x + (p.x - c.x) * scale, rect.center().y - (p.y - c.y) * scale);
        let mut objs: Vec<_> = map.objects.iter().collect();
        objs.sort_by(|a, b| a.max.z.partial_cmp(&b.max.z).unwrap());
        for o in objs.into_iter().filter(near) {
            let tall = ((o.max.z - min.z) / 20.0).clamp(0.0, 1.0);
            let g = (110.0 - 70.0 * tall) as u8;
            let r = egui::Rect::from_two_pos(to(o.min), to(o.max));
            painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(g + 40, g + 40, g + 45, 120));
        }
        let pts: Vec<egui::Pos2> = self.trail.iter().map(|p| to(*p)).collect();
        if pts.len() > 1 {
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, egui::Color32::from_rgb(200, 120, 40))));
        }
        let line_col = if walls > 0.0 { egui::Color32::from_rgb(220, 60, 60) } else { egui::Color32::from_gray(120) };
        painter.line_segment([to(pos), to(listener)], egui::Stroke::new(1.0, line_col));
        painter.circle_filled(to(launch), 5.0, egui::Color32::from_rgb(60, 200, 90));
        painter.circle_filled(to(listener), 5.0, egui::Color32::from_rgb(80, 140, 240));
        painter.circle_filled(to(pos), 6.0, egui::Color32::from_rgb(250, 150, 40));
        painter.text(
            rect.left_top() + egui::vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!("red line = walls between the Quad and you ({walls:.0})"),
            egui::FontId::proportional(12.0),
            egui::Color32::LIGHT_GRAY,
        );
        // Motor speeds over the last ~3 s: the warble you should hear.
        ui.label("Motor speeds, last 3 s (each line is a motor)");
        let avail = ui.available_size();
        let (resp, painter) = ui.allocate_painter(egui::vec2(avail.x, (avail.y - 10.0).max(120.0)), egui::Sense::hover());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 22, 26));
        let wmax = QuadDef::get(self.quad).omega_max();
        let cols = [
            egui::Color32::from_rgb(240, 100, 100),
            egui::Color32::from_rgb(100, 220, 120),
            egui::Color32::from_rgb(110, 160, 250),
            egui::Color32::from_rgb(240, 210, 90),
        ];
        let n = history.len().max(2);
        for m in 0..4 {
            let pts: Vec<egui::Pos2> = history
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    egui::pos2(
                        rect.left() + rect.width() * i as f32 / (n - 1) as f32,
                        rect.bottom() - rect.height() * (w[m] / wmax).clamp(0.0, 1.05) / 1.05,
                    )
                })
                .collect();
            if pts.len() > 1 {
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.2, cols[m])));
            }
        }
        painter.text(
            rect.left_top() + egui::vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("top = {:.0} RPM", wmax * 60.0 / std::f32::consts::TAU * 1.05),
            egui::FontId::proportional(11.0),
            egui::Color32::GRAY,
        );
    }
}
