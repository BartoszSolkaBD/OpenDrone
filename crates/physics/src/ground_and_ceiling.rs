//! Ground and ceiling effect: what a Map surface close to a rotor does to its
//! thrust (E20 and E21 in #10; flight-dynamics research §4.5). Near a floor a
//! rotor gets a cushion; under a ceiling it gets pulled up; and with only some
//! of its rotors over a surface, as half over a ledge, the Quad gets a kick.
//! They are the only flight effects that read the Map, through the same solid
//! parts the collisions use, and they only read it.
//!
//! # Looking down and up
//!
//! Every step, each rotor that pushes air looks from its hub along its own
//! axis, through the Map's solid parts: down, the way it blows its air, and
//! up, the way it draws it in. That is two parry3d ray casts per rotor, each
//! reaching [`REACH`] rotor radii ([`crate::MapCollision`]'s `nearest_along`).
//! What each finds is the rotor's distance to the surface, `z`.
//!
//! - Level over a level floor, down is straight down and `z` is the rotor's
//!   height, as in the published formulas. Tilted, the rotor looks along its
//!   tilted axis, so the floor seems farther (`z / cos tilt`) and the cushion
//!   fades as the rotor turns away from it, until on its side the rotor
//!   blows past the floor altogether. The formulas are for a rotor parallel
//!   to the surface; for the small tilts of a hover the two agree (Sanchez-
//!   Cuevas et al. §2.4, below, find a tilt's own moment small).
//! - Down and up are the rotor's own, so on its back a Quad's rotors draw air
//!   in from the floor, and get the ceiling's pull from it; a wall a rotor
//!   blows straight at cushions it as a floor would. The wall *beside* a
//!   rotor (E22, the wall effect) has no published closed-form model and is
//!   left for later.
//! - A rotor stopped or spinning backwards (Crash Flip) gets neither effect,
//!   and leaves no image (below): the formulas are for a rotor blowing air
//!   away from its intake.
//! - Beyond [`REACH`] rotor radii both effects are left out. There the
//!   ground effect is about 0.7% on both alpha Quads (1.2% with the body term
//!   at the top of its range, 4) and the ceiling effect under 0.04%, so a
//!   rotor's thrust steps by that much as it crosses that distance.
//!
//! # Ground effect (E20)
//!
//! Sanchez-Cuevas, Heredia and Ollero (research source S12, Int. J.
//! Aerospace Eng. 2017) model each rotor as a source of air flow, and the
//! ground as a mirror image of every rotor below it (potential flow with the
//! method of images, after Cheeseman and Bennett). The images push air back up
//! through the rotor, so for the same power it gives more thrust. For one
//! rotor at height `z`, with `R` its radius, their eq. (2):
//!
//! `T_IGE / T_OGE = 1 / (1 − (R/4z)²)`
//!
//! (+6.7% at one radius, +1.6% at two). For a quad hovering level over the
//! ground, each rotor also feels its two neighbours' images, `d` away, and the
//! diagonal one's, and the air the rotors throw back up under the body, a
//! "fountain", lifts the body: their eq. (4),
//!
//! `T_IGE / T_OGE = 1 / (1 − (R/4z)² − R²·z/√(d² + 4z²)³ − (R²/2)·z/√(2d² + 4z²)³ − 2R²·z/√(b² + 4z²)³·K_b)`
//!
//! with `d` the distance between neighbouring rotors' axes, `b` between
//! opposite ones (the diagonal) and `K_b` their empirical body-lift
//! coefficient, close to 2 in their tests: the Quad definition's
//! `ground_effect_body`. Their tests show the multirotor's ground effect
//! counting up to about 5 rotor radii, against about 2 for one rotor.
//!
//! **Each rotor on its own.** The Map isn't one flat floor, so each rotor
//! works out its own gain from its own distance, and the images of only the
//! rotors that see a surface below them, as Sanchez-Cuevas et al.'s own
//! simulator does (their eq. 10, per rotor from its own distance) and as they
//! treat one rotor, or three, over a surface (§2.3). In the same method of
//! images, the image of rotor `j`, `h` from rotor `i` in the rotors' plane,
//! cancels a share `(R²/4)·(z_i + z_j) / √(h² + (z_i + z_j)²)³` of rotor `i`'s
//! air: for rotor `i` itself (`h = 0`) that is `(R/4z)²`, and with every
//! rotor at one height the sum is exactly eq. (4)'s first three terms.
//!
//! **The body term with only some rotors over a surface** has no published
//! form: eq. (4) is for all four, and their three-rotor test (Figure 8)
//! measured more than the images alone, "though with less intensity" than
//! all four. So each rotor takes eq. (4)'s body term, at its own distance,
//! times the share of the other three rotors that also see a surface below
//! them: all of it with all four over the floor, two thirds with three, a
//! third with two, as half over a ledge, and none for a rotor alone, which
//! leaves eq. (2). The fountain forms between rotors over the ground, so the
//! more of them, the more of it. **This sharing is a modelling choice** with
//! no source.
//!
//! **Half over a ledge** the rotors over it lift harder than the rest, so the
//! Quad tips away from the surface (their §2.3 and Figure 7, the "partial
//! ground effect"): flying off an edge, the pitch kick.
//!
//! # Ceiling effect (E21)
//!
//! A ceiling blocks the air a rotor draws in. Hsiao and Chirarattananon
//! (research source S13, IEEE/ASME Trans. Mechatronics 2019, eqs. 6, 7 and 10)
//! work out by momentum theory that a rotor `z` below a ceiling needs `γ`
//! times less power for the same thrust, with
//!
//! `γ = ½ + ½·√(1 + α₀ / (8·(z/R)²))`
//!
//! and `α₀` their factor for the air's uneven flow round the rotor: 1 in the
//! plain model, which Elliott-Roe et al. (research source S14, IMAV 2024, its
//! eq. 2) use, and up to 1.60 in Hsiao and Chirarattananon's fit for a single
//! 23 mm prop. It is the Quad definition's `ceiling_effect_asymmetry`. Their
//! recirculation factor `α₁`, which weakens the effect, is taken as 0, as in
//! S14.
//!
//! Thrust grows with power to the ⅔, so **at the same power the thrust is
//! `γ^⅔` times its open-air thrust**, as Hsiao and Chirarattananon say of
//! their own measurement ("amplified by a factor of 4^(2/3) or 2.5 times for
//! the same power consumption", §V.C). Here a motor at a fixed speed gives
//! its prop the same power whatever the air does ([`crate::air`] holds the
//! power fixed the same way), so the gain is `γ^⅔`: +7.4% at half a radius,
//! +2.0% at one radius, +0.5% at two. It grows the closer the rotor gets:
//! positive feedback, which is why a whoop gets sucked onto a ceiling.
//!
//! (S14 itself writes `γ` as the thrust ratio at a fixed speed, which would
//! give +11% at half a radius; its own measurements fall below that closer
//! than half a radius. The research note's §4.5 has the correction.)
//!
//! # Together, and how close
//!
//! - A rotor with both a floor and a ceiling near gets both gains, multiplied.
//!   No source gives a rotor squeezed between the two.
//! - **Closer than [`CLOSEST`] rotor radius**, each formula is used at that
//!   distance. Sanchez-Cuevas et al.'s measurements and curves start at about
//!   half a radius (their Figures 3 and 6), Elliott-Roe et al. measure less
//!   ceiling effect than their formula closer than that, and Hsiao and
//!   Chirarattananon's larger prop got much less than the plain model near
//!   the ceiling. Without it, eq. (2) would grow without end at a quarter of
//!   a radius, which a 5″ resting on the ground already reaches. **This
//!   limit is a modelling choice.**
//! - Each gain multiplies the rotor's thrust in the air it moves through
//!   ([`crate::air`]); it acts along the rotor's axis at its hub, so a rotor
//!   lifting harder than the others also turns the Quad. The formulas are for
//!   a hovering rotor; Cheeseman and Bennett's cushion fades with forward
//!   speed, but the research gives no form for it, so the gain is the same at
//!   any speed. The rotor and duct drag stay as in open air.
//! - Every rotor counted as an image is taken to work as hard as the rotor
//!   it acts on, as in the published formulas, which are for rotors at one
//!   speed.
//!
//! # What it costs
//!
//! Two ray casts per rotor that pushes air, so eight a step for a flying
//! Quad, 64,000 a second at 8 kHz; none for a rotor stopped or spinning
//! backwards, and none at all with no Map shapes. Each tests every Map
//! shape's bounding box against the ray's stretch (at most [`REACH`] rotor
//! radii: 17.5 cm on the Whoop 65, 65 cm on the Freestyle 5″), and casts only
//! at the shapes it overlaps.

