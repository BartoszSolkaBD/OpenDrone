"""Brick walls with broken openings, bulges and lean (MIT OR Apache-2.0).

A wall of boxes round a rectangle (`MapBuilder.wall`) reads as a model: every
opening a perfect rectangle, every face a perfect plane. A brick wall that has
been knocked about is neither. `broken_wall` makes one wall as a single closed
mesh in plain Python (the same bytes on every computer: `odnoise`, `odcut`,
`odgeom`) where

- each **opening** is stepped course by course the way broken brickwork is: a
  window that has lost its sill and a corner of its lintel, a doorway with
  bricks missing from its jambs, a hole knocked through the wall;
- the face **bulges** a little between its ends and **leans**, by a distance
  in metres you choose;
- every few **courses** sit a few millimetres proud of the one below, so the
  light catches a ledge there (this costs triangles, so it is a choice);
- `loose_bricks` scatters the bricks that fell out, at the foot of each
  opening and on the sill.

```python
wall = broken_wall(run=12.0, height=4.5, thick=0.34, seed=2, bulge=0.03, lean=0.05, ledge=0.004,
                   openings=[Opening(3.0, 4.6, 1.1, 2.9, "window", sill_gone=True),
                             Opening(8.0, 10.0, 0.0, 2.4, "doorway"),
                             Opening(6.0, 7.5, 0.6, 2.2, "breach")])
m.solid("south_wall", wall.mesh, wall.pose(p0=(-6, 0), p1=(6, 0), z0=0.0), "brick_red_weathered", "none")
m.wall("south_wall_solid", (-6, 0), (6, 0), 0.0, 4.5, 0.34, "brick_red_weathered",
       openings=wall.collision_openings(), collision_only=True)       # the Quad meets boxes, not bricks
for k, brick in enumerate(wall.loose_bricks(Random(2, "bricks"))):
    m.solid("loose_bricks", brick, wall.pose((-6, 0), (6, 0), 0.0), "brick_red_weathered", "none")
```

The wall's own axes: `u` runs along it from 0 to `run` (x), `y` goes through
it (the front, the face with the wear, is +y, which is **to the left of the way
you walk from `p0` to `p1`**) and up is z from 0 to `height`. Its courses are
`COURSE` = 75 mm and its bricks `BRICK` = 225 mm long, the size of
`brick_red_weathered`'s; start the wall at a multiple of 0.225 m east or north
and the stepped edges fall on the picture's bricks.

The drawn wall (`mesh`) is a closed solid of the front skin, the back skin
and the walls of each opening. The collision-only part comes from
`collision_openings`: boxes round openings that sit inside the drawn ones, so
the Quad is never stopped by a hole's air, only sometimes lets a brick's edge
by.

The triangle cost, per square metre of wall face, is measured by
`triangles_per_square_metre`. A wall with no openings and `ledge` at 0 is a
grid of about 0.9 m by 0.6 m on each skin: 10 a square metre. Every course an
opening steps through is a row of its own, so the 14 x 4.5 m wall above, with
three, costs 38. `ledge` makes a ledge every `row_courses` courses away from
the openings: 31 a square metre on a plain wall, 46 on the one above.
"""
import math

import odgeom as g
from odcut import edge_loops, merge, shell
from odnoise import Random, noise2, smoothstep

COURSE = 0.075  # one course of brick_red_weathered, with its mortar
BRICK = 0.225  # one brick's length
HALF = BRICK / 2.0  # a half brick: the steps of a stepped edge


class Opening:
    """A hole to break in the wall: `u0..u1` metres along it and `v0..v1`
    above its foot, as for `MapBuilder.wall`. `kind` says how it breaks:

    - `"window"`: straight at first. The jambs are ragged and the lintel
      loses a brick or two. With `sill_gone` the bottom courses have broken
      away too, narrowing like a funnel below `v0`.
    - `"doorway"`: like a window, from `v0` (usually 0), with no sill.
    - `"breach"`: a hole knocked through: steps in and out all round and a
      stepped top.
    - `"framed"`: an opening that still holds its frame (an iron window, a
      louvre): its edges stay straight, cut to the frame, so no brick hangs
      over the frame and no gap opens beside it (#324). It keeps its
      courses as given, and drops no bricks.

    `ragged` (0 to 1) is how rough the edge is; it has a default for each
    kind. `drop` is how many courses a gone sill reaches down (default 3).
    """

    def __init__(self, u0, u1, v0, v1, kind="window", ragged=None, sill_gone=False, drop=3):
        assert kind in ("window", "doorway", "breach", "framed"), kind
        self.u0, self.u1, self.v0, self.v1, self.kind = u0, u1, v0, v1, kind
        self.ragged = ragged if ragged is not None else {"window": 0.45, "doorway": 0.4, "breach": 0.9, "framed": 0.0}[kind]
        self.sill_gone = sill_gone and kind == "window"
        self.drop = drop
        self.first_course = 0  # filled in by `broken_wall`: the stepped shape
        self.edges = []  # one (left, right) in metres for each course from first_course


