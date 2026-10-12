"""The Skate Park, "Bowl Park + Street Corner" (#17, #63): built in Blender.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/maps/skate-park/build.py -- --out <file.glb> [--previews <folder>]

`cargo xtask maps` does that. An outdoor concrete park, 62 x 40 m, next to a
quiet street, with houses to the north and trees all round. Most of it is
sunk into the ground. A street corner on the east side adds things to fly
through above head height. East is +x, north is +y, up is +z; the park's
origin is its deck level (0 m), a little west of its middle.

Every size below is the one in #17's table of must-have features: see
`crates/pack/tests/skate_park.rs`, which measures them in the finished Map.
"""
import math
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
from odmap import MapBuilder  # noqa: E402

LAUNCH = (-1.8, 6.0, 0.0, 270.0)  # x, y, ground height, compass heading

# What the previews show (odpreview.py): the park's flyable core, and the
# shots. Camera heights are those of a Quad landed on its Launch Spot:
# the Whoop 65's camera 3.5 cm up and the Freestyle 5"'s 7 cm, both tilted
# 30 degrees up (#14, #17).
PREVIEW = {
    "core": (-28.0, -20.0, 34.0, 20.0),
    "three_quarter": {"azimuth": 200.0, "elevation": 38.0, "distance": 78.0, "target": (3.0, 0.0, 2.0)},
    "figures": [(-2.5, 4.5, 0.0), (9.0, -2.0, 0.0), (-12.0, -14.0, -0.6), (18.0, -1.5, 0.0), (31.0, 9.0, 1.2)],
    "landed": [("whoop-65", 0.035), ("freestyle-5", 0.07)],
    "flights": [
        ("whoop-65-in-the-pool-bowl", (-15.5, 3.2, -0.7), 70.0, 6.0),
        ("freestyle-5-into-the-snake-run", (-27.0, -9.0, 5.0), 120.0, -28.0),
        ("freestyle-5-to-the-ring", (6.0, 7.0, 2.2), 90.0, 0.0),
        ("whoop-65-between-the-handrails", (22.5, 11.0, 0.7), 90.0, 2.0),
    ],
}


def smoothstep(e0, e1, x):
    t = min(1.0, max(0.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


def profile(d, depth, r):
    """How far below the deck, at `d` metres inside a bowl's edge: a vertical
    wall down to `depth - r`, then a round transition of radius `r`, then a
    flat bottom `depth` down."""
    if d <= 0 or depth <= 0.01:
        return 0.0
    r = min(r, depth)
    if d >= r:
        return -depth
    return -depth + r - math.sqrt(max(0.0, r * r - (r - d) ** 2))


def rounded_rect_inside(x, y, cx, cy, hx, hy, rc):
    qx, qy = abs(x - cx) - (hx - rc), abs(y - cy) - (hy - rc)
    out = math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - rc
    return -out


def rounded_rect_loop(cx, cy, hx, hy, rc, n=10):
    pts = []
    for sx, sy, a0 in ((1, 1, 0), (-1, 1, 90), (-1, -1, 180), (1, -1, 270)):
        ox, oy = cx + sx * (hx - rc), cy + sy * (hy - rc)
        for i in range(n + 1):
            a = math.radians(a0 + 90 * i / n)
            pts.append((ox + rc * math.cos(a), oy + rc * math.sin(a)))
    return pts


def polyline_closest(pts, x, y):
    best, s_best, s_acc = 1e9, 0.0, 0.0
    for (ax, ay), (bx, by) in zip(pts, pts[1:]):
        dx, dy = bx - ax, by - ay
        l2 = dx * dx + dy * dy
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / l2))
        px, py = ax + t * dx, ay + t * dy
        d = math.hypot(x - px, y - py)
        if d < best:
            best, s_best = d, s_acc + t * math.sqrt(l2)
        s_acc += math.sqrt(l2)
    return best, s_best, s_acc


