//! Flight Inputs, the one way into the Simulation, and the Radio Link that
//! carries their Channels to the Flight Controller (ADR-0007, ADR-0020).
//!
//! The Radio Link sends frames at the pilot's Packet Rate, like an ELRS
//! link, each carrying the newest Channels; the Flight Controller sees an
//! ELRS receiver on CRSF. It adds no delay beyond the frame spacing, and
//! sends nothing until the first Channels arrive. When its frames leave:
//!
//! - **On the device's report beat** (ADR-0020), when the Flying Input
//!   Device's Report Rate is known and the Packet Rate divides it evenly
//!   ([`PacketRate::reports_per_frame`]). The link learns the beat from
//!   the moments its Channels arrive, the Flight Inputs' own stamps, so a
//!   replay learns it exactly the same way: a report is due where the
//!   earliest of the last second's reports landed, counted on whole report
//!   periods, and each frame leaves a fixed margin ([`LOCK_MARGIN_MICROS`],
//!   0.75 ms) after its report is due. Every frame then carries exactly one
//!   fresh report, always about the same age. Between reports the beat
//!   carries on by itself. A report that arrives after its frame has left
//!   costs one repeated frame, like one lost ELRS packet.
//! - **On its own clock** otherwise: frame `k` at the first physics step at
//!   or after `k ÷ Packet Rate` seconds from the start. Scenarios with
//!   scripted sticks have no device, so they always get these plain regular
//!   frames.
//!
//! While the Flying Input Device is lost it sends nothing at all, and the
//! Flight Controller's Failsafe follows. A device is lost when the computer
//! reports it removed ([`FlightInput::InputDeviceLost`]), and, if it keeps
//! reporting while its sticks rest, after [`SILENT_FOR_SECONDS`] (1 s)
//! without a report (#27). A device that reports only changes is never lost
//! just because it goes quiet.

use std::collections::VecDeque;

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
    /// They arrive whenever a value changes, and, from a device that reports
    /// at rest, with every report it sends, changed or not: that is how the
    /// Radio Link hears its beat and knows it is still there.
    Channels(Channels),
    /// The Flying Input Device was lost: the computer reports it removed.
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

    /// How many of a device's reports one frame spans, when this Packet Rate
    /// divides the Report Rate evenly: the Report Rate ÷ the Packet Rate, a
    /// whole number of 1 or more. `None` when it doesn't divide it, or is
    /// faster than it, and the link then runs on its own clock.
    ///
    /// 333 Hz counts as a frame every 3 ms, three reports of a 1 kHz device,
    /// as ADR-0020 lists it: the DualSense over USB (250 Hz) locks at 250 and
    /// 50 Hz, and the Pocket (1 kHz) at 1000, 500, 333, 250, 100 and 50 Hz.
    pub fn reports_per_frame(self, report_rate: ReportRate) -> Option<u32> {
        let report_hz = report_rate.hz();
        if report_hz.is_multiple_of(self.hz) {
            return Some(report_hz / self.hz);
        }
        // 333 Hz is 1000 ÷ 3 Hz.
        let thirds = u64::from(report_hz) * 3;
        (self.hz == 333 && thirds.is_multiple_of(1000))
            .then(|| u32::try_from(thirds / 1000).unwrap_or(u32::MAX))
    }
}

/// How many times a second an Input Device sends its values to the
/// computer, on a beat of its own: 250 Hz for the DualSense over USB,
/// 1000 Hz for the Radiomaster Pocket with its RF module off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReportRate {
    hz: u32,
}

impl ReportRate {
    /// A Report Rate of `hz` reports a second, or `None` for zero.
    pub fn from_hz(hz: u32) -> Option<ReportRate> {
        (hz > 0).then_some(ReportRate { hz })
    }

    pub fn hz(self) -> u32 {
        self.hz
    }
}

