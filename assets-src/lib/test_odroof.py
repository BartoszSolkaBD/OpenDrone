"""Readable checks for sagging, damaged roofs (`odroof.py`, `odcut.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`). Each test's name is the sentence it proves. Basis: Rule,
from #322 and docs/building-a-map.md: the same bytes on every computer, a
mesh the builder can take (every triangle keeps an area after the corners are
rounded to 0.1 mm), shapes that stay inside their bounds, and a roof that is
no longer a plane.
"""
import hashlib
import math
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odcut  # noqa: E402
import odgeom as g  # noqa: E402
from odroof import OVERLAP, Roof, Skylight, Tear  # noqa: E402


def make(seed=4, **kw):
    south = dict(missing=[3], skylights=[Skylight(6.0, 7.0, 2.5, 5.0, "broken"), Skylight(9.5, 10.5, 2.5, 5.0, "gone"),
                                         Skylight(12.0, 13.0, 2.5, 5.0, "intact")])
    north = dict(missing=[11], tears=[Tear(10.0, 2.0, 0.6)])
    return Roof.gable(0.0, 0.0, 15.0, 12.0, 4.0, 5.5, seed, south=south, north=north, **kw)


def to_roof(roof, p):
    """A place in the Map as `(a, s, dz)`: along the eave, up the slope and
    above the plane roof."""
    r = roof.pose.rows
    d = g.sub(p, roof.pose.at)
    return tuple(r[0][k] * d[0] + r[1][k] * d[1] + r[2][k] * d[2] for k in range(3))


def digest(mesh):
    corners, faces = mesh
    return hashlib.sha256((repr([g.snap3(c) for c in corners]) + repr(faces)).encode()).hexdigest()


def snapped_triangles(mesh):
    corners, faces = mesh
    cs = [g.snap3(c) for c in corners]
    return cs, g.triangulate(cs, faces)


class Repeatable(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_roof_every_time(self):
        a, b = make(4)[0], make(4)[0]
        self.assertEqual(digest(a.iron()), digest(b.iron()))
        self.assertEqual(digest(a.glass()), digest(b.glass()))
        self.assertEqual(digest(a.collision_mesh()), digest(b.collision_mesh()))

    def test_another_seed_gives_another_roof(self):
        self.assertNotEqual(digest(make(4)[0].iron()), digest(make(5)[0].iron()))

    def test_the_roof_has_the_very_same_bytes_on_every_computer(self):
        # Observed: the digest on the Mac that wrote the test; CI's Linux
        # run proves the same on its computer.
        self.assertEqual(digest(make(4)[0].iron()), "7ca598cc78a3188f381e7d9ba2890526da83c83f1c5bbce53bcebe6cab9e4df8")


class Shape(unittest.TestCase):
    def test_the_roof_is_held_up_at_every_truss_and_sags_most_half_way_between(self):
        roof = make()[0]
        for a in roof.trusses():
            self.assertAlmostEqual(roof.sag_at(a), 0.0, places=12)
        for k in range(roof.bay_count):
            mid = (k + 0.5) * roof.truss_spacing
            self.assertGreater(roof.sag_at(mid), 0.0)
            self.assertLessEqual(roof.sag_at(mid), roof.sag_max * 1.4 + 1e-12)
            self.assertGreater(roof.sag_at(mid), roof.sag_at(mid - 0.6 * roof.truss_spacing / 2.0))

    def test_a_bay_that_has_fallen_in_sags_much_deeper_than_the_rest(self):
        roof = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 1, bays={2: 8.0})
        deep = roof.sag_at(2.5 * roof.truss_spacing)
        self.assertGreater(deep, 4.0 * roof.sag_at(1.5 * roof.truss_spacing))
        self.assertGreater(deep, 0.4)

    def test_every_corner_of_the_iron_stays_inside_the_roofs_bounds(self):
        roof = make()[0]
        corners, _ = roof.iron()
        margin = roof.thick + 1e-6
        for p in corners:
            a, s, dz = to_roof(roof, p)
            self.assertTrue(-1e-6 <= a <= roof.length + 1e-6, a)
            self.assertTrue(-1e-6 <= s <= roof.slope_length + 1e-6, s)
            self.assertGreater(dz, -(roof.sag_max * 1.4 + margin))
            self.assertLess(dz, roof.lift_max + roof.curl_max + roof.tear_max + 0.05)

    def test_the_roof_is_not_a_plane_its_sheets_lift_curl_and_sag_by_centimetres(self):
        roof = make()[0]
        dzs = [to_roof(roof, p)[2] for p in roof.iron()[0]]
        self.assertGreater(max(dzs) - min(dzs), 0.15)

    def test_the_two_slopes_of_a_gable_rise_to_the_same_ridge(self):
        south, north = make()
        top = max(max(p[2] for p in roof.iron()[0]) for roof in (south, north))
        self.assertAlmostEqual(south.point(0.0, south.slope_length)[2], 5.5, places=9)
        self.assertAlmostEqual(north.point(0.0, north.slope_length)[2], 5.5, places=9)
        self.assertGreater(top, 5.5 - 0.2)

    def test_the_slopes_face_up_and_the_iron_is_a_closed_solid_with_a_thickness(self):
        for roof in make():
            self.assertGreater(g.signed_volume(*roof.iron()), 0.0)
            self.assertGreater(to_roof(roof, roof.point(1.0, 1.0, 1.0))[2], 0.99)
            self.assertGreater(roof.pose.rows[2][2], 0.9, "its normal points up")


