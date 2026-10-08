//! Our Flight Controller, plugged into the Flight Controller seam.

use opendrone_flight_controller::{
    Channels, DebugRecord, FlightController, Rates, SensorReadings, SpinDirection as FcDirection,
    Tune,
};
use opendrone_maths::Fingerprinter;
use opendrone_physics::{MotorCommand, MotorCommands, QuadState, SpinDirection};

use crate::{FlightControllerSeam, PhysicsRate, SimulationTime};

/// What the sensors read from a Quad's state: the gyro (its true rotation,
/// with no noise) and the true attitude. The gyro's ±2000 °/s limit arrives
/// with #45.
pub fn sensor_readings(state: &QuadState) -> SensorReadings {
    SensorReadings {
        gyro: state.rotation,
        attitude: state.attitude,
    }
}

/// Our Flight Controller in the seam: one loop per physics step, its DShot
/// commands handed to the ESCs as Bluejay reads them.
#[derive(Clone, Debug)]
pub struct OurFlightController {
    flight_controller: FlightController,
}

impl OurFlightController {
    /// A "fresh" Flight Controller for a Quad starting in `start`, looping
    /// once per physics step, armed or not.
    pub fn new(
        tune: Tune,
        rates: Rates,
        physics_rate: PhysicsRate,
        armed: bool,
        start: &QuadState,
    ) -> OurFlightController {
        OurFlightController {
            flight_controller: FlightController::new(
                tune,
                rates,
                physics_rate.hz(),
                armed,
                &sensor_readings(start),
            ),
        }
    }
}

impl FlightControllerSeam for OurFlightController {
    fn step(
        &mut self,
        _time: SimulationTime,
        readings: &SensorReadings,
        frame: Option<&Channels>,
    ) -> MotorCommands {
        let commands = self.flight_controller.step(readings, frame);
        MotorCommands(commands.map(|command| {
            MotorCommand::from_dshot(
                command.dshot,
                match command.direction {
                    FcDirection::Normal => SpinDirection::Normal,
                    FcDirection::Reversed => SpinDirection::Reversed,
                },
            )
        }))
    }

    fn debug(&self) -> Option<DebugRecord> {
        Some(*self.flight_controller.debug())
    }

    fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.flight_controller.write_fingerprint(f);
    }
}
