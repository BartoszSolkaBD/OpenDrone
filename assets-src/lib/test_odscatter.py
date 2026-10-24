"""Readable checks for the scatter (`odscatter.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check does too). Each test's name is the
sentence it proves. Basis: Rule, from #339: the same seed gives the same
mess, items rest on the ground, nothing falls in a keep-clear zone (so an
aisle, a Gap or the Launch Spot stays open), a heap is bigger at its base and
slopes, and the items merge into one shape per material per cell. Lengths are
in metres.
"""
import hashlib
import math
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
from odground import Ground  # noqa: E402
from odscatter import Scatter  # noqa: E402

ROOM = (-8.0, -6.0, 8.0, 6.0)
AISLE = (-1.5, -6.0, 1.5, 6.0)
TOLERANCE = 0.001  # 1 mm


def hall(seed=7, ground=None, aisle=True, extra=False):
    """A strewn hall floor with a heap, debris along a wall foot, a column line, pallets and
    boards, on rolling ground."""
    ground = ground or Ground(3, amplitude=0.10, wavelength=6.0).height
    s = Scatter(seed, ground)
    if aisle:
        s.keep_clear_rect(*AISLE)
        s.keep_clear_circle(5.0, -3.0, 1.0)
    s.heap((5.0, 3.0), 2.0, 0.9, toward=(0.0, 1.0))
    s.wall_foot((8.0, -6.0), (-8.0, -6.0), depth=0.9, per_metre=15.0, core="concrete_smooth")
    s.strew(ROOM, 0.6)
    s.anchor_row((-7.0, 1.0), (7.0, 1.0), 2.0)
    if extra:
        s.strew((-7.0, -5.0, -3.0, 5.0), 1.0)
    s.pallets(-6.5, -4.0, 0.0)
    s.boards(-6.0, 4.0, 80.0)
    return s


def digest(s):
    text = repr([(name, material, [g.snap3(c) for c in mesh[0]], mesh[1]) for name, material, mesh in s.meshes()])
    text += repr([(rule, material, [g.snap3(c) for c in mesh[0]]) for rule, material, mesh in s.cores])
    return hashlib.sha256(text.encode()).hexdigest()


