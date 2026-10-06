//! PROTOTYPE (#34). A rough stand-in for the Simulation, only good enough to drive the sound:
//! four motors with lag, load and battery sag (Drela's first-order model), a P-only rate/angle
//! Flight Controller and mixer with Airmode, a point-mass body, prop wash as thrust noise the FC
//! fights (so the motors warble), Prop Strikes that brake a motor, Bluejay's ESC start-up and
//! Betaflight's beeper. It is NOT the game's physics and nothing here is tuned by ear.

use crate::beeps::{self, BfBeep, Beeper, EscPlayer};
use crate::map::MapGeom;
use crate::paths::{Path, PathKind};
use crate::quads::{QuadDef, QuadKind};
use crate::synth::{SoundFrame, now_ns};
use glam::{Quat, Vec3};
use std::sync::Arc;

pub const DT: f32 = 0.0005; // 2 kHz steps

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// 0..1 stick position (0 = bottom).
    pub throttle: f32,
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub arm: bool,
    /// Angle (self-level) instead of Acro.
    pub angle_mode: bool,
    pub crash_flip: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Throttle {
    Stick,
    /// Scripts: hold this height above the Launch Spot, m.
    Alt(f32),
    /// Scripts: hold whatever height the Quad is at now.
    Hold,
}

#[derive(Clone, Debug)]
pub enum Cmd {
    /// Reset: the battery goes in again (ESC start-up, arming blocked until the ready beep).
    PowerUp,
    /// Put the Quad mid-air with running ESCs ("settled"), for paths and quick tests.
    SettleInAir(f32),
    Strike { motor: usize, rub: f32, secs: f32 },
    /// A prop jammed: rubs until the ESC gives up after its restarts.
    Jam { motor: usize },
    Hit { speed: f32 },
    Link(bool),
    Path(Option<PathKind>),
    Pause(bool),
    /// Skip ahead 10 minutes of sitting idle (Bluejay's beacon).
    IdleTenMinutes,
    /// Pack charge 0..1 (drives LOW BATTERY / LAND NOW on the 5").
    Charge(f32),
}

