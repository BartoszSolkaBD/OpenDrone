"""Seeded numbers and smooth noise for the geometry helpers (MIT OR Apache-2.0).

A Map's `.glb` must be the same bytes on every computer (docs/building-a-map.md),
so nothing here uses Python's `random`, a clock, or a maths function a
processor may round differently (`sin`, `cos`, `exp`). It uses whole-number
arithmetic for the hashing, and only `+ - * /` and `floor` on 64-bit numbers
for the noise, which every computer does the very same way.

- `Random(seed, *keys)` is a stream of numbers fixed by the seed and keys:
  SHA-256 of them, then splitmix64 in Python's whole numbers. Give each part
  of a Map its own keys (`Random(7, "roof", sheet)`), so changing one part
  never moves another.
- `noise2(seed, x, y)` is smooth value noise from -1 to 1 with a feature size
  of about 1 unit; `fbm2` stacks octaves of it.
"""
import hashlib
import math

MASK = (1 << 64) - 1


class Random:
    """Numbers from a seed and any keys, the same on every computer."""

    def __init__(self, seed, *keys):
        text = repr((seed,) + tuple(keys)).encode()
        self.state = int.from_bytes(hashlib.sha256(text).digest()[:8], "little")

    def bits(self):
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)

    def unit(self):
        """A number in [0, 1), exact: 53 bits over 2^53."""
        return (self.bits() >> 11) / 9007199254740992.0

    def uniform(self, a, b):
        return a + (b - a) * self.unit()

    def chance(self, p):
        return self.unit() < p

    def pick(self, items):
        return items[int(self.unit() * len(items))]

    def child(self, *keys):
        """A new stream for a part, which doesn't move when the parent's
        other draws change."""
        return Random(self.bits(), *keys)


def _hash(seed, i, j):
    """A whole-number hash of a seed and two lattice numbers, from -1 to 1."""
    z = (seed * 0x9E3779B97F4A7C15 + i * 0xBF58476D1CE4E5B9 + j * 0x94D049BB133111EB + 0x1234567) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    z ^= z >> 31
    return (z >> 11) / 4503599627370496.0 - 1.0


def smoothstep(t):
    """0 at 0 and 1 at 1, flat at both ends; clamped."""
    t = 0.0 if t < 0.0 else 1.0 if t > 1.0 else t
    return t * t * (3.0 - 2.0 * t)


def noise2(seed, x, y):
    """Smooth value noise from -1 to 1, about one unit across a feature."""
    i, j = math.floor(x), math.floor(y)
    fx, fy = x - i, y - j
    ux = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0)
    uy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0)
    a, b = _hash(seed, i, j), _hash(seed, i + 1, j)
    c, d = _hash(seed, i, j + 1), _hash(seed, i + 1, j + 1)
    top = a + (b - a) * ux
    bottom = c + (d - c) * ux
    return top + (bottom - top) * uy


def fbm2(seed, x, y, wavelength, octaves=4, gain=0.5):
    """Layers of `noise2`: the first has features `wavelength` across, each
    next one half as big and `gain` as strong. The result stays within about
    -1 to 1 (it is divided by the sum of the strengths). Every octave is
    shifted and turned by a fixed amount, so the lattice never lines up."""
    total, strength, norm = 0.0, 1.0, 0.0
    f = 1.0 / wavelength
    for k in range(octaves):
        # A fixed turn of about 37 degrees per octave, as exact fractions.
        px = (x * 0.8 - y * 0.6) if k % 2 else (x * 0.6 + y * 0.8)
        py = (x * 0.6 + y * 0.8) if k % 2 else (-x * 0.8 + y * 0.6)
        total += strength * noise2(seed + 101 * k, px * f + 17.3 * k, py * f - 9.1 * k)
        norm += strength
        strength *= gain
        f *= 2.0
    return total / norm
