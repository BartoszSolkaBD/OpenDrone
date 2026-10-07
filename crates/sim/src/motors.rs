use opendrone_maths::Fingerprinter;

use crate::SimulationTime;

/// Which way a motor is told to spin. Crash Flip spins motors backwards
/// (ADR-0012), so every motor command carries its direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinDirection {
    Normal,
    Reversed,
}

/// What the Flight Controller tells one motor's ESC.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorCommand {
    /// From 0 (stopped) to 1 (full).
    pub throttle: f64,
    pub direction: SpinDirection,
}

/// The four motor commands, in Betaflight's motor order (1 to 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorCommands(pub [MotorCommand; 4]);

impl MotorCommands {
    /// Every motor stopped, spinning the normal way.
    pub const STOPPED: MotorCommands = MotorCommands::all(0.0);

    /// Every motor at the same throttle, spinning the normal way.
    pub const fn all(throttle: f64) -> MotorCommands {
        let command = MotorCommand {
            throttle,
            direction: SpinDirection::Normal,
        };
        MotorCommands([command; 4])
    }

    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|command| command.throttle.is_finite())
    }

    /// Feeds every command into a fingerprint, in motor order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        for command in self.0 {
            f.write_f64(command.throttle);
            f.write_u64(match command.direction {
                SpinDirection::Normal => 0,
                SpinDirection::Reversed => 1,
            });
        }
    }
}

/// What plugs into the Flight Controller seam: each tick it gives the four
/// motor commands.
///
/// Our Flight Controller (#48) will plug in here and receive sensor readings
/// and Channels; for now the only thing that plugs in is [`ScriptedMotors`].
pub trait FlightControllerSeam {
    /// The motor commands for the tick that starts at `time`.
    fn step(&mut self, time: SimulationTime) -> MotorCommands;

    /// Feeds whatever it remembers into the Simulation's fingerprint.
    fn write_fingerprint(&self, f: &mut Fingerprinter);
}

/// The scripted-motors stand-in for Physics and Thrust Stand Scenarios: motor
/// commands set at given moments, each holding until the next.
#[derive(Clone, Debug)]
pub struct ScriptedMotors {
    script: Vec<(SimulationTime, MotorCommands)>,
    next: usize,
    current: MotorCommands,
}

impl ScriptedMotors {
    /// A script of moments and the commands that start then. Before the first
    /// moment every motor is stopped. Moments are taken in time order; two
    /// at the same moment keep their order, so the later one wins.
    pub fn new(mut script: Vec<(SimulationTime, MotorCommands)>) -> ScriptedMotors {
        script.sort_by_key(|(time, _)| *time);
        ScriptedMotors {
            script,
            next: 0,
            current: MotorCommands::STOPPED,
        }
    }
}

impl FlightControllerSeam for ScriptedMotors {
    fn step(&mut self, time: SimulationTime) -> MotorCommands {
        while let Some((at, commands)) = self.script.get(self.next) {
            if *at > time {
                break;
            }
            self.current = *commands;
            self.next += 1;
        }
        self.current
    }

    fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.next as u64);
        self.current.write_fingerprint(f);
    }
}