def street_corner(m):
    """The street corner on the east side: a terrace at +1.2 m with a stair
    set, two handrails, a hubba and a bank; the pergola and the ring."""
    m.box_h("terrace", 31.5, 11, 5, 18, 0.0, 1.2, "concrete_plaza", collider="box")
    m.stairs("stair_set", (26.9, 11, 0), 90, 6.0, 6, 0.2, 0.35, "concrete_plaza")
    for y in (9.4, 12.6):
        m.rail("handrail", (26.5, y, 0.871), (29.0, y, 2.30), drop=0.95, posts=3)
    m.prism("hubba", [(-0.3, 0), (-0.3, 0.45), (2.1, 1.7), (2.1, 0)], 0.7, (26.9, 14.35, 0), 90, "concrete_smooth")
    m.bank("bank_to_terrace", (25.4, 4.5), 90, 4.0, 3.6, 1.2, "concrete_smooth")
    m.railing("terrace_railing", [(29.05, 6.6), (29.05, 7.9)], 1.2)
    m.railing("terrace_railing", [(29.05, 15.0), (29.05, 19.9)], 1.2)
    m.railing("terrace_railing", [(29.0, 2.05), (33.9, 2.05)], 1.2)
    for y in (5.0, 17.5):
        m.box_h("planter", 32.2, y, 2, 2, 1.2, 1.7, "concrete_smooth")
        m.tree("plaza_tree", 32.2, y, 7.0, z=1.7)
    # The pergola: slats at 3.6 m with 0.6 m Gaps between them.
    px, py = 19.0, 15.5
    for x in (px - 4.7, px, px + 4.7):
        for y in (py - 2.7, py + 2.7):
            m.box_h("pergola_column", x, y, 0.35, 0.35, 0, 3.3, "concrete_structure")
    for y in (py - 2.7, py + 2.7):
        m.box_h("pergola_beam", px, y, 10, 0.35, 3.3, 3.6, "concrete_structure")
    x = px - 4.85
    while x <= px + 4.9:
        m.box_h("pergola_slat", x, py, 0.3, 6.4, 3.6, 3.85, "concrete_structure")
        x += 0.9
    # The ring, flown through east to west between the bowls and the stairs:
    # a 3.1 m opening, its centre 2.2 m up.
    radius, cz = 1.75, 2.2
    m.box_h("ring_plinth", 18.5, 7, 0.6, 1.2, 0, 0.35, "concrete_structure")
    m.tube(
        "ring",
        [(18.5, 7 + radius * math.cos(t), cz + radius * math.sin(t)) for t in (2 * math.pi * i / 48 for i in range(48))],
        0.2,
        "steel_painted",
        segments=10,
        closed_loop=True,
    )


def backdrop(m):
    """What stands round the park, all solid, and nothing worth flying to past
    about 100 m: a quiet street to the north with houses along it, blocks of
    flats further out, trees all round, and ground running to the horizon."""
    m.box("street", (400, 8, 0.2), (0, 30, -0.14), "asphalt")
    for i, x in enumerate(range(-60, 61, 15)):
        m.box_h("house", x, 44, 10, 9, 0.0, 7 + (i % 3) * 2.5, "backdrop")
    for x, y, sx, sy, h in (
        (-60, -45, 30, 18, 12),
        (10, -50, 40, 20, 9),
        (60, -40, 20, 20, 16),
        (-75, 5, 18, 40, 11),
        (78, 10, 18, 44, 14),
        (-30, 70, 40, 16, 15),
        (45, 72, 36, 16, 10),
    ):
        m.box_h("block", x, y, sx, sy, 0.0, h, "backdrop")
    for x in range(-26, 34, 7):
        m.tree("tree", x, 23.5, 8.0)
        m.tree("tree", x + 3, -24, 9.0)
    for y in range(-16, 20, 8):
        m.tree("tree", -33, y, 10.0)
        m.tree("tree", 39, y + 4, 10.0)
    for x, y, h in ((-48, 20, 11), (-44, -12, 9), (52, 26, 12), (55, -14, 10), (62, 2, 11), (-52, 6, 10)):
        m.tree("tree", x, y, h, kind="conifer")
    # The ground, to the horizon: a slab 2 km square round the park's deck.
    core = PREVIEW["core"]
    m.slab("ground", -1000, -1000, 1000, 1000, -0.05, 0.3, "grass", holes=[core])


