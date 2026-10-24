"""Readable checks for the wear rules (`odwear.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`). Each test's name is the sentence it proves. Basis: Rule,
from #323 and the "Where things collect" table of docs/research/lived-in-
places.md: water runs down from a sill or an edge, moss grows low in shade
and damp and not on a dry wall that faces the sun, rust runs from a fixing,
soot rises from a fire, people wear a path to their ways in, and the same
seed makes the same Map.
"""
import hashlib
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import odgeom as g  # noqa: E402
from oddecal import Decal, Host  # noqa: E402
from odopening import Opening  # noqa: E402
from odwear import LAYER, MATERIAL, RUN_OVERLAP, SOURCE, Face, Floor, Wear  # noqa: E402

SOUTH_WEST = (-0.7071, -0.7071)  # the sun is in the south-west


def wall(name="south", run=14.0, ops=None, painted=False):
    """A wall face from (7, -4) walking west: its front is the south side."""
    if ops is None:
        ops = [Opening(2.2, 3.8, 1.2, 2.8, "window"), Opening(5.4, 7.2, 0.0, 2.4, "doorway"),
               Opening(9.6, 11.2, 0.4, 2.0, "breach")]
    return Face(name, (7.0, -4.0), (-7.0, -4.0), 0.0, 4.5, ["wall_" + name], ops, painted)


def north_wall():
    """The same wall seen from the other side: its front is the north side."""
    return Face("north", (-7.0, 4.0), (7.0, 4.0), 0.0, 4.5, ["wall_north"], [], False)


FLOOR = Floor(["ground"])


def fingerprint(wear):
    text = repr([(s.name, s.material, s.centre, s.facing, s.up, s.size, s.layer, s.depth) for s in wear.result()])
    return hashlib.sha256(text.encode()).hexdigest()


def everything(seed, level, sun=SOUTH_WEST):
    w = Wear(seed, level, sun)
    f = wall()
    w.runs_below_openings(f)
    w.runs_from_edge(f, gutters=[11.0])
    w.moss_at_foot(f, FLOOR, damp=[10.4])
    w.salt_at_foot(f, FLOOR)
    w.lichen_on(north_wall())
    w.rust_seam((-5.0, 4.0), (5.0, 4.0), 3.0, (0.0, -1.0, 0.0), ["wall_north"])
    w.soot_above(f, 6.0, 0.6)
    w.scuff_paths([(0.0, -12.0), (2.0, -5.0), (2.0, 3.0)], FLOOR)
    w.scratches_low(f)
    w.marks_near_openings(f)
    w.cracks_at_openings(f)
    w.flakes_at_foot(wall(painted=True), FLOOR)
    w.droppings_under((-4.0, 0.0), (4.0, 0.0), FLOOR)
    w.oil_at([(1.0, 1.0), (-2.0, 2.0)], FLOOR)
    w.damp_at([(2.0, 0.5, 1.4), (-3.0, 2.0, 0.8)], FLOOR)
    return w


class Repeatable(unittest.TestCase):
    def test_the_same_seed_gives_the_very_same_decals_every_time(self):
        self.assertEqual(fingerprint(everything(7, 0.7)), fingerprint(everything(7, 0.7)))

    def test_another_seed_gives_other_decals(self):
        self.assertNotEqual(fingerprint(everything(7, 0.7)), fingerprint(everything(8, 0.7)))

    def test_a_level_of_nothing_puts_no_wear_by_rule(self):
        self.assertEqual(everything(7, 0.0).result(), [])

    def test_a_higher_level_puts_more_wear_and_bigger_marks(self):
        low = sum(len(everything(s, 0.2).result()) for s in range(6))
        high = sum(len(everything(s, 1.0).result()) for s in range(6))
        self.assertGreater(high, low * 1.8)
        sizes = lambda lv: [s.size[0] for s in everything(3, lv).result() if s.kind == "runs"]
        self.assertGreater(max(sizes(1.0)), max(sizes(0.2)))

    def test_adding_a_rule_never_moves_what_another_rule_placed(self):
        only = Wear(7, 0.7, SOUTH_WEST)
        only.moss_at_foot(wall(), FLOOR)
        both = Wear(7, 0.7, SOUTH_WEST)
        both.runs_below_openings(wall())
        both.rust_below([(0.0, 0.0, 2.0)], (0.0, -1.0, 0.0), ["w"])
        both.moss_at_foot(wall(), FLOOR)
        pick = lambda w: [(s.name, s.centre, s.size) for s in w.result() if s.kind == "moss"]
        self.assertEqual(pick(only), pick(both))

    def test_a_hand_placed_decal_stays_whatever_the_level_and_the_clear_boxes(self):
        w = Wear(1, 0.0)
        w.hand("tag", "decal_tag", ["w"], (1.0, 2.0, 1.4), (0, -1, 0), (0, 0, 1), (1.0, 1.0), layer=2)
        w.keep_clear((-100, -100, -100), (100, 100, 100))
        self.assertEqual([s.name for s in w.result()], ["tag"])


