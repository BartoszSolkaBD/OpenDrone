//! The Quad's collision shape: simple shapes from its Quad definition (#16
//! §4, #26 §1, ADR-0012).

use core::f64::consts::{FRAC_PI_2, PI};

use opendrone_maths::functions::sin_cos;
use opendrone_maths::{Fingerprinter, Vec3};
use parry3d_f64::math::{Pose, Rot3};
use parry3d_f64::shape::SharedShape;

use crate::geometry::{from_parry, to_parry, turn};

/// The Quad's collision shape and how it bounces and slides, as its Quad
/// definition gives them. Every length is in metres; boxes are given front
/// to back, side to side and top to bottom, in body axes.
///
/// The physics builds these simple shapes from it:
///
/// - **the body**: a box centred on the centre of mass;
/// - **the pack**: a box straight above or below the centre of mass, where
///   the pack really sits;
/// - **a prop disc** per motor, solid for now (Prop Strikes come with #45),
///   [`PROP_DISC_THICKNESS`] thick, in the props' plane;
/// - on a whoop, **a duct ring** round each prop disc, centred on it: a
///   ring open at the top and bottom, made of [`DUCT_RING_SEGMENTS`] flat
///   segments.
///
/// The motors sit at the corners of a square X: each one half the diagonal
/// out from the centre, at 45° between forward and the side, in Betaflight's
/// motor order (1 rear right, 2 front right, 3 rear left, 4 front left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadShape {
    /// The frame and canopy.
    pub body: Vec3,
    /// The battery pack.
    pub pack: Vec3,
    /// How far the pack's centre sits above the centre of mass (below when
    /// negative).
    pub pack_height: f64,
    /// Motor to motor, across the frame.
    pub diagonal: f64,
    /// How far the props' plane sits above the centre of mass (below when
    /// negative).
    pub rotor_height: f64,
    pub prop_diameter: f64,
    /// `None` on a Quad without ducts.
    pub duct_rings: Option<DuctRings>,
    /// How much of the speed into a surface comes back out of it, from 0 (a
    /// dead stop) to 1 (all of it).
    pub bounce: f64,
    /// Coulomb friction between the Quad and the Map: the sideways push a
    /// surface can give, as a share of how hard it pushes back.
    pub friction: f64,
}

/// Each duct's ring, on a whoop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuctRings {
    pub inside_diameter: f64,
    /// How thick the ring's wall is.
    pub wall: f64,
    /// How tall the ring is.
    pub height: f64,
}

/// How thick each prop disc is, in metres. A Quad definition doesn't give a
/// prop's thickness; 2 mm keeps the disc thin, as ADR-0012 asks, until Prop
/// Strikes (#45) decide more.
pub const PROP_DISC_THICKNESS: f64 = 0.002;

/// How many flat segments make up each duct ring. On the alpha whoop's 37 mm
/// ducts, twelve keep the inside within 0.8 mm of a true circle, and never
/// inside it, so the ring always clears its prop disc.
pub const DUCT_RING_SEGMENTS: u32 = 12;

/// Which part of the Quad touched the Map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuadPart {
    Body,
    Pack,
    /// The duct ring round this motor's prop, by Betaflight's motor number (1
    /// to 4).
    DuctRing(u8),
    /// This motor's prop disc, by Betaflight's motor number (1 to 4).
    PropDisc(u8),
}