use opendrone_maths::Vec3;
use opendrone_maths::functions::{cbrt, max};

use crate::air::Push;
use crate::map::MapCollision;
use crate::{GroundAndCeiling, QuadState};

/// How far each rotor looks down and up, in rotor radii.
pub const REACH: f64 = 10.0;

/// The closest, in rotor radii, at which the formulas are used: nearer than
/// this, each gain holds at its value here.
pub const CLOSEST: f64 = 0.5;

/// What ground and ceiling effect need to know about the Quad.
pub(crate) struct Rotors<'a> {
    /// Each rotor's hub from the centre of mass, in body axes.
    pub positions: &'a [Vec3; 4],
    /// The props' radius, in metres.
    pub radius: f64,
    /// Opposite rotors' axes apart, in metres: Sanchez-Cuevas et al.'s `b`.
    pub diagonal: f64,
    pub strengths: &'a GroundAndCeiling,
}

/// How far each rotor that pushes air is from a Map surface along its own
/// axis, in metres: below it, the way it blows, and above it, the way it
/// draws air in. `None` where nothing is within reach, and for a rotor that
/// pushes no air.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Distances {
    pub below: [Option<f64>; 4],
    pub above: [Option<f64>; 4],
}

/// Each rotor looks down and up at the Map from where `state` puts it (see
/// "Looking down and up"). Only the rotors `pushing` air look.
pub(crate) fn look(
    map: &MapCollision,
    state: &QuadState,
    rotors: &Rotors,
    pushing: [bool; 4],
) -> Distances {
    let mut distances = Distances::default();
    if map.is_empty() || !above_zero(rotors.radius) {
        return distances;
    }
    let reach = REACH * rotors.radius;
    let down = state.attitude.body_to_world(Vec3::new(0.0, 0.0, -1.0));
    for (k, at) in rotors.positions.iter().enumerate() {
        if !pushing[k] {
            continue;
        }
        let hub = state.position + state.attitude.body_to_world(*at);
        distances.below[k] = map.nearest_along(hub, down, reach);
        distances.above[k] = map.nearest_along(hub, -down, reach);
    }
    distances
}

