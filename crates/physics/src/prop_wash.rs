//! Prop Wash: a rotor sinking into its own air loses some thrust, and its
//! thrust flickers (E19 in #10; ADR-0005; flight-dynamics research §4.4).
//!
//! It acts on each rotor on its own, through the air at its own hub, on top
//! of momentum theory's thrust ([`crate::air`]). How strong it is and how
//! fast it flickers come only from the Quad definition's `[feel]` numbers,
//! `prop_wash_strength` and `prop_wash_flicker`; no setting changes them. A
//! Test Quad with the strength at 0% is how a Scenario flies without it.
//!
//! # Where it acts
//!
//! A rotor giving a thrust `T` pushes the air down through its disc at
//! `v_h = √(T / 2ρA)` in a hover, with `A` the disc's area: the unit Johnson
//! measures descents in, with `T` the rotor's thrust as it is (here
//! momentum theory's, in the air the rotor moves through, before Prop Wash).
//! Sinking along its own axis, the rotor meets that air again: the vortex
//! ring state, from 0 to 2 `v_h` of descent. Its worst stretch, where the
//! flow through the disc "varies greatly", is a descent of **0.4 to 1.4
//! `v_h`** (Johnson's NASA report, research source S10, as Hoffmann et al.,
//! S4, reproduce it). That stretch is the band Prop Wash acts in. For a rotor
//! carrying a quarter of the Whoop 65's weight, `v_h` is 5.70 m/s, so the
//! band is a descent of 2.28–7.98 m/s (the research's "about 2.3–8 m/s"). A
//! rotor working harder has a faster `v_h`, so its band starts and ends at
//! faster descents: punching out of a dive pulls its rotors back into it.
//!
//! - **Inside the band** its strength rises smoothly from nothing at either
//!   edge to the whole at the middle (0.9 `v_h`): `(4u·(1 − u))²`, with `u`
//!   how far across the band the descent is, from 0 to 1.
//! - **Air across the disc escapes it.** With the air across the disc at
//!   2.75 times the descent along the axis or more, it is gone: Talaeizadeh
//!   et al.'s wind-tunnel rule for a safe descent (research source S11).
//!   Below that it fades as `(1 − r²)²`, with `r` the air across the disc
//!   over 2.75 times the descent. For a level Quad, that is forward speed
//!   against the descent rate.
//! - Climbing, hovering, sinking slower than 0.4 `v_h` or faster than
//!   1.4 `v_h`, a stopped rotor and one spinning backwards: none. A rotor at
//!   idle in a fast dive gives little thrust, so its `v_h` is small and the
//!   dive is far past its band, in the windmill brake state.
//!
//! The two shapes are smooth (no corner at the edges or at 2.75×) and have no
//! source of their own: the research gives where the effect is, not how it
//! grows inside.
//!
//! # What it does
//!
//! With `k` the Quad's strength, `I` the share above (0 to 1) and `n` the
//! rotor's flicker (−1 to 1), the rotor's thrust is momentum theory's times
//! `(1 − k·I)·(1 + k·I·n)` (research §4.4's cheap model, with one strength
//! for both its loss and its flicker, as the Quad definition gives one):
//!
//! - it loses `k·I` of its thrust on average, 20% at the middle of the band
//!   for both alpha Quads;
//! - and it flickers up to `k·I` of that either way, each rotor its own way,
//!   so the Quad is shaken in roll and pitch, not only lifted and dropped.
//!   With the strength at 20%, the thrust at the middle of the band swings
//!   between 64% and 96% of momentum theory's.
//!
//! The ducts' ram drag follows the thrust it is given.
//!
//! # The flicker
//!
//! Each rotor's flicker moves smoothly from one random level to the next,
//! `prop_wash_flicker` times a second (15 Hz on both alpha Quads): each
//! level is drawn evenly from −1 to 1, and between two the flicker glides as
//! `3p² − 2p³` of the way, with `p` the share of the time between them gone.
//! Each rotor keeps its own levels and its own timing, so no two rotors
//! flicker together. Nothing in the research says how fast it flickers, so
//! a Feel Test tunes it.
//!
//! The levels come from [`Random`], a seeded random number generator, with
//! the timing in seconds of Simulation Time: so the same seed gives the same
//! flicker, on every computer and at every physics rate, and a different seed
//! a different one. The flicker goes on all the time, in the band or not, so
//! a rotor entering the band meets it mid-flicker rather than at a fresh
//! start. The Simulation gives each Quad its own seed ([`Flicker::new`],
//! [`crate::QuadBody::with_flicker`]); the generator, the levels and the
//! timing are part of the Quad's state, so they are fingerprinted and copied
//! with it.
//!
//! Settled motors (ADR-0002) are worked out with the flicker at its middle:
//! they make up Prop Wash's average loss, so a settled Quad holds its stated
//! motion on average.

