//! On the Quad: the Listening Position of a camera's microphone on board, as
//! one Firewheel node, mono in and stereo out.
//!
//! The microphone cuts the lowest lows. Wind blows on it, growing steeply
//! with airspeed and with the props' downwash. A camera-style compressor
//! with a 20 ms attack then makes a punch-out swell while everything else
//! ducks, letting the first instant of the swell slip through. The camera's
//! audio rolls off the highest highs. Nothing clips: clipping was tried in
//! #34, and dropped. On the loudest punch-outs, a 5″'s at speed, the swell's
//! first instant would pass full scale, so a safety limiter at the very end
//! keeps every sample within it; nothing else reaches it.
//!
//! These settings are the listener's, so they're the game's, the same for
//! every Quad; a Quad's sound block never carries them.

use std::f32::consts::PI;

use firewheel::StreamInfo;
use firewheel::channel_config::{ChannelConfig, ChannelCount};
use firewheel::node::{
    AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, NodeError,
    ProcBuffers, ProcExtra, ProcInfo, ProcStreamCtx, ProcessStatus,
};

use crate::dsp::{Noise, OnePole, TwoPole, amplitude, decibels, smoothing};
use crate::voice::{Frames, FramesSlot};
use crate::{QuadFrame, QuadSound};

/// The camera microphone's settings, from #34's round 2.
#[derive(Clone, Copy, Debug)]
struct Microphone {
    /// Below this the microphone hears little, in Hz; above the other, the
    /// camera's audio rolls off.
    low_cut: f32,
    high_cut: f32,
    /// The wind's level at `wind_reference_speed` (m/s), growing as the
    /// speed over the microphone to `wind_exponent`.
    wind_level: f32,
    wind_reference_speed: f32,
    wind_exponent: f32,
    /// The share of the props' downwash that blows over the microphone.
    wind_downwash: f32,
    /// The wind's rumble sits below this, in Hz, rising with speed.
    wind_corner: f32,
    /// How much the wind gusts.
    wind_gust: f32,
    /// The compressor: above the threshold (dB), each `ratio` dB in gives
    /// 1 dB out. Its attack and release are in seconds, its make-up gain in
    /// dB.
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    make_up: f32,
}

const MICROPHONE: Microphone = Microphone {
    low_cut: 90.0,
    high_cut: 11_000.0,
    wind_level: 0.5,
    wind_reference_speed: 20.0,
    wind_exponent: 2.0,
    wind_downwash: 0.5,
    wind_corner: 350.0,
    wind_gust: 0.5,
    threshold: -26.0,
    ratio: 4.0,
    // Settled in #34: about 20 ms.
    attack: 0.020,
    release: 0.250,
    make_up: 10.0,
};

/// The compressor's soft knee, in dB.
const KNEE: f32 = 6.0;

/// How fast the Quad fades out on Pause, and back in on Resume: a time
/// constant, in seconds. Five of them, 0.1 s, bring it 43 dB down.
const PAUSE_FADE: f32 = 0.020;

/// The safety limiter's ceiling, as an amplitude (−0.9 dB of full scale),
/// and how fast it lets go, a time constant in seconds.
const CEILING: f32 = 0.9;
const LIMITER_RELEASE: f32 = 0.150;

/// The default volumes, as Firewheel's sliders: master 0.8 and Quad 1.0
/// (#34). A slider's loudness is its value squared.
const VOLUME: f32 = 0.8 * 0.8 * (1.0 * 1.0);

/// The camera's microphone on board.
pub(crate) struct OnTheQuadNode {
    pub(crate) quad: QuadSound,
}

impl AudioNode for OnTheQuadNode {
    type Configuration = FramesSlot;

    fn info(&self, _: &FramesSlot) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("on_the_quad")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::MONO,
                num_outputs: ChannelCount::STEREO,
            }))
    }

    fn construct_processor(
        &self,
        frames: &FramesSlot,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        Ok(OnTheQuad {
            quad: self.quad,
            frames: frames.take(),
            last: QuadFrame::default(),
            sample_rate: cx.stream_info.sample_rate.get() as f32,
            low_cut: TwoPole::default(),
            high_cut: TwoPole::default(),
            noise: Noise::new(99),
            wind_rumble: TwoPole::default(),
            wind_low_cut: TwoPole::default(),
            gusts: [OnePole::default(); 2],
            envelope: 0.0,
            pause_gain: 1.0,
            peak: 0.0,
        })
    }
}

struct OnTheQuad {
    quad: QuadSound,
    frames: Frames,
    last: QuadFrame,
    sample_rate: f32,
    low_cut: TwoPole,
    high_cut: TwoPole,
    noise: Noise,
    wind_rumble: TwoPole,
    wind_low_cut: TwoPole,
    gusts: [OnePole; 2],
    /// The compressor's level detector.
    envelope: f32,
    pause_gain: f32,
    /// The safety limiter's peak.
    peak: f32,
}

