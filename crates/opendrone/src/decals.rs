//! Decals and two materials on one mesh (#321): the numbers the shader
//! (`repeat_breaker.wesl`) draws them with, here so the readable checks can
//! hold them to what they should do. Keep the two in step.
//!
//! - **A blend.** A part of a Map whose `.glb` names a second material
//!   (`od_blend`) carries a weight at each corner (its vertex colour), from 0
//!   (its own material) to 1 (the second). The game hands the weight to the
//!   shader as the alpha of the mesh's vertex colour, with the colour itself
//!   white, so the standard material's own use of a vertex colour (a tint)
//!   changes nothing. The shader pushes the weight about by a noise in the
//!   middle of the edge, then lets the lighter parts of either picture (a
//!   stone, a tuft) show first: a height blend ([`blend_weight`]). So the edge
//!   is never the straight line between two corners, and dirt comes through
//!   grass at its gaps.
//! - **A decal.** A part tagged `od_decal` is a decal: its material's one
//!   picture and its mask, drawn over the surface under it with premultiplied
//!   alpha ([`decal_over`]), a hair above it, after the Map's solid parts. Its
//!   layer orders overlapping decals: a higher layer is drawn later, so it
//!   lies on top, whatever the camera does ([`decal_depth_bias`]). It casts no
//!   shadow, and a baked Map lights it with its atlas, at the coordinates the
//!   Map's script copied from the surface under it.
//!
//! Nothing here touches the Simulation.

/// How far the noise pushes a blend's weight, at most, in the middle of the
/// edge (where the weight is a half); none where it is 0 or 1.
pub const EDGE_WANDER: f32 = 0.5;

/// The two sizes of the edge's noise, in metres: a wander of half a metre,
/// and a finer one of 16 cm.
pub const EDGE_NOISE_COARSE: f32 = 0.55;
pub const EDGE_NOISE_FINE: f32 = 0.16;

/// How much the pictures' light and dark (as height) count against the
/// weight, and how wide the blend between the two is, in the same units.
pub const HEIGHT_GAIN: f32 = 3.0;
pub const BLEND_DEPTH: f32 = 0.2;

/// Below this a decal's mask is nothing: the noise its JPG leaves round the
/// mask's edge would otherwise show as a faint rectangle.
pub const MASK_FLOOR: f32 = 0.03;

/// The roughness of the film of water a wet decal puts over the surface under
/// it: smooth, as standing water is.
pub const FILM_ROUGHNESS: f32 = 0.2;

/// How much of the sky the film shows: a damp surface mostly darkens, and a
/// puddle's mirror is more than a decal can give.
pub const FILM_SHEEN: f32 = 0.35;

/// How far apart, in the order transparent parts are drawn in, one decal
/// layer is from the next. Bevy sorts them by their distance from the camera
/// plus this, so a step far bigger than any two overlapping decals' distance
/// apart puts a higher layer after a lower one from everywhere. It is also a
/// depth bias of that many of the depth buffer's smallest steps toward the
/// camera, which at 40 m is under half a millimetre a step.
pub const DECAL_LAYER_STEP: f32 = 100.0;

/// The depth bias a decal of `layer` (0 to 15) is drawn with: one step for
/// layer 0, so even it is drawn after other see-through parts at its place.
pub fn decal_depth_bias(layer: u8) -> f32 {
    (f32::from(layer) + 1.0) * DECAL_LAYER_STEP
}

/// How much of the second material shows, from the corner weight `w` (0 to
/// 1), the edge's noise there (`noise`, 0 to 1) and the two pictures' height
/// there: each one's luminance less its average (a little above 0 for a
/// light stone, below for a dark gap).
pub fn blend_weight(w: f32, noise: f32, height_a: f32, height_b: f32) -> f32 {
    let w = w.clamp(0.0, 1.0);
    let pushed = (w + (noise * 2.0 - 1.0) * EDGE_WANDER * 4.0 * w * (1.0 - w)).clamp(0.0, 1.0);
    let level_a = (1.0 - pushed) + height_a * HEIGHT_GAIN;
    let level_b = pushed + height_b * HEIGHT_GAIN;
    let top = level_a.max(level_b) - BLEND_DEPTH;
    let share_a = (level_a - top).max(0.0);
    let share_b = (level_b - top).max(0.0);
    share_b / (share_a + share_b).max(1e-6)
}

/// What a decal does to a spot of the surface under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalOver {
    /// How much of the decal's own colour shows (premultiplied: its lit
    /// colour is multiplied by this).
    pub own: f32,
    /// How much of the surface under it is left: what the surface's colour
    /// is multiplied by.
    pub under: f32,
    /// How strong the film of water's sheen is, over the part the decal's
    /// colour doesn't cover.
    pub film: f32,
}

/// A decal's effect where its mask is `mask` (0 to 1, as read from its
/// picture), for a material with `cover` (how much of its colour shows where
/// the mask is whole) and `wetness`.
pub fn decal_over(mask: f32, cover: f32, wetness: f32) -> DecalOver {
    let mask = ((mask - MASK_FLOOR) / (1.0 - MASK_FLOOR)).clamp(0.0, 1.0);
    let own = cover.clamp(0.0, 1.0) * mask;
    let wet = wetness.clamp(0.0, 1.0) * mask;
    let darken = wet * (1.0 - crate::surface_look::WET_KEEPS);
    let alpha = own + (1.0 - own) * darken;
    DecalOver {
        own,
        under: 1.0 - alpha,
        film: wet * (1.0 - own) * FILM_SHEEN,
    }
}
