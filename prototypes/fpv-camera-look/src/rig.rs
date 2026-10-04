//! PROTOTYPE (#28). Moves the camera without code: free-fly (keyboard + mouse), preset paths,
//! a toy acro quad on the Radio or Gamepad (NOT the real physics), and replay of a recorded flight.
//! The FPV Camera is bolted to the body: it sits at the Quad's camera position, tilted up by
//! Camera Tilt. Digital shows the pose from 15 ms ago (the delay is in the picture only).

use crate::input_sdl::SdlInput;
use crate::map::MapState;
use crate::paths::{SampledPath, PATHS};
use crate::settings::{Look, MapKind, QuadKind, Tuning};
use crate::signal::VideoSignal;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    FreeFly,
    Path,
    Acro,
    Replay,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Recording {
    pub map: MapKind,
    /// (seconds, position, rotation xyzw)
    pub samples: Vec<(f32, [f32; 3], [f32; 4])>,
}

#[derive(Clone, Debug)]
pub struct StickMap {
    pub device: usize,
    /// axis index for roll, pitch, throttle, yaw
    pub axis: [usize; 4],
    pub invert: [bool; 4],
    /// Throttle axis rests at -1 (radio) rather than at 0 (spring-centred gamepad stick).
    pub throttle_full_range: bool,
}

impl Default for StickMap {
    fn default() -> Self {
        Self { device: 0, axis: [0, 1, 2, 3], invert: [false, false, false, false], throttle_full_range: true }
    }
}

#[derive(Resource)]
pub struct Rig {
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub acc_f: Vec3,
    pub omega: Vec3,
    pub heading: f32,
    pub look_pitch: f32,
    pub control: Control,
    pub path_idx: usize,
    pub path: Option<SampledPath>,
    pub path_d: f32,
    pub path_t: f32,
    pub path_playing: bool,
    pub path_speed: f32,
    pub path_loop: bool,
    pub freefly_speed: f32,
    pub replay: Option<Recording>,
    pub replay_t: f32,
    pub recording: Option<Recording>,
    pub rec_t: f32,
    pub sticks: [f32; 4],
    pub stick_map: StickMap,
    pub stick_map_set_for: Option<String>,
    pub prev_stick_ns: u64,
    /// mach ns of a stick step used this frame (latency probe), if any.
    pub stick_step_ns: Option<u64>,
    pub prev_sticks: [f32; 4],
}

impl Default for Rig {
    fn default() -> Self {
        Self {
            pos: Vec3::new(0.0, 1.0, 0.0),
            rot: Quat::IDENTITY,
            vel: Vec3::ZERO,
            acc_f: Vec3::ZERO,
            omega: Vec3::ZERO,
            heading: 0.0,
            look_pitch: 0.0,
            control: Control::Path,
            path_idx: 0,
            path: None,
            path_d: 0.0,
            path_t: 0.0,
            path_playing: true,
            path_speed: 1.0,
            path_loop: true,
            freefly_speed: 4.0,
            replay: None,
            replay_t: 0.0,
            recording: None,
            rec_t: 0.0,
            sticks: [0.0; 4],
            stick_map: StickMap::default(),
            stick_map_set_for: None,
            prev_stick_ns: 0,
            stick_step_ns: None,
            prev_sticks: [0.0; 4],
        }
    }
}

#[derive(Resource, Default)]
pub struct PoseHistory {
    pub v: VecDeque<(f64, Vec3, Quat)>,
}

/// The body pose shown on screen (delayed for Digital). Parent of the 3D camera and the Quad model.
#[derive(Component)]
pub struct CameraRig;

#[derive(Component)]
pub struct FpvCam3d;

pub fn drag(q: QuadKind) -> (f32, f32) {
    match q {
        QuadKind::Whoop65 => (0.5, 0.15),
        QuadKind::Freestyle5 => (0.15, 0.045),
    }
}

/// Where the FPV Camera sits on the body (metres): up, forward.
pub fn camera_mount(q: QuadKind) -> Vec3 {
    match q {
        QuadKind::Whoop65 => Vec3::new(0.0, 0.018, -0.014),
        QuadKind::Freestyle5 => Vec3::new(0.0, 0.026, -0.055),
    }
}