/// The Flying Input Device's facts, which come with the set-up from its
/// Input Device profile. A Quad whose sticks are scripted, as in a Scenario's
/// Timeline, has no device, and its Radio Link sends plain regular frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputDeviceFacts {
    /// How often it reports, when known. The Radio Link locks to its beat
    /// when the Packet Rate divides it evenly.
    pub report_rate: Option<ReportRate>,
    /// Whether it keeps reporting while its sticks rest, so that 1 s without
    /// a report counts as lost. The caller says so only when the device
    /// really does: a DualSense whose motion sensors didn't start reports
    /// only changes, and is lost only when unplugged.
    pub reports_at_rest: bool,
}

/// How long after a report is due a locked frame leaves: just past the
/// arrival wobble measured on the dev Mac, 0–0.5 ms (ADR-0020).
pub const LOCK_MARGIN_MICROS: u64 = 750;

/// How long a device that reports at rest may say nothing before it counts
/// as lost, exactly as if it were unplugged (#27).
pub const SILENT_FOR_SECONDS: u64 = 1;

/// One frame as it leaves the Radio Link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The newest Channels.
    pub channels: Channels,
    /// How many of the device's reports arrived since the last frame left:
    /// the fresh ones it carries, the newest winning. 0 means it repeats the
    /// last frame's Channels. On the device's beat, Channels that the link
    /// tells apart as the same report count once: a Radio's report can reach
    /// the input thread in two polls, with some of its values in each (#30's
    /// Pocket recordings have changes 0.35 ms apart, on a 1 ms beat). On the
    /// link's own clock every Channels Flight Input counts.
    pub reports: u32,
    /// How long before the frame left its newest Channels arrived, in
    /// physics steps.
    pub age: u64,
}

/// One Quad's Radio Link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioLink {
    packet_rate: PacketRate,
    physics_hz: u64,
    reports_at_rest: bool,
    clock: Clock,
    /// The newest Channels to arrive, if any have, and the step they arrived
    /// at.
    channels: Option<(Channels, u64)>,
    /// Reports heard since the last frame left.
    fresh: u32,
    /// On the beat, the report the last Channels since the last frame came
    /// with, counted from the first report.
    fresh_report: Option<i64>,
    /// When the device last reported, or came back, for the silence rule.
    last_heard: Option<u64>,
    /// The computer reported the Flying Input Device removed.
    unplugged: bool,
    /// It reports at rest, and has said nothing for a second.
    silent: bool,
}

/// When the frames leave.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Clock {
    /// The link's own clock: the next frame's number.
    Own { next: u64 },
    /// The device's report beat.
    Beat(Beat),
}

/// The device's report beat, as the link learns it.
///
/// Times here are counted in beat units, physics steps × the Report Rate, so
/// that one report period is a whole number of them (the physics rate) and
/// every sum is exact.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Beat {
    reports_per_frame: i64,
    report_hz: i64,
    /// One report period, in beat units.
    period: i64,
    /// [`LOCK_MARGIN_MICROS`], in beat units, rounded up.
    margin: i64,
    /// Nothing until the first report arrives.
    learnt: Option<Learnt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Learnt {
    /// Where report 0's arrivals cluster, in beat units: each report is told
    /// apart by the whole report periods between it and this.
    centre: i64,
    /// The last second's reports, each as the moment report 0 would have
    /// been due had this one arrived the moment it was due, with the step
    /// it arrived at. Only the earliest and those after it that might
    /// become the earliest are kept, earliest first.
    earliest: VecDeque<(u64, i64)>,
    /// The report whose frame leaves next.
    next_report: i64,
}

/// How far the centre moves towards each report: a sixteenth of the way.
const CENTRE_STEPS: i64 = 16;

impl Beat {
    fn new(reports_per_frame: u32, report_rate: ReportRate, physics_hz: u64) -> Beat {
        let report_hz = u64::from(report_rate.hz());
        // 0.75 ms × physics rate × Report Rate, rounded up.
        let margin = (LOCK_MARGIN_MICROS * physics_hz * report_hz).div_ceil(1_000_000);
        Beat {
            reports_per_frame: i64::from(reports_per_frame),
            report_hz: whole(report_hz),
            period: whole(physics_hz),
            margin: whole(margin),
            learnt: None,
        }
    }

