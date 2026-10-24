"""Brickwork with real depth: courses, sunken mortar, missing bricks and spalled render (#340, MIT OR Apache-2.0).

A brick wall drawn as a flat face with a brick picture reads as a picture
from a metre away: the mortar lines don't cast a shadow and a missing brick
is only a dark patch. `brick_face` builds a wall's **outer leaf** brick by
brick, so near a flying line the wall has depth: every brick is a block that
stands a little proud of its sunken mortar, bricks are missing in patches
with brick-shaped edges (the hole goes back to the leaf behind, as the
entrance's broken edge does), a few have lost their face, and islands of
render cling on over them, with their own thickness and ragged edges.

```python
leaf = brick_face(run=6.0, height=4.5, seed=5, patches=[(1.8, 1.2, 0.5)], render=[(4.2, 2.6, 1.1, 0.8)],
                  holes=[(2.6, 4.4, 0.0, 2.4)])
pose = g.Pose(...)  # the wall's own axes, as `odopening.Wall.pose` gives them
m.solid("south_leaf", leaf.brick, pose, "brick_red_weathered", "none")
m.solid("south_render", leaf.render, pose, "concrete_smooth", "none")
```

The panel's own axes are `odopening`'s: `u` along the wall from 0 to
`run`, `y` out of the wall's face (the leaf stands from y = 0, the face of
the wall behind it, to y = `LEAF`), and `z` up from 0 to `height`. Put it in
front of a wall `LEAF` thinner than the whole: the face of that wall is what
shows through a missing brick, the inner leaf, so it should be brick too.

Bricks are laid in the picture's own running bond (`odopening.COURSE`,
`odopening.BRICK`, half a brick of stagger), so start the panel where the
picture's bricks start (a multiple of 0.225 m along the wall, at a
multiple of 0.075 m up) and the geometry and the picture agree.

The cost is about ten triangles a brick, close to 600 a square metre of
brickwork: use it on the stretches of wall the Quad flies past close, and
the picture (which has its own normal map) on the rest. `Leaf.stats` says
the bricks and triangles.
"""
import odgeom as g
from odkit import ragged_outline
from odnoise import Random, noise2
from odopening import BRICK, COURSE

LEAF = 0.1025  # a half-brick leaf: a brick's width
JOINT = 0.13 * COURSE  # the mortar joint, as wide as the picture's
RECESS = 0.012  # how far the mortar sits back from a brick's face


class Leaf:
    def __init__(self, brick, render, bricks, missing, rendered):
        self.brick, self.render = brick, render
        self.bricks, self.missing, self.rendered = bricks, missing, rendered

    def stats(self):
        return {"bricks": self.bricks, "missing": self.missing, "under render": self.rendered,
                "triangles": _tris(self.brick) + _tris(self.render)}


def _tris(mesh):
    return sum(len(f) - 2 for f in mesh[1])


def _inside(points, x, y):
    """Is `(x, y)` inside the polygon (even-odd rule)."""
    inside = False
    n = len(points)
    for i in range(n):
        (ax, ay), (bx, by) = points[i], points[(i + 1) % n]
        if (ay > y) != (by > y):
            t = (y - ay) / (by - ay)
            if x < ax + t * (bx - ax):
                inside = not inside
    return inside


