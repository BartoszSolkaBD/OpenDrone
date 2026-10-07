use core::ops::Mul;

use crate::Vec3;

/// A 3 × 3 matrix of 64-bit numbers, stored as three rows. A Quad's inertia
/// is one, in body axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub rows: [Vec3; 3],
}

impl Mat3 {
    /// The matrix that changes nothing.
    pub const IDENTITY: Mat3 = Mat3::diagonal(Vec3::new(1.0, 1.0, 1.0));

    pub const fn from_rows(x: Vec3, y: Vec3, z: Vec3) -> Mat3 {
        Mat3 { rows: [x, y, z] }
    }

    /// A matrix with these numbers down its diagonal and zero elsewhere, such
    /// as the inertia of a Quad whose body axes are its principal axes.
    pub const fn diagonal(d: Vec3) -> Mat3 {
        Mat3::from_rows(
            Vec3::new(d.x, 0.0, 0.0),
            Vec3::new(0.0, d.y, 0.0),
            Vec3::new(0.0, 0.0, d.z),
        )
    }

    /// The matrix's columns, as rows.
    pub fn transpose(self) -> Mat3 {
        let [a, b, c] = self.rows;
        Mat3::from_rows(
            Vec3::new(a.x, b.x, c.x),
            Vec3::new(a.y, b.y, c.y),
            Vec3::new(a.z, b.z, c.z),
        )
    }

    pub fn determinant(self) -> f64 {
        let [a, b, c] = self.rows;
        a.dot(b.cross(c))
    }

    /// The matrix that undoes this one, or `None` when there isn't one
    /// (a determinant of zero, or broken numbers).
    pub fn inverse(self) -> Option<Mat3> {
        let [a, b, c] = self.rows;
        let determinant = self.determinant();
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }
        // The columns of the inverse are the cross products of pairs of rows,
        // divided by the determinant.
        let columns = Mat3::from_rows(
            b.cross(c) / determinant,
            c.cross(a) / determinant,
            a.cross(b) / determinant,
        );
        Some(columns.transpose())
    }

    /// Every number in the matrix, row by row.
    pub fn numbers(self) -> [f64; 9] {
        let [a, b, c] = self.rows;
        [a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z]
    }
}

impl Mul<Vec3> for Mat3 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        let [a, b, c] = self.rows;
        Vec3::new(a.dot(v), b.dot(v), c.dot(v))
    }
}

impl Mul for Mat3 {
    type Output = Mat3;
    fn mul(self, other: Mat3) -> Mat3 {
        let columns = other.transpose();
        let [a, b, c] = self.rows;
        Mat3::from_rows(columns * a, columns * b, columns * c)
    }
}