    /// A report arrived at step `tick`: which one it is, counted from the
    /// first.
    fn hear(&mut self, tick: u64, physics_hz: u64) -> i64 {
        let at = whole(tick) * self.report_hz;
        let Some(learnt) = &mut self.learnt else {
            self.learnt = Some(Learnt {
                centre: at,
                earliest: VecDeque::from([(tick, at)]),
                next_report: 0,
            });
            return 0;
        };
        // The whole report periods from the centre, to the nearest.
        let periods = (at - learnt.centre + self.period / 2).div_euclid(self.period);
        let due = at - periods * self.period;
        learnt.centre += (due - learnt.centre) / CENTRE_STEPS;
        // Reports from over a second ago are forgotten, and so are any that
        // now lie more than half a period before the centre: early on, when
        // the centre starts from the first report rather than the middle of
        // the wobble, a report that came late can be counted against the
        // next report, as if it were early.
        let too_early = learnt.centre - self.period / 2;
        while learnt
            .earliest
            .front()
            .is_some_and(|(heard, due)| heard + physics_hz <= tick || *due < too_early)
        {
            learnt.earliest.pop_front();
        }
        while learnt.earliest.back().is_some_and(|(_, d)| *d >= due) {
            learnt.earliest.pop_back();
        }
        learnt.earliest.push_back((tick, due));
        periods
    }

    /// The step report `report`'s frame leaves on: the first at or after the
    /// margin past the moment it is due.
    fn frame_step(&self, learnt: &Learnt, report: i64) -> i64 {
        let due = learnt
            .earliest
            .front()
            .map_or(learnt.centre, |(_, due)| *due);
        let leaves = due + report * self.period + self.margin;
        // Rounded up to a whole step.
        -(-leaves).div_euclid(self.report_hz)
    }

    /// Whether a frame is due on the step at `now`. Frames due on earlier
    /// steps that never left are passed over, so at most one leaves.
    fn due(&mut self, now: u64) -> bool {
        let Some(learnt) = &self.learnt else {
            return false;
        };
        let mut next = learnt.next_report;
        let mut due = false;
        while self.frame_step(learnt, next) <= whole(now) {
            next += self.reports_per_frame;
            due = true;
        }
        if let Some(learnt) = &mut self.learnt {
            learnt.next_report = next;
        }
        due
    }

    fn write_fingerprint(&self, f: &mut Fingerprinter) {
        match &self.learnt {
            None => f.write_u64(0),
            Some(learnt) => {
                f.write_u64(1);
                f.write_u64(learnt.centre as u64);
                f.write_u64(learnt.next_report as u64);
                f.write_u64(learnt.earliest.len() as u64);
                for (heard, due) in &learnt.earliest {
                    f.write_u64(*heard);
                    f.write_u64(*due as u64);
                }
            }
        }
    }
}