/// How a quad must tilt to produce this acceleration at this velocity, facing `heading`.
pub fn attitude(acc: Vec3, vel: Vec3, heading: f32, q: QuadKind) -> Quat {
    let (c1, c2) = drag(q);
    let f = acc + Vec3::Y * 9.81 + vel * c1 + vel * vel.length() * c2;
    let up = f.try_normalize().unwrap_or(Vec3::Y);
    let fh = Vec3::new(-heading.sin(), 0.0, -heading.cos());
    let mut fwd = fh - up * fh.dot(up);
    if fwd.length_squared() < 1e-6 {
        fwd = Vec3::NEG_Z;
    }
    let fwd = fwd.normalize();
    let right = fwd.cross(up).normalize();
    let fwd = up.cross(right);
    Quat::from_mat3(&Mat3::from_cols(right, up, -fwd)).normalize()
}

pub fn select_path(rig: &mut Rig, idx: usize) {
    rig.path_idx = idx;
    rig.path = Some(SampledPath::new(&PATHS[idx]));
    restart_path(rig);
}

pub fn restart_path(rig: &mut Rig) {
    rig.path_d = 0.0;
    rig.path_t = 0.0;
    rig.acc_f = Vec3::ZERO;
    if let Some(p) = &rig.path {
        let (p0, _) = p.at(0.0);
        let (p1, _) = p.at(0.5);
        rig.pos = p0;
        let d = p1 - p0;
        rig.heading = (-d.x).atan2(-d.z);
        rig.vel = Vec3::ZERO;
        rig.rot = attitude(Vec3::ZERO, Vec3::ZERO, rig.heading, QuadKind::Whoop65);
    }
}

