"""Cutting ragged holes in meshes and giving sheets a thickness (MIT OR Apache-2.0).

The shared tools of the geometry helpers (`odground`, `odroof`, `odopening`).
Plain Python, no Blender: Blender's booleans and bevels don't keep their
output order from run to run, so a ragged hole is cut here, by a fixed rule,
and the same bytes come out on every computer (docs/building-a-map.md).

- `clip_grid` makes a sheet over a rectangle (a height at every corner, like
  `odgeom.heightfield_mesh`) and cuts out everything where a function says
  "hole". Triangles that the edge of the hole crosses are cut along it, and a
  corner of the new edge is put where the function crosses zero along the
  triangle's side, so the edge follows the function as closely as the grid
  allows. Give the function some noise and the edge is ragged.
- `shell` gives an open sheet a thickness, joining the two skins along every
  edge a triangle has on one side only (the sheet's rim and the walls of its
  holes).
- `merge` joins meshes into one part to draw, and `edge_loops` finds the rim.

A mesh is `(corners, faces)` as in `odgeom`; everything here makes triangles.
"""
import odgeom as g


def merge(meshes):
    """Many meshes as one: corners joined and faces renumbered."""
    corners, faces = [], []
    for c, f in meshes:
        base = len(corners)
        corners += list(c)
        faces += [tuple(i + base for i in face) for face in f]
    return corners, faces


def triangle_count(mesh):
    """How many triangles the builder will cut the mesh into."""
    return sum(len(face) - 2 for face in mesh[1])


def area(mesh):
    """The surface area of the mesh, in square metres."""
    corners, faces = mesh
    total = 0.0
    for face in faces:
        for a, b, c in g._fan(face):
            total += g.length(g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a]))) / 2.0
    return total


def _sdf_away_from_zero(d, eps):
    """A distance that is never within `eps` of the cut, so a corner never
    lands on top of another."""
    if d >= 0.0:
        return d if d > eps else eps
    return d if d < -eps else -eps


def clip_grid(x0, y0, x1, y1, step, height, hole=None, min_area=2e-5):
    """A sheet over a rectangle, facing up, `step` or so between corners, the
    height at `(x, y)` from `height(x, y)`, with a hole where `hole(x, y)` is
    negative or zero (leave `hole` out for none). Returns `(corners, faces)`
    with only triangles.

    A triangle the hole's edge crosses is clipped: a corner is put on each
    side the edge crosses, exactly where `hole` crosses zero along that side
    (found by halving it), and gets its height from `height`. A hole smaller
    than the grid can slip between corners and be missed, so a hole should be
    two or three steps across. Two triangles that share a side put their new corner in the very same
    place, so the sheet has no crack. A sliver smaller than `min_area` square
    metres is dropped: after the builder rounds corners to 0.1 mm it would
    have no area at all.
    """
    nx, ny = max(1, round((x1 - x0) / step)), max(1, round((y1 - y0) / step))
    eps = 0.02 * min((x1 - x0) / nx, (y1 - y0) / ny)
    corners = []
    index = {}

    def corner(x, y):
        key = (x, y)
        if key not in index:
            index[key] = len(corners)
            corners.append((x, y, height(x, y)))
        return index[key]

    values = {}

    def value(i, j):
        if (i, j) not in values:
            x = x0 + (x1 - x0) * i / nx
            y = y0 + (y1 - y0) * j / ny
            values[(i, j)] = (x, y, 1.0 if hole is None else _sdf_away_from_zero(hole(x, y), eps))
        return values[(i, j)]

    def cut(p, q):
        """The point on the side p-q where the hole function crosses zero,
        found by halving the side 40 times, from the same end every time."""
        if (p[0], p[1]) > (q[0], q[1]):
            p, q = q, p
        inside_p = p[2] > 0.0
        lo, hi = 0.0, 1.0
        for _ in range(40):
            mid = (lo + hi) / 2.0
            x, y = p[0] + (q[0] - p[0]) * mid, p[1] + (q[1] - p[1]) * mid
            if (hole(x, y) > 0.0) == inside_p:
                lo = mid
            else:
                hi = mid
        t = (lo + hi) / 2.0
        return (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, 0.0)

    faces = []

    def emit(poly):
        ids = [corner(p[0], p[1]) for p in poly]
        for k in range(1, len(ids) - 1):
            a, b, c = ids[0], ids[k], ids[k + 1]
            u, v = g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a])
            if u[0] * v[1] - u[1] * v[0] > 2.0 * min_area:
                faces.append((a, b, c))

    for j in range(ny):
        for i in range(nx):
            p00, p10, p11, p01 = value(i, j), value(i + 1, j), value(i + 1, j + 1), value(i, j + 1)
            z = [height(p[0], p[1]) for p in (p00, p10, p11, p01)]
            if abs(z[0] - z[2]) <= abs(z[1] - z[3]):
                tris = [(p00, p10, p11), (p00, p11, p01)]
            else:
                tris = [(p00, p10, p01), (p10, p11, p01)]
            for tri in tris:
                inside = [p for p in tri if p[2] > 0.0]
                if len(inside) == 3:
                    emit(tri)
                elif inside:
                    poly = []
                    for k in range(3):
                        p, q = tri[k], tri[(k + 1) % 3]
                        if p[2] > 0.0:
                            poly.append(p)
                        if (p[2] > 0.0) != (q[2] > 0.0):
                            poly.append(cut(p, q))
                    emit(poly)
    return corners, faces


def edge_loops(mesh):
    """The edges that only one triangle uses, as `(a, b)` in the direction
    the triangle runs them: the rim of a sheet and the edge of each hole."""
    corners, faces = mesh
    seen = {}
    order = []
    for face in faces:
        for k in range(len(face)):
            a, b = face[k], face[(k + 1) % len(face)]
            seen[(a, b)] = True
            order.append((a, b))
    return [(a, b) for a, b in order if (b, a) not in seen]


def shell(mesh, offset):
    """Gives an open sheet a thickness. `offset` is the vector from each of
    the sheet's corners to its back, for example `(0, 0, -0.01)` for a sheet
    that is 1 cm thick going down. The back skin faces the other way, and
    every edge a triangle has on one side only is joined front to back, so the
    result is closed, with its faces pointing out of it (a roof sheet's
    underside faces down)."""
    corners, faces = mesh
    n = len(corners)
    back = [g.add(c, offset) for c in corners]
    out = list(faces)
    out += [tuple(reversed(face)) for face in faces]
    out = [f if i < len(faces) else tuple(n + k for k in f) for i, f in enumerate(out)]
    for a, b in edge_loops(mesh):
        out.append((b, a, n + a))
        out.append((b, n + a, n + b))
    return corners + back, g.point_outwards(corners + back, out)
