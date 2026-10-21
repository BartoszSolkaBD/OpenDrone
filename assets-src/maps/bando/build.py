"""Bando, "Unfinished Tower, with rooms" (#17, #64): built in Blender.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/maps/bando/build.py -- --out <file.glb> [--previews <folder>]

`cargo xtask maps` does that. A five-floor concrete frame on a building site
where work stopped: 24 x 15 m, brick rooms in the west half of the two lowest
floors, an open lift shaft, a tower crane, and a yard, all on a 70 x 50 m
site inside city blocks. East is +x, north is +y, up is +z; the origin is the
middle of the site, at ground level (0 m). The building's own ground slab is a
0.15 m plinth.

Every size below is the one in #17's table of must-have features: see
`crates/pack/tests/bando.rs`, which measures them in the finished Map. The
thin parts are solid at their real thickness: 6 cm rails, the crane's 5 cm hook
cable and 3 cm rebar.
"""
import math
import os
import random
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
from odmap import MapBuilder  # noqa: E402

LAUNCH = (0.0, -17.0, 0.0, 0.0)  # x, y, ground height, compass heading

# The building: X0..X1 by Y0..Y1, floors every FH = 3.35 m. `Z[k]` is the top
# of floor k (Z[0] the ground slab, Z[5] the roof): 0.25 m of slab under it
# leaves 3.1 m clear under each ceiling.
FH, X0, X1, Y0, Y1 = 3.35, -12.0, 12.0, -7.5, 7.5
Z = [0.15 + FH * k for k in range(6)]
SLAB = 0.25
CLEAR = FH - SLAB  # 3.1 m under a ceiling

# The crane: the mast's middle, its height, and where the hook hangs.
CRANE = (20.0, 2.0)
MAST_TOP = 34.0
HOOK = (-2.0, 2.0)

# What the previews show (odpreview.py): the site, and the shots. Camera
# heights for the landed views are those of a Quad on its Launch Spot: the
# Whoop 65's camera 3.5 cm up and the Freestyle 5"'s 7 cm, both tilted 30
# degrees up (#14, #17).
PREVIEW = {
    "core": (-35.0, -25.0, 35.0, 25.0),
    "three_quarter": {"azimuth": 205.0, "elevation": 26.0, "distance": 92.0, "target": (3.0, 0.0, 14.0)},
    "figures": [(-7.0, -12.5, 0.0), (-9.0, -3.0, 0.15), (24.0, -11.0, 0.0), (0.0, -18.5, 0.0)],
    "landed": [("whoop-65", 0.035), ("freestyle-5", 0.07)],
    "flights": [
        ("whoop-65-in-a-ground-floor-room", (-11.2, -4.9, 1.1), 90.0, 0.0),
        ("freestyle-5-to-the-smashed-gap", (-3.6, 1.8, 1.3), 180.0, -2.0),
        ("whoop-65-between-the-columns-on-floor-2", (7.0, 4.6, 8.0), 270.0, -6.0),
        ("freestyle-5-past-the-crane", (26.0, -9.0, 31.0), 320.0, -35.0),
        ("freestyle-5-down-the-lift-shaft", (1.5, 0.0, 20.5), 0.0, -88.0),
    ],
}


def windows(centres, width=1.5, height=1.4, sill=0.9):
    """Window openings `(u0, u1, v0, v1)` centred `centres` metres along a
    wall: 1.5 x 1.4 m, their sills 0.9 m up."""
    return [(c - width / 2.0, c + width / 2.0, sill, sill + height) for c in centres]


def door(u, width=0.9, height=2.1):
    """A doorway starting `u` metres along a wall: 0.9 x 2.1 m."""
    return (u, u + width, 0.0, height)


