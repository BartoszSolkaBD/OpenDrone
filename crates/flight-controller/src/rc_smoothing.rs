//! How the Flight Controller times the Radio Link's frames and smooths the
//! steps between them: Betaflight 2026.6.2's `updateRcRefreshRate`,
//! `shouldUpdateSmoothing`, `rcSmoothingSetFilterCutoffs` and
//! `processRcSmoothingFilter` (`src/main/fc/rc.c`), reimplemented from their
//! behaviour; the Betaflight research §3.1.
//!
//! # Detecting the Packet Rate
//!
//! Each frame, the gap since the last one is held within 800 µs to 65.5 ms
//! (about 1,250 Hz to 15 Hz); a gap outside that is used held, but doesn't
//! count. The detected Packet Rate (Betaflight's "Detected Rx frequency")
//! then moves slowly:
//!
//! - A frame whose rate is within 20% of it moves it a tenth of the way.
//! - Three frames in a row more than 20% off on the same side snap it to the
//!   newest one's rate: the Packet Rate changed.
//! - Frames off on alternating sides are jitter, and are ignored.
//! - Every third frame that counts, RC smoothing and feedforward's smoothing
//!   are tuned again to it.
//! - While the signal is lost it holds.
//!
//! # RC smoothing
//!
//! A third-order low-pass (PT3) runs every loop on each setpoint, on the
//! throttle and on feedforward, turning the steps between frames into a
//! smooth curve. While Horizon flies, one more on each of the roll and pitch
//! sticks' deflection smooths the sticks Horizon's strength reads. Its cutoff
//! is the Tune's (`rc_smoothing_setpoint_cutoff`,
//! `rc_smoothing_throttle_cutoff`) or, at 0, worked out from the detected
//! Packet Rate: `rate × 1.5 ÷ (1 + rc_smoothing_auto_factor ÷ 10)`, never
//! below 15 Hz, cut to whole Hz. At the default auto factor 30 that is 93 Hz
//! at 250 Hz and 187 Hz at 500 Hz. With `rc_smoothing` off, the Flight
//! Controller uses each frame's setpoints, throttle and feedforward as they
//! come.
//!
//! # A fresh Flight Controller
//!
//! Power-up skips Betaflight's own: the detected Packet Rate starts settled
//! at the Radio Link's Packet Rate, with RC smoothing and feedforward tuned to
//! it, and each smoothing filter settles on the first frame's values, as if
//! the sticks had rested there since power-up. Betaflight starts at 100 Hz,
//! and its filters pass nothing until it has timed three frames.

use opendrone_maths::{Fingerprinter, functions};

use crate::filter::{Lowpass, pt3_gain};
use crate::tune::Tune;

/// The shortest and longest gap between frames that counts, in µs
/// (`RX_INTERVAL_MIN_US`, `RX_INTERVAL_MAX_US`).
const SHORTEST_GAP: u64 = 800;
const LONGEST_GAP: u64 = 65_500;
/// How far a frame's rate moves the detected Packet Rate (`smoothingFactor`).
const SMOOTHING: f64 = 0.1;
/// A frame's rate this share or more away from the detected Packet Rate is
/// off.
const OFF_BY: f64 = 0.2;
/// The lowest automatic RC smoothing cutoff, in Hz (`minCutoffHz`).
const LOWEST_CUTOFF: f64 = 15.0;

/// The Flight Controller's timing of the Radio Link's frames.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PacketRate {
    /// When the last frame arrived, in µs since power-up (`lastRxTimeUs`).
    last_frame_us: u64,
    /// The gap since the frame before, held within 800 µs to 65.5 ms, in µs
    /// (`currentRxIntervalUs`).
    pub gap_us: u64,
    /// The rate that gap gives, in Hz (`currentRxRateHz`).
    pub rate_hz: f64,
    /// The gap needed no holding (`isRxRateValid`).
    pub counts: bool,
    /// The detected Packet Rate, in Hz (`smoothedRxRateHz`).
    pub detected_hz: f64,
    /// Frames in a row that counted, since the last retune (`validCount`).
    in_line: u32,
    /// Frames in a row off on the same side (`outlierCount`), and which side
    /// (`prevOutlierSign`).
    off: u32,
    off_side: i8,
}

impl PacketRate {
    /// At power-up: settled at the Radio Link's Packet Rate, with a frame
    /// just arrived, at 0 µs, as the receiver counts it
    /// ([`crate::failsafe`]).
    pub fn settled(packet_rate_hz: u32) -> PacketRate {
        let hz = f64::from(packet_rate_hz.max(1));
        let gap_us =
            (1_000_000 / u64::from(packet_rate_hz.max(1))).clamp(SHORTEST_GAP, LONGEST_GAP);
        PacketRate {
            last_frame_us: 0,
            gap_us,
            rate_hz: 1e6 / gap_us as f64,
            counts: true,
            detected_hz: hz,
            in_line: 0,
            off: 0,
            off_side: 0,
        }
    }

