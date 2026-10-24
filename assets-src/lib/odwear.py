"""Rules that put wear on a Map (#323, MIT OR Apache-2.0).

A Map that is only geometry and repeating pictures looks new. Wear (water
runs, moss, rust, soot, scuffs, droppings) is what a lived-in place has, and
it is never even: it gathers where a force put it. Each rule here places
decals (`MapBuilder.decal`, `oddecal.py`) for one such cause, from the table
"Where things collect" in docs/research/lived-in-places.md, with a seeded
random choice, so the Map is the same every build.

```python
wear = Wear(seed=7, level=0.6, sun=(-0.7, -0.7))     # level 0 to 1; the sun is in the south-west
south = Face("south", (7.0, -4.0), (-7.0, -4.0), 0.0, 4.5, [south_wall], openings=ops)
yard = Floor([yard_ground], yard.height)
wear.runs_below_openings(south)                      # water runs below each sill and breach
wear.runs_from_edge(south, gutters=[11.0])           # and from the roof edge, a broken gutter in particular
wear.moss_at_foot(south, yard, damp=[10.4])          # moss where the wall is damp or in the shade
wear.rust_below([(3.0, -4.0, 2.2)], south.facing, [south_wall])
wear.scuff_paths([(0.0, -12.0), (2.0, -5.0)], Floor([hall_floor]))
wear.hand("tag_door", "decal_tag", [south_wall], (4.0, -4.0, 1.4), south.facing3, (0, 0, 1), (1.0, 1.0), layer=2)
wear.keep_clear((8.0, -5.0, 0.0), (12.0, -3.0, 3.0))   # no wear by rule in this box
wear.apply(m)                                        # puts every decal on the Map
```

A rule's choices come from `Random(seed, rule, surface, ...)`, so adding one
rule, or one surface, never moves what another rule placed. `level` (0 to
1) is how worn the place is: 0 puts nothing, and a higher level makes each
rule more likely to fire, and its marks bigger. Every rule keeps to the
placement it names, which the tests in `test_odwear.py` check: no moss on a
dry wall that faces the sun, a run always starts at an opening or an edge, a
tag only at arm's reach next to a way in.

The layers (`LAYER`) are few on purpose, because every layer is a separate
pipeline in the game: the damp and the runs under, then what stains, then
marks of people, then growing things, then loose and scratched things.

Plain Python on 64-bit numbers with `+ - * /` and `sqrt` (`odgeom`), so every
decal's place is the same on every computer. Nothing here needs Blender
until `apply`.
"""
import odgeom as g
from odnoise import Random

# The layer each kind of wear is drawn on: a higher layer is over a lower one.
LAYER = {
    "damp": 0, "runs": 0,
    "algae": 1, "salt": 1, "rust": 1, "oil": 1, "soot": 1,
    "tag": 2, "stencil": 2,
    "moss": 3, "lichen": 3,
    "scuffs": 4, "scratches": 4, "cracks": 4, "flakes": 4, "droppings": 4, "cobweb": 4,
}
MATERIAL = {kind: "decal_" + kind for kind in LAYER}
MATERIAL["rust"] = "decal_rust_bleed"

DRY_SHADE = 0.3  # a wall less shaded than this is dry: no moss, no algae
UP = (0.0, 0.0, 1.0)
SOURCE = 0.12  # how far down its picture a rust bleed's fixing is (`decal_rust_bleed`)
RUN_OVERLAP = 0.05  # a run's picture starts this share of its height above the edge, under its faded top


class Floor:
    """A floor or a ground the rules lay things on: the shapes a decal may lie
    on, and the height of it at `(x, y)` (flat at 0 if no function is given)."""

    def __init__(self, hosts, height=None):
        self.hosts = list(hosts)
        self.height = height if height is not None else (lambda x, y: 0.0)


