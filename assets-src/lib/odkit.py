"""The kit: the loose mess of a derelict works, made from scratch (#339, MIT OR Apache-2.0).

Bricks and brick chunks, concrete lumps with rebar, plaster chips, pieces of
roof sheet, planks and boards, pallets and a stack of them, pipes and a
fallen pipe run, drums, a cable tangle, a tyre, cans and bottles, plastic
scraps, and the anchor stubs and bolt plates left in a hall floor where the
machines and columns were cut away. `odscatter.py` puts them on a Map by
rules; this file only makes them.

It also holds the **pieces for hand placement** (#324): the things a Map's
script puts exactly where its story wants them, each as one mesh with its own
`Random` and its own knobs: a cable drum, a bottle crate, a bin bag, a
melted plastic chair, a grill grate, a bed frame with sagging springs, a
shopping trolley, a filing cabinet and its drawers, an office chair, a fuse box
with its door open, a skip, a machine plinth and its cut bolts, a pile of
sorted scrap, a gate leaf, and a crashed quad's frame, a propeller and an
antenna. They are kit items too: one name, one script. A pallet, drum, tyre, can
and bottle exist once, as `pallet_mesh`, `drum_mesh`, `tyre_mesh`, `can_mesh`
and `bottle_mesh`; the kit's items of the same names are built on them.

Every kit item is built in plain Python like `odgeom`, from a seed, so a Map
that uses them has the same bytes on every computer. An item is made in its
own axes: centred on x = y = 0, its lowest corner on z = 0, its length along
x. It comes back as its **parts**, a list of `(material, mesh)`, because a
concrete lump and its rebar are two materials. Each mesh is a set of closed
solids facing out. Sizes are real ones, in metres.

Each item has a few **variants**, `variant(name, k)`: variant k is always
built from `Random(KIT_SEED, name, k)`, so it is the same mesh in every Map,
and a scatter of a thousand bricks reuses a handful of shapes, turned and
scaled, instead of making a thousand.

The materials are the library's own (`assets-src/materials.toml`): no new
picture is needed. The kit has no rubber, so a tyre and a cable are
`concrete_dark`, a dark grey.

Adding an item: write a function `rnd -> parts` below and add an `Item` to
`KIT` with its name. `test_odkit.py` then checks it like the others: closed,
facing out, standing on z = 0, the same bytes twice.
"""
import math

import odgeom as g
from odcut import merge
from odnoise import Random, noise2

KIT_SEED = 339

BRICK = "brick_red_weathered"
CONCRETE = "concrete_wall_rough"
PLASTER = "concrete_smooth"
REBAR = "steel_rusty"
STEEL = "steel_rusty"
PAINTED = "steel_painted"
SHEET = "roof_corrugated_rusted"
WOOD = "wood_ramp"
RUBBER = "concrete_dark"
TIN = "steel_grey"
PLASTIC = ("container_blue", "container_red")


# ---------------------------------------------------------------- building blocks

def turned(yaw=0.0, pitch=0.0, roll=0.0):
    """Turn rows: roll about x, then pitch about y, then yaw about z (degrees,
    anticlockwise seen from the end of the axis)."""
    return g.mat_mul(g.rot_z(yaw), g.mat_mul(g.rot_y(pitch), g.rot_x(roll)))


def _box(parts, size, centre, rows=None):
    """Adds a box, `size` (x, y, z), turned by `rows` and centred on `centre`."""
    corners, faces = g.box_mesh(*size)
    pose = g.Pose(rows or turned(), centre)
    parts.append(([pose.apply(c) for c in corners], faces))


def _bar(parts, p, q, w, h=None):
    """Adds a square bar from `p` to `q`, `w` by `h` across."""
    pose = g.aiming(p, q)
    corners, faces = g.box_mesh(g.length(g.sub(q, p)), w, h if h is not None else w)
    pose = g.Pose(pose.rows, g.lerp(p, q, 0.5))
    parts.append(([pose.apply(c) for c in corners], faces))


def _cyl_between(parts, p, q, r, r1=None, segments=10):
    """Adds a cylinder from `p` to `q`, `r` across at `p` and `r1` at `q`."""
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


def moved(mesh, rows=None, at=(0.0, 0.0, 0.0)):
    """The mesh turned by `rows` and moved by `at`, not snapped."""
    pose = g.Pose(rows or turned(), at)
    return [pose.apply(c) for c in mesh[0]], list(mesh[1])