impl QuadShape {
    /// Feeds every number into a fingerprint, in a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let Vec3 { x, y, z } = self.body;
        f.write_f64s(&[x, y, z]);
        let Vec3 { x, y, z } = self.pack;
        f.write_f64s(&[x, y, z]);
        f.write_f64s(&[
            self.pack_height,
            self.diagonal,
            self.rotor_height,
            self.prop_diameter,
        ]);
        match self.duct_rings {
            None => f.write_u64(0),
            Some(rings) => {
                f.write_u64(1);
                f.write_f64s(&[rings.inside_diameter, rings.wall, rings.height]);
            }
        }
        f.write_f64s(&[self.bounce, self.friction]);
    }

    /// True when every size is above zero, every number is a real number,
    /// bounce is from 0 to 1 and friction is 0 or more.
    pub(crate) fn is_buildable(&self) -> bool {
        let above_zero = |n: f64| n > 0.0 && n.is_finite();
        let sizes = [
            self.body.x,
            self.body.y,
            self.body.z,
            self.pack.x,
            self.pack.y,
            self.pack.z,
            self.diagonal,
            self.prop_diameter,
        ];
        let rings = self.duct_rings.is_none_or(|r| {
            above_zero(r.inside_diameter) && above_zero(r.wall) && above_zero(r.height)
        });
        sizes.into_iter().all(above_zero)
            && rings
            && self.pack_height.is_finite()
            && self.rotor_height.is_finite()
            && (0.0..=1.0).contains(&self.bounce)
            && self.friction >= 0.0
            && self.friction.is_finite()
    }

    /// Where each motor sits, in body axes, in Betaflight's motor order.
    pub(crate) fn motor_positions(&self) -> [Vec3; 4] {
        // Half the diagonal at 45°: the same distance forward (or back) and
        // to the side.
        let a = self.diagonal / 2.0 * core::f64::consts::FRAC_1_SQRT_2;
        let z = self.rotor_height;
        [
            Vec3::new(-a, -a, z),
            Vec3::new(a, -a, z),
            Vec3::new(-a, a, z),
            Vec3::new(a, a, z),
        ]
    }

    /// Every simple shape, in a fixed order: the body, the pack, then for
    /// each motor in order its prop disc and its duct ring's segments.
    pub(crate) fn parts(&self) -> Vec<Part> {
        let level = Rot3::IDENTITY;
        let mut parts = vec![
            Part::new(
                QuadPart::Body,
                cuboid(self.body),
                Pose::from_parts(to_parry(Vec3::ZERO), level),
            ),
            Part::new(
                QuadPart::Pack,
                cuboid(self.pack),
                Pose::from_parts(to_parry(Vec3::new(0.0, 0.0, self.pack_height)), level),
            ),
        ];
        // parry3d's cylinders stand along their own y axis; a quarter turn
        // about forward lays a prop disc flat in the props' plane.
        let flat = turn(Vec3::new(1.0, 0.0, 0.0), FRAC_PI_2);
        let disc = SharedShape::cylinder(PROP_DISC_THICKNESS / 2.0, self.prop_diameter / 2.0);
        for (motor, centre) in (1u8..).zip(self.motor_positions()) {
            parts.push(Part::new(
                QuadPart::PropDisc(motor),
                disc.clone(),
                Pose::from_parts(to_parry(centre), flat),
            ));
            if let Some(rings) = self.duct_rings {
                ring_segments(rings, centre, motor, &mut parts);
            }
        }
        parts
    }
}

/// One duct ring as flat segments round `centre`: each a box as thick as the
/// wall and as tall as the ring, long enough to close the ring on the outside
/// (so nothing slips between two segments), with its inside face touching the
/// ring's inside circle at its middle.
fn ring_segments(rings: DuctRings, centre: Vec3, motor: u8, parts: &mut Vec<Part>) {
    let inside = rings.inside_diameter / 2.0;
    let outside = inside + rings.wall;
    let middle = inside + rings.wall / 2.0;
    let step = 2.0 * PI / f64::from(DUCT_RING_SEGMENTS);
    let (half_step_sine, half_step_cosine) = sin_cos(step / 2.0);
    let length = 2.0 * outside * half_step_sine / half_step_cosine;
    let segment = cuboid(Vec3::new(rings.wall, length, rings.height));
    for k in 0..DUCT_RING_SEGMENTS {
        let angle = step * f64::from(k);
        let (sine, cosine) = sin_cos(angle);
        let at = centre + Vec3::new(middle * cosine, middle * sine, 0.0);
        parts.push(Part::new(
            QuadPart::DuctRing(motor),
            segment.clone(),
            Pose::from_parts(to_parry(at), turn(Vec3::new(0.0, 0.0, 1.0), angle)),
        ));
    }
}

/// A box of this size (not half-size).
fn cuboid(size: Vec3) -> SharedShape {
    SharedShape::cuboid(size.x / 2.0, size.y / 2.0, size.z / 2.0)
}

/// One simple shape of the Quad, placed in body axes from the centre of mass.
#[derive(Clone, Debug)]
pub(crate) struct Part {
    pub part: QuadPart,
    pub shape: SharedShape,
    /// Where it sits and which way it points, in body axes, from the centre
    /// of mass.
    pub local: Pose,
    /// How far its farthest point is from the centre of mass.
    pub reach: f64,
}

impl Part {
    fn new(part: QuadPart, shape: SharedShape, local: Pose) -> Part {
        let sphere = shape.compute_local_bounding_sphere();
        let centre = from_parry(local.transform_point(sphere.center()));
        Part {
            part,
            reach: centre.length() + sphere.radius(),
            shape,
            local,
        }
    }
}
