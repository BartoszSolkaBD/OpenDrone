//! Channels: the stick and switch values a receiver hands the Flight
//! Controller.

use opendrone_maths::Fingerprinter;

/// One Channel: a whole number, the step an ExpressLRS receiver hands the
/// Flight Controller over CRSF (11 bits, from 0 to 2047).
///
/// ELRS puts −100%, centre and +100% at 172, 992 and 1811, which it means as
/// 988, 1500 and 2012 µs. So a Channel keeps an Input Device's full
/// resolution as far as a real ELRS link carries it: steps of 1024 ÷ 1639 =
/// 0.625 µs. Sticks written in percent, or values in µs from an Input
/// Device, are rounded to the nearest step ([`Channel::from_micros`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Channel(u16);

/// ELRS's step for −100% (988 µs).
const LOWEST: f64 = 172.0;
/// ELRS's steps from −100% to +100% (988 to 2012 µs).
const STEPS: f64 = 1639.0;

impl Channel {
    /// −100%, the bottom of the throttle, or a switch's low position: 988 µs.
    pub const LOW: Channel = Channel(172);
    /// Centre stick, or a three-position switch's middle: 1500 µs.
    pub const CENTRE: Channel = Channel(992);
    /// +100%, or a switch's high position: 2012 µs.
    pub const HIGH: Channel = Channel(1811);

    /// The Channel at a CRSF step, or `None` above 2047 (11 bits).
    pub const fn from_step(step: u16) -> Option<Channel> {
        if step <= 2047 {
            Some(Channel(step))
        } else {
            None
        }
    }

    /// Its CRSF step, from 0 to 2047.
    pub const fn step(self) -> u16 {
        self.0
    }

    /// The nearest step to a value in µs, as ELRS maps 988–2012 µs onto
    /// 172–1811, kept within 11 bits.
    pub fn from_micros(micros: f64) -> Channel {
        let step = (LOWEST + (micros - 988.0) * STEPS / 1024.0).round();
        Channel(step.clamp(0.0, 2047.0) as u16)
    }

    /// The nearest step to a stick (roll, pitch or yaw) from −1 (full left,
    /// back or left) to +1 (full right, forward or right): 1500 µs plus 512 µs
    /// for each whole stick.
    pub fn from_stick(share: f64) -> Channel {
        Channel::from_micros(1500.0 + 512.0 * share)
    }

    /// The nearest step to a throttle from 0 (bottom) to 1 (top): 988 µs plus
    /// 1024 µs for the whole travel.
    pub fn from_throttle(share: f64) -> Channel {
        Channel::from_micros(988.0 + 1024.0 * share)
    }

    /// A two-position switch: high (2012 µs) when on, low (988 µs) when off.
    pub fn from_switch(on: bool) -> Channel {
        if on { Channel::HIGH } else { Channel::LOW }
    }

    /// What Betaflight 2026.6 makes of it, in µs: 0.62477120195241 × step +
    /// 881, so 172 reads 988.46 µs, 992 reads 1500.77 µs and 1811 reads
    /// 2012.46 µs (`crsfReadRawRC`, legacy channel scale, `rx/crsf.c`).
    pub fn micros(self) -> f64 {
        0.624_771_201_952_41 * f64::from(self.0) + 881.0
    }
}

/// Every Channel the Flight Controller sees, as one Radio Link frame carries
/// them. Arm, Flight Mode and Crash Flip always arrive on AUX1, AUX2 and AUX3
/// (ADR-0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channels {
    /// Right is high.
    pub roll: Channel,
    /// Forward (nose down) is high.
    pub pitch: Channel,
    /// Up is high.
    pub throttle: Channel,
    /// Right is high.
    pub yaw: Channel,
    /// AUX1, Arm: high is armed.
    pub arm: Channel,
    /// AUX2, Flight Mode: low Acro, middle Horizon, high Angle.
    pub flight_mode: Channel,
    /// AUX3, Crash Flip: high is on.
    pub crash_flip: Channel,
}

impl Channels {
    /// Sticks centred, throttle at the bottom, every switch low.
    pub const RESTING: Channels = Channels {
        roll: Channel::CENTRE,
        pitch: Channel::CENTRE,
        throttle: Channel::LOW,
        yaw: Channel::CENTRE,
        arm: Channel::LOW,
        flight_mode: Channel::LOW,
        crash_flip: Channel::LOW,
    };

    /// Every Channel in a fixed order: roll, pitch, throttle, yaw, AUX1–3.
    pub fn all(&self) -> [Channel; 7] {
        [
            self.roll,
            self.pitch,
            self.throttle,
            self.yaw,
            self.arm,
            self.flight_mode,
            self.crash_flip,
        ]
    }

    /// Feeds every Channel into a fingerprint, in the fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for channel in self.all() {
            f.write_u64(u64::from(channel.step()));
        }
    }
}
