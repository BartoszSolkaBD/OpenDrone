"""Readable checks for wandering material boundaries (`odedge.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`). Each test's name is the sentence it proves. Basis: Rule,
from #322: grass meets dirt along a line that wanders, not a razor line; the
weights are the same on every computer, and a whole number of 255ths, so they
survive being written as 8-bit vertex colours. The decals ticket (#321) adds
the vertex-colour blend that draws them, so the shape is checked here by
numbers alone.
"""
import hashlib
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from odedge import WanderingEdge  # noqa: E402
from odground import Ground  # noqa: E402

LINE = [(-30.0, 0.0), (30.0, 0.0)]  # B (dirt) on the left of the way it runs: the north side


def edge(seed=5, **kw):
    args = dict(wander=1.4, band=0.6)
    args.update(kw)
    return WanderingEdge(LINE, seed, **args)


def contour(e, xs):
    """Where the weight crosses a half, going north from the south, at each x."""
    return [e.crossing(x, -4.0, 4.0) for x in xs]


class Repeatable(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_weights_every_time(self):
        mesh = g.heightfield_mesh(-10, -5, 10, 5, 0.25, lambda x, y: 0.0)
        self.assertEqual(edge(5).weights(mesh[0]), edge(5).weights(mesh[0]))

    def test_another_seed_gives_another_edge(self):
        xs = [x * 0.7 for x in range(-30, 30)]
        self.assertNotEqual(contour(edge(5), xs), contour(edge(6), xs))

    def test_the_weights_are_the_same_on_every_computer(self):
        # Rule: the noise is only whole-number hashing and + - * / on 64-bit
        # numbers. Observed: this digest, made on the Mac that wrote the test.
        mesh = g.heightfield_mesh(-10, -5, 10, 5, 0.25, lambda x, y: 0.0)
        self.assertEqual(hashlib.sha256(repr(edge(5).weights(mesh[0])).encode()).hexdigest(), "87c5b842c1bc23efd81930eaa22d2bee1465eb0617013a51a4652cc5a917262d")


class Shape(unittest.TestCase):
    def test_every_weight_is_from_0_to_1_and_a_whole_number_of_255ths(self):
        mesh = g.heightfield_mesh(-30, -10, 30, 10, 0.5, lambda x, y: 0.0)
        for w in edge().weights(mesh[0]):
            self.assertTrue(0.0 <= w <= 1.0)
            self.assertAlmostEqual(w * 255.0, round(w * 255.0), places=9)

    def test_it_is_all_material_a_far_on_the_right_and_all_material_b_far_on_the_left(self):
        e = edge()
        for x in range(-25, 26, 5):
            self.assertEqual(e.weight(x, -6.0), 0.0)
            self.assertEqual(e.weight(x, 6.0), 1.0)

    def test_the_edge_stays_within_its_wander_and_band_of_the_line(self):
        e = edge(wander=1.4, band=0.6)
        xs = [x * 0.5 for x in range(-58, 58)]
        for y in contour(e, xs):
            self.assertLessEqual(abs(y), 1.4 + 0.6 + 0.2)

    def test_the_edge_wanders_it_is_not_a_straight_line(self):
        ys = contour(edge(), [x * 0.5 for x in range(-58, 58)])
        mean = sum(ys) / len(ys)
        spread = (sum((y - mean) ** 2 for y in ys) / len(ys)) ** 0.5
        self.assertGreater(spread, 0.3, "it strays by tens of centimetres")
        self.assertGreater(max(ys) - min(ys), 0.8)

    def test_no_stretch_of_the_edge_is_straight_over_four_metres(self):
        # A straight stretch has a gradient that does not change. Over every
        # 4 m window the edge leaves the straight line between its ends by
        # at least 5 cm somewhere.
        xs = [x * 0.25 for x in range(-110, 111)]
        ys = contour(edge(), xs)
        window = 16  # 16 steps of 0.25 m
        for i in range(0, len(xs) - window, 8):
            y0, y1 = ys[i], ys[i + window]
            worst = max(abs(ys[i + k] - (y0 + (y1 - y0) * k / window)) for k in range(window + 1))
            self.assertGreater(worst, 0.05, f"the edge is straight to within 5 cm from x = {xs[i]}")

    def test_the_fade_across_the_edge_is_as_wide_as_the_band_asked_for(self):
        e = edge(wander=0.0, band=0.8, patches=0.0)
        # With nothing bending it, the 5 % and 95 % points are about the band apart.
        lo = e.crossing(3.0, -4.0, 4.0, level=0.05)
        hi = e.crossing(3.0, -4.0, 4.0, level=0.95)
        self.assertAlmostEqual(hi - lo, 0.8 * 0.68, delta=0.12)

    def test_the_weight_grows_across_the_band_from_a_to_b_without_going_back_far(self):
        e = edge(patches=0.0)
        ws = [e.weight(2.0, y * 0.05) for y in range(-100, 101)]
        for a, b in zip(ws, ws[1:]):
            self.assertGreaterEqual(b, a - 0.2)

    def test_patches_break_the_fade_into_a_ragged_one_that_a_clean_ramp_is_not(self):
        smooth, torn = edge(patches=0.0), edge(patches=1.0)
        xs = [x * 0.1 for x in range(-100, 100)]
        # Along one line through the band, the torn edge's weight is less
        # like a ramp in x: it changes direction more often.
        def reversals(e):
            n = 0
            ws = [e.weight(x, 0.0) for x in xs]
            for a, b, c in zip(ws, ws[1:], ws[2:]):
                if (b - a) * (c - b) < 0:
                    n += 1
            return n
        self.assertGreater(reversals(torn), reversals(smooth) - 1)


class OnAMesh(unittest.TestCase):
    def test_a_ground_mesh_gets_one_weight_for_every_corner_in_the_corners_order(self):
        ground = Ground(3, amplitude=0.05)
        corners, faces = ground.mesh((-10, -6, 10, 6), 0.25)
        w = edge().weights(corners)
        self.assertEqual(len(w), len(corners))
        for c, v in zip(corners[:50], w[:50]):
            self.assertEqual(v, edge().weight(c[0], c[1]))

    def test_a_mesh_with_corners_a_little_under_half_the_band_apart_fades_in_about_three_steps(self):
        # The fade is steepest in the middle: 1.5 / band a metre, so corners
        # 0.25 m apart across a 0.6 m band step by about 0.6, at most 0.8
        # where the bending is against the fade. That is as coarse as a mesh
        # may be for the band to show as a fade and not a jump.
        corners, _ = g.heightfield_mesh(-10, -6, 10, 6, 0.25, lambda x, y: 0.0)
        e = edge(patches=0.0)
        w = {(c[0], c[1]): e.weight(c[0], c[1]) for c in corners}
        worst = max(abs(w[(x, y)] - w[(x, y + 0.25)]) for (x, y) in w if (x, y + 0.25) in w)
        self.assertLess(worst, 0.8)
        self.assertGreater(worst, 0.4)


if __name__ == "__main__":
    unittest.main()
