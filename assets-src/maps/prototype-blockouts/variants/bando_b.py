"""PROTOTYPE (#17). Bando B: Factory Hall. One huge abandoned hall plus industrial structures outside."""
import math
from blockout_lib import Map, rect_subtract, window_row


def build():
    m = Map("bando_b", "B · Factory Hall", "Bando",
            "One huge abandoned factory hall (60 × 24 m, 11–14 m high) with a saw-tooth roof you can fly out of, "
            "a gantry crane, an office block of small rooms, and silos, pipe racks and a chimney outside.")
    m.core = (-50, -30, 50, 30)
    HALL = (-30.2, -12.2, 30.2, 12.2)
    m.ground(holes=[m.core])
    for x0, y0, x1, y1 in rect_subtract(m.core, [HALL]):
        m.box("yard", (x1 - x0, y1 - y0, 0.2), ((x0 + x1) / 2, (y0 + y1) / 2, -0.12), "asphalt", core=True)

    # Floor with a service pit
    PIT = (-22, -6.6, -6, -5.4)
    m.slab("hall_floor", *HALL, 0.15, 0.3, "concrete_plaza", holes=[PIT])
    m.box_h("pit_floor", -14, -6, 16.4, 1.6, -1.55, -1.35, "concrete_dark")
    for y in (-6.7, -5.3):
        m.wall("pit_wall", (-22.1, y), (-5.9, y), -1.35, 1.5, 0.2, "concrete_dark")
    for x in (-22.1, -5.9):
        m.wall("pit_wall", (x, -6.6), (x, -5.4), -1.35, 1.5, 0.2, "concrete_dark")

    # Brick walls, 11 m, big broken windows; doors and a breach
    W = window_row(60, 4, 4, 4, 7.5)
    z0, h = 0.15, 10.85
    X0, Y0, X1, Y1 = -30, -12, 30, 12
    m.walls_rect("wall", X0, Y0, X1, Y1, z0, h, 0.4, "brick_old", dict(s=W), group="wall_s")
    m.walls_rect("wall", X0, Y0, X1, Y1, z0, h, 0.4, "brick_old", dict(n=W + [(38, 43, 0, 7)]), group="wall_n")
    m.walls_rect("wall", X0, Y0, X1, Y1, z0, h, 0.4, "brick_old",
                 dict(e=[(3, 7, 0, 4.5), (10, 14, 0, 4.5), (2, 6, 7, 9.5), (9, 15, 7, 9.5), (18, 22, 7, 9.5)]),
                 group="wall_e")
    m.walls_rect("wall", X0, Y0, X1, Y1, z0, h, 0.4, "brick_old",
                 dict(w=[(9, 15, 0, 6), (2, 6, 7, 9.5), (18, 22, 7, 9.5)]), group="wall_w")
    for i in range(1, 8):
        x = -30 + 7.5 * i
        for y in (-11.6, 11.6):
            m.box_h("column", x, y, 0.4, 0.4, z0, 11, "steel_grey")
        m.box_h("column_mid", x, 0, 0.4, 0.4, z0, 11, "steel_grey")

    # Saw-tooth roof: 8 teeth, open glazing on each vertical face, two teeth missing sheets
    for i in range(8):
        xa, xb = -30 + 7.5 * i, -22.5 + 7.5 * i
        holes = {2: [(4, 12, 2, 6.5)], 5: [(14, 20, 1, 5)]}.get(i, [])
        m.panel("roof_slope", (xa, -12.2, 11.0), (0, 1, 0), (7.5, 0, 3), 24.4, math.hypot(7.5, 3), 0.15, holes,
                "metal_sheet", group="roof")
        m.beam("valley_beam", (xa, -12, 11), (xa, 12, 11), 0.35, 0.6, "steel_grey")
        m.beam("ridge_beam", (xb, -12, 14), (xb, 12, 14), 0.3, 0.4, "steel_grey")
        for y in (-12, 0, 12):
            m.beam("rafter", (xa, y, 11), (xb, y, 14), 0.2, 0.25, "steel_grey")
        for k in range(1, 10):
            y = -12 + 2.4 * k
            m.beam("mullion", (xb, y, 11), (xb, y, 14), 0.1, 0.1, "steel_grey")
        for y, g in ((-12, "wall_s"), (12, "wall_n")):
            m.prism("roof_gable", [(0, 11), (7.5, 11), (7.5, 14)], 0.4, (xa, y, 0), 90, "brick_old", group=g)

    # Gantry crane
    for y in (-11.2, 11.2):
        m.box_h("crane_runway", 0, y, 59.2, 0.4, 8.5, 9.1, "steel_painted")
        m.box_h("crane_truck", 6, y, 3.0, 0.5, 9.1, 9.7, "steel_painted")
    m.box_h("crane_bridge", 6, 0, 0.9, 22.4, 9.1, 10.1, "steel_painted")
    m.box_h("hoist", 6, 3, 1.2, 1.2, 8.2, 9.1, "steel_grey")
    m.cyl("hoist_chain", (6, 3, 8.2), (6, 3, 3.6), 0.03, "steel_grey", seg=5)
    m.box_h("hook_block", 6, 3, 0.5, 0.5, 3.0, 3.6, "steel_painted")

    # Machines and a lying tank
    for x in (-18, -10, -2):
        m.box_h("press", x, 6, 3, 2.5, z0, 3.65, "steel_grey")
    for x in (-24, -15, 0, 10):
        m.box_h("lathe", x, -9, 2.4, 1.0, z0, 1.35, "steel_grey")
    m.cyl("old_tank", (8, 7, 1.45), (16, 7, 1.45), 1.2, "steel_rusty", seg=16)
    for x in (9, 15):
        m.box_h("saddle", x, 7, 0.4, 2.0, z0, 0.7, "concrete_dark")
    m.rubble("rubble", -10.5, 9.5, 2.0, 15, 4, z=z0)
    m.rubble("rubble", -10.5, 14.5, 2.5, 22, 5)

    # Office block inside the east end: two storeys of small rooms, roof deck at 7 m
    OX0, OY0, OX1, OY1 = 19.0, -11.6, 29.6, -3.0
    m.slab("office_floor1", OX0, OY0, OX1, OY1, 3.6, 0.25, "concrete_structure", holes=[(24.8, -11.4, 29.4, -10.0)])
    m.slab("office_roof", OX0, OY0, OX1, OY1, 7.05, 0.25, "concrete_structure")
    for k, (zb, zt) in enumerate(((z0, 3.35), (3.6, 6.8))):
        hh = zt - zb
        m.wall("office_wall", (OX0, OY0), (OX0, OY1), zb, hh, 0.2, "concrete_structure",
               openings=[(5.5, 6.5, 0, 2.1), (1.5, 3.5, 1.0, 2.2)] if k == 0 else [(1.5, 3.5, 1.0, 2.2), (5.0, 7.0, 1.0, 2.2)])
        m.wall("office_wall", (OX0, OY1), (OX1, OY1), zb, hh, 0.2, "concrete_structure",
               openings=[(1, 2, 0, 2.1), (3.5, 5.5, 1.0, 2.2), (7.5, 9.5, 1.0, 2.2)] if k == 0 else
               [(1, 3, 1.0, 2.2), (4.5, 6.5, 1.0, 2.2), (8, 10, 1.0, 2.2)])
        m.wall("office_partition", (24.5, OY0), (24.5, OY1), zb, hh, 0.15, "concrete_structure", openings=[(4, 5, 0, 2.1)])
    m.stairs("office_stairs", (25.0, -10.7, z0), 90, 1.2, 16, (3.6 - z0) / 16, 0.28, "concrete_structure")
    m.railing("office_roof_rail", [(OX0 + 0.1, OY0), (OX0 + 0.1, OY1 - 0.1), (OX1, OY1 - 0.1)], 7.05)

    # Loading dock, canopy, trailers
    m.box_h("loading_dock", 31.6, 0, 2.8, 24, 0, 1.2, "concrete_structure")
    m.box_h("dock_canopy", 32.6, 0, 4.8, 24, 5.0, 5.2, "metal_sheet")
    for y in (-11.5, -4, 4, 11.5):
        m.box_h("canopy_post", 34.8, y, 0.25, 0.25, 0, 5.0, "steel_grey")
    for y in (-7, 0):
        m.box_h("trailer", 39.7, y, 13, 2.5, 1.25, 4.0, "metal_sheet")
        for x in (43.2, 44.6):
            for dy in (-0.95, 0.95):
                m.box_h("trailer_wheels", x, y + dy, 1.0, 0.5, 0, 1.0, "concrete_dark")
        for dy in (-0.9, 0.9):
            m.box_h("landing_leg", 35.5, y + dy, 0.2, 0.2, 0, 1.25, "steel_grey")

    # Silos, pipe rack, chimney
    for (sx, sy) in ((-42, 18), (-42, 8)):
        for a in range(4):
            ang = math.radians(45 + 90 * a)
            m.box_h("silo_leg", sx + 2.3 * math.cos(ang), sy + 2.3 * math.sin(ang), 0.4, 0.4, 0, 4.0, "steel_rusty")
        m.cyl("silo_hopper", (sx, sy, 2.0), (sx, sy, 4.0), 0.4, "steel_rusty", seg=16, r1=3.0)
        m.cyl("silo", (sx, sy, 4.0), (sx, sy, 16.0), 3.0, "steel_rusty", seg=20)
        m.cyl("silo_roof", (sx, sy, 16.0), (sx, sy, 17.5), 3.0, "steel_rusty", seg=20, r1=0.3)
    for x in range(-40, 15, 6):
        for y in (13.6, 16.4):
            m.box_h("rack_leg", x, y, 0.3, 0.3, 0, 6.0, "steel_grey")
        m.box_h("rack_beam", x, 15, 0.3, 3.1, 6.0, 6.3, "steel_grey")
    for y, r in ((14.2, 0.25), (15.0, 0.2), (15.8, 0.15)):
        m.cyl("pipe", (-40.5, y, 6.3 + r), (14.5, y, 6.3 + r), r, "steel_rusty", seg=10)
    m.cyl("chimney", (40, 20, 0), (40, 20, 42), 2.2, "brick_old", seg=16, r1=1.3)
    m.fence("site_fence", [(-4, -29), (-49, -29), (-49, 29), (49, 29), (49, -29), (4, -29)], h=2.2)

    for x in range(-60, 61, 12):
        m.tree("tree", x, 36, 10.0)
        m.tree("tree", x + 6, -36, 9.0)
    for args in ((-85, 0, 30, 60, 14), (85, 10, 30, 50, 18), (0, 70, 80, 20, 12), (10, -70, 60, 20, 10)):
        m.backdrop_block("block", *args)

    f = m.feature
    f("Main hall", "60 × 24 m, 11 m to the eaves, saw-tooth roof to 14 m.", (-14, 2, 6))
    f("Saw-tooth roof openings", "Each tooth has a 3 m tall open face with posts every 2.4 m; two teeth have missing sheets.",
      (-7.5, -4, 13))
    f("Central column row", "0.4 m columns every 7.5 m down the middle: a slalom.", (-7.5, 0, 3))
    f("Gantry crane", "Runway beams at 8.5 m, a bridge across the hall, a hook hanging to 3 m.", (6, 0, 9.6))
    f("Office block", "Two storeys of small rooms (doors 1 × 2.1 m, inner windows), roof deck at 7 m.", (24, -7, 7.1))
    f("Service pit", "16 m long, 1.2 m wide, 1.5 m deep.", (-14, -6, 0))
    f("Machines and tank", "Presses (3.5 m), lathes, a lying 2.4 m tank.", (-10, 6, 3.6))
    f("Breach and windows", "A 5 × 7 m hole in the north wall; 4 × 4 m broken windows all round.", (-10.5, 12, 3.5))
    f("Loading dock", "1.2 m dock under a 5 m canopy; two trailers with a 1.2 m gap underneath.", (38, -3.5, 4))
    f("Silos and pipe rack", "Two 17.5 m silos on legs (2 m under the hoppers); a 6 m pipe rack along the north wall.",
      (-42, 13, 12))
    f("Chimney", "42 m brick chimney.", (40, 20, 42))

    m.spot("L1", -27, 3, 90, "Inside the west end on the floor, facing down the 60 m hall.", z_hint=0.15, recommended=True)
    m.spot("L2", 22, -5.6, 270, "Office block, first floor, facing out of an inner window over the hall.", z_hint=3.6)
    m.spot("L3", 31.6, 0, 270, "Loading dock, +1.2 m, facing in through the open loading door.", z_hint=1.2)

    m.fly_shot("whoop_office", "Whoop on the office's first floor, lining up the door into the next room.",
               (20.5, -7.1, 4.4), 90, -2, "whoop")
    m.fly_shot("five_hall", "5\" at 9.5 m flying down the hall toward the gantry crane.", (-28, -4, 9.5), 90, 6, "five")

    m.plans = [dict(name="plan", caption="Roof plan from above", cut=None),
               dict(name="plan_ground", caption="Floor plan, cut at 2.5 m (roof hidden)", cut=2.5, hide=["roof"])]
    m.three_quarter.update(az=210, elev=36, dist=80, hide=("roof", "wall_s"), target=(0, 0, 4))
    m.three_quarter["caption"] = "Cutaway three-quarter view (roof sheets and south wall hidden)"
    m.extra_views.append(dict(name="exterior", caption="Exterior three-quarter view", az=225, elev=28, dist=120,
                              lens=28.0, target=(0, 4, 8)))
    for p in ((-25, 1, 0.15), (5, -3, 0.15), (32, 6, 1.2), (-38, 12, 0)):
        m.figure(*p)
    m.facts += ["Mostly one big interior volume: room for a 5\" to fly fast indoors.",
                "Outside structures (silos, rack, chimney) give the 5\" its high lines."]
    return m
