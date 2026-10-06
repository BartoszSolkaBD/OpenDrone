//! PROTOTYPE (#34). The two custom Firewheel nodes that make the Quad's sound live:
//!
//! - `QuadVoiceNode`: four motor voices (blade-pass tone and overtones, the growl between them,
//!   broadband whoosh, motor whine), frame hum, Bluejay's ESC tones, Betaflight's buzzer and
//!   Prop Strike ticks. Outputs: 0 = Quad group (motors, props, beeps), 1 = Crashes group (ticks).
//! - `ListenerNode`: the Listening Position. On the Quad adds wind on the mic, the mic's low cut,
//!   the camera-style compressor and optional clipping. Where you stand adds travel time (and so
//!   Doppler), distance fade, air absorption and wall muffling. Inputs/outputs: 0 = Quad, 1 = Crashes
//!   (the hit clips join input 1, so they fade and arrive late too).
//!
//! Both read the latest per-tick state (`SoundFrame`) the simulation publishes through a wait-free
//! triple buffer, and ramp to it across each audio block. Tuning values arrive as Firewheel
//! parameter patches (Diff/Patch).

use crate::dsp::*;
use crate::quads::{ListenerParams, SoundBlock};
use firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, NodeError, ProcBuffers, ProcExtra,
        ProcInfo, ProcessStatus,
    },
};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// What the simulation tells the sound each tick. Sound never writes back.
#[derive(Clone, Copy, Debug, Default)]
pub struct SoundFrame {
    pub seq: u64,
    /// ns since the process started (see `now_ns`), when the tick was published.
    pub wall_ns: u64,
    pub omega: [f32; 4],
    pub current: [f32; 4],
    /// How hard each prop rubs (0..1).
    pub rub: [f32; 4],
    /// Each ESC's tone pitch (0 = silent) and loudness (0..1).
    pub esc_hz: [f32; 4],
    pub esc_amp: [f32; 4],
    pub buzzer: bool,
    pub omega_max: f32,
    pub i_max: f32,
    pub blades: u32,
    pub poles: u32,
    pub airspeed: f32,
    /// The props' induced (downwash) speed, m/s.
    pub downwash: f32,
    pub quad_pos: [f32; 3],
    pub listener_pos: [f32; 3],
    /// Map parts on the line from the Quad to the pilot.
    pub walls: f32,
    pub paused: bool,
}

pub fn now_ns() -> u64 {
    static EPOCH: OnceLock<std::time::Instant> = OnceLock::new();
    EPOCH.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

pub type FrameOut = Arc<Mutex<Option<triple_buffer::Output<SoundFrame>>>>;

fn take_reader(slot: &Option<FrameOut>) -> triple_buffer::Output<SoundFrame> {
    slot.as_ref()
        .and_then(|s| s.lock().unwrap().take())
        .unwrap_or_else(|| triple_buffer::triple_buffer(&SoundFrame::default()).1)
}

/// Numbers the audio thread reports back to the UI (f32 bits in atomics).
#[derive(Default)]
pub struct AudioStats {
    pub block_frames: AtomicU32,
    pub sample_rate: AtomicU32,
    pub playback_delay_us: AtomicU32,
    pub frame_age_us: AtomicU32,
    pub gain_reduction_db: AtomicU32,
    pub peak_out: AtomicU32,
    pub clipped_blocks: AtomicU64,
    pub blocks: AtomicU64,
    pub distance_m: AtomicU32,
    pub delay_ms: AtomicU32,
    pub doppler: AtomicU32,
    pub walls: AtomicU32,
    pub wind: AtomicU32,
    pub voice_ns: AtomicU64,
    pub listener_ns: AtomicU64,
    pub processed_frames: AtomicU64,
}

pub fn store_f(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}
pub fn load_f(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

#[derive(Default)]
pub struct NodeShared {
    pub frames: Option<FrameOut>,
    pub stats: Option<Arc<AudioStats>>,
}

// ------------------------------------------------------------------------------------------------
// QuadVoiceNode
// ------------------------------------------------------------------------------------------------

#[derive(Diff, Patch, Clone, Copy, PartialEq, Debug)]
pub struct QuadVoiceNode {
    pub block: SoundBlock,
    /// The Quad's own level (quad_level_db) applies only Where you stand; On the Quad the camera
    /// mic sits next to the motors on either Quad.
    pub standing: bool,
}

const MAX_PARTIALS: usize = 48;

#[derive(Clone, Copy)]
struct Motor {
    theta: f32,
    blade_phase: f32,
    rng: Rng,
    jitter_lp: OnePole,
    bb: Svf,
    tick: Svf,
    tick_env: f32,
    esc_phase: f32,
    esc_env: f32,
    esc_ring1: Svf,
    esc_ring2: Svf,
    /// Fixed random phase per partial (blades aren't alike).
    phase: [(f32, f32); MAX_PARTIALS],
    /// Partial amplitudes at the start of the block and their per-sample step.
    amp: [f32; MAX_PARTIALS],
    damp: [f32; MAX_PARTIALS],
}

impl Motor {
    fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed.wrapping_mul(2654435761).wrapping_add(17));
        let mut phase = [(1.0, 0.0); MAX_PARTIALS];
        for p in phase.iter_mut() {
            let a = rng.uniform() * TAU;
            *p = (a.cos(), a.sin());
        }
        Motor {
            theta: rng.uniform() * TAU,
            blade_phase: 0.0,
            rng,
            jitter_lp: OnePole::default(),
            bb: Svf::default(),
            tick: Svf::default(),
            tick_env: 0.0,
            esc_phase: 0.0,
            esc_env: 0.0,
            esc_ring1: Svf::default(),
            esc_ring2: Svf::default(),
            phase,
            amp: [0.0; MAX_PARTIALS],
            damp: [0.0; MAX_PARTIALS],
        }
    }
}