    /// Betaflight's timing of each frame, or of a look while the signal is
    /// lost (`updateRcRefreshRate`): the gap from the last frame to `now_us`.
    pub fn frame_check(&mut self, now_us: u64, signal: bool) {
        let gap = now_us.saturating_sub(self.last_frame_us);
        if signal {
            self.last_frame_us = now_us;
        }
        self.gap_us = gap.clamp(SHORTEST_GAP, LONGEST_GAP);
        self.rate_hz = 1e6 / self.gap_us as f64;
        self.counts = gap == self.gap_us;
    }

    /// Moves the detected Packet Rate on by the newest frame, and says
    /// whether to retune RC smoothing and feedforward to it
    /// (`shouldUpdateSmoothing`).
    pub fn retune(&mut self, signal: bool) -> bool {
        if !(signal && self.counts) {
            self.in_line = 0;
            self.off = 0;
            return false;
        }
        let difference = self.rate_hz - self.detected_hz;
        if difference.abs() > self.detected_hz * OFF_BY {
            let side = if difference < 0.0 { -1 } else { 1 };
            if self.off == 0 {
                self.off = 1;
                self.off_side = side;
            } else if side != self.off_side {
                // Off on the other side: jitter, so the count starts again.
                self.off = 0;
                self.off_side = side;
            } else {
                self.off += 1;
            }
            self.in_line = 0;
        } else {
            self.detected_hz += SMOOTHING * (self.rate_hz - self.detected_hz);
            self.in_line += 1;
            self.off = 0;
        }
        if self.in_line >= 3 {
            self.in_line = 0;
            return true;
        }
        if self.off >= 3 {
            // The Packet Rate changed: snap to it.
            self.detected_hz = self.rate_hz;
            self.off = 0;
        }
        false
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.last_frame_us);
        f.write_u64(self.gap_us);
        f.write_f64(self.rate_hz);
        f.write_u64(u64::from(self.counts));
        f.write_f64(self.detected_hz);
        f.write_u64(u64::from(self.in_line));
        f.write_u64(u64::from(self.off));
        f.write_u64((i64::from(self.off_side) + 1) as u64);
    }
}

/// RC smoothing: the PT3 low-passes on the setpoints, the throttle and
/// feedforward.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RcSmoothing {
    on: bool,
    /// The automatic cutoffs' share of the detected Packet Rate
    /// (`autoSmoothnessFactorSetpoint`, `autoSmoothnessFactorThrottle`).
    auto_setpoint: f64,
    auto_throttle: f64,
    /// The Tune's cutoffs, in Hz: 0 is automatic.
    setpoint_setting: u8,
    throttle_setting: u8,
    /// The cutoffs in use, in whole Hz (`setpointCutoffFrequency`,
    /// `throttleCutoffFrequency`).
    pub setpoint_cutoff_hz: u16,
    pub throttle_cutoff_hz: u16,
    /// The loop's step, in seconds.
    dt: f64,
    /// Roll, pitch and yaw's setpoints, then the throttle.
    setpoint: [Lowpass; 4],
    feedforward: [Lowpass; 3],
    /// Roll and pitch's stick deflection, for Horizon
    /// (`filterRcDeflection`).
    deflection_lowpass: [Lowpass; 2],
    /// The newest frame's setpoints and throttle (`rxDataToSmooth`), and
    /// roll and pitch's deflection (`rcDeflection`), and whether the filters
    /// have settled on the first.
    newest: [f64; 4],
    newest_deflection: [f64; 2],
    settled: bool,
    /// The smoothed setpoints and throttle, and feedforward, from the last
    /// loop.
    pub smoothed: [f64; 4],
    pub feedforward_smoothed: [f64; 3],
    /// Roll and pitch's deflection as Horizon reads it, from −1 to +1
    /// (`getRcDeflection`): smoothed while Horizon flies, with RC smoothing
    /// on.
    pub deflection: [f64; 2],
}

