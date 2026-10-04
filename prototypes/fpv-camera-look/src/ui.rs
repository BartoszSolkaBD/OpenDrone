//! PROTOTYPE (#28). The on-screen tuning panel and the keyboard shortcuts. Everything the
//! maintainer tunes by eye is here, with the live numbers behind it.

use crate::input_sdl::SdlInput;
use crate::map::MapState;
use crate::rig::{self, Control, Recording, Rig};
use crate::settings::{self, Aspect, BreakupLevel, Look, MapKind, QuadKind, Tuning};
use crate::signal::VideoSignal;
use crate::{stats, Flash, LaunchArgs, MapRequest, Measurements};
use bevy::prelude::*;
use bevy::render::view::screenshot::save_to_disk;
use bevy_egui::{egui, EguiContexts};

#[derive(Resource, Default)]
pub struct EguiBusy {
    pub keyboard: bool,
    pub pointer: bool,
}

#[derive(Resource)]
pub struct UiState {
    pub hidden: bool,
    pub save_name: String,
    pub saved: Vec<String>,
    pub zoom_set: bool,
    pub last_recording: Option<Recording>,
}

impl Default for UiState {
    fn default() -> Self {
        Self { hidden: false, save_name: "my-tuning".into(), saved: settings::list_saved(), zoom_set: false, last_recording: None }
    }
}

fn take_screenshot(commands: &mut Commands, flash: &mut Flash, now: f64, output: &crate::Output) {
    let dir = settings::proto_dir().join("screenshots");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("shot-{}.png", crate::now_stamp()));
    flash.0 = Some((format!("Screenshot saved: {}", path.display()), now));
    commands.spawn(crate::screenshot_of(output)).observe(save_to_disk(path));
}

#[allow(clippy::too_many_arguments)]
pub fn shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    busy: Res<EguiBusy>,
    mut tuning: ResMut<Tuning>,
    mut ui: ResMut<UiState>,
    mut rig: ResMut<Rig>,
    map: Res<MapState>,
    mut req: ResMut<MapRequest>,
    mut commands: Commands,
    mut flash: ResMut<Flash>,
    time: Res<Time<Real>>,
    mut repeat: Local<(f32, Option<KeyCode>)>,
    mut autosave: Local<(f64,)>,
    mut exit: MessageWriter<AppExit>,
    output: Res<crate::Output>,
) {
    let now = time.elapsed_secs_f64();
    // Autosave every 10 s when something changed, so a tuning session is never lost.
    if tuning.is_changed() && now - autosave.0 > 10.0 {
        autosave.0 = now;
        let dir = settings::saved_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("autosave.toml"), toml::to_string_pretty(&*tuning).unwrap_or_default());
    }
    if keys.pressed(KeyCode::ShiftLeft) && keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if busy.keyboard {
        return;
    }
    if keys.just_pressed(KeyCode::KeyH) || keys.just_pressed(KeyCode::F1) {
        ui.hidden = !ui.hidden;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        tuning.look = if tuning.look == Look::Analog { Look::Digital } else { Look::Analog };
    }
    if keys.just_pressed(KeyCode::KeyB) {
        tuning.breakup = tuning.breakup.next();
    }
    if keys.just_pressed(KeyCode::KeyM) {
        req.0 = Some(if map.current == Some(MapKind::Bando) { MapKind::SkatePark } else { MapKind::Bando });
    }
    if keys.just_pressed(KeyCode::KeyR) {
        if rig.control == Control::Path {
            rig::restart_path(&mut rig);
            rig.path_playing = true;
        } else {
            rig::reset_to_launch(&mut rig, &map);
        }
    }
    if keys.just_pressed(KeyCode::KeyP) && rig.control == Control::Path {
        rig.path_playing = !rig.path_playing;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        rig.control = Control::FreeFly;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        rig.control = Control::Acro;
        rig::reset_to_launch(&mut rig, &map);
    }
    if keys.just_pressed(KeyCode::F12) {
        take_screenshot(&mut commands, &mut flash, now, &output);
    }
    // Camera Tilt (up/down) and FOV (left/right), 1 degree per press, repeat after 1 s every 200 ms (#23).
    let dirs = [
        (KeyCode::ArrowUp, 0, 1.0f32),
        (KeyCode::ArrowDown, 0, -1.0),
        (KeyCode::ArrowRight, 1, 1.0),
        (KeyCode::ArrowLeft, 1, -1.0),
    ];
    for (k, which, sign) in dirs {
        let mut fire = false;
        if keys.just_pressed(k) {
            fire = true;
            *repeat = (0.0, Some(k));
        } else if keys.pressed(k) && repeat.1 == Some(k) {
            repeat.0 += time.delta_secs();
            if repeat.0 >= 1.0 {
                fire = true;
                repeat.0 -= 0.2;
            }
        }
        if fire {
            if which == 0 {
                tuning.tilt_deg = (tuning.tilt_deg + sign).clamp(0.0, 80.0);
            } else {
                tuning.fov_deg = (tuning.fov_deg + sign).clamp(90.0, 170.0);
            }
            flash.0 = Some((format!("Camera Tilt {:.0}°   FOV {:.0}°", tuning.tilt_deg, tuning.fov_deg), now));
        }
    }
}