def build(out):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    core = PREVIEW["core"]
    east = core[2]

    # The deep pool bowl: 16 x 10 m, 1.8 m deep at the shallow (west) end and
    # 3.2 m at the deep (east) end, 0.6 m of vertical wall, steel coping.
    pool = dict(cx=-12, cy=6, hx=8, hy=5, rc=4)
    pool_patch = (-20.6, 0.4, -3.4, 11.6)

    def pool_depth(x, y):
        d = rounded_rect_inside(x, y, pool["cx"], pool["cy"], pool["hx"], pool["hy"], pool["rc"])
        depth = 1.8 + 1.4 * smoothstep(-17, -7, x)
        return profile(d, depth, depth - 0.6)

    m.heightfield("pool_bowl", *pool_patch, 0.2, pool_depth, "concrete_smooth")
    loop = rounded_rect_loop(pool["cx"], pool["cy"], pool["hx"] + 0.02, pool["hy"] + 0.02, pool["rc"] + 0.02)
    m.tube("pool_coping", [(x, y, -0.01) for x, y in loop], 0.04, "steel_painted", segments=8, closed_loop=True)

    # The cradle: round, 10 m across, 1.9 m deep, all transition.
    cradle = (7, 8, 5.0)
    cradle_patch = (1.4, 2.4, 12.6, 13.6)
    m.heightfield(
        "cradle_bowl",
        *cradle_patch,
        0.2,
        lambda x, y: profile(cradle[2] - math.hypot(x - cradle[0], y - cradle[1]), 1.9, 1.9),
        "concrete_smooth",
    )
    m.tube(
        "cradle_coping",
        [
            (cradle[0] + (cradle[2] + 0.02) * math.cos(t), cradle[1] + (cradle[2] + 0.02) * math.sin(t), -0.01)
            for t in (2 * math.pi * i / 64 for i in range(64))
        ],
        0.04,
        "steel_painted",
        segments=8,
        closed_loop=True,
    )

    # The snake run: 4.4 m wide, up to 1.5 m deep, ramping in at both ends.
    snake = [(-25, -16), (-15, -9.5), (-5, -15.5), (5, -9.5), (15, -15.5), (25, -10)]
    snake_patch = (-27.8, -18.8, 27.8, -6.4)

    def snake_depth(x, y):
        dist, s, total = polyline_closest(snake, x, y)
        depth = 1.5 * smoothstep(0, 7, s) * smoothstep(0, 7, total - s)
        return profile(2.2 - dist, depth, min(1.6, depth))

    m.heightfield("snake_run", *snake_patch, 0.25, snake_depth, "concrete_smooth")

    # The flat deck round the sunken parts, a slab 0.3 m thick.
    m.slab("deck", core[0], core[1], core[2], core[3], 0.0, 0.3, "concrete_plaza", holes=[pool_patch, cradle_patch, snake_patch])

    # The full pipe, 3.6 m across inside and 7 m long; a pyramid, benches, a
    # ledge and a shade shelter on the deck.
    m.annulus("full_pipe", (15, 0, 1.8), (22, 0, 1.8), 1.8, 2.05, "concrete_smooth")
    m.cylinder("pyramid", (-1, -1.5, 0), (-1, -1.5, 1.0), 3.6, "concrete_smooth", segments=4, r1=1.4)
    m.box_h("bench", -24.5, -2, 5, 0.5, 0, 0.45, "concrete_smooth", heading=90)
    m.box_h("bench", 25.5, -3, 5, 0.5, 0, 0.45, "concrete_smooth", heading=90)
    m.box_h("ledge", 8, -2, 6, 0.6, 0, 0.5, "concrete_smooth")
    for x in (-23.8, -16.2):
        for y in (14.2, 17.8):
            m.box_h("shelter_post", x, y, 0.2, 0.2, 0, 3.0, "steel_grey")
    m.box_h("shelter_roof", -20, 16, 8.6, 4.6, 3.0, 3.2, "metal_sheet")
    street_corner(m)

    # Light poles, 9 m, on the corners and the long sides.
    for x, y, z in ((-27.5, -19.5, 0), (east - 0.5, -19.5, 0), (-27.5, 19.5, 0), (0, 19.5, 0), (0, -19.5, 0), (east - 0.5, 19.5, 1.2)):
        m.light_pole("light_pole", x, y, 9.0, z=z, heading=180 if y > 0 else 0)

    backdrop(m)

    # The Launch Spot: the flat deck between the two bowls, facing west into
    # the pool bowl.
    m.launch_spot(*LAUNCH[:3], LAUNCH[3])
    m.export(out)
    return m


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    builder = build(out)
    print("skate-park:", builder.stats())
    if "--previews" in argv:
        import odpreview

        odpreview.render(builder, PREVIEW, LAUNCH, argv[argv.index("--previews") + 1])
