//! Betaflight 2026.6.2's low-pass filters (`src/main/common/filter.c`),
//! reimplemented from their behaviour: the first-, second- and third-order
//! filters PT1, PT2 and PT3, and the second-order state-variable filter its
//! CLI still calls BIQUAD.
//!
//! Each runs once a loop. A PTn filter is n first-order stages in a row, each
//! moving its state a share `k` of the way to its input:
//!
//! - PT1's `k` is `ω / (ω + 1)`, with `ω = 2π × cutoff × the loop's step`
//!   (`pt1FilterGain`).
//! - PT2 and PT3 move their cutoff up by 1.553774 and 1.961459
//!   (`1 / √(2^(1/n) − 1)`), so the whole filter is 3 dB down at the cutoff
//!   (`pt2FilterGain`, `pt3FilterGain`).
//! - A filter set by its delay instead takes `k = step / (step + delay)`
//!   (`pt1FilterGainFromDelay`).
//!
//! The BIQUAD is a Butterworth second-order low-pass in state-variable form
//! (`svfLowpassFilterApply`). Betaflight works its tangent out with a fast
//! approximation of sine and cosine; here it is the exact one, which differs
//! by about a millionth.
//!
//! # Settled
//!
//! A fresh Flight Controller skips Betaflight's power-up, so each filter
//! starts settled ([`Lowpass::settle`]): as if its input had held its first
//! value for a long time. Betaflight's own filters start at 0 and settle in
//! the seconds before a pilot can arm.

use core::f64::consts::{PI, SQRT_2};

use opendrone_maths::{Fingerprinter, functions};

/// How much PT2 and PT3 move their cutoff up (`CUTOFF_CORRECTION_PT2`,
/// `CUTOFF_CORRECTION_PT3`).
const CUTOFF_CORRECTION_PT2: f64 = 1.553_773_974;
const CUTOFF_CORRECTION_PT3: f64 = 1.961_459_177;

/// The BIQUAD's damping, `√2`: a Butterworth response.
const BUTTERWORTH: f64 = SQRT_2;

/// A low-pass filter's kind, as Betaflight's CLI names it
/// (`gyro_lpf1_type` and the like).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LowpassType {
    Pt1,
    /// Betaflight's CLI says BIQUAD; since 2025.12 it runs a state-variable
    /// filter (`FILTER_SVF`).
    Biquad,
    Pt2,
    Pt3,
}

impl LowpassType {
    /// Betaflight's words for the kinds, in its order.
    pub const WORDS: [&'static str; 4] = ["PT1", "BIQUAD", "PT2", "PT3"];

    /// The kind at its place in [`LowpassType::WORDS`].
    pub fn from_place(place: u32) -> LowpassType {
        match place {
            0 => LowpassType::Pt1,
            1 => LowpassType::Biquad,
            2 => LowpassType::Pt2,
            _ => LowpassType::Pt3,
        }
    }

    pub fn place(self) -> u64 {
        match self {
            LowpassType::Pt1 => 0,
            LowpassType::Biquad => 1,
            LowpassType::Pt2 => 2,
            LowpassType::Pt3 => 3,
        }
    }
}

/// PT1's share `k` for a cutoff in Hz, at a step of `dt` seconds
/// (`pt1FilterGain`).
pub(crate) fn pt1_gain(cutoff_hz: f64, dt: f64) -> f64 {
    let omega = 2.0 * PI * cutoff_hz * dt;
    omega / (omega + 1.0)
}

/// PT1's share `k` for a delay (time constant) in seconds, at a step of `dt`
/// seconds: 1, no filtering, for no delay (`pt1FilterGainFromDelay`).
pub(crate) fn pt1_gain_from_delay(delay: f64, dt: f64) -> f64 {
    if delay <= 0.0 { 1.0 } else { dt / (dt + delay) }
}

/// PT3's share `k` for a cutoff in Hz (`pt3FilterGain`).
pub(crate) fn pt3_gain(cutoff_hz: f64, dt: f64) -> f64 {
    pt1_gain(cutoff_hz * CUTOFF_CORRECTION_PT3, dt)
}

/// One low-pass filter: off, a PTn filter, or a BIQUAD.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Lowpass {
    /// No filter (`nullFilterApply`): the output is the input.
    Off,
    /// `order` first-order stages in a row, each with the share `k`
    /// (`pt1Filter_t`, `pt2Filter_t`, `pt3Filter_t`).
    Pt {
        order: usize,
        k: f64,
        state: [f64; 3],
    },
    /// The state-variable BIQUAD (`svfLowpassFilter_t`).
    Svf {
        a1: f64,
        a2: f64,
        f: f64,
        ic1: f64,
        ic2: f64,
    },
}

