//! Our Flight Controller, plugged into the Flight Controller seam, with the
//! Auto-arm Assist in front of it.

use opendrone_flight_controller::{
    Channels, DebugRecord, FlightController, Rates, SensorReadings, SpinDirection as FcDirection,
    Tune,
};
use opendrone_maths::Fingerprinter;
use opendrone_physics::{MotorCommand, MotorCommands, QuadState, SpinDirection};

use crate::auto_arm::AutoArm;
use crate::{FlightControllerSeam, PhysicsRate, SimulationTime};

/// What the sensors read from a Quad's state: the gyro (its true rotation,
/// with no noise) and the true attitude, and whether its ESCs have played
/// their ready beep. The gyro's ±2000 °/s limit arrives with #45.
pub fn sensor_readings(state: &QuadState, escs_ready: bool) -> SensorReadings {
    SensorReadings {
        gyro: state.rotation,
        attitude: state.attitude,
        escs_ready,
    }
}

/// Our Flight Controller in the seam: one loop per physics step, its DShot
/// commands handed to the ESCs as Bluejay reads them. With Auto-arm on, the
/// Assist drives the Arm switch in each Radio Link frame
/// ([`auto_arm`](crate::auto_arm)).
#[derive(Clone, Debug)]
pub struct OurFlightController {
    flight_controller: FlightController,
    loop_hz: u32,
    /// Auto-arm, when the pilot has it on.
    auto_arm: Option<AutoArm>,
}

impl OurFlightController {
    /// A "fresh" Flight Controller for a Quad starting in `start`, looping
    /// once per physics step, armed or not, with Auto-arm on or off.
    pub fn new(
        tune: Tune,
        rates: Rates,
        physics_rate: PhysicsRate,
        armed: bool,
        auto_arm: bool,
        start: &QuadState,
    ) -> OurFlightController {
        // Only the gyro is read here, for the D term's memory: the ESCs are
        // read every loop.
        OurFlightController {
            flight_controller: FlightController::new(
                tune,
                rates,
                physics_rate.hz(),
                armed,
                &sensor_readings(start, false),
            ),
            loop_hz: physics_rate.hz(),
            auto_arm: auto_arm.then(|| AutoArm::new(armed)),
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
        let shaped = match (&mut self.auto_arm, frame) {
            (Some(auto_arm), Some(frame)) => Some(auto_arm.shape(*frame, &self.flight_controller)),
            (_, frame) => frame.copied(),
        };
        let commands = self.flight_controller.step(readings, shaped.as_ref());
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

    /// A fresh Flight Controller, disarmed, with the same Tune and Rates;
    /// Auto-arm waits for a raise from low again.
    fn power_up(&mut self, readings: &SensorReadings) {
        self.flight_controller = FlightController::new(
            self.flight_controller.tune().clone(),
            self.flight_controller.rates().clone(),
            self.loop_hz,
            false,
            readings,
        );
        if self.auto_arm.is_some() {
            self.auto_arm = Some(AutoArm::new(false));
        }
    }

    fn debug(&self) -> Option<DebugRecord> {
        Some(*self.flight_controller.debug())
    }

    fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.flight_controller.write_fingerprint(f);
        match &self.auto_arm {
            None => f.write_u64(0),
            Some(auto_arm) => {
                f.write_u64(1);
                auto_arm.write_fingerprint(f);
            }
        }
    }
}