class WaterRuns(unittest.TestCase):
    def test_runs_always_start_at_an_opening_or_an_edge(self):
        f = wall()
        sills = {f.z0 + op.v0 for op in f.openings}
        for seed in range(8):
            w = Wear(seed, 1.0, SOUTH_WEST)
            w.runs_below_openings(f)
            w.runs_from_edge(f, gutters=[11.0])
            runs = [s for s in w.result() if s.kind == "runs"]
            self.assertTrue(runs)
            for s in runs:
                start = s.meta["start_z"]
                self.assertTrue(start in sills or start == f.z0 + f.height, f"{s.name} starts at {start}")
                # The picture's top is just above where the water comes over.
                top = s.centre[2] + s.size[1] / 2.0
                self.assertAlmostEqual(top, start + RUN_OVERLAP * s.size[1], places=6)

    def test_runs_hang_below_their_start_and_inside_the_openings_width(self):
        f = wall()
        w = Wear(2, 1.0, SOUTH_WEST)
        w.runs_below_openings(f)
        for s in w.result():
            k = s.meta["source"][1]
            op = f.openings[k]
            self.assertLess(s.centre[2], f.z0 + op.v0 + 0.01)
            self.assertTrue(op.u0 <= s.meta["u"] <= op.u1)

    def test_a_doorway_has_no_sill_so_no_runs(self):
        f = wall(ops=[Opening(5.4, 7.2, 0.0, 2.4, "doorway")])
        for seed in range(6):
            w = Wear(seed, 1.0)
            w.runs_below_openings(f)
            self.assertEqual(w.result(), [])

    def test_a_broken_gutter_always_has_its_run(self):
        f = wall(ops=[])
        for seed in range(6):
            w = Wear(seed, 0.3)
            w.runs_from_edge(f, gutters=[11.0])
            starts = [s.meta["u"] for s in w.result()]
            self.assertIn(11.0, starts)

    def test_damp_lies_where_water_stands_and_a_worn_place_grows_algae_in_a_big_one(self):
        w = Wear(1, 0.9)
        w.damp_at([(2.0, 0.5, 1.4), (-3.0, 2.0, 0.6)], FLOOR)
        kinds = [(s.kind, s.meta["spot"]) for s in w.result()]
        self.assertIn(("damp", (2.0, 0.5, 1.4)), kinds)
        self.assertIn(("algae", (2.0, 0.5, 1.4)), kinds)
        self.assertNotIn(("algae", (-3.0, 2.0, 0.6)), kinds)