/// Each rotor's thrust gain from the surfaces at these distances: its thrust
/// near them over its thrust in open air, at the same rotor speed (see
/// "Ground effect" and "Ceiling effect"). 1 for a rotor with nothing near.
pub(crate) fn gains(distances: &Distances, rotors: &Rotors) -> [f64; 4] {
    let r = rotors.radius;
    if !above_zero(r) {
        return [1.0; 4];
    }
    let closest = CLOSEST * r;
    let below = distances.below.map(|z| z.map(|z| max(z, closest)));
    let over = below.iter().filter(|z| z.is_some()).count();
    core::array::from_fn(|i| {
        let mut gain = 1.0;
        if let Some(z) = below[i] {
            // The share of the rotor's air the images cancel.
            let mut cancelled = 0.0;
            for (j, image) in below.iter().enumerate() {
                if let Some(z_j) = *image {
                    let apart = (rotors.positions[i] - rotors.positions[j]).length();
                    cancelled += r * r / 4.0 * image_share(apart, z + z_j);
                }
            }
            // The body term, for the share of the other rotors that see a
            // surface too.
            let others = (over - 1) as f64 / 3.0;
            let body = rotors.strengths.ground_effect_body;
            cancelled += 2.0 * r * r * body * z / cubed_root_sum(rotors.diagonal, 2.0 * z) * others;
            gain *= 1.0 / (1.0 - cancelled);
        }
        if let Some(z) = distances.above[i] {
            let height = max(z, closest) / r;
            let alpha = rotors.strengths.ceiling_effect_asymmetry;
            let gamma = 0.5 + 0.5 * (1.0 + alpha / (8.0 * height * height)).sqrt();
            gain *= cbrt(gamma * gamma);
        }
        gain
    })
}