#[derive(Clone, Copy, Debug)]
pub enum SimEvent {
    Hit { speed: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MotorState {
    Running,
    /// Stalled by a rub; the ESC retries a start every so often.
    Restarting { attempts: u32, next_try: f64 },
    Dead,
}

pub struct Sim {
    pub def: QuadDef,
    pub map: Arc<MapGeom>,
    pub listener: Vec3,
    pub t: f64,
    pub controls: Controls,
    pub throttle_mode: Throttle,
    pub paused: bool,
    // Power, ESCs, FC
    pub power_t: f64,
    pub esc_ready: bool,
    pub armed: bool,
    pub arm_block: &'static str,
    arm_switch_prev: bool,
    arm_needs_flip: bool,
    gyro_beep_at: f64,
    v_filtered: f32,
    start_at: [f64; 4],
    zero_since: [f64; 4],
    angle_prev: bool,
    pub link_ok: bool,
    link_lost_at: f64,
    pub failsafe: bool,
    esc: [EscPlayer; 4],
    motor_state: [MotorState; 4],
    beeper: Beeper,
    idle_since: f64,
    beacon_next: f64,
    pub charge: f32,
    low_bat_t: f32,
    // Motors and battery
    pub omega: [f32; 4],
    pub current: [f32; 4],
    pub duty: [f32; 4],
    pub rub: [f32; 4],
    rub_until: [f64; 4],
    jammed: [bool; 4],
    pub v_batt: f32,
    // Body
    pub pos: Vec3,
    pub vel: Vec3,
    pub att: Quat,
    pub rates: Vec3,
    pub on_ground: bool,
    pub airspeed: f32,
    pub downwash: f32,
    pub walls: f32,
    walls_next: f64,
    // Disturbances
    noise: [f32; 4],
    iterm: Vec3,
    rng: crate::dsp::Rng,
    pub wash: f32,
    // Paths
    path: Option<(PathKind, Path, f32)>,
    path_vel: Vec3,
    path_acc: Vec3,
    pub events: Vec<SimEvent>,
    seq: u64,
}

const G: f32 = 9.81;

/// Motor positions (x forward, y left) as multiples of arm/sqrt(2), and prop spin
/// (+1 = counter-clockwise seen from above). Order: rear-right, front-right, rear-left, front-left
/// (Betaflight's motor order); props-in.
const LAYOUT: [(f32, f32, f32); 4] = [(-1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (1.0, 1.0, -1.0)];

impl Sim {
    pub fn new(kind: QuadKind, map: Arc<MapGeom>) -> Self {
        let def = QuadDef::get(kind);
        let (cy, sy) = (map.launch_yaw.cos(), map.launch_yaw.sin());
        // The pilot stands 1.5 m behind the Launch Spot, ears at 1.6 m.
        let listener = map.launch + Vec3::new(-1.5 * cy, -1.5 * sy, 1.6);
        let mut s = Sim {
            v_batt: def.v_oc,
            def,
            listener,
            t: 0.0,
            controls: Controls { angle_mode: true, ..Default::default() },
            throttle_mode: Throttle::Stick,
            paused: false,
            power_t: 0.0,
            esc_ready: false,
            armed: false,
            arm_block: "",
            arm_switch_prev: false,
            arm_needs_flip: false,
            gyro_beep_at: f64::MAX,
            v_filtered: 0.0,
            start_at: [0.0; 4],
            zero_since: [0.0; 4],
            angle_prev: true,
            link_ok: true,
            link_lost_at: 0.0,
            failsafe: false,
            esc: Default::default(),
            motor_state: [MotorState::Running; 4],
            beeper: Beeper::default(),
            idle_since: 0.0,
            beacon_next: f64::MAX,
            charge: 0.8,
            low_bat_t: 0.0,
            omega: [0.0; 4],
            current: [0.0; 4],
            duty: [0.0; 4],
            rub: [0.0; 4],
            rub_until: [0.0; 4],
            jammed: [false; 4],
            pos: map.launch,
            vel: Vec3::ZERO,
            att: Quat::from_rotation_z(map.launch_yaw),
            rates: Vec3::ZERO,
            on_ground: true,
            airspeed: 0.0,
            downwash: 0.0,
            walls: 0.0,
            walls_next: 0.0,
            noise: [0.0; 4],
            iterm: Vec3::ZERO,
            rng: crate::dsp::Rng::new(1234),
            wash: 0.0,
            path: None,
            path_vel: Vec3::ZERO,
            path_acc: Vec3::ZERO,
            events: Vec::new(),
            seq: 0,
            map,
        };
        s.apply(Cmd::PowerUp);
        s
    }

    pub fn kind(&self) -> QuadKind {
        self.def.kind
    }

    pub fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::PowerUp => {
                self.power_t = self.t;
                self.esc_ready = false;
                self.armed = false;
                self.failsafe = false;
                self.link_ok = true;
                self.motor_state = [MotorState::Running; 4];
                self.jammed = [false; 4];
                self.rub = [0.0; 4];
                self.omega = [0.0; 4];
                self.pos = self.map.launch;
                self.vel = Vec3::ZERO;
                self.rates = Vec3::ZERO;
                self.att = Quat::from_rotation_z(self.map.launch_yaw);
                self.on_ground = true;
                self.path = None;
                self.arm_needs_flip = self.controls.arm;
                self.idle_since = self.t;
                self.beacon_next = f64::MAX;
                let dshot600 = self.def.dshot600;
                for (i, e) in self.esc.iter_mut().enumerate() {
                    // Four ESC chips power up together, a hair apart.
                    e.play(beeps::bluejay_power_up(dshot600), i as f32 * 0.0015);
                }
                self.beeper = Beeper::default();
                self.v_filtered = self.v_open();
                if self.def.has_buzzer {
                    // init.c's 0.5 s chirp, then (the fresh FC counts as calibrated at once, #32)
                    // the three GYRO_CALIBRATED beeps.
                    self.beeper.start(BfBeep::SystemInit);
                    self.gyro_beep_at = self.t + 0.5;
                }
            }
            Cmd::SettleInAir(h) => {
                self.apply(Cmd::PowerUp);
                for e in self.esc.iter_mut() {
                    e.stop();
                }
                self.beeper = Beeper::default();
                self.gyro_beep_at = f64::MAX;
                self.esc_ready = true;
                self.armed = true;
                self.controls.arm = true;
                self.arm_needs_flip = false;
                self.pos = self.map.launch + Vec3::Z * h;
                self.on_ground = false;
                let hover = self.hover_omega();
                self.omega = [hover; 4];
            }
            Cmd::Strike { motor, rub, secs } => {
                self.rub[motor] = rub;
                self.rub_until[motor] = self.t + secs as f64;
            }
            Cmd::Jam { motor } => {
                self.jammed[motor] = true;
                self.rub[motor] = 1.0;
                self.rub_until[motor] = f64::MAX;
            }
            Cmd::Hit { speed } => {
                self.events.push(SimEvent::Hit { speed });
                let kick = Vec3::new(self.rng.white(), self.rng.white(), 0.3).normalize_or_zero() * speed * 0.3;
                self.vel = -self.vel * 0.3 + kick;
                self.rates += Vec3::new(self.rng.white(), self.rng.white(), self.rng.white()) * speed * 2.0;
            }
            Cmd::Link(ok) => {
                if !ok && self.link_ok {
                    self.link_lost_at = self.t;
                }
                if ok {
                    self.beeper.stop(BfBeep::RxLost);
                    if self.failsafe {
                        // Betaflight needs the Arm switch flipped off and on after a Failsafe.
                        self.arm_needs_flip = true;
                    }
                    self.failsafe = false;
                }
                self.link_ok = ok;
            }
            Cmd::Path(p) => {
                self.path = p.map(|k| {
                    let path = Path::build(k, &self.map, self.def.cruise_speed, self.listener);
                    (k, path, 0.0)
                });
                if let Some((_, path, _)) = &self.path {
                    let (p0, v0) = path.at(0.0);
                    let (p1, _) = path.at(0.5);
                    self.pos = p0;
                    self.on_ground = false;
                    self.esc_ready = true;
                    self.armed = true;
                    self.controls.arm = true;
                    for e in self.esc.iter_mut() {
                        e.stop();
                    }
                    self.beeper = Beeper::default();
                    self.gyro_beep_at = f64::MAX;
                    let hover = self.hover_omega();
                    self.omega = [hover; 4];
                    self.path_vel = (p1 - p0).normalize_or_zero() * v0;
                    self.path_acc = Vec3::ZERO;
                }
            }
            Cmd::Pause(p) => self.paused = p,
            Cmd::IdleTenMinutes => {
                self.idle_since = self.t - 600.0;
            }
            Cmd::Charge(c) => self.charge = c.clamp(0.0, 1.0),
        }
    }

    fn hover_omega(&self) -> f32 {
        (self.def.mass * G / 4.0 / self.def.k_f).sqrt()
    }

    pub fn path_kind(&self) -> Option<PathKind> {
        self.path.as_ref().map(|p| p.0)
    }

    pub fn path_progress(&self) -> Option<f32> {
        self.path.as_ref().map(|(_, p, s)| s / p.length)
    }

    fn noise_step(&mut self) {
        // Low-passed noise (~15 Hz) per motor, unit-ish size.
        let a = 1.0 - (-2.0 * std::f32::consts::PI * 15.0 * DT).exp();
        for i in 0..4 {
            let w = self.rng.white() * 3.0;
            self.noise[i] += a * (w - self.noise[i]);
        }
    }

    /// Pack open-circuit voltage from the charge slider (LiHV on the whoop, LiPo on the 5").
    fn v_open(&self) -> f32 {
        let full = if self.def.cells == 1 { 4.35 } else { 4.2 };
        let per_cell = 3.3 + (full - 3.3) * self.charge.powf(0.7);
        per_cell * self.def.cells as f32
    }

    pub fn step(&mut self) {
        if self.paused {
            return;
        }
        let t = self.t;
        self.noise_step();
        let def = self.def.clone();

        // --- ESCs ready ≈1.66 s after power-up; Betaflight's gyro beeps after its chirp ---
        let ready_at = beeps::bluejay_ready_s() as f64;
        if !self.esc_ready && t - self.power_t >= ready_at {
            self.esc_ready = true;
            self.idle_since = t;
        }
        if t >= self.gyro_beep_at {
            self.gyro_beep_at = f64::MAX;
            if def.has_buzzer {
                self.beeper.start(BfBeep::GyroCalibrated);
            }
        }

        // --- Arming (Betaflight rules, with #32's BOOTGRACE until the ESCs are ready) ---
        let c = self.controls;
        if !c.arm {
            self.arm_needs_flip = false;
        }
        self.arm_switch_prev = c.arm;
        self.arm_block = if !self.esc_ready {
            "BOOTGRACE"
        } else if self.failsafe || !self.link_ok {
            "RXLOSS"
        } else if self.arm_needs_flip {
            "ARMSWITCH"
        } else if c.throttle > 0.05 && self.throttle_mode == Throttle::Stick && !self.armed {
            "THROTTLE"
        } else {
            ""
        };
        if self.path.is_none() {
            if !self.armed && c.arm && self.arm_block.is_empty() {
                self.armed = true;
                if def.has_buzzer {
                    self.beeper.start(BfBeep::Arming);
                }
            }
            if self.armed && !c.arm {
                self.armed = false;
                if def.has_buzzer {
                    self.beeper.start(BfBeep::Disarming);
                }
            }
        }
        if c.angle_mode != self.angle_prev {
            self.angle_prev = c.angle_mode;
            // enableFlightMode/disableFlightMode -> beeperConfirmationBeeps(1)
            if def.has_buzzer {
                self.beeper.start(BfBeep::ModeChange);
            }
        }
        if def.has_buzzer && c.crash_flip && self.armed {
            self.beeper.start(BfBeep::CrashFlip);
        }

        // --- Failsafe: 1.5 s, then DROP (disarm); RX_LOST asked for every 10 ms until the link returns ---
        if !self.link_ok && !self.failsafe && t - self.link_lost_at >= 1.5 {
            self.failsafe = true;
            self.armed = false;
        }
        let tick10 = (t * 100.0) as u64 != ((t + DT as f64) * 100.0) as u64;
        if def.has_buzzer && self.failsafe && tick10 {
            self.beeper.start(BfBeep::RxLost);
        }

        // --- Battery: Betaflight's 5 Hz battery task asks for BAT_LOW / BAT_CRIT_LOW ---
        let v_oc = self.v_open();
        let i_total: f32 = self.current.iter().sum();
        self.v_batt = (v_oc - def.r_batt * i_total).clamp(0.0, v_oc + 0.3);
        // battery.c vbatDisplayLpfPeriod = 30 -> pt1 at 1/(30/10) Hz (battery.h): tau ≈ 0.477 s.
        let a = 1.0 - (-DT / 0.477f32).exp();
        self.v_filtered += a * (self.v_batt - self.v_filtered);
        let tick200 = (t * 5.0) as u64 != ((t + DT as f64) * 5.0) as u64;
        if def.has_buzzer && tick200 {
            let per_cell = self.v_filtered / def.cells as f32;
            if per_cell <= 3.30 {
                self.beeper.start(BfBeep::BatCritLow);
            } else if per_cell <= 3.50 {
                self.beeper.start(BfBeep::BatLow);
            }
        }

        // --- Idle beacon (Bluejay: zero throttle with a signal for ≈10 min) ---
        let idle = !self.armed && self.esc_ready && self.link_ok;
        if !idle {
            self.idle_since = t;
            self.beacon_next = f64::MAX;
        } else if t - self.idle_since >= beeps::BLUEJAY_BEACON_DELAY_S as f64 && self.beacon_next == f64::MAX {
            self.beacon_next = t;
        }
        if self.beacon_next <= t {
            for e in self.esc.iter_mut() {
                e.play(beeps::bluejay_beacon(), 0.0);
            }
            self.beacon_next = t + beeps::BLUEJAY_BEACON_INTERVAL_S as f64;
        }

        // --- Rubs end ---
        for i in 0..4 {
            if !self.jammed[i] && t >= self.rub_until[i] {
                self.rub[i] = 0.0;
            }
        }

        // --- Motor commands ---
        let mut duty = [0.0f32; 4];
        let v = self.v_batt.max(0.1);
        let path_active = self.path.is_some();
        if path_active {
            duty = self.path_duty(v);
        } else if self.armed && self.esc_ready {
            duty = self.fc_duty(v);
        }
        if !self.esc_ready {
            duty = [0.0; 4];
        }
        for i in 0..4 {
            // Bluejay: the first non-zero throttle after zero waits 100 ms before starting.
            if duty[i] > 0.0 && self.duty[i] == 0.0 && !path_active {
                self.start_at[i] = t + beeps::BLUEJAY_START_WAIT_S as f64;
            }
            if duty[i] == 0.0 {
                self.zero_since[i] = self.zero_since[i].min(t);
            } else {
                self.zero_since[i] = f64::MAX;
            }
        }
        self.duty = duty;

        // --- Motors (J dw/dt = Kt i - k_m w^2 - rub), stalls and restarts as Bluejay does ---
        let kt = def.kt();
        let rub_torque = 1.6 * def.k_m * def.omega_max().powi(2);
        let mut thrust = [0.0f32; 4];
        for i in 0..4 {
            let w = self.omega[i];
            let driven = duty[i] > 0.0 && self.motor_state[i] == MotorState::Running && t >= self.start_at[i];
            let cur = if driven { (duty[i] * v - kt * w) / def.r_motor } else { 0.0 };
            let tau_e = if driven { kt * (cur - def.i0) } else { -kt * def.i0 * (w / 50.0).min(1.0) };
            let tau_rub = self.rub[i] * rub_torque * (0.3 + 0.7 * (w / def.omega_max()).min(1.0)) + self.rub[i] * kt * def.i0 * 3.0;
            let dw = (tau_e - def.k_m * w * w - tau_rub) / def.rotor_j * DT;
            let mut w2 = (w + dw).max(0.0);
            if w2 < 1.0 && dw < 0.0 {
                w2 = 0.0;
            }
            self.omega[i] = w2;
            self.current[i] = cur.max(-2.0 * def.i_max());
            thrust[i] = def.k_f * w2 * w2;
            match self.motor_state[i] {
                MotorState::Running => {
                    // A stall while running restarts silently after 100 ms (it doesn't count).
                    if driven && self.rub[i] > 0.0 && w2 < 0.04 * def.omega_max() {
                        self.motor_state[i] = MotorState::Restarting { attempts: 0, next_try: t + beeps::BLUEJAY_RESTART_GAP_S as f64 };
                    }
                }
                MotorState::Restarting { attempts, next_try } => {
                    if duty[i] == 0.0 {
                        self.motor_state[i] = MotorState::Running;
                    } else if t >= next_try {
                        if self.rub[i] < 0.5 {
                            self.motor_state[i] = MotorState::Running;
                        } else if attempts + 1 >= beeps::BLUEJAY_RESTART_ATTEMPTS {
                            // Three failed starts: three falling tones, then f1_short.
                            self.motor_state[i] = MotorState::Dead;
                            self.esc[i].play(beeps::bluejay_stall(), 0.0);
                        } else {
                            // A start attempt (≈50 ms) the jammed prop eats, then 100 ms wait.
                            self.omega[i] += 0.02 * def.omega_max();
                            self.motor_state[i] = MotorState::Restarting {
                                attempts: attempts + 1,
                                next_try: t + 0.05 + beeps::BLUEJAY_RESTART_GAP_S as f64,
                            };
                        }
                    }
                }
                MotorState::Dead => {
                    // Back in arming_wait: ≈0.32 s of zero throttle (the FC disarmed), then f2_short.
                    if t - self.zero_since[i] >= beeps::BLUEJAY_ZERO_THROTTLE_S as f64 && !self.esc[i].playing() {
                        self.esc[i].play(beeps::bluejay_ready_again(), 0.0);
                        self.motor_state[i] = MotorState::Running;
                        self.jammed[i] = false;
                        self.rub[i] = 0.0;
                    }
                }
            }
        }

        // --- Body ---
        if path_active {
            self.path_step(&thrust);
        } else {
            self.body_step(&thrust);
        }
        let t_total: f32 = thrust.iter().sum();
        self.downwash = (t_total.max(0.0) / (2.0 * 1.225 * 4.0 * def.disk_area())).sqrt();

        // --- Line to the pilot: walls (every 20 ms) ---
        if t >= self.walls_next {
            self.walls = self.map.walls_averaged(self.pos, self.listener, 1.0);
            self.walls_next = t + 0.02;
        }

        for e in self.esc.iter_mut() {
            e.step(DT);
        }
        self.beeper.step(DT);
        self.t += DT as f64;
    }

    /// Flight Controller: rates or angle from the sticks, P-only, mixer with Airmode, thrust to duty.
    fn fc_duty(&mut self, v: f32) -> [f32; 4] {
        let def = &self.def;
        let c = self.controls;
        let rate = |x: f32| (x * (def.center_rate_dps + (def.max_rate_dps - def.center_rate_dps) * x * x)).to_radians();
        let mut target = Vec3::new(rate(c.roll), rate(c.pitch), -rate(c.yaw));
        if c.angle_mode && !c.crash_flip {
            let max = 55f32.to_radians();
            let (yaw, _, _) = self.att.to_euler(glam::EulerRot::ZYX);
            let want = Quat::from_rotation_z(yaw) * Quat::from_rotation_y(c.pitch * max) * Quat::from_rotation_x(c.roll * max);
            let err = self.att.inverse() * want;
            let (axis, ang) = err.to_axis_angle();
            let ang = if ang > std::f32::consts::PI { ang - std::f32::consts::TAU } else { ang };
            let e = axis * ang;
            target.x = 7.0 * e.x;
            target.y = 7.0 * e.y;
        }
        let kp = 30.0;
        let err = target - self.rates;
        // A small I-term holds the off-centre CG with steady motor differences.
        if self.on_ground {
            self.iterm = Vec3::ZERO;
        } else {
            self.iterm = (self.iterm + err * DT * 300.0).clamp(Vec3::splat(-400.0), Vec3::splat(400.0));
        }
        let mut alpha = err * kp + self.iterm;
        // Prop wash and air: thrust noise the FC fights (so the motors warble).
        let i = Vec3::from(def.inertia);
        alpha.z *= 0.5;
        let torque = alpha * i;
        let a = def.arm / std::f32::consts::SQRT_2;
        let km_kf = def.k_m / def.k_f;
        let t_max = def.thrust_for_duty(1.0, v);
        let t_idle = def.thrust_for_duty(def.idle, v);
        // Collective from the stick (duty-like, as Betaflight) or from the script's height hold.
        let collective = match self.throttle_mode {
            Throttle::Stick => {
                let u = def.idle + (1.0 - def.idle) * c.throttle;
                def.thrust_for_duty(u, v) * 4.0
            }
            Throttle::Hold => def.mass * G,
            Throttle::Alt(h) => {
                let z_target = self.map.launch.z + h;
                let up = self.att * Vec3::Z;
                let az = 4.0 * (z_target - self.pos.z) - 3.0 * self.vel.z;
                (def.mass * (G + az.clamp(-6.0, 15.0)) / up.z.max(0.4)).max(0.0)
            }
        };
        if c.crash_flip {
            // Crash Flip: only the motors on the stick's side spin (reverse), others stop.
            let mut d = [0.0f32; 4];
            for k in 0..4 {
                let (_, y, _) = LAYOUT[k];
                let side = if c.roll.abs() > 0.2 { -(y * c.roll.signum()) } else { 0.0 };
                if side > 0.0 {
                    d[k] = 0.4 * c.roll.abs();
                }
            }
            return d;
        }
        let mut tt = [0.0f32; 4];
        for k in 0..4 {
            let (x, y, spin) = LAYOUT[k];
            let roll = torque.x / (4.0 * a) * y;
            let pitch = -torque.y / (4.0 * a) * x;
            let yaw = -torque.z / (4.0 * km_kf) * spin;
            tt[k] = collective / 4.0 + roll + pitch + yaw;
        }
        // Airmode: keep the differences, shift the whole mix into range.
        let hi = tt.iter().cloned().fold(f32::MIN, f32::max);
        let lo = tt.iter().cloned().fold(f32::MAX, f32::min);
        if hi - lo > t_max - t_idle {
            let scale = (t_max - t_idle) / (hi - lo);
            let mid = (hi + lo) * 0.5;
            for x in tt.iter_mut() {
                *x = mid + (*x - mid) * scale;
            }
        }
        let hi = tt.iter().cloned().fold(f32::MIN, f32::max);
        let lo = tt.iter().cloned().fold(f32::MAX, f32::min);
        let shift = if hi > t_max { t_max - hi } else if lo < t_idle { t_idle - lo } else { 0.0 };
        let mut d = [0.0f32; 4];
        for k in 0..4 {
            d[k] = def.duty_for_thrust(tt[k] + shift, v).clamp(def.idle, 1.0) * (1.0 + def.motor_spread[k]);
        }
        d
    }

    fn disturb(&mut self, thrust: &mut [f32; 4]) {
        // Prop wash: falling into your own air (Prop Wash comes from physics in the game; here
        // it's thrust noise sized by how fast the Quad sinks along its thrust axis).
        let up = self.att * Vec3::Z;
        let vh = (self.def.mass * G / (2.0 * 1.225 * 4.0 * self.def.disk_area())).sqrt();
        let sink = -self.vel.dot(up);
        self.wash = ((sink - 0.3 * vh) / vh).clamp(0.0, 1.0);
        let base = self.def.turbulence;
        for k in 0..4 {
            thrust[k] *= 1.0 + (base + 0.15 * self.wash) * self.noise[k];
        }
    }

    fn body_step(&mut self, thrust_in: &[f32; 4]) {
        let def = self.def.clone();
        let mut thrust = *thrust_in;
        self.disturb(&mut thrust);
        let a = def.arm / std::f32::consts::SQRT_2;
        let km_kf = def.k_m / def.k_f;
        let mut torque = Vec3::ZERO;
        let mut total = 0.0;
        for k in 0..4 {
            let (x, y, spin) = LAYOUT[k];
            torque.x += y * a * thrust[k];
            torque.y += -x * a * thrust[k];
            torque.z += -spin * km_kf * thrust[k];
            total += thrust[k];
        }
        // Weight acting at the off-centre CG (body frame, while flying).
        if !self.on_ground {
            let w = def.mass * G;
            torque.x += -w * def.cg_offset[1];
            torque.y += w * def.cg_offset[0];
        }
        let i = Vec3::from(def.inertia);
        self.rates += torque / i * DT;
        self.rates *= 1.0 - 0.2 * DT; // a touch of air damping
        self.att = (self.att * Quat::from_scaled_axis(self.rates * DT)).normalize();
        let up = self.att * Vec3::Z;
        let drag = -self.vel * (def.drag_lin * def.mass) - self.vel * self.vel.length() * def.drag_quad;
        let acc = up * (total / def.mass) + drag / def.mass - Vec3::Z * G;
        self.vel += acc * DT;
        self.pos += self.vel * DT;
        let ground = self.map.launch.z;
        if self.pos.z <= ground {
            if !self.on_ground && self.vel.z < -2.5 {
                self.events.push(SimEvent::Hit { speed: -self.vel.z });
            }
            self.pos.z = ground;
            if self.vel.z < 0.0 {
                self.vel.z = 0.0;
            }
            if total < def.mass * G * 0.9 {
                // Sitting on the ground: it rests level and still (no tumbling in this rough model).
                self.vel *= 0.9;
                let up_now = self.att * Vec3::Z;
                if up_now.z > 0.0 {
                    let (yaw, _, _) = self.att.to_euler(glam::EulerRot::ZYX);
                    self.att = Quat::from_rotation_z(yaw);
                    self.rates = Vec3::ZERO;
                } else {
                    self.rates *= 0.8;
                }
                self.on_ground = true;
            }
        } else {
            self.on_ground = false;
        }
        self.airspeed = self.vel.length();
    }

    /// Path flights: the Quad follows the line; each motor gets the duty that makes the thrust
    /// the line needs (plus the same disturbances), so motor sound still follows the flight.
    fn path_duty(&mut self, v: f32) -> [f32; 4] {
        let def = &self.def;
        let vel = self.path_vel;
        let drag = -vel * (def.drag_lin * def.mass) - vel * vel.length() * def.drag_quad;
        let f = def.mass * (self.path_acc + Vec3::Z * G) - drag;
        let total = f.length();
        let mut d = [0.0; 4];
        for k in 0..4 {
            let n = 1.0 + 0.012 * self.noise[k];
            d[k] = def.duty_for_thrust(total / 4.0 * n, v).clamp(def.idle, 1.0) * (1.0 + def.motor_spread[k]);
        }
        d
    }

    fn path_step(&mut self, thrust: &[f32; 4]) {
        let _ = thrust;
        let Some((_, path, s)) = &mut self.path else { return };
        let (_, spd) = path.at(*s);
        *s += spd * DT;
        let (p, _) = path.at(*s);
        let vel = (p - self.pos) / DT;
        // Smooth velocity and acceleration (the path is sampled every 0.1 m).
        let a = 1.0 - (-DT / 0.05f32).exp();
        let new_vel = self.path_vel + (vel - self.path_vel) * a;
        let acc = (new_vel - self.path_vel) / DT;
        self.path_acc = self.path_acc + (acc.clamp_length_max(40.0) - self.path_acc) * a;
        self.path_vel = new_vel;
        self.pos = p;
        self.vel = self.path_vel;
        self.airspeed = self.vel.length();
        let drag = self.vel * (self.def.drag_lin * self.def.mass) + self.vel * self.vel.length() * self.def.drag_quad;
        let f = self.def.mass * (self.path_acc + Vec3::Z * G) + drag;
        let up = f.normalize_or_zero();
        let yaw = self.vel.y.atan2(self.vel.x);
        let fwd = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = fwd.cross(up).normalize_or_zero();
        let fwd2 = up.cross(right);
        self.att = Quat::from_mat3(&glam::Mat3::from_cols(fwd2, -right, up));
        if *s >= path.length {
            self.path = None;
            self.throttle_mode = Throttle::Alt((self.pos.z - self.map.launch.z).max(0.5));
        }
    }

    pub fn frame(&mut self) -> SoundFrame {
        self.seq += 1;
        let mut esc_hz = [0.0; 4];
        let mut esc_amp = [0.0; 4];
        for i in 0..4 {
            let (hz, amp) = self.esc[i].output();
            esc_hz[i] = hz;
            esc_amp[i] = amp;
        }
        SoundFrame {
            seq: self.seq,
            wall_ns: now_ns(),
            omega: self.omega,
            current: self.current,
            rub: self.rub,
            esc_hz,
            esc_amp,
            buzzer: self.def.has_buzzer && self.beeper.on(),
            omega_max: self.def.omega_max(),
            i_max: self.def.i_max(),
            blades: self.def.blades,
            poles: self.def.poles,
            airspeed: self.airspeed,
            downwash: self.downwash,
            quad_pos: self.pos.to_array(),
            listener_pos: self.listener.to_array(),
            walls: self.walls,
            paused: self.paused,
        }
    }

    pub fn beeper_label(&self) -> &'static str {
        self.beeper.label()
    }

    pub fn motor_state_label(&self, i: usize) -> &'static str {
        match self.motor_state[i] {
            MotorState::Running => {
                if self.rub[i] > 0.0 {
                    "rubbing"
                } else {
                    "ok"
                }
            }
            MotorState::Restarting { .. } => "restarting",
            MotorState::Dead => "dead",
        }
    }
}
