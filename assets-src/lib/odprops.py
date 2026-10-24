"""Props: the loose things that tell a place's history (#324, MIT OR Apache-2.0).

A prop is a thing someone left: a pallet, a bottle crate, a bottle, a can, an
oil drum, a cable drum used as a table, a bed frame, a shopping trolley, a
filing cabinet, a tyre, a skip, the bits of a crashed quad. Each function here
builds one from boxes, cylinders and bars, in plain Python like `odgeom`, so a
Map that uses them has the same bytes on every computer
(docs/research/lived-in-places.md says which props a story needs and where
they go).

Every prop is made in its own axes: standing on z = 0, centred on x = y = 0,
its length along x. It comes back as a mesh `(corners, faces)` that
`MapBuilder.solid` (or an `odruin.Batch`) takes with a pose:

```python
from odprops import pallet, place
batch = Batch()
place(batch, pallet(Random(3, "p")), (4.0, 2.0, 0.0), yaw=20.0)
m.solid("pallets", batch.mesh(), None, "wood_weathered", "trimesh")
```

Each copy takes its own `Random`, so no two are the same: a board missing or
broken, a dent, a sag, a crack. Every prop is a set of closed solids facing
out, so it can be a `trimesh` collider; the research's rule is that a prop
bigger than about 20 cm that a Quad can hit is solid, and smaller ones are
drawn only. Sizes are real ones, in metres.
"""
import math

import odgeom as g
from odcut import merge
from odnoise import noise2


def turned(yaw=0.0, pitch=0.0, roll=0.0):
    """Turn rows: roll about x, then pitch about y, then yaw about z (degrees)."""
    return g.mat_mul(g.rot_z(yaw), g.mat_mul(g.rot_y(pitch), g.rot_x(roll)))


def place(batch, mesh, at, yaw=0.0, pitch=0.0, roll=0.0):
    """Adds a prop to a Batch, turned and moved to `at` (x, y, z)."""
    batch.add(mesh, g.Pose(turned(yaw, pitch, roll), at))
    return batch


def _box(parts, size, centre, rows=None):
    corners, faces = g.box_mesh(*size)
    pose = g.Pose(rows or turned(), centre)
    parts.append(([pose.apply(c) for c in corners], faces))


def _bar(parts, p, q, w, h=None):
    """A square bar from `p` to `q`, `w` by `h` across."""
    pose = g.aiming(p, q)
    corners, faces = g.box_mesh(g.length(g.sub(q, p)), w, h if h is not None else w)
    pose = g.Pose(pose.rows, g.lerp(p, q, 0.5))
    parts.append(([pose.apply(c) for c in corners], faces))


def _cyl(parts, p, q, r, r1=None, segments=10):
    corners, faces = g.cylinder_mesh(g.length(g.sub(q, p)), r, r1, segments)
    pose = g.along(p, q)
    parts.append(([pose.apply(c) for c in corners], faces))


def deform(mesh, f):
    """The mesh with each corner moved by `f(x, y, z) -> (x, y, z)`: a dent,
    a sag, a melt."""
    corners, faces = mesh
    return [f(*c) for c in corners], faces


def size_of(mesh):
    """`(x, y, z)` extent of a mesh."""
    cs = mesh[0]
    return tuple(max(c[i] for c in cs) - min(c[i] for c in cs) for i in range(3))


# ---------------------------------------------------------------- wood

