"""Readable checks for decals (`oddecal.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`). Each test's name is the sentence it proves. Basis: Rule,
from #321: a decal lies on the surface under it, a hair above it, follows its
shape, takes its picture once (coordinates 0 to 1) and the light baked for the
spot it lies on; it is the same on every computer.
"""
import hashlib
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from oddecal import LIFT, Decal, Host  # noqa: E402
from odground import Ground  # noqa: E402

UP, SOUTH, NORTH = (0.0, 0.0, 1.0), (0.0, -1.0, 0.0), (0.0, 1.0, 0.0)


def floor(step=0.5, height=lambda x, y: 0.0, rect=(-5.0, -5.0, 5.0, 5.0), light=False):
    corners, faces = g.heightfield_mesh(*rect, step, height)
    triangles = g.triangulate(corners, faces)
    light_uvs = None
    if light:
        # A made-up atlas that is easy to check: u and v grow with east and north.
        at = lambda p: (p[0] / 20.0 + 0.5, p[1] / 20.0 + 0.5)
        light_uvs = [tuple(at(corners[i]) for i in t) for t in triangles]
    return Host(corners, triangles, light_uvs)


def wall_box(x0=-3.0, x1=3.0, y=2.0, thick=0.3, height=3.0):
    """A wall as a closed box, its south face at `y`."""
    corners, faces = g.box_mesh(x1 - x0, thick, height)
    corners = [g.add(c, ((x0 + x1) / 2.0, y + thick / 2.0, height / 2.0)) for c in corners]
    return Host(corners, g.triangulate(corners, faces))


def area(corners, triangles):
    return sum(
        g.length(g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a]))) / 2.0
        for a, b, c in triangles
    )


def normal(corners, t):
    a, b, c = (corners[i] for i in t)
    return g.unit(g.cross(g.sub(b, a), g.sub(c, a)))


class OnAFloor(unittest.TestCase):
    def setUp(self):
        self.decal = Decal((1.0, 0.5, 0.0), UP, NORTH, (2.0, 1.0))
        self.mesh = self.decal.project([floor()])

    def test_it_covers_exactly_its_rectangle(self):
        corners, triangles, _uvs, _light = self.mesh
        self.assertAlmostEqual(area(corners, triangles), 2.0, delta=1e-3)
        xs = [c[0] for c in corners]
        ys = [c[1] for c in corners]
        self.assertAlmostEqual(min(xs), 0.0, delta=1e-4)
        self.assertAlmostEqual(max(xs), 2.0, delta=1e-4)
        self.assertAlmostEqual(min(ys), 0.0, delta=1e-4)
        self.assertAlmostEqual(max(ys), 1.0, delta=1e-4)

    def test_it_floats_two_millimetres_over_the_floor_and_faces_up(self):
        corners, triangles, _uvs, _light = self.mesh
        for c in corners:
            self.assertAlmostEqual(c[2], LIFT, delta=1e-4)
        for t in triangles:
            self.assertGreater(normal(corners, t)[2], 0.999)

    def test_its_picture_runs_from_0_to_1_across_it_with_the_top_toward_up(self):
        corners, _triangles, uvs, _light = self.mesh
        for (x, y, _z), (u, v) in zip(corners, uvs):
            self.assertTrue(0.0 <= u <= 1.0 and 0.0 <= v <= 1.0)
            self.assertAlmostEqual(u, x / 2.0, delta=1e-4)   # east is right on the picture
            self.assertAlmostEqual(v, y / 1.0, delta=1e-4)   # north (up) is its top

    def test_every_corner_is_rounded_to_a_tenth_of_a_millimetre(self):
        corners, _t, _u, _l = self.mesh
        for c in corners:
            self.assertEqual(c, g.snap3(c))

    def test_a_floor_with_no_light_coordinates_gives_a_decal_with_none(self):
        self.assertIsNone(self.mesh[3])


