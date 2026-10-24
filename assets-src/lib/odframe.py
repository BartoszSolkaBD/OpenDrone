"""A broken roof frame and buckled sheets (#340, MIT OR Apache-2.0).

`odroof.Roof` makes the sheets of a slope that sags between its trusses.
Seen against the sky, an old works' roof is more broken than that: the
sheets dip between the purlins and buckle where they were walked on or hit,
and where the sheets are gone the steel under them shows, its purlins bent
down in the middle, some hanging by one end, some gone, the trusses' web
members missing here and there. `BuckledRoof` is a `Roof` whose sheets also
buckle, and `Frame` is the steel under a gable of two of them. Plain Python,
the same bytes on every computer (`odnoise`, `odgeom`).

```python
south, north = BuckledRoof.gable(x0, y0, x1, y1, eave_z=6.0, ridge_z=7.6, seed=4, buckle=0.06,
                                 south=dict(missing=range(4, 11)), north=dict(missing=[2, 3]))
frame = Frame(south, north, seed=4)
m.solid("frame", frame.mesh(), None, "steel_beam_rusted", "none")
for p0, p1, w, h in frame.colliders():
    m.beam("frame_solid", p0, p1, w, h, "steel_beam_rusted", collision_only=True)
```

- **Purlins** run along the roof every `purlin_spacing` up the slope,
  one length per bay between two trusses. Under sheets they sag as the
  sheets do (`Roof.sag_at`). Under a bay whose sheets are all gone each
  may, from the seed, be bent down in the middle by up to `bend`, hang by
  one end, or be gone.
- **Trusses** stand at every `Roof.trusses()`: a top chord under each
  slope, a bottom chord across that sags a little, and a web of verticals
  and diagonals. In an open bay's trusses some web members are gone.
- **Keep-clear zones**: `clear(x, y)` (`odscatter.Scatter.clearance`, #339)
  says where the flying lines are: a purlin over one is never bent or left
  hanging, so nothing drops into the line from above.
- **Colliders**: `colliders()` gives the straight members the Quad should
  meet (the chords and the purlins that are still straight) as
  `(p0, p1, w, h)` for `MapBuilder.beam(..., collision_only=True)`. Bent and
  hanging members are only drawn.

The cost is about 10 triangles a straight member and 10 for every bend
segment of a bent one; `Frame.stats()` says them.
"""
import math

import odgeom as g
from odcut import merge
from odkit import bar
from odnoise import Random, noise2, smoothstep
from odroof import Roof


class BuckledRoof(Roof):
    """A `Roof` whose sheets also dip between the purlins (`dip` metres at
    most) and buckle in dents and ridges up to `buckle` metres, each sheet in
    its own way. `purlin_spacing` must be the frame's."""

    def __init__(self, *args, buckle=0.05, dip=0.025, purlin_spacing=1.4, **kw):
        Roof.__init__(self, *args, **kw)
        self.buckle, self.dip, self.purlin_spacing = buckle, dip, purlin_spacing

    def purlins(self):
        """The `s` of every purlin up the slope, from the eave to the ridge."""
        n = max(1, round(self.slope_length / self.purlin_spacing))
        return [self.slope_length * k / n for k in range(n + 1)]

    def _sheet_dz(self, i, a, s):
        z = Roof._sheet_dz(self, i, a, s)
        n = max(1, round(self.slope_length / self.purlin_spacing))
        t = s / self.slope_length * n
        t -= math.floor(t)
        z -= self.dip * 4.0 * t * (1.0 - t)
        f = self.faults[i]
        dent = noise2(f["noise"] + 101, a / 0.8, s / 1.1)
        z -= self.buckle * smoothstep((dent - 0.15) / 0.6)
        return z

    @staticmethod
    def gable(x0, y0, x1, y1, eave_z, ridge_z, seed, south=None, north=None, **shared):
        """As `Roof.gable`, of `BuckledRoof`s."""
        run, rise = (y1 - y0) / 2.0, ridge_z - eave_z
        a = BuckledRoof((x0, y0, eave_z), 90.0, x1 - x0, run, rise, seed * 2 + 1, **dict(shared, **(south or {})))
        b = BuckledRoof((x1, y1, eave_z), 270.0, x1 - x0, run, rise, seed * 2 + 2, **dict(shared, **(north or {})))
        return a, b