def pallet(rnd, missing=0.25, broken=0.25):
    """A Euro pallet, 1.2 x 0.8 x 0.144 m: three bottom boards, nine blocks,
    three cross boards and five deck boards, some deck boards gone or broken
    off short."""
    parts = []
    t = 0.022
    for y in (-0.35, 0.0, 0.35):
        _box(parts, (1.2, 0.1, t), (0.0, y, t / 2))
        for x in (-0.525, 0.0, 0.525):
            _box(parts, (0.145, 0.1, 0.078), (x, y, t + 0.039))
    for x in (-0.525, 0.0, 0.525):
        _box(parts, (0.145, 0.8, t), (x, 0.0, 0.1 + t / 2))
    for k, y in enumerate((-0.3275, -0.164, 0.0, 0.164, 0.3275)):
        width = 0.145 if k in (0, 2, 4) else 0.1
        if k not in (0, 4) and rnd.chance(missing):
            continue
        length, x = 1.2, 0.0
        if rnd.chance(broken):
            length = rnd.uniform(0.45, 0.85)
            x = (0.6 - length / 2) * (1.0 if rnd.chance(0.5) else -1.0)
        _box(parts, (length, width, t), (x, y, 0.122 + t / 2))
    return merge(parts)


def cable_drum():
    """A wooden cable drum stood on one flange as a table: flanges 1 m across
    and 0.05 thick, a core 0.44 across, 0.62 tall."""
    parts = []
    _cyl(parts, (0, 0, 0.0), (0, 0, 0.05), 0.5, segments=18)
    _cyl(parts, (0, 0, 0.05), (0, 0, 0.57), 0.22, segments=12)
    _cyl(parts, (0, 0, 0.57), (0, 0, 0.62), 0.5, segments=18)
    return merge(parts)


# ---------------------------------------------------------------- plastic and glass

def crate(rnd, cracked=False):
    """A plastic bottle crate, 0.4 x 0.3 x 0.3 m, open at the top: a floor,
    four walls and a divider; a cracked one has lost a piece of one wall."""
    parts = []
    w = 0.012
    _box(parts, (0.4, 0.3, w), (0, 0, w / 2))
    top = 0.3
    for sx in (-1, 1):
        _box(parts, (w, 0.3 - 2 * w, top - w), (sx * (0.2 - w / 2), 0, w + (top - w) / 2))
    for sy in (-1, 1):
        h = top - w
        if cracked and sy > 0:
            h = rnd.uniform(0.12, 0.2)
        _box(parts, (0.4, w, h), (0, sy * (0.15 - w / 2), w + h / 2))
    _box(parts, (0.4 - 2 * w, 0.008, 0.2), (0, 0, w + 0.1))
    return merge(parts)


def bottle(rnd, kind="beer"):
    """A bottle, standing: a body, a shoulder and a neck. `beer` is 0.24 m,
    `wine` 0.3."""
    h, r = (0.15, 0.032) if kind == "beer" else (0.2, 0.037)
    h *= rnd.uniform(0.95, 1.05)
    parts = []
    _cyl(parts, (0, 0, 0), (0, 0, h), r, segments=8)
    _cyl(parts, (0, 0, h), (0, 0, h + 0.04), r, 0.013, segments=8)
    _cyl(parts, (0, 0, h + 0.04), (0, 0, h + 0.09), 0.013, 0.014, segments=6)
    return merge(parts)


def can(rnd, crushed=0.0):
    """A drinks can, 0.115 m tall and 0.066 across; `crushed` (0 to 1)
    squashes it flat and buckles it."""
    mesh = g.cylinder_mesh(0.115, 0.033, None, 8)
    if crushed <= 0.0:
        return mesh
    squash = 1.0 - 0.7 * crushed
    kx, ky = rnd.uniform(-0.3, 0.3), rnd.uniform(-0.3, 0.3)

    def f(x, y, z):
        bulge = 1.0 + 0.4 * crushed * (z / 0.115) * (1.0 - z / 0.115) * 4.0
        return (x * bulge + kx * z * crushed, y * bulge * (1.0 - 0.3 * crushed) + ky * z * crushed, z * squash)
    return deform(mesh, f)


