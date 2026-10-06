//! PROTOTYPE (#34). Small DSP helpers used by the custom Firewheel nodes. Nothing here allocates
//! after construction, so it is safe on the audio thread.

use std::f32::consts::PI;

pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

/// xorshift32 white noise in [-1, 1].
#[derive(Clone, Copy)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    #[inline]
    pub fn white(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    #[inline]
    pub fn uniform(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
}

/// Topology-preserving state-variable filter (Simper/Cytomic form).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

pub struct SvfOut {
    pub lp: f32,
    pub bp: f32,
    pub hp: f32,
}

impl Svf {
    pub fn set(&mut self, fc: f32, q: f32, sr: f32) {
        let fc = fc.clamp(10.0, sr * 0.45);
        let g = (PI * fc / sr).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    #[inline]
    pub fn process(&mut self, v0: f32) -> SvfOut {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { lp: v2, bp: v1, hp: v0 - self.k * v1 - v2 }
    }
    /// Band-pass normalised to unity gain at the centre.
    #[inline]
    pub fn bp_norm(&mut self, v0: f32) -> f32 {
        let k = self.k;
        self.process(v0).bp * k
    }
    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
}

/// One-pole low-pass (also gives a high-pass as input minus low-pass).
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    pub z: f32,
    a: f32,
}

impl OnePole {
    pub fn set(&mut self, fc: f32, sr: f32) {
        let fc = fc.clamp(1.0, sr * 0.49);
        self.a = 1.0 - (-2.0 * PI * fc / sr).exp();
    }
    /// Smoothing towards a target with a time constant in seconds.
    pub fn set_tau(&mut self, tau_s: f32, sr: f32) {
        self.a = 1.0 - (-1.0 / (tau_s.max(1e-5) * sr)).exp();
    }
    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }
    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        x - self.lp(x)
    }
}

/// Peaking EQ biquad (RBJ cookbook), for the body/duct resonance.
#[derive(Clone, Copy, Default)]
pub struct Peak {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Peak {
    pub fn set(&mut self, fc: f32, q: f32, gain_db: f32, sr: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * fc.clamp(20.0, sr * 0.45) / sr;
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0 = 1.0 + alpha / a;
        self.b0 = (1.0 + alpha * a) / a0;
        self.b1 = (-2.0 * w.cos()) / a0;
        self.b2 = (1.0 - alpha * a) / a0;
        self.a1 = (-2.0 * w.cos()) / a0;
        self.a2 = (1.0 - alpha / a) / a0;
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Soft clipper blending tanh (soft) and a hard clamp, at a ceiling of 1.0.
#[inline]
pub fn clip(x: f32, hardness: f32) -> f32 {
    let soft = x.tanh();
    let hard = x.clamp(-1.0, 1.0);
    soft + (hard - soft) * hardness.clamp(0.0, 1.0)
}

/// A fractional delay line with cubic (Hermite) reads, for Where you stand's travel time.
pub struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    w: usize,
}

impl DelayLine {
    pub fn new(len_pow2: usize) -> Self {
        let n = len_pow2.next_power_of_two();
        Self { buf: vec![0.0; n], mask: n - 1, w: 0 }
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w = (self.w + 1) & self.mask;
    }
    /// Read `delay` samples back from the most recently pushed sample.
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.buf.len() - 4) as f32);
        let di = d.floor();
        let frac = d - di;
        let base = (self.w + self.buf.len() - 1 - di as usize) & self.mask;
        // Samples around the read point: y0 is newer, y1 older.
        let ym1 = self.buf[(base + 1) & self.mask];
        let y0 = self.buf[base];
        let y1 = self.buf[(base + self.buf.len() - 1) & self.mask];
        let y2 = self.buf[(base + self.buf.len() - 2) & self.mask];
        let c0 = y0;
        let c1 = 0.5 * (y1 - ym1);
        let c2 = ym1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
        let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);
        ((c3 * frac + c2) * frac + c1) * frac + c0
    }
    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = 0.0);
    }
}
