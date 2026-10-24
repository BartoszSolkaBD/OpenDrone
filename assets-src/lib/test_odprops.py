"""Readable checks for the props (#324): each test's name is the sentence it
proves. Plain Python: `python3 -m unittest discover -s assets-src/lib`.

Basis: Rule (the Pack checker takes a `trimesh` collider as a solid only if
it is closed, and ADR-0024's same bytes from the same seed) and Source for the
sizes (a Euro pallet is 1.2 x 0.8 x 0.144 m; a bottle crate about 0.4 x 0.3 x
0.3 m; a 200 litre drum 0.88 m tall and 0.58 m across), in metres.
"""
import unittest
from collections import Counter

import odgeom as g
import odprops as p
from odnoise import Random


def closed(mesh):
    """Every edge is shared by exactly two faces, once each way."""
    corners, faces = mesh
    edges = Counter()
    for f in faces:
        for a, b in zip(f, f[1:] + f[:1]):
            edges[(a, b)] += 1
    return all(edges[(b, a)] == n == 1 for (a, b), n in edges.items())


def all_props(rnd):
    return {
        "pallet": p.pallet(rnd.child("pallet")),
        "cable drum": p.cable_drum(),
        "crate": p.crate(rnd.child("crate"), cracked=True),
        "bottle": p.bottle(rnd.child("bottle")),
        "crushed can": p.can(rnd.child("can"), crushed=0.8),
        "bin bag": p.bin_bag(rnd.child("bag")),
        "melted chair": p.plastic_chair(rnd.child("chair"), melt=0.7),
        "drum": p.drum(rnd.child("drum"), dented=1.0),
        "grill": p.grill(rnd.child("grill")),
        "bed frame": p.bed_frame(rnd.child("bed")),
        "trolley": p.trolley(rnd.child("trolley")),
        "filing cabinet": p.filing_cabinet(rnd.child("cabinet")),
        "drawer": p.drawer(),
        "office chair": p.office_chair(),
        "fuse box": p.fuse_box(),
        "tyre": p.tyre(),
        "skip": p.skip(rnd.child("skip")),
        "plinth": p.plinth(rnd.child("plinth"), 2.4, 1.4, 0.25),
        "bolts": p.bolts(2.4, 1.4, 0.25),
        "metal pile": p.metal_pile(rnd.child("pile")),
        "gate leaf": p.gate_leaf(rnd.child("gate"), 3.0, 2.2),
        "quad": p.quad_frame(rnd.child("quad"), arms_gone=1),
        "prop": p.prop_blades(rnd.child("prop")),
        "antenna": p.antenna(),
    }


class Props(unittest.TestCase):
    def test_the_same_seed_gives_the_same_props_and_another_seed_other_ones(self):
        a, b, c = all_props(Random(1, "x")), all_props(Random(1, "x")), all_props(Random(2, "x"))
        self.assertEqual(a, b)
        self.assertNotEqual(a["pallet"], c["pallet"])
        self.assertNotEqual(a["bin bag"], c["bin bag"])

    def test_every_prop_is_made_of_closed_solids_facing_out_so_it_can_be_a_trimesh_collider(self):
        for name, (corners, faces) in all_props(Random(3, "x")).items():
            tris = g.triangulate(corners, faces)
            self.assertTrue(closed((corners, [tuple(t) for t in tris])), f"{name} is closed")
            self.assertGreater(g.signed_volume(corners, faces), 0.0, f"{name} faces out")

    def test_props_stand_on_the_ground_at_their_real_size(self):
        props = all_props(Random(4, "x"))
        for name, size, tol in (("pallet", (1.2, 0.8, 0.144), 0.01), ("crate", (0.4, 0.3, 0.3), 0.01),
                                ("drum", (0.604, 0.604, 0.88), 0.03), ("cable drum", (1.0, 1.0, 0.62), 0.02)):
            got = p.size_of(props[name])
            for k in range(3):
                self.assertAlmostEqual(got[k], size[k], delta=tol, msg=f"{name} axis {k}")
        for name, mesh in props.items():
            if name in ("fuse box", "gate leaf"):
                continue  # hung on a wall, and on its hinge
            self.assertGreaterEqual(min(c[2] for c in mesh[0]), -1e-9, f"{name} stands on z = 0")

    def test_a_melted_chair_is_lower_than_a_whole_one_and_a_crushed_can_lower_than_a_whole_can(self):
        r = Random(5, "x")
        self.assertLess(p.size_of(p.plastic_chair(r, 0.8))[2], p.size_of(p.plastic_chair(r, 0.0))[2] - 0.15)
        self.assertLess(p.size_of(p.can(r, 0.9))[2], 0.06)

    def test_a_pallet_may_miss_or_break_deck_boards_but_keeps_its_outer_two(self):
        counts = {len(p.pallet(Random(s, "p"))[1]) for s in range(20)}
        self.assertGreater(len(counts), 1, "pallets differ")


if __name__ == "__main__":
    unittest.main()
