//! The only place where OpenDrone's number types and parry3d's meet
//! ([ADR-0004]): every conversion to and from parry3d's types happens here,
//! and nothing outside `opendrone-physics` ever sees one of them.
//!
//! parry3d answers geometry questions only: what touches what, where, and
//! when a moving shape first meets another. The bounce, the slide and every
//! number that moves a Quad stay ours.
//!
//! [ADR-0004]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0004-parry3d-geometry-only-f64.md

use opendrone_maths::functions::sin_cos;
use opendrone_maths::{Attitude, Vec3};
use parry3d_f64::math::{Pose, Rot3, Vector};

/// Our vector as parry3d's.
pub(crate) fn to_parry(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}

/// parry3d's vector as ours.
pub(crate) fn from_parry(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Our attitude as parry3d's rotation: the same unit quaternion.
pub(crate) fn rotation(attitude: Attitude) -> Rot3 {
    let [w, x, y, z] = attitude.quaternion();
    Rot3::from_xyzw(x, y, z, w)
}

/// A position and an attitude as parry3d's pose.
pub(crate) fn pose(position: Vec3, attitude: Attitude) -> Pose {
    Pose::from_parts(to_parry(position), rotation(attitude))
}

/// A turn of `angle` radians about `axis` (which must have length one), with
/// the sine and cosine from `libm`, so the same on every computer.
pub(crate) fn turn(axis: Vec3, angle: f64) -> Rot3 {
    let (sine, cosine) = sin_cos(angle / 2.0);
    Rot3::from_xyzw(axis.x * sine, axis.y * sine, axis.z * sine, cosine)
}