def bin_bag(rnd):
    """A full bin bag slumped against something: a lumpy sack about 0.55 m
    tall, with a knot."""
    corners, faces = g.cylinder_mesh(0.5, 0.26, 0.18, 12)
    seed = int(rnd.unit() * 1e6)

    def f(x, y, z):
        r = 1.0 + 0.18 * noise2(seed, x * 5.0 + z * 3.0, y * 5.0 - z * 2.0)
        sag = 1.0 + 0.25 * (1.0 - z / 0.5) * (1.0 - z / 0.5)
        return (x * r * sag, y * r * sag, z * (0.9 + 0.1 * r))
    parts = [deform((corners, faces), f)]
    _cyl(parts, (0, 0, 0.47), (0.02, 0.0, 0.6), 0.05, 0.02, segments=6)
    return merge(parts)


def plastic_chair(rnd, melt=0.0):
    """A stacking garden chair: four legs, a seat and a back. `melt` (0 to
    1) slumps it as a fire leaves one: the seat sags and the whole chair
    leans and sinks toward one side."""
    parts = []
    for sx in (-1, 1):
        for sy in (-1, 1):
            _cyl(parts, (sx * 0.2, sy * 0.2, 0.0), (sx * 0.19, sy * 0.19, 0.42), 0.018, segments=6)
    _box(parts, (0.46, 0.46, 0.03), (0, 0, 0.435))
    _box(parts, (0.46, 0.03, 0.4), (0, 0.22, 0.65), turned(0, 0, -10))
    mesh = merge(parts)
    if melt <= 0.0:
        return mesh
    side = rnd.uniform(0.6, 1.0)

    def f(x, y, z):
        drop = melt * z * (0.35 + 0.4 * (x + 0.25) * side)
        sag = melt * 0.08 * max(0.0, 1.0 - (x * x + y * y) / 0.06) if z > 0.4 else 0.0
        return (x + melt * 0.15 * z, y, max(0.0, z - drop - sag))
    return deform(mesh, f)


# ---------------------------------------------------------------- steel

def drum(rnd, dented=0.0):
    """A 200 litre oil drum, 0.88 m tall and 0.58 across, with its two
    rolling hoops; `dented` pushes a dent into one side."""
    parts = []
    _cyl(parts, (0, 0, 0), (0, 0, 0.88), 0.29, segments=14)
    for z in (0.29, 0.59):
        _cyl(parts, (0, 0, z), (0, 0, z + 0.025), 0.302, segments=14)
    mesh = merge(parts)
    if dented <= 0.0:
        return mesh
    a = rnd.uniform(0.0, 360.0)
    dx, dy = math.cos(math.radians(a)), math.sin(math.radians(a))
    zc = rnd.uniform(0.3, 0.6)

    def f(x, y, z):
        d = x * dx + y * dy
        k = max(0.0, 1.0 - ((z - zc) / 0.25) ** 2) * max(0.0, d / 0.3)
        return (x - dx * dented * 0.08 * k, y - dy * dented * 0.08 * k, z)
    return deform(mesh, f)


def grill(rnd):
    """A steel grill grate, 0.55 x 0.35 m: a frame and nine bars."""
    parts = []
    for y in (-0.17, 0.17):
        _bar(parts, (-0.275, y, 0.005), (0.275, y, 0.005), 0.012, 0.01)
    for k in range(9):
        x = -0.24 + 0.06 * k
        _bar(parts, (x, -0.17, 0.012), (x, 0.17, 0.012), 0.006)
    return merge(parts)


