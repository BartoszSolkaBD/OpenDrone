//! Flight Inputs, the one way into the Simulation, and the Radio Link that
//! carries their Channels to the Flight Controller (ADR-0007).
//!
//! The Radio Link sends regular frames at the pilot's Packet Rate, like an
//! ELRS link, each carrying the newest Channels; the Flight Controller sees
//! an ELRS receiver on CRSF. It adds no delay beyond the frame spacing, and
//! sends nothing until the first Channels arrive. While the Flying Input
//! Device is lost it sends nothing at all, and the Flight Controller's
//! Failsafe follows. Locking to an Input Device's report beat (ADR-0020) and
//! Input smoothing come with the Radio Link ticket (#56).

use opendrone_flight_controller::Channels;
use opendrone_maths::Fingerprinter;

use crate::{PhysicsRate, SimulationTime};

/// Anything that enters the Simulation and can change a flight, stamped with
/// the Simulation Time it arrives at: Channels, the Flying Input Device lost
/// or back, and Reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightInput {
    /// The Flying Input Device's Channels, after its Calibration, channel
    /// mapping and deadband, at the whole-number steps a receiver outputs.
    Channels(Channels),
    /// The Flying Input Device was lost: unplugged, or silent for too long.
    /// The Radio Link sends no frames until it is back.
    InputDeviceLost,
    /// The Flying Input Device is back: the Radio Link's next frame carries
    /// the newest Channels.
    InputDeviceBack,
    /// Reset: the Quad back on the Launch Spot, landed and disarmed, powered
    /// up fresh as a new battery does (full charge, a fresh Flight
    /// Controller, ESCs starting up).
    Reset,
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
    /// The Flying Input Device is lost: no frame leaves.
    lost: bool,
}

impl RadioLink {
    pub fn new(packet_rate: PacketRate, physics_rate: PhysicsRate) -> RadioLink {
        RadioLink {
            packet_rate,
            physics_hz: u64::from(physics_rate.hz()),
            next: 0,
            channels: None,
            lost: false,
        }
    }

    /// A Flight Input's Channels arrived: the next frame carries them.
    pub fn hear(&mut self, channels: Channels) {
        self.channels = Some(channels);
    }

    /// The Flying Input Device was lost: no frame leaves until it is back.
    pub fn lose(&mut self) {
        self.lost = true;
    }

    /// The Flying Input Device is back: frames leave again, on the link's own
    /// beat, the next one carrying the newest Channels.
    pub fn back(&mut self) {
        self.lost = false;
    }

    /// True while the Flying Input Device is lost.
    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// The frame that leaves on the tick at `time`, if one is due. A frame
    /// due between two physics steps leaves on the later one; if two are
    /// due on one step (a Packet Rate faster than the physics), it leaves
    /// once. Before the first Channels arrive, and while the Flying Input
    /// Device is lost, no frame leaves: the frames due then are never sent.
    pub fn frame(&mut self, time: SimulationTime) -> Option<Channels> {
        let rate = u64::from(self.packet_rate.hz());
        let due = |k: u64| (k * self.physics_hz).div_ceil(rate);
        let mut sent = false;
        while due(self.next) <= time.ticks() {
            self.next += 1;
            sent = true;
        }
        if sent && !self.lost {
            self.channels
        } else {
            None
        }
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.packet_rate.hz()));
        f.write_u64(self.next);
        f.write_u64(u64::from(self.lost));
        match &self.channels {
            None => f.write_u64(0),
            Some(channels) => {
                f.write_u64(1);
                channels.write_fingerprint(f);
            }
        }
    }
}
