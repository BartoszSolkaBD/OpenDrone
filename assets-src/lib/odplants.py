"""Weeds and self-sown trees for detailed Maps, in plain Python (MIT OR Apache-2.0).

The maintainer's bar for the Factory (#290, #295): plants that look like
plants, not boxes and cones. The game has no see-through pictures, so a plant
here is made of real shapes: a weed is a clump of thin, bent blades and a few
broad leaves; a tree is a leaning trunk, branches and twigs as tapered round
bars, and a crown of hundreds of small leaves, each a flat diamond a few
millimetres thick (so it shows from both sides). Everything comes from a seed
(`odruin.Random`) and is the same on every computer.

Each function returns `odruin.Batch` meshes, placed in the Map, for
`MapBuilder.solid`: the wood of a tree is solid (`trimesh`), its leaves and the
weeds are only drawn (`none`), on purpose: a Quad flies through leaves and
grass, as a real one mostly does.
"""
import math

import odgeom as g
import odruin as r

UP = (0.0, 0.0, 1.0)


def _horizontal(deg):
    return (math.cos(math.radians(deg)), math.sin(math.radians(deg)), 0.0)


def _leaning(direction, deg):
    """Up, leant `deg` degrees towards the horizontal `direction`."""
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return g.unit(g.add(g.mul(UP, c), g.mul(direction, s)))


def leaf(length, width, gap=0.002):
    """A leaf in its own axes, pointing along +x from the origin: a flat
    diamond drawn from both sides, as two cards `gap` apart facing away from
    each other (the game draws only a face's front). Four triangles; not a
    solid, so only for parts that are drawn and not touched."""
    h = gap / 2.0
    corners = [
        (0.0, 0.0, h),
        (length * 0.4, width / 2.0, h),
        (length, 0.0, h),
        (length * 0.4, -width / 2.0, h),
        (0.0, 0.0, -h),
        (length * 0.4, width / 2.0, -h),
        (length, 0.0, -h),
        (length * 0.4, -width / 2.0, -h),
    ]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7)]
    return corners, faces


def weeds(rnd, centre, radius, blades, height, z=0.0):
    """A clump of weeds round `centre` (x, y) standing on `z`: `blades` thin
    blades up to `height` tall, bending outwards, and a rosette of broad
    leaves at the foot. A Batch, to be drawn only."""
    out = r.Batch()
    cx, cy = centre
    for _ in range(blades):
        d = radius * math.sqrt(rnd.unit())
        at = rnd.uniform(0.0, 360.0)
        p0 = (cx + d * math.cos(math.radians(at)), cy + d * math.sin(math.radians(at)), z - 0.01)
        out_dir = _horizontal(at + rnd.uniform(-50.0, 50.0))
        across = g.unit(g.cross(UP, out_dir))
        h = height * rnd.uniform(0.45, 1.05)
        w = rnd.uniform(0.012, 0.03)
        lean = rnd.uniform(4.0, 22.0)
        b1 = _leaning(out_dir, lean)
        low = h * rnd.uniform(0.45, 0.6)
        out.add(r.plate(p0, across, b1, [(-w / 2, 0.0), (w / 2, 0.0), (w * 0.35, low), (-w * 0.35, low)], 0.003))
        p1 = g.add(p0, g.mul(b1, low))
        b2 = _leaning(out_dir, lean + rnd.uniform(15.0, 45.0))
        out.add(r.plate(p1, across, b2, [(-w * 0.35, 0.0), (w * 0.35, 0.0), (0.0, h - low)], 0.003))
    for k in range(rnd.pick((3, 4, 5, 6))):
        at = k * 360.0 / 6 + rnd.uniform(0.0, 50.0)
        size = height * rnd.uniform(0.25, 0.45)
        rows = r.turned(at, -rnd.uniform(15.0, 40.0), rnd.uniform(-15.0, 15.0))
        out.add(leaf(size, size * 0.5, 0.004), g.Pose(rows, (cx, cy, z + 0.01)))
    return out