def frame(m):
    """The concrete frame: floors with holes, columns on a 6 x 5 m grid,
    stairs through matching openings, the open lift shaft and the collapsed
    bay."""
    # Alternating stair lanes in the east bay (x from, to), and the holes in
    # each floor: 2-5 m across, a few per floor, in different places.
    lanes = {0: (6.4, 8.2), 1: (9.4, 11.2)}
    broken = {
        1: [(-10.5, -6.5, -7.5, -4.0)],
        2: [(-4.5, 3.0, -1.5, 6.5), (-11.4, 2.7, -6.4, 7.1)],
        3: [(-10.0, 0.5, -7.0, 4.0), (3.5, -6.8, 5.5, -4.5)],
        4: [(-5.5, -3.0, -1.0, -0.5)],
        5: [(-9.5, 3.0, -6.5, 6.0), (3.5, 3.5, 5.5, 6.5)],
    }
    shaft = (0.4, -1.1, 2.6, 1.1)  # the lift shaft, 2.2 m square

    m.slab("ground_slab", X0 - 0.5, Y0 - 0.5, X1 + 0.5, Y1 + 0.5, Z[0], 0.3, "concrete_structure")
    for k in range(1, 6):
        lane = lanes[(k - 1) % 2]
        holes = [(lane[0] - 0.1, -7.25, lane[1] + 0.1, -2.45), shaft] + broken.get(k, [])
        m.slab(f"floor_{k}", X0, Y0, X1, Y1, Z[k], SLAB, "concrete_structure", holes=holes)

    rise, run = FH / 17.0, 0.27
    for k in range(5):
        top = Z[k + 1] - SLAB
        for x in (-11.8, -6.0, 0.0, 6.0, 11.8):
            for y in (-7.3, -2.5, 2.5, 7.3):
                m.box_h("column", x, y, 0.4, 0.4, Z[k], top, "concrete_structure")
        # One flight per floor, 17 steps of 0.197 m, climbing north through
        # the openings of the floor above, with a 6 cm handrail down each
        # side, at 0.9 m above the noses of the steps.
        lane = lanes[k % 2]
        m.stairs("stairs", ((lane[0] + lane[1]) / 2.0, -7.1, Z[k]), 0, lane[1] - lane[0], 17, rise, run, "concrete_structure")
        for x in (lane[0] + 0.06, lane[1] - 0.06):
            a0, a1 = 0.5, 16.0 * run
            z_at = lambda a: Z[k] + rise + 0.9 + a * rise / run  # noqa: E731
            m.rail("handrail", (x, -7.1 + a0, z_at(a0)), (x, -7.1 + a1, z_at(a1)), r=0.03, posts=4, drop=1.0)

    # The lift shaft: two concrete walls, open east and west, open to the sky
    # and down to the ground slab, a 17 m dive from the roof.
    for y in (-1.2, 1.2):
        m.wall("shaft_wall", (0.4, y), (2.6, y), Z[0], Z[5] + 1.2 - Z[0], 0.2, "concrete_dark")

    # The collapsed bay: floor 2's slab fell and rests on floor 1, a ramp
    # from floor 1 at y = 2.9 up to floor 2's edge at y = 7.0.
    low, high = Z[1] + SLAB / 2.0, Z[2] - SLAB / 2.0
    rise_y, rise_z = 7.0 - 2.9, high - low
    m.box(
        "collapsed_slab",
        (4.8, math.hypot(rise_y, rise_z), SLAB),
        (-8.9, (7.0 + 2.9) / 2.0, (high + low) / 2.0),
        "concrete_structure",
        tilt=(math.degrees(math.atan2(rise_z, rise_y)), 0.0),
    )


