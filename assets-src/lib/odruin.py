"""Damage and clutter for detailed Maps, in plain Python (MIT OR Apache-2.0).

The Factory (#295) is a ruin: broken wall edges, missing bricks and panes,
torn roof sheets, bent and fallen beams, rubble. ADR-0024 keeps the rule that
a Map's `.glb` is the same bytes on every computer, so none of this uses
Blender's booleans, bevels or random numbers. Everything here is worked out
from a seed with 64-bit Python numbers, like `odgeom`, and hands back meshes
`(corners, faces)` already placed in the Map, for `MapBuilder.solid`.

- `Random` (from `odnoise`, the geometry helpers' own) gives the same numbers for the same seed and keys on every
  computer: SHA-256 of the keys, then splitmix64 in Python's whole numbers.
- `Batch` joins many small solids (bricks, chunks, leaves) into one mesh, so
  a pile of rubble is one part to draw, not hundreds.
- `plate` cuts a flat outline out of a sheet of some thickness, on any plane:
  a torn roof sheet, a shard of glass, a skin of plaster.
- `breach` and `missing_bricks` are the openings that `MapBuilder.wall` cuts:
  a hole broken through a wall, stepped to the bricks, and the pockets where
  single bricks have fallen out.
- `lump`, `brick` and `mound` are rubble; `bent_bar` is a beam that has
  buckled.
"""
import math

import odgeom as g
from odnoise import Random  # noqa: F401  (the one Random, shared with the geometry helpers, #322)


# A brick of the Factory's walls (`brick_red_weathered`: eight bricks of
# 225 mm across 1.8 m, 24 courses of 75 mm) with its share of mortar.
BRICK = 0.225
COURSE = 0.075


class Batch:
    """Many solids as one mesh. `add` takes a mesh in its own axes and a pose
    (or None, when it is placed already)."""

    def __init__(self):
        self.corners = []
        self.faces = []

    def add(self, mesh, pose=None):
        corners, faces = mesh
        base = len(self.corners)
        pose = pose or g.Pose()
        self.corners += [pose.apply(c) for c in corners]
        self.faces += [tuple(base + i for i in f) for f in faces]
        return self

    def empty(self):
        return not self.faces

    def mesh(self):
        return list(self.corners), list(self.faces)


def turned(yaw, pitch=0.0, roll=0.0):
    """Turn rows: roll about x, then pitch about y, then yaw about z, in
    degrees."""
    return g.mat_mul(g.rot_z(yaw), g.mat_mul(g.rot_y(pitch), g.rot_x(roll)))


def plate(origin, a, b, outline, thick):
    """A flat outline `[(s, t), ...]` (any simple polygon) laid on the plane
    through `origin` spanned by the unit directions `a` and `b`, and given
    `thick` metres of thickness along `a x b`. A closed solid, faces
    outwards."""
    n = g.unit(g.cross(a, b))
    k = len(outline)
    area = sum(outline[i][0] * outline[(i + 1) % k][1] - outline[(i + 1) % k][0] * outline[i][1] for i in range(k))
    if area < 0.0:
        outline = list(reversed(outline))  # anticlockwise, so the sides face out like the caps
    bottom = [g.add(origin, g.add(g.mul(a, s), g.mul(b, t))) for s, t in outline]
    top = [g.add(p, g.mul(n, thick)) for p in bottom]
    corners = bottom + top
    faces = [(i, (i + 1) % k, k + (i + 1) % k, k + i) for i in range(k)]
    for p, q, r in g.triangulate_polygon(outline):
        faces.append((p, r, q))
        faces.append((k + p, k + q, k + r))
    return corners, g.point_outwards(corners, faces)


def jagged(p0, p1, rnd, teeth, depth):
    """The points between `p0` and `p1` (2D, both left out) of a torn edge:
    `teeth` zigzags reaching up to `depth` to the left of the line."""
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    ln = math.sqrt(dx * dx + dy * dy)
    nx, ny = -dy / ln, dx / ln
    out = []
    for i in range(1, 2 * teeth):
        t = (i + rnd.uniform(-0.3, 0.3)) / (2 * teeth)
        d = depth * (rnd.uniform(0.5, 1.0) if i % 2 else rnd.uniform(0.0, 0.25))
        out.append((p0[0] + dx * t + nx * d, p0[1] + dy * t + ny * d))
    return out


def breach(centre_u, bottom_v, width, height, rnd, step=2 * COURSE):
    """A hole broken through a brick wall, as openings for `MapBuilder.wall`:
    rows of `step` metres (two courses), each as wide as an oval `width` by
    `height` is there, give or take half a brick at each end, so the edge is
    stepped the way bricks fall out. `(u0, u1, v0, v1)` each."""
    rows = max(2, round(height / step))
    out = []
    for r in range(rows):
        v0 = bottom_v + r * step
        mid = (r + 0.5) / rows * 2.0 - 1.0
        half = width / 2.0 * math.sqrt(max(0.0, 1.0 - mid * mid * 0.85))
        left = half + rnd.pick((-0.5, 0.0, 0.0, 0.5, 1.0)) * BRICK / 2.0
        right = half + rnd.pick((-0.5, 0.0, 0.0, 0.5, 1.0)) * BRICK / 2.0
        if left + right > BRICK:
            out.append((centre_u - left, centre_u + right, v0, v0 + step))
    return out


