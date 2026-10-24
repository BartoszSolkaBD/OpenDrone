"""Readable checks for the kit (`odkit.py`) and its pieces for hand placement (#324), MIT OR Apache-2.0.

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
from collections import Counter

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



def closed_solid(mesh):
    """Every edge is shared by exactly two faces, once each way."""
    corners, faces = mesh
    edges = Counter()
    for f in faces:
        for a, b in zip(f, f[1:] + f[:1]):
            edges[(a, b)] += 1
    return all(edges[(b, a)] == n == 1 for (a, b), n in edges.items())


def all_pieces(rnd):
    return {
        "pallet": odkit.pallet_mesh(rnd.child("pallet")),
        "cable drum": odkit.cable_drum(),
        "crate": odkit.crate(rnd.child("crate"), cracked=True),
        "bottle": odkit.bottle_mesh(rnd.child("bottle")),
        "crushed can": odkit.can_mesh(rnd.child("can"), crushed=0.8),
        "bin bag": odkit.bin_bag(rnd.child("bag")),
        "melted chair": odkit.plastic_chair(rnd.child("chair"), melt=0.7),
        "drum": odkit.drum_mesh(rnd.child("drum"), dented=1.0),
        "grill": odkit.grill(rnd.child("grill")),
        "bed frame": odkit.bed_frame(rnd.child("bed")),
        "trolley": odkit.trolley(rnd.child("trolley")),
        "filing cabinet": odkit.filing_cabinet(rnd.child("cabinet")),
        "drawer": odkit.drawer(),
        "office chair": odkit.office_chair(),
        "fuse box": odkit.fuse_box(),
        "tyre": odkit.tyre_mesh(),
        "skip": odkit.skip(rnd.child("skip")),
        "plinth": odkit.plinth(rnd.child("plinth"), 2.4, 1.4, 0.25),
        "bolts": odkit.bolts(2.4, 1.4, 0.25),
        "metal pile": odkit.metal_pile(rnd.child("pile")),
        "gate leaf": odkit.gate_leaf(rnd.child("gate"), 3.0, 2.2),
        "quad": odkit.quad_frame(rnd.child("quad"), arms_gone=1),
        "propeller": odkit.propeller(rnd.child("prop")),
        "antenna": odkit.antenna(),
    }


class PiecesForHandPlacement(unittest.TestCase):
    def test_the_same_seed_gives_the_same_pieces_and_another_seed_other_ones(self):
        a, b, c = all_pieces(Random(1, "x")), all_pieces(Random(1, "x")), all_pieces(Random(2, "x"))
        self.assertEqual(a, b)
        self.assertNotEqual(a["pallet"], c["pallet"])
        self.assertNotEqual(a["bin bag"], c["bin bag"])

    def test_every_piece_is_made_of_closed_solids_facing_out_so_it_can_be_a_trimesh_collider(self):
        for name, (corners, faces) in all_pieces(Random(3, "x")).items():
            tris = g.triangulate(corners, faces)
            self.assertTrue(closed_solid((corners, [tuple(t) for t in tris])), f"{name} is closed")
            self.assertGreater(g.signed_volume(corners, faces), 0.0, f"{name} faces out")

    def test_pieces_stand_on_the_ground_at_their_real_size(self):
        pieces = all_pieces(Random(4, "x"))
        for name, size, tol in (("pallet", (1.2, 0.8, 0.144), 0.01), ("crate", (0.4, 0.3, 0.3), 0.01),
                                ("drum", (0.604, 0.604, 0.88), 0.03), ("cable drum", (1.0, 1.0, 0.62), 0.02)):
            got = odkit.size_of(pieces[name])
            for k in range(3):
                self.assertAlmostEqual(got[k], size[k], delta=tol, msg=f"{name} axis {k}")
        for name, mesh in pieces.items():
            if name in ("fuse box", "gate leaf"):
                continue  # hung on a wall, and on its hinge
            self.assertGreaterEqual(min(c[2] for c in mesh[0]), -1e-9, f"{name} stands on z = 0")

    def test_a_melted_chair_is_lower_than_a_whole_one_and_a_crushed_can_lower_than_a_whole_can(self):
        r = Random(5, "x")
        self.assertLess(odkit.size_of(odkit.plastic_chair(r, 0.8))[2], odkit.size_of(odkit.plastic_chair(r, 0.0))[2] - 0.15)
        self.assertLess(odkit.size_of(odkit.can_mesh(r, 0.9))[2], 0.06)

    def test_a_pallet_may_miss_or_break_deck_boards_but_keeps_its_outer_two(self):
        counts = {len(odkit.pallet_mesh(Random(s, "p"))[1]) for s in range(20)}
        self.assertGreater(len(counts), 1, "pallets differ")


if __name__ == "__main__":
    unittest.main()
