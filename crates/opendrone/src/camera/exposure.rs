//! Auto-exposure, as #28 §4 tuned it: Bevy's own Auto-exposure, kept inside
//! each Video Look's limits by a compensation curve, and metering the view
//! through a mask that weighs each part of the flat render by how much of the
//! picture it becomes.

use bevy::math::{UVec2, Vec2};

use super::lens::{Lens, Picture};
use super::look::AutoExposureLimits;

/// The range of light Auto-exposure meters, in stops of the view's
/// average luminance from Bevy's default exposure.
pub const METERING_RANGE: (f32, f32) = (-14.0, 6.0);

/// How many stops the camera moves the exposure when the view's average is
/// `average` stops from Bevy's default: it aims the average at the limits'
/// target, brightening by up to `max_brighten` and darkening by up to
/// `max_darken`. Positive brightens.
pub fn adaptation(limits: &AutoExposureLimits, average: f32) -> f32 {
    (limits.target - average).clamp(-limits.max_darken, limits.max_brighten)
}

/// The compensation curve that makes Bevy's Auto-exposure follow
/// [`adaptation`]: the exposure it settles on for each average, as four
/// points joined by straight lines, `(average, exposure)`.
pub fn compensation_points(limits: &AutoExposureLimits) -> [Vec2; 4] {
    let (low, high) = METERING_RANGE;
    let (target, brighten, darken) = (limits.target, limits.max_brighten, limits.max_darken);
    [
        Vec2::new(low, low + brighten),
        Vec2::new((target - brighten).max(low + 0.01), target),
        Vec2::new(
            (target + darken)
                .min(high - 0.01)
                .max(target - brighten + 0.02),
            target,
        ),
        Vec2::new(high, high - darken),
    ]
}

/// Where a compensation curve, as joined straight lines, is at an average.
pub fn curve_at(points: &[Vec2], average: f32) -> f32 {
    let first = points[0];
    let last = points[points.len() - 1];
    if average <= first.x {
        return first.y;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if average <= b.x {
            return a.y + (b.y - a.y) * (average - a.x) / (b.x - a.x);
        }
    }
    last.y
}

/// How much of the picture each part of the flat render becomes, from 0 to 1,
/// in a small grid as wide as the render reaches (`reach`, the tangents the
/// flat render covers to the right and to the top). The middle of a fisheye
/// picture holds a wider view than the edges show, so metering through the
/// flat render alone would count the edges too much.
///
/// Returns the grid's size and its weights, row by row, top first, as bytes.
pub fn metering_mask(picture: &Picture, lens: &Lens, reach: Vec2) -> (UVec2, Vec<u8>) {
    const WIDTH: u32 = 128;
    let height = ((WIDTH as f32 * reach.y / reach.x).round() as u32).max(8);
    let half = Vec2::new(picture.width, picture.height) * 0.5 / picture.frame_radius;
    let slope = |angle: f32| {
        if lens.curve > 1e-4 {
            1.0 / (lens.curve * angle).cos().powi(2)
        } else {
            1.0
        }
    };
    let mut weights = vec![0.0f32; (WIDTH * height) as usize];
    let mut heaviest = 0.0f32;
    for y in 0..height {
        for x in 0..WIDTH {
            // The tangents this cell of the flat render looks along.
            let tangent = Vec2::new(
                ((x as f32 + 0.5) / WIDTH as f32 - 0.5) * 2.0 * reach.x,
                ((y as f32 + 0.5) / height as f32 - 0.5) * 2.0 * reach.y,
            );
            let length = tangent.length().max(1e-5);
            let angle = length.atan();
            let distance = lens.distance_of(angle);
            let at = tangent / length * distance;
            if at.x.abs() > half.x || at.y.abs() > half.y {
                continue;
            }
            // Picture area per flat-render area: along the radius, and round it.
            let radial = slope(angle) / lens.at_corner * angle.cos().powi(2);
            let round = distance / length;
            let middle = 1.0 - 0.4 * distance.min(1.0).powi(2);
            let weight = radial * round * middle;
            weights[(y * WIDTH + x) as usize] = weight;
            heaviest = heaviest.max(weight);
        }
    }
    let bytes = weights
        .iter()
        .map(|w| ((w / heaviest.max(1e-9)).clamp(0.0, 1.0) * 255.0) as u8)
        .collect();
    (UVec2::new(WIDTH, height), bytes)
}