class Holes(unittest.TestCase):
    def test_a_missing_sheet_leaves_a_gap_the_width_of_a_sheet(self):
        roof = make()[0]
        w = roof.width
        for p in roof.iron()[0]:
            a = to_roof(roof, p)[0]
            self.assertFalse(3 * w + OVERLAP + 1e-6 < a < 4 * w - 1e-6, f"a roof corner is at a = {a} in the gap of sheet 3")

    def test_a_skylight_has_a_straight_opening_in_the_iron_and_a_panel_in_it(self):
        roof = make()[0]
        for p in roof.iron()[0]:
            a, s, _ = to_roof(roof, p)
            for sk in roof.skylights:
                self.assertFalse(sk.a0 + 1e-3 < a < sk.a1 - 1e-3 and sk.s0 + 1e-3 < s < sk.s1 - 1e-3)
        glass = roof.glass()
        self.assertGreater(len(glass[1]), 0)

    def test_a_gone_skylight_has_no_panel_and_an_intact_one_has_a_whole_panel(self):
        roof = make()[0]
        areas = {}
        for state in ("gone", "intact", "broken"):
            only = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, skylights=[Skylight(6.0, 7.0, 2.5, 5.0, state)])
            areas[state] = odcut.area(only.glass())
        self.assertEqual(areas["gone"], 0.0)
        self.assertGreater(areas["intact"], 2 * 1.0 * 2.5 * 0.99)
        self.assertLess(areas["broken"], 0.7 * areas["intact"])
        self.assertGreater(areas["broken"], 0.0, "some glass is left round the hole")

    def test_the_broken_panels_hole_is_ragged_not_a_rectangle(self):
        roof = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, skylights=[Skylight(6.0, 7.5, 2.0, 5.0, "broken")])
        hole = roof._glass_hole(0, roof.skylights[0])
        # Walk each row of the panel from the middle out to the hole's left
        # edge: a rectangle would give the same place every time.
        lefts = []
        for k in range(1, 12):
            s = 2.0 + 3.0 * k / 12.0
            a = 6.75
            while hole(a, s) <= 0.0 and a > 6.0:
                a -= 0.005
            lefts.append(a)
        self.assertGreater(max(lefts) - min(lefts), 0.05)
        self.assertGreater(len({round(x, 2) for x in lefts}), 4)

    def test_a_tear_in_the_iron_is_a_ragged_hole_not_a_circle(self):
        roof = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, tears=[Tear(7.0, 3.0, 0.6)])
        radii = []
        for k in range(24):
            ang = 2 * math.pi * k / 24
            r = 0.0
            while roof._iron_hole(7.0 + r * math.cos(ang), 3.0 + r * math.sin(ang)) <= 0.0 and r < 2.0:
                r += 0.002
            radii.append(r)
        self.assertGreater(max(radii) / min(radii), 1.25)
        self.assertGreater(min(radii), 0.2)
        for p in roof.iron()[0]:
            a, s, _ = to_roof(roof, p)
            self.assertGreaterEqual(roof._iron_hole(a, s), -1e-6, "no iron corner lies inside the tear")