struct VoiceProc {
    p: QuadVoiceNode,
    reader: triple_buffer::Output<SoundFrame>,
    stats: Option<Arc<AudioStats>>,
    last: SoundFrame,
    motors: [Motor; 4],
    body: Peak,
    hum1: Svf,
    hum2: Svf,
    buzz_phase: f32,
    buzz_env: f32,
    pause_gain: f32,
    sr: f32,
}

impl AudioNode for QuadVoiceNode {
    type Configuration = NodeShared;

    fn info(&self, _c: &Self::Configuration) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("quad_voice")
            .channel_config(ChannelConfig { num_inputs: ChannelCount::ZERO, num_outputs: ChannelCount::STEREO }))
    }

    fn construct_processor(
        &self,
        c: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        let sr = cx.stream_info.sample_rate.get() as f32;
        Ok(VoiceProc {
            p: *self,
            reader: take_reader(&c.frames),
            stats: c.stats.clone(),
            last: SoundFrame::default(),
            motors: [Motor::new(1), Motor::new(2), Motor::new(3), Motor::new(4)],
            body: Peak::default(),
            hum1: Svf::default(),
            hum2: Svf::default(),
            buzz_phase: 0.0,
            buzz_env: 0.0,
            pause_gain: 1.0,
            sr,
        })
    }
}

/// Partial amplitudes for one motor at speed `w` and load `load` (0..1).
fn partials(b: &SoundBlock, f: &SoundFrame, w: f32, load: f32, sr: f32, out: &mut [f32; MAX_PARTIALS]) -> usize {
    let blades = f.blades.max(1) as usize;
    let harmonics = b.bpf_harmonics.round().clamp(1.0, 16.0) as usize;
    let m_count = (blades * harmonics).min(MAX_PARTIALS);
    let wn = (w / f.omega_max.max(1.0)).clamp(0.0, 1.3);
    let loud = wn.powf(b.level_exponent) * (1.0 - b.load_loudness + b.load_loudness * (0.3 + 0.7 * load));
    let shaft_hz = w / TAU;
    let nyq = sr * 0.5;
    for (m0, a) in out.iter_mut().enumerate() {
        let m = m0 + 1;
        if m > m_count {
            *a = 0.0;
            continue;
        }
        let fm = shaft_hz * m as f32;
        let fade = ((0.92 * nyq - fm) / (0.12 * nyq)).clamp(0.0, 1.0);
        let pos = m as f32 / blades as f32; // in blade-pass harmonics
        let roll = pos.max(1.0).powf(-b.harmonic_rolloff);
        let mut amp = if m % blades == 0 {
            let h = (m / blades) as f32;
            b.bpf_level * roll * (1.0 + b.load_brightness * load * (h - 1.0) / harmonics as f32 * 2.0)
        } else {
            b.blade_mismatch * b.bpf_level * roll * 0.6
        };
        if m == 1 {
            amp += b.shaft_level;
        }
        *a = amp * loud * fade * 0.12;
    }
    m_count
}

