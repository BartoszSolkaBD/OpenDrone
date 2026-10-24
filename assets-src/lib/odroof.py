"""Sagging, damaged roofs of corrugated sheets (MIT OR Apache-2.0).

A roof that has stood unrepaired for years is not a plane. Its purlins sag
between the trusses, each sheet has lifted at an edge or curled at the eave or
torn off the ridge, some sheets are gone, and the skylights are broken with
ragged edges. `Roof` makes one slope of that, in plain Python, the same bytes
on every computer (`odnoise`, `odcut`, `odgeom`).

```python
south, north = Roof.gable(
    -15, -15, 15, 15, eave_z=6.0, ridge_z=8.5, seed=4,
    south=dict(missing=[3], skylights=[Skylight(6, 7, 3.0, 5.5, "broken"), Skylight(14, 15.2, 3.0, 5.5, "gone")]),
    north=dict(missing=[11], tears=[Tear(10.0, 2.0, 0.6)]),
)
for roof in (south, north):
    m.solid("roof", roof.iron(), None, "roof_corrugated_rusted", "none")
    m.solid("roof_glass", roof.glass(), None, "glass_wired_grimy", "none")
    m.solid("roof_collision", roof.collision_mesh(), None, "roof_corrugated_rusted", "trimesh", collision_only=True)
```

How a slope is laid out: `a` runs along the eave (0 at the slope's first end)
and `s` up the slope from the eave (0) to the ridge (`slope_length`). Trusses
stand at every `truss_spacing` along `a`; the roof is held up at each and
sags between them by `sag` metres (each bay by its own share of it). Sheets
are `sheet_width` wide, run from eave to ridge and lap over the next one.
`point(a, s)` gives the place on the plane roof, for a truss or a purlin.

What each sheet is given, from the seed: a lift of its exposed edge, a curl
of the eave end and, for some, a torn flap at the ridge end. Holes (missing
sheets, skylights, tears) are cut with `odcut.clip_grid`, so their edges are
cut triangles, and ragged where the hole function is.

The triangle cost is the grid's, per square metre of roof: a sheet with
`step` metres between corners is `2 / step**2` triangles a square metre for
one skin, twice that and a rim for a sheet with a thickness. At the default
0.45 m step a sheet with no hole costs 9 for an open sheet and 32 for a solid
one with its laps; a sheet with a tear in it is cut on a `hole_step` of 0.2 m
(about 5 times as many) so the tear's edge is ragged.
"""
import math

import odgeom as g
from odcut import clip_grid, merge, shell
from odnoise import Random, noise2, smoothstep

OVERLAP = 0.12  # a sheet reaches this far under the next
LAP_RISE = 0.035  # the next sheet's edge rides this much above it


class Skylight:
    """A skylight panel in the roof, `a0..a1` by `s0..s1`: the iron has a
    straight-edged opening there, and a panel of glass fills it. `state` is
    `"intact"`, `"broken"` (the panel has a ragged hole, most of its size) or
    `"gone"` (no panel at all)."""

    def __init__(self, a0, a1, s0, s1, state="broken"):
        assert state in ("intact", "broken", "gone"), state
        self.a0, self.a1, self.s0, self.s1, self.state = a0, a1, s0, s1, state


class Tear:
    """A ragged hole torn in the iron, `radius` metres across, centred at
    `(a, s)`."""

    def __init__(self, a, s, radius):
        self.a, self.s, self.radius = a, s, radius


def rect_distance(x, y, x0, y0, x1, y1):
    """The distance from `(x, y)` to the rectangle: negative inside it."""
    cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
    hx, hy = (x1 - x0) / 2.0, (y1 - y0) / 2.0
    dx, dy = abs(x - cx) - hx, abs(y - cy) - hy
    outside = math.sqrt(max(dx, 0.0) * max(dx, 0.0) + max(dy, 0.0) * max(dy, 0.0))
    return outside + min(max(dx, dy), 0.0)


def ragged(seed, x, y, size):
    """A number from -1 to 1 that changes about every `size` metres, mixed
    from two sizes of noise, to roughen the edge of a hole with."""
    return 0.7 * noise2(seed, x / size, y / size) + 0.3 * noise2(seed + 5, x / (size * 0.4), y / (size * 0.4))


