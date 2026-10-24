"""Readable checks for brick walls with broken openings (`odopening.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`). Each test's name is the sentence it proves. Basis: Rule,
from #322 and docs/building-a-map.md: the same bytes on every computer, a
closed mesh the builder can take (every triangle keeps an area after the
corners are rounded to 0.1 mm), shapes that stay inside their bounds, and no
opening left a perfect rectangle.
"""
import hashlib
import os
import sys
import unittest
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from odnoise import Random  # noqa: E402
from odopening import BRICK, COURSE, HALF, Opening, broken_wall, open_edges, triangles_per_square_metre  # noqa: E402

RUN, HEIGHT, THICK = 12.0, 4.5, 0.34


def openings():
    return [
        Opening(3.0, 4.6, 1.1, 2.9, "window", sill_gone=True),
        Opening(8.0, 10.0, 0.0, 2.4, "doorway"),
        Opening(6.0, 7.5, 0.6, 2.2, "breach"),
    ]


def wall(seed=2, **kw):
    args = dict(bulge=0.03, lean=0.05, ledge=0.0)
    args.update(kw)
    return broken_wall(RUN, HEIGHT, THICK, seed, openings=openings(), **args)


def digest(mesh):
    corners, faces = mesh
    return hashlib.sha256((repr([g.snap3(c) for c in corners]) + repr(faces)).encode()).hexdigest()


def snapped(mesh):
    corners, faces = mesh
    cs = [g.snap3(c) for c in corners]
    return cs, g.triangulate(cs, faces)


class Repeatable(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_wall_every_time(self):
        self.assertEqual(digest(wall(2).mesh), digest(wall(2).mesh))
        self.assertEqual(wall(2).collision_openings(), wall(2).collision_openings())

    def test_another_seed_gives_another_wall(self):
        self.assertNotEqual(digest(wall(2).mesh), digest(wall(3).mesh))

    def test_the_wall_has_the_very_same_bytes_on_every_computer(self):
        # Observed: the digest on the Mac that wrote the test; CI's Linux
        # run proves the same on its computer.
        self.assertEqual(digest(wall(2).mesh), "f52de66a297574c252e32aee25b06aec4cc231b4404b0f85a1cbab5d77819246")

    def test_the_loose_bricks_are_the_very_same_every_time(self):
        a = wall().loose_bricks(Random(2, "bricks"))
        b = wall().loose_bricks(Random(2, "bricks"))
        self.assertEqual([digest(m) for m in a], [digest(m) for m in b])


class Openings(unittest.TestCase):
    def test_no_opening_is_left_a_perfect_rectangle_whatever_the_seed(self):
        for kind, v0, v1 in (("window", 1.1, 2.9), ("doorway", 0.0, 2.4), ("breach", 0.6, 2.2)):
            for seed in range(25):
                op = Opening(3.0, 5.0, v0, v1, kind)
                w = broken_wall(RUN, HEIGHT, THICK, seed, openings=[op], bulge=0.0)
                lefts, rights = [e[0] for e in op.edges], [e[1] for e in op.edges]
                self.assertGreaterEqual(len(set(lefts)), 3, (kind, seed))
                self.assertGreaterEqual(len(set(rights)), 3, (kind, seed))
                box = (max(rights) - min(lefts)) * len(op.edges) * COURSE
                self.assertLess(w.hole_area(0), 0.97 * box, (kind, seed, "the hole fills its bounding box"))

    def test_every_edge_of_an_opening_is_stepped_to_a_half_brick_from_where_it_began(self):
        for seed in range(10):
            op = Opening(3.0, 5.0, 1.0, 2.8, "window")
            broken_wall(RUN, HEIGHT, THICK, seed, openings=[op])
            for l, r in op.edges:
                self.assertAlmostEqual((l - 3.0) / HALF, round((l - 3.0) / HALF), delta=1e-5)
                self.assertAlmostEqual((r - 5.0) / HALF, round((r - 5.0) / HALF), delta=1e-5)

    def test_a_window_with_its_sill_gone_reaches_below_where_it_began_and_narrows_going_down(self):
        op = Opening(3.0, 5.0, 1.0, 2.8, "window", sill_gone=True, drop=3)
        broken_wall(RUN, HEIGHT, THICK, 5, openings=[op])
        self.assertEqual(op.first_course, round(1.0 / COURSE) - 3)
        widths = [r - l for l, r in op.edges]
        self.assertLess(widths[0], widths[4], "the lowest course is narrower than one above the old sill")

    def test_a_breach_is_rougher_than_a_window_it_fills_less_of_its_bounding_box(self):
        def roughness(kind):
            total = 0.0
            for seed in range(20):
                op = Opening(3.0, 5.0, 1.0, 2.8, kind)
                w = broken_wall(RUN, HEIGHT, THICK, seed, openings=[op])
                box = (max(e[1] for e in op.edges) - min(e[0] for e in op.edges)) * len(op.edges) * COURSE
                total += 1.0 - w.hole_area(0) / box
            return total / 20.0
        self.assertGreater(roughness("breach"), roughness("window"))

    def test_the_drawn_face_has_the_area_of_the_wall_less_the_holes(self):
        w = wall(bulge=0.0, lean=0.0)
        corners, faces = w.skin
        face = 0.0
        for a, b, c in faces:
            n = g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a]))
            face += n[1] / 2.0
        holes = sum(w.hole_area(k) for k in range(3))
        self.assertAlmostEqual(face, RUN * HEIGHT - holes, delta=1e-6)

    def test_the_wall_has_a_stepped_edge_where_each_opening_is(self):
        # The skin's open edges are the wall's rim and each opening's edge:
        # more than the four sides' share, in steps along every opening.
        w = wall(bulge=0.0, lean=0.0)
        corners, _ = w.skin
        inside = [(a, b) for a, b in open_edges(w) if 0.1 < corners[a][0] < RUN - 0.1 and 0.1 < corners[a][2] < HEIGHT - 0.1]
        self.assertGreater(len(inside), 3 * 20, "each opening has many steps")

    def test_collision_openings_lie_inside_the_drawn_hole_course_by_course(self):
        for kind, v0, v1 in (("window", 1.1, 2.9), ("doorway", 0.0, 2.4), ("breach", 0.6, 2.2)):
            for seed in range(10):
                op = Opening(4.0, 6.0, v0, v1, kind, sill_gone=kind == "window")
                w = broken_wall(RUN, HEIGHT, THICK, seed, openings=[op])
                found = w.collision_openings()
                self.assertTrue(found, (kind, seed))
                for u0, u1, a, b in found:
                    self.assertLess(u0, u1)
                    for k, (l, r) in enumerate(op.edges):
                        j = op.first_course + k
                        if a - 1e-9 <= j * COURSE and (j + 1) * COURSE <= b + 1e-9:
                            self.assertLessEqual(l, u0 + 1e-9, (kind, seed))
                            self.assertGreaterEqual(r, u1 - 1e-9, (kind, seed))

    def test_openings_outside_the_wall_or_too_near_its_ends_are_refused(self):
        with self.assertRaises(AssertionError):
            broken_wall(RUN, HEIGHT, THICK, 1, openings=[Opening(0.2, 2.0, 0.0, 2.0, "doorway")])
        with self.assertRaises(AssertionError):
            broken_wall(RUN, HEIGHT, THICK, 1, openings=[Opening(3.0, 5.0, 3.0, 9.0, "window")])


