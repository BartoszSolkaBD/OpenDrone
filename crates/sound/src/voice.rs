//! The four motor voices and the frame hum, as one Firewheel node with one
//! mono output.
//!
//! Each motor's voice is a stack of tones at whole multiples of its rotation
//! rate: the blade-pass tone (blade count × rotation rate) and its overtones,
//! weaker tones between them from blades that aren't quite alike, and the
//! once-a-turn tone. Over them sit the props' whoosh, a band of noise
//! pulsing with the blades, and the motor's whine at its electrical rate
//! (pole pairs × rotation rate). The blade tips' hiss and the frame's
//! resonance are shared by all four. Each audio block ramps from the last
//! Quad state to the newest, sample by sample.

use std::f32::consts::TAU;
use std::sync::Mutex;

use firewheel::StreamInfo;
use firewheel::channel_config::{ChannelConfig, ChannelCount};
use firewheel::node::{
    AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, NodeError,
    ProcBuffers, ProcExtra, ProcInfo, ProcStreamCtx, ProcessStatus,
};

use crate::dsp::{BandPass, Noise, OnePole, Peak, TwoPole, unit_spread};
use crate::{QuadFrame, QuadSound, SoundBlock};

/// The most tones one motor makes: blade count × overtones.
const MOST_TONES: usize = 48;

/// The newest Quad state, read on the sound device's thread with no waiting.
pub(crate) type Frames = triple_buffer::Output<QuadFrame>;

/// Where a node finds its [`Frames`] once Firewheel builds its processor.
#[derive(Default)]
pub(crate) struct FramesSlot(pub(crate) Mutex<Option<Frames>>);

impl FramesSlot {
    pub(crate) fn new(frames: Frames) -> FramesSlot {
        FramesSlot(Mutex::new(Some(frames)))
    }

    /// The node's frames; a reader that never changes if they were taken.
    pub(crate) fn take(&self) -> Frames {
        self.0
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .unwrap_or_else(|| triple_buffer::triple_buffer(&QuadFrame::default()).1)
    }
}

/// The four motor voices and the frame hum.
pub(crate) struct VoiceNode {
    pub(crate) quad: QuadSound,
}

impl AudioNode for VoiceNode {
    type Configuration = FramesSlot;

    fn info(&self, _: &FramesSlot) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("quad_voice")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::MONO,
            }))
    }

    fn construct_processor(
        &self,
        frames: &FramesSlot,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        Ok(Voice::new(
            self.quad,
            frames.take(),
            cx.stream_info.sample_rate.get() as f32,
        ))
    }
}

/// One motor's voice.
#[derive(Clone, Copy)]
struct Motor {
    /// Where the rotor is in its turn, in radians.
    turn: f32,
    noise: Noise,
    /// The speed wobble: noise smoothed to about 30 Hz.
    wobble: OnePole,
    /// The slow flutter of its tones, in three bands.
    flutter: [OnePole; 3],
    whoosh: BandPass,
    /// Each tone's fixed starting phase, as (cos, sin): no two blades are
    /// alike.
    phase: [(f32, f32); MOST_TONES],
    /// Each tone's fixed strength difference, from −1 to 1.
    mismatch: [f32; MOST_TONES],
    /// Each tone's amplitude now, and its step each sample through the block.
    amplitude: [f32; MOST_TONES],
    step: [f32; MOST_TONES],
}

/// A motor's speed as a share of its top speed, and its load: its current
/// as a share of its top current.
fn speed_and_load(quad: &QuadSound, frame: &QuadFrame, motor: usize) -> (f32, f32) {
    let m = frame.motors[motor];
    let speed = (m.speed.abs() / quad.top_speed.max(1.0)).clamp(0.0, 1.3);
    let load = (m.current.abs() / quad.top_current.max(0.1)).clamp(0.0, 1.0);
    (speed, load)
}

/// How one motor's voice moves through an audio block: from the last Quad
/// state to the newest.
#[derive(Clone, Copy, Default)]
struct Ramp {
    /// How many tones it makes.
    tones: usize,
    /// Its speed as a share of top speed, its load, and its speed in rad/s,
    /// at the block's start and end.
    share: (f32, f32),
    load: (f32, f32),
    speed: (f32, f32),
}

impl Motor {
    fn new(seed: u32) -> Motor {
        let mut noise = Noise::new(seed.wrapping_mul(2_654_435_761).wrapping_add(17));
        let mut phase = [(1.0, 0.0); MOST_TONES];
        for p in &mut phase {
            let angle = noise.unit() * TAU;
            *p = (angle.cos(), angle.sin());
        }
        let mut mismatch = [0.0; MOST_TONES];
        for m in &mut mismatch {
            *m = noise.white();
        }
        Motor {
            turn: noise.unit() * TAU,
            noise,
            wobble: OnePole::default(),
            flutter: [OnePole::default(); 3],
            whoosh: BandPass::default(),
            phase,
            mismatch,
            amplitude: [0.0; MOST_TONES],
            step: [0.0; MOST_TONES],
        }
    }
}