def on_floor(mesh):
    """The mesh moved up or down so its lowest corner is on z = 0."""
    low = min(c[2] for c in mesh[0])
    return [(x, y, z - low) for x, y, z in mesh[0]], list(mesh[1])


def centred(mesh):
    """The mesh moved so the middle of its footprint is on x = y = 0."""
    xs = [c[0] for c in mesh[0]]
    ys = [c[1] for c in mesh[0]]
    cx, cy = (min(xs) + max(xs)) / 2.0, (min(ys) + max(ys)) / 2.0
    return [(x - cx, y - cy, z) for x, y, z in mesh[0]], list(mesh[1])


def extent(parts):
    """`(x0, y0, z0, x1, y1, z1)` round every part of an item."""
    cs = [c for _, (corners, _) in parts for c in corners]
    return tuple(min(c[i] for c in cs) for i in range(3)) + tuple(max(c[i] for c in cs) for i in range(3))


def _finish(parts):
    """Every part on the floor together and centred together, and the parts of
    one material merged, in the order they were first made."""
    corners = [c for _, (cs, _) in parts for c in cs]
    low = min(c[2] for c in corners)
    xs, ys = [c[0] for c in corners], [c[1] for c in corners]
    cx, cy = (min(xs) + max(xs)) / 2.0, (min(ys) + max(ys)) / 2.0
    order, by = [], {}
    for material, (cs, fs) in parts:
        if material not in by:
            order.append(material)
            by[material] = []
        by[material].append(([(x - cx, y - cy, z - low) for x, y, z in cs], fs))
    return [(material, merge(by[material])) for material in order]


def chipped_box(rnd, sx, sy, sz, chip):
    """A box whose eight corners are each knocked in by up to `chip` metres,
    so no two bricks are the same and none is a perfect box."""
    corners, faces = g.box_mesh(sx, sy, sz)
    out = []
    for x, y, z in corners:
        k = rnd.uniform(0.0, chip)
        out.append((x - math.copysign(k * rnd.unit(), x), y - math.copysign(k * rnd.unit(), y),
                    z - math.copysign(k * rnd.unit(), z)))
    return out, faces


def lump(rnd, sx, sy, sz, around=6, bands=3, rough=0.25):
    """A broken lump: a ball of `around` sides and `bands` bands, each corner
    pushed in or out by up to `rough` of its radius, scaled to `sx` by `sy`
    by `sz`. Few sides, so it reads as faceted broken stuff, not a pebble."""
    corners = [(0.0, 0.0, -0.5 * sz * rnd.uniform(1.0 - rough, 1.0))]
    phase = rnd.uniform(0.0, 2.0 * math.pi)
    for b in range(1, bands + 1):
        lat = -math.pi / 2.0 + math.pi * b / (bands + 1)
        for i in range(around):
            lon = phase + 2.0 * math.pi * (i + 0.5 * (b % 2)) / around
            r = rnd.uniform(1.0 - rough, 1.0 + rough * 0.4)
            corners.append((0.5 * sx * r * math.cos(lat) * math.cos(lon),
                            0.5 * sy * r * math.cos(lat) * math.sin(lon),
                            0.5 * sz * r * math.sin(lat)))
    corners.append((0.0, 0.0, 0.5 * sz * rnd.uniform(1.0 - rough, 1.0)))
    top = len(corners) - 1
    faces = []
    for i in range(around):
        j = (i + 1) % around
        faces.append((0, 1 + j, 1 + i))
        faces.append((top, 1 + (bands - 1) * around + i, 1 + (bands - 1) * around + j))
    for b in range(bands - 1):
        r0, r1 = 1 + b * around, 1 + (b + 1) * around
        for i in range(around):
            j = (i + 1) % around
            faces.append((r0 + i, r0 + j, r1 + j))
            faces.append((r0 + i, r1 + j, r1 + i))
    return corners, g.point_outwards(corners, faces)


def slab_outline(points, thick):
    """A flat outline of `(x, y)` points, anticlockwise, made a plate `thick`
    tall standing on z = 0."""
    k = len(points)
    corners = [(x, y, 0.0) for x, y in points] + [(x, y, thick) for x, y in points]
    faces = [(i, (i + 1) % k, k + (i + 1) % k, k + i) for i in range(k)]
    for a, b, c in g.triangulate_polygon(points):
        faces.append((a, c, b))
        faces.append((k + a, k + b, k + c))
    return corners, g.point_outwards(corners, faces)