impl RcSmoothing {
    /// At power-up, tuned to the detected Packet Rate.
    pub fn new(tune: &Tune, loop_hz: u32, detected_hz: f64) -> RcSmoothing {
        let mut smoothing = RcSmoothing {
            on: tune.rc_smoothing,
            auto_setpoint: 1.5 / (1.0 + f64::from(tune.rc_smoothing_auto_factor) / 10.0),
            auto_throttle: 1.5 / (1.0 + f64::from(tune.rc_smoothing_auto_factor_throttle) / 10.0),
            setpoint_setting: tune.rc_smoothing_setpoint_cutoff,
            throttle_setting: tune.rc_smoothing_throttle_cutoff,
            setpoint_cutoff_hz: u16::from(tune.rc_smoothing_setpoint_cutoff),
            throttle_cutoff_hz: u16::from(tune.rc_smoothing_throttle_cutoff),
            dt: 1.0 / f64::from(loop_hz.max(1)),
            setpoint: core::array::from_fn(|_| Lowpass::pt(3, 0.0)),
            feedforward: core::array::from_fn(|_| Lowpass::pt(3, 0.0)),
            deflection_lowpass: core::array::from_fn(|_| Lowpass::pt(3, 0.0)),
            newest: [0.0; 4],
            newest_deflection: [0.0; 2],
            settled: false,
            smoothed: [0.0; 4],
            feedforward_smoothed: [0.0; 3],
            deflection: [0.0; 2],
        };
        if smoothing.on {
            smoothing.set_cutoffs(detected_hz);
        }
        smoothing
    }

    /// Whether RC smoothing is on (`rc_smoothing`).
    pub fn on(&self) -> bool {
        self.on
    }

    /// Tunes the cutoffs to the detected Packet Rate, or to the Tune's own
    /// (`rcSmoothingSetFilterCutoffs`). Only with RC smoothing on.
    pub fn set_cutoffs(&mut self, detected_hz: f64) {
        if !self.on {
            return;
        }
        // Betaflight keeps each cutoff as a whole number of Hz, cutting off
        // the fraction.
        let cutoff = |setting: u8, share: f64| -> u16 {
            if setting == 0 {
                functions::max(LOWEST_CUTOFF, detected_hz * share).trunc() as u16
            } else {
                u16::from(setting)
            }
        };
        self.setpoint_cutoff_hz = cutoff(self.setpoint_setting, self.auto_setpoint);
        self.throttle_cutoff_hz = cutoff(self.throttle_setting, self.auto_throttle);
        let k_setpoint = pt3_gain(f64::from(self.setpoint_cutoff_hz), self.dt);
        let k_throttle = pt3_gain(f64::from(self.throttle_cutoff_hz), self.dt);
        for filter in self.setpoint.iter_mut().take(3) {
            filter.set_k(k_setpoint);
        }
        self.setpoint[3].set_k(k_throttle);
        for filter in self
            .feedforward
            .iter_mut()
            .chain(&mut self.deflection_lowpass)
        {
            filter.set_k(k_setpoint);
        }
    }

    /// One loop (`processRcSmoothingFilter`): `newest` is the frame's
    /// setpoints and throttle, and roll and pitch's deflection, when a
    /// frame's Channels were worked out this loop. With RC smoothing on, every
    /// filter takes one step, the deflection's only while Horizon flies; with
    /// it off, the newest values pass as they are.
    pub fn apply(
        &mut self,
        newest: Option<([f64; 4], [f64; 2])>,
        feedforward: [f64; 3],
        horizon: bool,
    ) {
        if let Some((newest, deflection)) = newest {
            self.newest = newest;
            self.newest_deflection = deflection;
            if !self.settled {
                self.settled = true;
                for (filter, value) in self.setpoint.iter_mut().zip(newest) {
                    filter.settle(value);
                }
                for (filter, value) in self.feedforward.iter_mut().zip(feedforward) {
                    filter.settle(value);
                }
                for (filter, value) in self.deflection_lowpass.iter_mut().zip(deflection) {
                    filter.settle(value);
                }
            }
        }
        if !self.on {
            self.smoothed = self.newest;
            self.feedforward_smoothed = feedforward;
            self.deflection = self.newest_deflection;
            return;
        }
        for (smoothed, (filter, value)) in self.deflection.iter_mut().zip(
            self.deflection_lowpass
                .iter_mut()
                .zip(self.newest_deflection),
        ) {
            *smoothed = if horizon { filter.apply(value) } else { value };
        }
        for (smoothed, (filter, value)) in self
            .smoothed
            .iter_mut()
            .zip(self.setpoint.iter_mut().zip(self.newest))
        {
            *smoothed = filter.apply(value);
        }
        for (smoothed, (filter, value)) in self
            .feedforward_smoothed
            .iter_mut()
            .zip(self.feedforward.iter_mut().zip(feedforward))
        {
            *smoothed = filter.apply(value);
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.setpoint_cutoff_hz));
        f.write_u64(u64::from(self.throttle_cutoff_hz));
        for filter in self
            .setpoint
            .iter()
            .chain(&self.feedforward)
            .chain(&self.deflection_lowpass)
        {
            filter.write_fingerprint(f);
        }
        f.write_f64s(&self.newest);
        f.write_f64s(&self.newest_deflection);
        f.write_u64(u64::from(self.settled));
        f.write_f64s(&self.smoothed);
        f.write_f64s(&self.feedforward_smoothed);
        f.write_f64s(&self.deflection);
    }
}