struct Voice {
    quad: QuadSound,
    frames: Frames,
    last: QuadFrame,
    sample_rate: f32,
    motors: [Motor; 4],
    body: Peak,
    hum_first: BandPass,
    hum_second: BandPass,
    hiss_noise: Noise,
    hiss_low_cut: TwoPole,
    hiss_high_cut: OnePole,
}

impl Voice {
    fn new(quad: QuadSound, frames: Frames, sample_rate: f32) -> Voice {
        Voice {
            quad,
            frames,
            last: QuadFrame::default(),
            sample_rate,
            motors: [Motor::new(1), Motor::new(2), Motor::new(3), Motor::new(4)],
            body: Peak::default(),
            hum_first: BandPass::default(),
            hum_second: BandPass::default(),
            hiss_noise: Noise::new(777),
            hiss_low_cut: TwoPole::default(),
            hiss_high_cut: OnePole::default(),
        }
    }
}

/// Each tone's amplitude for one motor at `speed` (rad/s, as a share of top
/// speed in `share`) and `load`, into `out`; how many tones there are.
fn tones(
    block: &SoundBlock,
    blades: usize,
    speed: f32,
    share: f32,
    load: f32,
    sample_rate: f32,
    out: &mut [f32; MOST_TONES],
) -> usize {
    let harmonics = block.harmonics.clamp(1, 16) as usize;
    let count = (blades * harmonics).min(MOST_TONES);
    let loud = share.powf(block.loudness_exponent)
        * (1.0 - block.load_loudness + block.load_loudness * (0.3 + 0.7 * load));
    let turn_hz = speed.abs() / TAU;
    let nyquist = sample_rate * 0.5;
    for (i, amplitude) in out.iter_mut().enumerate() {
        let m = i + 1;
        if m > count {
            *amplitude = 0.0;
            continue;
        }
        // Tones near the top of what the device can play fade out.
        let fade = ((0.92 * nyquist - turn_hz * m as f32) / (0.12 * nyquist)).clamp(0.0, 1.0);
        let rolloff = (m as f32 / blades as f32)
            .max(1.0)
            .powf(-block.harmonic_rolloff);
        let mut a = if m.is_multiple_of(blades) {
            let overtone = (m / blades) as f32;
            block.blade_pass_level
                * rolloff
                * (1.0 + block.load_brightness * load * (overtone - 1.0) / harmonics as f32 * 2.0)
        } else {
            block.blade_mismatch * block.blade_pass_level * rolloff * 0.6
        };
        if m == 1 {
            a += block.shaft_level;
        }
        *amplitude = a * loud * fade * 0.12;
    }
    count
}