class FollowsTheSurface(unittest.TestCase):
    def test_on_uneven_ground_every_corner_is_a_lift_above_the_ground_under_it(self):
        ground = Ground(3, amplitude=0.3, wavelength=4.0)
        host = floor(step=0.25, height=ground.height)
        corners, triangles, _uvs, _light = Decal((0.3, -0.2, 0.0), UP, NORTH, (3.0, 2.0)).project([host])
        self.assertGreater(len(triangles), 100)
        for c in corners:
            under = height_under(host, c[0], c[1])
            # Lifted straight up, so a sloping triangle is a hair less than the lift off it.
            self.assertGreater(c[2] - under, LIFT * 0.9 - 2e-4, c)
            self.assertLess(c[2] - under, LIFT + 2e-4, c)

    def test_it_takes_the_light_baked_for_the_spot_it_lies_on(self):
        corners, _t, _uvs, light = Decal((1.0, 0.5, 0.0), UP, NORTH, (2.0, 1.0)).project([floor(light=True)])
        self.assertIsNotNone(light)
        for c, (u, v) in zip(corners, light):
            self.assertAlmostEqual(u, c[0] / 20.0 + 0.5, delta=1e-5)
            self.assertAlmostEqual(v, c[1] / 20.0 + 0.5, delta=1e-5)

    def test_it_lies_only_on_faces_that_look_toward_it(self):
        # Deep enough to reach through the wall, it still lies on the south face alone.
        corners, triangles, _u, _l = Decal((0.0, 2.0, 1.5), SOUTH, UP, (2.0, 1.0), depth=2.0).project([wall_box()])
        self.assertAlmostEqual(area(corners, triangles), 2.0, delta=1e-3)
        for c in corners:
            self.assertAlmostEqual(c[1], 2.0 - LIFT, delta=1e-4)

    def test_a_strip_at_the_foot_of_a_wall_bends_from_the_floor_up_the_wall(self):
        facing = (0.0, -1.0, 1.0)   # out of the corner, half up and half south
        decal = Decal((0.0, 1.8, 0.2), facing, (0.0, 1.0, 1.0), (3.0, 0.8), depth=1.0)
        corners, triangles, _u, _l = decal.project([floor(), wall_box()])
        up = [t for t in triangles if normal(corners, t)[2] > 0.99]
        south = [t for t in triangles if normal(corners, t)[1] < -0.99]
        self.assertTrue(up and south, "on the floor and on the wall")
        self.assertEqual(len(up) + len(south), len(triangles))

    def test_a_decal_over_nothing_is_empty(self):
        self.assertEqual(Decal((40.0, 40.0, 0.0), UP, NORTH, (1.0, 1.0)).project([floor()])[:3], ([], [], []))


class Repeatable(unittest.TestCase):
    def test_the_same_decal_gives_the_very_same_mesh_every_time(self):
        ground = Ground(3, amplitude=0.2, wavelength=4.0)

        def digest():
            mesh = Decal((0.3, -0.2, 0.0), UP, (0.3, 1.0, 0.0), (2.5, 1.5)).project(
                [floor(step=0.25, height=ground.height, light=True)]
            )
            return hashlib.sha256(repr(mesh).encode()).hexdigest()

        self.assertEqual(digest(), digest())


def height_under(host, x, y):
    """The height of the host's triangle under (x, y), seen from above."""
    for a, b, c in host.triangles:
        pa, pb, pc = host.corners[a], host.corners[b], host.corners[c]
        d = (pb[1] - pc[1]) * (pa[0] - pc[0]) + (pc[0] - pb[0]) * (pa[1] - pc[1])
        if d == 0.0:
            continue
        l1 = ((pb[1] - pc[1]) * (x - pc[0]) + (pc[0] - pb[0]) * (y - pc[1])) / d
        l2 = ((pc[1] - pa[1]) * (x - pc[0]) + (pa[0] - pc[0]) * (y - pc[1])) / d
        l3 = 1.0 - l1 - l2
        if min(l1, l2, l3) >= -1e-6:
            return l1 * pa[2] + l2 * pb[2] + l3 * pc[2]
    raise AssertionError(f"nothing under {x}, {y}")


if __name__ == "__main__":
    unittest.main()
