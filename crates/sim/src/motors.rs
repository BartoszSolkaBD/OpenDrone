use opendrone_flight_controller::{Channels, DebugRecord, SensorReadings};
use opendrone_maths::Fingerprinter;
use opendrone_physics::MotorCommands;

use crate::SimulationTime;

/// What plugs into the Flight Controller seam: each tick it gives the four
/// motor commands.
///
/// Three things can plug in (ADR-0003): our Flight Controller
/// ([`OurFlightController`](crate::OurFlightController)), the
/// [`ScriptedMotors`] stand-in for Physics and Thrust Stand Scenarios, and
/// later perhaps a SITL bridge in its own crate.
pub trait FlightControllerSeam {
    /// The motor commands for the tick that starts at `time`, given what the
    /// sensors read at that moment and the Radio Link frame that arrived
    /// then, if one did.
    fn step(
        &mut self,
        time: SimulationTime,
        readings: &SensorReadings,
        frame: Option<&Channels>,
    ) -> MotorCommands;

    /// Reset, a new Map or a new Quad powers it up fresh, as a new battery
    /// does: `readings` are what the sensors read at that moment.
    fn power_up(&mut self, readings: &SensorReadings);

    /// What our Flight Controller's last loop did, for the flight log, the
    /// OSD and Scenarios; `None` for anything else that plugs in.
    fn debug(&self) -> Option<DebugRecord> {
        None
    }

    /// Feeds whatever it remembers into the Simulation's fingerprint.
    fn write_fingerprint(&self, f: &mut Fingerprinter);
}

/// The scripted-motors stand-in for Physics and Thrust Stand Scenarios: motor
/// commands set at given moments, each holding until the next. It reads
/// neither the sensors nor the Radio Link.
#[derive(Clone, Debug)]
pub struct ScriptedMotors {
    timeline: Vec<(SimulationTime, MotorCommands)>,
    next: usize,
    current: MotorCommands,
}

impl ScriptedMotors {
    /// A Timeline of moments and the commands that start then. Before the
    /// first moment every motor is stopped. Moments are taken in time order;
    /// two at the same moment keep their order, so the later one wins.
    pub fn new(mut timeline: Vec<(SimulationTime, MotorCommands)>) -> ScriptedMotors {
        timeline.sort_by_key(|(time, _)| *time);
        ScriptedMotors {
            timeline,
            next: 0,
            current: MotorCommands::STOPPED,
        }
    }

    /// The commands for the tick that starts at `time`.
    pub fn at(&mut self, time: SimulationTime) -> MotorCommands {
        while let Some((at, commands)) = self.timeline.get(self.next) {
            if *at > time {
                break;
            }
            self.current = *commands;
            self.next += 1;
        }
        self.current
    }
}

impl FlightControllerSeam for ScriptedMotors {
    fn step(
        &mut self,
        time: SimulationTime,
        _readings: &SensorReadings,
        _frame: Option<&Channels>,
    ) -> MotorCommands {
        self.at(time)
    }

    /// The stand-in has nothing to power up: its timeline goes on.
    fn power_up(&mut self, _readings: &SensorReadings) {}

    fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.next as u64);
        self.current.write_fingerprint(f);
    }
}
