"""Uneven ground for the Maps that must not look like a flat plane (MIT OR Apache-2.0).

A yard, a road or a factory floor is never level: it rolls a little, it dips
where water stands, rubble heaps against the walls, and the whole site leans a
few degrees. `Ground` is a height at every `(x, y)`, from layered seeded noise
and a few named features, with **flat patches** where the ground has to be
flat enough to land on (the Launch Spot's 3 m is one). Plain Python, the same
bytes on every computer (`odnoise`, `odgeom`).

```python
ground = Ground(
    seed=3, amplitude=0.10, slope=(0.01, 0.0),
    flats=[Flat(0.0, -17.0, 3.5)],                     # the Launch Spot stays level
    mounds=[Mound((-12, -7.5), (12, -7.5), 0.45, 1.6, spills=[(8.0, 2.0, 0.5)])],
    dips=water_dips(Random(3, "dips"), (-30, -30, 30, 30), count=6),
)
m.solid("yard", ground.mesh((-30, -30, 30, 30), 0.5), None, "ground_dirt_rubble", "trimesh")
```

The triangle cost is the grid's: a mesh with `step` metres between corners is
`2 / step**2` triangles per square metre (8 at 0.5 m, 2 at 1 m, 32 at 0.25 m).
`collision_mesh` is the same ground on a coarser grid, for a collision-only
part (`od_collision_only`): a part that is solid but never drawn.
"""
import math

import odgeom as g
from odnoise import fbm2, noise2, smoothstep


class Flat:
    """A round patch of ground made level. Inside `radius` the ground is
    exactly at `level` (default: the ground's own height at the centre), and
    it eases back into the rolling ground over the next `blend` metres."""

    def __init__(self, x, y, radius, blend=1.5, level=None):
        self.x, self.y, self.radius, self.blend, self.level = x, y, radius, blend, level

    def weight(self, x, y):
        dx, dy = x - self.x, y - self.y
        d = math.sqrt(dx * dx + dy * dy)
        return 1.0 - smoothstep((d - self.radius) / self.blend)


class FlatRect(Flat):
    """A rectangular level patch, for a floor or a slab: exactly level inside
    `(x0, y0, x1, y1)`, easing back over `blend` metres."""

    def __init__(self, x0, y0, x1, y1, blend=1.5, level=None):
        Flat.__init__(self, (x0 + x1) / 2.0, (y0 + y1) / 2.0, 0.0, blend, level)
        self.rect = (x0, y0, x1, y1)

    def weight(self, x, y):
        x0, y0, x1, y1 = self.rect
        dx = max(x0 - x, 0.0, x - x1)
        dy = max(y0 - y, 0.0, y - y1)
        return 1.0 - smoothstep(math.sqrt(dx * dx + dy * dy) / self.blend)


class Dip:
    """A shallow hollow where water would stand: `depth` metres at the
    centre, round, `radius` across to the rim."""

    def __init__(self, x, y, radius, depth):
        self.x, self.y, self.radius, self.depth = x, y, radius, depth

    def height(self, x, y):
        dx, dy = x - self.x, y - self.y
        q = (dx * dx + dy * dy) / (self.radius * self.radius)
        if q >= 1.0:
            return 0.0
        return -self.depth * (1.0 - q) * (1.0 - q)


class Mound:
    """Rubble heaped against a wall that runs from `p0` to `p1` (x, y): up
    to `height` metres at the wall, falling away over `width` metres, uneven
    along the wall and tapering off at its ends. It is on both sides of the
    wall, or one: `side` is 1 for the left of the way `p0` to `p1` runs, -1
    for the right, 0 for both. `spills` are `(along, spread, extra)`: a heap
    `extra` metres higher where something fell out of the wall, `along`
    metres from `p0`, `spread` metres either way and twice as wide."""

    def __init__(self, p0, p1, height, width, side=0, spills=(), seed=0):
        self.p0, self.p1, self.height, self.width, self.side = p0, p1, height, width, side
        self.spills = list(spills)
        self.seed = seed
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        self.length = math.sqrt(dx * dx + dy * dy)
        self.dir = (dx / self.length, dy / self.length)

    def height_at(self, x, y):
        rx, ry = x - self.p0[0], y - self.p0[1]
        along = rx * self.dir[0] + ry * self.dir[1]
        across = -rx * self.dir[1] + ry * self.dir[0]  # positive on the left
        if self.side and across * self.side < 0.0:
            return 0.0
        # Past an end, the distance runs on to the end's corner.
        past = max(-along, along - self.length, 0.0)
        dist = math.sqrt(past * past + across * across)
        total = 0.0
        if dist < self.width:
            q = 1.0 - dist / self.width
            uneven = 0.6 + 0.4 * noise2(self.seed, along / 2.5, across / 6.0)
            total += self.height * q * q * uneven
        for u, spread, extra in self.spills:
            w = 2.0 * spread
            du = (along - u) / spread
            if abs(du) < 1.0 and dist < w:
                q = 1.0 - dist / w
                total += extra * q * q * (1.0 - du * du) * (1.0 - du * du)
        return total


