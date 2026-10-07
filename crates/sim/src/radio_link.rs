//! Flight Inputs at the front door, and the Radio Link that carries their
//! Channels to the Flight Controller (ADR-0007).
//!
//! The Radio Link sends regular frames at the pilot's Packet Rate, like an
//! ELRS link, each carrying the newest Channels; the Flight Controller sees
//! an ELRS receiver on CRSF. It adds no delay beyond the frame spacing, and
//! sends nothing until the first Channels arrive. Locking to an Input
//! Device's report beat (ADR-0020), losing a device, and Input smoothing come
//! with the Radio Link ticket (#56) and the arming and Failsafe ticket (#52).

use opendrone_flight_controller::Channels;
use opendrone_maths::Fingerprinter;

use crate::{PhysicsRate, SimulationTime};

/// Anything that enters the Simulation and can change a flight, stamped with
/// the Simulation Time it arrives at. So far: Channels. An Input Device lost
/// or back, and Reset, arrive with #52.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightInput {
    /// The Flying Input Device's Channels, after its Calibration, channel
    /// mapping and deadband, at the whole-number steps a receiver outputs.
    Channels(Channels),
}

/// How many times a second the Radio Link delivers a frame, as in ELRS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PacketRate {
    hz: u32,
}

impl PacketRate {
    /// The Packet Rates a pilot can pick (the default is 250 Hz).
    pub const ALL: [u32; 7] = [50, 100, 150, 250, 333, 500, 1000];

    /// The Packet Rate of `hz` frames a second, if it is one a pilot can
    /// pick.
    pub fn from_hz(hz: u32) -> Option<PacketRate> {
        PacketRate::ALL.contains(&hz).then_some(PacketRate { hz })
    }

    pub fn hz(self) -> u32 {
        self.hz
    }
}

/// One Quad's Radio Link: regular frames, the first at the start, frame `k`
/// at the first physics step at or after `k ÷ Packet Rate` seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioLink {
    packet_rate: PacketRate,
    physics_hz: u64,
    /// The next frame's number.
    next: u64,
    /// The newest Channels to arrive, if any have.
    channels: Option<Channels>,
}

impl RadioLink {
    pub fn new(packet_rate: PacketRate, physics_rate: PhysicsRate) -> RadioLink {
        RadioLink {
            packet_rate,
            physics_hz: u64::from(physics_rate.hz()),
            next: 0,
            channels: None,
        }
    }

    /// A Flight Input's Channels arrived: the next frame carries them.
    pub fn hear(&mut self, channels: Channels) {
        self.channels = Some(channels);
    }

    /// The frame that leaves on the tick at `time`, if one is due. A frame
    /// due between two physics steps leaves on the later one; if two are
    /// due on one step (a Packet Rate faster than the physics), it leaves
    /// once. Before the first Channels arrive, no frame leaves.
    pub fn frame(&mut self, time: SimulationTime) -> Option<Channels> {
        let rate = u64::from(self.packet_rate.hz());
        let due = |k: u64| (k * self.physics_hz).div_ceil(rate);
        let mut sent = false;
        while due(self.next) <= time.ticks() {
            self.next += 1;
            sent = true;
        }
        if sent { self.channels } else { None }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.packet_rate.hz()));
        f.write_u64(self.next);
        match &self.channels {
            None => f.write_u64(0),
            Some(channels) => {
                f.write_u64(1);
                channels.write_fingerprint(f);
            }
        }
    }
}