impl AudioNodeProcessor for VoiceProc {
    fn events(&mut self, _info: &ProcInfo, events: &mut ProcEvents, _extra: &mut ProcExtra) {
        for patch in events.drain_patches::<QuadVoiceNode>() {
            self.p.apply(patch);
        }
    }

    fn process(&mut self, info: &ProcInfo, buffers: ProcBuffers, _extra: &mut ProcExtra) -> ProcessStatus {
        let t0 = std::time::Instant::now();
        let n = info.frames;
        let sr = self.sr;
        let f = *self.reader.read();
        let prev = self.last;
        self.last = f;
        let b = self.p.block;
        let inv_n = 1.0 / n as f32;

        if let Some(st) = &self.stats {
            st.block_frames.store(n as u32, Ordering::Relaxed);
            st.sample_rate.store(info.sample_rate.get(), Ordering::Relaxed);
            if let Some(d) = info.process_to_playback_delay {
                st.playback_delay_us.store(d.as_micros() as u32, Ordering::Relaxed);
            }
            if f.wall_ns > 0 {
                st.frame_age_us.store((now_ns().saturating_sub(f.wall_ns) / 1000) as u32, Ordering::Relaxed);
            }
            st.blocks.fetch_add(1, Ordering::Relaxed);
        }

        // Block-rate setup.
        let blades = f.blades.max(1) as f32;
        let pole_pairs = (f.poles / 2).max(1) as f32;
        self.body.set(b.body_peak_hz, b.body_peak_q, b.body_peak_db, sr);
        self.hum1.set(b.hum_mode1_hz, b.hum_mode1_q, sr);
        self.hum2.set(b.hum_mode2_hz, b.hum_mode2_q, sr);
        let mut m_counts = [0usize; 4];
        for i in 0..4 {
            let mo = &mut self.motors[i];
            let load0 = (prev.current[i] / prev.i_max.max(0.1)).clamp(0.0, 1.0);
            let load1 = (f.current[i] / f.i_max.max(0.1)).clamp(0.0, 1.0);
            let mut a0 = [0.0; MAX_PARTIALS];
            let mut a1 = [0.0; MAX_PARTIALS];
            let c0 = partials(&b, &prev, prev.omega[i], load0, sr, &mut a0);
            let c1 = partials(&b, &f, f.omega[i], load1, sr, &mut a1);
            m_counts[i] = c0.max(c1);
            for k in 0..MAX_PARTIALS {
                mo.amp[k] = a0[k];
                mo.damp[k] = (a1[k] - a0[k]) * inv_n;
            }
            let bpf = f.omega[i] / TAU * blades;
            mo.bb.set((bpf * b.broadband_center).max(80.0), b.broadband_q, sr);
            mo.tick.set(b.tick_hz, 2.5, sr);
            mo.esc_ring1.set(b.esc_ring_hz, 4.0, sr);
            mo.esc_ring2.set(b.esc_ring_hz * 2.3, 5.0, sr);
            mo.jitter_lp.set(25.0, sr);
        }
        let tick_decay = (-1.0 / (b.tick_decay_ms.max(0.1) * 1e-3 * sr)).exp();
        let env_a = 1.0 - (-1.0 / (0.002 * sr)).exp();
        let pause_target = if f.paused { 0.0 } else { 1.0 };
        let pause_a = 1.0 - (-1.0 / (0.025 * sr)).exp();
        let quad_gain = if self.p.standing { db_to_lin(b.quad_level_db) } else { 1.0 };
        let nyq = sr * 0.5;

        let (out_q, rest) = buffers.outputs.split_first_mut().unwrap();
        let out_c = &mut rest[0];
        for s in 0..n {
            let x = (s as f32 + 1.0) * inv_n;
            let mut motors_sum = 0.0f32;
            let mut ticks = 0.0f32;
            let mut shaft_exc = 0.0f32;
            let mut esc_sum = 0.0f32;
            for i in 0..4 {
                let w = prev.omega[i] + (f.omega[i] - prev.omega[i]) * x;
                let wn = (w / f.omega_max.max(1.0)).clamp(0.0, 1.3);
                let load = ((prev.current[i] + (f.current[i] - prev.current[i]) * x) / f.i_max.max(0.1)).clamp(0.0, 1.0);
                let rub = prev.rub[i] + (f.rub[i] - prev.rub[i]) * x;
                let mo = &mut self.motors[i];
                let jit = mo.jitter_lp.lp(mo.rng.white()) * 4.0 * b.jitter;
                let dtheta = w * (1.0 + jit) / sr;
                mo.theta += dtheta;
                if mo.theta > TAU {
                    mo.theta -= TAU;
                }
                // Blade passages for Prop Strike ticks.
                mo.blade_phase += dtheta * blades;
                if mo.blade_phase > TAU {
                    mo.blade_phase -= TAU;
                    if rub > 0.01 {
                        mo.tick_env = mo.tick_env.max(rub.sqrt());
                    }
                }
                let (zs, zc) = mo.theta.sin_cos();
                let (mut re, mut im) = (zc, zs);
                let mut tone = 0.0f32;
                let mut cos_bpf = 0.0f32;
                let bl = f.blades.max(1) as usize;
                for k in 0..m_counts[i] {
                    let a = mo.amp[k];
                    mo.amp[k] += mo.damp[k];
                    let (pc, ps) = mo.phase[k];
                    tone += a * (im * pc + re * ps);
                    if k + 1 == bl {
                        cos_bpf = re;
                    }
                    let nre = re * zc - im * zs;
                    im = re * zs + im * zc;
                    re = nre;
                }
                // Broadband whoosh, pulsing at the blade rate.
                let bb_amp = b.broadband_level * wn.powf(b.level_exponent + 0.5) * (0.6 + 0.4 * load) * 0.08;
                let nz = mo.rng.white();
                let bb = mo.bb.bp_norm(nz) * bb_amp * (1.0 + b.broadband_swish * cos_bpf);
                // Motor whine at the electrical frequency.
                let fe = w / TAU * pole_pairs;
                let whine = if fe < 0.9 * nyq { (mo.theta * pole_pairs).sin() * b.motor_whine * load * wn * 0.1 } else { 0.0 };
                motors_sum += tone + bb + whine;
                shaft_exc += zs * wn * wn;
                // Prop Strike ticks.
                if mo.tick_env > 1e-4 {
                    ticks += mo.tick.bp_norm(mo.rng.white() * mo.tick_env);
                    mo.tick_env *= tick_decay;
                } else {
                    mo.tick_env = 0.0;
                }
                // ESC tone (the windings driven as a speaker).
                let ea = prev.esc_amp[i] + (f.esc_amp[i] - prev.esc_amp[i]) * x;
                let target = if f.esc_hz[i] > 0.0 { ea } else { 0.0 };
                mo.esc_env += env_a * (target - mo.esc_env);
                if mo.esc_env > 1e-4 {
                    let hz = f.esc_hz[i].max(1.0);
                    mo.esc_phase += TAU * hz / sr;
                    if mo.esc_phase > TAU {
                        mo.esc_phase -= TAU;
                    }
                    // Bluejay drives two short current pulses per period, ≈199 µs apart
                    // (A→B then C→B); each one rings the motor's bell. The buzz knob blends
                    // that pulse train with a plain sine at the same pitch.
                    let ph = mo.esc_phase;
                    let gap = TAU * hz * crate::beeps::BLUEJAY_PULSE_GAP_S;
                    let step = TAU * hz / sr;
                    let mut imp = 0.0;
                    if ph < step {
                        imp += 1.0;
                    }
                    if ph >= gap && ph - step < gap {
                        imp += 1.0;
                    }
                    let ring = mo.esc_ring1.bp_norm(imp) + 0.5 * mo.esc_ring2.bp_norm(imp);
                    let br = b.esc_beep_brightness;
                    let e = (1.0 - br) * ph.sin() + br * ring * 6.0;
                    esc_sum += e * mo.esc_env;
                }
            }
            let motors = self.body.process(motors_sum);
            let hum_res = self.hum1.bp_norm(shaft_exc * 0.5 + motors * 0.3) + 0.7 * self.hum2.bp_norm(shaft_exc * 0.5 + motors * 0.3);
            let hum = b.hum_level * (b.hum_follow * shaft_exc * 0.15 + (1.0 - b.hum_follow) * hum_res * 0.3);
            // Betaflight's buzzer (an active buzzer: one fixed pitch, switched on and off).
            let bt = if f.buzzer { 1.0 } else { 0.0 };
            self.buzz_env += env_a * (bt - self.buzz_env);
            let mut buzz = 0.0;
            if self.buzz_env > 1e-4 {
                self.buzz_phase += TAU * b.buzzer_hz / sr;
                if self.buzz_phase > TAU {
                    self.buzz_phase -= TAU;
                }
                let ph = self.buzz_phase;
                buzz = (ph.sin() + 0.25 * (3.0 * ph).sin()) * self.buzz_env * b.buzzer_level * 0.5;
            }
            self.pause_gain += pause_a * (pause_target - self.pause_gain);
            let g = self.pause_gain;
            out_q[s] = (motors + hum) * quad_gain * g + (esc_sum * b.esc_beep_level * 0.25 + buzz) * g;
            out_c[s] = ticks * b.tick_level * 0.5 * g;
        }
        if let Some(st) = &self.stats {
            st.voice_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        ProcessStatus::OutputsModified
    }
}

// ------------------------------------------------------------------------------------------------
// ListenerNode
// ------------------------------------------------------------------------------------------------

#[derive(Diff, Patch, Clone, Copy, PartialEq, Debug)]
pub struct ListenerNode {
    pub p: ListenerParams,
}

struct ListenerProc {
    p: ListenerParams,
    reader: triple_buffer::Output<SoundFrame>,
    stats: Option<Arc<AudioStats>>,
    last: SoundFrame,
    sr: f32,
    // On the Quad
    hp: [[OnePole; 2]; 3],
    rng: Rng,
    wind_lp1: OnePole,
    wind_lp2: OnePole,
    gust_lp: OnePole,
    gust_lp2: OnePole,
    env: f32,
    gr_db: f32,
    lim_env: f32,
    // Where you stand
    delay: [DelayLine; 2],
    air: [[OnePole; 2]; 2],
    wall: [[OnePole; 2]; 2],
    walls_s: f32,
    last_delay: f32,
    // Switching
    mode_stand: bool,
    mode_gain: f32,
    pause_gain: f32,
}

impl AudioNode for ListenerNode {
    type Configuration = NodeShared;

