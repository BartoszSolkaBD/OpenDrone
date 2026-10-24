"""Readable checks for the relief (`odrelief.py`, `odbrick.py`, `odframe.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check does too). Each test's name is the
sentence it proves. Basis: Rule, from #340: the generators give the same
bytes from the same seed, nothing they make blocks a flying line marked as
clear (#339's keep-clear zones), and the ground stays in its range. Lengths
are in metres.
"""
import hashlib
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from odbrick import LEAF, RECESS, brick_face  # noqa: E402
from odframe import BuckledRoof, Frame  # noqa: E402
from odground import Ground  # noqa: E402
from odrelief import Relief, graded_mesh  # noqa: E402
from odscatter import Scatter  # noqa: E402

HALL = (-8.0, -6.0, 8.0, 6.0)
AISLE = (-1.5, -6.0, 1.5, 6.0)
TOLERANCE = 0.001  # 1 mm


def digest(mesh):
    corners, faces = mesh
    return hashlib.sha256(repr(([g.snap3(c) for c in corners], [tuple(f) for f in faces])).encode()).hexdigest()


def zones():
    s = Scatter(1)
    s.keep_clear_rect(*AISLE)
    s.keep_clear_circle(5.0, -3.0, 1.0)
    return s


def floor(seed=8, clear=True):
    plain = Ground(3, amplitude=0.02, wavelength=6.0)
    r = Relief(plain.height, seed, clear=zones().clearance if clear else None)
    r.crumble(HALL, depth=0.14, grain=0.35, walls=[((-8.0, 5.8), (8.0, 5.8))])
    r.rut((-8.0, -4.0), (8.0, -3.0))
    r.pothole(4.0, 2.0, 0.6, 0.1)
    r.worn(-5.0, 0.0, 0.25)
    r.worn_rect((3.0, 3.0, 5.0, 4.0))
    r.under_heap(-4.0, 4.0, 2.5, 0.3)
    return plain, r


def grid(rect, step):
    x0, y0, x1, y1 = rect
    nx, ny = round((x1 - x0) / step), round((y1 - y0) / step)
    return [(x0 + (x1 - x0) * i / nx, y0 + (y1 - y0) * j / ny) for j in range(ny + 1) for i in range(nx + 1)]


def roofs(**kw):
    return BuckledRoof.gable(-8.3, -6.3, 8.3, 6.3, 5.25, 6.9, 4, buckle=0.07, step=0.3,
                             south=dict(missing=list(range(4, 12))), north=dict(missing=[2, 3]), **kw)