def ragged_outline(rnd, rx, ry, sides):
    """A convex-ish outline about `rx` by `ry`, its corners at uneven angles
    and distances: a chip, a scrap, a flake."""
    angles = sorted(rnd.uniform(0.0, 2.0 * math.pi) for _ in range(sides))
    # Keep the corners spread, so no two are on top of each other.
    angles = [2.0 * math.pi * (i + 0.3 * (a / (2.0 * math.pi))) / sides for i, a in enumerate(angles)]
    return [(rx * rnd.uniform(0.6, 1.0) * math.cos(a), ry * rnd.uniform(0.6, 1.0) * math.sin(a)) for a in angles]


def bar(length, w, h, segments=1, bend=None):
    """A square bar along x from 0 to `length`, `w` by `h` across, cut in
    `segments` along its length; `bend(x) -> (dy, dz)` bows it."""
    corners = []
    for s in range(segments + 1):
        x = length * s / segments
        dy, dz = bend(x) if bend else (0.0, 0.0)
        for y, z in ((-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2)):
            corners.append((x, y + dy, z + dz))
    faces = []
    for s in range(segments):
        a, b = 4 * s, 4 * (s + 1)
        for i in range(4):
            j = (i + 1) % 4
            faces.append((a + i, a + j, b + j, b + i))
    faces.append((3, 2, 1, 0))
    e = 4 * segments
    faces.append((e, e + 1, e + 2, e + 3))
    return corners, g.point_outwards(corners, faces)


def _cyl(height, r, segments, r1=None):
    return g.cylinder_mesh(height, r, r1, segments)


# ---------------------------------------------------------------- masonry

def brick(rnd):
    """A whole brick, 215 x 102.5 x 65 mm, its corners chipped."""
    return [(BRICK, chipped_box(rnd, 0.215, 0.1025, 0.065, 0.012))]


def brick_chunk(rnd):
    """A half or a third of a brick, broken ragged."""
    length = rnd.uniform(0.07, 0.13)
    return [(BRICK, lump(rnd, length, 0.1, 0.065, around=5, bands=2, rough=0.18))]


def concrete_lump(rnd):
    """A lump of broken concrete 0.2 to 0.5 m across, with one to three bent
    rebar ends sticking out of it."""
    s = rnd.uniform(0.2, 0.5)
    sx, sy, sz = s, s * rnd.uniform(0.55, 0.9), s * rnd.uniform(0.35, 0.6)
    parts = [(CONCRETE, lump(rnd, sx, sy, sz, around=7, bands=3, rough=0.22))]
    for _ in range(1 + int(rnd.unit() * 3)):
        a = rnd.uniform(0.0, 2.0 * math.pi)
        z = rnd.uniform(-0.15, 0.2) * sz
        start = (0.3 * sx * math.cos(a), 0.3 * sy * math.sin(a), z)
        reach = rnd.uniform(0.15, 0.45)
        mid = (start[0] + 0.6 * reach * math.cos(a), start[1] + 0.6 * reach * math.sin(a),
               z + rnd.uniform(-0.03, 0.08))
        bend = a + rnd.uniform(-1.2, 1.2)
        end = (mid[0] + 0.5 * reach * math.cos(bend), mid[1] + 0.5 * reach * math.sin(bend),
               mid[2] + rnd.uniform(-0.05, 0.12))
        parts.append((REBAR, g.tube_mesh([start, mid, end], 0.006, 5)))
    return _finish(parts)


def concrete_chunk(rnd):
    """A fist-sized bit of concrete, 6 to 15 cm."""
    s = rnd.uniform(0.06, 0.15)
    return [(CONCRETE, lump(rnd, s, s * rnd.uniform(0.6, 0.9), s * rnd.uniform(0.4, 0.7), around=5, bands=2, rough=0.25))]


def plaster_chip(rnd):
    """A flake of plaster or render 4 to 12 cm across and 5 to 9 mm thick."""
    r = rnd.uniform(0.02, 0.06)
    pts = ragged_outline(rnd, r, r * rnd.uniform(0.5, 1.0), 5 + int(rnd.unit() * 3))
    return [(PLASTER, slab_outline(pts, rnd.uniform(0.005, 0.009)))]


# ---------------------------------------------------------------- roof and wood