/// `s / √(h² + s²)³`: how strongly an image `s` below a rotor (counting both
/// distances to the surface) and `h` across from it pushes air up through it,
/// per unit of `R²/4`.
fn image_share(h: f64, s: f64) -> f64 {
    s / cubed_root_sum(h, s)
}

/// True when `x` is a number above zero ("not a number" isn't).
fn above_zero(x: f64) -> bool {
    x > 0.0
}

/// `√(a² + b²)³`.
fn cubed_root_sum(a: f64, b: f64) -> f64 {
    let squares = a * a + b * b;
    squares * squares.sqrt()
}

/// True when the ground effect can work for this Quad: its body term and
/// ceiling asymmetry are 0 or more, and even with every rotor at the closest
/// distance where each image pushes hardest, its rotors still push air down
/// (the share eq. (4) cancels stays below the whole). A Quad whose body term
/// is too big for its rotors' layout fails, rather than giving endless or
/// backwards thrust near a floor.
pub(crate) fn can_work(rotors: &Rotors) -> bool {
    let strengths = rotors.strengths;
    let zero_or_more = |value: f64| value >= 0.0 && value.is_finite();
    if !zero_or_more(strengths.ground_effect_body)
        || !zero_or_more(strengths.ceiling_effect_asymmetry)
    {
        return false;
    }
    let r = rotors.radius;
    if !above_zero(r) {
        return true;
    }
    // Every image term s / √(h² + s²)³ is largest at s = h/√2, and s is at
    // least twice the closest distance.
    let least = 2.0 * CLOSEST * r;
    let most = |h: f64| image_share(h, max(h / core::f64::consts::SQRT_2, least));
    (0..4).all(|i| {
        let mut cancelled = 0.0;
        for j in 0..4 {
            let apart = (rotors.positions[i] - rotors.positions[j]).length();
            cancelled += r * r / 4.0 * most(apart);
        }
        // The body term, z / √(b² + 4z²)³, is half the image term at s = 2z.
        cancelled += 2.0 * r * r * strengths.ground_effect_body * most(rotors.diagonal) / 2.0;
        cancelled < 1.0
    })
}