class Frame:
    """The steel under a gable of two `BuckledRoof`s from `BuckledRoof.gable`
    (see the module's note)."""

    def __init__(self, south, north, seed, purlin=(0.10, 0.16), chord=(0.16, 0.26), web=(0.08, 0.08), panels=6,
                 bend=0.45, hang=0.25, gone=0.2, web_gone=0.3, chord_sag=0.06, clear=None):
        self.south, self.north, self.seed = south, north, seed
        self.members = []  # (kind, p0, p1, w, h, bend function or None, straight)
        rnd = Random(seed, "frame")
        drop = -(purlin[1] / 2.0 + 0.01)  # a purlin's middle, under the sheets
        # ---------- purlins ----------
        for side, roof in (("south", south), ("north", north)):
            spans = roof.trusses()
            for k in range(len(spans) - 1):
                a0, a1 = spans[k], spans[k + 1]
                open_bay = all(i in roof.missing for i in range(roof.sheet_count)
                               if i * roof.width < a1 - 0.05 and (i + 1) * roof.width > a0 + 0.05)
                for s in roof.purlins():
                    r = rnd.child("purlin", side, k, round(s * 1000))
                    p0, p1 = roof.point(a0, s, drop), roof.point(a1, s, drop)
                    sag = [roof.sag_at(a0 + (a1 - a0) * t / 6.0) for t in range(7)]
                    if open_bay and r.chance(gone):
                        continue
                    # Over a keep-clear zone a purlin may be gone but never
                    # bent or hanging down into the flying line.
                    # The span is sampled every 0.25 m, so a zone as narrow as
                    # a doorway cannot slip between the samples.
                    steps = max(1, math.ceil(g.length(g.sub(p1, p0)) / 0.25))
                    over_clear = clear is not None and any(
                        clear(p0[0] + (p1[0] - p0[0]) * i / steps, p0[1] + (p1[1] - p0[1]) * i / steps) <= 0.0
                        for i in range(steps + 1))
                    if open_bay and r.chance(hang) and not over_clear:
                        # Hanging by one end: swung down about that end.
                        if r.chance(0.5):
                            p0, p1 = p1, p0
                        length = g.length(g.sub(p1, p0))
                        down = length * r.uniform(0.35, 0.8)
                        across = math.sqrt(length * length - down * down)
                        d = g.unit((p1[0] - p0[0], p1[1] - p0[1], 0.0))
                        end = (p0[0] + d[0] * across, p0[1] + d[1] * across, p0[2] - down)
                        self.members.append(("purlin", p0, end, purlin[0], purlin[1], None, False))
                        continue
                    extra = r.uniform(0.3, 1.0) * bend if open_bay and r.chance(0.6) and not over_clear else 0.0
                    twist = r.uniform(-0.3, 0.3) * extra

                    def f(x, length, sag=sag, extra=extra, twist=twist):
                        t = x / length
                        j = min(int(t * 6.0), 5)
                        u = t * 6.0 - j
                        base = sag[j] + (sag[j + 1] - sag[j]) * u
                        b = 4.0 * t * (1.0 - t)
                        return twist * b, -base - extra * b
                    self.members.append(("purlin", p0, p1, purlin[0], purlin[1], f, extra == 0.0))
        # ---------- trusses ----------
        for k, a in enumerate(south.trusses()):
            r = rnd.child("truss", k)
            an = north.length - a
            open_near = any(self._open(roof, aa) for roof, aa in ((south, a), (north, an)))
            top_drop = drop * 2.0 - chord[1] / 2.0
            eave_s = south.point(a, 0.0, top_drop)
            ridge = south.point(a, south.slope_length, top_drop)
            eave_n = north.point(an, 0.0, top_drop)
            # The bottom chord ties the two eaves, a little below the top chord's feet.
            foot_s = (eave_s[0], eave_s[1], eave_s[2] - 0.05)
            foot_n = (eave_n[0], eave_n[1], eave_n[2] - 0.05)
            self.members.append(("chord", eave_s, ridge, chord[0], chord[1], None, True))
            self.members.append(("chord", ridge, eave_n, chord[0], chord[1], None, True))
            sag = chord_sag * r.uniform(0.4, 1.2)

            def tie(x, length, sag=sag):
                t = x / length
                return 0.0, -sag * 4.0 * t * (1.0 - t)
            self.members.append(("chord", foot_s, foot_n, chord[0] * 0.8, chord[1] * 0.6, tie, True))
            # The web: verticals and diagonals between the chords.
            for p in range(1, panels):
                t = p / panels
                top = g.lerp(eave_s, ridge, t * 2.0) if t <= 0.5 else g.lerp(ridge, eave_n, t * 2.0 - 1.0)
                bottom = g.lerp(foot_s, foot_n, t)
                bottom = (bottom[0], bottom[1], bottom[2] - sag * 4.0 * t * (1.0 - t))
                prev_t = (p - 1) / panels if t <= 0.5 else (p + 1) / panels
                pb = g.lerp(foot_s, foot_n, prev_t)
                pb = (pb[0], pb[1], pb[2] - sag * 4.0 * prev_t * (1.0 - prev_t))
                for kind, q0, q1 in (("web", bottom, top), ("web", pb, top)):
                    if g.length(g.sub(q1, q0)) < 0.2:
                        continue
                    if open_near and r.chance(web_gone):
                        continue
                    self.members.append((kind, q0, q1, web[0], web[1], None, False))

    @staticmethod
    def _open(roof, a):
        i = int(a / roof.width)
        return any(j in roof.missing for j in (i - 1, i, i + 1))

    def mesh(self):
        meshes = []
        for kind, p0, p1, w, h, f, _ in self.members:
            length = g.length(g.sub(p1, p0))
            if f is None:
                m = bar(length, w, h)
            else:
                m = bar(length, w, h, segments=6, bend=lambda x, f=f, length=length: f(x, length))
            pose = g.aiming(p0, p1)
            meshes.append(g.placed(m, pose))
        return merge(meshes)

    def colliders(self):
        return [(p0, p1, w, h) for kind, p0, p1, w, h, f, straight in self.members if straight]

    def stats(self):
        mesh = self.mesh()
        kinds = {}
        for kind, *_ in self.members:
            kinds[kind] = kinds.get(kind, 0) + 1
        return {"members": kinds, "triangles": sum(len(f) - 2 for f in mesh[1])}