def bed_frame(rnd, sag=0.12):
    """A single steel bed frame, 1.9 x 0.9 m, its top 0.3 m up: angle-iron
    sides, four legs, a headboard of bars, and a grid of springs that sags
    `sag` metres in the middle (no mattress: it burnt)."""
    parts = []
    z = 0.3
    for y in (-0.45, 0.45):
        _bar(parts, (-0.95, y, z), (0.95, y, z), 0.035)
    for x in (-0.95, 0.95):
        _bar(parts, (x, -0.45, z), (x, 0.45, z), 0.035)
        for y in (-0.45, 0.45):
            _bar(parts, (x, y, 0.0), (x, y, z + (0.55 if x < 0 else 0.0)), 0.03)
    _bar(parts, (-0.95, -0.45, 0.8), (-0.95, 0.45, 0.8), 0.025)
    for k in range(1, 6):
        y = -0.45 + 0.15 * k
        _bar(parts, (-0.95, y, z), (-0.95, y, 0.8), 0.012)
    # Springs: wires along the bed, sagging, and three across them.
    for k in range(1, 9):
        y = -0.45 + 0.1 * k
        pts = [(-0.93 + 1.86 * i / 6, y, z - sag * (1.0 - ((i / 3.0) - 1.0) ** 2) * (1.0 - (y / 0.45) ** 2) * rnd.uniform(0.8, 1.2))
               for i in range(7)]
        for p, q in zip(pts, pts[1:]):
            _bar(parts, p, q, 0.008)
    for x in (-0.5, 0.0, 0.5):
        _bar(parts, (x, -0.43, z - sag * 0.6), (x, 0.43, z - sag * 0.6), 0.008)
    return merge(parts)


def trolley(rnd):
    """A shopping trolley, 0.9 long, 0.55 wide and 1 m tall: a wire basket
    on a frame with four wheels and a handle; one wheel is gone."""
    parts = []
    gone = int(rnd.unit() * 4)
    for k, (x, y) in enumerate(((-0.4, -0.25), (-0.4, 0.25), (0.35, -0.2), (0.35, 0.2))):
        _bar(parts, (x, y, 0.07), (x, y, 0.2), 0.02)
        if k != gone:
            _cyl(parts, (x, y - 0.02, 0.06), (x, y + 0.02, 0.06), 0.06, segments=8)
    _bar(parts, (-0.4, 0.0, 0.2), (0.35, 0.0, 0.2), 0.03)
    _bar(parts, (-0.4, -0.25, 0.2), (-0.4, 0.25, 0.2), 0.025)
    # The basket: a top rim, a floor and wires, narrowing to the front.
    def rim(z, scale):
        return [(-0.45, -0.27 * scale, z), (0.45, -0.22 * scale, z), (0.45, 0.22 * scale, z), (-0.45, 0.27 * scale, z)]
    for z, s in ((0.5, 0.9), (0.95, 1.0)):
        ring = rim(z, s)
        for p, q in zip(ring, ring[1:] + ring[:1]):
            _bar(parts, p, q, 0.012)
    lo, hi = rim(0.5, 0.9), rim(0.95, 1.0)
    for side in range(4):
        a0, a1, b0, b1 = lo[side], lo[(side + 1) % 4], hi[side], hi[(side + 1) % 4]
        n = max(2, int(g.length(g.sub(a1, a0)) / 0.09))
        for i in range(1, n):
            t = i / n
            _bar(parts, g.lerp(a0, a1, t), g.lerp(b0, b1, t), 0.006)
    for i in range(1, 9):
        t = i / 9
        _bar(parts, g.lerp(lo[0], lo[1], t), g.lerp(lo[3], lo[2], t), 0.006)
    _bar(parts, (-0.55, -0.27, 1.0), (-0.55, 0.27, 1.0), 0.03)
    for y in (-0.26, 0.26):
        _bar(parts, (-0.45, y, 0.95), (-0.55, y, 1.0), 0.015)
    return merge(parts)


def filing_cabinet(rnd):
    """A four-drawer steel filing cabinet, 0.47 wide, 0.62 deep and 1.32
    tall, its front dented in. Its drawers are separate: `drawer()`."""
    mesh = g.box_mesh(0.47, 0.62, 1.32)
    dx, dz = rnd.uniform(-0.1, 0.1), rnd.uniform(0.3, 1.0)

    def f(x, y, z):
        z += 0.66
        k = max(0.0, 1.0 - ((x - dx) ** 2 + (z - dz) ** 2) / 0.05)
        return (x, y + (0.04 * k if y > 0 else 0.0) * -1.0, z)
    return deform(mesh, f)


