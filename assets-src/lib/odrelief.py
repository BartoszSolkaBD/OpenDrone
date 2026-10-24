"""Real relief for ground and floors (#340, MIT OR Apache-2.0).

`odground.Ground` makes ground that rolls. A derelict works' ground does
more: lorries have cut ruts in the yard, potholes hold water, the ground
rises under a heap of rubble, the floor round a column or a machine's plinth
is worn smooth and a little dished, and a hall whose ceiling came down has a
thick layer of crumbled plaster and debris over its floor, deep enough to read
as relief and not as a picture. `Relief` adds those to any height function,
and `graded_mesh` draws the result with small cells where the Quad flies low
and big ones elsewhere. Plain Python, the same bytes on every computer
(`odnoise`, `odgeom`): only `+ - * /`, `floor` and `sqrt`.

```python
floor = Ground(8, amplitude=0.012, wavelength=5.0)
r = Relief(floor.height, seed=8, clear=scatter.clearance)    # #339's keep-clear zones
r.crumble((-10, -6, 10, 8), depth=0.10)                      # plaster down over the hall
r.worn(4.0, 3.0, 0.6)                                         # round a column
r.under_heap(-5.5, 7.0, 2.6, 0.25)                           # the floor rises under a heap
mesh = graded_mesh((-10, -6, 10, 8), r.height, fine=[(-3, -6, 3, 8)], fine_step=0.2, coarse_step=0.8)
m.solid("floor", mesh, None, "concrete_floor_worn", "trimesh")
```

- **Keep-clear zones.** `clear` is a function giving how far a point is
  outside the nearest keep-clear zone (`odscatter.Scatter.clearance`, so the
  zones are the scatter's own, #339). Inside a zone the relief is held to
  `clear_rise` metres above the plain ground (a rut or a pothole may stay
  below it), easing in over
  `clear_band` metres outside it, so no lump rises into a flying line.
- **Range.** The relief never takes the ground more than `low` below or `high`
  above the plain ground (`Relief.lowest`, `Relief.highest`).
- **The cost.** A cell of `graded_mesh` is two triangles; a coarse block is a
  fan of four or more round its middle. Ground that is all fine costs `2 /
  fine_step**2` triangles a square metre (50 at 0.2 m, 22 at 0.3 m); ground
  that is all coarse about `4 / coarse_step**2` (6 at 0.8 m, 4 at 1 m).
"""
import math

from odnoise import fbm2, noise2, smoothstep


def _segment_distance(x, y, p0, p1):
    """Distance from `(x, y)` to the segment, and how far along it (0 to 1)
    the nearest point is."""
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    ll = dx * dx + dy * dy
    t = ((x - p0[0]) * dx + (y - p0[1]) * dy) / ll if ll > 0.0 else 0.0
    t = min(max(t, 0.0), 1.0)
    ex, ey = x - p0[0] - t * dx, y - p0[1] - t * dy
    return math.sqrt(ex * ex + ey * ey), t


def _rect_outside(x, y, rect):
    """Signed distance from `(x, y)` to the rectangle: below 0 inside."""
    x0, y0, x1, y1 = rect
    dx, dy = max(x0 - x, x - x1), max(y0 - y, y - y1)
    if dx > 0.0 or dy > 0.0:
        dx, dy = max(dx, 0.0), max(dy, 0.0)
        return math.sqrt(dx * dx + dy * dy)
    return max(dx, dy)