def brick(m):
    """Brick on the two lowest floors, west half: four 6 x 5 m rooms a floor
    off a 5 m corridor, 0.9 x 2.1 m doorways, one smashed gap of about 2 m per
    floor, and outside walls with 1.5 x 1.4 m windows. The upper three floors
    stay open."""
    h = CLEAR
    south = windows([3.0, 9.0])  # rooms A and B, from x = -12 going east
    north = windows([3.0, 9.0, 15.0, 21.0])  # the whole north wall
    west = windows([2.5, 6.0, 9.0, 12.5])  # from the south end
    east = windows([2.5, 6.0, 9.0, 12.5])
    for k in (0, 1):
        z = Z[k]
        # The outside walls, 0.25 m thick, 1.5 cm inside the slab's edges so
        # that no face is level with a column's (two faces level with each
        # other are drawn as flicker).
        m.wall("brick_outside", (X0, Y0 + 0.14), (0.0, Y0 + 0.14), z, h, 0.25, "brick", openings=south)
        m.wall("brick_outside", (X0, Y1 - 0.14), (X1, Y1 - 0.14), z, h, 0.25, "brick", openings=north)
        # The west wall: at the corridor's end a doorway out, 0.9 x 2.1 m, on
        # the ground floor.
        west_openings = west + ([door(7.05)] if k == 0 else [])
        m.wall("brick_outside", (X0 + 0.14, Y0), (X0 + 0.14, Y1), z, h, 0.25, "brick", openings=west_openings)
        if k == 1:
            m.wall("brick_outside", (X1 - 0.14, Y0), (X1 - 0.14, Y1), z, h, 0.25, "brick", openings=east)
        # The rooms: A and B south of the corridor, C and D north of it.
        south_wall = [door(2.0), (7.0, 9.0, 0.0, 2.4)] if k == 0 else [door(2.0), door(7.6)]
        cross_south = [door(1.75)] if k == 0 else [(1.2, 3.0, 0.0, 2.2)]
        m.wall("brick_room", (-11.6, -2.5), (-0.2, -2.5), z, h, 0.2, "brick", openings=south_wall)
        m.wall("brick_room", (-11.6, 2.5), (-0.2, 2.5), z, h, 0.2, "brick", openings=[door(2.0), door(7.6)])
        m.wall("brick_room", (-6.0, -7.1), (-6.0, -2.7), z, h, 0.2, "brick", openings=cross_south)
        m.wall("brick_room", (-6.0, 2.7), (-6.0, 7.1), z, h, 0.2, "brick", openings=[door(1.75)])


def roof(m):
    """A parapet with Gaps, a water tank on legs, rebar stubs."""
    z = Z[5]
    # South: a 3.5 m gap; north: a 3 m gap; east: a gap only the whoop fits.
    south_gap = [(14.0, 17.5, 0.0, 1.0)]
    north_gap = [(3.0, 6.0, 0.0, 1.0)]
    east_gap = [(3.9, 4.7, 0.0, 1.0)]
    m.wall("parapet", (X0 - 0.1, Y0 - 0.1), (X1 + 0.1, Y0 - 0.1), z, 1.0, 0.2, "concrete_structure", openings=south_gap)
    m.wall("parapet", (X1 + 0.1, Y1 + 0.1), (X0 - 0.1, Y1 + 0.1), z, 1.0, 0.2, "concrete_structure", openings=north_gap)
    m.wall("parapet", (X1 + 0.1, Y0 - 0.1), (X1 + 0.1, Y1 + 0.1), z, 1.0, 0.2, "concrete_structure", openings=east_gap)
    m.wall("parapet", (X0 - 0.1, Y1 + 0.1), (X0 - 0.1, Y0 - 0.1), z, 1.0, 0.2, "concrete_structure")
    for dx in (-0.8, 0.8):
        for dy in (-0.8, 0.8):
            m.box_h("tank_leg", -9.0 + dx, -5.0 + dy, 0.15, 0.15, z, z + 1.2, "steel_rusty")
    m.cylinder("water_tank", (-9.0, -5.0, z + 1.2), (-9.0, -5.0, z + 3.2), 1.1, "steel_rusty", segments=14)
    # Rebar stubs, 3 cm thick and 1 m tall, four to a column top.
    for cx, cy in ((-6.0, 2.5), (0.0, -2.5), (6.0, 2.5), (11.8, -7.3)):
        for ox, oy in ((-0.12, -0.12), (0.12, -0.12), (-0.12, 0.12), (0.12, 0.12)):
            m.cylinder("rebar", (cx + ox, cy + oy, z), (cx + ox, cy + oy, z + 1.0), 0.015, "steel_rusty", segments=8)