class Shape(unittest.TestCase):
    def test_the_wall_is_a_closed_solid_with_every_edge_shared_by_two_triangles(self):
        for ledge in (0.0, 0.004):
            for seed in (1, 2, 3, 4):
                w = wall(seed, ledge=ledge)
                cs, tris = snapped(w.mesh)
                edges = Counter()
                for t in tris:
                    for k in range(3):
                        edges[(t[k], t[(k + 1) % 3])] += 1
                for (a, b), n in edges.items():
                    self.assertEqual(n, 1, (ledge, seed, "an edge is run twice the same way"))
                    self.assertEqual(edges.get((b, a), 0), 1, (ledge, seed, "an edge has no partner"))
                self.assertGreater(g.signed_volume(*w.mesh), 0.0)

    def test_any_seed_gives_a_closed_solid_the_builder_can_take(self):
        for seed in range(15):
            w = wall(seed, ledge=0.003)
            cs, tris = snapped(w.mesh)
            edges = Counter()
            for t in tris:
                for k in range(3):
                    edges[(t[k], t[(k + 1) % 3])] += 1
            self.assertTrue(all(n == 1 and edges.get((b, a), 0) == 1 for (a, b), n in edges.items()), seed)
            for a, b, c in tris:
                self.assertGreater(g.length(g.cross(g.sub(cs[b], cs[a]), g.sub(cs[c], cs[a]))), 0.0, seed)

    def test_the_volume_is_the_wall_less_its_holes_times_its_thickness(self):
        w = wall(bulge=0.0, lean=0.0)
        holes = sum(w.hole_area(k) for k in range(3))
        self.assertAlmostEqual(g.signed_volume(*w.mesh), (RUN * HEIGHT - holes) * THICK, delta=0.02)

    def test_every_triangle_of_the_wall_survives_the_builders_rounding(self):
        for ledge in (0.0, 0.004):
            for seed in (1, 2, 3):
                cs, tris = snapped(wall(seed, ledge=ledge).mesh)
                for a, b, c in tris:
                    self.assertGreater(g.length(g.cross(g.sub(cs[b], cs[a]), g.sub(cs[c], cs[a]))), 0.0, (ledge, seed))

    def test_every_corner_stays_inside_the_walls_bounds(self):
        w = wall(ledge=0.004, bulge=0.03, lean=0.05)
        slack = 0.03 + 0.05 + 0.004 + 1e-9
        for u, y, z in w.mesh[0]:
            self.assertTrue(0.0 <= u <= RUN)
            self.assertTrue(0.0 <= z <= HEIGHT + 1e-9)
            self.assertTrue(-THICK / 2.0 - slack <= y <= THICK / 2.0 + slack)

    def test_a_wall_with_no_bulge_and_no_lean_has_a_perfectly_flat_face(self):
        w = wall(bulge=0.0, lean=0.0)
        ys = {round(c[1], 9) for c in w.skin[0]}
        self.assertEqual(ys, {THICK / 2.0})

    def test_the_face_bulges_by_centimetres_and_never_by_more_than_asked(self):
        w = wall(bulge=0.03, lean=0.0)
        ys = [c[1] - THICK / 2.0 for c in w.skin[0]]
        self.assertGreater(max(ys) - min(ys), 0.02)
        self.assertLessEqual(max(abs(y) for y in ys), 0.03 + 1e-9)

    def test_the_top_of_a_leaning_wall_is_the_lean_further_out_than_its_foot(self):
        w = wall(bulge=0.0, lean=0.06)
        top = max(c[1] for c in w.skin[0] if c[2] > HEIGHT - 1e-9)
        foot = min(c[1] for c in w.skin[0] if c[2] < 1e-9)
        self.assertAlmostEqual(top - foot, 0.06, places=9)

    def test_courses_sit_proud_or_back_by_no_more_than_the_ledge(self):
        w = wall(bulge=0.0, lean=0.0, ledge=0.004)
        ys = {round(c[1] - THICK / 2.0, 6) for c in w.skin[0]}
        self.assertGreater(len(ys), 3)
        self.assertLessEqual(max(abs(y) for y in ys), 0.004 + 1e-9)

    def test_the_pose_puts_the_front_on_the_left_of_the_way_from_p0_to_p1(self):
        pose = wall().pose((0.0, 0.0), (12.0, 0.0), 1.0)
        east = g.sub(pose.apply((1.0, 0.0, 0.0)), pose.apply((0.0, 0.0, 0.0)))
        front = g.sub(pose.apply((0.0, 1.0, 0.0)), pose.apply((0.0, 0.0, 0.0)))
        self.assertAlmostEqual(east[0], 1.0, places=9)
        self.assertAlmostEqual(front[1], 1.0, places=9)
        self.assertAlmostEqual(pose.apply((0.0, 0.0, 0.0))[2], 1.0, places=9)