def corrugated(length, width, thick, pitch=0.076, depth=0.018, crumple=None):
    """A strip of corrugated sheet `length` along x, `width` across y, its
    ridges running along x; `crumple(x, y) -> dz` bends it."""
    waves = max(1, round(width / pitch))
    n = waves * 2  # a ridge and a trough a wave: a zigzag, which reads as corrugation at a few metres
    profile = [(-width / 2 + width * i / n, depth / 2 * (1.0 if i % 2 else -1.0)) for i in range(n + 1)]
    steps = max(1, round(length / 0.35))
    corners = []
    for s in range(steps + 1):
        x = -length / 2 + length * s / steps
        for side in (0.0, thick):
            for y, z in profile:
                dz = crumple(x, y) if crumple else 0.0
                corners.append((x, y, z + dz + side))
    m = len(profile)
    ring = 2 * m

    def at(s, side, i):
        return s * ring + side * m + i
    faces = []
    for s in range(steps):
        for i in range(m - 1):
            faces.append((at(s, 1, i), at(s, 1, i + 1), at(s + 1, 1, i + 1), at(s + 1, 1, i)))  # top
            faces.append((at(s, 0, i), at(s + 1, 0, i), at(s + 1, 0, i + 1), at(s, 0, i + 1)))  # bottom
        for i, (a, b) in ((0, (1, 0)), (m - 1, (0, 1))):  # the long edges
            faces.append((at(s, a, i), at(s + 1, a, i), at(s + 1, b, i), at(s, b, i)))
    for s, flip in ((0, False), (steps, True)):  # the cut ends
        for i in range(m - 1):
            f = (at(s, 0, i), at(s, 0, i + 1), at(s, 1, i + 1), at(s, 1, i))
            faces.append(tuple(reversed(f)) if flip else f)
    return corners, g.point_outwards(corners, faces)


def roof_sheet_piece(rnd):
    """A torn piece of corrugated roof sheet, 0.5 to 1.3 m long, bent and
    buckled where it fell."""
    length, width = rnd.uniform(0.5, 1.3), rnd.uniform(0.35, 0.8)
    bow, kink, twist = rnd.uniform(-0.15, 0.25), rnd.uniform(-0.6, 0.6), rnd.uniform(-0.25, 0.25)
    kx = rnd.uniform(-0.3, 0.3) * length

    def crumple(x, y):
        return bow * (1.0 - (2.0 * x / length) ** 2) * 0.3 + kink * max(0.0, x - kx) * 0.4 + twist * x * y
    return [(SHEET, on_floor(corrugated(length, width, 0.004, crumple=crumple)))]


def plank(rnd):
    """A scaffold board or plank, 1.5 to 3 m long, bowed a little, sometimes
    snapped short."""
    length = rnd.uniform(1.5, 3.0) if rnd.chance(0.7) else rnd.uniform(0.6, 1.2)
    w, h = rnd.uniform(0.1, 0.22), rnd.uniform(0.022, 0.038)
    bow = rnd.uniform(-0.03, 0.03)
    mesh = bar(length, w, h, 4, lambda x: (0.0, bow * math.sin(math.pi * x / length)))
    return [(WOOD, on_floor(centred(mesh)))]


def board(rnd):
    """An offcut of board, 0.4 to 1 m long and 15 to 30 cm wide."""
    mesh = chipped_box(rnd, rnd.uniform(0.4, 1.0), rnd.uniform(0.15, 0.3), rnd.uniform(0.012, 0.025), 0.004)
    return [(WOOD, on_floor(mesh))]


def pallet_mesh(rnd, missing=0.25, broken=0.2):
    """A Euro pallet, 1.2 x 0.8 x 0.144 m: three bottom boards, nine blocks,
    three cross boards and five deck boards; each deck board between the outer
    two is gone with chance `missing`, and each is broken off short with chance
    `broken`."""
    parts = []
    t = 0.022

    def box(size, centre):
        parts.append(moved(g.box_mesh(*size), None, centre))
    for y in (-0.35, 0.0, 0.35):
        box((1.2, 0.1, t), (0.0, y, t / 2))
        for x in (-0.525, 0.0, 0.525):
            box((0.145, 0.1, 0.078), (x, y, t + 0.039))
    for x in (-0.525, 0.0, 0.525):
        box((0.145, 0.8, t), (x, 0.0, 0.1 + t / 2))
    for k, y in enumerate((-0.3275, -0.164, 0.0, 0.164, 0.3275)):
        width = 0.145 if k in (0, 2, 4) else 0.1
        if k not in (0, 4) and rnd.chance(missing):
            continue
        length, x = 1.2, 0.0
        if rnd.chance(broken):
            length = rnd.uniform(0.45, 0.85)
            x = (0.6 - length / 2) * (1.0 if rnd.chance(0.5) else -1.0)
        box((length, width, t), (x, y, 0.122 + t / 2))
    return merge(parts)