impl AudioNodeProcessor for Voice {
    fn process(
        &mut self,
        info: &ProcInfo,
        buffers: ProcBuffers,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        let n = info.frames;
        let sample_rate = self.sample_rate;
        let now = *self.frames.read();
        let before = self.last;
        self.last = now;
        let block = self.quad.block;
        let blades = self.quad.blades.max(1) as usize;
        let pole_pairs = (self.quad.poles / 2).max(1) as f32;
        let per_sample = 1.0 / n as f32;
        let nyquist = sample_rate * 0.5;

        self.body.set(
            block.body_peak,
            block.body_peak_q,
            block.body_peak_gain,
            sample_rate,
        );
        self.hum_first
            .set(block.hum_first_mode, block.hum_first_mode_q, sample_rate);
        self.hum_second
            .set(block.hum_second_mode, block.hum_second_mode_q, sample_rate);
        self.hiss_low_cut.set(block.hiss_corner, sample_rate);
        self.hiss_high_cut.set(14_000.0, sample_rate);
        let flutter_hz = [12.0, 25.0, 45.0];
        let flutter_spread = flutter_hz.map(|hz| unit_spread(hz, sample_rate));
        let wobble_spread = unit_spread(30.0, sample_rate);

        let mut ramps = [Ramp::default(); 4];
        for (i, (motor, ramp)) in self.motors.iter_mut().zip(&mut ramps).enumerate() {
            let quad = &self.quad;
            let (share0, load0) = speed_and_load(quad, &before, i);
            let (share1, load1) = speed_and_load(quad, &now, i);
            let (speed0, speed1) = (before.motors[i].speed, now.motors[i].speed);
            let mut start = [0.0; MOST_TONES];
            let mut end = [0.0; MOST_TONES];
            let c0 = tones(
                &block,
                blades,
                speed0,
                share0,
                load0,
                sample_rate,
                &mut start,
            );
            let c1 = tones(&block, blades, speed1, share1, load1, sample_rate, &mut end);
            *ramp = Ramp {
                tones: c0.max(c1),
                share: (share0, share1),
                load: (load0, load1),
                speed: (speed0.abs(), speed1.abs()),
            };
            for k in 0..MOST_TONES {
                motor.amplitude[k] = start[k];
                motor.step[k] = (end[k] - start[k]) * per_sample;
            }
            let blade_pass_hz = speed1.abs() / TAU * blades as f32;
            motor.whoosh.set(
                (blade_pass_hz * block.whoosh_centre).max(80.0),
                block.whoosh_q,
                sample_rate,
            );
            motor.wobble.set(30.0, sample_rate);
            for (f, hz) in motor.flutter.iter_mut().zip(flutter_hz) {
                f.set(hz, sample_rate);
            }
        }
        let mismatch_depth = 0.6 * block.blade_mismatch;
        let hiss_exponent = block.loudness_exponent + 0.5;

        let out = &mut buffers.outputs[0][..n];
        for (s, sample) in out.iter_mut().enumerate() {
            let x = (s as f32 + 1.0) * per_sample;
            let mut motors = 0.0;
            let mut hiss_power = 0.0;
            let mut shaft_push = 0.0;
            for (motor, ramp) in self.motors.iter_mut().zip(&ramps) {
                let along = |(from, to): (f32, f32)| from + (to - from) * x;
                let share = along(ramp.share);
                let load = along(ramp.load);
                let speed = along(ramp.speed);
                let wobble = (motor.wobble.low(motor.noise.white()) * wobble_spread)
                    .clamp(-3.0, 3.0)
                    * block.speed_wobble;
                motor.turn += speed * (1.0 + wobble) / sample_rate;
                if motor.turn > TAU {
                    motor.turn -= TAU;
                }
                let (sin, cos) = motor.turn.sin_cos();
                let mut flutter = [0.0; 3];
                for (j, f) in flutter.iter_mut().enumerate() {
                    let white = motor.noise.white();
                    *f = (motor.flutter[j].low(white) * flutter_spread[j]).clamp(-2.5, 2.5)
                        * block.tone_roughness
                        * 0.35;
                }
                // Each tone k + 1 times the rotation rate, by turning a
                // phasor one rotation step at a time.
                let (mut re, mut im) = (cos, sin);
                let mut tone = 0.0;
                let mut blade_pass_cos = 0.0;
                for k in 0..ramp.tones {
                    let a = motor.amplitude[k]
                        * (1.0 + mismatch_depth * motor.mismatch[k])
                        * (1.0 + flutter[k % 3]).max(0.0);
                    motor.amplitude[k] += motor.step[k];
                    let (pc, ps) = motor.phase[k];
                    tone += a * (im * pc + re * ps);
                    if k + 1 == blades {
                        blade_pass_cos = re;
                    }
                    let next = re * cos - im * sin;
                    im = re * sin + im * cos;
                    re = next;
                }
                let whoosh_level = block.whoosh_level
                    * share.powf(block.loudness_exponent)
                    * (0.6 + 0.4 * load)
                    * 0.08;
                let white = motor.noise.white();
                let whoosh = motor.whoosh.pass(white)
                    * whoosh_level
                    * (1.0 + block.whoosh_swish * blade_pass_cos);
                let whine_hz = speed / TAU * pole_pairs;
                let whine = if whine_hz < 0.9 * nyquist {
                    (motor.turn * pole_pairs).sin() * block.motor_whine * load * share * 0.1
                } else {
                    0.0
                };
                motors += tone + whoosh + whine;
                hiss_power += share.powf(2.0 * hiss_exponent);
                shaft_push += sin * share * share;
            }
            let white = self.hiss_noise.white();
            let hiss = self.hiss_high_cut.low(self.hiss_low_cut.high(white))
                * block.hiss_level
                * hiss_power.sqrt()
                * 0.16;
            let motors = self.body.pass(motors + hiss);
            let ring_in = shaft_push * 0.5 + motors * 0.3;
            let ring = self.hum_first.pass(ring_in) + 0.7 * self.hum_second.pass(ring_in);
            let hum = block.hum_level
                * (block.hum_follow * shaft_push * 0.15 + (1.0 - block.hum_follow) * ring * 0.3);
            *sample = motors + hum;
        }
        ProcessStatus::OutputsModified
    }

    fn new_stream(&mut self, stream_info: &StreamInfo, _: &mut ProcStreamCtx) {
        self.sample_rate = stream_info.sample_rate.get() as f32;
    }
}
