use crate::Fingerprinter;

/// A seeded random number generator that gives the same numbers on every
/// computer: the Simulation's randomness, such as Prop Wash's flicker, comes
/// only from one of these, kept in its state ([ADR-0001]).
///
/// It is SplitMix64 (Steele, Lea and Flood, "Fast splittable pseudorandom
/// number generators", OOPSLA 2014; the version Vigna publishes beside
/// xoshiro): one 64-bit number of state, moved on by a fixed odd step and
/// mixed with shifts and multiplications. It uses whole-number arithmetic
/// only, so it never depends on the platform's maths, and copying it copies
/// where it is in its sequence.
///
/// [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Random {
    state: u64,
}

/// SplitMix64's step: 2⁶⁴ divided by the golden ratio, made odd.
const STEP: u64 = 0x9e37_79b9_7f4a_7c15;

impl Random {
    /// A generator whose sequence the seed alone decides.
    pub fn new(seed: u64) -> Random {
        Random { state: seed }
    }

    /// The next whole number, any of the 2⁶⁴ equally likely.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(STEP);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// The next number from 0 up to (not including) 1, each of the 2⁵³
    /// steps of 2⁻⁵³ equally likely. Worked out exactly: the top 53 bits of
    /// [`Random::next_u64`], scaled by a power of two.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1_u64 << 53) as f64)
    }

    /// The next number from −1 up to (not including) 1, equally likely
    /// anywhere between.
    pub fn signed(&mut self) -> f64 {
        2.0 * self.uniform() - 1.0
    }

    /// Feeds where it is in its sequence into a fingerprint.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        f.write_u64(self.state);
    }
}