class Cost(unittest.TestCase):
    def test_every_triangle_of_the_roof_survives_the_builders_rounding(self):
        for roof in make():
            for mesh in (roof.iron(), roof.glass(), roof.collision_mesh()):
                if not mesh[0]:
                    continue
                cs, tris = snapped_triangles(mesh)
                for a, b, c in tris:
                    self.assertGreater(g.length(g.cross(g.sub(cs[b], cs[a]), g.sub(cs[c], cs[a]))), 0.0)

    def test_a_solid_sheet_costs_about_twice_an_open_one_and_both_stay_under_the_budget(self):
        solid = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4)
        open_sheet = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, thick=0.0)
        area = 15.0 * solid.slope_length
        solid_cost = odcut.triangle_count(solid.iron()) / area
        open_cost = odcut.triangle_count(open_sheet.iron()) / area
        # Observed at the default 0.45 m grid: about 12 for an open sheet and 25 for a solid one.
        self.assertLess(open_cost, 14.0)
        self.assertLess(solid_cost, 30.0)
        self.assertGreater(solid_cost, 1.7 * open_cost)

    def test_a_sheet_with_a_tear_is_cut_on_a_finer_grid_than_one_without(self):
        plain = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4)
        torn = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, tears=[Tear(7.0, 3.0, 0.6)])
        extra = odcut.triangle_count(torn.iron()) - odcut.triangle_count(plain.iron())
        # Observed: about 1100 triangles for each sheet the tear touches (this
        # one touches two), cut at 0.2 m instead of 0.45 m.
        self.assertGreater(extra, 1500)
        self.assertLess(extra, 3000)

    def test_a_skylight_costs_no_more_than_the_iron_it_replaces_and_its_panel(self):
        plain = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4)
        lit = Roof((0, 0, 4), 90.0, 15.0, 6.0, 1.5, 4, skylights=[Skylight(6.0, 7.0, 2.5, 5.0, "intact")])
        self.assertLess(odcut.triangle_count(lit.iron()) + odcut.triangle_count(lit.glass()),
                        odcut.triangle_count(plain.iron()) + 400)

    def test_the_collision_mesh_is_far_cheaper_than_the_drawn_roof(self):
        roof = make()[0]
        self.assertLess(odcut.triangle_count(roof.collision_mesh()) * 10, odcut.triangle_count(roof.iron()))

    def test_the_collision_mesh_leaves_a_missing_sheet_open(self):
        roof = make()[0]
        for p in roof.collision_mesh()[0]:
            a = to_roof(roof, p)[0]
            self.assertFalse(3 * roof.width + OVERLAP + 1e-6 < a < 4 * roof.width - 1e-6)


class Cutting(unittest.TestCase):
    def test_a_sheet_with_a_hole_cut_out_has_the_area_of_the_sheet_less_the_hole(self):
        # Rule: clipping along the zero of a distance function. A round hole
        # of radius 1 in a 10 m square, cut on a fine grid.
        hole = lambda x, y: math.hypot(x - 5.0, y - 5.0) - 1.0
        mesh = odcut.clip_grid(0.0, 0.0, 10.0, 10.0, 0.25, lambda x, y: 0.0, hole)
        self.assertAlmostEqual(odcut.area(mesh), 100.0 - math.pi, delta=0.05)

    def test_two_triangles_that_share_a_side_share_the_corner_the_cut_puts_on_it(self):
        hole = lambda x, y: math.hypot(x - 5.0, y - 5.0) - 1.3
        corners, faces = odcut.clip_grid(0.0, 0.0, 10.0, 10.0, 0.5, lambda x, y: 0.0, hole)
        self.assertEqual(len({(c[0], c[1]) for c in corners}), len(corners), "no corner is made twice")
        loops = odcut.edge_loops((corners, faces))
        outside = [e for e in loops if min(corners[e[0]][0], corners[e[1]][0], corners[e[0]][1], corners[e[1]][1]) > 0.1
                   and max(corners[e[0]][0], corners[e[1]][0], corners[e[0]][1], corners[e[1]][1]) < 9.9]
        for a, b in outside:
            for i in (a, b):
                d = math.hypot(corners[i][0] - 5.0, corners[i][1] - 5.0) - 1.3
                self.assertLess(abs(d), 1e-9, "an edge of the hole lies on the hole")

    def test_a_shell_is_a_closed_solid_the_thickness_it_was_given(self):
        mesh = odcut.clip_grid(0.0, 0.0, 4.0, 4.0, 1.0, lambda x, y: 0.0, lambda x, y: math.hypot(x - 2, y - 2) - 0.8)
        shell = odcut.shell(mesh, (0.0, 0.0, -0.05))
        self.assertAlmostEqual(g.signed_volume(*shell), odcut.area(mesh) * 0.05, delta=0.01)


if __name__ == "__main__":
    unittest.main()
