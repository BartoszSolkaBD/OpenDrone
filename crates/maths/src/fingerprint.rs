use core::fmt;

/// A short summary of a set of numbers: if any number changes by even one
/// bit, the fingerprint changes too (except in astronomically rare cases).
/// Two computers that give the same fingerprint worked out the same numbers.
///
/// It is shown as 16 hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint(pub u64);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Builds a [`Fingerprint`] from numbers fed in one by one, in a fixed order.
///
/// It uses 64-bit FNV-1a over each number's bytes, least significant byte
/// first, so it gives the same result on every computer. Two numbers that are
/// equal but have different bits are written the same way first, as the
/// determinism research advises: every "not a number" is written as one
/// pattern (its sign differs between ARM and x86), and −0 is written as +0.
#[derive(Clone, Debug)]
pub struct Fingerprinter {
    hash: u64,
}

const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;
const ONE_NOT_A_NUMBER: u64 = 0x7ff8_0000_0000_0000;

impl Fingerprinter {
    pub fn new() -> Fingerprinter {
        Fingerprinter { hash: OFFSET_BASIS }
    }

    pub fn write_u64(&mut self, n: u64) {
        for byte in n.to_le_bytes() {
            self.hash ^= u64::from(byte);
            self.hash = self.hash.wrapping_mul(PRIME);
        }
    }

    pub fn write_f64(&mut self, n: f64) {
        let bits = if n.is_nan() {
            ONE_NOT_A_NUMBER
        } else if n == 0.0 {
            0
        } else {
            n.to_bits()
        };
        self.write_u64(bits);
    }

    pub fn write_f64s(&mut self, numbers: &[f64]) {
        for &n in numbers {
            self.write_f64(n);
        }
    }

    /// Feeds in another fingerprint, such as the previous step's.
    pub fn write_fingerprint(&mut self, fingerprint: Fingerprint) {
        self.write_u64(fingerprint.0);
    }

    pub fn finish(&self) -> Fingerprint {
        Fingerprint(self.hash)
    }
}

impl Default for Fingerprinter {
    fn default() -> Fingerprinter {
        Fingerprinter::new()
    }
}