def pallet(rnd):
    """One pallet, some deck boards gone or broken off short."""
    return [(WOOD, pallet_mesh(rnd))]


def pallet_stack(rnd):
    """Two to six pallets stacked by hand: each one a little off square and
    turned a few degrees, the top one sometimes askew."""
    parts = []
    z = 0.0
    for k in range(2 + int(rnd.unit() * 5)):
        yaw = rnd.uniform(-4.0, 4.0) if k else 0.0
        if k > 1 and rnd.chance(0.25):
            yaw += rnd.uniform(-25.0, 25.0)
        at = (rnd.uniform(-0.05, 0.05), rnd.uniform(-0.04, 0.04), z)
        parts.append((WOOD, moved(pallet_mesh(rnd.child("pallet", k)), turned(yaw), at)))
        z += 0.144
    return _finish(parts)


# ---------------------------------------------------------------- steel

def pipe(rnd):
    """A length of steel pipe, 1 to 3 m, 10 to 30 cm across."""
    length, r = rnd.uniform(1.0, 3.0), rnd.uniform(0.05, 0.15)
    mesh = moved(g.annulus_mesh(length, r * 0.85, r, 12), turned(0.0, 90.0), (-length / 2, 0.0, 0.0))
    return [(STEEL, on_floor(mesh))]


def pipe_run(rnd):
    """A fallen pipe run: two or three lengths still joined at their flanges,
    kinked where they broke, one end lifted on its bracket."""
    r = rnd.uniform(0.06, 0.12)
    parts = []
    x, y, z, heading = 0.0, 0.0, r, 0.0
    lift = rnd.uniform(0.0, 0.35)
    count = 2 + int(rnd.unit() * 2)
    for k in range(count):
        length = rnd.uniform(1.5, 2.6)
        rise = lift / count if k == count - 1 else 0.0
        end = (x + length * math.cos(heading), y + length * math.sin(heading), z + rise)
        rows = g.along((x, y, z), end).rows
        parts.append((STEEL, moved(g.annulus_mesh(g.length(g.sub(end, (x, y, z))), r * 0.85, r, 12), rows, (x, y, z))))
        parts.append((STEEL, moved(_cyl(0.03, r * 1.5, 12), rows, (x, y, z))))  # its flange
        x, y, z = end
        heading += math.radians(rnd.uniform(-18.0, 18.0))
    return _finish(parts)


def drum_mesh(rnd, dented=0.0):
    """A 200 litre oil drum, 0.88 m tall and 0.58 across, with its two rolling
    hoops; `dented` (0 to 1) pushes a dent up to 8 cm deep into one side."""
    parts = []
    _cyl_between(parts, (0, 0, 0), (0, 0, 0.88), 0.29, segments=14)
    for z in (0.29, 0.59):
        _cyl_between(parts, (0, 0, z), (0, 0, z + 0.025), 0.302, segments=14)
    mesh = merge(parts)
    if dented <= 0.0:
        return mesh
    a = rnd.uniform(0.0, 2.0 * math.pi)
    dx, dy, zc = math.cos(a), math.sin(a), rnd.uniform(0.3, 0.6)

    def dent(x, y, z):
        k = max(0.0, 1.0 - ((z - zc) / 0.25) ** 2) * max(0.0, (x * dx + y * dy) / 0.3)
        return (x - dx * dented * 0.08 * k, y - dy * dented * 0.08 * k, z)
    return deform(mesh, dent)


def drum(rnd):
    """A 200 litre drum, rusted, painted or blue plastic-coated, half of them
    dented."""
    material = rnd.pick((STEEL, STEEL, PAINTED, PLASTIC[0]))
    return [(material, drum_mesh(rnd, 0.875 if rnd.chance(0.5) else 0.0))]


def drum_lying(rnd):
    """A drum on its side."""
    material, mesh = drum(rnd)[0]
    return [(material, on_floor(centred(moved(mesh, turned(0.0, 90.0)))))]


