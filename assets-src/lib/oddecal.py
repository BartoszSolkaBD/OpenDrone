"""Decals: pictures that lie on a Map's surfaces (#321), MIT OR Apache-2.0.

A decal is a stain, a wet patch, moss at the foot of a wall, a water streak, a
crack or a tag: one picture, put once where the Map wants it, over a surface
whose own picture repeats. It is a thin mesh of its own that hugs the surface
under it, a hair above it, so the game draws it over that surface.

The decal is a **projector**: a rectangle `size` metres (width, height) round
`centre`, facing `facing` (out of the surface, toward whoever looks at it),
with the picture's top toward `up`. It reaches `depth` metres into and out of
the surface along `facing`. Every triangle of the surfaces it is put on that
faces it (within `max_angle`) and lies in its box is cut to the box, so the
decal follows uneven ground, a bulging wall or the bend where a wall meets the
floor. Each corner gets two sets of picture coordinates:

- where it is in the decal's picture, from 0 to 1 across it, the picture's
  top at `up`;
- where the surface under it is in the Map's light atlas, if the surface has
  one: worked out from the surface's own, inside the triangle it was cut from,
  so the decal is lit by the very light baked for the spot it lies on.

The cut corners are lifted `lift` metres along `facing` (2 mm), all the same
way, so a decal over two triangles that meet at an angle has no crack where
they meet. Everything is plain Python on 64-bit numbers with the four basic
operations and square roots (`odgeom`), so it is the same on every computer,
and every corner is rounded to 0.1 mm as the Map's are.
"""
import math

import odgeom as g

LIFT = 0.002  # metres the decal floats over the surface it lies on
UV_GRID = 100000.0  # a decal's picture coordinates are rounded to 1e-5


class Host:
    """A surface a decal can lie on: its corners, its triangles (corner
    numbers, anticlockwise seen from the front) and, if it has them, its
    light atlas coordinates, three `(u, v)` for each triangle in its order."""

    def __init__(self, corners, triangles, light_uvs=None):
        self.corners = corners
        self.triangles = triangles
        self.light_uvs = light_uvs


class Decal:
    def __init__(self, centre, facing, up, size, depth=0.4, max_angle=75.0, lift=LIFT):
        self.centre = tuple(float(c) for c in centre)
        self.facing = g.unit(facing)
        right = g.cross(up, self.facing)
        assert g.length(right) > 1e-9, "a decal's `up` must not point the way it faces"
        self.right = g.unit(right)
        self.up = g.unit(g.cross(self.facing, self.right))
        self.width, self.height = float(size[0]), float(size[1])
        assert self.width > 0.0 and self.height > 0.0, "a decal has a size"
        self.depth = float(depth)
        self.min_facing = math.cos(math.radians(max_angle))
        self.lift = float(lift)

    def _local(self, p):
        """Where a point is in the decal's box: across (0 to 1), up (0 to 1)
        and out of the surface (metres)."""
        d = g.sub(p, self.centre)
        return (
            g.dot(d, self.right) / self.width + 0.5,
            g.dot(d, self.up) / self.height + 0.5,
            g.dot(d, self.facing),
        )

    def project(self, hosts):
        """The decal's mesh on these surfaces: `(corners, triangles, uvs,
        light_uvs)`, one picture coordinate and (if every host has them) one
        light atlas coordinate per corner; `light_uvs` is None if any host has
        none. Corners aren't shared between triangles. Empty if the decal
        lies on nothing."""
        with_light = all(h.light_uvs is not None for h in hosts)
        corners, triangles, uvs, light = [], [], [], []
        half = self.depth / 2.0
        planes = (
            lambda q: q[0],
            lambda q: 1.0 - q[0],
            lambda q: q[1],
            lambda q: 1.0 - q[1],
            lambda q: q[2] + half,
            lambda q: half - q[2],
        )
        for host in hosts:
            for k, (a, b, c) in enumerate(host.triangles):
                pa, pb, pc = host.corners[a], host.corners[b], host.corners[c]
                n = g.cross(g.sub(pb, pa), g.sub(pc, pa))
                size = g.length(n)
                if size == 0.0 or g.dot(n, self.facing) / size < self.min_facing:
                    continue
                luv = host.light_uvs[k] if with_light else ((0.0, 0.0),) * 3
                # A point is (place, its place in the box, light atlas coordinate).
                polygon = [(p, self._local(p), tuple(map(float, l))) for p, l in zip((pa, pb, pc), luv)]
                for inside in planes:
                    polygon = _clip(polygon, inside)
                    if len(polygon) < 3:
                        break
                if len(polygon) < 3:
                    continue
                self._add(polygon, with_light, corners, triangles, uvs, light)
        return corners, triangles, uvs, (light if with_light else None)

    def _add(self, polygon, with_light, corners, triangles, uvs, light):
        points = []
        for p, q, l in polygon:
            at = g.snap3(g.add(p, g.mul(self.facing, self.lift)))
            if points and points[-1][0] == at:
                continue
            uv = tuple(min(1.0, max(0.0, round(x * UV_GRID) / UV_GRID)) + 0.0 for x in q[:2])
            points.append((at, uv, l))
        if len(points) > 2 and points[0][0] == points[-1][0]:
            points.pop()
        for i in range(1, len(points) - 1):
            tri = (points[0], points[i], points[i + 1])
            n = g.cross(g.sub(tri[1][0], tri[0][0]), g.sub(tri[2][0], tri[0][0]))
            if g.length(n) <= 1e-10:
                continue  # no area left after rounding
            base = len(corners)
            for at, uv, l in tri:
                corners.append(at)
                uvs.append(uv)
                if with_light:
                    light.append(l)
            triangles.append((base, base + 1, base + 2))


def _clip(polygon, inside):
    """The part of a convex polygon where `inside(box place) >= 0`, its new
    corners placed (and their coordinates found) along the cut edges."""
    out = []
    for i, current in enumerate(polygon):
        previous = polygon[i - 1]
        dc, dp = inside(current[1]), inside(previous[1])
        if (dc >= 0.0) != (dp >= 0.0):
            t = dp / (dp - dc)
            out.append((
                g.lerp(previous[0], current[0], t),
                g.lerp(previous[1], current[1], t),
                (previous[2][0] + (current[2][0] - previous[2][0]) * t,
                 previous[2][1] + (current[2][1] - previous[2][1]) * t),
            ))
        if dc >= 0.0:
            out.append(current)
    return out
