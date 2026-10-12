"""Plain-Python geometry for OpenDrone's Map scripts (MIT OR Apache-2.0).

Nothing here touches Blender. Maps are built from numbers, and the numbers must
come out the same on every computer, so that CI can rebuild a Map with Blender
on Linux and demand the very same `.glb` a Mac built (#63). That is why:

- every shape is worked out here in 64-bit numbers, with Python's own
  arithmetic, and handed to Blender already placed in the Map (Blender's own
  maths is 32-bit, and its last digit can differ between processors);
- every corner is rounded to 0.1 mm, then to the 32-bit number the `.glb`
  stores, so Blender, the exporter and the Pack checker all see the very same
  value (`snap`);
- every solid is cut into triangles here, by a fixed rule, never by Blender's
  own triangulation, whose choice between equal diagonals can differ.

Units are metres. East is +x, north is +y and up is +z, as in Blender's Z-up
world: the glTF export turns that into glTF's Y-up. Compass headings run
clockwise from north: 0 is north, 90 is east.

A mesh is `(corners, faces)`: a list of `(x, y, z)` and a list of faces, each
three or four corner numbers, anticlockwise seen from the side the face points
to. A closed solid's faces point out of it.
"""
import math
import struct

GRID = 10000.0  # corners are rounded to 1 / GRID = 0.1 mm


def snap(x):
    """Rounds to 0.1 mm, then to the 32-bit number the `.glb` stores, so every
    program reads back exactly this value. Never gives -0.0."""
    f32 = struct.unpack("<f", struct.pack("<f", round(x * GRID) / GRID))[0]
    return f32 + 0.0


def snap3(v):
    return (snap(v[0]), snap(v[1]), snap(v[2]))


# ---------- vectors ----------
def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def mul(a, s):
    return (a[0] * s, a[1] * s, a[2] * s)


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def length(a):
    return math.sqrt(dot(a, a))


def unit(a):
    n = length(a)
    return (a[0] / n, a[1] / n, a[2] / n)


def lerp(a, b, t):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


# ---------- turns ----------
class Pose:
    """A turn and a move: a corner `v` in the shape's own axes lands at
    `R v + t` in the Map. `R` is three rows."""

    def __init__(self, rows=((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)), at=(0.0, 0.0, 0.0)):
        self.rows = rows
        self.at = at

    def apply(self, v):
        r = self.rows
        return (
            dot(r[0], v) + self.at[0],
            dot(r[1], v) + self.at[1],
            dot(r[2], v) + self.at[2],
        )