def water_dips(rnd, rect, count, radius=(0.8, 2.5), depth=(0.03, 0.08), keep_out=()):
    """`count` dips at seeded places in the rectangle `(x0, y0, x1, y1)`,
    none inside a `keep_out` circle `(x, y, radius)`. The Dips come back in
    the order they were drawn, so a Map can put a puddle in each."""
    x0, y0, x1, y1 = rect
    dips = []
    tries = 0
    while len(dips) < count and tries < 50 * count:
        tries += 1
        r = rnd.uniform(*radius)
        x, y = rnd.uniform(x0 + r, x1 - r), rnd.uniform(y0 + r, y1 - r)
        d = rnd.uniform(*depth)
        if any(math.sqrt((x - kx) * (x - kx) + (y - ky) * (y - ky)) < kr + r for kx, ky, kr in keep_out):
            continue
        dips.append(Dip(x, y, r, d))
    return dips


class Ground:
    """The height of uneven ground at every point.

    - `amplitude`: how far the rolling ground strays from the plane, in
      metres, at most about this much either way (a yard 0.10, a road 0.03, a
      concrete floor 0.01). `wavelength` is the size of its biggest swells.
    - `slope`: `(rise per metre east, rise per metre north)`; 0.02 is a
      slope of about 1 degree.
    - `base`: the height of the plane at the origin.
    - `flats`, `mounds` and `dips`: see `Flat`, `Mound` and `Dip`. A flat
      patch wins over everything else inside it.
    """

    def __init__(self, seed, amplitude=0.10, wavelength=14.0, slope=(0.0, 0.0), base=0.0,
                 flats=(), mounds=(), dips=()):
        self.seed, self.amplitude, self.wavelength = seed, amplitude, wavelength
        self.slope, self.base = slope, base
        self.flats, self.mounds, self.dips = list(flats), list(mounds), list(dips)
        for k, mound in enumerate(self.mounds):
            mound.seed = mound.seed or seed + 7 + k
        # A flat patch with no level is level at the ground's own height at
        # its centre, before any patch is applied.
        self.levels = [f.level if f.level is not None else self.rolling(f.x, f.y) for f in self.flats]

    def rolling(self, x, y):
        """The ground with no flat patches."""
        h = self.base + self.slope[0] * x + self.slope[1] * y
        h += self.amplitude * fbm2(self.seed, x, y, self.wavelength, octaves=5, gain=0.55)
        for dip in self.dips:
            h += dip.height(x, y)
        for mound in self.mounds:
            h += mound.height_at(x, y)
        return h

    def height(self, x, y):
        h = self.rolling(x, y)
        for flat, level in zip(self.flats, self.levels):
            w = flat.weight(x, y)
            if w > 0.0:
                h += (level - h) * w
        return h

    def mesh(self, rect, step):
        """The ground as a sheet over the rectangle `(x0, y0, x1, y1)`, a
        corner every `step` or so, facing up. 2 / step**2 triangles a square
        metre."""
        return g.heightfield_mesh(*rect, step, self.height)

    def collision_mesh(self, rect, step):
        """The same ground on a coarser grid, for a collision-only part. It
        follows the drawn ground to within `gap` (see `max_gap`)."""
        return g.heightfield_mesh(*rect, step, self.height)

    def max_gap(self, rect, step, coarse_step):
        """The most the coarser mesh differs in height from the finer one, in
        metres, checked at every corner of the finer one (a bound on how far
        the Quad's solid ground can be from what is drawn)."""
        fine = self.mesh(rect, step)
        coarse = grid_height(rect, coarse_step, self.height)
        return max(abs(z - coarse(x, y)) for x, y, z in fine[0])


def grid_height(rect, step, height):
    """The height of the sheet `odgeom.heightfield_mesh(*rect, step, height)`
    makes, at any `(x, y)` over it: each square is two triangles, cut along
    the diagonal that changes height least, as the mesh cuts it."""
    x0, y0, x1, y1 = rect
    nx, ny = max(1, round((x1 - x0) / step)), max(1, round((y1 - y0) / step))

    def at(x, y):
        fi = min(max((x - x0) / (x1 - x0) * nx, 0.0), nx - 1e-9)
        fj = min(max((y - y0) / (y1 - y0) * ny, 0.0), ny - 1e-9)
        i, j = int(fi), int(fj)
        u, v = fi - i, fj - j
        xa, xb = x0 + (x1 - x0) * i / nx, x0 + (x1 - x0) * (i + 1) / nx
        ya, yb = y0 + (y1 - y0) * j / ny, y0 + (y1 - y0) * (j + 1) / ny
        za, zb, zc, zd = height(xa, ya), height(xb, ya), height(xb, yb), height(xa, yb)
        if abs(za - zc) <= abs(zb - zd):  # cut a-c: lower triangle a, b, c
            return za + (zb - za) * u + (zc - zb) * v if u >= v else za + (zc - zd) * u + (zd - za) * v
        # cut b-d: triangles a, b, d and b, c, d
        return za + (zb - za) * u + (zd - za) * v if u + v <= 1.0 else zb + (zc - zb) * v + (zd - zc) * (1.0 - u)
    return at