use opendrone_maths::functions::fmod;
use opendrone_maths::{Fingerprinter, Random, Vec3};

/// The Quad definition's Prop Wash numbers (see the module).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropWash {
    /// How much thrust a rotor loses at the middle of the band, and the most
    /// its thrust flickers either way there, as a share from 0 to 1.
    pub strength: f64,
    /// How many times a second each rotor's flicker moves to a new level, in
    /// Hz.
    pub flicker: f64,
}

impl PropWash {
    /// No Prop Wash.
    pub const NONE: PropWash = PropWash {
        strength: 0.0,
        flicker: 0.0,
    };

    /// True when the strength is from 0 to 1 and the flicker 0 Hz or more,
    /// both real numbers.
    pub(crate) fn is_usable(&self) -> bool {
        (0.0..=1.0).contains(&self.strength) && self.flicker >= 0.0 && self.flicker.is_finite()
    }
}

/// The band's edges, as a descent along the rotor's axis in units of its
/// `v_h` (research §4.4).
pub const BAND: [f64; 2] = [0.4, 1.4];

/// Air across the disc at this many times the descent along the axis, or
/// more, escapes Prop Wash (research §4.4, Talaeizadeh et al.).
pub const ESCAPE: f64 = 2.75;

/// The share of Prop Wash's whole strength that acts on a rotor giving
/// `thrust` newtons, before Prop Wash, with its hub moving through still air
/// at `air` (body axes, m/s), from 0 to 1 (see the module's "Where it acts").
pub fn share(thrust: f64, air: Vec3, disc_area: f64, air_density: f64) -> f64 {
    // Written so that "not a number" acts nowhere.
    if !(thrust > 0.0 && disc_area > 0.0 && air_density > 0.0) {
        return 0.0;
    }
    let pushed = (thrust / (2.0 * air_density * disc_area)).sqrt();
    let descent = -air.z;
    if descent.is_nan() || descent <= 0.0 {
        return 0.0;
    }
    let across = (air.x * air.x + air.y * air.y).sqrt();
    in_the_band(descent / pushed) * clear_of_it(across / (ESCAPE * descent))
}

/// How far into the band a descent of `descent` (in units of `v_h`) is, from
/// 0 at either edge to 1 at its middle.
fn in_the_band(descent: f64) -> f64 {
    let u = (descent - BAND[0]) / (BAND[1] - BAND[0]);
    if !(u > 0.0 && u < 1.0) {
        return 0.0;
    }
    let bump = 4.0 * u * (1.0 - u);
    bump * bump
}

/// How much of it is left with the air across the disc at `r` times the
/// escape ratio's worth: all of it with none across, none from 1 on.
fn clear_of_it(r: f64) -> f64 {
    if r.is_nan() || r >= 1.0 {
        return 0.0;
    }
    let left = 1.0 - r * r;
    left * left
}

