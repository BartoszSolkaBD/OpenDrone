//! What the Flight Controller tells each motor's ESC.

use opendrone_maths::Fingerprinter;

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
    /// The ESC's drive, from 0 (stopped) to 1 (full): the share of the
    /// battery's voltage it puts across the motor, as a DShot throttle value
    /// is to Bluejay.
    pub throttle: f64,
    pub direction: SpinDirection,
}

/// The four motor commands, in Betaflight's motor order (1 to 4: rear right,
/// front right, rear left, front left).
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