def drawer():
    """A filing drawer pulled out and dropped: an open steel box 0.42 x 0.6 x
    0.26 with its front plate."""
    parts = []
    w = 0.008
    _box(parts, (0.42, 0.6, w), (0, 0, w / 2))
    for sx in (-1, 1):
        _box(parts, (w, 0.6, 0.24), (sx * 0.206, 0, 0.13))
    _box(parts, (0.42, w, 0.24), (0, -0.296, 0.13))
    _box(parts, (0.46, 0.02, 0.3), (0, 0.31, 0.15))
    return merge(parts)


def office_chair():
    """An office swivel chair, 0.6 across its five-star base and 1 m to the
    top of its back."""
    parts = []
    for k in range(5):
        a = math.radians(72 * k)
        _bar(parts, (0, 0, 0.08), (0.3 * math.cos(a), 0.3 * math.sin(a), 0.05), 0.04, 0.03)
        _cyl(parts, (0.3 * math.cos(a), 0.3 * math.sin(a), 0.0), (0.3 * math.cos(a), 0.3 * math.sin(a), 0.05), 0.025, segments=6)
    _cyl(parts, (0, 0, 0.08), (0, 0, 0.42), 0.025, segments=8)
    _box(parts, (0.48, 0.48, 0.08), (0, 0, 0.46))
    _box(parts, (0.44, 0.06, 0.48), (0, 0.25, 0.78), turned(0, 0, -8))
    for sx in (-1, 1):
        _bar(parts, (sx * 0.26, -0.15, 0.5), (sx * 0.26, -0.15, 0.68), 0.025)
        _box(parts, (0.06, 0.28, 0.03), (sx * 0.26, -0.08, 0.69))
    return merge(parts)


def fuse_box(door_open=110.0):
    """An empty steel fuse board box on a wall, 0.4 wide, 0.6 tall and 0.15
    deep, its back on y = 0 and its open front toward -y, the door swung open
    by `door_open` degrees on its left hinge."""
    parts = []
    w = 0.006
    _box(parts, (0.4, w, 0.6), (0, -w / 2, 0.3))
    for sx in (-1, 1):
        _box(parts, (w, 0.15, 0.6), (sx * (0.2 - w / 2), -0.075, 0.3))
    for z in (w / 2, 0.6 - w / 2):
        _box(parts, (0.4, 0.15, w), (0, -0.075, z))
    a = math.radians(door_open)
    hx, hy = -0.2, -0.15
    door = g.box_mesh(0.4, 0.006, 0.6)
    pose = g.Pose(g.rot_z(-door_open), (hx + 0.2 * math.cos(a), hy - 0.2 * math.sin(a), 0.3))
    parts.append(([pose.apply(c) for c in door[0]], door[1]))
    return merge(parts)


def tyre():
    """A car tyre lying flat: 0.62 across, 0.2 tall, 0.4 inside."""
    return g.annulus_mesh(0.2, 0.2, 0.31, 18)


def skip(rnd):
    """A builder's skip, 3.6 long at its top, 1.7 wide and 1.1 tall: a steel
    floor, two sides and sloping ends, open at the top."""
    parts = []
    w = 0.03
    top, bottom, h = 1.8, 1.2, 1.1
    _box(parts, (2 * bottom, 1.7, w), (0, 0, w / 2))
    for sy in (-1, 1):
        outline = [(-bottom, 0.0), (bottom, 0.0), (top, h), (-top, h)]
        parts.append(_plate_y(outline, sy * (0.85 - w / 2), w))
    for sx in (-1, 1):
        _bar(parts, (sx * bottom, 0.0, w), (sx * top, 0.0, h), 1.7, w)
    return merge(parts)


def _plate_y(outline, y, thick):
    """An outline of `(x, z)` points made a plate `thick` across y, centred on
    `y`."""
    k = len(outline)
    corners = [(x, y - thick / 2, z) for x, z in outline] + [(x, y + thick / 2, z) for x, z in outline]
    faces = [(i, (i + 1) % k, k + (i + 1) % k, k + i) for i in range(k)]
    for a, b, c in g.triangulate_polygon(outline):
        faces.append((a, c, b))
        faces.append((k + a, k + b, k + c))
    return corners, g.point_outwards(corners, faces)


