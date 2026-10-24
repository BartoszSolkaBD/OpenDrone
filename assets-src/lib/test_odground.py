"""Readable checks for uneven ground (`odground.py`, `odnoise.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check does too). Each test's name is the
sentence it proves. Basis: Rule, from #322 and docs/building-a-map.md: a Map's
`.glb` is the same bytes on every computer, the ground rolls and dips but a
3 m Launch Spot stays level, and nothing is left a flat plane.
"""
import hashlib
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from odground import Dip, Flat, FlatRect, Ground, Mound, water_dips  # noqa: E402
from odnoise import Random, noise2  # noqa: E402

YARD = (-30.0, -30.0, 30.0, 30.0)
SMALL = (-14.0, -14.0, 14.0, 4.0)  # round the heap against the wall at y = -7.5


def yard(seed=3, **kw):
    return Ground(
        seed, amplitude=0.10, slope=(0.01, 0.0), flats=[Flat(0.0, -17.0, 3.5)],
        mounds=[Mound((-12.0, -7.5), (12.0, -7.5), 0.45, 1.6, spills=[(8.0, 2.0, 0.5)])],
        dips=water_dips(Random(seed, "dips"), YARD, 6), **kw)


def digest(mesh):
    """A fingerprint of a mesh as the builder would write it: every corner
    rounded to 0.1 mm and then to the 32-bit number the file stores."""
    corners, faces = mesh
    text = repr([g.snap3(c) for c in corners]) + repr(faces)
    return hashlib.sha256(text.encode()).hexdigest()