def rot_x(deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return ((1.0, 0.0, 0.0), (0.0, c, -s), (0.0, s, c))


def rot_y(deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return ((c, 0.0, s), (0.0, 1.0, 0.0), (-s, 0.0, c))


def rot_z(deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return ((c, -s, 0.0), (s, c, 0.0), (0.0, 0.0, 1.0))


def mat_mul(a, b):
    cols = [(b[0][j], b[1][j], b[2][j]) for j in range(3)]
    return tuple(tuple(dot(row, col) for col in cols) for row in a)


def heading_vec(heading):
    """The horizontal direction a compass heading points to."""
    return (math.sin(math.radians(heading)), math.cos(math.radians(heading)), 0.0)


def facing(at, heading):
    """A pose that turns the shape clockwise (seen from above) to a compass
    heading: its own +y axis points along `heading`."""
    return Pose(rot_z(-heading), at)


def aiming(start, end, side_hint=(0.0, 0.0, 1.0)):
    """A pose at `start` with the shape's own +x axis along `start` to `end`,
    and its +z axis as near to `side_hint` as it can be: a beam or rail laid
    from one point to another."""
    x = unit(sub(end, start))
    y = cross(side_hint, x)
    if length(y) < 1e-6:
        y = cross((1.0, 0.0, 0.0), x)
    y = unit(y)
    z = cross(x, y)
    rows = ((x[0], y[0], z[0]), (x[1], y[1], z[1]), (x[2], y[2], z[2]))
    return Pose(rows, start)


def along(start, end):
    """A pose at `start` with the shape's own +z axis along `start` to `end`:
    a cylinder or post laid from one point to another."""
    z = unit(sub(end, start))
    hint = (1.0, 0.0, 0.0) if abs(z[0]) < 0.9 else (0.0, 1.0, 0.0)
    y = unit(cross(z, hint))
    x = cross(y, z)
    rows = ((x[0], y[0], z[0]), (x[1], y[1], z[1]), (x[2], y[2], z[2]))
    return Pose(rows, start)


# ---------- solids ----------
# A face is three or four corner numbers, anticlockwise seen from the side it
# points to. `triangulate` cuts them up once the corners are placed and
# snapped.
def _fan(face):
    return [(face[0], face[i], face[i + 1]) for i in range(1, len(face) - 1)]


def signed_volume(corners, faces):
    total = 0.0
    for face in faces:
        for a, b, c in _fan(face):
            total += dot(corners[a], cross(corners[b], corners[c]))
    return total / 6.0


def point_outwards(corners, faces):
    """Turns every face round if the solid's faces point inwards."""
    if signed_volume(corners, faces) < 0.0:
        return [tuple(reversed(face)) for face in faces]
    return list(faces)


def triangulate(corners, faces, convex=False):
    """Cuts every face into triangles by a fixed rule, never Blender's.
    A four-cornered face is cut along its first and third corners. For a
    convex solid it is cut along whichever diagonal leaves no dent: a face of
    a tapered cylinder is flat only before its corners are rounded, and after,
    one of its two diagonals dents the solid very slightly, which the Pack
    checker refuses at any size."""
    triangles = []
    for face in faces:
        if len(face) == 3:
            triangles.append(tuple(face))
            continue
        a, b, c, d = face
        if convex:
            normal = cross(sub(corners[b], corners[a]), sub(corners[c], corners[a]))
            if dot(normal, sub(corners[d], corners[a])) > 0.0:
                # d sticks out past the plane of a, b, c: cut b to d.
                triangles += [(a, b, d), (b, c, d)]
                continue
        triangles += [(a, b, c), (a, c, d)]
    return triangles


def box_mesh(sx, sy, sz):
    """A box of this size, centred on its own origin."""
    hx, hy, hz = sx / 2.0, sy / 2.0, sz / 2.0
    corners = [(x * hx, y * hy, z * hz) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)]
    faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    return corners, point_outwards(corners, faces)


def _ring(r, z, segments, phase=0.0):
    return [
        (r * math.cos(2.0 * math.pi * i / segments + phase), r * math.sin(2.0 * math.pi * i / segments + phase), z)
        for i in range(segments)
    ]


def cylinder_mesh(height, r0, r1=None, segments=12):
    """A cylinder or cone frustum standing on its own +z axis from 0 to
    `height`, `r0` across at the bottom and `r1` (default `r0`) at the top; a
    radius of 0 is a point. A cap at each end that has a radius, each a fan
    round a corner at its middle."""
    r1 = r0 if r1 is None else r1
    corners, faces = [], []
    ring0 = len(corners)
    corners += _ring(r0, 0.0, segments) if r0 > 0 else [(0.0, 0.0, 0.0)]
    ring1 = len(corners)
    corners += _ring(r1, height, segments) if r1 > 0 else [(0.0, 0.0, height)]
    at0 = [ring0 + (i if r0 > 0 else 0) for i in range(segments)]
    at1 = [ring1 + (i if r1 > 0 else 0) for i in range(segments)]
    for i in range(segments):
        j = (i + 1) % segments
        if r0 > 0 and r1 > 0:
            faces.append((at0[i], at0[j], at1[j], at1[i]))
        elif r0 > 0:
            faces.append((at0[i], at0[j], at1[i]))
        else:
            faces.append((at0[i], at1[j], at1[i]))
    # The side faces run anticlockwise along the bottom edge, so the bottom
    # cap runs the other way round, to share each edge in the opposite
    # direction.
    for ring, r, z, bottom in ((at0, r0, 0.0, True), (at1, r1, height, False)):
        if r > 0:
            middle = len(corners)
            corners.append((0.0, 0.0, z))
            for i in range(segments):
                a, b = ring[i], ring[(i + 1) % segments]
                faces.append((middle, b, a) if bottom else (middle, a, b))
    return corners, point_outwards(corners, faces)


def triangulate_polygon(points):
    """Ear clipping of a simple polygon given as `(u, v)` points, in a fixed
    order: the same triangles on every computer. Triangles come out
    anticlockwise."""
    n = len(points)
    area = sum(points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1] for i in range(n))
    order = list(range(n)) if area > 0 else list(reversed(range(n)))
    triangles = []

    def cross2(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    def inside(p, a, b, c):
        return cross2(a, b, p) >= -1e-12 and cross2(b, c, p) >= -1e-12 and cross2(c, a, p) >= -1e-12

    while len(order) > 3:
        for k in range(len(order)):
            a, b, c = order[k - 1], order[k], order[(k + 1) % len(order)]
            pa, pb, pc = points[a], points[b], points[c]
            if cross2(pa, pb, pc) <= 1e-12:
                continue  # a reflex or flat corner is no ear
            if any(
                inside(points[o], pa, pb, pc)
                for o in order
                if o not in (a, b, c) and points[o] not in (pa, pb, pc)
            ):
                continue
            triangles.append((a, b, c))
            del order[k]
            break
        else:
            # Only flat corners left (a profile with collinear points).
            triangles.append((order[0], order[1], order[2]))
            del order[1]
    triangles.append((order[0], order[1], order[2]))
    return [
        (a, b, c) if cross2(points[a], points[b], points[c]) >= 0 else (a, c, b) for a, b, c in triangles
    ]


def prism_mesh(profile, width):
    """A closed outline of `(along, up)` points, cut out across `width`,
    centred on x = 0: the profile's `along` runs on the shape's own y axis and
    its `up` on z, so a quarter pipe's side or a flight of steps is a
    profile."""
    n = len(profile)
    corners = [(x, a, z) for x in (-width / 2.0, width / 2.0) for a, z in profile]
    faces = [(i, (i + 1) % n, n + (i + 1) % n, n + i) for i in range(n)]
    for a, b, c in triangulate_polygon(profile):
        faces.append((a, b, c))
        faces.append((n + a, n + c, n + b))
    return corners, point_outwards(corners, faces)


def tube_mesh(path, radius, segments=8, closed_loop=False):
    """A round bar swept along a polyline: coping, a ring, a curved rail. An
    open bar is closed with a flat cap at each end."""
    n = len(path)
    corners, faces = [], []
    up = (0.0, 0.0, 1.0)
    for k, p in enumerate(path):
        if closed_loop:
            t = unit(sub(path[(k + 1) % n], path[(k - 1) % n]))
        else:
            t = unit(sub(path[min(k + 1, n - 1)], path[max(k - 1, 0)]))
        ref = up if abs(dot(t, up)) < 0.9 else (1.0, 0.0, 0.0)
        a = unit(cross(t, ref))
        b = unit(cross(t, a))
        for i in range(segments):
            angle = 2.0 * math.pi * i / segments
            corners.append(add(p, add(mul(a, radius * math.cos(angle)), mul(b, radius * math.sin(angle)))))
    rings = n if closed_loop else n - 1
    for k in range(rings):
        k2 = (k + 1) % n
        for i in range(segments):
            j = (i + 1) % segments
            faces.append((k * segments + i, k * segments + j, k2 * segments + j, k2 * segments + i))
    if not closed_loop:
        for ring, end, start in ((0, 0, True), ((n - 1) * segments, n - 1, False)):
            middle = len(corners)
            corners.append(path[end])
            for i in range(segments):
                a, b = ring + i, ring + (i + 1) % segments
                faces.append((middle, b, a) if start else (middle, a, b))
    return corners, point_outwards(corners, faces)


def annulus_mesh(length_along, r_in, r_out, segments=28):
    """A thick-walled tube standing on its own +z axis from 0 to
    `length_along`: a full pipe."""
    corners, faces = [], []
    for z in (0.0, length_along):
        corners += _ring(r_out, z, segments)
        corners += _ring(r_in, z, segments)
    o0, i0, o1, i1 = 0, segments, 2 * segments, 3 * segments
    for i in range(segments):
        j = (i + 1) % segments
        faces.append((o0 + i, o0 + j, o1 + j, o1 + i))  # outside
        faces.append((i0 + j, i0 + i, i1 + i, i1 + j))  # inside
        faces.append((o0 + j, o0 + i, i0 + i, i0 + j))  # end ring 0
        faces.append((o1 + i, o1 + j, i1 + j, i1 + i))  # end ring 1
    return corners, point_outwards(corners, faces)


def heightfield_mesh(x0, y0, x1, y1, step, height):
    """A sheet over a rectangle, the height from `height(x, y)` at a corner
    every `step` or so, facing up. Each square is cut along the diagonal that
    changes height least."""
    nx, ny = max(1, round((x1 - x0) / step)), max(1, round((y1 - y0) / step))
    corners = []
    for j in range(ny + 1):
        y = y0 + (y1 - y0) * j / ny
        for i in range(nx + 1):
            x = x0 + (x1 - x0) * i / nx
            corners.append((x, y, height(x, y)))
    faces = []
    for j in range(ny):
        for i in range(nx):
            a = j * (nx + 1) + i
            b, c, d = a + 1, a + nx + 2, a + nx + 1
            if abs(corners[a][2] - corners[c][2]) <= abs(corners[b][2] - corners[d][2]):
                faces += [(a, b, c), (a, c, d)]
            else:
                faces += [(a, b, d), (b, c, d)]
    return corners, faces


def rect_subtract(outer, holes):
    """The rectangle `outer` (x0, y0, x1, y1) minus the `holes`, as a few
    rectangles, in a fixed order."""
    x0, y0, x1, y1 = outer
    hs = [(max(a, x0), max(b, y0), min(c, x1), min(d, y1)) for a, b, c, d in holes]
    hs = [h for h in hs if h[2] > h[0] + 1e-6 and h[3] > h[1] + 1e-6]
    xs = sorted({x0, x1, *[h[0] for h in hs], *[h[2] for h in hs]})
    ys = sorted({y0, y1, *[h[1] for h in hs], *[h[3] for h in hs]})

    def solid(cx, cy):
        return not any(a < cx < c and b < cy < d for a, b, c, d in hs)

    rows = []
    for j in range(len(ys) - 1):
        cy = (ys[j] + ys[j + 1]) / 2
        run = None
        for i in range(len(xs) - 1):
            cx = (xs[i] + xs[i + 1]) / 2
            if solid(cx, cy):
                run = [run[0], xs[i + 1]] if run else [xs[i], xs[i + 1]]
            elif run:
                rows.append((run[0], ys[j], run[1], ys[j + 1]))
                run = None
        if run:
            rows.append((run[0], ys[j], run[1], ys[j + 1]))
    merged = []
    for r in rows:
        for k, m in enumerate(merged):
            if abs(m[0] - r[0]) < 1e-6 and abs(m[2] - r[2]) < 1e-6 and abs(m[3] - r[1]) < 1e-6:
                merged[k] = (m[0], m[1], m[2], r[3])
                break
        else:
            merged.append(r)
    return merged


def placed(mesh, pose):
    """The mesh moved into the Map by a pose, every corner snapped."""
    corners, faces = mesh
    return [snap3(pose.apply(c)) for c in corners], list(faces)