/// A count of steps as a signed number for the beat's sums. A Simulation
/// would need to run for longer than the universe has existed to pass it.
fn whole(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl RadioLink {
    /// A Radio Link at the pilot's Packet Rate, for a Quad flown by an Input
    /// Device with these facts, or by scripted sticks (`None`).
    pub fn new(
        packet_rate: PacketRate,
        physics_rate: PhysicsRate,
        device: Option<InputDeviceFacts>,
    ) -> RadioLink {
        let physics_hz = u64::from(physics_rate.hz());
        let beat = device
            .and_then(|d| d.report_rate)
            .and_then(|r| Some(Beat::new(packet_rate.reports_per_frame(r)?, r, physics_hz)));
        RadioLink {
            packet_rate,
            physics_hz,
            reports_at_rest: device.is_some_and(|d| d.reports_at_rest),
            clock: match beat {
                Some(beat) => Clock::Beat(beat),
                None => Clock::Own { next: 0 },
            },
            channels: None,
            fresh: 0,
            fresh_report: None,
            last_heard: None,
            unplugged: false,
            silent: false,
        }
    }

    /// A report's Channels arrived, stamped `at`: the next frame carries
    /// them. On the device's beat, the link learns from when it arrived.
    pub fn hear(&mut self, at: SimulationTime, channels: Channels) {
        let tick = at.ticks();
        self.channels = Some((channels, tick));
        self.last_heard = Some(tick);
        self.silent = false;
        let report = match &mut self.clock {
            Clock::Beat(beat) => Some(beat.hear(tick, self.physics_hz)),
            Clock::Own { .. } => None,
        };
        if report.is_none() || report != self.fresh_report {
            self.fresh = self.fresh.saturating_add(1);
        }
        self.fresh_report = report;
    }

    /// The computer reported the Flying Input Device removed: no frame leaves
    /// until it is back.
    pub fn lose(&mut self, _at: SimulationTime) {
        self.unplugged = true;
    }

    /// The Flying Input Device is back: frames leave again, on the link's
    /// beat, the next one carrying the newest Channels. A device that reports
    /// at rest has a second from now to say something.
    pub fn back(&mut self, at: SimulationTime) {
        self.unplugged = false;
        self.last_heard = Some(at.ticks());
        self.silent = false;
    }

    /// True while the Flying Input Device counts as lost: unplugged, or
    /// silent for too long.
    pub fn is_lost(&self) -> bool {
        self.unplugged || self.silent
    }

    /// True when the frames follow the device's report beat (ADR-0020).
    pub fn is_locked(&self) -> bool {
        matches!(self.clock, Clock::Beat(_))
    }

    /// The frame that leaves on the tick at `time`, if one is due. A frame
    /// due between two physics steps leaves on the later one; if two are due
    /// on one step (a Packet Rate faster than the physics), it leaves once.
    /// Before the first Channels arrive, and while the Flying Input Device is
    /// lost, no frame leaves: the frames due then are never sent. Call it
    /// once a tick, after the tick's Flight Inputs have arrived.
    pub fn frame(&mut self, time: SimulationTime) -> Option<Frame> {
        let now = time.ticks();
        if self.reports_at_rest
            && !self.unplugged
            && self
                .last_heard
                .is_some_and(|heard| now >= heard + SILENT_FOR_SECONDS * self.physics_hz)
        {
            self.silent = true;
        }
        let due = match &mut self.clock {
            Clock::Own { next } => {
                let rate = u64::from(self.packet_rate.hz());
                let physics_hz = self.physics_hz;
                let due = |k: u64| (k * physics_hz).div_ceil(rate);
                let mut sent = false;
                while due(*next) <= now {
                    *next += 1;
                    sent = true;
                }
                sent
            }
            Clock::Beat(beat) => beat.due(now),
        };
        let (channels, heard) = self.channels?;
        if !due || self.is_lost() {
            return None;
        }
        let frame = Frame {
            channels,
            reports: self.fresh,
            age: now.saturating_sub(heard),
        };
        self.fresh = 0;
        self.fresh_report = None;
        Some(frame)
    }

    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(u64::from(self.packet_rate.hz()));
        f.write_u64(u64::from(self.reports_at_rest));
        match &self.clock {
            Clock::Own { next } => {
                f.write_u64(0);
                f.write_u64(*next);
            }
            Clock::Beat(beat) => {
                f.write_u64(1);
                beat.write_fingerprint(f);
            }
        }
        match &self.channels {
            None => f.write_u64(0),
            Some((channels, heard)) => {
                f.write_u64(1);
                channels.write_fingerprint(f);
                f.write_u64(*heard);
            }
        }
        f.write_u64(u64::from(self.fresh));
        match self.fresh_report {
            None => f.write_u64(0),
            Some(report) => {
                f.write_u64(1);
                f.write_u64(report as u64);
            }
        }
        f.write_u64(self.last_heard.unwrap_or(u64::MAX));
        f.write_u64(u64::from(self.unplugged));
        f.write_u64(u64::from(self.silent));
    }
}