def plinth(rnd, sx, sy, h):
    """A machine plinth: a concrete block `sx` by `sy`, `h` tall, its corners
    and an edge chipped off, so its outline isn't a rectangle. The cut anchor
    bolts are `bolts(...)`."""
    box = [(-sx / 2, -sy / 2), (sx / 2, -sy / 2), (sx / 2, sy / 2), (-sx / 2, sy / 2)]  # anticlockwise
    pts = []
    for i, c in enumerate(box):
        prev, nxt = box[i - 1], box[(i + 1) % 4]
        for other, cut in ((prev, rnd.uniform(0.02, 0.09)), (nxt, rnd.uniform(0.02, 0.09))):
            d = g.length((other[0] - c[0], other[1] - c[1], 0.0))
            pts.append((c[0] + (other[0] - c[0]) * cut / d, c[1] + (other[1] - c[1]) * cut / d))
    k = len(pts)
    corners = [(x, y, 0.0) for x, y in pts] + [(x, y, h) for x, y in pts]
    faces = [(i, (i + 1) % k, k + (i + 1) % k, k + i) for i in range(k)]
    for a, b, c in g.triangulate_polygon(pts):
        faces.append((a, c, b))
        faces.append((k + a, k + b, k + c))
    return corners, g.point_outwards(corners, faces)


def bolts(sx, sy, h, inset=0.12):
    """Anchor bolts cut nearly flush on top of a plinth: four short stubs."""
    parts = []
    for cx in (-1, 1):
        for cy in (-1, 1):
            x, y = cx * (sx / 2 - inset), cy * (sy / 2 - inset)
            _cyl(parts, (x, y, h - 0.005), (x, y, h + 0.012), 0.012, segments=6)
    return merge(parts)


def metal_pile(rnd, count=24):
    """Scrap the thieves sorted and left: steel offcuts, bent conduit and a
    few window frames, heaped about 1.6 x 1.2 m and 0.5 tall."""
    parts = []
    for i in range(count):
        length = rnd.uniform(0.4, 1.4)
        x, y = rnd.uniform(-0.6, 0.6), rnd.uniform(-0.45, 0.45)
        z = 0.03 + 0.4 * (1.0 - (x / 0.8) ** 2) * (1.0 - (y / 0.6) ** 2) * rnd.unit()
        a = rnd.uniform(0, 180)
        dx, dy = math.cos(math.radians(a)) * length / 2, math.sin(math.radians(a)) * length / 2
        dz = rnd.uniform(-0.1, 0.1)
        if i % 3 == 0:  # conduit, bent once
            mid = (x + rnd.uniform(-0.1, 0.1), y + rnd.uniform(-0.1, 0.1), z + 0.05)
            parts.append(g.tube_mesh([(x - dx, y - dy, z + dz), mid, (x + dx, y + dy, z - dz)], 0.012, 6))
        else:
            w = rnd.pick((0.03, 0.04, 0.05))
            _bar(parts, (x - dx, y - dy, z + dz), (x + dx, y + dy, z - dz), w, w * rnd.uniform(0.2, 1.0))
    for k in range(2):  # window frames, lying on top
        a = rnd.uniform(0, 180)
        cx, cy, z = rnd.uniform(-0.3, 0.3), rnd.uniform(-0.2, 0.2), 0.45 + 0.05 * k
        rows = turned(a, rnd.uniform(-8, 8), rnd.uniform(-8, 8))
        for (px, py), (qx, qy) in (((-0.5, -0.3), (0.5, -0.3)), ((0.5, -0.3), (0.5, 0.3)), ((0.5, 0.3), (-0.5, 0.3)), ((-0.5, 0.3), (-0.5, -0.3))):
            p = g.Pose(rows, (cx, cy, z)).apply((px, py, 0.0))
            q = g.Pose(rows, (cx, cy, z)).apply((qx, qy, 0.0))
            _bar(parts, p, q, 0.04, 0.025)
    mesh = merge(parts)
    low = min(c[2] for c in mesh[0])
    return [(x, y, z - low) for x, y, z in mesh[0]], mesh[1]