class Face:
    """One vertical face of a wall, the face that has the wear: `p0` to `p1`
    (x, y) is the way you walk along it with its front on your left, as for
    `broken_wall(...).pose(p0, p1)`; it rises `height` metres from `z0`.
    `hosts` are the shapes a decal on it may lie on, `openings` the wall's
    `Opening`s (anything with `u0 u1 v0 v1 kind`), `painted` says the wall
    is painted (paint flakes off it)."""

    def __init__(self, name, p0, p1, z0, height, hosts, openings=(), painted=False):
        self.name, self.z0, self.height = name, float(z0), float(height)
        self.p0, self.p1 = (float(p0[0]), float(p0[1])), (float(p1[0]), float(p1[1]))
        self.hosts, self.openings, self.painted = list(hosts), list(openings), painted
        dx, dy = self.p1[0] - self.p0[0], self.p1[1] - self.p0[1]
        self.length = g.length((dx, dy, 0.0))
        assert self.length > 0.0, f"face {name} has no length"
        self.along = (dx / self.length, dy / self.length)
        self.facing = (-self.along[1], self.along[0])  # left of the way you walk
        self.facing3 = (self.facing[0], self.facing[1], 0.0)

    def xy(self, u, out=0.0):
        """The place `u` metres along the face and `out` metres out of it."""
        return (self.p0[0] + self.along[0] * u + self.facing[0] * out,
                self.p0[1] + self.along[1] * u + self.facing[1] * out)

    def point(self, u, z, out=0.0):
        x, y = self.xy(u, out)
        return (x, y, z)

    def blocked(self, u, z, w, h, margin=0.1):
        """Whether a `w` by `h` rectangle centred at `u` along and `z` high
        overlaps an opening (the wall has no surface there)."""
        for op in self.openings:
            if (u + w / 2 > op.u0 - margin and u - w / 2 < op.u1 + margin
                    and z + h / 2 > self.z0 + op.v0 - margin and z - h / 2 < self.z0 + op.v1 + margin):
                return True
        return False

    def doorway_at(self, u, margin=0.3):
        """Whether `u` is in a way that reaches the floor (a doorway, or any
        opening that starts below knee height)."""
        return any(op.u0 - margin < u < op.u1 + margin and op.v0 < 0.5 for op in self.openings)


class Spec:
    """One decal a rule decided on: what `MapBuilder.decal` is asked, and
    `kind` (the rule's), `why` (the cause, in words) and `meta` (numbers a
    check can read: where a run starts, which way the sun lies)."""

    def __init__(self, kind, name, material, hosts, centre, facing, up, size, layer, depth, why, meta=None, hand=False):
        self.kind, self.name, self.material, self.hosts = kind, name, material, list(hosts)
        self.centre = tuple(float(c) for c in centre)
        self.facing, self.up = tuple(float(c) for c in facing), tuple(float(c) for c in up)
        self.size = (float(size[0]), float(size[1]))
        self.layer, self.depth, self.why = layer, depth, why
        self.meta = meta or {}
        self.hand = hand


