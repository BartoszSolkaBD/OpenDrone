//! PROTOTYPE (#28). The Video Signal and Breakup, worked out between frames. Visual only: it never
//! touches the flight.
//!
//! Received power (dBm) = 10 log10(VTX mW x level multiplier)
//!                      - free-space loss at 5.8 GHz (20 log10(d) + 47.7 dB)
//!                      - walls (dB per metre of concrete crossed, capped per wall)
//!                      + flutter (slow random wobble).
//! Analog: clean above `analog_clean_dbm`, full static at `analog_lost_dbm`.
//! Digital: perfect above `digital_perfect_dbm`, smear -> stutter -> freeze at `digital_lost_dbm`,
//! and after a freeze the picture comes back `relock_s` after the signal does.

use crate::map::MapState;
use crate::settings::{BreakupLevel, Look, Tuning};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DigitalState {
    #[default]
    Ok,
    Smear,
    Stutter,
    Frozen,
    Relocking,
}

impl DigitalState {
    pub fn label(self) -> &'static str {
        match self {
            DigitalState::Ok => "clean",
            DigitalState::Smear => "soft / smeared",
            DigitalState::Stutter => "stutter",
            DigitalState::Frozen => "FROZEN (signal lost)",
            DigitalState::Relocking => "re-locking",
        }
    }
}

#[derive(Resource, Default)]
pub struct VideoSignal {
    pub receiver: Vec3,
    pub distance_m: f32,
    pub walls: u32,
    pub open_surfaces: u32,
    pub concrete_m: f32,
    pub wall_db: f32,
    pub fspl_db: f32,
    pub tx_dbm: f32,
    pub flutter_db: f32,
    pub rx_dbm: f32,
    pub ray_us: f32,
    /// 0..1 before the Light cap and Reduce-motion limiting.
    pub analog_raw: f32,
    pub digital_raw: f32,
    /// What the picture shows (after caps and limiting).
    pub analog_shown: f32,
    pub digital_shown: f32,
    pub digital_state: DigitalState,
    pub relock_left_s: f32,
    /// Whether the Digital camera delivers a new frame this frame (stutter / freeze).
    pub digital_new_frame: bool,
    pub hold_until: f64,
    pub extra_delay_ms: f32,
    flutter_state: f32,
    rng: u64,
}

impl VideoSignal {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
}

pub fn fspl_db(d_m: f32) -> f32 {
    20.0 * d_m.max(1.0).log10() + 47.7
}

