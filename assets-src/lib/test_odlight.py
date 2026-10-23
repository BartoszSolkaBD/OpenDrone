"""Readable checks for the light atlas's unwrap (`odlight.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check does too). Each test's name is the
sentence it proves. Basis: Rule, from #293 and docs/building-a-map.md: the
second UV set must be the same bytes on every computer, every face gets a
margin, and the texel density is the Map's to choose.
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
import odlight  # noqa: E402


def box(sx, sy, sz, at=(0.0, 0.0, 0.0)):
    """A box as `(corners, triangles, cm)` the way `unwrap` takes a mesh."""
    corners, faces = g.box_mesh(sx, sy, sz)
    corners = [(x + at[0], y + at[1], z + at[2]) for x, y, z in corners]
    return corners, g.triangulate(corners, faces), None


def plane(size, z=0.0):
    corners = [(0.0, 0.0, z), (size, 0.0, z), (size, size, z), (0.0, size, z)]
    return corners, [(0, 1, 2), (0, 2, 3)], None


class Unwrap(unittest.TestCase):
    def test_a_box_is_six_charts_and_a_flat_sheet_is_one(self):
        self.assertEqual(len(odlight.charts_of(*box(1, 2, 3)[:2])), 6)
        self.assertEqual(len(odlight.charts_of(*plane(4.0)[:2])), 1)

    def test_the_same_meshes_give_the_very_same_picture_coordinates_every_time(self):
        meshes = [box(2, 3, 1), box(0.5, 0.5, 4, (3, 0, 0)), plane(10.0)]
        first = odlight.unwrap(meshes, 20.0)
        second = odlight.unwrap(list(meshes), 20.0)
        self.assertEqual(first, second)

    def test_every_picture_coordinate_is_inside_the_atlas(self):
        uvs, _, _ = odlight.unwrap([box(2, 3, 1), plane(10.0)], 20.0)
        for mesh in uvs:
            for value in mesh:
                self.assertGreaterEqual(value, 0.0)
                self.assertLessEqual(value, 1.0)

    def test_a_metre_square_at_10_cm_a_texel_is_ten_texels_across_with_a_margin_each_side(self):
        charts = odlight.flatten([plane(1.0)], 10.0)
        self.assertEqual((charts[0].w, charts[0].h), (10 + 2 * odlight.MARGIN,) * 2)

    def test_a_coarser_density_gives_a_smaller_atlas(self):
        meshes = [box(20, 20, 5), plane(60.0)]
        fine = odlight.unwrap(meshes, 10.0)[1]
        coarse = odlight.unwrap(meshes, 40.0)[1]
        self.assertLess(coarse[0] * coarse[1], fine[0] * fine[1])

    def test_a_shape_can_choose_a_density_of_its_own(self):
        corners, triangles, _ = plane(10.0)
        own = odlight.flatten([(corners, triangles, 100.0)], 10.0)
        self.assertEqual(own[0].w, 10 + 2 * odlight.MARGIN)

    def test_a_huge_surface_is_made_coarser_to_stay_within_the_biggest_chart(self):
        charts = odlight.flatten([plane(2000.0)], 10.0, max_chart=512)
        self.assertEqual(charts[0].w, 512 + 2 * odlight.MARGIN)

    def test_no_two_charts_overlap_and_all_lie_inside_the_atlas(self):
        meshes = [box(2, 3, 1), box(0.5, 0.5, 4, (3, 0, 0)), plane(10.0), box(30, 20, 6, (50, 0, 0))]
        charts = odlight.flatten(meshes, 25.0)
        width, height = odlight.pack(charts)
        for i, a in enumerate(charts):
            self.assertLessEqual(a.x + a.w, width)
            self.assertLessEqual(a.y + a.h, height)
            for b in charts[i + 1:]:
                apart = a.x + a.w <= b.x or b.x + b.w <= a.x or a.y + a.h <= b.y or b.y + b.h <= a.y
                self.assertTrue(apart, f"charts {i} and another overlap")

    def test_every_triangle_keeps_its_area_in_texels_to_within_a_few_percent(self):
        corners, triangles, _ = box(4, 2, 3)
        uvs, (width, height), _ = odlight.unwrap([(corners, triangles, None)], 10.0)
        for t, tri in enumerate(triangles):
            a, b, c = (corners[i] for i in tri)
            n = g.cross(g.sub(b, a), g.sub(c, a))
            real = 0.5 * g.length(n) * 100.0 ** 2 / 10.0 ** 2  # texels
            p = [(uvs[0][6 * t + 2 * k] * width, uvs[0][6 * t + 2 * k + 1] * height) for k in range(3)]
            flat = 0.5 * abs((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]))
            self.assertAlmostEqual(flat / real, 1.0, delta=0.05)

    def test_an_atlas_that_cant_fit_says_to_raise_the_texel_size(self):
        with self.assertRaises(ValueError) as caught:
            odlight.unwrap([plane(100.0)], 1.0, max_chart=4096, max_size=256)
        self.assertIn("Raise `texel_cm`", str(caught.exception))


class LightValues(unittest.TestCase):
    GOOD = {
        "sun_elevation": "37 °",
        "sun_azimuth": "225 °",
        "sun_colour": "#fff2d6",
        "sun_strength": "3.0",
        "sky_colour": "#a5c4f0",
        "sky_strength": "0.5",
    }

    def test_the_light_values_are_read_in_degrees_and_linear_colours(self):
        light = odlight.read_light(self.GOOD)
        self.assertEqual((light["sun_elevation"], light["sun_azimuth"]), (37.0, 225.0))
        self.assertEqual((light["sun_strength"], light["sky_strength"]), (3.0, 0.5))
        self.assertAlmostEqual(light["sun_colour"][0], 1.0)
        self.assertAlmostEqual(light["sky_colour"][2], 0.8714, places=3)

    def test_a_value_left_out_or_unknown_or_wrong_is_refused_in_a_sentence(self):
        for bad, words in (
            ({k: v for k, v in self.GOOD.items() if k != "sky_colour"}, "needs sky_colour"),
            (dict(self.GOOD, moon="1"), "doesn't know moon"),
            (dict(self.GOOD, sun_elevation="95 °"), "from 0"),
            (dict(self.GOOD, sun_colour="orange"), "sRGB"),
            (dict(self.GOOD, sun_strength="-1"), "plain number"),
        ):
            with self.assertRaises(ValueError) as caught:
                odlight.read_light(bad)
            self.assertIn(words, str(caught.exception))

    def test_the_light_key_changes_with_every_value_and_not_with_their_order(self):
        key = odlight.light_key(self.GOOD)
        self.assertEqual(key, odlight.light_key(dict(reversed(list(self.GOOD.items())))))
        for name in self.GOOD:
            other = dict(self.GOOD, **{name: self.GOOD[name].replace("3", "4").replace("f", "e")})
            if other != self.GOOD:
                self.assertNotEqual(key, odlight.light_key(other), name)

    def test_the_sun_is_in_the_compass_direction_and_as_high_as_asked(self):
        east = odlight.sun_direction(30.0, 90.0)
        self.assertAlmostEqual(east[0], 0.8660, places=3)
        self.assertAlmostEqual(east[2], 0.5, places=6)
        north = odlight.sun_direction(0.0, 0.0)
        self.assertAlmostEqual(north[1], 1.0, places=9)


if __name__ == "__main__":
    unittest.main()