# ---------------------------------------------------------------- the gate

def gate_leaf(rnd, width, height):
    """One leaf of a steel yard gate, `width` along x from its hinge at x = 0
    and `height` tall: a box frame, a middle rail and upright bars every
    12 cm, a few bent."""
    parts = []
    f = 0.05
    for z in (0.08, height / 2, height):
        _bar(parts, (0.0, 0.0, z), (width, 0.0, z), f, f)
    for x in (0.0, width):
        _bar(parts, (x, 0.0, 0.05), (x, 0.0, height + f / 2), f, f)
    n = int(width / 0.12)
    for i in range(1, n):
        x = width * i / n
        bend = rnd.uniform(-0.05, 0.05) if rnd.chance(0.15) else 0.0
        _bar(parts, (x, 0.0, 0.08), (x, bend, height / 2), 0.02)
        _bar(parts, (x, bend, height / 2), (x, 0.0, height), 0.02)
    return merge(parts)


# ---------------------------------------------------------------- quads

def quad_frame(rnd, size=0.25, arms_gone=0):
    """A crashed quad seen from a distance: an X of carbon arms `size` metres
    motor to motor, a body, four motors and their props; `arms_gone` arms are
    snapped off near the body (and their motors gone)."""
    parts = []
    half = size / 2 / math.sqrt(2.0)
    _box(parts, (size * 0.35, size * 0.18, size * 0.12), (0, 0, size * 0.06 + 0.004))
    for k, (sx, sy) in enumerate(((1, 1), (-1, 1), (-1, -1), (1, -1))):
        if k < arms_gone:
            _bar(parts, (0, 0, 0.004), (sx * half * 0.35, sy * half * 0.35, 0.004), size * 0.06, 0.004)
            continue
        _bar(parts, (0, 0, 0.004), (sx * half, sy * half, 0.004), size * 0.06, 0.004)
        _cyl(parts, (sx * half, sy * half, 0.006), (sx * half, sy * half, 0.006 + size * 0.08), size * 0.055, segments=8)
        a = rnd.uniform(0, 180)
        r = size * 0.24
        top = 0.006 + size * 0.08 + 0.002
        _bar(parts, (sx * half - r * math.cos(math.radians(a)), sy * half - r * math.sin(math.radians(a)), top),
             (sx * half + r * math.cos(math.radians(a)), sy * half + r * math.sin(math.radians(a)), top), size * 0.04, 0.003)
    return merge(parts)


def prop_blades(rnd, diameter=0.127, blades=3, snapped=1):
    """A loose propeller (a 5-inch one is 0.127 m): a hub and its blades,
    `snapped` of them broken short."""
    parts = []
    _cyl(parts, (0, 0, 0), (0, 0, 0.008), 0.007, segments=6)
    for k in range(blades):
        a = math.radians(360.0 / blades * k + rnd.uniform(-4, 4))
        length = diameter / 2 * (rnd.uniform(0.3, 0.55) if k < snapped else 1.0)
        _bar(parts, (0, 0, 0.004), (length * math.cos(a), length * math.sin(a), 0.004), 0.012, 0.003)
    return merge(parts)


def antenna(length=0.12):
    """A bent video antenna: a thin lead and its cap."""
    parts = []
    _bar(parts, (0, 0, 0.002), (length * 0.7, 0.01, 0.006), 0.004)
    _bar(parts, (length * 0.7, 0.01, 0.006), (length, 0.04, 0.01), 0.004)
    _cyl(parts, (length, 0.04, 0.0), (length, 0.04, 0.02), 0.01, segments=6)
    return merge(parts)
