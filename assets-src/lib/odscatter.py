"""The scatter: rules that put the kit's mess on a Map (#339, MIT OR Apache-2.0).

A `Scatter` takes the ground's height (any `height(x, y)`, such as
`odground.Ground.height`), a seed and the **keep-clear zones** the pilot flies
through, then rules add kit items (`odkit.py`) to it:

```python
from odscatter import Scatter
s = Scatter(seed=7, ground=floor.height)
s.keep_clear_rect(-1.5, -6.0, 1.5, 6.0)          # the aisle
s.keep_clear_circle(0.0, -12.0, 3.5)             # the Launch Spot
s.heap((6.0, 3.0), 2.4, 1.1, toward=(0.0, 1.0))  # rubble, higher against the north wall
s.wall_foot((8, -6), (-8, -6), depth=0.9)        # plaster along the south wall's foot
s.strew((-8, -6, 8, 6), 0.8)                     # debris in clusters over the floor
s.anchor_row((-6, 2), (6, 2), 3.0)               # bolt plates on the old column line
s.pallets(7.0, 5.0, 30.0)                        # a pile of pallets in a corner
s.boards(-6.5, 4.5, 80.0)                        # boards and planks thrown in a heap
s.place("plank", 0.0, 0.0, 15.0)                 # one by hand
s.patches((-8, -6, 8, 6), 6, on=["floor"])       # puddles and moss: decals
s.apply(m)                                       # onto the MapBuilder `m`
```

- **The same seed, the same mess.** Each rule draws from `Random(seed, rule,
  its number)`, so adding a rule, or changing one, never moves what another
  placed.
- **Resting.** An item is turned, then lowered until its lowest corner,
  measured against the surface under that corner, touches it, then sunk by
  its kit share of its height (a brick a little, a lump of concrete more). On
  a heap or along a wall foot the surface is the ground plus the heap's or
  the wedge's own shape, and flat things lie along its slope.
- **Keep-clear zones.** An item with any corner in a zone is dropped, and a
  heap's or a wall foot's core is cut away at the zone's edge, so an aisle, a
  Gap or the Launch Spot stays open.
- **Few draws.** `apply` merges every item of one material in one `cell` (8 m
  squares) into one shape, which takes its light like any other shape (the
  second UV set, if the Map asks for baked light). `stats()` says the
  triangles and the draws.
- **Colliders.** Loose items are only drawn. A heap's core is a `trimesh`
  surface the Quad meets, and a big upright item (a pallet stack, a drum) gets
  a collision-only box.
"""
import math

import odgeom as g
import odkit
from odcut import clip_grid, merge, triangle_count
from odnoise import Random, fbm2

# What each rule throws down unless told otherwise: kit item -> weight.
HEAP_MIX = {"concrete_lump": 5, "concrete_chunk": 8, "brick": 6, "brick_chunk": 10, "plaster_chip": 6,
            "roof_sheet_piece": 0.6, "plank": 0.4, "plastic_scrap": 1.5, "tyre": 0.15, "can": 0.4, "pipe": 0.15}
WALL_FOOT_MIX = {"plaster_chip": 12, "brick_chunk": 4, "concrete_chunk": 4, "brick": 1.5, "plastic_scrap": 0.5,
             "can": 0.3}
STREW_MIX = {"plaster_chip": 8, "brick_chunk": 3, "concrete_chunk": 3, "brick": 2, "board": 0.5, "plank": 0.2,
             "roof_sheet_piece": 0.3, "can": 0.6, "bottle": 0.5, "plastic_scrap": 1.0, "cable_tangle": 0.15,
             "concrete_lump": 0.4}
ANCHOR_MIX = {"bolt_plate": 3, "anchor_stubs": 2}
BOARD_MIX = {"plank": 3, "board": 2}

# How far each kind may tip over, in degrees, before it is laid on what is
# under it. A brick or a lump lands any way up; a plank or a sheet lies flat.
TUMBLE = {"brick": 90.0, "brick_chunk": 180.0, "concrete_lump": 40.0, "concrete_chunk": 180.0,
          "plaster_chip": 6.0, "roof_sheet_piece": 8.0, "plank": 3.0, "board": 5.0, "plastic_scrap": 10.0}
# Kinds a rule may make bigger or smaller (a brick stays a brick's size).
SCALES = ("concrete_lump", "concrete_chunk", "brick_chunk", "plaster_chip")
# Kinds that always stand upright, whatever they are on.
UPRIGHT = ("pallet", "pallet_stack", "drum", "drum_lying", "pipe", "pipe_run", "anchor_stubs", "bolt_plate",
           "cable_tangle", "tyre", "can", "bottle")


