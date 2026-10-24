"""Readable checks for the kit (`odkit.py`), MIT OR Apache-2.0.

They run with plain Python, no Blender (`python3 -m unittest discover -s
assets-src/lib`, which CI's Maps check does too). Each test's name is the
sentence it proves. Basis: Rule, from #339 and docs/building-a-map.md (a Map
is the same bytes on every computer; a mesh the builder takes has no triangle
that rounds to nothing), and Source for the sizes: a brick is 215 x 102.5 x
65 mm (BS EN 771-1's work size), a Euro pallet 1200 x 800 x 144 mm (EN
13698-1), a 200 litre drum about 880 mm tall and 585 mm across (ISO 15750).
"""
import hashlib
import os
import re
import sys
import unittest

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import odgeom as g  # noqa: E402
import odkit  # noqa: E402
from odnoise import Random  # noqa: E402


def digest(parts):
    text = repr([(m, [g.snap3(c) for c in cs], fs) for m, (cs, fs) in parts])
    return hashlib.sha256(text.encode()).hexdigest()


def every_variant():
    for name, item in sorted(odkit.KIT.items()):
        for k in range(item.variants):
            yield name, k, odkit.variant(name, k)


def library_materials():
    with open(os.path.join(here, "..", "materials.toml"), encoding="utf-8") as f:
        return set(re.findall(r"^\[materials\.([a-z0-9_]+)\]", f.read(), re.M))


def edges_pair_up(mesh):
    """Every edge of every face is met once each way: the mesh is closed and
    its faces agree on which side is out."""
    count = {}
    for face in mesh[1]:
        for i in range(len(face)):
            a, b = face[i], face[(i + 1) % len(face)]
            count[(a, b)] = count.get((a, b), 0) + 1
    return all(n == 1 and count.get((b, a)) == 1 for (a, b), n in count.items())


class TheSameBytes(unittest.TestCase):
    def test_every_kit_item_rebuilds_to_the_same_bytes(self):
        for name, item in sorted(odkit.KIT.items()):
            for k in range(item.variants):
                a = odkit._finish(item.build(Random(odkit.KIT_SEED, name, k)))
                b = odkit._finish(item.build(Random(odkit.KIT_SEED, name, k)))
                self.assertEqual(digest(a), digest(b), f"{name} variant {k}")

    def test_the_variants_of_one_item_are_not_all_the_same(self):
        for name, item in sorted(odkit.KIT.items()):
            seen = {digest(odkit.variant(name, k)) for k in range(item.variants)}
            self.assertGreater(len(seen), 1, name)


class Solid(unittest.TestCase):
    def test_every_kit_item_is_closed_and_faces_out(self):
        for name, k, parts in every_variant():
            for material, mesh in parts:
                self.assertTrue(edges_pair_up(mesh), f"{name} {k} {material} is not closed")
                self.assertGreater(g.signed_volume(*mesh), 0.0, f"{name} {k} {material} faces in")

    def test_no_kit_triangle_rounds_to_nothing(self):
        for name, k, parts in every_variant():
            for material, (corners, faces) in parts:
                snapped = [g.snap3(c) for c in corners]
                for a, b, c in g.triangulate(snapped, faces):
                    area = g.length(g.cross(g.sub(snapped[b], snapped[a]), g.sub(snapped[c], snapped[a])))
                    self.assertGreater(area, 0.0, f"{name} {k} {material}")

    def test_every_kit_item_stands_on_z_0_centred_on_its_own_origin(self):
        for name, k, parts in every_variant():
            x0, y0, z0, x1, y1, z1 = odkit.extent(parts)
            self.assertAlmostEqual(z0, 0.0, places=9, msg=name)
            self.assertAlmostEqual((x0 + x1) / 2.0, 0.0, places=9, msg=name)
            self.assertAlmostEqual((y0 + y1) / 2.0, 0.0, places=9, msg=name)

    def test_every_kit_material_is_in_the_material_library(self):
        library = library_materials()
        for name, k, parts in every_variant():
            for material, _ in parts:
                self.assertIn(material, library, name)


class Light(unittest.TestCase):
    def test_a_small_things_faces_each_span_two_texels_of_the_atlas_when_asked_and_not_otherwise(self):
        import odlight
        corners, faces = odkit.variant("brick", 0)[0][1]
        triangles = g.triangulate(corners, faces)
        margin = odlight.MARGIN
        asked = odlight.flatten([(corners, triangles, None, 2.0)], 20.0)
        self.assertTrue(all(c.w >= 2 + 2 * margin and c.h >= 2 + 2 * margin for c in asked))
        plain = odlight.flatten([(corners, triangles, None)], 20.0)
        self.assertTrue(any(c.w < 2 + 2 * margin or c.h < 2 + 2 * margin for c in plain))


class Sizes(unittest.TestCase):
    def size(self, name, k=0):
        x0, y0, z0, x1, y1, z1 = odkit.extent(odkit.variant(name, k))
        return x1 - x0, y1 - y0, z1 - z0

    def test_a_brick_is_215_by_102_by_65_mm_less_its_chipped_corners(self):
        for k in range(odkit.KIT["brick"].variants):
            for got, want in zip(self.size("brick", k), (0.215, 0.1025, 0.065)):
                self.assertLessEqual(got, want + 1e-9)
                self.assertGreater(got, want - 0.025)

    def test_a_pallet_is_a_euro_pallet_1_2_by_0_8_by_0_144_m(self):
        for k in range(odkit.KIT["pallet"].variants):
            sx, sy, sz = self.size("pallet", k)
            self.assertAlmostEqual(sx, 1.2, places=6)
            self.assertAlmostEqual(sy, 0.8, places=6)
            self.assertAlmostEqual(sz, 0.144, places=6)

    def test_a_pallet_stack_is_two_to_six_pallets_tall(self):
        for k in range(odkit.KIT["pallet_stack"].variants):
            pallets = round(self.size("pallet_stack", k)[2] / 0.144)
            self.assertGreaterEqual(pallets, 2)
            self.assertLessEqual(pallets, 6)

    def test_a_standing_drum_is_0_88_m_tall_and_about_0_6_m_across(self):
        for k in range(odkit.KIT["drum"].variants):
            sx, sy, sz = self.size("drum", k)
            self.assertAlmostEqual(sz, 0.88, places=6)
            self.assertLess(abs(max(sx, sy) - 0.6), 0.03)

    def test_a_concrete_lump_has_rebar_sticking_out_of_it(self):
        for k in range(odkit.KIT["concrete_lump"].variants):
            materials = [m for m, _ in odkit.variant("concrete_lump", k)]
            self.assertEqual(materials, [odkit.CONCRETE, odkit.REBAR])


if __name__ == "__main__":
    unittest.main()
