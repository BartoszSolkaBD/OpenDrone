"""PROTOTYPE (#17). Bando A: Unfinished Tower. A five-floor concrete frame that was never finished."""
import math
from blockout_lib import Map, window_row


def build():
    m = Map("bando_a", "A · Unfinished Tower", "Bando",
            "A five-floor concrete frame on a building site that stopped: open floors on a column grid, "
            "holes through every slab, an open lift shaft, a collapsed bay and a tower crane still standing.")
    m.core = (-35, -25, 35, 25)
    m.ground()
    m.box("site", (70, 50, 0.2), (0, 0, -0.13), "dirt", core=False)

    FH, X0, X1, Y0, Y1 = 3.35, -12.0, 12.0, -7.5, 7.5
    z = [0.15 + FH * k for k in range(6)]          # floor tops; z[5] is the roof
    lanes = {0: (6.4, 8.2), 1: (9.4, 11.2)}           # alternating stair lanes in the east bay
    broken = {1: [(-10.5, -6.5, -7.5, -4.0)],
              2: [(-4.5, 3.0, -1.5, 6.5), (-11.6, 2.7, -6.2, 7.1)],
              3: [(-10.0, 0.5, -7.0, 4.0), (3.5, -6.8, 5.5, -4.5)],
              4: [(-5.5, -3.0, -1.0, -0.5)],
              5: [(-9.5, 3.0, -6.5, 6.0), (3.5, 3.5, 5.5, 6.5)]}
    shaft = (0.4, -1.1, 2.6, 1.1)

    m.slab("ground_slab", X0 - 0.5, Y0 - 0.5, X1 + 0.5, Y1 + 0.5, z[0], 0.3, "concrete_structure")
    for k in range(1, 6):
        lx = lanes[(k - 1) % 2]
        holes = [(lx[0] - 0.1, -7.25, lx[1] + 0.1, -2.45), shaft] + broken.get(k, [])
        m.slab(f"floor_{k}", X0, Y0, X1, Y1, z[k], 0.25, "concrete_structure", holes=holes, group=f"above_{k}")
    for k in range(5):
        top = z[k + 1] - 0.25
        for x in (-11.8, -6, 0, 6, 11.8):
            for y in (-7.3, -2.5, 2.5, 7.3):
                m.box_h("column", x, y, 0.4, 0.4, z[k], top, "concrete_structure", group=f"above_{k + 1}" if k else None)
        lx = lanes[k % 2]
        m.stairs("stairs", ((lx[0] + lx[1]) / 2, -7.1, z[k]), 0, lx[1] - lx[0], 17, FH / 17, 0.27,
                 "concrete_structure", group=f"above_{k + 1}" if k else None)
    for y in (-1.2, 1.2):   # lift shaft: two concrete walls, open east and west
        m.wall("shaft_wall", (0.3, y), (2.7, y), z[0], z[5] + 1.2 - z[0], 0.2, "concrete_dark")

    # The collapsed bay: floor 2's slab fell and rests on floor 1
    dz, dy = 6.7 - 3.65, 7.0 - 2.9
    m.box("collapsed_slab", (5.2, math.hypot(dy, dz), 0.25), (-8.9, 4.95, (6.7 + 3.65) / 2), "concrete_structure",
          rot=(-math.degrees(math.atan2(dz, dy)), 0, 0))

    # Brick infill on some sides of some floors
    win = lambda L, skip=(): window_row(L, 1.5, 1.4, 0.9, 3.0, skip=skip)
    h = FH - 0.25
    infill = {0: dict(n=win(24), w=[(6.5, 7.6, 0, 2.2)] + win(15, skip=(2,)), s=win(12)),
              1: dict(e=win(15), n=win(12)),
              2: dict(s=win(12)),
              3: dict(w=win(15))}
    for k, sides in infill.items():
        if "s" in sides and k in (0, 2):   # half-length south walls
            m.wall("brick_infill", (X0 if k == 0 else 0, Y0 + 0.1), (0 if k == 0 else X1, Y0 + 0.1), z[k], h, 0.25,
                   "brick_old", openings=sides.pop("s"), group=f"above_{k + 1}" if k else None)
        if "n" in sides and k == 1:
            m.wall("brick_infill", (X1, Y1 - 0.1), (0, Y1 - 0.1), z[k], h, 0.25, "brick_old", openings=sides.pop("n"),
                   group="above_2")
        if sides:
            m.walls_rect("brick_infill", X0 + 0.1, Y0 + 0.1, X1 - 0.1, Y1 - 0.1, z[k], h, 0.25, "brick_old", sides,
                         group=f"above_{k + 1}" if k else None)

    # Roof: parapet with gaps, water tank on legs, rebar stubs
    m.walls_rect("parapet", X0, Y0, X1, Y1, z[5], 1.0, 0.2, "concrete_structure",
                 dict(s=[(14, 17.5, 0, 1.0)], e=[], n=[(3, 6, 0, 1.0)], w=[]))
    for dx in (-0.8, 0.8):
        for dy in (-0.8, 0.8):
            m.box_h("tank_leg", -9 + dx, -5 + dy, 0.15, 0.15, z[5], z[5] + 1.2, "steel_rusty")
    m.cyl("water_tank", (-9, -5, z[5] + 1.2), (-9, -5, z[5] + 3.2), 1.1, "steel_rusty", seg=14)
    for (cx, cy) in ((-6, 2.5), (0, -2.5), (6, 2.5), (11.8, -7.3)):
        for (ox, oy) in ((-0.12, -0.12), (0.12, -0.12), (-0.12, 0.12), (0.12, 0.12)):
            m.cyl("rebar", (cx + ox, cy + oy, z[5]), (cx + ox, cy + oy, z[5] + 1.0), 0.016, "steel_rusty", seg=5)

    # Tower crane: lattice mast, jib over the building, counter-jib, hook
    CX, CY, TOP = 20.0, 2.0, 34.0
    s = 0.8
    corners = [(CX - s, CY - s), (CX + s, CY - s), (CX + s, CY + s), (CX - s, CY + s)]
    for (x, y) in corners:
        m.beam("crane_chord", (x, y, 0), (x, y, TOP), 0.16, 0.16, "steel_painted", group="crane")
    lv = [i * 2.4 for i in range(int(TOP / 2.4) + 1)]
    for i, zz in enumerate(lv):
        for a, b in zip(corners, corners[1:] + corners[:1]):
            m.beam("crane_brace", (*a, zz), (*b, zz), 0.08, 0.08, "steel_painted", group="crane")
            if i + 1 < len(lv):
                p, q = (a, b) if i % 2 else (b, a)
                m.beam("crane_diag", (*p, zz), (*q, lv[i + 1]), 0.06, 0.06, "steel_painted", group="crane")
    m.box_h("crane_cab", CX, CY, 2.2, 2.2, TOP, TOP + 2.0, "steel_painted", group="crane")
    J0, J1, JZ = CX - 1.1, -15.0, TOP + 2.0
    for dy in (-0.6, 0.6):
        m.beam("jib_chord", (J0, CY + dy, JZ), (J1, CY + dy, JZ), 0.12, 0.12, "steel_painted", group="crane")
        m.beam("cjib_chord", (CX + 1.1, CY + dy, JZ), (CX + 11, CY + dy, JZ), 0.14, 0.14, "steel_painted", group="crane")
    m.beam("jib_top", (J0, CY, JZ + 1.4), (J1, CY, JZ + 1.4), 0.12, 0.12, "steel_painted", group="crane")
    x = J0
    while x > J1 + 0.1:
        nx = max(J1, x - 2.5)
        for dy in (-0.6, 0.6):
            m.beam("jib_diag", (x, CY + dy, JZ), (nx, CY, JZ + 1.4), 0.06, 0.06, "steel_painted", group="crane")
        m.beam("jib_rung", (nx, CY - 0.6, JZ), (nx, CY + 0.6, JZ), 0.06, 0.06, "steel_painted", group="crane")
        x = nx
    m.box_h("counterweight", CX + 9.5, CY, 2.0, 2.2, JZ - 2.2, JZ, "concrete_dark", group="crane")
    m.beam("crane_peak", (CX, CY, JZ), (CX, CY, JZ + 5), 0.3, 0.3, "steel_painted", group="crane")
    m.cyl("jib_tie", (CX, CY, JZ + 5), (J1 + 8, CY, JZ + 1.4), 0.03, "steel_grey", seg=5, group="crane")
    m.cyl("cjib_tie", (CX, CY, JZ + 5), (CX + 11, CY, JZ), 0.03, "steel_grey", seg=5, group="crane")
    m.box_h("trolley", -2, CY, 1.2, 1.4, JZ - 0.5, JZ - 0.05, "steel_grey", group="crane")
    m.cyl("hook_cable", (-2, CY, JZ - 0.5), (-2, CY, 20.6), 0.025, "steel_grey", seg=5, group="crane")
    m.box_h("hook_block", -2, CY, 0.5, 0.5, 20.0, 20.6, "steel_painted", group="crane")

    # Yard
    m.container("container", 25, -15, 0)
    m.container("container", 28.2, -15, 0, mat="container_red")
    m.container("container", 26.6, -15.5, 90, z=2.59)
    for (x, y, r, n, seed) in ((-20, 8, 4.0, 40, 1), (6, 13, 3.0, 25, 2), (-5, -14, 2.5, 18, 3)):
        m.rubble("rubble", x, y, r, n, seed)
    m.box_h("site_office", -24, -16, 6, 2.5, 0.3, 3.0, "metal_sheet")
    m.cyl("dirt_mound", (-25, 15, 0), (-25, 15, 2.0), 5.0, "dirt", seg=14, r1=1.5)
    m.fence("site_fence", [(-3, -24), (-34, -24), (-34, 24), (34, 24), (34, -24), (3, -24)], h=2.0)

    # Context
    for x in range(-40, 41, 10):
        m.tree("tree", x, 30, 10.0)
    for args in ((-70, 10, 30, 40, 24), (75, -5, 26, 50, 30), (0, 70, 60, 20, 18), (-20, -70, 50, 20, 12),
                 (45, -60, 25, 25, 40)):
        m.backdrop_block("city_block", *args)

    f = m.feature
    f("Concrete frame", "24 × 15 m, five floors every 3.35 m, roof at 16.9 m, 0.4 m columns on a 6 × 5 m grid.",
      (-6, -2.5, 2.0))
    f("Open lift shaft", "2.2 m square, open from the roof to the ground floor: a 17 m dive.", (1.5, 0, 16.9))
    f("Stairs", "One flight per floor in the east bay, through matching floor openings (1.8 × 4.8 m).", (8.5, -5, 8.0))
    f("Broken floors", "Holes 2–5 m across in every floor, at different places on each.", (-8.5, 2.25, 10.2))
    f("Collapsed bay", "Floor 2's slab fell onto floor 1: a 37° ramp-shaped gap.", (-8.9, 4.95, 5.2))
    f("Brick infill", "Some sides of the lower floors are bricked in, with 1.5 × 1.4 m windows.", (-12, 0, 2.0))
    f("Roof", "Parapet with two gaps, a water tank on 1.2 m legs, rebar stubs.", (-9, -5, 19.0))
    f("Tower crane", "34 m lattice mast (1.3 m openings), 35 m jib over the building, hook hanging at 20 m.",
      (20, 2, 30.0))
    f("Yard", "Containers (one stacked), rubble piles, site office, dirt mound, site fence.", (26, -15, 4.0))

    m.spot("L1", -8.5, 0, 90, "Ground floor inside the frame, facing east between the columns.", z_hint=0.15)
    m.spot("L2", -3, -4.5, 90, "Roof, +16.9 m, facing the crane.", z_hint=16.9)
    m.spot("L3", 0, -17, 0, "In the yard, facing the building.", recommended=True)

    m.fly_shot("whoop_floor2", "Whoop on floor 2, between the columns toward the collapsed bay.", (7, 4.6, 8.0), 270, -6,
               "whoop")
    m.fly_shot("five_crane", "5\" diving past the crane toward the building.", (26, -9, 31), 320, -35, "five")
    m.fly_shot("five_shaft", "5\" over the roof, looking straight down the open lift shaft.", (1.5, 0, 20.5), 0, -88,
               "five")

    m.plans = [dict(name="plan", caption="Roof plan from above", cut=None),
               dict(name="plan_ground", caption="Ground-floor plan, cut at 1.8 m", cut=1.8,
                    hide=[f"above_{k}" for k in range(1, 6)] + ["crane"])]
    m.three_quarter.update(az=215, elev=26, dist=92, target=(3, 0, 14))
    for p in ((-7, -12.5, 0), (-9, -3, 0.15), (24, -11, 0)):
        m.figure(*p)
    m.facts += ["Vertical: 17 m of building plus a 41 m crane.",
                "Floors are open-sided, so most of the inside is lit and visible from outside."]
    return m
