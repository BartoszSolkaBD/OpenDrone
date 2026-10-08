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

impl MotorCommand {
    /// What Bluejay v0.21.0 makes of a DShot throttle value from the Flight
    /// Controller (`Isrs.asm` L196–L290, reimplemented): 0 to 47 are "stop"
    /// and DShot's special commands, so no drive; 48 to 2047 give 0 to 1999,
    /// which Bluejay stretches to its 2048 steps of power by adding a
    /// fortieth (in whole numbers: twice the value, ÷ 16, ÷ 5), held at 2047,
    /// its full power.
    pub fn from_dshot(value: u16, direction: SpinDirection) -> MotorCommand {
        let throttle = if value < 48 {
            0.0
        } else {
            let x = u32::from(value.min(2047) - 48);
            let power = (x + (2 * x / 16) / 5).min(2047);
            f64::from(power) / 2047.0
        };
        MotorCommand {
            throttle,
            direction,
        }
    }
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