class Relief:
    """Relief over a plain height `base(x, y)`: see the module's note.

    Each feature adds to (or takes from) the plain ground; `height(x, y)`
    is the plain ground plus all of them, held in range and in the
    keep-clear zones. Features are evaluated in the order they were added,
    and each draws its noise from its own seed, so adding one never moves
    another."""

    def __init__(self, base, seed, clear=None, clear_rise=0.02, clear_band=0.6, low=0.3, high=0.6,
                 bumps=0.015, bump_wavelength=0.9):
        self.base, self.seed = base, seed
        self.clear = clear
        self.clear_rise, self.clear_band = clear_rise, clear_band
        self.lowest, self.highest = -low, high
        self.bump_amp, self.bump_wavelength = bumps, bump_wavelength
        self.adds = []  # (kind, function(x, y) -> metres)
        self.smooth = []  # functions(x, y) -> 0..1: how much the fine relief is worn away

    # ---------------------------------------------------------- features
    def rut(self, p0, p1, gauge=1.7, width=0.35, depth=0.07):
        """Two wheel ruts side by side, `gauge` apart centre to centre, from
        `p0` to `p1`: each `width` across and `depth` deep, with a low ridge
        of pushed-up dirt along its edges, shallower toward the ends."""
        seed = self.seed * 31 + 7 * len(self.adds) + 1
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        ll = math.sqrt(dx * dx + dy * dy)
        nx, ny = -dy / ll, dx / ll
        half = gauge / 2.0
        tracks = [((p0[0] + nx * s, p0[1] + ny * s), (p1[0] + nx * s, p1[1] + ny * s)) for s in (-half, half)]

        def f(x, y):
            total = 0.0
            for a, b in tracks:
                d, t = _segment_distance(x, y, a, b)
                if d > width * 1.4:
                    continue
                ends = smoothstep(t * ll / 2.0) * smoothstep((1.0 - t) * ll / 2.0)
                wobble = 0.7 + 0.3 * noise2(seed, t * ll / 1.7, 0.0)
                q = d / (width / 2.0)
                if q < 1.0:
                    total -= depth * wobble * ends * (1.0 - q * q) * (1.0 - q * q)
                else:  # the ridge pushed up at the rut's edges
                    r = (q - 1.0) / 0.8
                    total += 0.25 * depth * wobble * ends * max(0.0, 1.0 - r * r) if r < 1.0 else 0.0
            return total
        self.adds.append(("rut", f))

    def pothole(self, x, y, radius, depth):
        """A pothole: steep-sided where the surface broke away, `depth` deep
        across most of its `radius`, its rim ragged (unlike a `Dip`, which is
        a soft hollow)."""
        seed = self.seed * 31 + 7 * len(self.adds) + 2

        def f(px, py):
            dx, dy = px - x, py - y
            d = math.sqrt(dx * dx + dy * dy)
            if d > radius * 1.5:
                return 0.0
            rim = radius * (1.0 + 0.25 * noise2(seed, px / 0.35, py / 0.35))
            q = d / rim
            if q >= 1.0:
                return 0.0
            return -depth * (1.0 - smoothstep((q - 0.6) / 0.4)) * (0.85 + 0.15 * noise2(seed + 1, px / 0.2, py / 0.2))
        self.adds.append(("pothole", f))

    def worn(self, x, y, radius, depth=0.015, band=1.0):
        """A worn footprint round a column or a machine at `(x, y)`, `radius`
        across: the bumps and debris are worn away within `band` metres of
        it, and the floor is dished `depth` deep where feet and wheels went
        round it."""
        def wear(px, py):
            dx, dy = px - x, py - y
            d = math.sqrt(dx * dx + dy * dy) - radius
            return 1.0 - smoothstep(d / band) if d < band else 0.0

        def f(px, py):
            dx, dy = px - x, py - y
            d = math.sqrt(dx * dx + dy * dy) - radius
            if d < 0.0 or d > band:
                return 0.0
            t = d / band
            return -depth * 4.0 * t * (1.0 - t)
        self.smooth.append(wear)
        self.adds.append(("worn", f))

    def worn_rect(self, rect, depth=0.015, band=1.0):
        """`worn` round a rectangular plinth or machine base `(x0, y0, x1, y1)`."""
        def wear(px, py):
            d = _rect_outside(px, py, rect)
            return 1.0 - smoothstep(d / band) if d < band else 0.0

        def f(px, py):
            d = _rect_outside(px, py, rect)
            if d < 0.0 or d > band:
                return 0.0
            t = d / band
            return -depth * 4.0 * t * (1.0 - t)
        self.smooth.append(wear)
        self.adds.append(("worn", f))

    def under_heap(self, x, y, radius, height):
        """The ground rising under a heap: a broad, low swell `height` tall,
        reaching `radius` out, so a heap of rubble sits on a slope of its own
        spoil rather than on a flat floor."""
        seed = self.seed * 31 + 7 * len(self.adds) + 3

        def f(px, py):
            dx, dy = px - x, py - y
            q = (dx * dx + dy * dy) / (radius * radius)
            if q >= 1.0:
                return 0.0
            return height * (1.0 - q) * (1.0 - q) * (0.8 + 0.2 * noise2(seed, px / 0.8, py / 0.8))
        self.adds.append(("under_heap", f))

    def crumble(self, rect, depth=0.10, grain=0.45, cover=0.75, edge=0.6, walls=None, wall_depth=0.12):
        """A thick layer of crumbled plaster and debris over `rect` (x0, y0,
        x1, y1): lumps and drifts up to about `depth` deep, `grain` metres
        across, covering about `cover` of the floor (bare floor shows
        between the drifts). It thins out over `edge` metres at the
        rectangle's sides. `walls` is a list of segments `(p0, p1)` along
        which the layer is thicker by up to `wall_depth`, as it lies where a
        ceiling and a wall's render came down together."""
        seed = self.seed * 31 + 7 * len(self.adds) + 4
        walls = list(walls or ())
        threshold = 0.45 - 1.1 * cover  # the drift noise is above this on about `cover` of the floor

        def f(x, y):
            inside = -_rect_outside(x, y, rect)
            if inside <= 0.0:
                return 0.0
            fade = smoothstep(inside / edge)
            # Where the layer lies: big drifts with bare floor between them.
            drift = fbm2(seed, x, y, grain * 5.0, octaves=2, gain=0.5)
            where = smoothstep((drift - threshold) / 0.35)
            for p0, p1 in walls:
                d, _ = _segment_distance(x, y, p0, p1)
                if d < 1.2:
                    where = max(where, 1.0 - d / 1.2)
            if where <= 0.0:
                return 0.0
            # Its lumps: fragments `grain` across, sharp-topped.
            n = 0.65 * noise2(seed + 5, x / grain, y / grain) + 0.35 * noise2(seed + 9, x / (grain * 0.4), y / (grain * 0.4))
            lump = 0.5 + 0.5 * n
            layer = depth * where * (0.3 + 0.7 * lump * lump)
            for p0, p1 in walls:
                d, _ = _segment_distance(x, y, p0, p1)
                if d < 1.2:
                    q = 1.0 - d / 1.2
                    layer += wall_depth * q * q * (0.6 + 0.4 * lump)
            return layer * fade
        self.adds.append(("crumble", f))

    # ---------------------------------------------------------- the height
    def relief(self, x, y):
        """How far the relief takes the ground from the plain ground, in
        metres, held in range and in the keep-clear zones."""
        return self._relief(x, y)[0]

    def layer(self, x, y):
        """How thick the crumbled layer (`crumble`) is at `(x, y)`, in
        metres, after wear and the keep-clear zones: for a blend weight, so
        the layer shows a picture of its own."""
        return self._relief(x, y)[1]

    def blend_weights(self, corners, full=0.04):
        """A weight from 0 to 1 for each corner, of a mesh of this ground: 1
        where the crumbled layer is `full` metres thick or more, for
        `MapBuilder.solid(..., blend=(material, weights))`."""
        return [min(1.0, self.layer(x, y) / full) for x, y, _ in corners]

    def _relief(self, x, y):
        worn = 0.0
        for w in self.smooth:
            worn = max(worn, w(x, y))
        r = 0.0
        if self.bump_amp:
            r += self.bump_amp * fbm2(self.seed * 31 + 999, x, y, self.bump_wavelength, octaves=3, gain=0.5) * (1.0 - worn)
        crumbled = 0.0
        for kind, f in self.adds:
            v = f(x, y)
            if kind == "crumble":
                v *= 1.0 - worn
                crumbled += v
            r += v
        r = min(max(r, self.lowest), self.highest)
        if self.clear is not None:
            d = self.clear(x, y)
            if d < self.clear_band:
                # Inside a zone the relief rises clear_rise at most; it eases
                # out to the full relief over clear_band metres outside.
                hold = 1.0 - smoothstep(d / self.clear_band) if d > 0.0 else 1.0
                held = min(r, self.clear_rise)  # a rut or a pothole may stay
                r = r + (held - r) * hold
                crumbled *= 1.0 - hold
        return r, max(0.0, crumbled)

    def height(self, x, y):
        return self.base(x, y) + self.relief(x, y)


