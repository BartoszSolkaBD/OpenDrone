//! Where the four rotors sit on the frame, and which way each prop turns.

use opendrone_maths::Vec3;

/// Which way the props turn, seen from above, as Betaflight's
/// `yaw_motors_reversed` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropDirection {
    /// Betaflight's default, `yaw_motors_reversed = OFF`: the front props'
    /// leading edges sweep in over the nose. Motors 2 (front right) and 3
    /// (rear left) turn anticlockwise, 1 (rear right) and 4 (front left)
    /// clockwise.
    PropsIn,
    /// `yaw_motors_reversed = ON`: every prop turns the other way.
    PropsOut,
}

/// The rotors' layout: an X frame with its motors on the diagonals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorLayout {
    /// Motor to motor, across the frame, in metres.
    pub diagonal: f64,
    /// How far the props' plane sits above the centre of mass (below when
    /// negative), in metres.
    pub rotor_height: f64,
    pub direction: PropDirection,
}

impl RotorLayout {
    /// Each rotor's hub from the centre of mass, in body axes (forward, left,
    /// up), in Betaflight's motor order: rear right, front right, rear left,
    /// front left.
    pub fn positions(&self) -> [Vec3; 4] {
        // Each motor is half the diagonal from the middle, at 45° to the
        // axes: forward and sideways by half the diagonal over √2.
        let a = self.diagonal / (2.0 * core::f64::consts::SQRT_2);
        let h = self.rotor_height;
        [
            Vec3::new(-a, -a, h),
            Vec3::new(a, -a, h),
            Vec3::new(-a, a, h),
            Vec3::new(a, a, h),
        ]
    }

    /// Which way each prop turns when its motor spins the normal way, in
    /// motor order: +1 anticlockwise seen from above (turning about the body's
    /// up axis), -1 clockwise.
    pub fn turning(&self) -> [f64; 4] {
        let props_in = [-1.0, 1.0, 1.0, -1.0];
        match self.direction {
            PropDirection::PropsIn => props_in,
            PropDirection::PropsOut => props_in.map(|t| -t),
        }
    }
}