class Placement:
    """One kit item on the Map: its variant's parts turned, scaled and moved
    (`parts`, as `[(material, mesh)]`), where it is, and the rule that put it."""

    def __init__(self, rule, kind, k, parts, at, yaw, lift, sink):
        self.rule, self.kind, self.k, self.parts = rule, kind, k, parts
        self.at, self.yaw = at, yaw
        self.lift = lift  # how far its lowest corner was put above what is under it (a board on a pile), metres
        self.sink = sink  # how far it was then sunk, metres

    def corners(self):
        return [c for _, (cs, _) in self.parts for c in cs]


def _weighted(rnd, mix):
    total = sum(mix.values())
    x = rnd.unit() * total
    for kind in sorted(mix):
        x -= mix[kind]
        if x < 0.0:
            return kind
    return sorted(mix)[-1]


def _towards(n):
    """Rows that turn +z onto the unit vector `n` the shortest way."""
    nx, ny, nz = n
    s = math.sqrt(nx * nx + ny * ny)
    if s < 1e-9:
        return ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
    ax, ay = -ny / s, nx / s  # the axis, in the plane
    c, sn = nz, s
    t = 1.0 - c
    return ((t * ax * ax + c, t * ax * ay, sn * ay),
            (t * ax * ay, t * ay * ay + c, -sn * ax),
            (-sn * ay, sn * ax, c))


