//! Small sound-making parts the Quad's sound is built from: noise, filters
//! and level conversions. Nothing here allocates once made, so all of it is
//! safe on the sound device's own thread.

use std::f32::consts::{PI, TAU};

/// Decibels to an amplitude: +6 dB is about twice as loud.
pub(crate) fn amplitude(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// An amplitude to decibels, with silence at −180 dB.
pub(crate) fn decibels(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-9).log10()
}

/// How far a one-pole smoother moves towards its target each sample, for a
/// time constant `seconds`.
pub(crate) fn smoothing(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds.max(1e-5) * sample_rate)).exp()
}

/// White noise from a seeded xorshift: the same seed always gives the same
/// noise.
#[derive(Clone, Copy)]
pub(crate) struct Noise(u32);

impl Noise {
    pub(crate) fn new(seed: u32) -> Noise {
        Noise(seed.max(1))
    }

    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// From −1 to 1.
    pub(crate) fn white(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// From 0 to 1.
    pub(crate) fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
}

/// A one-pole low-pass, which also gives a high-pass (the input minus its
/// low-pass).
#[derive(Clone, Copy, Default)]
pub(crate) struct OnePole {
    z: f32,
    a: f32,
}

impl OnePole {
    pub(crate) fn set(&mut self, corner_hz: f32, sample_rate: f32) {
        let corner = corner_hz.clamp(1.0, sample_rate * 0.49);
        self.a = 1.0 - (-TAU * corner / sample_rate).exp();
    }

    pub(crate) fn low(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }

    pub(crate) fn high(&mut self, x: f32) -> f32 {
        x - self.low(x)
    }
}

/// Two one-pole filters in a row, for a steeper edge.
#[derive(Clone, Copy, Default)]
pub(crate) struct TwoPole([OnePole; 2]);

impl TwoPole {
    pub(crate) fn set(&mut self, corner_hz: f32, sample_rate: f32) {
        for pole in &mut self.0 {
            pole.set(corner_hz, sample_rate);
        }
    }

    pub(crate) fn low(&mut self, x: f32) -> f32 {
        let y = self.0[0].low(x);
        self.0[1].low(y)
    }

    pub(crate) fn high(&mut self, x: f32) -> f32 {
        let y = self.0[0].high(x);
        self.0[1].high(y)
    }
}

/// A state-variable filter (Simper's form), used as a band-pass with unity
/// gain at its centre.
#[derive(Clone, Copy, Default)]
pub(crate) struct BandPass {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl BandPass {
    pub(crate) fn set(&mut self, centre_hz: f32, q: f32, sample_rate: f32) {
        let centre = centre_hz.clamp(10.0, sample_rate * 0.45);
        let g = (PI * centre / sample_rate).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub(crate) fn pass(&mut self, x: f32) -> f32 {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v1 * self.k
    }
}

/// A peaking filter (the RBJ cookbook's), for the frame's or the duct's
/// resonance.
#[derive(Clone, Copy, Default)]
pub(crate) struct Peak {
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
    pub(crate) fn set(&mut self, centre_hz: f32, q: f32, gain_db: f32, sample_rate: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = TAU * centre_hz.clamp(20.0, sample_rate * 0.45) / sample_rate;
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let a0 = 1.0 + alpha / a;
        self.b0 = (1.0 + alpha * a) / a0;
        self.b1 = (-2.0 * w.cos()) / a0;
        self.b2 = (1.0 - alpha * a) / a0;
        self.a1 = self.b1;
        self.a2 = (1.0 - alpha / a) / a0;
    }

    pub(crate) fn pass(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// How much a one-pole low-pass at `corner_hz` shrinks white noise, as the
/// factor that brings it back to a spread of about 1.
pub(crate) fn unit_spread(corner_hz: f32, sample_rate: f32) -> f32 {
    let a = 1.0 - (-TAU * corner_hz / sample_rate).exp();
    // Uniform white noise from −1 to 1 has a spread of 1/√3.
    1.0 / (0.577 * (a / (2.0 - a)).sqrt())
}
