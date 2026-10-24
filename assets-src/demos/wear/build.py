"""A test Map for the wear rules (#323): not a Map of the Pack.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/demos/wear/build.py -- --out <file.glb> [--wear 0.7] [--seed 5] [--light-cm 25]

A brick hall, 12 x 8 m, with a doorway and a window in its south wall, a window
in its north wall, a doorway in its west wall (painted brick), two steel beams
across it, a fire pit by the west wall and two drums, standing in a yard. The
sun is in the south-west, so the south and west walls are dry and the north
and east walls are in shade. With `--wear 0` the hall is clean; a level from
0 to 1 asks `assets-src/lib/odwear.py` for wear: water runs below the sills and
the roof edge, moss and lichen on the shaded sides, rust under the beams'
ends, soot over the fire, scuffs along the path between the doors, oil by the
drums, damp in a floor dip, droppings under the beams, cobwebs in the upper
corners and a few tags beside the doorways.

Nothing builds this in CI and nothing commits its `.glb`: it is a way to see the
rules and to count what they cost. To fly or render it, copy this script to
`assets-src/maps/wear-demo/build.py` on a checkout of your own, give
`packs/opendrone/maps/wear-demo/` a `map.toml` copied from a Map's, with a
`[light]` of the sun at 225 degrees, and add the Map to `MAPS` in
`crates/opendrone/src/main.rs` (docs/building-a-map.md, "Wear").
East is +x, north is +y, up is +z; the origin is the middle of the hall.
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
from odground import Dip, Flat, FlatRect, Ground  # noqa: E402
from odmap import MapBuilder  # noqa: E402
from odwear import Face, Floor, Wear  # noqa: E402

SEED = 5
HALL = (-6.0, -4.0, 6.0, 4.0)
WALL_H, WALL_T = 4.0, 0.3
IN, OUT = WALL_T / 2.0, WALL_T / 2.0  # a face is half a wall's thickness from its line
SUN = (-0.7071, -0.7071)  # the direction to the sun, (east, north): south-west
LAUNCH = (0.0, -9.0, 0.0)


class Op:
    """An opening as `odwear.Face` reads it: `u0 u1 v0 v1 kind`."""

    def __init__(self, u0, u1, v0, v1, kind="window"):
        self.u0, self.u1, self.v0, self.v1, self.kind = u0, u1, v0, v1, kind


def mirror(openings, run):
    """The same openings counted from the other end of a wall `run` metres
    long, for the face on its other side."""
    return [Op(run - o.u1, run - o.u0, o.v0, o.v1, o.kind) for o in openings]


def names(m, prefix):
    """The names of the boxes of the wall called `prefix`."""
    return [n for n in m.shapes if n == prefix or (n.startswith(prefix + "_") and n[len(prefix) + 1:].isdigit())]


def build(out, level=0.7, seed=SEED, light_cm=None):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    if light_cm is not None:
        m.lightmap(light_cm)
    x0, y0, x1, y1 = HALL

    # ---------- ground and floor ----------
    yard = Ground(seed, amplitude=0.08, wavelength=9.0, flats=[Flat(LAUNCH[0], LAUNCH[1], 3.0),
                  FlatRect(x0 - 0.5, y0 - 0.5, x1 + 0.5, y1 + 0.5, blend=0.6, level=-0.1)])
    yard_ground = m.solid("yard", yard.mesh((-20.0, -20.0, 20.0, 20.0), 0.5), None, "grass_weeds", "trimesh")
    dips = [Dip(1.5, -1.5, 1.4, 0.025)]
    floor = Ground(seed + 1, amplitude=0.01, wavelength=5.0, base=0.0, dips=dips)
    hall_floor = m.solid("hall_floor", floor.mesh((x0, y0, x1, y1), 0.35), None, "concrete_floor_worn", "trimesh")

    # ---------- walls ----------
    south_ops = [Op(2.0, 3.4, 1.2, 2.6, "window"), Op(7.0, 8.4, 0.0, 2.2, "doorway")]   # u from the east end
    north_ops = [Op(4.5, 5.9, 1.2, 2.6, "window")]                                       # u from the west end
    west_ops = [Op(2.5, 3.9, 0.0, 2.2, "doorway")]                                        # u from the north end
    holes = lambda ops: [(o.u0, o.u1, o.v0, o.v1) for o in ops]
    m.wall("south", (x1, y0), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered", openings=holes(south_ops))
    m.wall("north", (x0, y1), (x1, y1), 0.0, WALL_H, WALL_T, "brick_red_weathered", openings=holes(north_ops))
    m.wall("west", (x0, y1), (x0, y0), 0.0, WALL_H, WALL_T, "brick_painted_peeling", openings=holes(west_ops))
    m.wall("east", (x1, y0), (x1, y1), 0.0, WALL_H, WALL_T, "brick_red_weathered")
    # Two steel beams across the hall, a fire pit and two drums.
    beams = [((-2.5, y0 + IN), (-2.5, y1 - IN)), ((2.0, y0 + IN), (2.0, y1 - IN))]
    for k, (a, b) in enumerate(beams):
        m.beam(f"beam_{k}", (a[0], a[1], 3.6), (b[0], b[1], 3.6), 0.2, 0.3, "steel_beam_rusted", collider="none")
    m.cylinder("fire_pit", (-4.6, -1.8, 0.0), (-4.6, -1.8, 0.3), 0.55, "brick_red_weathered", segments=14, collider="none")
    drums = [(3.2, 1.8), (4.0, 2.4)]
    for k, (x, y) in enumerate(drums):
        m.cylinder(f"drum_{k}", (x, y, 0.0), (x, y, 0.9), 0.3, "steel_painted", segments=12, collider="none")

    # ---------- wear ----------
    wear = Wear(seed, level, sun=SUN)
    yard_floor = Floor([yard_ground], yard.height)
    inside = Floor([hall_floor], floor.height)
    r = lambda n: names(m, n)
    faces = dict(
        south_out=Face("south_out", (x1, y0 - OUT), (x0, y0 - OUT), 0.0, WALL_H, r("south"), south_ops),
        south_in=Face("south_in", (x0, y0 + IN), (x1, y0 + IN), 0.0, WALL_H, r("south"), mirror(south_ops, 12.0)),
        north_out=Face("north_out", (x0, y1 + OUT), (x1, y1 + OUT), 0.0, WALL_H, r("north"), north_ops),
        north_in=Face("north_in", (x1, y1 - IN), (x0, y1 - IN), 0.0, WALL_H, r("north"), mirror(north_ops, 12.0)),
        west_out=Face("west_out", (x0 - OUT, y0), (x0 - OUT, y1), 0.0, WALL_H, r("west"),
                      mirror(west_ops, 8.0), painted=True),
        west_in=Face("west_in", (x0 + IN, y1), (x0 + IN, y0), 0.0, WALL_H, r("west"), west_ops, painted=True),
        east_out=Face("east_out", (x1 + OUT, y1), (x1 + OUT, y0), 0.0, WALL_H, r("east")),
        east_in=Face("east_in", (x1 - IN, y0), (x1 - IN, y1), 0.0, WALL_H, r("east")),
    )
    if level > 0.0:
        for name in ("south_out", "north_out", "west_out", "east_out"):
            f = faces[name]
            wear.runs_below_openings(f)
            wear.runs_from_edge(f, gutters=[10.5] if name == "south_out" else [])
            wear.moss_at_foot(f, yard_floor, damp=[10.5] if name == "south_out" else [])
            wear.salt_at_foot(f, yard_floor, damp=[10.5] if name == "south_out" else [])
            wear.lichen_on(f)
            wear.scratches_low(f)
            wear.marks_near_openings(f)
            wear.cracks_at_openings(f)
        for name in ("south_in", "north_in", "east_in"):
            f = faces[name]
            wear.scratches_low(f)
            wear.runs_from_edge(f)
        # The painted west wall's inside: flakes on the floor, and the fire pit's soot.
        wear.flakes_at_foot(faces["west_in"], inside)
        wear.soot_above(faces["west_in"], 5.8, 0.9)
        wear.cracks_at_openings(faces["west_in"])
        # Rust where the beams meet the walls, from their ends.
        for (a, b) in beams:
            wear.rust_below([(a[0], a[1], 3.45)], (0.0, 1.0, 0.0), r("south"))
            wear.rust_below([(b[0], b[1], 3.45)], (0.0, -1.0, 0.0), r("north"))
            wear.droppings_under(a, b, inside)
        # Cobwebs in the upper corners, scuffs between the doors, oil, damp.
        h = 0.7071
        wear.cobwebs_in_corners([
            (x0 + IN, y1 - IN, WALL_H, (h, -h), r("west") + r("north")),
            (x1 - IN, y1 - IN, WALL_H, (-h, -h), r("east") + r("north")),
            (x0 + IN, y0 + IN, WALL_H, (h, h), r("west") + r("south")),
            (x1 - IN, y0 + IN, WALL_H, (-h, h), r("east") + r("south")),
        ])
        wear.moss_in_corners([(x1 + OUT + 0.0, y1 + OUT + 0.0, (h, h), r("east") + r("north"))], yard_floor)
        wear.scuff_paths([(-1.7, y0 + 0.4), (-1.7, 0.8), (x0 + 0.5, 0.8)], inside)
        wear.oil_at(drums, inside)
        wear.damp_at([(d.x, d.y, d.radius) for d in dips], inside)
        wear.cracks_on_floor(HALL, inside, 4)
        wear.hand("tag_by_the_door", "decal_tag", r("south"), (-0.6, y0 - OUT, 1.5), (0.0, -1.0, 0.0), (0.0, 0.0, 1.0), (1.0, 1.0),
                  layer=2, depth=0.6)
        wear.apply(m)

    m.launch_spot(LAUNCH[0], LAUNCH[1], yard.height(LAUNCH[0], LAUNCH[1]), 0.0)
    m.export(out)
    area = sum(f.length * f.height for f in faces.values()) + (x1 - x0) * (y1 - y0)
    return m, wear, area


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    arg = lambda key, default: type(default)(argv[argv.index(key) + 1]) if key in argv else default
    builder, wear, area = build(arg("--out", "wear-demo.glb"), arg("--wear", 0.7), arg("--seed", SEED),
                                     float(argv[argv.index("--light-cm") + 1]) if "--light-cm" in argv else None)
    stats = builder.stats()
    print("wear-demo:", stats)
    counts = wear.counts()
    print("  decals by kind:", ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
    total = sum(counts.values())
    print(f"  {total} decals on {area:.0f} m2 of wall and floor: {total / area:.2f} a square metre")
    if "decal_triangles" in stats:
        print(f"  {stats['decal_triangles']} decal triangles: {stats['decal_triangles'] / area:.1f} a square metre")
