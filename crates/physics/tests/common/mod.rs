//! A whoop-sized Quad for the physics' readable checks: the Whoop 65's
//! numbers, typed in SI units so these checks don't need the Pack reader.

#![allow(dead_code)]

use opendrone_maths::{Attitude, DEGREE, Mat3, Vec3};
use opendrone_physics::{
    BatteryParameters, Drag, DuctRings, EscParameters, GroundAndCeiling, MotorParameters, Mount,
    PropDirection, PropParameters, PropWash, QuadParameters, QuadShape, QuadStart, QuadState,
    RotorLayout, StartingMotors, World,
};

pub const WORLD: World = World {
    gravity: 9.81,
    air_density: 1.225,
};

/// One step at 8 kHz, in seconds.
pub const STEP: f64 = 1.0 / 8000.0;

/// RPM per volt in rad/s per volt.
pub const KV: f64 = 2.0 * core::f64::consts::PI / 60.0;

pub fn whoop() -> QuadParameters {
    QuadParameters {
        mass: 0.0312,
        inertia: Mat3::diagonal(Vec3::new(7.0e-6, 9.0e-6, 14.0e-6)),
        drag: Drag {
            body_area: Vec3::ZERO,
            rotor: 0.0,
            duct_ram: 0.0,
            duct_offset: 0.0,
        },
        prop_wash: PropWash::NONE,
        rotors: RotorLayout {
            diagonal: 0.066,
            rotor_height: 0.008,
            direction: PropDirection::PropsIn,
        },
        props: PropParameters {
            diameter: 0.035,
            thrust_coefficient: 0.29,
            power_coefficient: 0.26,
            rotor_inertia: 0.25e-7,
            reverse_thrust: 0.5,
            reverse_torque: 1.0,
            grip: 0.5,
        },
        motors: MotorParameters {
            kv: 19500.0 * KV,
            poles: 12,
            winding_resistance: 0.5,
            no_load_current: 0.3,
            no_load_voltage: 4.0,
            spin_up: 0.035,
            slow_down: 0.035,
        },
        esc: EscParameters {
            start_wait: 0.1,
            startup_power_limit: 0.0196,
            restart_tries: 3,
        },
        battery: BatteryParameters {
            cells: 1,
            capacity: 0.320 * 3600.0,
            voltage_curve: vec![
                (1.0, 4.35),
                (0.8, 4.15),
                (0.5, 3.92),
                (0.2, 3.77),
                (0.0, 3.30),
            ],
            resistance: 0.029,
            connector: 0.010,
            recovery: 3.3,
            slow_sag: 0.0,
        },
        shape: whoop_shape(),
        gyro_range: 2000.0 * DEGREE,
        ground_and_ceiling: WHOOP_GROUND_AND_CEILING,
    }
}

/// The Whoop 65's ground and ceiling effect numbers.
pub const WHOOP_GROUND_AND_CEILING: GroundAndCeiling = GroundAndCeiling {
    ground_effect_body: 2.0,
    ceiling_effect_asymmetry: 1.0,
};

/// The Whoop 65's collision shape, as its Quad definition gives it.
pub fn whoop_shape() -> QuadShape {
    QuadShape {
        body: Vec3::new(0.035, 0.030, 0.020),
        pack: Vec3::new(0.064, 0.010, 0.006),
        pack_height: -0.006,
        diagonal: 0.066,
        rotor_height: 0.008,
        prop_diameter: 0.035,
        duct_rings: Some(DuctRings {
            inside_diameter: 0.037,
            wall: 0.0015,
            height: 0.014,
        }),
        bounce: 0.3,
        friction: 0.5,
    }
}

pub fn still_at(attitude: Attitude, rotation: Vec3) -> QuadState {
    QuadState {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        attitude,
        rotation,
    }
}

/// Level and still, its motors and ESCs started as `motors`, on a full pack.
pub fn start(motors: StartingMotors, mount: Mount) -> QuadStart {
    QuadStart {
        state: still_at(Attitude::BODY_IS_WORLD, Vec3::ZERO),
        motors,
        battery: 1.0,
        mount,
    }
}
