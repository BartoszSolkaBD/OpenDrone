//! Our Flight Controller, plugged into the Flight Controller seam, with the
//! Auto-arm Assist in front of it.

use opendrone_flight_controller::{
    BatteryReading, Channels, DebugRecord, FlightController, Rates, SensorReadings,
    SpinDirection as FcDirection, Tune,
};
use opendrone_maths::Fingerprinter;
use opendrone_physics::{
    MotorCommand, MotorCommands, QuadBody, QuadState, SpinDirection, gyro_reading,
};

use crate::auto_arm::AutoArm;
use crate::{FlightControllerSeam, PacketRate, PhysicsRate, SimulationTime};

/// What the sensors read from a Quad's state: the gyro (its true rotation,
/// with no noise, each axis clipped at the board's range, `gyro_range` in
/// rad/s, #26 §4) and the true attitude, and whether its ESCs have played
/// their ready beep. With no battery and the motors still: what a fresh
/// Flight Controller reads before its first loop, when it reads only the
/// gyro. Each loop reads the whole board ([`board_readings`]).
pub fn sensor_readings(state: &QuadState, escs_ready: bool, gyro_range: f64) -> SensorReadings {
    SensorReadings::still(
        gyro_reading(state.rotation, gyro_range),
        state.attitude,
        escs_ready,
    )
}

/// Everything the board reads of a Quad each loop: the gyro and attitude as
/// [`sensor_readings`] reads them, whether the ESCs are ready, the battery's
/// voltage and current at its terminals, and each motor's speed in RPM, as
/// its ESC reports it.
pub fn board_readings(body: &QuadBody) -> SensorReadings {
    let battery = body.battery();
    SensorReadings {
        battery: BatteryReading {
            voltage: battery.voltage,
            current: battery.current,
        },
        motor_rpm: body
            .motors()
            .map(|motor| motor.speed * 60.0 / (2.0 * core::f64::consts::PI)),
        ..sensor_readings(
            body.state(),
            body.escs_ready(),
            body.parameters().gyro_range,
        )
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
    /// The Radio Link's Packet Rate, which a fresh Flight Controller has
    /// settled to.
    packet_rate: PacketRate,
    /// Auto-arm, when the pilot has it on.
    auto_arm: Option<AutoArm>,
}

impl OurFlightController {
    /// A "fresh" Flight Controller for a Quad whose sensors read `start`
    /// ([`sensor_readings`]), looping once per physics step, its RC smoothing
    /// and feedforward settled to the Radio Link's `packet_rate`, armed or
    /// not, with Auto-arm on or off.
    pub fn new(
        tune: Tune,
        rates: Rates,
        physics_rate: PhysicsRate,
        packet_rate: PacketRate,
        armed: bool,
        auto_arm: bool,
        start: &SensorReadings,
    ) -> OurFlightController {
        // Only the gyro is read here, for the D term's memory: the ESCs are
        // read every loop.
        OurFlightController {
            flight_controller: FlightController::new(
                tune,
                rates,
                physics_rate.hz(),
                packet_rate.hz(),
                armed,
                start,
            ),
            loop_hz: physics_rate.hz(),
            packet_rate,
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

    /// A fresh Flight Controller, disarmed, with the same Tune and Rates,
    /// settled to the same Packet Rate; Auto-arm waits for a raise from low
    /// again.
    fn power_up(&mut self, readings: &SensorReadings) {
        self.flight_controller = FlightController::new(
            self.flight_controller.tune().clone(),
            self.flight_controller.rates().clone(),
            self.loop_hz,
            self.packet_rate.hz(),
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

    fn clone_box(&self) -> Box<dyn FlightControllerSeam> {
        Box::new(self.clone())
    }
}