impl Lowpass {
    /// A filter of `kind` at `cutoff_hz`, running every `dt` seconds; off
    /// when the cutoff is 0. A BIQUAD above `nyquist_hz` is off too, as
    /// Betaflight leaves it (`gyroInitLowpassFilterLpf`, `pidInitFilters`).
    pub fn new(kind: LowpassType, cutoff_hz: f64, dt: f64, nyquist_hz: f64) -> Lowpass {
        if cutoff_hz <= 0.0 {
            return Lowpass::Off;
        }
        match kind {
            LowpassType::Pt1 => Lowpass::pt(1, pt1_gain(cutoff_hz, dt)),
            LowpassType::Pt2 => Lowpass::pt(2, pt1_gain(cutoff_hz * CUTOFF_CORRECTION_PT2, dt)),
            LowpassType::Pt3 => Lowpass::pt(3, pt3_gain(cutoff_hz, dt)),
            LowpassType::Biquad if cutoff_hz <= nyquist_hz => {
                let (a1, a2, f) = svf_coefficients(cutoff_hz, dt);
                Lowpass::Svf {
                    a1,
                    a2,
                    f,
                    ic1: 0.0,
                    ic2: 0.0,
                }
            }
            LowpassType::Biquad => Lowpass::Off,
        }
    }

    /// A PTn filter of `order` stages with the share `k`.
    pub fn pt(order: usize, k: f64) -> Lowpass {
        Lowpass::Pt {
            order,
            k,
            state: [0.0; 3],
        }
    }

    /// Moves the cutoff, keeping the filter's state, as Betaflight's dynamic
    /// low-passes do (`pt1FilterUpdateCutoff`, `svfLowpassFilterUpdate`). An
    /// Off filter stays off.
    pub fn set_cutoff(&mut self, kind: LowpassType, cutoff_hz: f64, dt: f64) {
        match self {
            Lowpass::Off => {}
            Lowpass::Pt { k, .. } => {
                *k = match kind {
                    LowpassType::Pt2 => pt1_gain(cutoff_hz * CUTOFF_CORRECTION_PT2, dt),
                    LowpassType::Pt3 => pt3_gain(cutoff_hz, dt),
                    LowpassType::Pt1 | LowpassType::Biquad => pt1_gain(cutoff_hz, dt),
                };
            }
            Lowpass::Svf { a1, a2, f, .. } => {
                (*a1, *a2, *f) = svf_coefficients(cutoff_hz, dt);
            }
        }
    }

    /// Sets a PTn filter's share `k` straight (`pt1FilterUpdateCutoff` with a
    /// gain worked out elsewhere).
    pub fn set_k(&mut self, new_k: f64) {
        if let Lowpass::Pt { k, .. } = self {
            *k = new_k;
        }
    }

    /// The filter as if its input had held at `value` for a long time.
    pub fn settle(&mut self, value: f64) {
        match self {
            Lowpass::Off => {}
            Lowpass::Pt { state, .. } => *state = [value; 3],
            Lowpass::Svf { ic1, ic2, .. } => {
                *ic1 = 0.0;
                *ic2 = value;
            }
        }
    }

    /// One step: takes the input, gives the output.
    pub fn apply(&mut self, input: f64) -> f64 {
        match self {
            Lowpass::Off => input,
            Lowpass::Pt { order, k, state } => {
                let mut x = input;
                // Betaflight's PT2 and PT3 keep their first stage in
                // `state1`; here the stages run first to last.
                for stage in state.iter_mut().take(*order) {
                    *stage += *k * (x - *stage);
                    x = *stage;
                }
                x
            }
            Lowpass::Svf {
                a1,
                a2,
                f,
                ic1,
                ic2,
            } => {
                let v3 = input - *ic2;
                let v1 = *a1 * *ic1 + *a2 * v3;
                let v2 = *ic2 + *f * v1;
                *ic1 = 2.0 * v1 - *ic1;
                *ic2 = 2.0 * v2 - *ic2;
                v2
            }
        }
    }

    pub fn write_fingerprint(&self, fp: &mut Fingerprinter) {
        match self {
            Lowpass::Off => fp.write_u64(0),
            Lowpass::Pt { order, k, state } => {
                fp.write_u64(*order as u64);
                fp.write_f64(*k);
                fp.write_f64s(state);
            }
            Lowpass::Svf {
                a1,
                a2,
                f,
                ic1,
                ic2,
            } => {
                fp.write_u64(4);
                fp.write_f64s(&[*a1, *a2, *f, *ic1, *ic2]);
            }
        }
    }
}

/// The BIQUAD's coefficients for a cutoff (`svfLowpassFilterUpdate`).
fn svf_coefficients(cutoff_hz: f64, dt: f64) -> (f64, f64, f64) {
    let (sn, cs) = functions::sin_cos(PI * cutoff_hz * dt);
    let f = sn / cs;
    let inv_denom = 1.0 / (1.0 + f * (f + BUTTERWORTH));
    (inv_denom, f * inv_denom, f)
}

/// A dynamic low-pass's cutoff, in Hz, at a throttle from 0 to 1: with an
/// expo, a curve from the lowest cutoff to the highest
/// (`dynLpfCutoffFreq`); without, Betaflight's older curve, never below the
/// lowest (`dynThrottle`).
pub(crate) fn dynamic_cutoff(throttle: f64, lowest: u16, highest: u16, expo: u8) -> f64 {
    let (lowest, highest) = (f64::from(lowest), f64::from(highest));
    if expo > 0 {
        let expo = f64::from(expo) / 10.0;
        let curve = throttle * (1.0 - throttle) * expo + throttle;
        (highest - lowest) * curve + lowest
    } else {
        let curve = throttle * (1.0 - throttle * throttle / 3.0) * 1.5;
        functions::max(curve * highest, lowest)
    }
}