def brick_face(run, height, seed, patches=(), loose=0.03, faced=0.04, render=(), holes=(), stagger_odd=True,
               jitter=0.003, render_thick=0.015):
    """The outer leaf of a brick wall, `run` long and `height` tall (see the
    module's note), as a `Leaf` with two meshes, `brick` and `render`.

    - `patches`: `(u, z, radius)`: bricks missing in a patch round there,
      its edge ragged brick by brick.
    - `loose`: the share of other bricks missing one by one; `faced`: the
      share that have lost their face and sit back 2 cm.
    - `render`: `(u, z, width, height)`: an island of render about that big,
      ragged, `render_thick` thick over the bricks (the bricks under it are
      left out).
    - `holes`: `(u0, u1, z0, z1)`: openings in the wall behind; no brick
      is laid over them, and the bricks round them show their ends.
    - `stagger_odd`: which courses are half a brick along (the picture's
      odd courses).
    - `jitter`: how far a brick may stand proud of or back from its
      neighbours, at most."""
    rnd = Random(seed, "leaf")
    courses = max(1, int(height / COURSE + 1e-6))
    top = courses * COURSE
    y_m = LEAF - RECESS

    # ---------- the islands of render ----------
    islands = []
    r_ren = rnd.child("render")
    for k, (u, z, w, h) in enumerate(render):
        outline = ragged_outline(r_ren.child(k), w / 2.0, h / 2.0, 14)
        islands.append([(u + x, z + y) for x, y in outline])

    # ---------- which slots have a brick ----------
    slots = []  # per course: list of (a, b, state) with state "brick", "faced", "gone"
    gone = rendered = laid = 0
    for c in range(courses):
        z0 = c * COURSE
        zc = z0 + COURSE / 2.0
        shift = 0.5 * BRICK if (c % 2 == 1) == stagger_odd else 0.0
        edges = [0.0]
        u = BRICK - shift if shift else BRICK
        while u < run - 1e-9:
            edges.append(u)
            u += BRICK
        edges.append(run)
        row = []
        for i in range(len(edges) - 1):
            a, b = edges[i], edges[i + 1]
            if b - a < 0.04:  # a sliver at the end: part of the next slot
                continue
            uc = (a + b) / 2.0
            r = Random(seed, "slot", c, i)
            state = "brick"
            if any(h0 < b - 1e-6 and h1 > a + 1e-6 and v0 < z0 + COURSE - 1e-6 and v1 > z0 + 1e-6 for h0, h1, v0, v1 in holes):
                state = "hole"
            else:
                for pu, pz, pr in patches:
                    du, dz = (uc - pu) / pr, (zc - pz) / (pr * 0.8)
                    ragged = 0.35 * noise2(seed + 17, uc / 0.3, zc / 0.2) + 0.25 * (r.unit() - 0.5)
                    if du * du + dz * dz < (1.0 + ragged) * (1.0 + ragged):
                        state = "gone"
                if state == "brick" and r.chance(loose):
                    state = "gone"
                if state == "brick" and any(all(_inside(isl, x, zc) for x in (a, b)) for isl in islands):
                    state = "rendered"
                if state == "brick" and r.chance(faced):
                    state = "faced"
            if state == "gone":
                gone += 1
            elif state == "rendered":
                rendered += 1
            elif state != "hole":
                laid += 1
            row.append((a, b, state, r.uniform(-jitter, jitter)))
        slots.append(row)

    corners, faces = [], []

    def quad(p, q, r_, s, normal):
        """A quad facing `normal`."""
        n = len(corners)
        corners.extend([p, q, r_, s])
        face = (n, n + 1, n + 2, n + 3)
        if g.dot(g.cross(g.sub(q, p), g.sub(s, p)), normal) < 0.0:
            face = tuple(reversed(face))
        faces.append(face)

    def P(u, y, z):
        return (u, y, z)

    def solid(state):
        return state in ("brick", "faced", "rendered")

    for c, row in enumerate(slots):
        z0, z1 = c * COURSE, c * COURSE + COURSE
        # The mortar: one strip over each run of slots that are not gone.
        run_start = None
        for i, (a, b, state, _) in enumerate(row + [(run, run, "end", 0.0)]):
            if solid(state):
                if run_start is None:
                    run_start = a
            elif run_start is not None:
                quad(P(run_start, y_m, z0), P(a, y_m, z0), P(a, y_m, z1), P(run_start, y_m, z1), (0, 1, 0))
                run_start = None
        for i, (a, b, state, proud) in enumerate(row):
            if state in ("brick", "faced"):
                yf = LEAF + proud - (0.02 if state == "faced" else 0.0)
                ba, bb, bz0, bz1 = a + JOINT / 2.0, b - JOINT / 2.0, z0 + JOINT / 2.0, z1 - JOINT / 2.0
                if a <= 0.0:
                    ba = a
                if b >= run:
                    bb = b
                quad(P(ba, yf, bz0), P(bb, yf, bz0), P(bb, yf, bz1), P(ba, yf, bz1), (0, 1, 0))
                if yf > y_m + 1e-4:
                    quad(P(ba, y_m, bz0), P(bb, y_m, bz0), P(bb, yf, bz0), P(ba, yf, bz0), (0, 0, -1))
                    quad(P(ba, y_m, bz1), P(bb, y_m, bz1), P(bb, yf, bz1), P(ba, yf, bz1), (0, 0, 1))
                    quad(P(ba, y_m, bz0), P(ba, yf, bz0), P(ba, yf, bz1), P(ba, y_m, bz1), (-1, 0, 0))
                    quad(P(bb, y_m, bz0), P(bb, yf, bz0), P(bb, yf, bz1), P(bb, y_m, bz1), (1, 0, 0))
                else:  # sunk below the mortar: a pocket lined back to it
                    quad(P(ba, yf, bz0), P(bb, yf, bz0), P(bb, y_m, bz0), P(ba, y_m, bz0), (0, 0, 1))
                    quad(P(ba, yf, bz1), P(bb, yf, bz1), P(bb, y_m, bz1), P(ba, y_m, bz1), (0, 0, -1))
                    quad(P(ba, yf, bz0), P(ba, y_m, bz0), P(ba, y_m, bz1), P(ba, yf, bz1), (1, 0, 0))
                    quad(P(bb, yf, bz0), P(bb, y_m, bz0), P(bb, y_m, bz1), P(bb, yf, bz1), (-1, 0, 0))
            if solid(state):
                continue
            # A slot that is gone (or over a hole): line it back to the wall
            # behind, wherever a solid slot is next to it, and at the leaf's
            # own ends.
            left = row[i - 1][2] if i > 0 else "end"
            right = row[i + 1][2] if i + 1 < len(row) else "end"
            if solid(left) or left == "end":
                quad(P(a, 0.0, z0), P(a, y_m, z0), P(a, y_m, z1), P(a, 0.0, z1), (1, 0, 0))
            if solid(right) or right == "end":
                quad(P(b, 0.0, z0), P(b, y_m, z0), P(b, y_m, z1), P(b, 0.0, z1), (-1, 0, 0))
            for other, zz, normal in ((c - 1, z0, (0, 0, 1)), (c + 1, z1, (0, 0, -1))):
                if not 0 <= other < len(slots):
                    if state != "hole":  # an opening's foot or head is the wall's own
                        quad(P(a, 0.0, zz), P(b, 0.0, zz), P(b, y_m, zz), P(a, y_m, zz), normal)
                    continue
                for oa, ob, ostate, _ in slots[other]:
                    lo, hi = max(a, oa), min(b, ob)
                    if hi - lo > 1e-6 and solid(ostate):
                        quad(P(lo, 0.0, zz), P(hi, 0.0, zz), P(hi, y_m, zz), P(lo, y_m, zz), normal)
        # The leaf's own ends and top and foot, where a slot is solid.
        for a, b, state, _ in row:
            if not solid(state):
                continue
            if a <= 0.0:
                quad(P(a, 0.0, z0), P(a, y_m, z0), P(a, y_m, z1), P(a, 0.0, z1), (-1, 0, 0))
            if b >= run:
                quad(P(b, 0.0, z0), P(b, y_m, z0), P(b, y_m, z1), P(b, 0.0, z1), (1, 0, 0))
            if c == 0:
                quad(P(a, 0.0, z0), P(b, 0.0, z0), P(b, y_m, z0), P(a, y_m, z0), (0, 0, -1))
            if c == courses - 1:
                quad(P(a, 0.0, top), P(b, 0.0, top), P(b, y_m, top), P(a, y_m, top), (0, 0, 1))

    brick = (corners, faces)

    # ---------- the render: plates over the bricks ----------
    rc, rf = [], []
    for isl in islands:
        pts = isl
        assert all(0.0 <= u <= run and 0.0 <= z <= top for u, z in pts), "an island of render must lie on the leaf"
        k = len(pts)
        base = len(rc)
        y0, y1 = LEAF - 0.004, LEAF + render_thick
        rc.extend([(u, y0, z) for u, z in pts] + [(u, y1, z) for u, z in pts])
        local = [(base + i, base + (i + 1) % k, base + k + (i + 1) % k, base + k + i) for i in range(k)]
        for a, b, cc in g.triangulate_polygon(pts):
            local.append((base + a, base + b, base + cc))
            local.append((base + k + a, base + k + cc, base + k + b))
        part = (rc[base:], [tuple(i - base for i in f) for f in local])
        fixed = g.point_outwards(*part)
        rf.extend(tuple(i + base for i in f) for f in fixed)
    return Leaf(brick, (rc, rf), laid, gone, rendered)


def cill(width, depth=0.12, thick=0.06, drop=0.02):
    """A window cill in the wall's own axes: a slab `width` long standing
    `depth` out from the wall's face (y = 0) and `thick` tall, its top
    falling `drop` to the front so rain runs off, standing on z = 0 and
    centred on u = 0."""
    w = width / 2.0
    corners = [(-w, -0.05, 0.0), (w, -0.05, 0.0), (w, depth, 0.0), (-w, depth, 0.0),
               (-w, -0.05, thick), (w, -0.05, thick), (w, depth, thick - drop), (-w, depth, thick - drop)]
    faces = [(3, 2, 1, 0), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    return corners, g.point_outwards(corners, faces)
