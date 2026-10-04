"""PROTOTYPE (#17). Skate Park B: Concrete Bowl Park. Sunken transitions you carve and dive into.
With street_corner=True it builds the Round 2 merge (B plus A's street corner), see skate_park_ba.py."""
import math
from blockout_lib import Map, rect_subtract


def profile(d, H, r):
    """Depth below the deck at inside distance d from a bowl's edge: a short vertical wall, then a
    round transition of radius r, then a flat bottom at -H."""
    if d <= 0 or H <= 0.01:
        return 0.0
    r = min(r, H)
    if d >= r:
        return -H
    return -H + r - math.sqrt(max(0.0, r * r - (r - d) ** 2))


def smoothstep(e0, e1, x):
    t = min(1.0, max(0.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


def rounded_rect_inside(x, y, cx, cy, hx, hy, rc):
    qx, qy = abs(x - cx) - (hx - rc), abs(y - cy) - (hy - rc)
    out = math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - rc
    return -out


def rounded_rect_loop(cx, cy, hx, hy, rc, n=10):
    pts = []
    for k, (sx, sy, a0) in enumerate(((1, 1, 0), (-1, 1, 90), (-1, -1, 180), (1, -1, 270))):
        ox, oy = cx + sx * (hx - rc), cy + sy * (hy - rc)
        for i in range(n + 1):
            a = math.radians(a0 + 90 * i / n)
            pts.append((ox + rc * math.cos(a), oy + rc * math.sin(a)))
    return pts


def polyline_closest(pts, x, y):
    best, s_best, s_acc = 1e9, 0.0, 0.0
    for (ax, ay), (bx, by) in zip(pts, pts[1:]):
        dx, dy = bx - ax, by - ay
        L2 = dx * dx + dy * dy
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / L2))
        px, py = ax + t * dx, ay + t * dy
        d = math.hypot(x - px, y - py)
        if d < best:
            best, s_best = d, s_acc + t * math.sqrt(L2)
        s_acc += math.sqrt(L2)
    return best, s_best, s_acc


def street_corner(m):
    """Skate Park A's street corner, moved onto B's east side (Round 2 merge)."""
    # Terrace at +1.2 m along the east edge, with A's stair set, handrails, hubba and bank
    m.box_h("terrace", 31.5, 11, 5, 18, 0.0, 1.2, "concrete_plaza")
    m.stairs("stair_set", (26.9, 11, 0), 90, 6.0, 6, 0.2, 0.35, "concrete_plaza")
    for y in (9.4, 12.6):
        m.rail("handrail", (26.5, y, 0.871), (29.0, y, 2.30), drop=0.95, posts=3)
    m.prism("hubba", [(-0.3, 0), (-0.3, 0.45), (2.1, 1.7), (2.1, 0)], 0.7, (26.9, 14.35, 0), 90, "concrete_smooth")
    m.bank("bank_to_terrace", (25.4, 4.5), 90, 4.0, 3.6, 1.2)
    m.railing("terrace_railing", [(29.05, 6.6), (29.05, 7.9)], 1.2)
    m.railing("terrace_railing", [(29.05, 15.0), (29.05, 19.9)], 1.2)
    m.railing("terrace_railing", [(29.0, 2.05), (33.9, 2.05)], 1.2)
    for y in (5.0, 17.5):
        m.box_h("planter", 32.2, y, 2, 2, 1.2, 1.7, "concrete_smooth")
        m.tree("plaza_tree", 32.2, y, 7.0, z=1.7, core=True)
    # Pergola with 0.6 m gaps between its slats
    PX, PY = 19.0, 15.5
    for x in (PX - 4.7, PX, PX + 4.7):
        for y in (PY - 2.7, PY + 2.7):
            m.box_h("pergola_column", x, y, 0.35, 0.35, 0, 3.3, "concrete_structure")
    for y in (PY - 2.7, PY + 2.7):
        m.box_h("pergola_beam", PX, y, 10, 0.35, 3.3, 3.6, "concrete_structure")
    x = PX - 4.85
    while x <= PX + 4.9:
        m.box_h("pergola_slat", x, PY, 0.3, 6.4, 3.6, 3.85, "concrete_structure")
        x += 0.9
    # Ring sculpture, flown through east-west between the bowls and the stairs
    R, cz = 1.75, 2.15
    m.box_h("ring_plinth", 18.5, 7, 0.6, 1.2, 0, 0.35, "concrete_structure")
    m.tube("ring_sculpture", [(18.5, 7 + R * math.cos(t), cz + R * math.sin(t))
                              for t in (2 * math.pi * i / 48 for i in range(48))], 0.2, "steel_painted",
           seg=10, closed_loop=True)