# ---------------------------------------------------------------- the mesh

def graded_mesh(rect, height, fine=(), fine_step=0.25, coarse_step=1.0):
    """The ground `height(x, y)` as a sheet over `rect` (x0, y0, x1, y1),
    facing up: cells `fine_step` across inside the rectangles in `fine`
    (and the blocks they touch), blocks `coarse_step` across elsewhere.

    The rectangle is cut into blocks about `coarse_step` square, each into
    `k` x `k` cells of `fine_step` (k = coarse_step / fine_step, rounded). A
    fine block is a grid of those cells, two triangles each, cut along the
    diagonal that changes height least. A coarse block is a fan round its
    middle through its four corners, and through every corner of a fine
    neighbour along their shared edge, so the sheet has no crack where fine
    meets coarse."""
    x0, y0, x1, y1 = rect
    k = max(1, round(coarse_step / fine_step))
    nbx = max(1, round((x1 - x0) / coarse_step))
    nby = max(1, round((y1 - y0) / coarse_step))
    nx, ny = nbx * k, nby * k

    def gx(i):
        return x0 + (x1 - x0) * i / nx

    def gy(j):
        return y0 + (y1 - y0) * j / ny

    is_fine = []
    for bj in range(nby):
        row = []
        for bi in range(nbx):
            bx0, bx1, by0, by1 = gx(bi * k), gx((bi + 1) * k), gy(bj * k), gy((bj + 1) * k)
            row.append(any(fx0 < bx1 and fx1 > bx0 and fy0 < by1 and fy1 > by0 for fx0, fy0, fx1, fy1 in fine))
        is_fine.append(row)

    def fine_block(bi, bj):
        return 0 <= bi < nbx and 0 <= bj < nby and is_fine[bj][bi]

    corners, index, faces = [], {}, []

    def vid(i, j):
        key = (i, j)
        if key not in index:
            x, y = gx(i), gy(j)
            index[key] = len(corners)
            corners.append((x, y, height(x, y)))
        return index[key]

    for bj in range(nby):
        for bi in range(nbx):
            i0, j0 = bi * k, bj * k
            if is_fine[bj][bi]:
                for j in range(j0, j0 + k):
                    for i in range(i0, i0 + k):
                        a, b, c, d = vid(i, j), vid(i + 1, j), vid(i + 1, j + 1), vid(i, j + 1)
                        if abs(corners[a][2] - corners[c][2]) <= abs(corners[b][2] - corners[d][2]):
                            faces += [(a, b, c), (a, c, d)]
                        else:
                            faces += [(a, b, d), (b, c, d)]
                continue
            # The loop round a coarse block, anticlockwise from its south-west
            # corner, with every fine corner along an edge it shares with a
            # fine block.
            loop = []
            south = range(0, k) if fine_block(bi, bj - 1) else [0]
            loop += [vid(i0 + t, j0) for t in south]
            east = range(0, k) if fine_block(bi + 1, bj) else [0]
            loop += [vid(i0 + k, j0 + t) for t in east]
            north = range(0, k) if fine_block(bi, bj + 1) else [0]
            loop += [vid(i0 + k - t, j0 + k) for t in north]
            west = range(0, k) if fine_block(bi - 1, bj) else [0]
            loop += [vid(i0, j0 + k - t) for t in west]
            cx, cy = (gx(i0) + gx(i0 + k)) / 2.0, (gy(j0) + gy(j0 + k)) / 2.0
            centre = len(corners)
            corners.append((cx, cy, height(cx, cy)))
            for t in range(len(loop)):
                faces.append((centre, loop[t], loop[(t + 1) % len(loop)]))
    return corners, faces


def fine_share(rect, fine, coarse_step=1.0):
    """How much of `rect`'s area `graded_mesh` draws fine, 0 to 1 (for the
    cost report)."""
    x0, y0, x1, y1 = rect
    nbx = max(1, round((x1 - x0) / coarse_step))
    nby = max(1, round((y1 - y0) / coarse_step))
    n = 0
    for bj in range(nby):
        for bi in range(nbx):
            bx0, bx1 = x0 + (x1 - x0) * bi / nbx, x0 + (x1 - x0) * (bi + 1) / nbx
            by0, by1 = y0 + (y1 - y0) * bj / nby, y0 + (y1 - y0) * (bj + 1) / nby
            if any(fx0 < bx1 and fx1 > bx0 and fy0 < by1 and fy1 > by0 for fx0, fy0, fx1, fy1 in fine):
                n += 1
    return n / (nbx * nby)