    fn info(&self, _c: &Self::Configuration) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("listener")
            .channel_config(ChannelConfig { num_inputs: ChannelCount::STEREO, num_outputs: ChannelCount::STEREO }))
    }

    fn construct_processor(
        &self,
        c: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        let sr = cx.stream_info.sample_rate.get() as f32;
        let len = (sr * 2.5) as usize;
        Ok(ListenerProc {
            p: self.p,
            reader: take_reader(&c.frames),
            stats: c.stats.clone(),
            last: SoundFrame::default(),
            sr,
            hp: [[OnePole::default(); 2]; 3],
            rng: Rng::new(99),
            wind_lp1: OnePole::default(),
            wind_lp2: OnePole::default(),
            gust_lp: OnePole::default(),
            gust_lp2: OnePole::default(),
            env: 0.0,
            gr_db: 0.0,
            lim_env: 0.0,
            delay: [DelayLine::new(len), DelayLine::new(len)],
            air: [[OnePole::default(); 2]; 2],
            wall: [[OnePole::default(); 2]; 2],
            walls_s: 0.0,
            last_delay: 0.0,
            mode_stand: self.p.where_you_stand,
            mode_gain: 1.0,
            pause_gain: 1.0,
        })
    }
}

#[inline]
fn two_hp(f: &mut [OnePole; 2], x: f32) -> f32 {
    let y = f[0].hp(x);
    f[1].hp(y)
}