def missing_bricks(u0, u1, v0, v1, count, rnd):
    """Pockets where single bricks have fallen out of a wall's face, on the
    brick grid of a running bond, inside the rectangle. Cut them from one
    leaf of a wall built in two, so the wall stays whole behind them."""
    out = []
    taken = set()
    tries = 0
    while len(out) < count and tries < count * 20:
        tries += 1
        row = int(rnd.uniform(v0, v1 - COURSE) // COURSE)
        shift = BRICK / 2.0 if row % 2 else 0.0
        col = int((rnd.uniform(u0, u1 - BRICK) - shift) // BRICK)
        wide = 2 if rnd.chance(0.25) else 1
        if any((row, col + k) in taken for k in range(wide)):
            continue
        a = col * BRICK + shift
        b = a + BRICK * wide
        if a < u0 or b > u1:
            continue
        for k in range(wide):
            taken.add((row, col + k))
        out.append((a, b, row * COURSE, (row + 1) * COURSE))
    return sorted(out)


def lump(size, rnd):
    """A chunk of broken concrete or brickwork about `size` across, in its own
    axes: a box with every corner pushed in or out by up to a quarter."""
    hx = size * rnd.uniform(0.35, 0.6)
    hy = size * rnd.uniform(0.3, 0.5)
    hz = size * rnd.uniform(0.15, 0.35)
    corners, faces = g.box_mesh(2 * hx, 2 * hy, 2 * hz)
    corners = [
        (x * rnd.uniform(0.75, 1.2), y * rnd.uniform(0.75, 1.2), z * rnd.uniform(0.7, 1.15)) for x, y, z in corners
    ]
    return corners, g.point_outwards(corners, faces)


def brick(rnd, broken=0.4):
    """A loose brick, whole or a broken part of one, in its own axes."""
    length = BRICK * (rnd.uniform(0.4, 0.75) if rnd.chance(broken) else 1.0)
    return g.box_mesh(length, 0.108, 0.065)


def scatter(batch, rnd, centre, radius, count, make, z=0.0, lie=True):
    """`count` pieces from `make(rnd)` round `centre` (x, y), each at a random
    turn, thicker towards the middle, resting at height `z` (or the height
    `z(x, y)` gives)."""
    for _ in range(count):
        r = radius * math.sqrt(rnd.unit())
        ang = rnd.uniform(0.0, 360.0)
        x = centre[0] + r * math.cos(math.radians(ang))
        y = centre[1] + r * math.sin(math.radians(ang))
        mesh = make(rnd)
        lowest = min(c[2] for c in mesh[0])
        tilt = (rnd.uniform(-12, 12), rnd.uniform(-12, 12)) if lie else (rnd.uniform(-60, 60), rnd.uniform(-60, 60))
        rows = turned(rnd.uniform(0.0, 360.0), tilt[0], tilt[1])
        ground = z(x, y) if callable(z) else z
        batch.add(mesh, g.Pose(rows, (x, y, ground - lowest * 0.6)))
    return batch


def mound(cx, cy, rx, ry, height, rnd):
    """The height of a heap of rubble over `(x, y)`: highest in the middle,
    lumpy, and sinking 2 cm under the floor at its rim so it never lies flush
    with it. Use it with `MapBuilder.heightfield` over the rectangle
    `cx +- rx`, `cy +- ry`."""
    bumps = [(rnd.uniform(-0.6, 0.6) * rx, rnd.uniform(-0.6, 0.6) * ry, rnd.uniform(0.25, 0.5)) for _ in range(5)]

    def h(x, y):
        dx, dy = (x - cx) / rx, (y - cy) / ry
        d = dx * dx + dy * dy
        if d >= 1.0:
            return -0.02
        base = (1.0 - d) * (1.0 - d)
        extra = 0.0
        for bx, by, s in bumps:
            ex, ey = (x - cx - bx) / (rx * s), (y - cy - by) / (ry * s)
            e = ex * ex + ey * ey
            if e < 1.0:
                extra += (1.0 - e) * (1.0 - e) * 0.35
        return height * base * (0.75 + extra) - 0.02

    return h


def bent_bar(points, w, h):
    """A beam buckled along the points: one box from each point to the next,
    each running half a width past its ends so the joints close. A Batch."""
    batch = Batch()
    for p, q in zip(points, points[1:]):
        d = g.unit(g.sub(q, p))
        a = g.sub(p, g.mul(d, w / 2.0))
        b = g.add(q, g.mul(d, w / 2.0))
        pose = g.aiming(a, b)
        batch.add(g.box_mesh(g.length(g.sub(b, a)), w, h), g.Pose(pose.rows, g.lerp(a, b, 0.5)))
    return batch


def shards(w, h, rnd):
    """What is left of a broken pane `w` by `h` (in its own frame, origin at a
    corner): a few jagged triangles in its corners and along its edges, as
    outlines for `plate`. May be none."""
    out = []
    for cx, cy, sx, sy in ((0, 0, 1, 1), (w, 0, -1, 1), (w, h, -1, -1), (0, h, 1, -1)):
        if rnd.chance(0.55):
            a = rnd.uniform(0.15, 0.6) * w
            b = rnd.uniform(0.15, 0.6) * h
            inner = (cx + sx * a * rnd.uniform(0.1, 0.4), cy + sy * b * rnd.uniform(0.3, 0.6))
            out.append([(cx, cy), (cx + sx * a, cy), inner, (cx, cy + sy * b)])
    return out
