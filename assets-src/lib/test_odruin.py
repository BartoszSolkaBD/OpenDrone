"""Readable checks for the damage and plant helpers (`odruin.py`, `odplants.py`),
MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check runs). Each test's name is the sentence
it proves. Basis: Rule, from ADR-0024 and #295: a Map's shapes come from a
seed and are the same on every computer, and a solid the Pack checker reads
as one must be closed and face outwards.
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
import odplants  # noqa: E402
import odruin as r  # noqa: E402


def closed_and_outward(mesh):
    """Every edge of every face is met by exactly one face running it the
    other way (closed, consistently wound), and the volume is positive."""
    corners, faces = mesh
    tris = g.triangulate(corners, faces)
    edges = {}
    for a, b, c in tris:
        for e in ((a, b), (b, c), (c, a)):
            edges[e] = edges.get(e, 0) + 1
    closed = all(n == 1 and edges.get((e[1], e[0])) == 1 for e, n in edges.items())
    volume = sum(g.dot(corners[a], g.cross(corners[b], corners[c])) for a, b, c in tris) / 6.0
    return closed, volume


class Damage(unittest.TestCase):
    def test_the_same_seed_gives_the_same_numbers_and_another_seed_doesnt(self):
        a = [r.Random(7, "wall").unit() for _ in range(3)]
        b = [r.Random(7, "wall").unit() for _ in range(3)]
        c = [r.Random(8, "wall").unit() for _ in range(3)]
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertTrue(all(0.0 <= x < 1.0 for x in a))

    def test_a_plate_is_a_closed_solid_facing_out_whichever_way_its_outline_runs(self):
        outline = [(0.0, 0.0), (1.0, 0.0), (0.6, 0.3), (1.0, 0.8), (0.0, 0.7)]
        for o in (outline, list(reversed(outline))):
            closed, volume = closed_and_outward(r.plate((1.0, 2.0, 3.0), (1.0, 0.0, 0.0), (0.0, 0.6, 0.8), o, 0.02))
            self.assertTrue(closed)
            self.assertGreater(volume, 0.0)

    def test_a_breach_is_stepped_to_the_bricks_and_stays_inside_its_oval(self):
        rows = r.breach(10.0, 0.5, 2.8, 2.4, r.Random(1, "breach"))
        self.assertGreater(len(rows), 10)
        for u0, u1, v0, v1 in rows:
            self.assertAlmostEqual(v1 - v0, 2 * r.COURSE)
            self.assertGreater(u0, 10.0 - 1.4 - r.BRICK)
            self.assertLess(u1, 10.0 + 1.4 + r.BRICK)
            # Each end sits on a half-brick from the oval's edge.
            self.assertGreater(u1 - u0, r.BRICK)

    def test_missing_bricks_sit_on_the_brick_grid_and_never_overlap(self):
        pockets = r.missing_bricks(0.0, 6.0, 0.0, 3.0, 12, r.Random(3, "pockets"))
        self.assertEqual(len(pockets), 12)
        for a, b, v0, v1 in pockets:
            self.assertAlmostEqual(v1 - v0, r.COURSE)
            row = round(v0 / r.COURSE)
            shift = r.BRICK / 2.0 if row % 2 else 0.0
            self.assertAlmostEqual(((a - shift) / r.BRICK) % 1.0 * r.BRICK % r.BRICK, 0.0, places=6)
        for i, p in enumerate(pockets):
            for q in pockets[i + 1:]:
                self.assertFalse(p[0] < q[1] - 1e-9 and q[0] < p[1] - 1e-9 and p[2] < q[3] - 1e-9 and q[2] < p[3] - 1e-9)

    def test_rubble_lumps_and_bricks_are_closed_solids(self):
        rnd = r.Random(5, "rubble")
        for _ in range(20):
            for mesh in (r.lump(0.4, rnd), r.brick(rnd)):
                closed, volume = closed_and_outward(mesh)
                self.assertTrue(closed)
                self.assertGreater(volume, 0.0)

    def test_a_mound_sinks_under_the_floor_at_its_rim_and_rises_in_the_middle(self):
        h = r.mound(0.0, 0.0, 3.0, 2.0, 0.8, r.Random(2, "mound"))
        self.assertLess(h(3.0, 0.0), 0.0)
        self.assertLess(h(0.0, -2.0), 0.0)
        self.assertGreater(h(0.0, 0.0), 0.5)

    def test_a_bent_bar_is_made_of_closed_boxes(self):
        bar = r.bent_bar([(0, 0, 0), (2, 0, 0.3), (3, 0.5, 1.2)], 0.1, 0.15)
        self.assertEqual(len(bar.faces), 12)
        closed, volume = closed_and_outward(bar.mesh())
        self.assertTrue(closed)
        self.assertGreater(volume, 0.0)


class Plants(unittest.TestCase):
    def test_a_tree_is_the_same_from_the_same_seed_and_has_wood_and_hundreds_of_leaves(self):
        wood, leaves = odplants.tree(r.Random(4, "tree"), (0.0, 0.0, 0.0), 7.0, leaves=600)
        again, _ = odplants.tree(r.Random(4, "tree"), (0.0, 0.0, 0.0), 7.0, leaves=600)
        self.assertEqual(wood.corners, again.corners)
        self.assertGreaterEqual(len(leaves.faces) // 2, 500)
        top = max(c[2] for c in wood.corners + leaves.corners)
        self.assertGreater(top, 6.0)
        self.assertLess(top, 9.5)
        closed, volume = closed_and_outward(wood.mesh())
        self.assertTrue(closed)
        self.assertGreater(volume, 0.0)

    def test_a_leaf_is_drawn_from_both_sides(self):
        corners, faces = odplants.leaf(0.2, 0.1)
        front, back = faces
        n_front = g.cross(g.sub(corners[front[1]], corners[front[0]]), g.sub(corners[front[2]], corners[front[0]]))
        n_back = g.cross(g.sub(corners[back[1]], corners[back[0]]), g.sub(corners[back[2]], corners[back[0]]))
        self.assertGreater(n_front[2], 0.0)
        self.assertLess(n_back[2], 0.0)

    def test_a_clump_of_weeds_is_no_taller_than_asked_and_the_same_from_the_same_seed(self):
        clump = odplants.weeds(r.Random(9, "weeds"), (1.0, 1.0), 0.3, 20, 0.6)
        again = odplants.weeds(r.Random(9, "weeds"), (1.0, 1.0), 0.3, 20, 0.6)
        self.assertEqual(clump.corners, again.corners)
        self.assertGreater(len(clump.faces), 20 * 2 * 5)
        self.assertLessEqual(max(c[2] for c in clump.corners), 0.6 * 1.05)
        self.assertGreater(max(c[2] for c in clump.corners), 0.3)


if __name__ == "__main__":
    unittest.main()