/// What Prop Wash makes of a rotor's thrust: `(1 − k·I)·(1 + k·I·n)`, for the
/// Quad's strength `k`, the share `I` that acts on it and its flicker `n`.
pub fn thrust_factor(strength: f64, share: f64, flicker: f64) -> f64 {
    let acting = strength * share;
    (1.0 - acting) * (1.0 + acting * flicker)
}

/// Each rotor's flicker, and the seeded generator its levels come from (see
/// the module's "The flicker"). Part of the Quad's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Flicker {
    random: Random,
    rotors: [Glide; 4],
}

/// One rotor's flicker: gliding from one level to the next.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Glide {
    /// The share of the time from `from` to `to` gone, from 0 up to 1.
    gone: f64,
    from: f64,
    to: f64,
}

impl Flicker {
    /// The flicker the seed alone decides: each rotor in turn, in
    /// Betaflight's motor order, draws where it starts between two levels,
    /// then the two levels.
    pub fn new(seed: u64) -> Flicker {
        let mut random = Random::new(seed);
        let rotors = [(); 4].map(|_| Glide {
            gone: random.uniform(),
            from: random.signed(),
            to: random.signed(),
        });
        Flicker { random, rotors }
    }

    /// Moves every rotor's flicker on by `dt` seconds, at `rate` new levels a
    /// second, drawing each new level in turn, rotor by rotor.
    pub(crate) fn step(&mut self, rate: f64, dt: f64) {
        for glide in &mut self.rotors {
            glide.gone += rate * dt;
            if glide.gone.is_nan() || glide.gone < 1.0 {
                continue;
            }
            if glide.gone >= 2.0 {
                // Faster than the physics steps: a level it skipped over
                // could never have shown, so it glides on from a fresh one.
                glide.gone = fmod(glide.gone, 1.0);
                glide.from = self.random.signed();
            } else {
                glide.gone -= 1.0;
                glide.from = glide.to;
            }
            glide.to = self.random.signed();
        }
    }

    /// Each rotor's flicker now, from −1 to 1, in Betaflight's motor order.
    pub fn values(&self) -> [f64; 4] {
        self.rotors.map(|glide| {
            let p = glide.gone;
            glide.from + (glide.to - glide.from) * p * p * (3.0 - 2.0 * p)
        })
    }