class Cost(unittest.TestCase):
    def test_a_wall_with_three_openings_stays_under_the_triangle_budget(self):
        # Observed: 2316 triangles for this 12 x 4.5 m wall (43 a square metre
        # of face) and 2676 with 4 mm ledges (50).
        self.assertLess(triangles_per_square_metre(wall()), 50.0)
        self.assertLess(triangles_per_square_metre(wall(ledge=0.004)), 60.0)

    def test_ledges_cost_triangles_and_openings_cost_more_than_plain_wall(self):
        plain = broken_wall(RUN, HEIGHT, THICK, 2)
        self.assertLess(triangles_per_square_metre(plain), 10.0)
        self.assertGreater(triangles_per_square_metre(wall(ledge=0.004)), triangles_per_square_metre(wall()))
        self.assertGreater(triangles_per_square_metre(wall()), triangles_per_square_metre(plain))


class LooseBricks(unittest.TestCase):
    def test_there_are_bricks_at_the_foot_of_every_opening_and_each_is_brick_sized(self):
        w = wall()
        meshes = w.loose_bricks(Random(2, "bricks"))
        self.assertEqual(len(meshes), 3)
        for k, (corners, faces) in enumerate(meshes):
            self.assertGreaterEqual(len(corners) // 8, 3)
            op = w.openings[k]
            lo = min(l for l, _ in op.edges) - 0.3 - BRICK
            hi = max(r for _, r in op.edges) + 0.3 + BRICK
            for u, y, z in corners:
                self.assertTrue(lo <= u <= hi)
                self.assertGreater(z, -0.15)
                self.assertLess(z, 3.0)

    def test_bricks_lie_on_the_ground_the_script_gives(self):
        meshes = wall().loose_bricks(Random(2, "bricks"), floor=lambda u, y: 0.5)
        lowest = min(c[2] for mesh in meshes for c in mesh[0] if c[2] < 1.0)
        self.assertGreater(lowest, 0.5 - 0.09)

    def test_every_brick_triangle_survives_the_builders_rounding(self):
        for mesh in wall().loose_bricks(Random(2, "bricks")):
            cs, tris = snapped(mesh)
            for a, b, c in tris:
                self.assertGreater(g.length(g.cross(g.sub(cs[b], cs[a]), g.sub(cs[c], cs[a]))), 0.0)


if __name__ == "__main__":
    unittest.main()