class SameSeed(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_ground_mesh(self):
        a = graded_mesh(HALL, floor()[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        b = graded_mesh(HALL, floor()[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        self.assertEqual(digest(a), digest(b))

    def test_another_seed_gives_another_ground(self):
        a = graded_mesh(HALL, floor(8)[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        b = graded_mesh(HALL, floor(9)[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        self.assertNotEqual(digest(a), digest(b))

    def test_the_same_seed_gives_the_very_same_brickwork(self):
        a = brick_face(4.5, 3.0, 5, patches=[(1.5, 1.0, 0.5)], render=[(3.2, 2.0, 1.0, 0.7)])
        b = brick_face(4.5, 3.0, 5, patches=[(1.5, 1.0, 0.5)], render=[(3.2, 2.0, 1.0, 0.7)])
        self.assertEqual((digest(a.brick), digest(a.render)), (digest(b.brick), digest(b.render)))

    def test_the_same_seed_gives_the_very_same_roof_and_frame(self):
        (s1, n1), (s2, n2) = roofs(), roofs()
        self.assertEqual(digest(s1.iron()), digest(s2.iron()))
        self.assertEqual(digest(Frame(s1, n1, 4).mesh()), digest(Frame(s2, n2, 4).mesh()))


class Ground_(unittest.TestCase):
    def test_the_relief_stays_within_its_range_of_the_plain_ground(self):
        plain, r = floor()
        for x, y in grid(HALL, 0.1):
            d = r.height(x, y) - plain.height(x, y)
            self.assertGreaterEqual(d, r.lowest - TOLERANCE)
            self.assertLessEqual(d, r.highest + TOLERANCE)

    def test_nothing_rises_into_a_flying_line_marked_as_clear(self):
        plain, r = floor()
        clear = zones()
        for x, y in grid(HALL, 0.1):
            if clear.clearance(x, y) <= 0.0:
                self.assertLessEqual(r.height(x, y) - plain.height(x, y), r.clear_rise + TOLERANCE, (x, y))

    def test_without_a_keep_clear_zone_the_aisle_has_its_relief(self):
        plain, r = floor(clear=False)
        rise = max(r.height(x, y) - plain.height(x, y) for x, y in grid(AISLE, 0.1))
        self.assertGreater(rise, 0.05)

    def test_the_crumbled_layer_is_thick_enough_to_read_as_relief(self):
        plain, r = floor(clear=False)
        layers = sorted(r.layer(x, y) for x, y in grid((-7.0, -5.0, 7.0, 5.0), 0.1))
        self.assertGreater(layers[len(layers) // 2], 0.03)  # half the floor under 3 cm or more
        self.assertGreater(layers[-1], 0.10)

    def test_the_layer_is_thicker_along_the_wall_it_came_down_from(self):
        plain, r = floor(clear=False)
        near = sum(r.layer(x, 5.6) for x in range(-6, 7)) / 13.0
        far = sum(r.layer(x, 0.5) for x in range(-6, 7)) / 13.0
        self.assertGreater(near, far)

    def test_the_floor_round_a_column_is_worn_smooth(self):
        plain, r = floor(clear=False)

        def roughness(cx, cy):
            pts = [(cx + 0.05 * i, cy + 0.05 * j) for i in range(-4, 5) for j in range(-4, 5)]
            ds = [r.relief(x, y) for x, y in pts]
            return max(ds) - min(ds)
        self.assertLess(roughness(-5.0, 0.6), roughness(-2.5, 0.6))

    def test_a_pothole_is_below_the_ground_round_it(self):
        r = Relief(lambda x, y: 0.0, 8)
        r.pothole(4.0, 2.0, 0.6, 0.1)
        self.assertLess(r.relief(4.0, 2.0), r.relief(4.0, 3.2) - 0.05)

    def test_the_graded_mesh_has_no_crack_where_fine_meets_coarse(self):
        corners, faces = graded_mesh(HALL, floor()[1].height, fine=[AISLE, (4.0, 1.0, 6.0, 3.0)], fine_step=0.2, coarse_step=0.8)
        edges = {}
        for f in faces:
            for k in range(3):
                e = tuple(sorted((f[k], f[(k + 1) % 3])))
                edges[e] = edges.get(e, 0) + 1
        x0, y0, x1, y1 = HALL
        for (a, b), n in sorted(edges.items()):
            if n == 1:  # an open edge lies on the sheet's own border
                pa, pb = corners[a], corners[b]
                on_border = (abs(pa[0] - pb[0]) < 1e-9 and pa[0] in (x0, x1)) or (abs(pa[1] - pb[1]) < 1e-9 and pa[1] in (y0, y1))
                self.assertTrue(on_border, (pa, pb))
            else:
                self.assertEqual(n, 2)

    def test_the_graded_mesh_faces_up_and_covers_its_rectangle_once(self):
        corners, faces = graded_mesh(HALL, floor()[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        area = 0.0
        for a, b, c in faces:
            n = g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a]))
            self.assertGreater(n[2], 0.0)
            area += n[2] / 2.0
        self.assertAlmostEqual(area, (HALL[2] - HALL[0]) * (HALL[3] - HALL[1]), places=6)

    def test_the_graded_mesh_is_fine_only_where_asked(self):
        fine = graded_mesh(HALL, floor()[1].height, fine=[HALL], fine_step=0.2, coarse_step=0.8)
        graded = graded_mesh(HALL, floor()[1].height, fine=[AISLE], fine_step=0.2, coarse_step=0.8)
        self.assertLess(len(graded[1]), len(fine[1]) / 3)


class Brickwork(unittest.TestCase):
    def leaf(self):
        return brick_face(4.5, 3.0, 5, patches=[(1.0, 1.2, 0.5)], render=[(3.4, 2.2, 1.0, 0.7)],
                          holes=[(2.0, 3.2, 0.0, 2.1)])

    def test_no_brick_is_laid_over_an_opening_so_the_flying_line_through_it_stays_open(self):
        corners, _ = self.leaf().brick
        for u, y, z in corners:
            inside = 2.0 + 1e-6 < u < 3.2 - 1e-6 and z < 2.1 - 1e-6
            self.assertFalse(inside and y > 1e-6, (u, y, z))

    def test_bricks_stand_proud_of_their_sunken_mortar(self):
        ys = sorted({round(c[1], 4) for c in self.leaf().brick[0]})
        self.assertIn(round(LEAF - RECESS, 4), ys)
        self.assertGreater(ys[-1], LEAF - RECESS + 0.005)

    def test_a_patch_of_missing_bricks_goes_back_to_the_wall_behind(self):
        leaf = self.leaf()
        self.assertGreater(leaf.missing, 5)
        near = [c for c in leaf.brick[0] if abs(c[0] - 1.0) < 0.6 and abs(c[2] - 1.2) < 0.5]
        self.assertTrue(any(abs(c[1]) < 1e-9 for c in near))

    def test_render_covers_the_bricks_under_it(self):
        leaf = self.leaf()
        self.assertGreater(leaf.rendered, 0)
        self.assertTrue(all(c[1] >= LEAF - 0.005 for c in leaf.render[0]))


class RoofAndFrame(unittest.TestCase):
    def test_sheets_dip_between_the_purlins(self):
        south, _ = roofs()
        s0, s1 = south.purlins()[1], south.purlins()[2]
        on = south._sheet_dz(0, 1.0, s0) - BuckledRoof.__mro__[1]._sheet_dz(south, 0, 1.0, s0)
        mid = south._sheet_dz(0, 1.0, (s0 + s1) / 2.0) - BuckledRoof.__mro__[1]._sheet_dz(south, 0, 1.0, (s0 + s1) / 2.0)
        self.assertLess(mid, on)

    def test_only_purlins_with_no_sheets_over_them_are_bent_or_hanging(self):
        south, north = roofs()
        frame = Frame(south, north, 4)
        bent = [m for m in frame.members if m[0] == "purlin" and not m[6]]
        self.assertGreater(len(bent), 3)
        for kind, p0, p1, *_ in bent:
            mid = ((p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0)
            roof = south if mid[1] < 0.0 else north
            a = mid[0] - (-8.3) if roof is south else 8.3 - mid[0]
            i = min(int(a / roof.width), roof.sheet_count - 1)
            self.assertIn(i, roof.missing, (p0, p1))

    def test_nothing_of_the_frame_hangs_down_into_a_flying_line_marked_as_clear(self):
        south, north = roofs()
        clear = zones()

        def lowest_over_the_aisle(frame):
            return min(z for x, y, z in frame.mesh()[0] if clear.clearance(x, y) <= 0.0)
        self.assertGreater(lowest_over_the_aisle(Frame(south, north, 4, clear=clear.clearance)), 4.5)
        # Without the zone, the same seed lets a purlin hang into it.
        self.assertLess(lowest_over_the_aisle(Frame(south, north, 4)), 4.5)

    def test_a_flying_line_narrower_than_a_purlin_span_is_still_kept_clear(self):
        south, north = roofs()
        for seed in range(12):
            for x0 in (-3.9, -1.3, 0.3, 2.6):
                narrow = Scatter(1)
                narrow.keep_clear_rect(x0, -6.0, x0 + 0.8, 6.0)  # a doorway's width
                frame = Frame(south, north, seed, clear=narrow.clearance)
                low = min(z for x, y, z in frame.mesh()[0] if narrow.clearance(x, y) <= 0.0)
                self.assertGreater(low, 4.3, (seed, x0))

    def test_a_hanging_purlin_keeps_its_length(self):
        south, north = roofs()
        frame = Frame(south, north, 4)
        hanging = [m for m in frame.members if m[0] == "purlin" and m[5] is None and not m[6]]
        self.assertTrue(hanging)
        for _, p0, p1, *_ in hanging:
            self.assertAlmostEqual(g.length(g.sub(p1, p0)), south.truss_spacing, delta=0.01)


if __name__ == "__main__":
    unittest.main()