    /// Feeds the generator, the levels and the timing into a fingerprint, in
    /// a fixed order.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        self.random.write_fingerprint(f);
        for glide in &self.rotors {
            f.write_f64s(&[glide.gone, glide.from, glide.to]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: f64 = 9.621_127e-4; // a 35 mm prop's disc
    const RHO: f64 = 1.225;

    /// The thrust whose `v_h` is 1 m/s, so speeds below are in units of
    /// `v_h`.
    fn unit_thrust() -> f64 {
        2.0 * RHO * AREA
    }

    fn share_at(descent: f64, across: f64) -> f64 {
        share(unit_thrust(), Vec3::new(across, 0.0, -descent), AREA, RHO)
    }

    #[test]
    fn it_acts_only_while_the_rotor_sinks_through_the_band() {
        for descent in [-3.0, -0.5, 0.0, 0.2, 0.4, 1.4, 1.6, 2.0, 5.0] {
            assert_eq!(share_at(descent, 0.0), 0.0, "descent {descent} v_h");
        }
        assert_eq!(share_at(0.9, 0.0), 1.0);
        assert!(share_at(0.41, 0.0) > 0.0 && share_at(1.39, 0.0) > 0.0);
        // Halfway up either side, (4 × 0.25 × 0.75)² = 0.5625.
        assert!((share_at(0.65, 0.0) - 0.5625).abs() < 1e-12);
        assert!((share_at(1.15, 0.0) - 0.5625).abs() < 1e-12);
    }

    #[test]
    fn air_across_the_disc_at_2_75_times_the_descent_escapes_it() {
        assert_eq!(share_at(0.9, 2.75 * 0.9), 0.0);
        assert_eq!(share_at(0.9, 3.0 * 0.9), 0.0);
        assert!(share_at(0.9, 2.7 * 0.9) > 0.0);
        // Half the ratio leaves (1 − 0.25)² of it.
        assert!((share_at(0.9, 1.375 * 0.9) - 0.5625).abs() < 1e-12);
    }

    #[test]
    fn a_stopped_or_backwards_rotor_and_broken_numbers_feel_none() {
        let sinking = Vec3::new(0.0, 0.0, -0.9);
        assert_eq!(share(0.0, sinking, AREA, RHO), 0.0);
        assert_eq!(share(-1.0, sinking, AREA, RHO), 0.0);
        assert_eq!(share(f64::NAN, sinking, AREA, RHO), 0.0);
        assert_eq!(
            share(unit_thrust(), Vec3::new(0.0, 0.0, f64::NAN), AREA, RHO),
            0.0
        );
    }

    #[test]
    fn at_the_middle_of_the_band_a_20_percent_strength_swings_the_thrust_from_64_to_96_percent() {
        assert!((thrust_factor(0.2, 1.0, -1.0) - 0.64).abs() < 1e-15);
        assert!((thrust_factor(0.2, 1.0, 0.0) - 0.8).abs() < 1e-15);
        assert!((thrust_factor(0.2, 1.0, 1.0) - 0.96).abs() < 1e-15);
        assert_eq!(thrust_factor(0.2, 0.0, 0.7), 1.0);
        assert_eq!(thrust_factor(0.0, 1.0, 0.7), 1.0);
    }

    #[test]
    fn each_rotor_flickers_its_own_way_between_minus_1_and_1() {
        let mut flicker = Flicker::new(1);
        let mut seen = Vec::new();
        for _ in 0..8000 {
            flicker.step(15.0, 1.0 / 8000.0);
            let values = flicker.values();
            assert!(values.iter().all(|n| (-1.0..=1.0).contains(n)));
            seen.push(values);
        }
        for a in 0..4 {
            for b in (a + 1)..4 {
                assert!(seen.iter().any(|v| v[a] != v[b]), "rotors {a} and {b}");
            }
        }
    }

    #[test]
    fn the_flicker_glides_without_jumps() {
        // At 15 Hz and 8 kHz, a glide from −1 to 1 moves at most 1.5 × 2 ×
        // 15 / 8000 = 0.0056 a step.
        let mut flicker = Flicker::new(4);
        let mut last = flicker.values();
        for _ in 0..16_000 {
            flicker.step(15.0, 1.0 / 8000.0);
            let now = flicker.values();
            for k in 0..4 {
                assert!((now[k] - last[k]).abs() <= 0.005_63);
            }
            last = now;
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_flicker_and_another_seed_another() {
        let run = |seed| {
            let mut flicker = Flicker::new(seed);
            (0..4000)
                .map(|_| {
                    flicker.step(15.0, 1.0 / 8000.0);
                    flicker.values()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(1), run(1));
        assert_ne!(run(1), run(2));
    }

    #[test]
    fn the_flicker_is_the_same_at_4_and_8_khz() {
        // Its timing is in seconds and its levels are drawn in the same
        // order, so only rounding in the timing differs.
        let mut slow = Flicker::new(9);
        let mut fast = Flicker::new(9);
        for _ in 0..4000 {
            slow.step(15.0, 1.0 / 4000.0);
            fast.step(15.0, 1.0 / 8000.0);
            fast.step(15.0, 1.0 / 8000.0);
            for (s, f) in slow.values().iter().zip(fast.values()) {
                assert!((s - f).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn a_flicker_faster_than_the_physics_steps_still_draws_a_bounded_number_of_levels() {
        let mut flicker = Flicker::new(5);
        flicker.step(1e15, 1.0 / 8000.0);
        assert!(flicker.values().iter().all(|n| (-1.0..=1.0).contains(n)));
    }
}
