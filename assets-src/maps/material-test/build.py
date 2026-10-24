"""Throwaway test surface for #320 (never committed): a floor, a wall and some
metal plates, each in several states of the same material, under baked light."""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
from odmap import MapBuilder  # noqa: E402

FLOORS = [
    "concrete_floor_flat",    # no roughness picture
    "concrete_floor_worn",    # roughness picture, dry
    "concrete_floor_wet50",   # picture, wet 0.5
    "concrete_floor_wet100",  # picture, wet 1.0
]
WALLS = ["brick_red_flat", "brick_red_weathered", "brick_red_wet", "brick_painted_wet"]
PLATES = ["steel_flat", "steel_m0", "steel_m50", "steel_m100", "roof_m0", "roof_m80", "steel_ao", "steel_mm"]


def build(out):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    m.lightmap(texel_cm=25)
    for i, material in enumerate(FLOORS):
        x0 = -12.0 + 6.0 * i
        m.box_h("floor", x0 + 3.0, 0.0, 6.0, 16.0, -0.3, 0.0, material)
    for i, material in enumerate(WALLS):
        y0 = -8.0 + 4.0 * i
        m.wall("wall", (-12.5, y0), (-12.5, y0 + 4.0), 0.0, 4.0, 0.3, material)
    m.box_h("floor_east", 26.0, 0.0, 28.0, 16.0, -0.3, 0.0, "concrete_floor_worn")
    for i, material in enumerate(PLATES):
        x = 14.0 + 3.0 * i
        m.box_h("plate", x, -3.0, 2.6, 0.15, 0.0, 3.0, material)
    m.slab("ground", -400, -400, 400, 400, 0.0, 0.3, "ground_dirt_rubble", holes=[(-12.0, -8.0, 40.0, 9.0)])
    m.launch_spot(-6.0, -14.0, 0.0, 0.0)
    m.export(out)
    return m


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    print("material-test:", build(out).stats())