impl OnTheQuad {
    /// How fast air moves over the microphone, in m/s: the airspeed, and a
    /// share of the props' downwash, from momentum theory: the mean thrust T
    /// pushes air through each prop's disc of area A at √(T ÷ 2ρA).
    fn air_over_microphone(&self, frame: &QuadFrame) -> f32 {
        let disc = PI * (self.quad.prop_diameter * 0.5).powi(2);
        let thrust = frame.motors.iter().map(|m| m.thrust.max(0.0)).sum::<f32>() / 4.0;
        let downwash = (thrust / (2.0 * self.quad.air_density.max(0.01) * disc.max(1e-6))).sqrt();
        (frame.airspeed.powi(2) + (downwash * MICROPHONE.wind_downwash).powi(2)).sqrt()
    }

    fn wind_level(speed: f32) -> f32 {
        MICROPHONE.wind_level
            * (speed / MICROPHONE.wind_reference_speed)
                .powf(MICROPHONE.wind_exponent)
                .min(4.0)
    }
}

impl AudioNodeProcessor for OnTheQuad {
    fn process(
        &mut self,
        info: &ProcInfo,
        buffers: ProcBuffers,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        let n = info.frames;
        let rate = self.sample_rate;
        let now = *self.frames.read();
        let before = self.last;
        self.last = now;
        let per_sample = 1.0 / n as f32;
        let mic = MICROPHONE;

        let speed = self.air_over_microphone(&now);
        let wind0 = Self::wind_level(self.air_over_microphone(&before));
        let wind1 = Self::wind_level(speed);
        self.low_cut.set(mic.low_cut, rate);
        self.wind_low_cut.set(mic.low_cut, rate);
        self.high_cut.set(mic.high_cut, rate);
        self.wind_rumble.set(
            mic.wind_corner * (1.0 + speed / mic.wind_reference_speed),
            rate,
        );
        self.gusts[0].set(1.5, rate);
        self.gusts[1].set(0.4, rate);
        let attack = smoothing(mic.attack, rate);
        let release = smoothing(mic.release, rate);
        let make_up = amplitude(mic.make_up);
        let slope = 1.0 / mic.ratio - 1.0;
        let pause_step = smoothing(PAUSE_FADE, rate);
        let pause_target = if now.paused { 0.0 } else { 1.0 };
        let let_go = 1.0 - smoothing(LIMITER_RELEASE, rate);

        let ProcBuffers {
            inputs, outputs, ..
        } = buffers;
        let input = &inputs[0][..n];
        let (left, right) = outputs.split_at_mut(1);
        let (left, right) = (&mut left[0][..n], &mut right[0][..n]);
        for s in 0..n {
            let x = (s as f32 + 1.0) * per_sample;
            let quad = self.low_cut.high(input[s]);
            let gust = 1.0
                + mic.wind_gust
                    * (self.gusts[0].low(self.noise.white()) * 6.0
                        + self.gusts[1].low(self.noise.white()) * 10.0);
            let rumble = self.wind_rumble.low(self.noise.white());
            let wind = self
                .wind_low_cut
                .high(rumble * (wind0 + (wind1 - wind0) * x) * gust.max(0.0) * 1.5);
            let heard = quad + wind;

            let level = heard.abs();
            let towards = if level > self.envelope {
                attack
            } else {
                release
            };
            self.envelope += towards * (level - self.envelope);
            let over = decibels(self.envelope) - mic.threshold;
            let reduction = if over <= -KNEE * 0.5 {
                0.0
            } else if over >= KNEE * 0.5 {
                over * slope
            } else {
                let o = over + KNEE * 0.5;
                slope * o * o / (2.0 * KNEE)
            };
            let gain = amplitude(reduction) * make_up;

            self.pause_gain += pause_step * (pause_target - self.pause_gain);
            let y = self.high_cut.low(heard * gain * self.pause_gain * VOLUME);

            // The safety limiter, not part of the camera's character: it
            // jumps to each peak above its ceiling at once, so no sample
            // ever goes past it, and lets go slowly.
            self.peak = (self.peak * let_go).max(y.abs());
            let y = if self.peak > CEILING {
                y * CEILING / self.peak
            } else {
                y
            };
            left[s] = y;
            right[s] = y;
        }
        ProcessStatus::OutputsModified
    }

    fn new_stream(&mut self, stream_info: &StreamInfo, _: &mut ProcStreamCtx) {
        self.sample_rate = stream_info.sample_rate.get() as f32;
    }
}