class Growth(unittest.TestCase):
    def moss(self, face, **kw):
        out = []
        for seed in range(12):
            w = Wear(seed, 1.0, SOUTH_WEST)
            w.moss_at_foot(face, FLOOR, **kw)
            out += w.result()
        return out

    def test_no_moss_on_a_dry_south_facing_wall(self):
        self.assertEqual(self.moss(wall()), [])

    def test_a_shaded_north_facing_wall_grows_moss(self):
        self.assertGreater(len(self.moss(north_wall())), 12)

    def test_a_sunny_wall_grows_moss_only_where_water_comes_down(self):
        moss = self.moss(wall(), damp=[10.4])
        self.assertTrue(moss)
        for s in moss:
            self.assertLess(abs(s.meta["u"] - 10.4), 1.5)

    def test_moss_lies_low_on_the_ground_and_up_the_wall_at_once(self):
        for s in self.moss(north_wall()):
            self.assertAlmostEqual(s.centre[2], 0.24, places=6)
            self.assertGreater(s.facing[2], 0.5)  # out of the corner: half up
            self.assertIn("ground", s.hosts)
            self.assertIn("wall_north", s.hosts)

    def test_moss_leaves_a_doorway_alone(self):
        f = wall(ops=[Opening(5.4, 7.2, 0.0, 2.4, "doorway")])
        f.facing = (0.0, 1.0)  # shaded, to be sure it would grow here
        for seed in range(12):
            w = Wear(seed, 1.0, (0.0, -1.0))
            w.moss_at_foot(f, FLOOR)
            for s in w.result():
                self.assertFalse(5.4 - 0.3 < s.meta["u"] < 7.2 + 0.3)

    def test_a_shaded_corner_grows_moss_and_a_sunny_one_doesnt(self):
        w = Wear(1, 1.0, SOUTH_WEST)
        w.moss_in_corners([(0, 0, (0.7071, 0.7071), ["a"]), (5, 0, (-0.7071, -0.7071), ["b"])] * 8, FLOOR)
        self.assertTrue(w.result())
        self.assertTrue(all(s.hosts[0] == "a" for s in w.result()))

    def test_lichen_grows_only_on_a_shaded_wall_and_never_over_an_opening(self):
        sunny = Wear(2, 1.0, SOUTH_WEST)
        sunny.lichen_on(wall())
        self.assertEqual(sunny.result(), [])
        f = Face("north", (-7.0, 4.0), (7.0, 4.0), 0.0, 4.5, ["n"], [Opening(2.0, 4.0, 1.0, 3.0, "window")])
        got = []
        for seed in range(10):
            shaded = Wear(seed, 1.0, SOUTH_WEST)
            shaded.lichen_on(f)
            got += shaded.result()
        self.assertTrue(got)
        for s in got:
            _x, _y, z = s.centre
            along = (s.centre[0] - f.p0[0]) / f.along[0]
            self.assertFalse(f.blocked(along, z, s.size[0], s.size[1], margin=0.0), s.name)
            self.assertGreaterEqual(s.meta["shade"], 0.5)

    def test_rust_has_its_fixing_in_the_top_of_its_picture(self):
        w = Wear(1, 1.0)
        w.rust_seam((-3.0, 4.0), (3.0, 4.0), 3.0, (0.0, -1.0, 0.0), ["steel"])
        self.assertGreaterEqual(len(w.result()), 3)
        for s in w.result():
            fx, fy, fz = s.meta["fixing"]
            down = (s.centre[2] + s.size[1] / 2.0 - fz) / s.size[1]  # share of the way down from the picture's top
            self.assertAlmostEqual(down, 0.5 - (0.5 - SOURCE), places=6)
            self.assertEqual(s.material, "decal_rust_bleed")
            self.assertEqual((s.centre[0], s.centre[1]), (fx, fy))

    def test_salt_gathers_where_damp_rises_and_more_where_it_is_shaded(self):
        near = Wear(1, 1.0, SOUTH_WEST)
        near.salt_at_foot(wall(), FLOOR, damp=[8.8])
        self.assertTrue(near.result())
        self.assertTrue(all(s.meta["damp"] or s.meta["shade"] < 0.3 for s in near.result()))
        n_sunny = len(self._salt(wall()))
        n_shaded = len(self._salt(north_wall()))
        self.assertGreater(n_shaded, n_sunny)

    def _salt(self, face):
        out = []
        for seed in range(10):
            w = Wear(seed, 1.0, SOUTH_WEST)
            w.salt_at_foot(face, FLOOR)
            out += w.result()
        return out


class Fire(unittest.TestCase):
    def test_soot_starts_just_above_the_flame_and_rises(self):
        f = wall(ops=[])
        w = Wear(1, 0.8)
        w.soot_above(f, 6.0, 0.7)
        (s,) = w.result()
        self.assertAlmostEqual(s.meta["bottom"], 0.7 - 0.05 * s.size[1], places=6)
        self.assertGreater(s.centre[2], 0.7 + s.size[1] * 0.4)
        self.assertGreaterEqual(s.size[1], 1.5)

    def test_soot_over_a_burnt_door_comes_out_of_its_top(self):
        f = wall()
        w = Wear(1, 1.0)
        w.soot_over_door(f, 1)  # the doorway, 5.4 to 7.2 m, 2.4 m high
        (s,) = w.result()
        self.assertAlmostEqual(s.meta["flame_z"], 2.2)
        self.assertGreater(s.size[0], 1.8)  # wider than the doorway

    def test_soot_on_a_ceiling_faces_down(self):
        w = Wear(1, 1.0)
        w.soot_ceiling(1.0, 2.0, 4.5, ["roof"])
        (s,) = w.result()
        self.assertEqual(s.facing, (0.0, 0.0, -1.0))


