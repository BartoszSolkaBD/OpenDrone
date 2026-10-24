"""Readable checks for the texture generators (ADR-0024, #292).

Each test's name is the plain sentence it proves. They need numpy, so they run
in the pinned Blender's Python, which has it (CI does exactly this; so can
you):

    blender --background --factory-startup --python-exit-code 1 \\
        --python assets-src/generators/test_generators.py

and any Python with numpy (`python3 assets-src/generators/test_generators.py`)
runs them too, apart from the one that decodes with Blender's own JPEG reader.

Basis: Rule (ADR-0024: the pictures are committed and a check regenerates
them, so a generator must give the same bytes for the same seed; a texture
repeats, so it must have no seam; a decal is put once and its rectangle must
never show, so its mask is clear along every edge, #321), and Observed for the
normal map's shape.
The pictures are made at 512 pixels, to be quick: a generator draws the same
picture at any size.
"""

import os
import sys
import tomllib
import unittest

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(REPO, "assets-src", "lib"))
sys.path.insert(0, HERE)

import odjpeg  # noqa: E402
import odtex  # noqa: E402
import odtexgen  # noqa: E402
import decalwork  # noqa: E402

PIXELS = 512


def generators():
    """Every generator the material library uses, with its seed."""
    with open(os.path.join(REPO, "assets-src", "materials.toml"), "rb") as f:
        textures = tomllib.load(f)["textures"]
    return sorted(
        (entry["generator"], entry["seed"]) for entry in textures.values() if "generator" in entry
    )


def _has_bpy():
    try:
        import bpy  # noqa: F401

        return True
    except ImportError:
        return False


def steps_round_the_tile(picture, axis):
    """How big the step is from the last column (axis 1) or row (axis 0) back
    to the first, and the biggest step between any two neighbouring lines
    inside the picture, each as the mean difference over the line."""
    p = picture.astype(np.float64)
    across = tuple(a for a in range(p.ndim) if a != axis)
    steps = np.abs(np.roll(p, -1, axis=axis) - p).mean(axis=across)
    return steps[-1], steps[:-1].max()


