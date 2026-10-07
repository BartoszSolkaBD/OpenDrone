use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Three 64-bit numbers: a position, speed, force or rotation in three
/// dimensions, in world or body axes (see the crate's "Directions").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// All three parts zero.
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    /// The dot product: the parts multiplied in pairs and added, in x, y, z
    /// order.
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// The cross product: at right angles to both, by the right-hand rule.
    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// How long the vector is.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// True when all three parts are real numbers: not "not a number", and
    /// not endless.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, other: Vec3) {
        *self = *self + other;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, other: Vec3) {
        *self = *self - other;
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, scale: f64) -> Vec3 {
        Vec3::new(self.x * scale, self.y * scale, self.z * scale)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, vector: Vec3) -> Vec3 {
        vector * self
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, divisor: f64) -> Vec3 {
        Vec3::new(self.x / divisor, self.y / divisor, self.z / divisor)
    }
}