class People(unittest.TestCase):
    PATH = [(0.0, 0.0), (30.0, 0.0), (30.0, 10.0)]

    def test_scuffs_follow_the_path_and_stay_within_its_width(self):
        for seed in range(6):
            w = Wear(seed, 1.0)
            w.scuff_paths(self.PATH, FLOOR, width=1.0)
            self.assertTrue(w.result())
            for s in w.result():
                x, y, _z = s.centre
                on_first = abs(y) <= 0.16 and -0.01 <= x <= 30.01
                on_second = abs(x - 30.0) <= 0.16 and -0.01 <= y <= 10.01
                self.assertTrue(on_first or on_second, s.name)

    def test_the_wear_is_heavier_toward_a_paths_ends(self):
        near_end, middle = 0, 0
        for seed in range(20):
            w = Wear(seed, 0.6)
            w.scuff_paths([(0.0, 0.0), (40.0, 0.0)], FLOOR)
            for s in w.result():
                x = s.centre[0]
                if x < 3.0 or x > 37.0:
                    near_end += 1
                elif 15.0 < x < 25.0:
                    middle += 1
        # The two ends together are 6 m of the path and the middle 10 m: per metre, more wear at the ends.
        self.assertGreater(near_end / 6.0, middle / 10.0 * 1.3)

    def test_scratches_gather_near_the_floor(self):
        f = Face("long", (60.0, -4.0), (0.0, -4.0), 0.0, 4.5, ["w"], [])
        heights = []
        for seed in range(6):
            w = Wear(seed, 1.0)
            w.scratches_low(f)
            heights += [s.meta["z"] for s in w.result()]
        self.assertGreater(len(heights), 100)
        self.assertLess(sum(heights) / len(heights), 0.9)
        self.assertLess(max(heights), 1.6)

    def test_tags_and_stencils_are_beside_a_way_in_at_arms_reach(self):
        f = wall()
        found = []
        for seed in range(20):
            w = Wear(seed, 1.0)
            w.marks_near_openings(f)
            found += w.result()
        self.assertTrue({s.kind for s in found} >= {"tag", "stencil"})
        for s in found:
            self.assertTrue(0.5 + s.size[1] / 2.0 - 0.3 <= s.meta["z"] <= 2.2 - s.size[1] / 2.0 + 0.6)
            self.assertTrue(1.0 <= s.meta["z"] <= 1.6)
            op = f.openings[s.meta["opening"]]
            self.assertLessEqual(op.v0, 1.0)
            along = (f.p0[0] - s.centre[0])
            width_gap = min(abs(along - op.u0), abs(along - op.u1))
            self.assertLess(width_gap, 0.5 + s.size[0] + 0.01)
            self.assertFalse(f.blocked(along, s.centre[2], s.size[0], s.size[1], margin=0.0))

    def test_a_window_too_high_to_stand_at_gets_no_tag(self):
        f = wall(ops=[Opening(2.0, 3.0, 2.5, 3.5, "window")])
        for seed in range(10):
            w = Wear(seed, 1.0)
            w.marks_near_openings(f)
            self.assertEqual(w.result(), [])