#[inline]
fn two_lp(f: &mut [OnePole; 2], x: f32) -> f32 {
    let y = f[0].lp(x);
    f[1].lp(y)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl AudioNodeProcessor for ListenerProc {
    fn events(&mut self, _info: &ProcInfo, events: &mut ProcEvents, _extra: &mut ProcExtra) {
        for patch in events.drain_patches::<ListenerNode>() {
            let mut node = ListenerNode { p: self.p };
            node.apply(patch);
            self.p = node.p;
        }
    }

    fn process(&mut self, info: &ProcInfo, buffers: ProcBuffers, _extra: &mut ProcExtra) -> ProcessStatus {
        let t0 = std::time::Instant::now();
        let n = info.frames;
        let sr = self.sr;
        let f = *self.reader.read();
        let prev = self.last;
        self.last = f;
        let p = self.p;
        let inv_n = 1.0 / n as f32;

        for ch in self.hp.iter_mut() {
            for h in ch.iter_mut() {
                h.set(p.mic_lowcut_hz, sr);
            }
        }
        // Wind: louder and brighter with airspeed and the downwash over the mic.
        let v0 = (prev.airspeed.powi(2) + (prev.downwash * p.wind_downwash).powi(2)).sqrt();
        let v1 = (f.airspeed.powi(2) + (f.downwash * p.wind_downwash).powi(2)).sqrt();
        let wl = |v: f32| p.wind_level * (v / p.wind_ref_speed.max(1.0)).powf(p.wind_exponent).min(4.0);
        let (wind0, wind1) = (wl(v0), wl(v1));
        let cutoff = p.wind_cutoff_hz * (1.0 + v1 / p.wind_ref_speed.max(1.0));
        self.wind_lp1.set(cutoff, sr);
        self.wind_lp2.set(cutoff, sr);
        self.gust_lp.set(1.5, sr);
        self.gust_lp2.set(0.4, sr);

        let att = 1.0 - (-1.0 / (p.comp_attack_ms.max(0.1) * 1e-3 * sr)).exp();
        let rel = 1.0 - (-1.0 / (p.comp_release_ms.max(1.0) * 1e-3 * sr)).exp();
        let makeup = db_to_lin(p.comp_makeup_db);
        let lim_att = 1.0 - (-1.0 / (0.0005 * sr)).exp();
        let lim_rel = 1.0 - (-1.0 / (0.15 * sr)).exp();
        let onquad_gain = db_to_lin(p.onquad_level_db);
        let drive = db_to_lin(p.clip_drive_db);

        // Where you stand: distance at both ends of the block.
        let r0 = dist(prev.quad_pos, prev.listener_pos).max(0.3);
        let r1 = dist(f.quad_pos, f.listener_pos).max(0.3);
        let c_sound = 343.0;
        let air_fc = (p.air_cutoff_100m_hz * 100.0 / r1.max(1.0)).clamp(500.0, 20000.0);
        for ch in self.air.iter_mut() {
            for a in ch.iter_mut() {
                a.set(air_fc, sr);
            }
        }
        let walls_a = 1.0 - (-(n as f32) / (0.08 * sr)).exp();
        let walls_target = if p.wall_muffle_on { f.walls.min(3.0) } else { 0.0 };
        self.walls_s += walls_a * (walls_target - self.walls_s);
        let w = self.walls_s;
        let wall_fc = 20000.0 * (p.wall_muffle_hz / 20000.0).powf(w.min(1.0)) * (1.0 / (1.0 + 0.5 * (w - 1.0).max(0.0)));
        for ch in self.wall.iter_mut() {
            for a in ch.iter_mut() {
                a.set(wall_fc.max(80.0), sr);
            }
        }
        // The first wall costs the full loss; each further one (up to two more) 40% of it, since
        // sound also bends round a building rather than only going through it.
        let wall_gain = db_to_lin(-p.wall_loss_db * (w.min(1.0) + 0.4 * (w - 1.0).clamp(0.0, 2.0)));
        let stand_gain = db_to_lin(p.stand_level_db) * wall_gain;
        let ref_m = p.stand_ref_m.max(0.1);

        let mode_a = 1.0 - (-1.0 / (0.015 * sr)).exp();
        let pause_target = if f.paused { 0.0 } else { 1.0 };
        let pause_a = 1.0 - (-1.0 / (0.025 * sr)).exp();

        let in_q = buffers.inputs[0];
        let in_c = buffers.inputs[1];
        let (out_q, rest) = buffers.outputs.split_first_mut().unwrap();
        let out_c = &mut rest[0];
        let mut peak = 0.0f32;
        let mut delay_samples = 0.0;
        for s in 0..n {
            let x = (s as f32 + 1.0) * inv_n;
            let q = in_q[s];
            let c = in_c[s];
            // Always feed the delay lines so a switch to Where you stand starts clean.
            self.delay[0].push(q);
            self.delay[1].push(c);
            // Fade out, switch, fade in when the Listening Position changes.
            let want = p.where_you_stand;
            let target = if want == self.mode_stand { 1.0 } else { 0.0 };
            self.mode_gain += mode_a * (target - self.mode_gain);
            if self.mode_gain < 0.01 && want != self.mode_stand {
                self.mode_stand = want;
            }
            let (yq, yc) = if !self.mode_stand {
                // On the Quad: camera mic.
                let qh = two_hp(&mut self.hp[0], q);
                let ch = two_hp(&mut self.hp[1], c);
                let gust = 1.0 + p.wind_gust * (self.gust_lp.lp(self.rng.white()) * 6.0 + self.gust_lp2.lp(self.rng.white()) * 10.0);
                let wn = self.wind_lp2.lp(self.wind_lp1.lp(self.rng.white()));
                let wind = wn * (wind0 + (wind1 - wind0) * x) * gust.max(0.0) * 1.5;
                let wh = two_hp(&mut self.hp[2], wind);
                let mut g = 1.0;
                if p.comp_on {
                    let det = (qh + ch + wh).abs();
                    let a = if det > self.env { att } else { rel };
                    self.env += a * (det - self.env);
                    let lvl = lin_to_db(self.env);
                    let over = lvl - p.comp_threshold_db;
                    let knee = 6.0;
                    let gr = if over <= -knee * 0.5 {
                        0.0
                    } else if over >= knee * 0.5 {
                        over * (1.0 / p.comp_ratio.max(1.0) - 1.0)
                    } else {
                        let o = over + knee * 0.5;
                        (1.0 / p.comp_ratio.max(1.0) - 1.0) * o * o / (2.0 * knee)
                    };
                    self.gr_db = gr;
                    g = db_to_lin(gr) * makeup;
                } else {
                    self.gr_db = 0.0;
                }
                let mut yq = (qh + wh) * g * onquad_gain;
                let mut yc = ch * g * onquad_gain;
                if p.clip_on {
                    yq = clip(yq * drive, p.clip_hardness) / drive;
                    yc = clip(yc * drive, p.clip_hardness) / drive;
                }
                (yq, yc)
            } else {
                // Where you stand: travel time (Doppler follows), distance, air, walls.
                let r = r0 + (r1 - r0) * x;
                let d = if p.delay_on { r / c_sound * sr } else { 1.0 };
                delay_samples = d;
                let mut yq = self.delay[0].read(d);
                let mut yc = self.delay[1].read(d);
                let g = stand_gain * (ref_m / r.max(ref_m));
                yq = two_lp(&mut self.air[0], yq);
                yc = two_lp(&mut self.air[1], yc);
                yq = two_lp(&mut self.wall[0], yq);
                yc = two_lp(&mut self.wall[1], yc);
                let (yq, yc) = (yq * g, yc * g);
                // A safety limiter (not a character compressor): only a punch-out right next to
                // you reaches it, so the loudest moment doesn't hard-clip.
                let det = (yq + yc).abs();
                let a = if det > self.lim_env { lim_att } else { lim_rel };
                self.lim_env += a * (det - self.lim_env);
                let lg = if self.lim_env > 0.7 { 0.7 / self.lim_env } else { 1.0 };
                self.gr_db = lin_to_db(lg);
                (yq * lg, yc * lg)
            };
            self.pause_gain += pause_a * (pause_target - self.pause_gain);
            let gm = self.mode_gain * self.pause_gain;
            out_q[s] = yq * gm;
            out_c[s] = yc * gm;
            peak = peak.max((yq + yc).abs());
        }
        if let Some(st) = &self.stats {
            store_f(&st.gain_reduction_db, self.gr_db);
            store_f(&st.peak_out, peak);
            store_f(&st.distance_m, r1);
            let dms = if self.mode_stand { delay_samples / sr * 1000.0 } else { 0.0 };
            // Doppler factor from how fast the delay is changing.
            let dd = (dms - self.last_delay) / (n as f32 / sr * 1000.0);
            self.last_delay = dms;
            store_f(&st.delay_ms, dms);
            store_f(&st.doppler, 1.0 - dd);
            store_f(&st.walls, w);
            store_f(&st.wind, wind1);
            if peak > 1.0 {
                st.clipped_blocks.fetch_add(1, Ordering::Relaxed);
            }
            st.listener_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
            st.processed_frames.fetch_add(n as u64, Ordering::Relaxed);
        }
        ProcessStatus::OutputsModified
    }
}
