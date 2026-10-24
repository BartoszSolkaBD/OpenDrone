"""A look-test Map for the kit and the scatter (#339): not a Map of the Pack.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/demos/kit/build.py -- --out <file.glb> [--plain] [--seed N] [--light-cm 25]

A stripped hall, 20 x 14 m, open to the sky: a concrete floor, three walls
and a row of steel columns, its fourth side open onto a dirt yard. The kit
(`assets-src/lib/odkit.py`) is put down by the scatter's rules
(`odscatter.py`): a rubble heap in the yard and one against the hall's north
wall, plaster and brick heaped along the wall feet, a strewn floor, bolt plates and
anchor stubs where a column line was cut away, pallets and boards piled in
corners, a fallen pipe run, drums, a tyre, puddles and moss. A 4 m aisle runs
from the Launch Spot through the doorway down the hall, and the scatter keeps
it clear.

`--plain` leaves the kit out, to measure what it costs; `--seed` changes every
choice the rules make; `--light-cm` asks for baked light.

Nothing builds this in CI and nothing commits its `.glb`: to fly or render it,
build it into `packs/opendrone/maps/kit-demo/` on a checkout of your own,
give that folder a `map.toml` like any Map's (copy Bando's, with `model =
"kit-demo.glb"`) and add `("kit-demo", "opendrone/kit-demo")` to `MAPS`
in `crates/opendrone/src/main.rs`. East is +x, north is +y, up is +z.
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
from odground import Flat, FlatRect, Ground  # noqa: E402
from odmap import MapBuilder  # noqa: E402
from odscatter import Scatter  # noqa: E402

YARD = (-22.0, -22.0, 26.0, 16.0)
HALL = (-10.0, -6.0, 10.0, 8.0)
WALL_H, WALL_T = 5.0, 0.3
LAUNCH = (0.0, -16.0)
FLOOR_Z = 0.05


def build(out, seed=7, dressed=True, light_cm=None):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    if light_cm is not None:
        m.lightmap(light_cm)
    x0, y0, x1, y1 = HALL

    # ---------- the yard and the hall's floor ----------
    yard = Ground(5, amplitude=0.12, wavelength=9.0, slope=(0.004, 0.003),
                  flats=[Flat(LAUNCH[0], LAUNCH[1], 3.5), FlatRect(x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3, blend=0.8, level=0.0)])
    yard_shape = m.solid("yard", yard.mesh(YARD, 0.5), None, "ground_dirt_rubble", "trimesh")
    floor = Ground(8, amplitude=0.012, wavelength=5.0, base=FLOOR_Z)
    floor_shape = m.solid("floor", floor.mesh(HALL, 0.5), None, "concrete_floor_worn", "trimesh")
    m.launch_spot(LAUNCH[0], LAUNCH[1], yard.height(*LAUNCH), 0.0)

    # ---------- walls and columns ----------
    door = (8.0, 12.0, 0.0, 4.2)  # 4 m wide in the south wall, from x = -2 to 2
    m.wall("south_wall", (x1, y0), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered", openings=[door])
    m.wall("north_wall", (x0, y1), (x1, y1), 0.0, WALL_H, WALL_T, "concrete_wall_rough")
    m.wall("west_wall", (x0, y1), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered")
    for x in (-8.0, -4.0, 4.0, 8.0):
        m.box_h("column", x, 3.0, 0.3, 0.3, FLOOR_Z, WALL_H, "steel_beam_rusted")
    m.beam("tie", (x0 + WALL_T / 2, 3.0, WALL_H - 0.2), (x1, 3.0, WALL_H - 0.2), 0.25, 0.35, "steel_beam_rusted")

    def ground(x, y):
        inside = x0 + 0.01 < x < x1 - 0.01 and y0 + 0.01 < y < y1 - 0.01
        return floor.height(x, y) if inside else yard.height(x, y)

    stats = None
    if dressed:
        s = Scatter(seed, ground)
        s.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.5)  # the Launch Spot
        s.keep_clear_rect(-2.0, YARD[1], 2.0, y1 - 1.0)  # the aisle, from the Launch Spot through the doorway
        s.keep_clear_rect(x0 - 1.0, y0 - 0.2, x1 + 1.0, y0 + 0.2)  # the south wall itself
        for x in (-8.0, -4.0, 4.0, 8.0):
            s.keep_clear_rect(x - 0.2, 2.8, x + 0.2, 3.2)  # the columns
        inside_n, inside_w, inside_s = y1 - WALL_T / 2, x0 + WALL_T / 2, y0 + WALL_T / 2
        s.keep_clear_rect(x0 - 1.0, inside_n, x1 + 1.0, y1 + 1.0)  # inside the north and west walls
        s.keep_clear_rect(x0 - 1.0, y0 - 1.0, inside_w, y1 + 1.0)

        # The rubble heap in the yard, by the hall's open east side, and one
        # against the north wall, where its top fell in.
        s.heap((15.0, 1.0), 2.8, 1.4)
        s.heap((-5.5, inside_n - 0.4), 2.2, 1.1, toward=(0.0, 1.0))
        # Plaster and brick along the wall feet, on a low wedge of crumbled render.
        s.wall_foot((x1, inside_n), (x0, inside_n), depth=1.1, height=0.14, core="rubble")
        s.wall_foot((inside_w, y1), (inside_w, y0), depth=0.9, height=0.12, core="rubble")
        s.wall_foot((x0, inside_s), (x1, inside_s), depth=0.7, height=0.08, per_metre=25.0, core="rubble")
        # The floor under the gone roof: strewn in clusters, and the yard more thinly.
        s.strew((x0, y0, x1, y1), 2.0, bed="rubble")
        s.strew((x1 + 0.5, -14.0, 22.0, 12.0), 0.25)
        s.strew((-18.0, -14.0, x0 - 0.5, 12.0), 0.15)
        # Where a second column line was cut away: bolt plates and anchor stubs.
        s.anchor_row((-8.0, -1.5), (8.0, -1.5), 4.0, missing=0.0)
        s.anchor_row((-6.0, 6.0), (6.0, 6.0), 3.0)
        # Pallets and boards piled in the corners, a pipe run that fell, drums.
        s.pallets(-8.4, -4.4, 0.0)
        s.pallets(8.4, 6.6, 0.0, stacks=(2, 3))
        s.boards(-7.6, 1.2, 75.0)
        s.boards(6.5, -4.4, 10.0)
        s.place("plank", 5.6, 0.6, 100.0)
        s.place("plank", 3.4, -3.3, 35.0)
        s.place("pipe_run", 4.5, 4.6, 8.0)
        s.place("drum", 9.0, -2.6, 0.0, k=0)
        s.place("drum", 9.2, -1.8, 30.0, k=1)
        s.place("drum_lying", 7.9, -2.2, 70.0)
        s.place("cable_tangle", -4.6, -3.2, 0.0)
        s.place("tyre", 12.6, -1.8, 0.0)
        s.place("roof_sheet_piece", -3.6, 0.8, 20.0)
        # Puddles and moss on the floor and in the yard.
        s.patches((x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5), 9, on=[floor_shape])
        s.patches((x1 + 1.0, -12.0, 20.0, 10.0), 5, on=[yard_shape], kinds=("damp", "moss", "moss"))
        s.apply(m)
        stats = s.stats()

    m.export(out)
    return m, stats


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 7
    light_cm = float(argv[argv.index("--light-cm") + 1]) if "--light-cm" in argv else None
    builder, stats = build(argv[argv.index("--out") + 1], seed, "--plain" not in argv, light_cm)
    print("kit-demo:", builder.stats())
    if stats:
        print("  scatter:", stats)
