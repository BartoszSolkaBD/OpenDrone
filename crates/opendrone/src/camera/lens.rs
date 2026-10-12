//! The Lens and the picture's shape: where the 4:3 picture sits in the
//! window, how a pixel of it maps to a direction the Quad's camera sees, and
//! how wide the flat render underneath has to be to cover it ([ADR-0009]).
//!
//! Angles are in radians inside and in degrees at the edges, as the pilot
//! sees them.
//!
//! [ADR-0009]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0009-fisheye-from-one-warped-render.md

use bevy::math::{UVec2, Vec2};

/// The stock fisheye curve, which Analog uses: the angle from the middle of
/// the picture grows in step with the distance from it.
pub const STOCK_CURVE: f32 = 0.0;

/// The gentler curve Digital uses (#28 §1): closer to a flat lens, so the
/// middle of the picture holds more of the flat render's detail.
pub const GENTLE_CURVE: f32 = 0.35;

/// The picture's aspect ratio, a pilot setting. Presets don't touch it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aspect {
    /// The camera's own picture, 4:3.
    #[default]
    FourThree,
    /// Crops the top and bottom of the 4:3 picture and never stretches it.
    SixteenNine,
}

impl Aspect {
    /// Width over height.
    pub fn ratio(self) -> f32 {
        match self {
            Aspect::FourThree => 4.0 / 3.0,
            Aspect::SixteenNine => 16.0 / 9.0,
        }
    }
}

/// Where the picture sits in the window, in pixels: its top left corner and
/// its size, with the half-diagonal of the camera's own 4:3 frame, which is
/// what FOV is measured across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Half the distance from corner to corner of the 4:3 frame whose width
    /// is the picture's width. A 16:9 picture is that frame with its top and
    /// bottom cropped, so it shares this radius and never stretches.
    pub frame_radius: f32,
}

impl Picture {
    /// The biggest picture of this aspect ratio that fits the window,
    /// centred, with whole pixels.
    pub fn in_window(window: UVec2, aspect: Aspect) -> Picture {
        let (w, h) = (window.x as f32, window.y as f32);
        let ratio = aspect.ratio();
        let width = w.min(h * ratio).floor();
        let height = (width / ratio).floor();
        Picture {
            x: ((w - width) * 0.5).floor(),
            y: ((h - height) * 0.5).floor(),
            width,
            height,
            // The 4:3 frame is `width` wide and `width × 3/4` tall: its
            // half-diagonal is 5/8 of its width.
            frame_radius: 0.625 * width,
        }
    }

    pub fn size(&self) -> UVec2 {
        UVec2::new(self.width as u32, self.height as u32)
    }

    /// The height of the camera's own 4:3 frame, which TV lines are counted
    /// across: 16:9 shows only a crop of it.
    pub fn frame_height(&self) -> f32 {
        self.width * 0.75
    }
}

/// A Quad's Lens at one FOV and curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    /// The angle off the axis that the corner of the 4:3 frame sees: half
    /// the FOV, in radians.
    pub theta_max: f32,
    /// The curve: [`STOCK_CURVE`] or above, more for gentler.
    pub curve: f32,
    /// What the curve makes of `theta_max`, to scale distances to 1 at the
    /// corner.
    pub at_corner: f32,
}

impl Lens {
    /// The Lens for a FOV in degrees, measured corner to corner of the 4:3
    /// picture, like the number on a camera's box.
    pub fn new(fov_degrees: f32, curve: f32) -> Lens {
        let theta_max = (fov_degrees.clamp(60.0, 175.0) * 0.5).to_radians();
        // The curve must keep the corner's angle under 90°.
        let curve = curve.clamp(0.0, (std::f32::consts::FRAC_PI_2 - 0.05) / theta_max);
        let at_corner = if curve > 1e-4 {
            (curve * theta_max).tan() / curve
        } else {
            theta_max
        };
        Lens {
            theta_max,
            curve,
            at_corner,
        }
    }

    /// The angle off the axis seen at a distance from the middle of the
    /// picture, where 1 is the corner of the 4:3 frame.
    pub fn angle_at(&self, distance: f32) -> f32 {
        if self.curve > 1e-4 {
            (distance * self.at_corner * self.curve).atan() / self.curve
        } else {
            distance * self.theta_max
        }
    }

    /// The distance from the middle of the picture at which an angle off the
    /// axis lands, where 1 is the corner of the 4:3 frame.
    pub fn distance_of(&self, angle: f32) -> f32 {
        if self.curve > 1e-4 {
            (self.curve * angle).tan() / self.curve / self.at_corner
        } else {
            angle / self.theta_max
        }
    }

    /// How far, in tangents of the angle off the axis, the flat render must
    /// reach to the right and to the top to cover the picture's corners.
    /// Tangents run out near 90°, so the angle stops just short of it.
    pub fn flat_reach(&self, picture: &Picture) -> Vec2 {
        let half = Vec2::new(picture.width, picture.height) * 0.5 / picture.frame_radius;
        let corner = half.length();
        let angle = self.angle_at(corner).min(1.53);
        half / corner * angle.tan()
    }
}