def _stepped(op, rnd, run):
    """The `(left, right)` of the hole in each course from the lowest, stepped
    in half bricks. Left and right each wander a few steps, a bonded wall
    alternates by a half brick from one course to the next, and a gone sill,
    a breach's top and a lintel's corners narrow."""
    j0, j1 = round(op.v0 / COURSE), max(round(op.v1 / COURSE), round(op.v0 / COURSE) + 2)
    if op.kind == "framed":
        return j0, [(op.u0, op.u1)] * (j1 - j0)
    drop = op.drop if op.sill_gone else 0
    reach = 1 + int(op.ragged * 3)
    p = 0.2 + 0.5 * op.ragged
    first = max(0, j0 - drop)
    total = j1 - first
    left = right = 0
    out = []
    taper = {"window": int(round(op.ragged * 1.5)), "doorway": int(round(op.ragged * 1.5)), "breach": 2 + int(op.ragged * 2)}[op.kind]
    for k in range(total):
        j = first + k
        if rnd.chance(p):
            left = max(-reach, min(reach, left + rnd.pick((-1, 1))))
        if rnd.chance(p):
            right = max(-reach, min(reach, right + rnd.pick((-1, 1))))
        inward = 0
        if j < j0:  # the broken-away sill: the lower, the narrower
            inward += (j0 - j) + (1 if rnd.chance(0.5) else 0)
        from_top = j1 - 1 - j
        if from_top < taper:  # a ragged lintel: the higher, the narrower
            inward += (taper - from_top) + (1 if rnd.chance(0.5) else 0)
        bond = (j % 2) if op.kind != "breach" else (1 if rnd.chance(0.5) else 0)
        li = left + inward + bond
        ri = right - inward - (bond if rnd.chance(0.5) else 0)
        if op.kind == "breach" and 0.3 < k / max(1, total - 1) < 0.7:  # bulges out at the middle courses
            li, ri = li - 1, ri + 1
        # Whole half bricks from where the opening began, never a gap under
        # 0.45 m, and never nearer than 0.2 m to the wall's ends.
        lo = math.ceil((0.2 - op.u0) / HALF - 1e-9)
        hi = math.floor((run - 0.2 - op.u1) / HALF + 1e-9)
        li, ri = max(li, lo), min(ri, hi)
        while (op.u1 - op.u0) + HALF * (ri - li) < 0.45 and (li > lo or ri < hi):
            li, ri = max(li - 1, lo), min(ri + 1, hi)
        out.append((round(op.u0 + HALF * li, 6), round(op.u1 + HALF * ri, 6)))
    return first, out


class Wall:
    """What `broken_wall` makes. `mesh` is the drawn wall; the rest answers
    questions about it."""

    def __init__(self, run, height, thick, openings, mesh, skin, seed):
        self.run, self.height, self.thick = run, height, thick
        self.openings, self.mesh, self.skin, self.seed = openings, mesh, skin, seed

    def pose(self, p0, p1, z0=0.0):
        """Puts the wall in the Map: its foot on `z0`, running from `p0` to
        `p1` (x, y), its front (+y) on the left of that way."""
        return g.aiming((p0[0], p0[1], z0), (p1[0], p1[1], z0))

    def hole_area(self, k):
        """The area of opening `k`'s hole in the wall face, in square metres."""
        op = self.openings[k]
        return sum((r - l) * COURSE for l, r in op.edges)

    def collision_openings(self, band=4):
        """Openings for `MapBuilder.wall`, in its `(u0, u1, v0, v1)` form,
        that each lie inside the drawn hole: the hole's courses are taken
        `band` at a time (0.3 m for 4) and each band's opening is what all its
        courses share. The Quad is never stopped by air, and a ragged edge may
        let a brick's end by."""
        out = []
        for op in self.openings:
            for b in range(0, len(op.edges), band):
                chunk = op.edges[b:b + band]
                l, r = max(e[0] for e in chunk), min(e[1] for e in chunk)
                v0 = (op.first_course + b) * COURSE
                v1 = min((op.first_course + b + len(chunk)) * COURSE, self.height)
                if r - l > 0.05 and v1 > v0:
                    out.append((l, r, v0, v1))
        return out

    def loose_bricks(self, rnd, per_square_metre=7.0, floor=lambda u, y: 0.0):
        """The bricks that fell out, a mesh each opening, in the wall's own
        axes: spread along the hole's width and out to both sides of the wall
        at the foot (`floor(u, y)` is the height of the ground there), with
        some on the sill if there is one. `per_square_metre` is how many for
        each square metre of hole."""
        out = []
        for k, op in enumerate(self.openings):
            r = rnd.child("opening", k)
            if op.kind == "framed":
                out.append(([], []))
                continue
            count = max(3, round(self.hole_area(k) * per_square_metre))
            lo = min(l for l, _ in op.edges)
            hi = max(rr for _, rr in op.edges)
            on_sill = 0 if op.sill_gone or op.kind == "doorway" else count // 4
            bricks = []
            for n in range(count):
                u = r.uniform(lo - 0.3, hi + 0.3)
                if n < on_sill:
                    y = r.uniform(-self.thick / 2 + 0.05, self.thick / 2 - 0.05)
                    z = op.first_course * COURSE + 0.04 * 0.5 + 0.0375
                else:
                    side = 1.0 if r.chance(0.55) else -1.0
                    y = side * (self.thick / 2 + abs(r.uniform(-0.1, 0.9) * r.unit()))
                    z = floor(u, y) + 0.03
                bricks.append(_brick(u, y, z, r))
            out.append(merge(bricks))
        return out