class Scatter:
    def __init__(self, seed, ground=None, cell=8.0):
        self.seed = seed
        self.ground = ground or (lambda x, y: 0.0)
        self.cell = cell
        self.zones = []  # (kind, numbers): signed distance outside a zone > 0
        self.placements = []
        self.cores = []  # (name, material, mesh, collider)
        self.decals = []
        self.rules = {}  # rule name -> how many times it was used
        self.dropped = 0

    # ---------------------------------------------------------- keep-clear
    def keep_clear_rect(self, x0, y0, x1, y1):
        """A rectangle the rules leave empty: an aisle, a doorway, a Gap's
        approach."""
        self.zones.append(("rect", (min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1))))

    def keep_clear_circle(self, x, y, r):
        """A circle the rules leave empty: the Launch Spot."""
        self.zones.append(("circle", (x, y, r)))

    def clearance(self, x, y):
        """How far `(x, y)` is outside the nearest keep-clear zone, in metres:
        below 0 inside one. Large when there is none."""
        best = 1e9
        for kind, v in self.zones:
            if kind == "rect":
                x0, y0, x1, y1 = v
                dx, dy = max(x0 - x, x - x1), max(y0 - y, y - y1)
                d = math.sqrt(max(dx, 0.0) ** 2 + max(dy, 0.0) ** 2) if dx > 0.0 or dy > 0.0 else max(dx, dy)
            else:
                cx, cy, r = v
                d = math.sqrt((x - cx) ** 2 + (y - cy) ** 2) - r
            best = min(best, d)
        return best

    def _rnd(self, rule):
        n = self.rules.get(rule, 0)
        self.rules[rule] = n + 1
        return Random(self.seed, rule, n)

    # ---------------------------------------------------------- placing one
    def _put(self, rule, rnd, kind, x, y, yaw=None, support=None, scale=1.0, lift=0.0, fence=None):
        """Turns, rests and sinks one variant of `kind` at `(x, y)` on the
        surface `support(x, y)` (the ground if None), `lift` metres above it.
        `fence(corners) -> (dx, dy)` moves it before it is rested, out of a
        wall. Returns the Placement, or None when it touched a keep-clear
        zone."""
        support = support or self.ground
        item = odkit.KIT[kind]
        k = int(rnd.unit() * item.variants)
        yaw = rnd.uniform(0.0, 360.0) if yaw is None else yaw
        tumble = TUMBLE.get(kind, 0.0)
        rows = odkit.turned(yaw, rnd.uniform(-tumble, tumble), rnd.uniform(-tumble, tumble))
        if kind not in UPRIGHT:
            # Lie along the slope: turn +z onto the surface's normal.
            e = 0.05
            dzx = (support(x + e, y) - support(x - e, y)) / (2.0 * e)
            dzy = (support(x, y + e) - support(x, y - e)) / (2.0 * e)
            n = g.unit((-dzx, -dzy, 1.0))
            rows = g.mat_mul(_towards(n), rows)
        parts = []
        for material, (cs, fs) in odkit.variant(kind, k):
            pose = g.Pose(rows, (x, y, 0.0))
            parts.append((material, ([pose.apply((cx * scale, cy * scale, cz * scale)) for cx, cy, cz in cs], fs)))
        corners = [c for _, (cs, _) in parts for c in cs]
        if fence is not None:
            fx, fy = fence(corners)
            x, y = x + fx, y + fy
            parts = [(m, ([(cx + fx, cy + fy, cz) for cx, cy, cz in cs], fs)) for m, (cs, fs) in parts]
            corners = [c for _, (cs, _) in parts for c in cs]
        # Rest: the corner that is lowest against the surface under it touches it.
        gap = min(c[2] - support(c[0], c[1]) for c in corners)
        height = max(c[2] for c in corners) - min(c[2] for c in corners)
        sink = rnd.uniform(*item.sink) * height
        dz = lift - gap - sink
        parts = [(m, ([(cx, cy, cz + dz) for cx, cy, cz in cs], fs)) for m, (cs, fs) in parts]
        if any(self.clearance(c[0], c[1]) <= 0.0 for _, (cs, _) in parts for c in cs):
            self.dropped += 1
            return None
        p = Placement(rule, kind, k, parts, (x, y, support(x, y) + lift), yaw, lift, sink)
        self.placements.append(p)
        return p

    def place(self, kind, x, y, yaw, k=None):
        """One kit item by hand, upright, at `(x, y)` turned `yaw` degrees
        (anticlockwise from east). Keep-clear zones still drop it."""
        rnd = self._rnd("place")
        if k is not None:
            rnd = _Fixed(rnd, k / odkit.KIT[kind].variants)
        return self._put("place", rnd, kind, x, y, yaw)

    # ---------------------------------------------------------- the rules
    def heap(self, centre, radius, height, mix=None, count=None, toward=None, core="rubble", core_step=0.25):
        """A heap of rubble `radius` metres round and `height` tall at its
        top: a core shape (in the material `core`, or None for none) and
        `count` kit items on it (about 30 a square metre if None), the big
        ones at its foot and the small ones on top. `toward` (a direction
        `(x, y)`) leans the heap that way, higher against a wall."""
        rnd = self._rnd("heap")
        mix = mix or HEAP_MIX
        cx, cy = centre
        tx, ty = toward or (0.0, 0.0)
        noise_seed = int(rnd.unit() * 1e9)

        def bump(x, y):
            dx, dy = (x - cx) / radius, (y - cy) / radius
            d = math.sqrt(dx * dx + dy * dy)
            if d >= 1.0:
                return 0.0
            t = 1.0 - d
            lean = 1.0 + 0.6 * (dx * tx + dy * ty)
            rough = 1.0 + 0.25 * fbm2(noise_seed, x, y, 0.8, 3)
            return height * t * t * (3.0 - 2.0 * t) * lean * rough

        def surface(x, y):
            return self.ground(x, y) + bump(x, y)
        if core:
            self._core("heap", core, (cx - radius, cy - radius, cx + radius, cy + radius), core_step,
                       lambda x, y: surface(x, y) - 0.03, lambda x, y: bump(x, y) - 0.005)
        count = count if count is not None else int(30.0 * math.pi * radius * radius)
        for _ in range(count):
            a = rnd.uniform(0.0, 2.0 * math.pi)
            r = radius * math.sqrt(rnd.unit()) * 0.97
            x, y = cx + r * math.cos(a), cy + r * math.sin(a)
            kind = _weighted(rnd, mix)
            scale = (1.35 - 0.7 * (1.0 - r / radius)) * rnd.uniform(0.8, 1.15) if kind in SCALES else 1.0
            self._put("heap", rnd, kind, x, y, support=surface, scale=scale)
        return surface

    def wall_foot(self, p0, p1, depth=0.8, height=0.12, per_metre=40.0, mix=None, core=None, core_step=0.2):
        """Debris fallen along a wall's foot, on the front of the wall from
        `p0` to `p1` (the front is on the left of the way from `p0` to `p1`,
        as `odopening.broken_wall(...).pose` and `odwear.Face` have it):
        thickest against the wall, thinning out `depth` metres from it, up to
        `height` deep; `per_metre` items a metre of wall. `core` (a material)
        lays a low wedge of crumbled stuff under the items."""
        rnd = self._rnd("wall_foot")
        mix = mix or WALL_FOOT_MIX
        ux, uy = p1[0] - p0[0], p1[1] - p0[1]
        run = math.sqrt(ux * ux + uy * uy)
        ux, uy = ux / run, uy / run
        nx, ny = -uy, ux
        noise_seed = int(rnd.unit() * 1e9)

        def local(x, y):
            return (x - p0[0]) * ux + (y - p0[1]) * uy, (x - p0[0]) * nx + (y - p0[1]) * ny

        def wedge(x, y):
            s, t = local(x, y)
            if t < 0.0 or t >= depth or s < 0.0 or s > run:
                return 0.0
            end = min(1.0, s / 0.6, (run - s) / 0.6)
            along = 0.6 + 0.4 * fbm2(noise_seed, s, 0.0, 1.5, 3)
            return height * (1.0 - t / depth) ** 2 * along * end

        def surface(x, y):
            return self.ground(x, y) + wedge(x, y)
        def fence(corners):
            # Never into the wall: out by however far a corner went in.
            inside = min(local(c[0], c[1])[1] for c in corners)
            shift = max(0.0, 0.01 - inside)
            return nx * shift, ny * shift

        if core:
            self._core_strip("wall_foot", core, p0, (ux, uy), (nx, ny), run, depth, core_step,
                             lambda x, y: surface(x, y) - 0.015, lambda x, y: wedge(x, y) - 0.003)
        for _ in range(int(per_metre * run)):
            s = rnd.uniform(0.0, run)
            t = depth * rnd.unit() ** 1.8
            kind = _weighted(rnd, mix)
            scale = rnd.uniform(0.8, 1.2) if kind in SCALES else 1.0
            self._put("wall_foot", rnd, kind, p0[0] + ux * s + nx * t, p0[1] + uy * s + ny * t,
                      support=surface, scale=scale, fence=fence)
        return surface

    def strew(self, rect, per_m2=0.8, mix=None, cluster=(0.4, 1.6), loose=0.25, bed=None, bed_height=(0.04, 0.1),
              bed_step=0.2):
        """Things strewn over a floor or a yard under a broken roof: `per_m2`
        items a square metre on average, most of them in clusters `cluster`
        metres across (a fall of plaster, where a ceiling came down), `loose`
        of them anywhere. `bed` (a material) lays a low bed of crumbled stuff
        under each cluster, `bed_height` metres deep in its middle, so a
        strewn floor has relief and not only bits."""
        rnd = self._rnd("strew")
        mix = mix or STREW_MIX
        x0, y0, x1, y1 = rect
        area = (x1 - x0) * (y1 - y0)
        count = int(per_m2 * area)
        clusters = [(rnd.uniform(x0, x1), rnd.uniform(y0, y1), rnd.uniform(*cluster), rnd.uniform(*bed_height))
                    for _ in range(max(1, int(count / 25)))]
        noise_seed = int(rnd.unit() * 1e9)

        def one(c, x, y):
            cx, cy, r, h = c
            d = math.sqrt((x - cx) ** 2 + (y - cy) ** 2) / r
            if d >= 1.0:
                return 0.0
            t = 1.0 - d
            return h * t * t * (3.0 - 2.0 * t) * (1.0 + 0.3 * fbm2(noise_seed, x, y, 0.5, 2))

        def beds(x, y):
            return max(one(c, x, y) for c in clusters) if bed else 0.0

        def surface(x, y):
            return self.ground(x, y) + beds(x, y)
        if bed:
            sheets = []
            for i, c in enumerate(clusters):
                cx, cy, r, _ = c
                box = (max(x0, cx - r), max(y0, cy - r), min(x1, cx + r), min(y1, cy + r))
                if box[2] - box[0] < 2 * bed_step or box[3] - box[1] < 2 * bed_step:
                    continue
                earlier = clusters[:i]

                def keep(x, y, c=c, earlier=earlier):
                    k = min(one(c, x, y) - 0.003, self.clearance(x, y))
                    for e in earlier:  # where an earlier bed lies, that one is drawn
                        k = min(k, 0.003 - one(e, x, y))
                    return k
                mesh = clip_grid(*box, bed_step, lambda x, y: surface(x, y) - 0.01, keep)
                if mesh[1]:
                    sheets.append(mesh)
            if sheets:
                self.cores.append(("strew", bed, merge(sheets)))
        for _ in range(count):
            if rnd.chance(loose):
                x, y = rnd.uniform(x0, x1), rnd.uniform(y0, y1)
            else:
                cx, cy, r, _ = clusters[int(rnd.unit() * len(clusters))]
                # Thicker in the middle: the sum of two draws.
                x = cx + r * (rnd.unit() + rnd.unit() - 1.0)
                y = cy + r * (rnd.unit() + rnd.unit() - 1.0)
                if not (x0 <= x <= x1 and y0 <= y <= y1):
                    continue
            kind = _weighted(rnd, mix)
            scale = rnd.uniform(0.8, 1.2) if kind in SCALES else 1.0
            self._put("strew", rnd, kind, x, y, support=surface, scale=scale)
        return surface

    def anchor_row(self, p0, p1, spacing, mix=None, missing=0.15):
        """Bolt plates and anchor stubs every `spacing` metres along a line,
        where a row of columns or machines was cut away; `missing` of them
        were dug out."""
        rnd = self._rnd("anchor_row")
        mix = mix or ANCHOR_MIX
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        run = math.sqrt(dx * dx + dy * dy)
        yaw = math.degrees(math.atan2(dy, dx))
        n = int(round(run / spacing))
        for i in range(n + 1):
            t = i / n if n else 0.0
            kind = _weighted(rnd, mix)
            if rnd.chance(missing):
                continue
            self._put("anchor_row", rnd, kind, p0[0] + dx * t, p0[1] + dy * t, yaw + rnd.uniform(-3.0, 3.0))

    def pallets(self, x, y, heading, stacks=(1, 3), loose=(0, 2)):
        """A pile of pallets in a corner: one to three stacks side by side,
        turned to `heading` (degrees anticlockwise from east), and a loose
        pallet or two dropped beside them."""
        rnd = self._rnd("pallets")
        a = math.radians(heading)
        ux, uy = math.cos(a), math.sin(a)
        n = stacks[0] + int(rnd.unit() * (stacks[1] - stacks[0] + 1))
        for i in range(n):
            off = (i - (n - 1) / 2.0) * 0.86 + rnd.uniform(-0.05, 0.05)
            self._put("pallets", rnd, "pallet_stack", x - uy * off, y + ux * off, heading + 90.0 + rnd.uniform(-6, 6))
        for _ in range(loose[0] + int(rnd.unit() * (loose[1] - loose[0] + 1))):
            side = rnd.uniform(-1.0, 1.0) * (n * 0.45 + 0.6)
            self._put("pallets", rnd, "pallet", x + ux * rnd.uniform(0.9, 1.4) - uy * side,
                      y + uy * rnd.uniform(0.9, 1.4) + ux * side, heading + rnd.uniform(-40.0, 40.0))

    def boards(self, x, y, heading, count=(6, 12), mix=None, spread=0.4):
        """Boards and planks thrown in a heap, roughly along `heading`
        (degrees anticlockwise from east), each lying on the ones below."""
        rnd = self._rnd("boards")
        mix = mix or BOARD_MIX
        n = count[0] + int(rnd.unit() * (count[1] - count[0] + 1))
        lift = 0.0
        for _ in range(n):
            kind = _weighted(rnd, mix)
            p = self._put("boards", rnd, kind, x + rnd.uniform(-spread, spread), y + rnd.uniform(-spread, spread),
                          heading + rnd.uniform(-14.0, 14.0), lift=lift)
            if p is not None:
                lift += 0.022

    def patches(self, rect, count, on, kinds=("damp", "damp", "moss", "algae"), size=(0.7, 2.4)):
        """Puddles and moss on a floor: `count` decals of `kinds` (the
        `odwear` kinds: damp is a puddle's wet stain, moss, algae) at seeded
        places in `rect`, on the shapes named in `on`, none centred in a
        keep-clear zone. Made by `apply`, after the shapes they lie on."""
        rnd = self._rnd("patches")
        x0, y0, x1, y1 = rect
        for _ in range(count):
            x, y = rnd.uniform(x0, x1), rnd.uniform(y0, y1)
            kind = rnd.pick(kinds)
            s = rnd.uniform(*size)
            a = rnd.uniform(0.0, 2.0 * math.pi)
            if self.clearance(x, y) <= 0.0:
                continue
            self.decals.append((kind, list(on), (x, y, self.ground(x, y)), (math.cos(a), math.sin(a), 0.0),
                                (s, s * rnd.uniform(0.6, 1.0))))

    # ---------------------------------------------------------- cores
    def _core(self, rule, material, rect, step, height, inside):
        """A sheet over `rect` at `height(x, y)`, only where `inside(x, y)` is
        above zero and outside every keep-clear zone."""
        mesh = clip_grid(*rect, step, height, lambda x, y: min(inside(x, y), self.clearance(x, y)))
        if mesh[1]:
            self.cores.append((rule, material, mesh))

    def _core_strip(self, rule, material, p0, u, n, run, depth, step, height, inside):
        """`_core` over a strip along a wall: worked out along and out from
        the wall, then put in the Map."""
        def world(s, t):
            return p0[0] + u[0] * s + n[0] * t, p0[1] + u[1] * s + n[1] * t

        def h(s, t):
            return height(*world(s, t))

        def keep(s, t):
            x, y = world(s, t)
            return min(inside(x, y), self.clearance(x, y))
        corners, faces = clip_grid(0.0, 0.01, run, depth, step, h, keep)
        if faces:
            corners = [(*world(s, t), z) for s, t, z in corners]
            if u[0] * n[1] - u[1] * n[0] < 0.0:
                faces = [tuple(reversed(f)) for f in faces]
            self.cores.append((rule, material, (corners, faces)))

    # ---------------------------------------------------------- out
    def meshes(self):
        """What `apply` puts on the Map: `[(cell name, material, mesh)]`,
        every item of one material in one cell merged, in a fixed order."""
        groups = {}
        for p in self.placements:
            ix, iy = math.floor(p.at[0] / self.cell), math.floor(p.at[1] / self.cell)
            for material, mesh in p.parts:
                groups.setdefault((material, ix, iy), []).append(mesh)
        return [(f"{ix}_{iy}", material, merge(groups[(material, ix, iy)]))
                for material, ix, iy in sorted(groups)]

    def stats(self):
        """Items, triangles and draws, by kind and by material."""
        meshes = self.meshes()
        by_material = {}
        for _, material, mesh in meshes:
            by_material[material] = by_material.get(material, 0) + triangle_count(mesh)
        kinds = {}
        for p in self.placements:
            kinds[p.kind] = kinds.get(p.kind, 0) + 1
        core_tris = sum(triangle_count(mesh) for _, _, mesh in self.cores)
        return {
            "items": len(self.placements), "dropped": self.dropped, "kinds": dict(sorted(kinds.items())),
            "item_triangles": sum(by_material.values()), "core_triangles": core_tris,
            "draws": len(meshes) + len(self.cores), "decals": len(self.decals),
            "triangles_by_material": dict(sorted(by_material.items())),
        }

    def apply(self, m, name="kit", light_cm=None, light_least=2.0):
        """Puts everything on the MapBuilder `m`: the merged items (drawn
        only), the cores (a `trimesh` surface), a collision box for each big
        upright item, then the decals. Returns the names of the shapes made.
        If the Map asks for baked light, each face of an item spans at least
        `light_least` texels of the atlas, so a brick's face isn't left black
        (`MapBuilder.solid`)."""
        names = []
        for rule, material, mesh in self.cores:
            names.append(m.solid(f"{name}_{rule}", mesh, None, material, "trimesh", light_cm=light_cm).name)
        for cell, material, mesh in self.meshes():
            names.append(m.solid(f"{name}_{cell}_{material}", mesh, None, material, "none", light_cm=light_cm,
                                 light_least=light_least).name)
        for p in self.placements:
            if not odkit.KIT[p.kind].solid:
                continue
            x0, y0, z0, x1, y1, z1 = odkit.extent(odkit.variant(p.kind, p.k))
            a = math.radians(p.yaw)
            ox, oy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
            low = min(c[2] for c in p.corners())
            centre = (p.at[0] + ox * math.cos(a) - oy * math.sin(a), p.at[1] + ox * math.sin(a) + oy * math.cos(a),
                      low + (z1 - z0) / 2.0)
            m.box(f"{name}_solid", (x1 - x0, y1 - y0, z1 - z0), centre, "concrete_dark", heading=-p.yaw,
                  collision_only=True)
        if self.decals:
            import odwear
            for i, (kind, on, centre, up, size) in enumerate(self.decals):
                m.decal(f"{name}_{kind}", odwear.MATERIAL[kind], on, centre, (0.0, 0.0, 1.0), up, size,
                        layer=odwear.LAYER[kind])
        return names


class _Fixed:
    """A Random whose first draw is fixed, to choose a variant by hand."""

    def __init__(self, rnd, first):
        self.rnd, self.first = rnd, first

    def unit(self):
        if self.first is not None:
            v, self.first = self.first, None
            return v
        return self.rnd.unit()

    def uniform(self, a, b):
        return a + (b - a) * self.unit()