def build(merged=False):
    if merged:
        m = Map("skate_park_ba", "B+A · Bowl Park + Street Corner", "Skate Park",
                "Round 2: variant B's sunken bowls, snake run and full pipe, plus A's street corner on the east side: "
                "a terrace with a stair set and handrails, a hubba and a bank, the pergola and the ring sculpture.")
        m.core = (-28, -20, 34, 20)
    else:
        m = Map("skate_park_b", "B · Concrete Bowl Park", "Skate Park",
                "A sunken concrete park: a deep pool bowl, a round cradle, a long winding snake run and a "
                "full pipe. Everything is carved into the ground, so lines flow down and up rather than over.")
        m.core = (-28, -20, 28, 20)
    m.ground(holes=[m.core])
    E = m.core[2]

    # Deep pool bowl: 16 x 10 m, shallow end 1.8 m (west) to deep end 3.2 m (east)
    B = dict(cx=-12, cy=6, hx=8, hy=5, rc=4)
    bowl_patch = (-20.6, 0.4, -3.4, 11.6)

    def bowl_z(x, y):
        d = rounded_rect_inside(x, y, B["cx"], B["cy"], B["hx"], B["hy"], B["rc"])
        H = 1.8 + 1.4 * smoothstep(-17, -7, x)
        return profile(d, H, min(2.6, 0.85 * H))

    m.heightfield("pool_bowl", *bowl_patch, 0.2, bowl_z, "concrete_smooth")
    loop = rounded_rect_loop(B["cx"], B["cy"], B["hx"] + 0.02, B["hy"] + 0.02, B["rc"] + 0.02)
    m.tube("pool_coping", [(x, y, -0.01) for x, y in loop], 0.04, "steel_painted", seg=8, closed_loop=True)

    # Cradle: round bowl, 10 m across, 1.9 m deep
    C = (7, 8, 5.0)
    cradle_patch = (1.4, 2.4, 12.6, 13.6)
    m.heightfield("cradle_bowl", *cradle_patch, 0.2,
                  lambda x, y: profile(C[2] - math.hypot(x - C[0], y - C[1]), 1.9, 1.9), "concrete_smooth")
    m.tube("cradle_coping", [(C[0] + (C[2] + 0.02) * math.cos(t), C[1] + (C[2] + 0.02) * math.sin(t), -0.01)
                             for t in (2 * math.pi * i / 64 for i in range(64))], 0.04, "steel_painted",
           seg=8, closed_loop=True)

    # Snake run: 4.4 m wide, up to 1.5 m deep, ramps in and out at the ends
    snake = [(-25, -16), (-15, -9.5), (-5, -15.5), (5, -9.5), (15, -15.5), (25, -10)]
    snake_patch = (-27.8, -18.8, 27.8, -6.4)

    def snake_z(x, y):
        dist, s, L = polyline_closest(snake, x, y)
        H = 1.5 * smoothstep(0, 7, s) * smoothstep(0, 7, L - s)
        return profile(2.2 - dist, H, min(1.6, H))

    m.heightfield("snake_run", *snake_patch, 0.25, snake_z, "concrete_smooth")

    # Flat deck around the patches
    for i, (x0, y0, x1, y1) in enumerate(rect_subtract(m.core, [bowl_patch, cradle_patch, snake_patch])):
        m.slab("deck", x0, y0, x1, y1, 0.0, 0.3, "concrete_plaza")

    # Full pipe, pyramid, benches, shelter (the volcano only without the street corner)
    m.annulus("full_pipe", (15, 0, 1.8), (22, 0, 1.8), 1.8, 2.05, "concrete_smooth")
    if not merged:
        m.cyl("volcano", (21, 13, 0), (21, 13, 1.6), 4.5, "concrete_smooth", seg=20, r1=1.5, collider="convex")
    m.cyl("pyramid", (-1, -1.5, 0), (-1, -1.5, 1.0), 3.6, "concrete_smooth", seg=4, r1=1.4, smooth=False)
    m.box_h("bench", -24.5, -2, 5, 0.5, 0, 0.45, "concrete_smooth", heading=90)
    m.box_h("bench", 25.5, -3, 5, 0.5, 0, 0.45, "concrete_smooth", heading=90)
    m.box_h("ledge", 8, -2, 6, 0.6, 0, 0.5, "concrete_smooth")
    for x in (-23.8, -16.2):
        for y in (14.2, 17.8):
            m.box_h("shelter_post", x, y, 0.2, 0.2, 0, 3.0, "steel_grey")
    m.box_h("shelter_roof", -20, 16, 8.6, 4.6, 3.0, 3.2, "metal_sheet")
    if merged:
        street_corner(m)

    poles = [(-27.5, -19.5, 0), (E - 0.5, -19.5, 0), (-27.5, 19.5, 0), (0, 19.5, 0), (0, -19.5, 0),
             (E - 0.5, 19.5, 1.2 if merged else 0)]
    for (x, y, z) in poles:
        m.light_pole("light_pole", x, y, 9.0, z=z, heading=180 if y > 0 else 0)

    # Context: a quiet residential street to the north, trees all round
    m.box("street", (400, 8, 0.2), (0, 30, -0.14), "asphalt", core=False)
    for i, x in enumerate(range(-60, 61, 15)):
        m.backdrop_block("house", x, 44, 10, 9, 7 + (i % 3) * 2.5)
    for args in ((-60, -45, 30, 18, 12), (10, -50, 40, 20, 9), (60, -40, 20, 20, 16)):
        m.backdrop_block("block", *args)
    for x in range(-26, int(E), 7):
        m.tree("tree", x, 23.5, 8.0)
        m.tree("tree", x + 3, -24, 9.0)
    for y in range(-16, 20, 8):
        m.tree("tree", -33, y, 10.0)
        m.tree("tree", E + 5, y + 4, 10.0)

    f = m.feature
    f("Pool bowl", "16 × 10 m, shallow end 1.8 m deep (west) to deep end 3.2 m (east), 0.6 m of vertical wall, steel coping.",
      (-12, 6, -1.5))
    f("Cradle bowl", "Round, 10 m across, 1.9 m deep, all transition.", (7, 8, -1.0))
    f("Snake run", "About 55 m long and 4.4 m wide, winding, up to 1.5 m deep; ramps in at both ends.", (-5, -15.5, -1.0))
    f("Full pipe", "A concrete tube 3.6 m across inside, 7 m long: fly straight through.", (18.5, 0, 1.8))
    if merged:
        f("Stair set and handrails", "6 steps up to a +1.2 m terrace, two handrails 0.9 m above the steps.", (28, 11, 1.2))
        f("Hubba and bank", "A sloped ledge beside the stairs; a 3.6 m bank up to the terrace.", (27.5, 4.5, 0.8))
        f("Pergola", "10 × 6 m. Slats at 3.6 m with 0.6 m gaps (a whoop gap, a tight 5\" gap); 3.3 m clear underneath.",
          (19, 15.5, 3.6))
        f("Ring sculpture", "A steel ring with a 3.1 m opening, centre at 2.2 m, flown east-west.", (18.5, 7, 2.2))
    else:
        f("Volcano", "9 m across at the base, 1.6 m high, flat 3 m top.", (21, 13, 1.6))
    f("Pyramid", "Four-sided hip, 1 m high, in the middle of the deck.", (-1, -1.5, 1.0))
    f("Shade shelter", "8 × 4 m roof at 3 m on four posts.", (-20, 16, 3.1))
    f("Light poles", "9 m poles on the corners and the long sides.", (E - 0.5, -19.5, 9))

    m.spot("L1", -1.8, 6, 270, "Deck between the two bowls, facing into the pool bowl's deep end.", recommended=True)
    if merged:
        m.spot_tag = "Chosen in Round 1"
        m.round = 2
    else:
        m.spot("L2", -7, 6, 270, "Flat bottom of the pool bowl's deep end, 3.2 m down, walls all round.", z_hint=-3.2)
        m.spot("L3", 21, 13, 225, "Top of the volcano, +1.6 m, facing across the park.", z_hint=1.6)

    m.fly_shot("whoop_bowl", "Whoop at 1.2 m above the pool floor, carving toward the deep-end wall.",
               (-15.5, 3.2, -0.7), 70, 6, "whoop")
    m.fly_shot("five_snake", "5\" dropping into the snake run from 5 m.", (-27, -9, 5.0), 120, -28, "five")
    if merged:
        m.fly_shot("five_ring", "5\" at 2.2 m crossing from the bowls toward the ring and the stair set.",
                   (6, 7, 2.2), 90, 0, "five")
        m.fly_shot("whoop_rails", "Whoop at 0.7 m lining up the stairs between the handrails.", (22.5, 11, 0.7), 90, 2,
                   "whoop")
        m.three_quarter.update(az=200, elev=38, dist=78, target=(3, 0, 2.0))
    else:
        m.three_quarter.update(az=200, elev=38, dist=72)
    figs = [(-2.5, 4.5, 0), (9, -2, 0), (-12, -14, -0.6), (18, -1.5, 0)] + ([(31, 9, 1.2)] if merged else [])
    for p in figs:
        m.figure(*p)
    m.facts += ["Most features sit below the deck: from a landed Quad the Map looks almost flat.",
                "Heightfield transitions at 20–25 cm resolution: the vertical bowl walls show as steep slopes."]
    if merged:
        m.facts += ["The street corner adds what stands above head height, besides the poles: terrace, pergola, ring."]
    return m
