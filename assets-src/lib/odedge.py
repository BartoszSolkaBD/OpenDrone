"""Material boundaries that wander: grass fading into dirt (MIT OR Apache-2.0).

Two ground materials that meet along a ruled line look cut with a knife. Real
grass creeps into dirt along an edge that wanders, gives way to bare patches
and sends out tongues. `WanderingEdge` is the **weight of material B** at any
point near a line: 0 is all material A, 1 is all material B, and the weight
changes across a band whose edge is bent by seeded noise, so the line the
weight crosses 0.5 on has no straight run.

Plain Python and the same numbers on every computer (`odnoise`: only the four
basic operations on 64-bit numbers). The weight is made a whole number of
255ths, so it is the same when it is written into a file as 8-bit vertex
colours, and read back.

How it reaches a Map: `weights(corners)` returns one weight for each corner of
a mesh, in the corners' order, for the vertex-colour blend the decals ticket
(#321) adds to the builder: `COLOR_0` of the mesh says how much of material B
shows through material A. Until that merges nothing writes them to the `.glb`
(the builder's `export_vertex_color` is still `"NONE"`), and
`test_odedge.py` checks the shape of the edge by numbers alone. The weight is a
vertex colour, so the mesh must have corners close enough to hold the band:
about `band / 2` apart or less (0.25 m for a 0.5 m band): `Ground.mesh(rect,
0.25)` in `odground` makes such a sheet, at 32 triangles a square metre.

```python
edge = WanderingEdge([(-20, 0), (0, 3), (20, 1)], seed=5, wander=1.4, band=0.6)   # B on the left of the line
mesh = ground.mesh((-20, -10, 20, 10), 0.25)                                       # corners every 0.25 m
w = edge.weights(mesh[0])                                                         # one weight per corner
```
"""
import math

from odnoise import fbm2, noise2, smoothstep


def _signed_distance(points, x, y):
    """The distance from `(x, y)` to the polyline, positive to its left."""
    best, side = 1e30, 1.0
    for (ax, ay), (bx, by) in zip(points, points[1:]):
        dx, dy = bx - ax, by - ay
        ll = dx * dx + dy * dy
        t = 0.0 if ll == 0.0 else max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / ll))
        px, py = ax + dx * t, ay + dy * t
        d = math.sqrt((x - px) * (x - px) + (y - py) * (y - py))
        if d < best:
            best = d
            side = 1.0 if dx * (y - ay) - dy * (x - ax) >= 0.0 else -1.0
    return best * side


class WanderingEdge:
    """The weight of material B near a line (see the module's note).

    - `line`: the polyline `[(x, y), ...]` the boundary follows on average.
      Material B is on its left, A on its right.
    - `wander`: how far the boundary strays from the line, in metres, at
      most about this much either way; `wavelength`: how long its swings are.
    - `band`: how wide the fade is across the boundary, in metres.
    - `patches`: how much the boundary is broken into tongues and islands,
      from 0 (none) to 1: a fine noise that pushes the weight toward 0 or 1
      inside the band.
    """

    def __init__(self, line, seed, wander=1.2, band=0.6, wavelength=7.0, patches=0.5):
        assert len(line) >= 2 and band > 0.0
        self.line, self.seed = [tuple(p) for p in line], seed
        self.wander, self.band, self.wavelength, self.patches = wander, band, wavelength, patches

    def offset(self, x, y):
        """How far the boundary has moved to the left of the line at this
        point: layers of noise, so it has swings of every size."""
        return self.wander * fbm2(self.seed, x, y, self.wavelength, octaves=4, gain=0.6)

    def weight(self, x, y):
        """The weight of material B at `(x, y)`, from 0 to 1, a whole number
        of 255ths."""
        s = _signed_distance(self.line, x, y) - self.offset(x, y)
        # Torn edges: the band is shifted by a fine noise, so the fade is
        # not a clean ramp.
        s += self.patches * 0.9 * self.band * noise2(self.seed + 9, x / 0.7, y / 0.7)
        w = smoothstep((s + self.band / 2.0) / self.band)
        return round(w * 255.0) / 255.0

    def weights(self, corners):
        """One weight for each corner of a mesh, in the corners' order."""
        return [self.weight(c[0], c[1]) for c in corners]

    def crossing(self, x, y0, y1, level=0.5):
        """Where the weight crosses `level` along the line `x`, going from
        `y0` (the weight there is below the level) to `y1` (above it), found
        by halving; for checking the shape of the edge."""
        lo, hi = y0, y1
        for _ in range(48):
            mid = (lo + hi) / 2.0
            if self.weight(x, mid) < level:
                lo = mid
            else:
                hi = mid
        return (lo + hi) / 2.0

