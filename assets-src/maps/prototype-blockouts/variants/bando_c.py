"""PROTOTYPE (#17). Bando C: Ruined Compound. Low brick ruins around an overgrown courtyard."""
import math, random
from blockout_lib import Map


def windows(L, sills, w=1.4, h=1.8, spacing=3.0, avoid=()):
    out = []
    n = int((L - 1.0) // spacing)
    start = (L - (n - 1) * spacing - w) / 2
    for s in sills:
        for i in range(n):
            u = start + i * spacing
            if not any(u < a1 + 0.3 and u + w > a0 - 0.3 and s < b1 and s + h > b0 for a0, a1, b0, b1 in avoid):
                out.append((u, u + w, s, s + h))
    return out


def build():
    m = Map("bando_c", "C · Ruined Compound", "Bando",
            "Two-storey brick ruins around an overgrown courtyard: roofs gone to bare rafters, a tunnel through "
            "one wing, a walkway bridge to a three-storey tower, and a water tower outside the walls.")
    m.core = (-40, -35, 40, 35)
    m.ground()
    m.box("compound_ground", (60, 48, 0.2), (0, -2, -0.13), "dirt", core=False)
    m.box("road", (6, 34, 0.2), (0, -43, -0.12), "asphalt", core=False)

    Z0, F1, ROOF = 0.15, 3.6, 7.2
    H2 = ROOF - Z0
    LOW, HIGH = (0.85, 4.35)

    # North wing: 40 x 8 m, two storeys, roof gone to rafters
    doors_s = [(5, 7.5, 0, 2.8), (17, 19.5, 0, 2.8), (29, 31.5, 0, 2.8), (3, 4.2, 0, 2.2), (36.5, 38, 3.45, 5.65)]
    m.walls_rect("north_wing", -22, 10, 18, 18, Z0, H2, 0.4, "brick_old",
                 dict(s=doors_s + windows(40, (LOW, HIGH), avoid=doors_s + [(0, 8.4, 0, 9)]),
                      n=windows(40, (LOW, HIGH)), e=windows(8, (LOW, HIGH), spacing=4), w=windows(8, (HIGH,), spacing=4)))
    m.slab("north_wing_floor1", -22, 10, 18, 18, F1, 0.25, "timber_old",
           holes=[(-15, 11, -11, 14), (2, 13, 6, 17)], group="upper")
    for x in (-8, 6):
        m.wall("cross_wall", (x, 10.2), (x, 17.8), Z0, H2, 0.3, "brick_old",
               openings=[(3.4, 4.4, 0, 2.2), (3.4, 4.4, 3.45, 5.65)])
    rng = random.Random(7)
    x = -21.6
    while x < 17.7:
        if rng.random() > 0.25:
            for y in (10, 18):
                m.beam("rafter", (x, y, ROOF), (x, 14, 9.7), 0.12, 0.18, "timber_old", group="roof")
        x += 1.2
    m.beam("ridge_beam", (-22, 14, 9.7), (18, 14, 9.7), 0.2, 0.25, "timber_old", group="roof")
    for y in (12, 16):
        m.beam("purlin", (-22, y, 8.45), (18, y, 8.45), 0.12, 0.15, "timber_old", group="roof")
    for xg in (-22, 18):
        m.prism("gable", [(-4.2, ROOF), (4.2, ROOF), (0, 9.7)], 0.4, (xg, 14, 0), 0, "brick_old")

    # West wing: 8 x 26 m, a 3 x 3 m tunnel through the ground floor, half its roof left
    tun_e, tun_w = (13, 16, 0, 3.0), (10, 13, 0, 3.0)
    e_ops = [tun_e, (8, 9.5, 0, 2.4)]
    m.walls_rect("west_wing", -22, -16, -14, 10, Z0, H2, 0.4, "brick_old",
                 dict(e=e_ops + windows(26, (LOW, HIGH), avoid=e_ops), w=[tun_w] + windows(26, (LOW, HIGH), avoid=[tun_w]),
                      s=windows(8, (LOW, HIGH), spacing=4)))
    for y in (-3, 0):
        m.wall("tunnel_wall", (-21.8, y), (-14.2, y), Z0, 3.2, 0.3, "brick_old")
    m.slab("west_wing_floor1", -22, -16, -14, 10, F1, 0.25, "timber_old", holes=[(-21, 4, -17, 8)], group="upper")
    m.slab("west_wing_roof", -22, -16, -14, -4, ROOF + 0.15, 0.25, "concrete_structure", group="roof")

    # East tower: 10 x 16 m, three storeys, flat roof with a parapet
    T = (12, -14, 22, 2)
    T_ROOF = 10.8
    n_ops = [(6, 7.5, 3.45, 5.65), (2, 3.2, 0, 2.2)]
    w_ops = [(6, 8.5, 0, 2.8)]
    sills3 = (0.85, 4.45, 8.05)
    m.walls_rect("tower", *T, Z0, T_ROOF - Z0, 0.4, "brick_old",
                 dict(n=n_ops + windows(10, sills3, avoid=n_ops), s=windows(10, sills3) + [(4.4, 5.6, 0, 2.2)],
                      e=windows(16, sills3), w=w_ops + windows(16, sills3, avoid=w_ops)))
    m.slab("tower_floor1", *T, F1, 0.25, "concrete_structure", holes=[(14, -12, 16, -9)], group="upper")
    m.slab("tower_floor2", *T, ROOF, 0.25, "concrete_structure", holes=[(18, -4, 20.5, -1)], group="upper")
    m.slab("tower_roof", *T, T_ROOF, 0.25, "concrete_structure", group="roof")
    m.walls_rect("tower_parapet", *T, T_ROOF, 1.0, 0.25, "brick_old", dict(s=[], e=[], n=[], w=[]), group="roof")
    for dx in (-0.9, 0.9):
        for dy in (-0.9, 0.9):
            m.box_h("tank_leg", 19 + dx, -10 + dy, 0.15, 0.15, T_ROOF, T_ROOF + 1.5, "steel_rusty", group="roof")
    m.cyl("roof_tank", (19, -10, T_ROOF + 1.5), (19, -10, T_ROOF + 3.5), 1.2, "steel_rusty", seg=14, group="roof")

    # Walkway bridge between the north wing and the tower, at the first floor
    m.slab("walkway", 14.5, 2.2, 16, 9.8, F1, 0.25, "concrete_structure")
    for x in (14.5, 16):
        m.railing("walkway_rail", [(x, 2.3), (x, 9.7)], F1, h=1.0, spacing=1.9)

    # Boundary walls with a gate and breaches; a collapsed shed leaning on the south wall
    m.wall("boundary_s", (-30, -26), (30, -26), 0, 2.4, 0.4, "brick_old", openings=[(28, 32, 0, 2.4)])
    m.wall("boundary_w", (-30, -26), (-30, 22), 0, 2.4, 0.4, "brick_old", openings=[(20, 25, 0, 2.4)])
    m.wall("boundary_n", (-30, 22), (30, 22), 0, 2.4, 0.4, "brick_old", openings=[(40, 46, 0, 2.4)])
    m.wall("boundary_e", (30, 22), (30, -26), 0, 2.4, 0.4, "brick_old", openings=[(30, 34, 0, 2.4)])
    for x in (-2.4, 2.4):
        m.box_h("gate_pillar", x, -26, 0.8, 0.8, 0, 4.0, "brick_old")
    m.box_h("gate_lintel", 0, -26, 5.6, 0.8, 3.5, 4.2, "brick_old")
    m.box("collapsed_shed_roof", (10, math.hypot(5.8, 2.4), 0.15), (-21, -22.9, 1.2), "metal_sheet",
          rot=(-math.degrees(math.atan2(2.4, 5.8)), 0, 0))
    for x in (-25.8, -16.2):
        m.box_h("shed_post", x, -20.3, 0.15, 0.15, 0, 0.9, "timber_old")

    # Courtyard
    for (x, y, h) in ((-5, -6, 10), (6, -12, 9), (-2, 3, 11)):
        m.tree("courtyard_tree", x, y, h, core=True)
    m.annulus("dry_fountain", (5, -2, 0), (5, -2, 0.6), 2.6, 3.0, "concrete_structure", seg=24)
    for (x, y, r, n, seed) in ((-10, 6, 2.5, 20, 11), (9, 6, 2.0, 14, 12), (-24, 2, 2.5, 18, 13), (24, -20, 2.0, 12, 14)):
        m.rubble("rubble", x, y, r, n, seed)
    m.box_h("burned_car", -6, -16, 1.8, 4.2, 0.25, 1.45, "steel_rusty", heading=30)

    # Water tower outside the north-east corner
    WX, WY, LEG = 34, 28, 14.0
    legs = [(WX - 2, WY - 2), (WX + 2, WY - 2), (WX + 2, WY + 2), (WX - 2, WY + 2)]
    for (x, y) in legs:
        m.box_h("wt_leg", x, y, 0.3, 0.3, 0, LEG, "steel_rusty")
    for z0, z1 in ((0.5, 7), (7, LEG)):
        for a, b in zip(legs, legs[1:] + legs[:1]):
            m.beam("wt_brace", (*a, z0), (*b, z1), 0.08, 0.08, "steel_rusty")
            m.beam("wt_brace", (*b, z0), (*a, z1), 0.08, 0.08, "steel_rusty")
            m.beam("wt_ring", (*a, z1), (*b, z1), 0.12, 0.12, "steel_rusty")
    m.cyl("wt_tank", (WX, WY, LEG), (WX, WY, LEG + 4), 3.0, "steel_rusty", seg=20)
    m.cyl("wt_roof", (WX, WY, LEG + 4), (WX, WY, LEG + 5.5), 3.0, "steel_rusty", seg=20, r1=0.3)

    for x in range(-44, 45, 8):
        m.tree("tree", x, 40, 11.0)
        if abs(x) > 6:
            m.tree("tree", x, -33, 10.0)
    for y in range(-28, 37, 8):
        m.tree("tree", -37, y, 11.0)
        m.tree("tree", 40, y - 4, 10.0)
    for args in ((-90, 20, 40, 60, 10), (90, -10, 40, 60, 14), (0, 90, 100, 20, 8)):
        m.backdrop_block("block", *args)

    f = m.feature
    f("North wing", "40 × 8 m, two storeys; the roof is gone except rafters every 1.2 m (about 1 m gaps).", (-2, 14, 8.5))
    f("Tunnel", "A 3 × 3.2 m passage through the west wing's ground floor.", (-18, -1.5, 1.6))
    f("East tower", "10 × 16 m, three storeys, flat roof at 10.8 m with a parapet and a tank.", (17, -6, 10.8))
    f("Walkway bridge", "1.5 m wide, 8 m long, at 3.6 m, joining the north wing and the tower.", (15.25, 6, 3.6))
    f("Broken floors", "Timber floors with holes on the first floor of both wings and in the tower.", (-13, 12.5, 3.6))
    f("Courtyard", "About 28 × 26 m: three trees (9–11 m), a dry fountain, rubble, a burned-out car.", (0, -6, 2))
    f("Boundary wall and gate", "2.4 m walls with breaches; the gate is 4.8 × 3.5 m.", (0, -26, 3.8))
    f("Collapsed shed", "A roof sheet leaning on the south wall: a wedge-shaped gap underneath.", (-21, -22.9, 1.2))
    f("Water tower", "14 m legs with X-bracing, tank top at 19.5 m, outside the walls.", (34, 28, 16))

    m.spot("L1", 0, -14, 0, "Courtyard, facing the north wing.", recommended=True)
    m.spot("L2", 15.25, 6, 270, "On the walkway bridge, +3.6 m, facing across the courtyard.", z_hint=3.6)
    m.spot("L3", 0, -31, 0, "Outside the gate on the road, facing in.")

    m.fly_shot("whoop_rafters", "Whoop on the north wing's first floor, under the bare rafters.", (-19, 12.5, 5.6), 90, 8,
               "whoop")
    m.fly_shot("whoop_tunnel", "Whoop at 1.3 m lining up the tunnel from the courtyard.", (-9, -1.5, 1.3), 270, 0, "whoop")
    m.fly_shot("five_tower", "5\" diving off the water tower toward the compound.", (34, 23, 23), 220, -35, "five")

    m.plans = [dict(name="plan", caption="Roof plan from above", cut=None),
               dict(name="plan_ground", caption="Ground-floor plan, cut at 2.2 m", cut=2.2, hide=["roof", "upper"])]
    m.three_quarter.update(az=210, elev=36, dist=95, target=(0, 0, 3))
    for p in ((2, -12, 0), (-12.5, -4.6, 0), (15.25, 4, 3.6), (1, -29, 0)):
        m.figure(*p)
    m.facts += ["Spread out and mostly low (2.4–11 m), with one 19.5 m landmark.",
                "Lots of small openings: windows 1.4 × 1.8 m, doors 1–2.5 m."]
    return m