/// Advance the path by dt (fixed sub-steps so screenshots and benchmarks are repeatable).
pub fn advance_path(rig: &mut Rig, dt: f32, quad: QuadKind) {
    let Some(path) = rig.path.take() else { return };
    let len = path.length();
    let mut ended = false;
    let mut left = dt;
    while left > 1e-6 {
        let h = left.min(1.0 / 240.0);
        left -= h;
        let (_, v_here) = path.at(rig.path_d);
        let step = v_here.max(0.2) * rig.path_speed * h;
        let nd = rig.path_d + step;
        let (np, _) = path.at(nd);
        let new_vel = (np - rig.pos) / h;
        let acc = (new_vel - rig.vel) / h;
        let a = (h / 0.15).min(1.0);
        let af = rig.acc_f;
        rig.acc_f = af + (acc - af) * a;
        rig.vel = new_vel;
        rig.pos = np;
        rig.path_d = nd;
        rig.path_t += h;
        let vh = Vec3::new(rig.vel.x, 0.0, rig.vel.z);
        if vh.length() > 0.4 {
            let target = (-vh.x).atan2(-vh.z);
            let mut dh = target - rig.heading;
            dh = (dh + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            rig.heading += dh * (h / 0.12).min(1.0);
        }
        let target_rot = attitude(rig.acc_f, rig.vel, rig.heading, quad);
        rig.rot = rig.rot.slerp(target_rot, (h / 0.06).min(1.0));
        if rig.path_d >= len {
            ended = true;
            break;
        }
    }
    rig.path = Some(path);
    if ended {
        if rig.path_loop {
            restart_path(rig);
        } else {
            rig.path_playing = false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn step_rig(
    mut rig: ResMut<Rig>,
    tuning: Res<Tuning>,
    map: Res<MapState>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_btn: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    input: Res<SdlInput>,
    time: Res<Time<Real>>,
    egui_busy: Res<crate::ui::EguiBusy>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.05);
    let quad = tuning.quad;
    rig.stick_step_ns = None;

    // Sticks (for Acro, and the latency probe).
    {
        let shared = input.0.lock().unwrap();
        let live: Vec<usize> =
            shared.devices.iter().enumerate().filter(|(_, d)| !d.name.ends_with("(unplugged)")).map(|(i, _)| i).collect();
        if let Some(&first) = live.first() {
            if !live.contains(&rig.stick_map.device) {
                rig.stick_map.device = first;
            }
            let dev = &shared.devices[rig.stick_map.device];
            if rig.stick_map_set_for.as_deref() != Some(dev.name.as_str()) {
                // Defaults: gamepad Mode 2 (left stick throttle/yaw); radio AETR.
                rig.stick_map = if dev.is_gamepad {
                    StickMap { device: rig.stick_map.device, axis: [2, 3, 1, 0], invert: [false, true, true, false], throttle_full_range: false }
                } else {
                    StickMap { device: rig.stick_map.device, axis: [0, 1, 2, 3], invert: [false, false, false, false], throttle_full_range: true }
                };
                rig.stick_map_set_for = Some(dev.name.clone());
            }
            let m = rig.stick_map.clone();
            let get = |i: usize| -> f32 {
                let v = dev.axes.get(m.axis[i]).copied().unwrap_or(0.0);
                if m.invert[i] { -v } else { v }
            };
            let thr_raw = get(2);
            let thr = (thr_raw + 1.0) * 0.5;
            let new = [get(0), get(1), thr.clamp(0.0, 1.0), get(3)];
            // A sharp stick step (latency probe): any axis moving > 0.5 since last frame.
            let big = new.iter().zip(rig.prev_sticks.iter()).any(|(a, b)| (a - b).abs() > 0.5);
            if big && dev.last_change_ns != rig.prev_stick_ns {
                rig.stick_step_ns = Some(dev.last_change_ns);
            }
            rig.prev_stick_ns = dev.last_change_ns;
            rig.prev_sticks = new;
            rig.sticks = new;
        }
    }

    let prev_pos = rig.pos;
    match rig.control {
        Control::Path => {
            if rig.path.is_none() {
                let idx = crate::paths::paths_for(map.current.unwrap_or_default()).first().copied().unwrap_or(0);
                select_path(&mut rig, idx);
            }
            if rig.path_playing {
                advance_path(&mut rig, dt, quad);
            }
        }
        Control::FreeFly => {
            let mut wish = Vec3::ZERO;
            if !egui_busy.keyboard {
                if keys.pressed(KeyCode::KeyW) {
                    wish.z -= 1.0;
                }
                if keys.pressed(KeyCode::KeyS) {
                    wish.z += 1.0;
                }
                if keys.pressed(KeyCode::KeyA) {
                    wish.x -= 1.0;
                }
                if keys.pressed(KeyCode::KeyD) {
                    wish.x += 1.0;
                }
                if keys.pressed(KeyCode::KeyE) || keys.pressed(KeyCode::Space) {
                    wish.y += 1.0;
                }
                if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyC) {
                    wish.y -= 1.0;
                }
            }
            let fast = if keys.pressed(KeyCode::ShiftLeft) { 3.0 } else { 1.0 };
            if !egui_busy.pointer && scroll.delta.y != 0.0 {
                rig.freefly_speed = (rig.freefly_speed * (1.0 + 0.1 * scroll.delta.y.signum())).clamp(0.5, 40.0);
            }
            if mouse_btn.pressed(MouseButton::Right) && !egui_busy.pointer {
                rig.heading -= mouse.delta.x * 0.003;
                rig.look_pitch = (rig.look_pitch - mouse.delta.y * 0.003).clamp(-1.2, 1.2);
            }
            let yaw = Quat::from_rotation_y(rig.heading);
            let target = yaw * Vec3::new(wish.x, 0.0, wish.z) * rig.freefly_speed * fast + Vec3::Y * wish.y * rig.freefly_speed * 0.7 * fast;
            let a = 1.0 - (-dt / 0.35).exp();
            let new_vel = rig.vel + (target - rig.vel) * a;
            let acc = if dt > 0.0 { (new_vel - rig.vel) / dt } else { Vec3::ZERO };
            let af = rig.acc_f;
            rig.acc_f = af + (acc - af) * (dt / 0.1).min(1.0);
            rig.vel = new_vel;
            let p = rig.pos + rig.vel * dt;
            rig.pos = p;
            let body = attitude(rig.acc_f, rig.vel, rig.heading, quad);
            let right = body * Vec3::X;
            rig.rot = Quat::from_axis_angle(right, rig.look_pitch) * body;
        }
        Control::Acro => {
            // Toy rate-mode quad. NOT the alpha's physics; only here to move the camera like a quad.
            let (twr, c1, c2) = match quad {
                QuadKind::Whoop65 => (3.5, 0.5, 0.15),
                QuadKind::Freestyle5 => (8.0, 0.15, 0.045),
            };
            let rate = |x: f32| x * (200.0 + 470.0 * x * x) * std::f32::consts::PI / 180.0;
            let sticks = rig.sticks;
            let target = Vec3::new(-rate(sticks[1]), -rate(sticks[3]), -rate(sticks[0]));
            let mut left = dt;
            while left > 1e-6 {
                let h = left.min(0.001);
                left -= h;
                let w = rig.omega;
                rig.omega = w + (target - w) * (h / 0.03).min(1.0);
                let om = rig.omega;
                let dq = Quat::from_scaled_axis(om * h);
                rig.rot = (rig.rot * dq).normalize();
                let thrust = rig.rot * Vec3::Y * (sticks[2] * sticks[2] * twr * 9.81);
                let v = rig.vel;
                let acc = thrust - Vec3::Y * 9.81 - v * c1 - v * v.length() * c2;
                rig.vel += acc * h;
                let vel = rig.vel;
                rig.pos += vel * h;
            }
        }
        Control::Replay => {
            if let Some(rec) = rig.replay.clone() {
                rig.replay_t += dt;
                let t = rig.replay_t;
                let i = rec.samples.partition_point(|s| s.0 < t);
                if i >= rec.samples.len() {
                    rig.replay_t = 0.0;
                } else if i > 0 {
                    let (t0, p0, q0) = rec.samples[i - 1];
                    let (t1, p1, q1) = rec.samples[i];
                    let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
                    rig.pos = Vec3::from(p0).lerp(Vec3::from(p1), k);
                    rig.rot = Quat::from_array(q0).slerp(Quat::from_array(q1), k);
                }
            }
        }
    }

    // Collisions (free-fly and acro): stop at the first surface.
    if matches!(rig.control, Control::FreeFly | Control::Acro) && map.built && prev_pos != rig.pos {
        let d = rig.pos - prev_pos;
        let ext = d.normalize_or_zero() * 0.06;
        if let Some((t, n)) = map.geom.first_hit(prev_pos, rig.pos + ext) {
            let hit = prev_pos + (d + ext) * t;
            rig.pos = hit + n * 0.06;
            let vn = rig.vel.dot(n);
            if vn < 0.0 {
                let v = rig.vel;
                rig.vel = (v - n * vn * 1.3) * 0.5;
            }
        }
    }

    if let Some(mut rec) = rig.recording.take() {
        rig.rec_t += dt;
        rec.samples.push((rig.rec_t, rig.pos.to_array(), rig.rot.to_array()));
        rig.recording = Some(rec);
    }
}

pub fn reset_to_launch(rig: &mut Rig, map: &MapState) {
    rig.pos = map.launch_pos + Vec3::Y * 0.3;
    rig.heading = map.launch_yaw;
    rig.vel = Vec3::ZERO;
    rig.omega = Vec3::ZERO;
    rig.acc_f = Vec3::ZERO;
    rig.look_pitch = 0.0;
    rig.rot = Quat::from_rotation_y(rig.heading);
}

/// Record the true pose, then show it (or, for Digital, the pose from `delay` ago).
pub fn apply_pose(
    rig: Res<Rig>,
    tuning: Res<Tuning>,
    sig: Res<VideoSignal>,
    time: Res<Time<Real>>,
    mut hist: ResMut<PoseHistory>,
    mut rigs: Query<&mut Transform, (With<CameraRig>, Without<FpvCam3d>)>,
    mut cams: Query<(&mut Transform, &mut Camera), With<FpvCam3d>>,
    bypass: Res<crate::Bypass>,
) {
    let now = time.elapsed_secs_f64();
    hist.v.push_back((now, rig.pos, rig.rot));
    while hist.v.len() > 2 && now - hist.v[1].0 > 0.5 {
        hist.v.pop_front();
    }
    let delay_s = match tuning.look {
        Look::Analog => 0.0,
        Look::Digital => (tuning.digital.delay_ms + sig.extra_delay_ms) as f64 / 1000.0,
    };
    let t = now - delay_s;
    let (mut pos, mut rot) = (rig.pos, rig.rot);
    if delay_s > 0.0 {
        let i = hist.v.partition_point(|s| s.0 < t);
        if i == 0 {
            if let Some(s) = hist.v.front() {
                pos = s.1;
                rot = s.2;
            }
        } else if i < hist.v.len() {
            let (t0, p0, q0) = hist.v[i - 1];
            let (t1, p1, q1) = hist.v[i];
            let k = if t1 > t0 { ((t - t0) / (t1 - t0)) as f32 } else { 0.0 };
            pos = p0.lerp(p1, k);
            rot = q0.slerp(q1, k);
        }
    }
    let show = sig.digital_new_frame || bypass.0;
    for mut tf in &mut rigs {
        if show {
            tf.translation = pos;
            tf.rotation = rot;
        }
    }
    for (mut tf, mut cam) in &mut cams {
        tf.translation = camera_mount(tuning.quad);
        tf.rotation = Quat::from_rotation_x(tuning.tilt_deg.to_radians());
        if cam.is_active != show {
            cam.is_active = show;
        }
    }
}