def crane(m):
    """The tower crane: a 34 m lattice mast with 1.3 m openings, a 35 m jib
    over the building, a counter-jib, and a hook hanging at 20 m on a 5 cm
    cable."""
    cx, cy = CRANE
    top = MAST_TOP
    s = 0.75  # the chords are 0.2 m square, so 1.3 m between them
    corners = [(cx - s, cy - s), (cx + s, cy - s), (cx + s, cy + s), (cx - s, cy + s)]
    for x, y in corners:
        m.beam("crane_chord", (x, y, 0.0), (x, y, top), 0.2, 0.2, "steel_painted")
    levels = [i * 2.4 for i in range(int(top / 2.4) + 1)]
    for i, z in enumerate(levels):
        for a, b in zip(corners, corners[1:] + corners[:1]):
            m.beam("crane_brace", (*a, z), (*b, z), 0.08, 0.08, "steel_painted")
            if i + 1 < len(levels):
                p, q = (a, b) if i % 2 else (b, a)
                m.beam("crane_diagonal", (*p, z), (*q, levels[i + 1]), 0.06, 0.06, "steel_painted")
    m.box_h("crane_cab", cx, cy, 2.2, 2.2, top, top + 2.0, "steel_painted")
    jz = top + 2.0
    j0, j1 = cx - 1.1, -15.0  # the jib, 35 m from the mast to its tip
    for dy in (-0.6, 0.6):
        m.beam("jib_chord", (j0, cy + dy, jz), (j1, cy + dy, jz), 0.12, 0.12, "steel_painted")
        m.beam("counter_jib_chord", (cx + 1.1, cy + dy, jz), (cx + 11.0, cy + dy, jz), 0.14, 0.14, "steel_painted")
    m.beam("jib_top", (j0, cy, jz + 1.4), (j1, cy, jz + 1.4), 0.12, 0.12, "steel_painted")
    x = j0
    while x > j1 + 0.1:
        nx = max(j1, x - 2.5)
        for dy in (-0.6, 0.6):
            m.beam("jib_diagonal", (x, cy + dy, jz), (nx, cy, jz + 1.4), 0.06, 0.06, "steel_painted")
        m.beam("jib_rung", (nx, cy - 0.6, jz), (nx, cy + 0.6, jz), 0.06, 0.06, "steel_painted")
        x = nx
    m.box_h("counterweight", cx + 9.5, cy, 2.0, 2.2, jz - 2.2, jz, "concrete_dark")
    m.beam("crane_peak", (cx, cy, jz), (cx, cy, jz + 5.0), 0.3, 0.3, "steel_painted")
    m.cylinder("jib_tie", (cx, cy, jz + 5.0), (j1 + 8.0, cy, jz + 1.4), 0.03, "steel_grey", segments=6)
    m.cylinder("counter_jib_tie", (cx, cy, jz + 5.0), (cx + 11.0, cy, jz), 0.03, "steel_grey", segments=6)
    hx, hy = HOOK
    m.box_h("trolley", hx, hy, 1.2, 1.4, jz - 0.5, jz - 0.05, "steel_grey")
    # The hook cable: 5 cm across, a straight cylinder from the trolley down to
    # the hook block, whose bottom hangs at 20 m.
    m.cylinder("hook_cable", (hx, hy, jz - 0.5), (hx, hy, 20.6), 0.025, "steel_grey", segments=12)
    m.box_h("hook_block", hx, hy, 0.5, 0.5, 20.0, 20.6, "steel_painted")