def anchor_stubs(rnd):
    """Four anchor bolts cut off a few centimetres above the floor, where a
    machine stood, with a nut left on one or two."""
    parts = []
    sx, sy = rnd.uniform(0.25, 0.6), rnd.uniform(0.25, 0.6)
    for cx in (-1, 1):
        for cy in (-1, 1):
            if rnd.chance(0.12):
                continue
            h = rnd.uniform(0.02, 0.07)
            at = (cx * sx / 2, cy * sy / 2, 0.0)
            parts.append((STEEL, moved(_cyl(h, rnd.uniform(0.009, 0.013), 6), None, at)))
            if rnd.chance(0.4):
                parts.append((STEEL, moved(_cyl(0.016, 0.019, 6), turned(rnd.uniform(0.0, 60.0)), at)))
    if not parts:
        parts.append((STEEL, _cyl(0.04, 0.011, 6)))
    return _finish(parts)


def bolt_plate(rnd):
    """A column's base plate left in the floor when the column was cut away:
    a steel plate 0.3 to 0.45 m square, four bolts and nuts, and often the
    stub of the column on it."""
    s = rnd.uniform(0.3, 0.45)
    t = rnd.uniform(0.015, 0.025)
    parts = [(STEEL, moved(chipped_box(rnd, s, s * rnd.uniform(0.9, 1.0), t, 0.003), None, (0.0, 0.0, t / 2)))]
    for cx in (-1, 1):
        for cy in (-1, 1):
            at = (cx * (s / 2 - 0.045), cy * (s / 2 - 0.045), t)
            parts.append((STEEL, moved(_cyl(rnd.uniform(0.03, 0.05), 0.011, 6), None, at)))
            parts.append((STEEL, moved(_cyl(0.018, 0.02, 6), turned(rnd.uniform(0.0, 60.0)), at)))
    if rnd.chance(0.6):  # an H-section stub, cut at a slant
        h = rnd.uniform(0.04, 0.25)
        w = s * 0.6
        for y in (-w / 2 + 0.006, w / 2 - 0.006):
            parts.append((STEEL, moved(g.box_mesh(w, 0.012, h), None, (0.0, y, t + h / 2))))
        parts.append((STEEL, moved(g.box_mesh(0.01, w - 0.024, h * 0.9), None, (0.0, 0.0, t + h * 0.45))))
    return _finish(parts)


def cable_tangle(rnd):
    """A tangle of stripped cable lying in loops, about a metre across."""
    pts = []
    a, r = rnd.uniform(0.0, 2.0 * math.pi), rnd.uniform(0.2, 0.45)
    cx, cy = 0.0, 0.0
    for i in range(18):
        a += rnd.uniform(0.5, 1.1)
        cx += rnd.uniform(-0.04, 0.04)
        cy += rnd.uniform(-0.04, 0.04)
        rr = r * rnd.uniform(0.7, 1.2)
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a), 0.01 + 0.02 * (i % 3) + rnd.uniform(0.0, 0.03)))
    return [(RUBBER, on_floor(g.tube_mesh(pts, rnd.uniform(0.006, 0.01), 5)))]


def tyre_mesh(width=0.2, segments=16):
    """A car tyre lying flat: 0.62 across, `width` tall, 0.4 inside."""
    return g.annulus_mesh(width, 0.2, 0.31, segments)


def tyre(rnd):
    """A car tyre, 0.62 across and 0.17 to 0.22 wide, lying flat or leaning."""
    mesh = tyre_mesh(rnd.uniform(0.17, 0.22))
    lean = rnd.uniform(0.0, 25.0) if rnd.chance(0.5) else 0.0
    return [(RUBBER, on_floor(moved(mesh, turned(0.0, lean))))]


def can_mesh(rnd, crushed=0.0):
    """A drinks can, 0.115 m tall and 0.066 across, standing; `crushed` (0 to
    1) squashes it flat and buckles it."""
    mesh = g.cylinder_mesh(0.115, 0.033, None, 8)
    if crushed <= 0.0:
        return mesh
    squash = 1.0 - 0.7 * crushed
    kx, ky = rnd.uniform(-0.3, 0.3), rnd.uniform(-0.3, 0.3)

    def crush(x, y, z):
        bulge = 1.0 + 0.4 * crushed * (z / 0.115) * (1.0 - z / 0.115) * 4.0
        return (x * bulge + kx * z * crushed, y * bulge * (1.0 - 0.3 * crushed) + ky * z * crushed, z * squash)
    return deform(mesh, crush)