class Repeatable(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_ground_every_time(self):
        self.assertEqual(digest(yard().mesh(YARD, 1.0)), digest(yard().mesh(YARD, 1.0)))

    def test_another_seed_gives_other_ground(self):
        self.assertNotEqual(digest(yard(3).mesh(YARD, 1.0)), digest(yard(4).mesh(YARD, 1.0)))

    def test_the_noise_gives_the_same_numbers_on_every_computer(self):
        # Rule: only whole-number hashing and + - * / on 64-bit numbers, so
        # this digest, made on the Mac that wrote the test, holds on Linux
        # and Windows too. If it changes, every Map built on the noise moves.
        values = [noise2(5, x * 0.37, y * 0.53) for x in range(-20, 20) for y in range(-20, 20)]
        self.assertEqual(
            hashlib.sha256(repr(values).encode()).hexdigest(),
            "a855ff1c33d6da71f8e2b0f223ad7b089a34405ce28be6cbfe2a4a75fcb5842f",
        )

    def test_a_ground_mesh_has_the_very_same_bytes_every_time_it_is_made(self):
        # Observed: the digest of the finished yard on the Mac that wrote the
        # test; CI's Linux run proves the same on its computer.
        self.assertEqual(digest(yard().mesh(YARD, 1.0)), "dbb179c7d9bed6b222346d07233b51586ee30d6e958004d485de4b2a29158165")


class Shape(unittest.TestCase):
    def test_rolling_ground_stays_within_its_amplitude_of_the_plane(self):
        ground = Ground(7, amplitude=0.10)
        heights = [ground.height(x * 1.3, y * 1.7) for x in range(-30, 30) for y in range(-30, 30)]
        self.assertLessEqual(max(heights), 0.10)
        self.assertGreaterEqual(min(heights), -0.10)

    def test_ground_is_not_a_flat_plane_it_rolls_by_a_few_centimetres(self):
        ground = Ground(7, amplitude=0.10)
        heights = [ground.height(x, y) for x in range(-30, 30) for y in range(-30, 30)]
        self.assertGreater(max(heights) - min(heights), 0.08)

    def test_a_concrete_floor_rolls_much_less_than_a_yard(self):
        yard_ground, floor = Ground(7, amplitude=0.10), Ground(7, amplitude=0.01, wavelength=6.0)
        spread = lambda gr: max(gr.height(x, y) for x in range(30) for y in range(30)) - min(gr.height(x, y) for x in range(30) for y in range(30))
        self.assertLess(spread(floor), spread(yard_ground) / 5.0)

    def test_a_slope_rises_by_its_gradient_across_the_map(self):
        ground = Ground(7, amplitude=0.0, slope=(0.02, 0.0), base=1.0)
        self.assertAlmostEqual(ground.height(50.0, 3.0) - ground.height(0.0, 3.0), 1.0, places=9)

    def test_a_dip_is_lowest_in_the_middle_and_gone_at_its_rim(self):
        ground = Ground(7, amplitude=0.0, dips=[Dip(5.0, 5.0, 2.0, 0.06)])
        self.assertAlmostEqual(ground.height(5.0, 5.0), -0.06, places=9)
        self.assertEqual(ground.height(7.0, 5.0), 0.0)
        self.assertLess(ground.height(5.5, 5.0), ground.height(6.5, 5.0))

    def test_dips_land_inside_the_rectangle_and_out_of_the_keep_out_circles(self):
        dips = water_dips(Random(1, "d"), YARD, 8, keep_out=[(0.0, -17.0, 6.0)])
        self.assertEqual(len(dips), 8)
        for d in dips:
            self.assertTrue(YARD[0] <= d.x - d.radius and d.x + d.radius <= YARD[2])
            self.assertTrue(YARD[1] <= d.y - d.radius and d.y + d.radius <= YARD[3])
            self.assertGreater(((d.x - 0.0) ** 2 + (d.y + 17.0) ** 2) ** 0.5, 6.0 + d.radius)

    def test_rubble_heaps_against_a_wall_and_falls_away_from_it(self):
        ground = Ground(7, amplitude=0.0, mounds=[Mound((0.0, 0.0), (10.0, 0.0), 0.5, 1.5, side=1)])
        self.assertGreater(ground.height(5.0, 0.1), 0.15)
        self.assertGreater(ground.height(5.0, 0.1), ground.height(5.0, 1.0))
        self.assertEqual(ground.height(5.0, 1.6), 0.0)
        self.assertEqual(ground.height(5.0, -0.5), 0.0, "side=1 is the left of the way the wall runs")

    def test_rubble_is_higher_where_something_fell_out_of_the_wall(self):
        plain = Ground(7, amplitude=0.0, mounds=[Mound((0.0, 0.0), (10.0, 0.0), 0.5, 1.5, seed=1)])
        spilled = Ground(7, amplitude=0.0, mounds=[Mound((0.0, 0.0), (10.0, 0.0), 0.5, 1.5, spills=[(5.0, 1.5, 0.6)], seed=1)])
        # at 0.1 m from the wall a spill's heap is 0.6 m times (1 - 0.1 / 3) squared
        self.assertAlmostEqual(spilled.height(5.0, 0.1) - plain.height(5.0, 0.1), 0.6 * (1 - 0.1 / 3.0) ** 2, places=9)
        self.assertEqual(spilled.height(9.0, 0.1), plain.height(9.0, 0.1))


class LevelPatches(unittest.TestCase):
    def test_the_launch_spots_3_m_stays_level_to_a_tenth_of_a_millimetre_in_the_finished_mesh(self):
        ground = yard()
        corners, _ = ground.mesh(YARD, 0.5)
        level = ground.levels[0]
        near = [g.snap3(c) for c in corners if ((c[0] - 0.0) ** 2 + (c[1] + 17.0) ** 2) ** 0.5 <= 3.5]
        self.assertGreater(len(near), 100)
        self.assertEqual({round(c[2] * 1e4) for c in near}, {round(g.snap(level) * 1e4)})

    def test_the_level_patch_is_level_to_the_edge_of_its_radius_and_eases_out_over_its_blend(self):
        ground = yard()
        level = ground.levels[0]
        self.assertAlmostEqual(ground.height(3.5, -17.0), level, places=12)
        self.assertAlmostEqual(ground.height(-3.5, -17.0), level, places=12)
        far = max(abs(ground.height(0.0 + 3.5 + 1.5 + k, -17.0) - level) for k in (0.0, 1.0, 2.0))
        self.assertGreater(far, 0.0)

    def test_a_level_patch_keeps_the_given_level_whatever_the_ground_does(self):
        ground = Ground(7, amplitude=0.2, flats=[FlatRect(-2.0, -2.0, 2.0, 2.0, level=1.25)])
        for x in (-2.0, -1.0, 0.0, 2.0):
            for y in (-2.0, 0.0, 2.0):
                self.assertEqual(ground.height(x, y), 1.25)
        self.assertNotEqual(ground.height(6.0, 6.0), 1.25)

    def test_a_level_patch_with_no_level_is_level_at_the_grounds_own_height_there(self):
        ground = Ground(7, amplitude=0.1, slope=(0.0, 0.05), flats=[Flat(0.0, 10.0, 3.0)])
        self.assertAlmostEqual(ground.levels[0], ground.rolling(0.0, 10.0), places=12)


class Cost(unittest.TestCase):
    def test_the_triangles_a_square_metre_are_two_over_the_step_squared(self):
        ground = yard()
        for step, want in ((0.5, 8.0), (1.0, 2.0)):
            corners, faces = ground.mesh(YARD, step)
            self.assertAlmostEqual(len(faces) / (60.0 * 60.0), want, places=6)

    def test_the_collision_mesh_of_rolling_ground_stays_within_a_few_centimetres_of_the_drawn_ground(self):
        # Observed: rolling yard ground of 0.10 m, drawn at 0.5 m, collision at 2 m.
        self.assertLess(Ground(3, amplitude=0.10).max_gap(SMALL, 0.5, 2.0), 0.06)

    def test_a_finer_collision_mesh_follows_a_heap_of_rubble_more_closely(self):
        ground = yard()
        self.assertLess(ground.max_gap(SMALL, 0.5, 1.0), ground.max_gap(SMALL, 0.5, 2.0))

    def test_every_triangle_of_the_ground_survives_the_builders_rounding(self):
        corners, faces = yard().mesh(YARD, 0.5)
        cs = [g.snap3(c) for c in corners]
        for a, b, c in faces:
            self.assertGreater(g.length(g.cross(g.sub(cs[b], cs[a]), g.sub(cs[c], cs[a]))), 0.0)


if __name__ == "__main__":
    unittest.main()