class Wear:
    """The rules, a wear `level` (0 to 1) and a `seed`. Each rule adds `Spec`s;
    `apply` puts them on a `MapBuilder`. `sun` is the horizontal direction to
    the sun, `(east, north)`: `(0, 1)` for a sun in the north, `(0.7071,
    0.7071)` in the north-east; leave it out and no wall counts as dry."""

    def __init__(self, seed, level, sun=None):
        assert 0.0 <= level <= 1.0, "wear level is 0 to 1"
        self.seed, self.level = seed, float(level)
        if sun is not None:
            length = g.length((sun[0], sun[1], 0.0))
            assert length > 0.0, "the sun's direction can't be zero"
            sun = (sun[0] / length, sun[1] / length)
        self.sun = sun
        self.scale = 0.8 + 0.4 * self.level  # how big a level makes its marks
        self.specs = []
        self.clear_boxes = []

    # ---------- the helpers every rule uses ----------

    def _rnd(self, *keys):
        return Random(self.seed, *keys)

    def shade(self, facing):
        """How shaded a surface is that faces the horizontal direction
        `facing`: 0 facing the sun, 1 facing away, 0.5 across it. 0.5 for
        every surface if the sun is not known."""
        if self.sun is None:
            return 0.5
        d = g.unit((facing[0], facing[1], 0.0))
        return 0.5 - 0.5 * (d[0] * self.sun[0] + d[1] * self.sun[1])

    def _add(self, kind, name, hosts, centre, facing, up, size, depth, why, meta=None, layer=None, material=None):
        if self.level == 0.0:
            return None  # a level of nothing is a clean Map
        spec = Spec(kind, f"{kind}_{name}", material or MATERIAL[kind], hosts, centre, facing, up, size,
                    LAYER[kind] if layer is None else layer, depth, why, meta)
        self.specs.append(spec)
        return spec

    def _spread(self, rnd, lo, hi, count, gap):
        """`count` numbers in `[lo, hi]` at least `gap` apart where they can be."""
        out = []
        for _ in range(count):
            for _try in range(12):
                x = rnd.uniform(lo, hi)
                if all(abs(x - o) >= gap for o in out):
                    out.append(x)
                    break
        return sorted(out)

    # ---------- water: runs, damp, algae ----------

    def runs_below_openings(self, face):
        """Water runs below each window sill and breach, down from its bottom
        edge: one run for each metre of width or so, spreading as it goes. A
        doorway has no sill, so none."""
        for k, op in enumerate(face.openings):
            if op.kind == "doorway":
                continue
            rnd = self._rnd("runs", face.name, k)
            if not rnd.chance(0.3 + 0.7 * self.level):
                continue
            width = op.u1 - op.u0
            for i in range(1 + int(width // 1.4)):
                w = rnd.uniform(0.8, 1.3) * self.scale
                h = rnd.uniform(1.5, 2.6) * self.scale
                u = rnd.uniform(op.u0 + 0.15, op.u1 - 0.15)
                self._run(face, f"{face.name}_{k}_{i}", u, face.z0 + op.v0, (w, h),
                          f"below the {op.kind} at {op.u0:.1f} to {op.u1:.1f} m", ("opening", k))

    def runs_from_edge(self, face, gutters=()):
        """Water runs down from the wall's top edge, where the roof sheds onto
        it: at each `gutters` position (a broken gutter, a roof's lowest
        corner) for certain, and at a few seeded places along it, more of them
        the higher the level."""
        rnd = self._rnd("edge", face.name)
        top = face.z0 + face.height
        spots = [("gutter", float(u)) for u in gutters]
        count = int(face.length / 4.0 * self.level + rnd.unit())
        for u in self._spread(rnd, 0.6, face.length - 0.6, count, 2.5):
            if all(abs(u - gu) > 1.5 for _kind, gu in spots):
                spots.append(("edge", u))
        for i, (kind, u) in enumerate(spots):
            w = rnd.uniform(0.9, 1.5) * self.scale
            h = rnd.uniform(1.8, 3.2) * self.scale
            self._run(face, f"{face.name}_edge_{i}", u, top, (w, h), f"under the roof edge ({kind})", ("edge", kind))

    def _run(self, face, name, u, start_z, size, why, source):
        w, h = size
        top = start_z + RUN_OVERLAP * h
        self._add("runs", name, face.hosts, face.point(u, top - h / 2.0), face.facing3, UP, size, 1.0, why,
                  dict(start_z=start_z, source=source, u=u))

    def damp_at(self, spots, floor):
        """Damp where water stands: a wet patch at each `(x, y, radius)`, a
        Dip's or a roof opening's, wider the higher the level, and, in the
        bigger ones of a worn place, a skin of algae in the middle."""
        for k, (x, y, r) in enumerate(spots):
            rnd = self._rnd("damp", k)
            if self.level == 0.0:
                continue
            w, h = 2.3 * r * self.scale, 1.7 * r * self.scale
            up = rnd.pick(((0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0)))
            z = floor.height(x, y)
            self._add("damp", f"{k}", floor.hosts, (x, y, z), UP, up, (w, h), 0.5, f"water stands at {x:.1f}, {y:.1f}",
                      dict(spot=(x, y, r)))
            if self.level >= 0.5 and r >= 1.0:
                a = (1.3 * r * self.scale, 1.1 * r * self.scale)
                self._add("algae", f"{k}", floor.hosts, (x + rnd.uniform(-0.2, 0.2) * r, y + rnd.uniform(-0.2, 0.2) * r, z),
                          UP, up, a, 0.5, f"algae in the damp at {x:.1f}, {y:.1f}", dict(spot=(x, y, r)))

    # ---------- growth: moss, salt, lichen ----------

    def moss_at_foot(self, face, floor, damp=()):
        """Moss along the wall's foot, lying on the ground and climbing the
        wall at once: where the wall is shaded, always more the darker it is;
        on a wall that faces the sun only near a `damp` position (an `u`
        along the face where water comes down or soil has built up), since a
        dry, sunny wall grows none."""
        shade = self.shade(face.facing)
        slots = max(1, int(face.length // 1.3))
        for i in range(slots):
            rnd = self._rnd("moss", face.name, i)
            u = (i + 0.5) * face.length / slots + rnd.uniform(-0.3, 0.3)
            near = any(abs(u - d) < 1.5 for d in damp)
            if shade < DRY_SHADE and not near:
                continue
            p = self.level * (0.25 + 0.75 * shade)
            if near:
                p = max(p, 0.8 * self.level)
            if not rnd.chance(p) or face.doorway_at(u):
                continue
            ox, oy = face.xy(u, -0.04)
            fx, fy = face.xy(u, 0.25)
            foot = floor.height(fx, fy)
            w = rnd.uniform(1.2, 1.8) * self.scale
            h = rnd.uniform(1.1, 1.5) * self.scale
            corner = g.unit((face.facing[0], face.facing[1], 1.0))
            self._add("moss", f"{face.name}_{i}", face.hosts + floor.hosts, (ox, oy, foot + 0.24), corner, UP, (w, h), 1.4,
                      f"damp low on {'a shaded' if shade >= DRY_SHADE else 'a damp'} wall", dict(shade=shade, u=u, damp=near))

    def moss_in_corners(self, corners, floor):
        """Moss in the inside corners at the foot where two walls meet: each
        corner is `(x, y, facing, hosts)`, `facing` the horizontal direction
        out of the corner. Shade decides as for a wall; a corner holds damp
        and soil, so it is a little more likely than a wall's foot."""
        for k, (x, y, facing, hosts) in enumerate(corners):
            shade = self.shade(facing)
            rnd = self._rnd("moss_corner", k)
            if shade < DRY_SHADE or not rnd.chance(min(1.0, self.level * (0.4 + 0.8 * shade))):
                continue
            d = g.unit((facing[0], facing[1], 0.0))
            foot = floor.height(x + d[0] * 0.25, y + d[1] * 0.25)
            w = rnd.uniform(1.3, 1.9) * self.scale
            self._add("moss", f"corner_{k}", list(hosts) + floor.hosts, (x - d[0] * 0.04, y - d[1] * 0.04, foot + 0.24),
                      g.unit((d[0], d[1], 1.0)), UP, (w, w * 0.9), 1.4, "damp in a shaded corner", dict(shade=shade))

    def salt_at_foot(self, face, floor, damp=()):
        """Efflorescence along the foot where damp rises: near a `damp`
        position, or on a shaded wall at a lower chance."""
        shade = self.shade(face.facing)
        slots = max(1, int(face.length // 1.6))
        for i in range(slots):
            rnd = self._rnd("salt", face.name, i)
            u = (i + 0.5) * face.length / slots + rnd.uniform(-0.3, 0.3)
            near = any(abs(u - d) < 1.5 for d in damp)
            p = (0.9 if near else 0.35 * shade) * self.level
            if not rnd.chance(p) or face.doorway_at(u):
                continue
            w, h = rnd.uniform(1.0, 1.5) * self.scale, rnd.uniform(0.8, 1.1) * self.scale
            fx, fy = face.xy(u, 0.2)
            self._add("salt", f"{face.name}_{i}", face.hosts, face.point(u, floor.height(fx, fy) + h / 2.0 - 0.02), face.facing3, UP,
                      (w, h), 0.8, "damp rising at the foot", dict(damp=near, shade=shade))

    def lichen_on(self, face):
        """Lichen on a wall that is mostly in shade (a lichen wants light but
        not a baked wall), in clumps at arm's reach to above the head, never
        over an opening."""
        shade = self.shade(face.facing)
        if shade < 0.5:
            return
        rnd = self._rnd("lichen", face.name)
        count = int(face.length * 0.25 * self.level * shade + rnd.unit())
        for i, u in enumerate(self._spread(rnd, 0.5, face.length - 0.5, count, 1.5)):
            z = face.z0 + rnd.uniform(0.7, min(2.6, face.height - 0.6))
            w = rnd.uniform(0.8, 1.4) * self.scale
            if face.blocked(u, z, w, w):
                continue
            self._add("lichen", f"{face.name}_{i}", face.hosts, face.point(u, z), face.facing3, UP, (w, w), 0.8,
                      "a shaded, still wall", dict(shade=shade))

    # ---------- steel: rust ----------

    def rust_below(self, fixings, facing, hosts):
        """Rust below each steel fixing, bolt, seam end or beam end: a bleed
        whose picture has its source (`SOURCE` of the way down) on the fixing,
        so the streaks run down from it. `fixings` are `(x, y, z)`, `facing`
        the direction out of the surface."""
        for k, (x, y, z) in enumerate(fixings):
            rnd = self._rnd("rust", k, round(x, 3), round(y, 3), round(z, 3))
            if not rnd.chance(0.35 + 0.65 * self.level):
                continue
            h = rnd.uniform(1.0, 1.9) * self.scale
            w = h * rnd.uniform(0.6, 0.9)
            self._add("rust", f"{k}", hosts, (x, y, z - (0.5 - SOURCE) * h), facing, UP, (w, h), 0.8,
                      "water carries rust down from a fixing", dict(fixing=(x, y, z)))

    def rust_seam(self, p0, p1, z, facing, hosts, spacing=0.9):
        """A bolt or rivet every `spacing` metres along a seam from `p0` to
        `p1` (x, y) at height `z`, each given to `rust_below`."""
        d = (p1[0] - p0[0], p1[1] - p0[1], 0.0)
        n = max(1, int(g.length(d) // spacing))
        self.rust_below([(p0[0] + d[0] * (i + 0.5) / n, p0[1] + d[1] * (i + 0.5) / n, z) for i in range(n)], facing, hosts)

    # ---------- fire: soot ----------

    def soot_above(self, face, u, flame_z, width=1.6):
        """A plume of soot up the wall from a fire at `u` along the face, the
        flame at `flame_z`: black just above it, fanning as it rises."""
        rnd = self._rnd("soot", face.name, round(u, 3))
        if self.level == 0.0:
            return
        w, h = width * self.scale, rnd.uniform(2.0, 2.6) * self.scale
        self._add("soot", f"{face.name}_{round(u, 2)}", face.hosts, face.point(u, flame_z + h / 2.0 - 0.05 * h), face.facing3, UP,
                  (w, h), 0.8, "smoke rises from a fire", dict(flame_z=flame_z, bottom=flame_z - 0.05 * h))

    def soot_over_door(self, face, index):
        """Soot above a burnt doorway: black over its lintel, where the smoke
        came out, fanning wider than the doorway."""
        op = face.openings[index]
        self.soot_above(face, (op.u0 + op.u1) / 2.0, face.z0 + op.v1 - 0.2, width=(op.u1 - op.u0) * 1.3 + 0.4)

    def soot_ceiling(self, x, y, z_ceiling, hosts):
        """Soot on the ceiling right above a fire at `(x, y)`: a wide, dark
        stain, blackest in the middle."""
        if self.level == 0.0:
            return
        s = 2.4 * self.scale
        self._add("soot", f"ceiling_{round(x, 2)}_{round(y, 2)}", hosts, (x, y, z_ceiling), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), (s, s),
                  0.8, "smoke gathers under the ceiling", dict(ceiling=True))

    # ---------- people: scuffs, scratches, tags ----------

    def scuff_paths(self, points, floor, width=1.0):
        """Scuffs worn along the shortest way between the places people use,
        the polyline `points` of `(x, y)` (door to Gap to door), more of them
        and more worn toward its ends, where people turn and slow."""
        for s in range(len(points) - 1):
            a, b = points[s], points[s + 1]
            d = (b[0] - a[0], b[1] - a[1], 0.0)
            length = g.length(d)
            if length == 0.0:
                continue
            dirn = (d[0] / length, d[1] / length)
            n = max(1, int(length // 0.9))
            for i in range(n):
                rnd = self._rnd("scuffs", s, i)
                t = (i + 0.5) / n
                ends = min(t * length, (1 - t) * length)
                nearness = max(0.0, 1.0 - ends / 3.0)
                if not rnd.chance(self.level * (0.55 + 0.45 * nearness)):
                    continue
                side = rnd.uniform(-0.15, 0.15) * width
                x = a[0] + dirn[0] * t * length - dirn[1] * side
                y = a[1] + dirn[1] * t * length + dirn[0] * side
                self._add("scuffs", f"{s}_{i}", floor.hosts, (x, y, floor.height(x, y)), UP, (dirn[0], dirn[1], 0.0),
                          (width * rnd.uniform(0.9, 1.2), 1.7), 0.5, "people walk here", dict(path=(a, b), ends=nearness))

    def scratches_low(self, face):
        """Scratches on a wall, thick at the height of a bag or a boot and
        thinning upward: heavier wear near the floor."""
        rnd = self._rnd("scratches", face.name)
        count = int(face.length * 0.5 * self.level + rnd.unit())
        for i in range(count):
            u = rnd.uniform(0.4, face.length - 0.4)
            z = face.z0 + 0.3 + 1.2 * rnd.unit() * rnd.unit()
            w = rnd.uniform(0.5, 0.8) * self.scale
            if face.blocked(u, z, w, w):
                continue
            self._add("scratches", f"{face.name}_{i}", face.hosts, face.point(u, z), face.facing3, UP, (w, w), 0.6,
                      "bags and boots at the foot of a wall", dict(z=z - face.z0))

    def marks_near_openings(self, face):
        """Tags and stencils beside the ways in, on the wall at arm's reach
        (0.5 to 2.2 m) where someone could stand, never over an opening;
        about one in four marks is a stencil."""
        for k, op in enumerate(face.openings):
            if op.v0 > 1.0:  # a window too high to stand at is no way in
                continue
            rnd = self._rnd("marks", face.name, k)
            for i, side in enumerate((-1, 1)):
                if not rnd.chance(0.5 * self.level):
                    continue
                kind = "stencil" if rnd.chance(0.25) else "tag"
                w = rnd.uniform(0.9, 1.3) * self.scale
                u = (op.u0 - 0.5 - w / 2.0) if side < 0 else (op.u1 + 0.5 + w / 2.0)
                z = face.z0 + rnd.uniform(1.0, 1.6)
                if u - w / 2 < 0.1 or u + w / 2 > face.length - 0.1 or face.blocked(u, z, w, w):
                    continue
                self._add(kind, f"{face.name}_{k}_{i}", face.hosts, face.point(u, z), face.facing3, UP, (w, w), 0.6,
                          "someone stood beside the way in", dict(z=z - face.z0, opening=k))

    # ---------- age: cracks, flakes, cobwebs, droppings, oil ----------

    def cracks_at_openings(self, face):
        """Cracks running up and out from the top corners of each opening,
        where a lintel has settled."""
        for k, op in enumerate(face.openings):
            rnd = self._rnd("cracks", face.name, k)
            for i, (u, sign) in enumerate(((op.u0, -1.0), (op.u1, 1.0))):
                if not rnd.chance(0.25 + 0.6 * self.level):
                    continue
                s = rnd.uniform(0.9, 1.4) * self.scale
                z = face.z0 + op.v1
                cu, cz = u + sign * 0.35 * s, z + 0.35 * s
                if cu < 0.2 or cu > face.length - 0.2 or face.blocked(cu, cz, s * 0.4, s * 0.4, margin=0.0):
                    continue
                self._add("cracks", f"{face.name}_{k}_{i}", face.hosts, face.point(cu, cz), face.facing3, UP, (s, s), 0.6,
                          "a lintel has settled", dict(corner=(u, z), opening=k))

    def cracks_on_floor(self, rect, floor, count):
        """`count` cracks at seeded places in `(x0, y0, x1, y1)`, fewer the
        lower the level."""
        x0, y0, x1, y1 = rect
        rnd = self._rnd("floor_cracks", round(x0, 2), round(y0, 2))
        for i in range(int(count * self.level + rnd.unit())):
            x, y = rnd.uniform(x0 + 0.5, x1 - 0.5), rnd.uniform(y0 + 0.5, y1 - 0.5)
            s = rnd.uniform(1.0, 1.8) * self.scale
            self._add("cracks", f"floor_{i}", floor.hosts, (x, y, floor.height(x, y)), UP, rnd.pick(((0.0, 1.0, 0.0), (1.0, 0.0, 0.0))),
                      (s, s), 0.5, "an old slab cracks", dict())

    def flakes_at_foot(self, face, floor):
        """Flakes of paint on the floor along the foot of a painted wall:
        paint has peeled and fallen straight down."""
        if not face.painted:
            return
        slots = max(1, int(face.length // 1.5))
        for i in range(slots):
            rnd = self._rnd("flakes", face.name, i)
            if not rnd.chance(0.55 * self.level):
                continue
            u = (i + 0.5) * face.length / slots + rnd.uniform(-0.3, 0.3)
            if face.doorway_at(u):
                continue
            w = rnd.uniform(0.7, 1.1) * self.scale
            x, y = face.xy(u, 0.35)
            self._add("flakes", f"{face.name}_{i}", floor.hosts, (x, y, floor.height(x, y)), UP, face.facing3, (w, w), 0.5,
                      "paint peels and falls", dict(u=u))

    def cobwebs_in_corners(self, corners):
        """Cobwebs in the top corners where two walls meet under a ceiling:
        each corner is `(x, y, z_top, facing, hosts)`, `facing` the horizontal
        direction out of the corner; the web hangs from the ceiling line."""
        for k, (x, y, z_top, facing, hosts) in enumerate(corners):
            rnd = self._rnd("cobweb", k)
            if not rnd.chance(0.7 * self.level):
                continue
            s = rnd.uniform(0.8, 1.2) * self.scale
            d = g.unit((facing[0], facing[1], 0.0))
            self._add("cobweb", f"{k}", hosts, (x, y, z_top - s / 2.0 + 0.015 * s), d, UP, (s, s), 0.8,
                      "spiders keep the corners", dict(corner=(x, y, z_top)))

    def droppings_under(self, p0, p1, floor):
        """Bird droppings on the floor under a beam, truss or ledge from `p0` to
        `p1` (x, y): scattered along it and a little either side."""
        d = (p1[0] - p0[0], p1[1] - p0[1], 0.0)
        length = g.length(d)
        if length == 0.0:
            return
        dirn = (d[0] / length, d[1] / length)
        rnd = self._rnd("droppings", round(p0[0], 3), round(p0[1], 3))
        for i in range(int(length * 0.8 * self.level + rnd.unit())):
            t = rnd.uniform(0.0, length)
            side = rnd.uniform(-0.25, 0.25)
            x = p0[0] + dirn[0] * t - dirn[1] * side
            y = p0[1] + dirn[1] * t + dirn[0] * side
            s = rnd.uniform(0.5, 0.9) * self.scale
            self._add("droppings", f"{i}", floor.hosts, (x, y, floor.height(x, y)), UP,
                      rnd.pick(((0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0))), (s, s), 0.5,
                      "birds perch on the beam above", dict(beam=(p0, p1), t=t, side=side))

    def oil_at(self, spots, floor):
        """Oil under each `(x, y)` where a machine, a drum or a can stood: not
        every one leaks, more of them the higher the level."""
        for k, (x, y) in enumerate(spots):
            rnd = self._rnd("oil", k, round(x, 3), round(y, 3))
            if not rnd.chance(0.4 + 0.5 * self.level):
                continue
            s = rnd.uniform(0.7, 1.3) * self.scale
            ox, oy = x + rnd.uniform(-0.25, 0.25), y + rnd.uniform(-0.25, 0.25)
            self._add("oil", f"{k}", floor.hosts, (ox, oy, floor.height(ox, oy)), UP,
                      rnd.pick(((0.0, 1.0, 0.0), (1.0, 0.0, 0.0))), (s, s), 0.5, "something leaked here", dict(spot=(x, y)))

    # ---------- the author's own say ----------

    def hand(self, name, material, on, centre, facing, up, size, layer=0, depth=0.4):
        """A decal placed by hand, on top of what the rules do: it is never
        moved or dropped by the level, the seed or `keep_clear`."""
        spec = Spec("hand", name, material, on, centre, facing, up, size, layer, depth, "placed by hand", hand=True)
        self.specs.append(spec)
        return spec

    def keep_clear(self, low, high):
        """Locks a box `(x, y, z)` to `(x, y, z)` against the rules: any decal
        a rule would put with its centre in it is dropped (hand-placed ones
        stay). Use it where the Map's story needs a clean wall."""
        self.clear_boxes.append((tuple(map(float, low)), tuple(map(float, high))))

    # ---------- what came out ----------

    def result(self):
        """The decals to put, in the order the rules made them, without those
        in a cleared box."""
        def cleared(spec):
            return any(all(lo[i] <= spec.centre[i] <= hi[i] for i in range(3)) for lo, hi in self.clear_boxes)
        return [s for s in self.specs if s.hand or not cleared(s)]

    def counts(self):
        """How many decals of each kind `result` has."""
        out = {}
        for s in self.result():
            out[s.kind] = out.get(s.kind, 0) + 1
        return out

    def apply(self, m):
        """Puts every decal of `result` on the `MapBuilder` `m`; returns how many."""
        done = self.result()
        for s in done:
            m.decal(s.name, s.material, s.hosts, s.centre, s.facing, s.up, s.size, layer=s.layer, depth=s.depth)
        return len(done)