/// Work out the signal from the Quad's true position (never the delayed one).
pub fn update_signal(
    mut sig: ResMut<VideoSignal>,
    map: Res<MapState>,
    tuning: Res<Tuning>,
    rig: Res<crate::rig::Rig>,
    time: Res<Time<Real>>,
) {
    if sig.rng == 0 {
        sig.rng = 0x9E37_79B9_7F4A_7C15;
    }
    let s = &tuning.signal;
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let rx = map.launch_pos + Vec3::Y * s.receiver_height_m;
    sig.receiver = rx;
    let quad = rig.pos;
    sig.distance_m = quad.distance(rx);

    // Walls between Quad and pilot.
    let t0 = std::time::Instant::now();
    let crossings = if map.built { map.geom.crossings(quad, rx) } else { Vec::new() };
    sig.ray_us = t0.elapsed().as_secs_f32() * 1e6;
    let len = sig.distance_m;
    let mut walls = 0u32;
    let mut open = 0u32;
    let mut concrete = 0.0f32;
    let mut per_obj: std::collections::HashMap<usize, Vec<(f32, bool)>> = Default::default();
    for c in &crossings {
        per_obj.entry(c.obj).or_default().push((c.t, c.entering));
    }
    for (obj, hits) in per_obj {
        if !map.geom.objects[obj].closed {
            open += hits.len() as u32;
            concrete += hits.len() as f32 * s.open_surface_m;
            continue;
        }
        let mut start: Option<f32> = None;
        let mut first = true;
        for (t, entering) in hits.iter().copied() {
            if entering {
                start = Some(t);
            } else {
                // Exiting: if we never saw the entry, the Quad (or the pilot) is inside the solid.
                let a = start.take().unwrap_or(if first { 0.0 } else { t });
                concrete += ((t - a) * len).min(s.wall_max_m);
                walls += 1;
            }
            first = false;
        }
        if let Some(a) = start {
            concrete += ((1.0 - a) * len).min(s.wall_max_m);
            walls += 1;
        }
    }
    sig.walls = walls;
    sig.open_surfaces = open;
    sig.concrete_m = concrete;
    sig.wall_db = concrete * s.wall_db_per_m;
    sig.fspl_db = fspl_db(len);

    // Flutter: a slow random wobble (a few times a second).
    let target = (sig.rand() - 0.5) * 2.0;
    let a = (dt * 6.0).min(1.0);
    sig.flutter_state += (target - sig.flutter_state) * a;
    sig.flutter_db = sig.flutter_state * s.flutter_db;

    let mult = tuning.power_mult();
    sig.tx_dbm = 10.0 * (tuning.vtx_mw() * mult.unwrap_or(1.0)).max(1e-6).log10();
    sig.rx_dbm = sig.tx_dbm - sig.fspl_db - sig.wall_db + sig.flutter_db;

    let lin = |v: f32, good: f32, bad: f32| ((good - v) / (good - bad)).clamp(0.0, 1.0);
    let (mut a_raw, mut d_raw) = (
        lin(sig.rx_dbm, s.analog_clean_dbm, s.analog_lost_dbm),
        lin(sig.rx_dbm, s.digital_perfect_dbm, s.digital_lost_dbm),
    );
    if tuning.breakup == BreakupLevel::Off {
        a_raw = 0.0;
        d_raw = 0.0;
    }
    sig.analog_raw = a_raw;
    sig.digital_raw = d_raw;
    let (a_cap, d_cap) = if tuning.breakup == BreakupLevel::Light {
        (s.light_cap_analog, s.light_cap_digital.min(0.99))
    } else {
        (1.0, 1.0)
    };
    let a_target = a_raw.min(a_cap);
    let d_target = d_raw.min(d_cap);

    // Reduce motion: static fades in and out, at most 3 flashes a second (rise + fall >= 1/3 s).
    if tuning.reduce_motion {
        let max_step = 6.0 * dt;
        sig.analog_shown += (a_target - sig.analog_shown).clamp(-max_step, max_step);
    } else {
        sig.analog_shown = a_target;
    }
    sig.digital_shown = d_target;

    // Digital link: smear -> stutter -> freeze, re-lock after the signal returns.
    let now = time.elapsed_secs_f64();
    let relock = tuning.digital.relock_s.max(0.0);
    let lost = d_target >= 1.0;
    sig.digital_new_frame = true;
    match sig.digital_state {
        DigitalState::Frozen | DigitalState::Relocking => {
            if lost {
                sig.digital_state = DigitalState::Frozen;
                sig.relock_left_s = relock;
            } else if d_target < 0.97 {
                if sig.digital_state == DigitalState::Frozen {
                    sig.digital_state = DigitalState::Relocking;
                    sig.relock_left_s = relock;
                }
                sig.relock_left_s -= dt;
                if sig.relock_left_s <= 0.0 {
                    sig.digital_state = DigitalState::Ok;
                }
            }
            if sig.digital_state != DigitalState::Ok {
                sig.digital_new_frame = false;
            }
        }
        _ => {
            if lost {
                sig.digital_state = DigitalState::Frozen;
                sig.relock_left_s = relock;
                sig.digital_new_frame = false;
            } else if d_target >= 0.5 {
                sig.digital_state = DigitalState::Stutter;
                // Hold the last frame for a random while; longer holds as the signal fades.
                if now < sig.hold_until {
                    sig.digital_new_frame = false;
                } else {
                    let k = ((d_target - 0.5) * 2.0).clamp(0.0, 1.0);
                    if sig.rand() < 0.08 + 0.5 * k {
                        let hold = 0.04 + sig.rand() as f64 * (0.06 + 0.35 * k as f64);
                        sig.hold_until = now + hold;
                    }
                }
            } else if d_target > 0.02 {
                sig.digital_state = DigitalState::Smear;
            } else {
                sig.digital_state = DigitalState::Ok;
            }
        }
    }
    if tuning.look != Look::Digital {
        sig.digital_new_frame = true;
    }
    // The delay rises near the edge of range.
    let edge = ((d_target - 0.3) / 0.7).clamp(0.0, 1.0);
    sig.extra_delay_ms = edge * edge * tuning.digital.edge_extra_delay_ms;
}
