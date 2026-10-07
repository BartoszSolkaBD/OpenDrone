use core::f64::consts::{FRAC_PI_2, TAU};

use crate::functions::{asin, atan2, fmod, sin_cos};
use crate::{Mat3, Vec3};

/// Which way a Quad points: the turn that takes its body directions (forward,
/// left, up) to world directions (east, north, up).
///
/// It is kept as a unit quaternion, never as three angles, so it has no
/// gimbal lock and turns at any speed without drifting. [`Attitude::turned_by`]
/// uses the exponential map, which is exact for a steady rotation
/// (flight-dynamics research §2.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attitude {
    w: f64,
    x: f64,
    y: f64,
    z: f64,
}

/// An attitude in the words pilots use, in radians.
///
/// - `heading`: where the nose points, as on a compass: 0 is north and a
///   quarter turn (π/2) is east. From 0 up to a full turn.
/// - `pitch`: nose up is positive. From −π/2 to π/2.
/// - `roll`: right side down is positive. From −π to π.
///
/// They are applied in that order: heading, then pitch, then roll, as in
/// aviation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotAngles {
    pub roll: f64,
    pub pitch: f64,
    pub heading: f64,
}

/// How fast a Quad rotates about its own axes, in the words pilots and
/// Betaflight use, in radians per second: rolling right, pitching nose up and
/// yawing nose right are positive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotRates {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
}

impl PilotRates {
    /// The pilot's rates of a rotation given in body axes (forward, left, up).
    pub fn from_body(rotation: Vec3) -> PilotRates {
        // Rolling right turns about the forward axis; pitching nose up turns
        // about the right axis (minus left); yawing nose right turns about the
        // down axis (minus up).
        PilotRates {
            roll: rotation.x,
            pitch: -rotation.y,
            yaw: -rotation.z,
        }
    }

    /// The same rotation in body axes (forward, left, up).
    pub fn to_body(self) -> Vec3 {
        Vec3::new(self.roll, -self.pitch, -self.yaw)
    }
}

impl Attitude {
    /// Body directions are world directions: level, with the nose east.
    pub const BODY_IS_WORLD: Attitude = Attitude {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// The attitude a pilot describes by roll, pitch and heading.
    pub fn from_pilot_angles(angles: PilotAngles) -> Attitude {
        // Heading is measured clockwise from north, and the maths turns
        // anticlockwise from east, so the turn about up is a quarter turn
        // minus the heading. Nose up is a turn about the left axis by minus
        // the pitch. Right side down is a turn about the forward axis.
        let about_up = turn(Vec3::new(0.0, 0.0, 1.0), FRAC_PI_2 - angles.heading);
        let about_left = turn(Vec3::new(0.0, 1.0, 0.0), -angles.pitch);
        let about_forward = turn(Vec3::new(1.0, 0.0, 0.0), angles.roll);
        about_up.then(about_left).then(about_forward).normalised()
    }

    /// The roll, pitch and heading a pilot would read off this attitude.
    pub fn pilot_angles(self) -> PilotAngles {
        let Attitude { w, x, y, z } = self;
        let roll = atan2(2.0 * (w * x + y * z), 1.0 - 2.0 * (x * x + y * y));
        let sine_of_pitch = (2.0 * (w * y - z * x)).clamp(-1.0, 1.0);
        let pitch = -asin(sine_of_pitch);
        let about_up = atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z));
        let mut heading = fmod(FRAC_PI_2 - about_up, TAU);
        if heading < 0.0 {
            heading += TAU;
        }
        if heading >= TAU {
            heading = 0.0;
        }
        PilotAngles {
            roll,
            pitch,
            heading,
        }
    }

    /// This attitude after turning by `rotation` (radians, in body axes): the
    /// exponential map, exact for a rotation that is steady over the turn.
    pub fn turned_by(self, rotation: Vec3) -> Attitude {
        let angle = rotation.length();
        if angle == 0.0 {
            return self;
        }
        self.then(turn(rotation / angle, angle)).normalised()
    }

    /// A body direction (forward, left, up) as a world direction (east,
    /// north, up).
    pub fn body_to_world(self, v: Vec3) -> Vec3 {
        // v + 2w(u × v) + 2u × (u × v), with u the quaternion's vector part.
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }

    /// A world direction (east, north, up) as a body direction (forward,
    /// left, up).
    pub fn world_to_body(self, v: Vec3) -> Vec3 {
        self.inverse().body_to_world(v)
    }

    /// The turn back: world directions to body directions.
    pub fn inverse(self) -> Attitude {
        Attitude {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    /// The same turn as a rotation matrix, whose columns are the body's
    /// forward, left and up directions in the world.
    pub fn to_matrix(self) -> Mat3 {
        let forward = self.body_to_world(Vec3::new(1.0, 0.0, 0.0));
        let left = self.body_to_world(Vec3::new(0.0, 1.0, 0.0));
        let up = self.body_to_world(Vec3::new(0.0, 0.0, 1.0));
        Mat3::from_rows(forward, left, up).transpose()
    }

    /// The quaternion's four numbers, w first.
    pub fn quaternion(self) -> [f64; 4] {
        [self.w, self.x, self.y, self.z]
    }

    /// True when all four numbers are real numbers.
    pub fn is_finite(self) -> bool {
        self.quaternion().iter().all(|n| n.is_finite())
    }

    /// This turn followed by `next`, where `next` is measured in this turn's
    /// body axes (the Hamilton product `self ⊗ next`).
    fn then(self, next: Attitude) -> Attitude {
        let (a, b) = (self, next);
        Attitude {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
            z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        }
    }

    /// Back to length one, so rounding never builds up.
    fn normalised(self) -> Attitude {
        let length = self.quaternion().iter().map(|n| n * n).sum::<f64>().sqrt();
        Attitude {
            w: self.w / length,
            x: self.x / length,
            y: self.y / length,
            z: self.z / length,
        }
    }
}

/// A turn by `angle` radians about `axis`, which must have length one.
fn turn(axis: Vec3, angle: f64) -> Attitude {
    let (sine, cosine) = sin_cos(angle / 2.0);
    Attitude {
        w: cosine,
        x: axis.x * sine,
        y: axis.y * sine,
        z: axis.z * sine,
    }
}