class Roof:
    """One slope of a roof. See the module's note for the layout.

    - `origin`: `(x, y, z)` of the eave's first end; `heading`: the compass
      heading the eave runs along from there; the slope rises to the left of
      that heading (so an eave running east rises to the north).
    - `length`: the eave's length; `run`: the slope's horizontal depth;
      `rise`: how much higher the ridge is than the eave.
    - `sag`: how far a bay sags at most, `lift`, `curl` and `tear` how far a
      sheet's edge, eave end and ridge flap rise at most.
    - `missing`: sheets that are gone, by number from the first end (0, 1,
      ...). `bays`: `{bay: factor}`, a bay that sags `factor` times its share
      (a bay that has fallen in is 8).
    - `thick`: the sheet's thickness; 0 makes it an open sheet.
    - `step`: the grid, metres between corners; `hole_step`: the finer grid
      of a sheet that has a tear in it, so the tear's edge is ragged to that
      size (a skylight's straight edge is exact on any grid).
    """

    def __init__(self, origin, heading, length, run, rise, seed, truss_spacing=2.5, sheet_width=1.0,
                 sag=0.08, lift=0.12, curl=0.25, tear=0.4, missing=(), skylights=(), tears=(), bays=None,
                 thick=0.012, step=0.45, hole_step=0.2):
        self.origin, self.heading = origin, heading
        self.length, self.run, self.rise = length, run, rise
        self.slope_length = math.sqrt(run * run + rise * rise)
        self.seed, self.sag_max = seed, sag
        self.sheet_width, self.thick, self.step, self.hole_step = sheet_width, thick, step, hole_step
        self.lift_max, self.curl_max, self.tear_max = lift, curl, tear
        self.missing = set(missing)
        self.skylights, self.tears = list(skylights), list(tears)
        self.bay_count = max(1, round(length / truss_spacing))
        self.truss_spacing = length / self.bay_count
        rnd = Random(seed, "bays")
        self.bay_share = [rnd.uniform(0.55, 1.4) * (bays or {}).get(k, 1.0) for k in range(self.bay_count)]
        self.sheet_count = max(1, round(length / sheet_width))
        self.width = length / self.sheet_count
        # Each sheet's own faults, drawn in a fixed order from its own stream.
        self.faults = []
        for i in range(self.sheet_count):
            r = Random(seed, "sheet", i)
            self.faults.append({
                "lift": lift * r.uniform(0.2, 1.0) if r.chance(0.45) else 0.0,
                "curl": curl * r.uniform(0.3, 1.0) if r.chance(0.4) else 0.0,
                "tear": tear * r.uniform(0.4, 1.0) if r.chance(0.15) else 0.0,
                "tear_len": r.uniform(0.6, 1.4),
                "noise": r.bits() & 0xFFFF,
            })
        a = (math.sin(math.radians(heading)), math.cos(math.radians(heading)), 0.0)
        up = (-a[1], a[0], 0.0)  # left of the eave
        s3 = g.unit((up[0] * run, up[1] * run, rise))
        n = g.cross(a, s3)
        rows = ((a[0], s3[0], n[0]), (a[1], s3[1], n[1]), (a[2], s3[2], n[2]))
        self.pose = g.Pose(rows, origin)

    # ---------- the shape ----------
    def trusses(self):
        """The `a` of every truss, `bay_count + 1` of them."""
        return [k * self.truss_spacing for k in range(self.bay_count + 1)]

    def sag_at(self, a):
        """How far the purlins sag below the plane at `a` metres along the
        eave: 0 at a truss, deepest half-way between two."""
        k = min(int(a / self.truss_spacing), self.bay_count - 1)
        t = (a - k * self.truss_spacing) / self.truss_spacing
        return self.sag_max * self.bay_share[k] * 4.0 * t * (1.0 - t)

    def point(self, a, s, dz=0.0):
        """The place in the Map at `a` along the eave and `s` up the slope on
        the plane roof (no sag), `dz` above it along the roof's normal."""
        return self.pose.apply((a, s, dz))

    def _sheet_dz(self, i, a, s):
        f = self.faults[i]
        al = a - i * self.width
        z = -self.sag_at(a) + LAP_RISE * (1.0 - min(max(al / self.width, 0.0), 1.0))
        if f["curl"] and s < 0.7:
            z += f["curl"] * (1.0 - s / 0.7) * (1.0 - s / 0.7)
        if f["tear"]:
            tail = smoothstep((s - (self.slope_length - f["tear_len"])) / f["tear_len"])
            z += f["tear"] * tail * tail
        if f["lift"] and al < 0.35:
            z += f["lift"] * smoothstep((0.35 - al) / 0.35) * (0.45 + 0.55 * (0.5 + 0.5 * noise2(f["noise"], s / 1.3, 0.0)))
        return z

    # ---------- the holes ----------
    def _iron_hole(self, x, y):
        d = 1e9
        for sk in self.skylights:
            d = min(d, rect_distance(x, y, sk.a0, sk.s0, sk.a1, sk.s1))
        for k, t in enumerate(self.tears):
            r = t.radius * (1.0 + 0.4 * ragged(self.seed + 31 * k, x, y, max(0.25, t.radius * 0.7)))
            d = min(d, math.sqrt((x - t.a) * (x - t.a) + (y - t.s) * (y - t.s)) - r)
        return d

    def _gaps(self, x, y):
        """The distance to the gaps the missing sheets leave: from where the
        sheet before ends (its lap runs under the gone one's place) to where
        the sheet after begins."""
        d = 1e9
        for i in self.missing:
            d = min(d, rect_distance(x, y, i * self.width + OVERLAP, -1.0, (i + 1) * self.width, self.slope_length + 1.0))
        return d

    def _glass_hole(self, k, sk):
        w, h = sk.a1 - sk.a0, sk.s1 - sk.s0
        inset = 0.1 * min(w, h)
        shape = (sk.a0 + inset, sk.s0 + inset, sk.a1 - inset, sk.s1 - inset)
        amp = 0.18 * min(w, h)

        def hole(x, y):
            return rect_distance(x, y, *shape) + amp * ragged(self.seed + 977 * (k + 1), x, y, 0.45)

        return hole

    # ---------- the meshes ----------
    def _place(self, mesh):
        corners, faces = mesh
        return [self.pose.apply(c) for c in corners], faces

    def iron(self):
        """Every iron sheet, one mesh placed in the Map: lapped, sagging,
        lifted and curled, with the skylight openings and the tears cut out,
        and the missing sheets left out. Empty if none are left."""
        meshes = []
        for i in range(self.sheet_count):
            if i in self.missing:
                continue
            a0 = i * self.width
            a1 = min(self.length, a0 + self.width + OVERLAP)
            skylit = any(sk.a0 < a1 and sk.a1 > a0 for sk in self.skylights)
            torn = any(t.a - 1.5 * t.radius < a1 and t.a + 1.5 * t.radius > a0 for t in self.tears)
            hole = self._iron_hole if (skylit or torn) else None
            sheet = clip_grid(a0, 0.0, a1, self.slope_length, self.hole_step if torn else self.step,
                              lambda a, s, i=i: self._sheet_dz(i, a, s), hole)
            if sheet[1]:
                meshes.append(shell(sheet, (0.0, 0.0, -self.thick)) if self.thick else sheet)
        return self._place(merge(meshes)) if meshes else ([], [])

    def glass(self):
        """The skylight panels that are not gone, one mesh placed in the Map;
        a broken one has its ragged hole cut out."""
        meshes = []
        for k, sk in enumerate(self.skylights):
            if sk.state == "gone":
                continue
            hole = self._glass_hole(k, sk) if sk.state == "broken" else None
            panel = clip_grid(sk.a0, sk.s0, sk.a1, sk.s1, min(0.2, self.step),
                              lambda a, s: -self.sag_at(a) + 0.5 * LAP_RISE, hole)
            if panel[1]:
                meshes.append(shell(panel, (0.0, 0.0, -0.006)))
        return self._place(merge(meshes)) if meshes else ([], [])

    def collision_mesh(self, step=1.5):
        """A coarse open surface for a collision-only part: the roof's sag
        on a grid `step` apart, with the missing sheets, the skylight
        openings and the tears left open and the broken panels' holes shut
        (the Quad meets the glass that remains as it meets the iron)."""
        def hole(x, y):
            return min(self._iron_hole(x, y), self._gaps(x, y))

        sheets = [clip_grid(0.0, 0.0, self.length, self.slope_length, step, lambda a, s: -self.sag_at(a), hole)]
        # Panels that are not gone fill their openings again.
        for sk in self.skylights:
            if sk.state != "gone":
                sheets.append(clip_grid(sk.a0, sk.s0, sk.a1, sk.s1, step, lambda a, s: -self.sag_at(a)))
        return self._place(merge(sheets))

    @staticmethod
    def gable(x0, y0, x1, y1, eave_z, ridge_z, seed, south=None, north=None, **shared):
        """The two slopes of a gable roof over the rectangle, its ridge
        running east-west: `(south slope, north slope)`. `south` and `north`
        are each slope's own `missing`, `skylights`, `tears` and `bays`; the
        rest of the keywords (`sag`, `thick`, `step`, ...) go to both."""
        run, rise = (y1 - y0) / 2.0, ridge_z - eave_z
        a = Roof((x0, y0, eave_z), 90.0, x1 - x0, run, rise, seed * 2 + 1, **dict(shared, **(south or {})))
        b = Roof((x1, y1, eave_z), 270.0, x1 - x0, run, rise, seed * 2 + 2, **dict(shared, **(north or {})))
        return a, b