fn slider(ui: &mut egui::Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>, text: &str) {
    ui.add(egui::Slider::new(v, range).text(text));
}

fn ms(v: f32) -> String {
    if v.is_finite() { format!("{v:.2}") } else { "-".into() }
}

#[allow(clippy::too_many_arguments)]
pub fn panel(
    mut contexts: EguiContexts,
    mut tuning: ResMut<Tuning>,
    mut ui_state: ResMut<UiState>,
    mut rig: ResMut<Rig>,
    map: Res<MapState>,
    mut req: ResMut<MapRequest>,
    sig: Res<VideoSignal>,
    meas: Res<Measurements>,
    input: Res<SdlInput>,
    args: Res<LaunchArgs>,
    mut busy: ResMut<EguiBusy>,
    mut flash: ResMut<Flash>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    output: Res<crate::Output>,
) -> Result {
    let Ok(ctx) = contexts.ctx_mut() else { return Ok(()) };
    if !ui_state.zoom_set {
        ctx.set_zoom_factor(1.2);
        ui_state.zoom_set = true;
    }
    let now = time.elapsed_secs_f64();
    busy.keyboard = ctx.egui_wants_keyboard_input();
    busy.pointer = ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui();

    // The sim's own notice (Tilt/FOV readout, saves) sits crisp on top, outside the video.
    if let Some((text, t0)) = flash.0.clone() {
        if now - t0 < 2.0 {
            egui::Area::new("flash".into()).anchor(egui::Align2::CENTER_TOP, [0.0, 24.0]).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(text).size(20.0).strong());
                });
            });
        } else {
            flash.0 = None;
        }
    }
    if ui_state.hidden {
        return Ok(());
    }

    let n = 240usize;
    let tail = |v: &std::collections::VecDeque<crate::render::FrameGpu>, f: &dyn Fn(&crate::render::FrameGpu) -> f32| {
        stats(v.iter().rev().take(n).map(f).collect())
    };
    let cam = tail(&meas.gpu, &|g| g.mips + g.look);
    let look = tail(&meas.gpu, &|g| g.look);
    let mips = tail(&meas.gpu, &|g| g.mips);
    let cpu = stats(meas.cpu_ms.iter().rev().take(n).copied().collect());

    egui::Window::new("FPV camera look · PROTOTYPE #28")
        .default_pos([8.0, 8.0])
        .default_width(380.0)
        .default_height(1050.0)
        .vscroll(true)
        .show(ctx, |ui| {
            // --- Always visible: what you're looking at, and the two numbers that matter.
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} · FOV {:.0}° · Tilt {:.0}° · {} · Breakup {}",
                    map.current.map(|m| m.label()).unwrap_or("-"),
                    if tuning.look == Look::Analog { "Analog" } else { "Digital" },
                    tuning.fov_deg,
                    tuning.tilt_deg,
                    if tuning.aspect == Aspect::FourThree { "4:3" } else { "16:9" },
                    tuning.breakup.label()
                ))
                .strong(),
            );
            ui.label(format!(
                "{:.0} fps · frame {} ms (p95 {}, 45 fps = 22.2 ms){}",
                1000.0 / cpu.0.max(0.01),
                ms(cpu.0),
                ms(cpu.1),
                if meas.pipelines_waiting > 0 {
                    format!(" · compiling {} shaders, numbers not valid yet", meas.pipelines_waiting)
                } else {
                    String::new()
                }
            ));
            let frac = (cam.0 / 2.0).clamp(0.0, 1.0);
            let col = if cam.0 <= 2.0 { egui::Color32::from_rgb(60, 160, 80) } else { egui::Color32::from_rgb(200, 60, 50) };
            ui.add(
                egui::ProgressBar::new(if cam.0.is_finite() { frac } else { 0.0 })
                    .fill(col)
                    .text(format!("Camera steps on the GPU: {} ms of 2.00 ms (p95 {})", ms(cam.0), ms(cam.1))),
            );
            let sig_line = match tuning.look {
                Look::Analog => format!("Breakup {:.0}%", sig.analog_shown * 100.0),
                Look::Digital => format!("Digital: {} ({:.0}%)", sig.digital_state.label(), sig.digital_shown * 100.0),
            };
            ui.label(format!(
                "Video Signal {:.1} dBm · {:.0} m · {} walls, {:.2} m concrete (−{:.0} dB) · {}",
                sig.rx_dbm, sig.distance_m, sig.walls + sig.open_surfaces, sig.concrete_m, sig.wall_db, sig_line
            ));
            ui.separator();

            egui::CollapsingHeader::new("Fly").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Map:");
                    for m in [MapKind::SkatePark, MapKind::Bando] {
                        if ui.selectable_label(map.current == Some(m), m.label()).clicked() {
                            req.0 = Some(m);
                        }
                    }
                    if !map.built {
                        ui.label("(loading…)");
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Quad:");
                    for q in [QuadKind::Whoop65, QuadKind::Freestyle5] {
                        if ui.selectable_label(tuning.quad == q, q.label()).clicked() && tuning.quad != q {
                            tuning.quad = q;
                            tuning.fov_deg = q.default_fov();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Move by:");
                    let mut c = rig.control;
                    ui.selectable_value(&mut c, Control::Path, "Preset path");
                    ui.selectable_value(&mut c, Control::FreeFly, "Free-fly (F)");
                    ui.selectable_value(&mut c, Control::Acro, "Radio / Gamepad (G)");
                    if ui_state.last_recording.is_some() {
                        ui.selectable_value(&mut c, Control::Replay, "Replay");
                    }
                    if c != rig.control {
                        if c == Control::Acro || c == Control::FreeFly {
                            rig::reset_to_launch(&mut rig, &map);
                        }
                        if c == Control::Replay {
                            rig.replay = ui_state.last_recording.clone();
                            rig.replay_t = 0.0;
                        }
                        rig.control = c;
                    }
                });
                match rig.control {
                    Control::Path => {
                        let cur = map.current.unwrap_or_default();
                        for idx in crate::paths::paths_for(cur) {
                            if ui.selectable_label(rig.path_idx == idx && rig.path.is_some(), crate::paths::PATHS[idx].name).clicked() {
                                rig::select_path(&mut rig, idx);
                                rig.path_playing = true;
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button(if rig.path_playing { "Pause (P)" } else { "Play (P)" }).clicked() {
                                rig.path_playing = !rig.path_playing;
                            }
                            if ui.button("Restart (R)").clicked() {
                                rig::restart_path(&mut rig);
                                rig.path_playing = true;
                            }
                            ui.checkbox(&mut rig.path_loop, "loop");
                        });
                        let len = rig.path.as_ref().map(|p| p.length()).unwrap_or(1.0);
                        let mut d = rig.path_d;
                        if ui.add(egui::Slider::new(&mut d, 0.0..=len).text("position on path (m)")).changed() {
                            let q = tuning.quad;
                            if d < rig.path_d {
                                rig::restart_path(&mut rig);
                            }
                            while rig.path_d < d {
                                rig::advance_path(&mut rig, 1.0 / 120.0, q);
                            }
                        }
                        slider(ui, &mut rig.path_speed, 0.25..=3.0, "path speed ×");
                    }
                    Control::FreeFly => {
                        ui.label("WASD move · E/Space up · Q/C down · Shift faster · hold right mouse to look · wheel = speed");
                        slider(ui, &mut rig.freefly_speed, 0.5..=40.0, "speed m/s");
                    }
                    Control::Acro => {
                        ui.label("Toy rate-mode quad (NOT the alpha's physics). Throttle low = falls. R = back to the Launch Spot.");
                    }
                    Control::Replay => {}
                }
                ui.horizontal(|ui| {
                    if ui.button("Back to Launch Spot (R)").clicked() {
                        rig::reset_to_launch(&mut rig, &map);
                    }
                    let recording = rig.recording.is_some();
                    if ui.button(if recording { "Stop recording" } else { "Record flight" }).clicked() {
                        if let Some(rec) = rig.recording.take() {
                            let dir = settings::saved_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            let p = dir.join(format!("flight-{}.json", crate::now_stamp()));
                            let _ = std::fs::write(&p, serde_json::to_string(&rec).unwrap_or_default());
                            flash.0 = Some((format!("Flight saved: {}", p.display()), now));
                            ui_state.last_recording = Some(rec);
                        } else {
                            rig.recording = Some(Recording { map: map.current.unwrap_or_default(), samples: Vec::new() });
                            rig.rec_t = 0.0;
                        }
                    }
                });
                // Input devices.
                let shared = input.0.lock().unwrap();
                match shared.sdl_ok {
                    Some(true) if shared.devices.is_empty() => {
                        ui.label("SDL: no Radio or Gamepad connected (plug in the Pocket with RF off, or a DualSense).");
                    }
                    Some(true) => {
                        let names: Vec<String> = shared.devices.iter().map(|d| d.name.clone()).collect();
                        let dev_idx = rig.stick_map.device.min(names.len() - 1);
                        egui::ComboBox::from_label("stick device").selected_text(names[dev_idx].clone()).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut rig.stick_map.device, i, n);
                            }
                        });
                        let dev = &shared.devices[dev_idx];
                        let labels = ["roll", "pitch", "throttle", "yaw"];
                        let sticks = rig.sticks;
                        for i in 0..4 {
                            ui.horizontal(|ui| {
                                ui.label(format!("{:<8}", labels[i]));
                                let mut a = rig.stick_map.axis[i] as i32;
                                ui.add(egui::DragValue::new(&mut a).range(0..=(dev.axes.len().max(1) as i32 - 1)).prefix("axis "));
                                rig.stick_map.axis[i] = a.max(0) as usize;
                                ui.checkbox(&mut rig.stick_map.invert[i], "invert");
                                let v = if i == 2 { sticks[2] } else { (sticks[i] + 1.0) * 0.5 };
                                ui.add(egui::ProgressBar::new(v.clamp(0.0, 1.0)).desired_width(90.0).text(format!("{:+.2}", sticks[i])));
                            });
                        }
                    }
                    Some(false) => {
                        ui.label(format!("SDL failed: {}", shared.error));
                    }
                    None => {
                        ui.label("SDL starting…");
                    }
                }
            });

            egui::CollapsingHeader::new("Camera").default_open(true).show(ui, |ui| {
                slider(ui, &mut tuning.fov_deg, 90.0..=170.0, "FOV ° corner to corner (Left/Right arrow keys)");
                slider(ui, &mut tuning.tilt_deg, 0.0..=80.0, "Camera Tilt ° (Up/Down arrow keys)");
                ui.horizontal(|ui| {
                    ui.label("Aspect:");
                    ui.selectable_value(&mut tuning.aspect, Aspect::FourThree, "4:3");
                    ui.selectable_value(&mut tuning.aspect, Aspect::SixteenNine, "16:9");
                    ui.separator();
                    ui.label("Look (V):");
                    ui.selectable_value(&mut tuning.look, Look::Analog, "Analog");
                    ui.selectable_value(&mut tuning.look, Look::Digital, "Digital");
                });
                ui.horizontal(|ui| {
                    ui.label("Breakup (B):");
                    for l in BreakupLevel::ALL {
                        ui.selectable_value(&mut tuning.breakup, l, l.label());
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut tuning.reduce_motion, "Reduce motion (≤3 flashes/s)");
                    ui.checkbox(&mut tuning.show_own_quad, "Own ducts/props");
                });
                if ui.button("Back to #14's starting values (everything)").clicked() {
                    let q = tuning.quad;
                    *tuning = Tuning::default();
                    tuning.quad = q;
                    tuning.fov_deg = q.default_fov();
                }
            });

            let quad_label = tuning.quad.label();
            egui::CollapsingHeader::new(format!("Camera on the {quad_label} (Quad values)")).default_open(true).show(ui, |ui| {
                let c = tuning.camera_mut();
                ui.label("Analog camera");
                ui.push_id("cam_a", |ui| {
                    slider(ui, &mut c.analog.dynamic_range_ev, 4.0..=16.0, "dynamic range (stops)");
                    slider(ui, &mut c.analog.lines, 240.0..=1080.0, "lines top to bottom (NTSC = 480)");
                    slider(ui, &mut c.analog.horizontal, 150.0..=1200.0, "sharpness across (TV lines)");
                });
                ui.label("Digital camera");
                ui.push_id("cam_d", |ui| {
                    slider(ui, &mut c.digital.dynamic_range_ev, 4.0..=16.0, "dynamic range (stops)");
                    slider(ui, &mut c.digital.lines, 360.0..=1440.0, "lines top to bottom (1080p = 1080)");
                });
            });

            egui::CollapsingHeader::new("Analog look").default_open(tuning.look == Look::Analog).show(ui, |ui| {
                let a = &mut tuning.analog;
                slider(ui, &mut a.softness, 0.0..=3.0, "extra softness (1 = the camera alone)");
                slider(ui, &mut a.grain, 0.0..=0.2, "grain");
                slider(ui, &mut a.colour_bleed, 0.0..=10.0, "colour bleed (analog px)");
                slider(ui, &mut a.contrast, 0.6..=1.8, "contrast");
                slider(ui, &mut a.saturation, 0.0..=1.6, "saturation");
                slider(ui, &mut a.brightness, 0.4..=2.0, "brightness");
                slider(ui, &mut a.source_scale, 0.5..=2.5, "Map drawn at × picture pixels");
                slider(ui, &mut a.lens_curve, 0.0..=0.8, "lens curve (0 = stock fisheye, up = gentler)");
                exposure_ui(ui, &mut a.exposure, "analog");
            });

            egui::CollapsingHeader::new("Digital look").default_open(tuning.look == Look::Digital).show(ui, |ui| {
                let d = &mut tuning.digital;
                slider(ui, &mut d.source_scale, 0.5..=2.5, "Map drawn at × picture pixels (centre sharpness)");
                slider(ui, &mut d.lens_curve, 0.0..=0.8, "lens curve (0 = stock fisheye, up = gentler)");
                slider(ui, &mut d.sharpen, 0.0..=1.5, "sharpen");
                slider(ui, &mut d.contrast, 0.6..=1.8, "contrast");
                slider(ui, &mut d.saturation, 0.0..=1.6, "saturation");
                slider(ui, &mut d.brightness, 0.4..=2.0, "brightness");
                slider(ui, &mut d.delay_ms, 0.0..=60.0, "delay behind Analog (ms)");
                slider(ui, &mut d.edge_extra_delay_ms, 0.0..=80.0, "extra delay at edge of range (ms)");
                slider(ui, &mut d.relock_s, 0.0..=3.0, "re-lock after the signal returns (s)");
                exposure_ui(ui, &mut d.exposure, "digital");
            });

            egui::CollapsingHeader::new("Video Signal and Breakup").default_open(false).show(ui, |ui| {
                ui.label(format!(
                    "Now: VTX {:.0} mW ×{} = {:.1} dBm · free space −{:.1} dB · walls −{:.1} dB · flutter {:+.1} dB = {:.1} dBm · ray check {:.0} µs",
                    tuning.vtx_mw(),
                    tuning.power_mult().map(|m| format!("{m:.0}")).unwrap_or("-".into()),
                    sig.tx_dbm,
                    sig.fspl_db,
                    sig.wall_db,
                    sig.flutter_db,
                    sig.rx_dbm,
                    sig.ray_us
                ));
                let s = &mut tuning.signal;
                slider(ui, &mut s.vtx_mw_whoop, 1.0..=1000.0, "VTX mW, Whoop 65");
                slider(ui, &mut s.vtx_mw_five, 1.0..=2000.0, "VTX mW, Freestyle 5\"");
                slider(ui, &mut s.light_power_mult, 1.0..=64.0, "Light = VTX power ×");
                slider(ui, &mut s.medium_power_mult, 1.0..=32.0, "Medium = VTX power ×");
                slider(ui, &mut s.realistic_power_mult, 0.25..=4.0, "Realistic = VTX power ×");
                slider(ui, &mut s.wall_db_per_m, 0.0..=200.0, "wall loss, dB per metre of concrete");
                slider(ui, &mut s.wall_max_m, 0.05..=2.0, "count at most this many m per wall");
                slider(ui, &mut s.open_surface_m, 0.0..=1.0, "one-sided surface counts as m");
                slider(ui, &mut s.analog_clean_dbm, -100.0..=-50.0, "Analog: clean above (dBm)");
                slider(ui, &mut s.analog_lost_dbm, -110.0..=-60.0, "Analog: full static at (dBm)");
                slider(ui, &mut s.digital_perfect_dbm, -100.0..=-50.0, "Digital: perfect above (dBm)");
                slider(ui, &mut s.digital_lost_dbm, -110.0..=-60.0, "Digital: lost (freeze) at (dBm)");
                slider(ui, &mut s.flutter_db, 0.0..=8.0, "flutter (dB)");
                slider(ui, &mut s.fresnel_m, 0.0..=2.0, "spread walls over (m) — 0 = thin line, abrupt");
                slider(ui, &mut s.smoothing_s, 0.0..=1.5, "signal follows changes over (s)");
                slider(ui, &mut s.light_cap_analog, 0.3..=1.0, "Light: Analog tops out at");
                slider(ui, &mut s.light_cap_digital, 0.3..=0.99, "Light: Digital tops out at (0.5+ = stutter)");
                slider(ui, &mut s.receiver_height_m, 0.5..=3.0, "receiver height above Launch Spot (m)");
            });

            egui::CollapsingHeader::new("Stand-in lighting (not part of the look)").default_open(false).show(ui, |ui| {
                let sc = &mut tuning.scene;
                slider(ui, &mut sc.sun_lux, 10_000.0..=150_000.0, "sun (lux)");
                slider(ui, &mut sc.ambient_nits, 0.0..=8000.0, "sky/ambient (nits) — stands in for baked light");
                ui.checkbox(&mut sc.shadows, "sun shadows");
                ui.checkbox(&mut sc.msaa, "MSAA 4× on the Map render");
            });

            egui::CollapsingHeader::new("Measurements").default_open(true).show(ui, |ui| {
                ui.label(format!(
                    "Metal timestamps: {} · pipelined rendering {} (launch with ./run.sh --pipelined to compare)",
                    match meas.ts_supported {
                        Some(true) => "on",
                        Some(false) => "NOT supported",
                        None => "?",
                    },
                    if args.pipelined { "ON" } else { "OFF" }
                ));
                egui::Grid::new("gpu").striped(true).show(ui, |ui| {
                    ui.label("GPU, last 240 frames");
                    ui.label("median ms");
                    ui.label("p95 ms");
                    ui.end_row();
                    for (name, v) in [
                        ("camera: mip chain", mips),
                        ("camera: merged pass", look),
                        ("camera steps total (cap 2 ms)", cam),
                    ] {
                        ui.label(name);
                        ui.label(ms(v.0));
                        ui.label(ms(v.1));
                        ui.end_row();
                    }
                    ui.label("frame time (wall clock)");
                    ui.label(ms(cpu.0));
                    ui.label(ms(cpu.1));
                    ui.end_row();
                });
                ui.label(format!(
                    "Map drawn at {}×{} px, {} mip levels · Map loaded in {:.0} ms",
                    meas.source_size.x, meas.source_size.y, meas.mip_levels, map.load_ms
                ));
                ui.label("Hide this panel (H) for clean numbers: the panel itself costs GPU time.");
                let lat: Vec<f32> = meas.latency.iter().rev().take(50).map(|l| l.to_present_ms).collect();
                let latg: Vec<f32> = meas.latency.iter().rev().take(50).filter_map(|l| l.to_gpu_done_ms).collect();
                let (lp, lp95, _) = stats(lat.clone());
                let (lg, lg95, _) = stats(latg);
                ui.label(format!(
                    "Stick-to-frame (flick a stick sharply; last {} flicks): to present {} ms (p95 {}), to GPU done {} ms (p95 {})",
                    lat.len(),
                    ms(lp),
                    ms(lp95),
                    ms(lg),
                    ms(lg95)
                ));
            });

            egui::CollapsingHeader::new("Save / load tuning").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut ui_state.save_name);
                    if ui.button("Save tuning").clicked() {
                        match settings::save_tuning(&tuning, &ui_state.save_name) {
                            Ok(p) => flash.0 = Some((format!("Saved {}", p.display()), now)),
                            Err(e) => flash.0 = Some((format!("Save failed: {e}"), now)),
                        }
                        ui_state.saved = settings::list_saved();
                    }
                    if ui.button("Screenshot (F12)").clicked() {
                        take_screenshot(&mut commands, &mut flash, now, &output);
                    }
                });
                let saved = ui_state.saved.clone();
                for f in saved.iter().rev().take(8) {
                    ui.horizontal(|ui| {
                        ui.label(f);
                        if ui.small_button("load").clicked() {
                            match settings::load_tuning(std::path::Path::new(f)) {
                                Ok(t) => {
                                    *tuning = t;
                                    flash.0 = Some((format!("Loaded {f}"), now));
                                }
                                Err(e) => flash.0 = Some((format!("Load failed: {e}"), now)),
                            }
                        }
                    });
                }
            });

            egui::CollapsingHeader::new("Keys").default_open(false).show(ui, |ui| {
                ui.label("H hide panel · V Analog/Digital · B Breakup level · M switch Map · Up/Down arrows Camera Tilt · Left/Right arrows FOV");
                ui.label("P play/pause path · R restart path / back to Launch Spot · F free-fly · G Radio/Gamepad · F12 screenshot");
                ui.label("Cmd+Q quits.");
            });
        });
    Ok(())
}

fn exposure_ui(ui: &mut egui::Ui, e: &mut settings::ExposureTuning, id: &str) {
    ui.push_id(id, |ui| {
        ui.checkbox(&mut e.enabled, "auto-exposure");
        slider(ui, &mut e.speed_brighten, 0.1..=10.0, "brightens at (stops/s)");
        slider(ui, &mut e.speed_darken, 0.1..=10.0, "darkens at (stops/s)");
        slider(ui, &mut e.max_brighten_ev, 0.0..=8.0, "may brighten up to (stops above the sunlit setting)");
        slider(ui, &mut e.max_darken_ev, 0.0..=8.0, "may darken up to (stops below the sunlit setting)");
        slider(ui, &mut e.target_ev, -5.0..=1.0, "aims the average at (stops; lower = darker picture)");
        ui.horizontal(|ui| {
            ui.label("bright light:");
            for t in settings::ToneCurve::ALL {
                ui.selectable_value(&mut e.tone, t, t.label());
            }
        });
    });
}