class Age(unittest.TestCase):
    def test_cracks_start_at_an_openings_top_corners(self):
        f = wall()
        found = []
        for seed in range(10):
            w = Wear(seed, 1.0)
            w.cracks_at_openings(f)
            found += w.result()
        self.assertTrue(found)
        for s in found:
            u, z = s.meta["corner"]
            along = f.p0[0] - s.centre[0]
            self.assertLess(g.length((along - u, 0.0, s.centre[2] - z)), 1.0)
            self.assertGreater(s.centre[2], z)

    def test_flakes_fall_only_from_a_painted_wall_and_lie_on_the_floor(self):
        bare = Wear(1, 1.0)
        bare.flakes_at_foot(wall(), FLOOR)
        self.assertEqual(bare.result(), [])
        painted = Wear(1, 1.0)
        painted.flakes_at_foot(wall(painted=True), FLOOR)
        self.assertTrue(painted.result())
        for s in painted.result():
            self.assertEqual(s.facing, (0.0, 0.0, 1.0))
            self.assertEqual(s.hosts, ["ground"])

    def test_cobwebs_hang_from_the_ceiling_in_a_corner(self):
        w = Wear(1, 1.0)
        w.cobwebs_in_corners([(i * 3.0, 0.0, 4.5, (0.7071, 0.7071), ["a", "b"]) for i in range(6)])
        self.assertTrue(w.result())
        for s in w.result():
            self.assertAlmostEqual(s.centre[2] + s.size[1] / 2.0, 4.5 + 0.015 * s.size[1], places=6)

    def test_droppings_fall_under_the_beam_and_a_little_either_side(self):
        w = Wear(1, 1.0)
        w.droppings_under((-4.0, 1.0), (4.0, 1.0), FLOOR)
        self.assertGreaterEqual(len(w.result()), 4)
        for s in w.result():
            self.assertTrue(-4.0 <= s.centre[0] <= 4.0)
            self.assertLessEqual(abs(s.centre[1] - 1.0), 0.251)

    def test_oil_lies_near_where_a_machine_stood(self):
        w = Wear(1, 1.0)
        w.oil_at([(i * 4.0, 0.0) for i in range(10)], FLOOR)
        self.assertTrue(w.result())
        for s in w.result():
            x, y = s.meta["spot"]
            self.assertLessEqual(g.length((s.centre[0] - x, s.centre[1] - y, 0.0)), 0.36)

    def test_floor_cracks_stay_inside_the_rectangle(self):
        w = Wear(1, 1.0)
        w.cracks_on_floor((0.0, 0.0, 10.0, 6.0), FLOOR, 6)
        self.assertTrue(w.result())
        for s in w.result():
            self.assertTrue(0.0 <= s.centre[0] <= 10.0 and 0.0 <= s.centre[1] <= 6.0)


class ClearAndLock(unittest.TestCase):
    def test_a_cleared_box_keeps_the_rules_out_and_the_rest_stays(self):
        free = everything(5, 1.0)
        locked = everything(5, 1.0)
        locked.keep_clear((-1.0, -100.0, -1.0), (4.0, 100.0, 10.0))
        gone = [s for s in free.result() if -1.0 <= s.centre[0] <= 4.0]
        self.assertTrue(gone)
        self.assertEqual(len(locked.result()), len(free.result()) - len(gone))
        self.assertTrue(all(not (-1.0 <= s.centre[0] <= 4.0) for s in locked.result()))


class WhatTheyAskFor(unittest.TestCase):
    def test_every_kind_has_a_decal_material_in_the_library(self):
        with open(os.path.join(HERE, "..", "materials.toml")) as f:
            text = f.read()
        library = set(re.findall(r"^\[materials\.(\w+)\]", text, re.M))
        for kind, material in MATERIAL.items():
            self.assertIn(material, library, kind)

    def test_few_layers_are_used_and_every_one_is_a_valid_decal_layer(self):
        self.assertLessEqual(len(set(LAYER.values())), 5)
        self.assertTrue(all(0 <= v <= 15 for v in LAYER.values()))
        used = {s.layer for s in everything(3, 1.0).result()}
        self.assertLessEqual(len(used), 5)

    def test_every_decal_can_be_projected_and_every_decal_asked_for_makes_a_valid_projector(self):
        for s in everything(4, 1.0).result():
            Decal(s.centre, s.facing, s.up, s.size, depth=s.depth)

    def test_a_run_below_a_sill_lies_on_the_wall_under_it(self):
        corners, faces = g.box_mesh(14.0, 0.34, 4.5)
        corners = [g.add(c, (0.0, -4.0 + 0.17, 2.25)) for c in corners]
        host = Host(corners, g.triangulate(corners, faces))
        f = Face("south", (7.0, -4.0), (-7.0, -4.0), 0.0, 4.5, ["w"], [Opening(2.2, 3.8, 1.2, 2.8, "window")])
        w = Wear(3, 1.0)
        w.runs_below_openings(f)
        (s, *_rest) = [s for s in w.result()]
        corners, triangles, uvs, _light = Decal(s.centre, s.facing, s.up, s.size, depth=s.depth).project([host])
        self.assertTrue(triangles)
        # The picture's top is at the sill, up the wall: every corner is at or below it.
        self.assertLessEqual(max(c[2] for c in corners), 1.2 + RUN_OVERLAP * s.size[1] + 0.003)
        self.assertTrue(all(abs(c[1] - (-4.0 - 0.002)) < 0.01 for c in corners))


if __name__ == "__main__":
    unittest.main()