def can(rnd):
    """A drinks can, 0.115 m tall, standing, on its side or crushed."""
    kind = rnd.unit()
    if kind < 0.4:
        mesh = can_mesh(rnd, rnd.uniform(0.55, 0.93))
    else:
        mesh = can_mesh(rnd)
        if kind < 0.8:
            mesh = moved(mesh, turned(0.0, 90.0))
    return [(TIN, on_floor(centred(mesh)))]


def bottle_mesh(rnd, kind="beer"):
    """A bottle, standing: a body, a shoulder and a neck. `beer` is 0.24 m,
    `wine` 0.3."""
    h, r = (0.15, 0.032) if kind == "beer" else (0.2, 0.037)
    h *= rnd.uniform(0.95, 1.05)
    parts = []
    _cyl_between(parts, (0, 0, 0), (0, 0, h), r, segments=8)
    _cyl_between(parts, (0, 0, h), (0, 0, h + 0.04), r, 0.013, segments=8)
    _cyl_between(parts, (0, 0, h + 0.04), (0, 0, h + 0.09), 0.013, 0.014, segments=6)
    return merge(parts)


def bottle(rnd):
    """A bottle, 0.24 to 0.3 m, most of them lying down."""
    mesh = bottle_mesh(rnd, "beer" if rnd.chance(0.6) else "wine")
    if rnd.chance(0.7):
        mesh = moved(mesh, turned(rnd.uniform(0.0, 180.0), 90.0))
    return [(TIN, on_floor(centred(mesh)))]


def plastic_scrap(rnd):
    """A broken piece of plastic crate or sheet, 10 to 30 cm, bent."""
    r = rnd.uniform(0.05, 0.15)
    pts = ragged_outline(rnd, r, r * rnd.uniform(0.4, 0.9), 6)
    corners, faces = slab_outline(pts, 0.006)
    k = rnd.uniform(-2.0, 2.0)
    corners = [(x, y, z + k * x * x) for x, y, z in corners]
    return [(rnd.pick(PLASTIC), on_floor((corners, faces)))]


# ---------------------------------------------------------------- pieces for hand placement

# These come back as one mesh, not parts, and a Map's script puts each where
# its story wants it (a bed frame in the driest corner, a gate on its hinge)
# with `place` or a pose, in its own material. Each takes its own `Random`.
# The five that are also kit items (`pallet_mesh`, `drum_mesh`, `tyre_mesh`,
# `can_mesh`, `bottle_mesh`) sit with them above, with the knobs a hand
# placement needs.

def place(batch, mesh, at, yaw=0.0, pitch=0.0, roll=0.0):
    """Adds a piece to an `odruin.Batch`, turned and moved to `at` (x, y, z)."""
    batch.add(mesh, g.Pose(turned(yaw, pitch, roll), at))
    return batch


def cable_drum():
    """A wooden cable drum stood on one flange as a table: flanges 1 m across
    and 0.05 thick, a core 0.44 across, 0.62 tall."""
    parts = []
    _cyl_between(parts, (0, 0, 0.0), (0, 0, 0.05), 0.5, segments=18)
    _cyl_between(parts, (0, 0, 0.05), (0, 0, 0.57), 0.22, segments=12)
    _cyl_between(parts, (0, 0, 0.57), (0, 0, 0.62), 0.5, segments=18)
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
    _cyl_between(parts, (0, 0, 0.47), (0.02, 0.0, 0.6), 0.05, 0.02, segments=6)
    return merge(parts)


def plastic_chair(rnd, melt=0.0):
    """A stacking garden chair: four legs, a seat and a back. `melt` (0 to
    1) slumps it as a fire leaves one: the seat sags and the whole chair
    leans and sinks toward one side."""
    parts = []
    for sx in (-1, 1):
        for sy in (-1, 1):
            _cyl_between(parts, (sx * 0.2, sy * 0.2, 0.0), (sx * 0.19, sy * 0.19, 0.42), 0.018, segments=6)
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