def _brick(u, y, z, r):
    """One loose brick lying at `(u, y, z)`, turned and a little tilted."""
    corners, faces = g.box_mesh(BRICK, 0.1125, COURSE * 0.97)
    rows = g.mat_mul(g.rot_z(r.uniform(0, 360)), g.mat_mul(g.rot_y(r.uniform(-10, 10)), g.rot_x(r.uniform(-10, 10))))
    pose = g.Pose(rows, (u, y, z))
    # Snapped before it is cut into triangles: a turned brick's faces are flat
    # only to the last bit of a sine, so which diagonal "leaves no dent" on
    # the unsnapped corners can differ between computers (#324 found the
    # Factory's `.glb` differing on Linux this way). On the corners the file
    # stores, every computer decides alike.
    placed = [g.snap3(pose.apply(c)) for c in corners]
    return placed, g.triangulate(placed, faces, convex=True)


def broken_wall(run, height, thick, seed, openings=(), bulge=0.03, lean=0.0, ledge=0.0, row_courses=4,
                cell=0.9, coarse_rows=8):
    """A brick wall `run` long and `height` high, `thick` through, as one
    closed mesh (see the module's note).

    - `openings`: `Opening`s, each at least 0.45 m from the wall's ends.
    - `bulge`: the face moves in and out by up to this many metres, smoothly
      (0 for a true plane); `lean`: the top of the wall is this far further
      toward the front than its foot (negative leans back).
    - `ledge`: each `row_courses` courses sit up to this many metres proud
      or back of the one below (0 for none; 0.004 is 4 mm). With 0, the
      wall's face is cut in rows `coarse_rows` courses tall away from the
      openings.
    - `cell`: about how wide the base grid is, in metres.
    """
    assert run > 1.0 and height > COURSE * 2, "a wall is longer than a metre and taller than two courses"
    rnd = Random(seed, "wall")
    courses = max(1, math.ceil(height / COURSE - 0.02))
    for k, op in enumerate(openings):
        assert op.u0 >= 0.45 and op.u1 <= run - 0.45 and op.u1 > op.u0 + 0.5, f"opening {k} must sit 0.45 m or more inside the wall's ends and be over half a metre wide"
        assert 0.0 <= op.v0 < op.v1 <= height + 1e-9, f"opening {k} must be between the wall's foot and top"
        op.first_course, op.edges = _stepped(op, rnd.child("opening", k), run)
    openings = list(openings)

    # The grid: courses in rows, columns round the steps.
    fine = set()
    for op in openings:
        fine.update(range(op.first_course, op.first_course + len(op.edges)))
    per_row = max(1, row_courses) if ledge > 0.0 else max(1, coarse_rows)
    rows = []
    j = 0
    while j < courses:
        if j in fine:
            rows.append((j, j + 1))
            j += 1
            continue
        end = j + 1
        while end < courses and end - j < per_row and end not in fine:
            end += 1
        rows.append((j, end))
        j = end
    n = max(1, round(run / cell))
    base = [run * i / n for i in range(1, n)]
    n_fine = max(1, round(run / (2.0 * cell)))
    base_fine = [run * i / n_fine for i in range(1, n_fine)]

    def holes_in(j):
        """The holes in course `j`, left to right."""
        found = []
        for op in openings:
            k = j - op.first_course
            if 0 <= k < len(op.edges):
                found.append(op.edges[k])
        return sorted(found)

    def lines_of(row):
        j0, j1 = rows[row]
        holes = holes_in(j0) if j0 in fine else []
        if not holes:
            return set(base)
        edges = {x for l, r in holes for x in (l, r)}
        return {u for u in base_fine if all(abs(u - e) > 0.04 for e in edges)} | edges

    def runs_of(row):
        """The solid stretches of a row, left to right, as `(a, b)`."""
        j0, j1 = rows[row]
        out, at = [], 0.0
        for l, r in (holes_in(j0) if j0 in fine else []):
            if l > at:
                out.append((at, l))
            at = max(at, r)
        if at < run:
            out.append((at, run))
        return out

    own = [lines_of(row) for row in range(len(rows))]

    def chain(row, other):
        """The corners along the boundary between a row and the one on that
        side of it (`other`, or none at the wall's foot and top): the lines of
        both, so neither has a crack."""
        return sorted({0.0, run} | own[row] | (own[other] if other is not None else set()))

    bulge_seed = seed * 3 + 11
    half = thick / 2.0

    def face_y(u, v):
        env = smoothstep(u / 1.2) * smoothstep((run - u) / 1.2) * smoothstep(v / 0.6)
        b = bulge * env * (0.6 * noise2(bulge_seed, u / 3.2, v / 2.4) + 0.4 * noise2(bulge_seed + 3, u / 1.2, v / 1.0))
        return half + b + lean * (v / height)

    ledges = []
    r_led = rnd.child("ledges")
    stepped = [rows[k][0] in fine for k in range(len(rows))]
    for k in range(len(rows)):
        value = round(r_led.uniform(-ledge, ledge) / 0.0005) * 0.0005 if ledge > 0.0 else 0.0
        # No ledge in or next to the rows an opening steps through: where a
        # ledge meets the end of a run, a sliver with no area is left.
        near = stepped[k] or (k > 0 and stepped[k - 1]) or (k + 1 < len(rows) and stepped[k + 1])
        ledges.append(0.0 if near else value)

    corners, index = [], {}

    def vid(u, v, row):
        y = face_y(u, v) + ledges[row] * smoothstep(u / 0.6) * smoothstep((run - u) / 0.6)
        key = (round(u * 1e5), round(v * 1e5), round(y * 1e5))
        if key not in index:
            index[key] = len(corners)
            corners.append((u, y, v))
        return index[key]

    def v_at(j):
        return height if j >= courses else j * COURSE

    faces = []

    def strip(bottom, top, va, vb, row, a, b):
        """Triangles between the corners along the bottom and along the top of
        a stretch from `a` to `b`, facing the front."""
        ub = [u for u in bottom if a - 1e-9 <= u <= b + 1e-9]
        ut = [u for u in top if a - 1e-9 <= u <= b + 1e-9]
        lo = [vid(u, va, row) for u in ub]
        hi = [vid(u, vb, row) for u in ut]
        i = j = 0
        while i < len(lo) - 1 or j < len(hi) - 1:
            if i < len(lo) - 1 and (j == len(hi) - 1 or ub[i + 1] <= ut[j + 1] + 1e-12):
                faces.append((lo[i], hi[j], lo[i + 1]))
                i += 1
            else:
                faces.append((lo[i], hi[j], hi[j + 1]))
                j += 1

    for ri, (j0, j1) in enumerate(rows):
        va, vb = v_at(j0), v_at(j1)
        bottom = chain(ri, ri - 1 if ri > 0 else None)
        top = chain(ri, ri + 1 if ri + 1 < len(rows) else None)
        for a, b in runs_of(ri):
            strip(bottom, top, va, vb, ri, a, b)
        if ri + 1 < len(rows) and ledges[ri] != ledges[ri + 1]:
            # The ledge between this row and the next, where both are there.
            nxt = ri + 1
            for a, b in runs_of(ri):
                for c, d in runs_of(nxt):
                    lo, hi = max(a, c), min(b, d)
                    if hi - lo < 1e-9:
                        continue
                    us = [u for u in top if lo - 1e-9 <= u <= hi + 1e-9]
                    for ua, ub in zip(us, us[1:]):
                        a0, a1 = vid(ua, vb, ri), vid(ub, vb, ri)
                        b0, b1 = vid(ua, vb, nxt), vid(ub, vb, nxt)
                        # The same corner order faces up where the next row
                        # is set back and down where it juts out.
                        # A ledge dies away to nothing at the wall's ends.
                        for tri in ((a0, b1, a1), (a0, b0, b1)):
                            if len(set(tri)) == 3:
                                faces.append(tri)
    skin = (corners, faces)
    mesh = shell(skin, (0.0, -thick, 0.0))
    return Wall(run, height, thick, openings, mesh, skin, seed)


def triangles_per_square_metre(wall):
    """How many triangles the wall's drawn mesh costs for each square metre
    of its face (its run times its height, the openings not taken off)."""
    return sum(len(f) - 2 for f in wall.mesh[1]) / (wall.run * wall.height)


def open_edges(wall):
    """The edges of the wall's face that only one triangle uses: the wall's
    rim and each opening's edge, for checking an opening isn't a rectangle."""
    return edge_loops(wall.skin)