def yard(m):
    """Shipping containers (one stacked), rubble, a site office, a dirt mound
    and the site fence, all clear of the Launch Spot."""
    m.box_h("container", 25.0, -15.0, 2.44, 6.06, 0.0, 2.59, "container_blue")
    m.box_h("container", 28.2, -15.0, 2.44, 6.06, 0.0, 2.59, "container_red")
    m.box_h("container", 26.6, -15.5, 2.44, 6.06, 2.59, 5.18, "container_blue", heading=90.0)
    for name, (x, y, radius, count, seed) in {"rubble_a": (-20.0, 8.0, 4.0, 40, 1), "rubble_b": (6.0, 13.0, 3.0, 25, 2), "rubble_c": (-12.0, -11.0, 2.5, 18, 3)}.items():
        rng = random.Random(seed)
        for _ in range(count):
            angle, d = rng.uniform(0.0, 2.0 * math.pi), radius * math.sqrt(rng.random())
            size = 0.8 * (1.0 - 0.6 * d / radius)
            sx, sy, sz = (size * rng.uniform(0.4, 1.2) for _ in range(3))
            m.box(
                name,
                (sx, sy, sz),
                (x + d * math.cos(angle), y + d * math.sin(angle), sz * 0.3),
                "rubble",
                heading=rng.uniform(0.0, 180.0),
                tilt=(rng.uniform(-25.0, 25.0), rng.uniform(-25.0, 25.0)),
            )
    m.box_h("site_office", -24.0, -16.0, 6.0, 2.5, 0.0, 2.7, "metal_sheet")
    m.cylinder("dirt_mound", (-25.0, 15.0, 0.0), (-25.0, 15.0, 2.0), 5.0, "dirt", segments=14, r1=1.5)
    # The site fence: posts every 2.5 m and two 6 cm rails, the top one at 2 m.
    corners = [(-3.0, -24.0), (-34.0, -24.0), (-34.0, 24.0), (34.0, 24.0), (34.0, -24.0), (3.0, -24.0)]
    for a, b in zip(corners, corners[1:]):
        n = max(1, round(math.hypot(b[0] - a[0], b[1] - a[1]) / 2.5))
        for i in range(n):
            x, y = a[0] + (b[0] - a[0]) * i / n, a[1] + (b[1] - a[1]) * i / n
            m.cylinder("fence_post", (x, y, 0.0), (x, y, 2.0), 0.035, "steel_grey", segments=6)
        for z in (1.0, 2.0):
            m.cylinder("fence_rail", (a[0], a[1], z), (b[0], b[1], z), 0.03, "steel_grey", segments=8)
    x, y = corners[-1]
    m.cylinder("fence_post", (x, y, 0.0), (x, y, 2.0), 0.035, "steel_grey", segments=6)


def backdrop(m):
    """What stands round the site, all solid, and nothing worth flying to past
    about 100 m: city blocks, a row of trees, and ground running to the
    horizon."""
    for x, y, sx, sy, h in (
        (-70, 10, 30, 40, 24),
        (75, -5, 26, 50, 30),
        (0, 70, 60, 20, 18),
        (-20, -70, 50, 20, 12),
        (45, -60, 25, 25, 40),
        (-55, 55, 24, 24, 20),
        (60, 50, 30, 22, 26),
        (-75, -45, 22, 30, 16),
        (90, -50, 20, 24, 22),
    ):
        m.box_h("city_block", x, y, sx, sy, 0.0, h, "backdrop")
    for x in range(-40, 41, 10):
        m.tree("tree", x, 30.0, 10.0)
    for y in range(-20, 21, 20):
        m.tree("tree", -40.0, y, 9.0)
        m.tree("tree", 40.0, y + 5.0, 11.0)
    # The ground, to the horizon: a slab 2 km square round the site, and the
    # site's own dirt, level with it.
    core = PREVIEW["core"]
    m.slab("ground", -1000, -1000, 1000, 1000, 0.0, 0.3, "concrete_plaza", holes=[core])
    m.box_h("ground_site", 0.0, 0.0, 70.0, 50.0, -0.3, 0.0, "dirt")


def build(out):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    frame(m)
    brick(m)
    roof(m)
    crane(m)
    yard(m)
    backdrop(m)
    # The Launch Spot: in the yard, 17 m south of the building, facing it.
    m.launch_spot(*LAUNCH[:3], LAUNCH[3])
    m.export(out)
    return m


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    builder = build(out)
    print("bando:", builder.stats())
    if "--previews" in argv:
        import odpreview

        odpreview.render(builder, PREVIEW, LAUNCH, argv[argv.index("--previews") + 1])