# ---------------------------------------------------------------- steel and furniture

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
            _cyl_between(parts, (x, y - 0.02, 0.06), (x, y + 0.02, 0.06), 0.06, segments=8)
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
        _cyl_between(parts, (0.3 * math.cos(a), 0.3 * math.sin(a), 0.0), (0.3 * math.cos(a), 0.3 * math.sin(a), 0.05), 0.025, segments=6)
    _cyl_between(parts, (0, 0, 0.08), (0, 0, 0.42), 0.025, segments=8)
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
            _cyl_between(parts, (x, y, h - 0.005), (x, y, h + 0.012), 0.012, segments=6)
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
    motor to motor, a body, four motors and their propellers; `arms_gone` arms are
    snapped off near the body (and their motors gone)."""
    parts = []
    half = size / 2 / math.sqrt(2.0)
    _box(parts, (size * 0.35, size * 0.18, size * 0.12), (0, 0, size * 0.06 + 0.004))
    for k, (sx, sy) in enumerate(((1, 1), (-1, 1), (-1, -1), (1, -1))):
        if k < arms_gone:
            _bar(parts, (0, 0, 0.004), (sx * half * 0.35, sy * half * 0.35, 0.004), size * 0.06, 0.004)
            continue
        _bar(parts, (0, 0, 0.004), (sx * half, sy * half, 0.004), size * 0.06, 0.004)
        _cyl_between(parts, (sx * half, sy * half, 0.006), (sx * half, sy * half, 0.006 + size * 0.08), size * 0.055, segments=8)
        a = rnd.uniform(0, 180)
        r = size * 0.24
        top = 0.006 + size * 0.08 + 0.002
        _bar(parts, (sx * half - r * math.cos(math.radians(a)), sy * half - r * math.sin(math.radians(a)), top),
             (sx * half + r * math.cos(math.radians(a)), sy * half + r * math.sin(math.radians(a)), top), size * 0.04, 0.003)
    return merge(parts)


def propeller(rnd, diameter=0.127, blades=3, snapped=1):
    """A loose propeller (a 5-inch one is 0.127 m): a hub and its blades,
    `snapped` of them broken short."""
    parts = []
    _cyl_between(parts, (0, 0, 0), (0, 0, 0.008), 0.007, segments=6)
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
    _cyl_between(parts, (length, 0.04, 0.0), (length, 0.04, 0.02), 0.01, segments=6)
    return merge(parts)


# ---------------------------------------------------------------- the kit

class Item:
    """A kit item: how to build it, how many variants a scatter draws from,
    how far it sinks into what it lies on (a share of its height, from the
    seed between the two numbers), and whether it is big enough for the Quad
    to hit, so a scatter gives it a collision box when it stands upright."""

    def __init__(self, build, variants=4, sink=(0.0, 0.0), solid=False):
        self.build, self.variants, self.sink, self.solid = build, variants, sink, solid


KIT = {
    "brick": Item(brick, 6, (0.0, 0.25)),
    "brick_chunk": Item(brick_chunk, 6, (0.1, 0.35)),
    "concrete_lump": Item(concrete_lump, 6, (0.15, 0.35)),
    "concrete_chunk": Item(concrete_chunk, 6, (0.1, 0.35)),
    "plaster_chip": Item(plaster_chip, 8),
    "roof_sheet_piece": Item(roof_sheet_piece, 4),
    "plank": Item(plank, 5),
    "board": Item(board, 5),
    "pallet": Item(pallet, 4, solid=True),
    "pallet_stack": Item(pallet_stack, 4, solid=True),
    "pipe": Item(pipe, 3, solid=True),
    "pipe_run": Item(pipe_run, 3),
    "drum": Item(drum, 4, solid=True),
    "drum_lying": Item(drum_lying, 3, solid=True),
    "cable_tangle": Item(cable_tangle, 3),
    "tyre": Item(tyre, 3, (0.0, 0.2)),
    "can": Item(can, 5),
    "bottle": Item(bottle, 4),
    "plastic_scrap": Item(plastic_scrap, 5),
    "anchor_stubs": Item(anchor_stubs, 4),
    "bolt_plate": Item(bolt_plate, 4),
}

_CACHE = {}


def variant(name, k):
    """Variant `k` of the kit item `name` (any whole number; it wraps round
    the item's variants): its parts, `[(material, mesh)]`, standing on z = 0
    and centred, the same in every Map."""
    item = KIT[name]
    k = k % item.variants
    if (name, k) not in _CACHE:
        _CACHE[(name, k)] = _finish(item.build(Random(KIT_SEED, name, k)))
    return _CACHE[(name, k)]
