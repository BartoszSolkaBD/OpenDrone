"""Makes the textures that are made from scratch (ADR-0024, #292).

Run by `cargo xtask generate-textures` in the pinned Blender, whose Python has
numpy, and never by hand:

    blender --background --factory-startup --python odtexgen.py -- \\
        --out FOLDER --sheet FILE.jpg \\
        --texture NAME GENERATOR SEED PIXELS [--texture ...]

For each texture it runs the script `assets-src/generators/GENERATOR.py` (its
`make(seed, pixels)` gives three uint8 pictures: colour, normal map and
roughness) and writes `FOLDER/NAME/diffuse.jpg`, `normal.jpg` and
`roughness.jpg`; and it writes one contact sheet of every colour picture.
Everything it writes is the same bytes on every computer (`odjpeg.py`).

The pictures: the colour one is saved at a quality of 88 with its colour at
half size, the normal map at 88 with every channel at full size (a normal
map's channels are directions, not colour), and the roughness at 85 as a grey
picture.
"""

import importlib
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
GENERATORS = os.path.join(os.path.dirname(HERE), "generators")
for folder in (HERE, GENERATORS):
    if folder not in sys.path:
        sys.path.insert(0, folder)

import odjpeg  # noqa: E402

DIFFUSE_QUALITY = 88
NORMAL_QUALITY = 88
ROUGHNESS_QUALITY = 85

# The contact sheet: every colour picture shrunk to this many pixels across,
# four to a row.
SHEET_CELL = 512
SHEET_COLUMNS = 4
SHEET_QUALITY = 80


def encode_pictures(diffuse, normal, roughness):
    """The three pictures of a texture as JPG bytes."""
    return (
        odjpeg.encode(diffuse, DIFFUSE_QUALITY, halve_colour=True),
        odjpeg.encode(normal, NORMAL_QUALITY),
        odjpeg.encode(roughness, ROUGHNESS_QUALITY),
    )


def make_arrays(generator, seed, pixels):
    """What the generator `generator` draws for a seed: three uint8 pictures,
    colour (pixels, pixels, 3), normal map (pixels, pixels, 3) and roughness
    (pixels, pixels)."""
    module = importlib.import_module(generator)
    pictures = module.make(seed, pixels)
    for picture in pictures:
        assert picture.dtype == np.uint8 and picture.shape[:2] == (pixels, pixels), (
            f"{generator} must make square uint8 pictures of {pixels} pixels"
        )
    return pictures


def make_pictures(generator, seed, pixels):
    """The three JPG files of a texture, as bytes `(diffuse, normal,
    roughness)`, and the colour picture as an array (for the contact sheet)."""
    diffuse, normal, roughness = make_arrays(generator, seed, pixels)
    return encode_pictures(diffuse, normal, roughness), diffuse


def shrink(picture, factor):
    """A colour picture at `1/factor` of its size in each direction: each new
    pixel is the whole-number average of a `factor` x `factor` square."""
    if factor == 1:
        return picture
    h, w, _ = picture.shape
    squares = picture.astype(np.int64).reshape(h // factor, factor, w // factor, factor, 3)
    total = squares.sum(axis=(1, 3))
    half = (factor * factor) // 2
    return ((total + half) // (factor * factor)).astype(np.uint8)


def contact_sheet(pictures):
    """One picture of every colour picture in a grid, in the order given."""
    cells = []
    for picture in pictures:
        pixels = picture.shape[0]
        if pixels % SHEET_CELL:
            raise ValueError(f"a {pixels}-pixel picture doesn't shrink to {SHEET_CELL} evenly")
        cells.append(shrink(picture, pixels // SHEET_CELL))
    rows = (len(cells) + SHEET_COLUMNS - 1) // SHEET_COLUMNS
    sheet = np.full((rows * SHEET_CELL, SHEET_COLUMNS * SHEET_CELL, 3), 24, dtype=np.uint8)
    for i, cell in enumerate(cells):
        top = (i // SHEET_COLUMNS) * SHEET_CELL
        left = (i % SHEET_COLUMNS) * SHEET_CELL
        sheet[top : top + SHEET_CELL, left : left + SHEET_CELL] = cell
    return odjpeg.encode(sheet, SHEET_QUALITY, halve_colour=True)


def parse(argv):
    out = None
    sheet = None
    textures = []
    i = 0
    while i < len(argv):
        flag = argv[i]
        if flag == "--out":
            out = argv[i + 1]
            i += 2
        elif flag == "--sheet":
            sheet = argv[i + 1]
            i += 2
        elif flag == "--texture":
            name, generator, seed, pixels = argv[i + 1 : i + 5]
            textures.append((name, generator, int(seed), int(pixels)))
            i += 5
        else:
            raise SystemExit(f"odtexgen: unknown argument {flag!r}")
    if out is None or not textures:
        raise SystemExit("odtexgen: needs --out FOLDER and at least one --texture NAME GENERATOR SEED PIXELS")
    return out, sheet, textures


def main(argv):
    out, sheet, textures = parse(argv)
    colours = []
    for name, generator, seed, pixels in textures:
        files, colour = make_pictures(generator, seed, pixels)
        folder = os.path.join(out, name)
        os.makedirs(folder, exist_ok=True)
        for filename, data in zip(("diffuse.jpg", "normal.jpg", "roughness.jpg"), files):
            with open(os.path.join(folder, filename), "wb") as f:
                f.write(data)
        colours.append(colour)
        print(f"odtexgen: {name} ({generator}, seed {seed}, {pixels} px): {sum(len(d) for d in files)} bytes")
    if sheet:
        with open(sheet, "wb") as f:
            f.write(contact_sheet(colours))


if __name__ == "__main__":
    args = sys.argv
    main(args[args.index("--") + 1 :] if "--" in args else args[1:])