class SameSeed(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_scatter(self):
        self.assertEqual(digest(hall(7)), digest(hall(7)))

    def test_another_seed_gives_another_scatter(self):
        self.assertNotEqual(digest(hall(7)), digest(hall(8)))

    def test_adding_a_rule_never_moves_what_another_rule_placed(self):
        before = [(p.rule, p.kind, p.at) for p in hall(7).placements if p.rule != "strew"]
        after = [(p.rule, p.kind, p.at) for p in hall(7, extra=True).placements if p.rule != "strew"]
        self.assertEqual(before, after)


class Resting(unittest.TestCase):
    def test_every_strewn_item_rests_on_the_ground_neither_floating_nor_buried_past_its_sink(self):
        ground = Ground(3, amplitude=0.10, wavelength=6.0).height
        s = hall(ground=ground)
        checked = 0
        for p in s.placements:
            if p.rule not in ("strew", "anchor_row", "pallets", "place"):
                continue
            gap = min(c[2] - ground(c[0], c[1]) for c in p.corners())
            self.assertLessEqual(gap, TOLERANCE, f"{p.kind} floats {gap:.4f} m")
            self.assertGreaterEqual(gap, -p.sink - TOLERANCE, f"{p.kind} is buried {gap:.4f} m")
            checked += 1
        self.assertGreater(checked, 50)

    def test_heap_items_rest_on_the_heap_and_wall_foot_items_on_the_wall_foot_wedge(self):
        ground = Ground(3, amplitude=0.10, wavelength=6.0).height
        s = Scatter(7, ground)
        heap = s.heap((5.0, 3.0), 2.0, 0.9)
        foot = s.wall_foot((8.0, -6.0), (-8.0, -6.0), depth=0.9)
        for p in s.placements:
            surface = heap if p.rule == "heap" else foot
            gap = min(c[2] - surface(c[0], c[1]) for c in p.corners())
            self.assertLessEqual(gap, TOLERANCE, f"{p.rule} {p.kind} floats {gap:.4f} m")
            self.assertGreaterEqual(gap, -p.sink - TOLERANCE, f"{p.rule} {p.kind} is buried {gap:.4f} m")

    def test_a_strewn_floor_with_a_bed_has_relief_and_its_items_rest_on_the_bed(self):
        s = Scatter(7)
        surface = s.strew(ROOM, 1.0, bed="rubble")
        self.assertGreaterEqual(max(surface(x * 0.25, y * 0.25) for x in range(-32, 33) for y in range(-24, 25)), 0.03)
        self.assertEqual([rule for rule, _, _ in s.cores], ["strew"])
        for p in s.placements:
            gap = min(c[2] - surface(c[0], c[1]) for c in p.corners())
            self.assertLessEqual(gap, TOLERANCE, f"{p.kind} floats {gap:.4f} m")
            self.assertGreaterEqual(gap, -p.sink - TOLERANCE, f"{p.kind} is buried {gap:.4f} m")

    def test_a_board_pile_stacks_each_board_on_the_ones_below(self):
        s = Scatter(7)
        s.boards(0.0, 0.0, 0.0, count=(8, 8))
        lows = [min(c[2] for c in p.corners()) for p in s.placements]
        self.assertEqual(lows, sorted(lows))
        self.assertGreater(lows[-1], 0.1)


class KeepClear(unittest.TestCase):
    def test_no_corner_of_any_item_or_core_falls_in_a_keep_clear_zone(self):
        s = hall()
        corners = [c for _, _, mesh in s.meshes() for c in mesh[0]] + [c for _, _, mesh in s.cores for c in mesh[0]]
        x0, y0, x1, y1 = AISLE
        for x, y, _ in corners:
            self.assertFalse(x0 + TOLERANCE < x < x1 - TOLERANCE and y0 < y < y1, f"({x:.3f}, {y:.3f}) in the aisle")
            self.assertGreater(math.hypot(x - 5.0, y + 3.0), 1.0 - TOLERANCE)

    def test_the_keep_clear_zones_do_drop_items_that_would_be_there(self):
        self.assertGreater(hall().dropped, 0)
        bare = hall(aisle=False)
        x0, y0, x1, y1 = AISLE
        self.assertTrue(any(x0 < p.at[0] < x1 for p in bare.placements))


class Shapes(unittest.TestCase):
    def test_a_heap_is_tallest_near_its_middle_and_slopes_to_nothing_at_its_edge(self):
        s = Scatter(7)
        heap = s.heap((0.0, 0.0), 2.0, 1.0, count=0)
        self.assertGreater(heap(0.0, 0.0), 0.6)
        self.assertLess(heap(1.0, 0.0), heap(0.3, 0.0))
        self.assertLess(heap(1.8, 0.0), 0.1)
        self.assertEqual(heap(2.1, 0.0), 0.0)

    def test_a_heap_leant_toward_a_wall_is_higher_on_that_side(self):
        heap = Scatter(7).heap((0.0, 0.0), 2.0, 1.0, toward=(0.0, 1.0), count=0)
        self.assertGreater(heap(0.0, 1.0), heap(0.0, -1.0))

    def test_a_heap_has_its_big_lumps_at_its_foot_and_small_ones_on_top(self):
        s = Scatter(7)
        s.heap((0.0, 0.0), 2.5, 1.0, mix={"concrete_chunk": 1})
        foot, top = [], []
        for p in s.placements:
            cs = p.corners()
            size = max(c[0] for c in cs) - min(c[0] for c in cs)
            (foot if math.hypot(p.at[0], p.at[1]) > 1.8 else top if math.hypot(p.at[0], p.at[1]) < 0.8 else []).append(size)
        self.assertGreater(sum(foot) / len(foot), 1.3 * sum(top) / len(top))

    def test_debris_along_a_wall_foot_is_thickest_against_the_wall_and_never_goes_into_it(self):
        s = Scatter(7)
        s.wall_foot((8.0, 0.0), (-8.0, 0.0), depth=1.0)  # walking west: the front, on the left, is south
        near = sum(1 for p in s.placements if -0.33 < p.at[1] <= 0.0)
        far = sum(1 for p in s.placements if p.at[1] <= -0.66)
        self.assertGreater(near, 2 * far)
        for p in s.placements:
            self.assertLessEqual(max(c[1] for c in p.corners()), -0.01 + TOLERANCE)

    def test_an_anchor_row_puts_its_plates_on_the_line_at_its_spacing(self):
        s = Scatter(7)
        s.anchor_row((-6.0, 2.0), (6.0, 2.0), 3.0, missing=0.0)
        self.assertEqual([round(p.at[0], 6) for p in s.placements], [-6.0, -3.0, 0.0, 3.0, 6.0])
        self.assertTrue(all(abs(p.at[1] - 2.0) < 1e-9 for p in s.placements))


class Output(unittest.TestCase):
    def test_items_merge_into_one_shape_per_material_per_cell(self):
        s = hall()
        names = [(name, material) for name, material, _ in s.meshes()]
        self.assertEqual(len(names), len(set(names)))
        cells = {name for name, _ in names}
        materials = {material for _, material in names}
        self.assertLessEqual(len(names), len(cells) * len(materials))
        self.assertLess(len(names), len(s.placements) / 20)

    def test_no_triangle_of_the_scatter_rounds_to_nothing(self):
        s = hall()
        for _, material, (corners, faces) in s.meshes() + s.cores:
            snapped = [g.snap3(c) for c in corners]
            for a, b, c in g.triangulate(snapped, faces):
                area = g.length(g.cross(g.sub(snapped[b], snapped[a]), g.sub(snapped[c], snapped[a])))
                self.assertGreater(area, 0.0, material)

    def test_the_stats_count_every_triangle_drawn(self):
        s = hall()
        st = s.stats()
        self.assertEqual(st["items"], len(s.placements))
        self.assertEqual(st["draws"], len(s.meshes()) + len(s.cores))
        self.assertEqual(st["item_triangles"], sum(st["triangles_by_material"].values()))


if __name__ == "__main__":
    unittest.main()