def _bar(batch, p0, p1, r0, r1, segments=7):
    batch.add(g.cylinder_mesh(g.length(g.sub(p1, p0)), r0, r1, segments), g.along(p0, p1))


def tree(rnd, base, height, leaves=1400):
    """A self-sown tree on `base` (x, y, z), `height` tall: a trunk that
    leans a little, branches and twigs, and a crown of about `leaves` leaves
    clustered at the twigs' ends. Returns `(wood, foliage)`, two Batches."""
    wood = r.Batch()
    crown = r.Batch()
    lean_dir = _horizontal(rnd.uniform(0.0, 360.0))
    radius = 0.06 + height * 0.011
    # The trunk: three bars, each leaning a little more.
    pts = [base]
    for k in range(3):
        step = height * 0.28
        d = _leaning(lean_dir, rnd.uniform(2.0, 7.0) * (k + 1))
        jog = _horizontal(rnd.uniform(0.0, 360.0))
        pts.append(g.add(g.add(pts[-1], g.mul(d, step)), g.mul(jog, step * 0.05)))
    radii = [radius, radius * 0.8, radius * 0.6, radius * 0.4]
    for k in range(3):
        _bar(wood, pts[k], pts[k + 1], radii[k], radii[k + 1], 8)

    def on_trunk(t):
        s = t * 3.0
        k = min(2, int(s))
        return g.lerp(pts[k], pts[k + 1], s - k), radii[k] + (radii[k + 1] - radii[k]) * (s - k)

    ends = []
    branches = 8
    for i in range(branches):
        t = 0.38 + 0.6 * i / branches + rnd.uniform(-0.03, 0.03)
        start, rt = on_trunk(min(t, 0.99))
        az = i * 137.5 + rnd.uniform(-20.0, 20.0)
        el = rnd.uniform(28.0, 55.0)
        dirn = g.unit(g.add(g.mul(_horizontal(az), math.cos(math.radians(el))), g.mul(UP, math.sin(math.radians(el)))))
        length = height * rnd.uniform(0.28, 0.38) * (1.15 - t * 0.5)
        end = g.add(start, g.mul(dirn, length))
        rb = rt * 0.55
        _bar(wood, start, end, rb, rb * 0.3, 6)
        for j in range(3):
            f = rnd.uniform(0.45, 1.0)
            s0 = g.lerp(start, end, f)
            tw_dir = g.unit(g.add(dirn, g.add(g.mul(_horizontal(az + rnd.uniform(-90.0, 90.0)), 0.8), (0.0, 0.0, rnd.uniform(0.0, 0.6)))))
            s1 = g.add(s0, g.mul(tw_dir, length * rnd.uniform(0.35, 0.55)))
            _bar(wood, s0, s1, max(0.012, rb * 0.35), 0.006, 5)
            ends.append((s0, s1))
        ends.append((g.lerp(start, end, 0.6), end))
    # The leader at the top.
    top = g.add(pts[-1], g.mul(_leaning(lean_dir, 5.0), height * 0.16))
    _bar(wood, pts[-1], top, radii[-1], 0.01, 6)
    ends.append((pts[-1], top))

    # Leaves: clustered round the outer part of every twig and branch end.
    per = max(1, leaves // len(ends))
    for s0, s1 in ends:
        span = g.length(g.sub(s1, s0))
        cluster = 0.45 + span * 0.3
        for _ in range(per):
            p = g.lerp(s0, s1, rnd.uniform(0.3, 1.1))
            off = (rnd.uniform(-1, 1), rnd.uniform(-1, 1), rnd.uniform(-0.6, 0.8))
            p = g.add(p, g.mul(off, cluster))
            size = rnd.uniform(0.22, 0.4)  # a spray of leaves, as the picture shows several
            rows = r.turned(rnd.uniform(0.0, 360.0), rnd.uniform(-35.0, 35.0), rnd.uniform(-40.0, 40.0))
            crown.add(leaf(size, size * rnd.uniform(0.45, 0.65)), g.Pose(rows, p))
    return wood, crown