class TheJpegWriter(unittest.TestCase):
    def test_writes_the_same_bytes_every_time(self):
        picture = np.arange(64 * 64 * 3, dtype=np.int64).reshape(64, 64, 3) % 251
        picture = picture.astype(np.uint8)
        self.assertEqual(odjpeg.encode(picture, 90), odjpeg.encode(picture, 90))

    def test_writes_a_file_that_starts_and_ends_as_a_jpeg_does(self):
        picture = np.full((32, 32, 3), 128, dtype=np.uint8)
        data = odjpeg.encode(picture, 85, halve_colour=True)
        self.assertEqual(data[:2], b"\xff\xd8")
        self.assertEqual(data[-2:], b"\xff\xd9")

    def test_a_picture_that_isnt_a_whole_number_of_16_pixel_squares_is_refused(self):
        with self.assertRaises(ValueError):
            odjpeg.encode(np.zeros((40, 40), dtype=np.uint8), 85)

    @unittest.skipUnless(_has_bpy(), "needs Blender's Python")
    def test_gives_a_picture_blenders_own_reader_reads_back_almost_as_it_was(self):
        import bpy

        rng_x, rng_y = np.meshgrid(np.arange(96), np.arange(96))
        picture = np.stack(
            [rng_x * 255 // 96, rng_y * 255 // 96, (rng_x ^ rng_y) & 255], axis=-1
        ).astype(np.uint8)
        folder = os.path.join(REPO, "target", "generator-tests")
        os.makedirs(folder, exist_ok=True)
        for name, halve in (("full", False), ("halved", True)):
            path = os.path.join(folder, f"{name}.jpg")
            with open(path, "wb") as f:
                f.write(odjpeg.encode(picture, 95, halve_colour=halve))
            image = bpy.data.images.load(path)
            image.colorspace_settings.name = "Non-Color"
            width, height = image.size
            self.assertEqual((width, height), (96, 96))
            pixels = np.array(image.pixels[:], dtype=np.float32).reshape(height, width, image.channels)
            read = pixels[::-1, :, :3] * 255.0
            error = np.abs(read - picture.astype(np.float64)).mean()
            # A quality of 95 on a smooth picture: well within 3 of 255.
            self.assertLess(error, 3.0, name)


class EachGenerator(unittest.TestCase):
    """Run for every texture made from scratch in the material library."""

    @classmethod
    def setUpClass(cls):
        cls.made = {}
        for generator, seed in generators():
            first = odtexgen.make_arrays(generator, seed, PIXELS)
            again = odtexgen.make_arrays(generator, seed, PIXELS)
            other = odtexgen.make_arrays(generator, seed + 1, PIXELS)
            cls.made[generator] = (first, again, other)

    def test_the_library_makes_the_first_slices_eight_materials(self):
        self.assertGreaterEqual(len(self.made), 8)

    def test_the_same_seed_gives_the_same_bytes(self):
        for generator, (first, again, _other) in self.made.items():
            self.assertEqual(odtexgen.encode_pictures(*first), odtexgen.encode_pictures(*again), generator)

    def materials(self):
        """The generators of materials' pictures, which repeat: not decals."""
        return {g: made for g, made in self.made.items() if len(made[0]) == 3}

    def decals(self):
        """The generators of decals, which make a fourth picture, the mask."""
        return {g: made for g, made in self.made.items() if len(made[0]) == 4}

    def test_a_different_seed_gives_different_bytes(self):
        for generator, (first, _again, other) in self.made.items():
            kinds = ("colour", "normal", "roughness", "mask")
            for kind, a, b in zip(kinds, odtexgen.encode_pictures(*first), odtexgen.encode_pictures(*other)):
                self.assertNotEqual(a, b, f"{generator}'s {kind} picture")

    def test_each_picture_repeats_without_a_seam(self):
        for generator, ((diffuse, normal, roughness), _again, _other) in self.materials().items():
            for kind, picture in (("colour", diffuse), ("normal", normal), ("roughness", roughness)):
                for axis, name in ((1, "left to right"), (0, "top to bottom")):
                    wrap, biggest = steps_round_the_tile(picture, axis)
                    self.assertLessEqual(wrap, biggest * 1.25 + 1.0, f"{generator}'s {kind}, {name}")

    def test_the_normal_map_leans_out_of_the_surface_and_isnt_flat(self):
        for generator, ((_d, normal, _r), _again, _other) in self.materials().items():
            n = normal.astype(np.float64) / 255.0 * 2.0 - 1.0
            self.assertGreater(n[..., 2].min(), 0.0, f"{generator}: every normal points out of the surface")
            self.assertGreater(n[..., 2].mean(), 0.6, generator)
            self.assertLess(n[..., 2].mean(), 0.9995, f"{generator}'s normal map has relief")
            # Slopes lean both ways, so the red and green channels average near the middle.
            self.assertLess(abs(n[..., 0].mean()), 0.1, generator)
            self.assertLess(abs(n[..., 1].mean()), 0.1, generator)

    def test_the_colour_isnt_blank_and_no_picture_is_one_flat_tone(self):
        for generator, ((diffuse, normal, roughness), _again, _other) in self.materials().items():
            self.assertGreater(diffuse.std(), 6.0, generator)
            self.assertGreater(roughness.std(), 1.0, generator)
            self.assertGreater(diffuse.mean(), 25, f"{generator} isn't black")
            self.assertLess(diffuse.mean(), 225, f"{generator} isn't white")

    def test_the_library_makes_decals_of_moss_a_stain_and_a_damp_patch(self):
        self.assertTrue({"decal_moss", "decal_stain", "decal_damp"} <= set(self.decals()))

    def test_the_library_has_at_least_twelve_kinds_of_wear_as_decals(self):
        # #323: runs, salt, lichen, algae, rust, scuffs, scratches, cracks, soot,
        # oil, droppings, paint flakes, cobwebs, tags and stencils, with the
        # first slice's moss, stain and damp.
        wanted = {"decal_runs", "decal_salt", "decal_lichen", "decal_algae", "decal_rust_bleed", "decal_scuffs",
                  "decal_scratches", "decal_cracks", "decal_soot", "decal_oil", "decal_droppings", "decal_flakes",
                  "decal_cobweb", "decal_tag", "decal_stencil", "decal_moss", "decal_stain", "decal_damp"}
        self.assertTrue(wanted <= set(self.decals()))
        self.assertGreaterEqual(len(self.decals()), 12)

    def test_a_decals_mask_has_soft_edges_not_a_hard_cut(self):
        # Where a decal covers at all (mask over 3%), a good part of it is
        # partly covered, not all or nothing. A stencil's paint and a flake's
        # edge are the hardest, and still have a tenth of it soft.
        for generator, ((_d, _n, _r, mask), _again, _other) in self.decals().items():
            covered = mask > 8
            partial = ((mask > 8) & (mask < 230)).sum() / max(1, covered.sum())
            self.assertGreater(partial, 0.08, generator)

    def test_a_decals_colour_is_filled_in_under_its_clear_parts_so_a_blurred_edge_doesnt_darken(self):
        for generator, ((diffuse, _n, _r, mask), _again, _other) in self.decals().items():
            clear = mask == 0
            self.assertGreater(diffuse[clear].mean(), 8, f"{generator}: black under the clear mask")

    def test_a_decals_mask_is_clear_along_every_edge_so_its_rectangle_never_shows(self):
        for generator, ((_d, _n, _r, mask), _again, _other) in self.decals().items():
            edge = max(1, int(decalwork.CLEAR_EDGE * PIXELS))
            for name, strip in (("top", mask[:edge]), ("bottom", mask[-edge:]),
                                ("left", mask[:, :edge]), ("right", mask[:, -edge:])):
                self.assertEqual(strip.max(), 0, f"{generator}'s mask along its {name} edge")

    def test_a_decals_mask_is_nearly_whole_somewhere_and_covers_part_of_it(self):
        for generator, ((_d, _n, _r, mask), _again, _other) in self.decals().items():
            # A thin mark (a run, a crack, a scratch) is never whole for long.
            self.assertGreaterEqual(mask.max(), 200, generator)
            covered = (mask > 127).mean()
            self.assertGreater(covered, 0.01, generator)
            self.assertLess(covered, 0.75, generator)

    def test_a_decals_normal_map_leans_out_of_the_surface(self):
        for generator, ((_d, normal, _r, _m), _again, _other) in self.decals().items():
            n = normal.astype(np.float64) / 255.0 * 2.0 - 1.0
            self.assertGreater(n[..., 2].min(), 0.0, generator)


class TheBuildingBlocks(unittest.TestCase):
    def test_noise_repeats_across_the_tile(self):
        X, Y = odtex.grid(64)
        a = odtex.fbm(X, Y, 4, 3, 5)
        b = odtex.fbm(X + 1.0, Y + 1.0, 4, 3, 5)
        self.assertTrue(np.array_equal(a, b))

    def test_the_same_seed_gives_the_same_noise_and_another_seed_doesnt(self):
        X, Y = odtex.grid(32)
        self.assertTrue(np.array_equal(odtex.fbm(X, Y, 4, 3, 5), odtex.fbm(X, Y, 4, 3, 5)))
        self.assertFalse(np.array_equal(odtex.fbm(X, Y, 4, 3, 5), odtex.fbm(X, Y, 4, 3, 6)))

    def test_a_brick_wall_has_a_joint_between_bricks_and_the_courses_are_staggered(self):
        X, Y = odtex.grid(256)
        wall = odtex.bricks(X, Y, 8, 4, 0.2, 1)
        self.assertGreater(wall["joint"].mean(), 0.05)
        self.assertLess(wall["joint"].mean(), 0.5)
        # Two neighbouring courses have their vertical joints in different places.
        row_a = wall["joint"][16, :] > 0.5  # inside a course, between its joints
        row_b = wall["joint"][48, :] > 0.5
        self.assertFalse(np.array_equal(row_a, row_b))

    def test_the_seam_check_does_catch_a_picture_that_does_not_repeat(self):
        ramp = np.tile(np.arange(64, dtype=np.uint8)[:, None] * 4, (1, 64))
        wrap, biggest = steps_round_the_tile(ramp, 0)
        self.assertGreater(wrap, biggest * 10.0)

    def test_streaks_only_run_down_from_where_the_source_is(self):
        X, Y = odtex.grid(128)
        source = np.zeros_like(X)
        source[20, 60:64] = 1.0
        down = odtex.streaks(source, 64, 1, X, Y, 16, 0.98)
        self.assertGreater(down[40:60, 58:66].max(), 0.0)
        self.assertEqual(down[5:15, :].max(), 0.0)


if __name__ == "__main__":
    # Under Blender, the arguments before `--` are Blender's own.
    sys.argv = [sys.argv[0]] + (sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    unittest.main(exit=True)