/// Adds the extra lift these gains give to the air's push: each rotor's
/// thrust times its gain less one, along the body's up axis at its hub, so it
/// lifts the Quad and turns it about its centre of mass. A rotor with a gain
/// of exactly 1, or no thrust, changes nothing.
pub(crate) fn add_lift(push: &mut Push, gains: &[f64; 4], positions: &[Vec3; 4]) {
    for k in 0..4 {
        let thrust = push.thrusts[k];
        if gains[k] == 1.0 || !above_zero(thrust) {
            continue;
        }
        let extra = Vec3::new(0.0, 0.0, thrust * (gains[k] - 1.0));
        push.force += extra;
        push.torque += positions[k].cross(extra);
        push.thrusts[k] = thrust * gains[k];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOOP: GroundAndCeiling = GroundAndCeiling {
        ground_effect_body: 2.0,
        ceiling_effect_asymmetry: 1.0,
    };

    /// The Whoop 65's rotors: 35 mm props on a 66 mm diagonal.
    fn whoop<'a>(positions: &'a [Vec3; 4], strengths: &'a GroundAndCeiling) -> Rotors<'a> {
        Rotors {
            positions,
            radius: 0.0175,
            diagonal: 0.066,
            strengths,
        }
    }

    fn layout() -> [Vec3; 4] {
        crate::RotorLayout {
            diagonal: 0.066,
            rotor_height: 0.008,
            direction: crate::PropDirection::PropsIn,
        }
        .positions()
    }

    #[test]
    fn one_rotor_over_the_ground_gains_cheeseman_and_bennetts_6_7_percent_at_one_radius() {
        let positions = layout();
        let distances = Distances {
            below: [Some(0.0175), None, None, None],
            above: [None; 4],
        };
        let gains = gains(&distances, &whoop(&positions, &WHOOP));
        // 1 / (1 − (1/4)²) = 16/15.
        assert!((gains[0] - 16.0 / 15.0).abs() < 1e-15, "{}", gains[0]);
        assert_eq!(&gains[1..], &[1.0; 3]);
    }

    #[test]
    fn four_rotors_level_over_the_ground_gain_sanchez_cuevas_eq_4() {
        let positions = layout();
        let (r, b) = (0.0175, 0.066);
        let d = b / core::f64::consts::SQRT_2;
        for z in [0.00875, 0.0175, 0.035, 0.07, 0.15] {
            let distances = Distances {
                below: [Some(z); 4],
                above: [None; 4],
            };
            let gains = gains(&distances, &whoop(&positions, &WHOOP));
            let cubed = |x: f64| x * x * x;
            let eq_4 = 1.0
                / (1.0
                    - (r / (4.0 * z)) * (r / (4.0 * z))
                    - r * r * z / cubed((d * d + 4.0 * z * z).sqrt())
                    - r * r / 2.0 * z / cubed((2.0 * d * d + 4.0 * z * z).sqrt())
                    - 2.0 * r * r * z / cubed((b * b + 4.0 * z * z).sqrt()) * 2.0);
            for gain in gains {
                assert!(
                    (gain - eq_4).abs() < 1e-12,
                    "at {z} m: {gain} against {eq_4}"
                );
            }
        }
    }

    #[test]
    fn under_a_ceiling_the_gain_is_gamma_to_the_two_thirds() {
        let positions = layout();
        let distances = Distances {
            below: [None; 4],
            above: [Some(0.0175); 4],
        };
        let gains = gains(&distances, &whoop(&positions, &WHOOP));
        // γ = ½ + ½·√(1 + 1/8) at one radius.
        let gamma: f64 = 0.5 + 0.5 * (1.125_f64).sqrt();
        for gain in gains {
            assert!((gain - cbrt(gamma * gamma)).abs() < 1e-15);
        }
    }

    #[test]
    fn closer_than_half_a_radius_each_gain_holds_at_half_a_radius() {
        let positions = layout();
        let at = |z: f64| {
            gains(
                &Distances {
                    below: [Some(z); 4],
                    above: [Some(z); 4],
                },
                &whoop(&positions, &WHOOP),
            )
        };
        assert_eq!(at(0.0), at(0.00875));
        assert_eq!(at(0.004), at(0.00875));
        assert!(at(0.009)[0] < at(0.00875)[0]);
    }

    #[test]
    fn a_body_term_too_big_for_the_layout_cant_work() {
        let positions = layout();
        assert!(can_work(&whoop(&positions, &WHOOP)));
        let huge = GroundAndCeiling {
            ground_effect_body: 40.0,
            ..WHOOP
        };
        assert!(!can_work(&whoop(&positions, &huge)));
        let negative = GroundAndCeiling {
            ceiling_effect_asymmetry: -1.0,
            ..WHOOP
        };
        assert!(!can_work(&whoop(&positions, &negative)));
    }
}
