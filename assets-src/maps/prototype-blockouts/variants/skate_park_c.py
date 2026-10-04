"""PROTOTYPE (#17). Skate Park C: Indoor Warehouse Park. Wooden ramps inside a big shed."""
import math
from blockout_lib import Map, window_row


def build():
    m = Map("skate_park_c", "C · Indoor Warehouse Park", "Skate Park",
            "A skate park inside a 48 × 28 m shed: wooden vert ramp, mini ramp, spine and funbox under "
            "roof trusses, a café mezzanine, open roller doors and skylights to escape through.")
    m.core = (-26, -16, 26, 16)
    m.ground()
    m.box("yard", (80, 26, 0.2), (0, -27.15, -0.13), "asphalt", core=False)
    m.slab("hall_floor", -24, -14, 24, 14, 0.0, 0.3, "concrete_plaza")

    # Walls: 7 m to the eaves, roller doors and high windows
    H = 7.0
    win = window_row(48, 3, 1.5, 4.5, 6)
    m.wall("wall_s", (-24, -14), (24, -14), 0, H, 0.3, "metal_sheet", group="wall_s",
           openings=[(21.5, 26.5, 0, 5.0)] + [w for w in win if not (w[1] > 21 and w[0] < 27)])
    m.wall("wall_n", (24, 14), (-24, 14), 0, H, 0.3, "metal_sheet", group="wall_n",
           openings=[(30.5, 31.5, 0, 2.1)] + win)
    win_e = window_row(28, 3, 1.5, 4.5, 6)
    m.wall("wall_e", (24, -14), (24, 14), 0, H, 0.3, "metal_sheet", group="wall_e",
           openings=[(12, 16, 0, 4.5)] + [w for w in win_e if not (w[1] > 11.5 and w[0] < 16.5)])
    m.wall("wall_w", (-24, 14), (-24, -14), 0, H, 0.3, "metal_sheet", group="wall_w", openings=win_e)
    for x, g in ((-24, "wall_w"), (24, "wall_e")):
        m.prism("gable", [(-14.15, H), (14.15, H), (0, 10.0)], 0.3, (x, 0, 0), 0, "metal_sheet", group=g)
    for x in range(-18, 19, 6):
        for y in (-13.7, 13.7):
            m.box_h("column", x, y, 0.3, 0.3, 0, H, "steel_grey")

    # Roof: two panels with open skylights; trusses every 6 m
    V = math.hypot(14, 3) + 0.08
    sky = [(10, 13, 5, 8), (22.5, 25.5, 5, 8), (35, 38, 5, 8)]
    m.panel("roof_s", (-24.3, -14.2, H), (1, 0, 0), (0, 14, 3), 48.6, V, 0.2, sky, "metal_sheet", group="roof")
    m.panel("roof_n", (-24.3, 14.2, H), (1, 0, 0), (0, -14, 3), 48.6, V, 0.2, [], "metal_sheet", group="roof")
    top = lambda y: H + 2.85 * (1 - abs(y) / 14)
    for x in range(-18, 19, 6):
        m.beam("truss_chord", (x, -14, H), (x, 14, H), 0.2, 0.3, "steel_grey", group="trusses")
        m.beam("truss_top", (x, -14, H), (x, 0, top(0)), 0.2, 0.25, "steel_grey", group="trusses")
        m.beam("truss_top", (x, 14, H), (x, 0, top(0)), 0.2, 0.25, "steel_grey", group="trusses")
        ys = [-14, -10.5, -7, -3.5, 0, 3.5, 7, 10.5, 14]
        for i, y in enumerate(ys[1:-1], 1):
            m.beam("truss_web", (x, y, H), (x, y, top(y)), 0.12, 0.12, "steel_grey", group="trusses")
        for a, b in zip(ys, ys[1:]):
            lo, hi = (a, b) if abs(a) > abs(b) else (b, a)
            m.beam("truss_diag", (x, lo, H), (x, hi, top(hi)), 0.1, 0.1, "steel_grey", group="trusses")

    # Ramps
    for front, hd in (((-10, 2), 0), ((-10, -2), 180)):
        m.quarter_pipe("vert_ramp", front, hd, 10, 3.0, vert=0.6, deck=2.0)
    m.railing("vert_deck_rail", [(-15, 7.0), (-5, 7.0)], 3.6, h=1.0)
    m.railing("vert_deck_rail", [(-15, -7.0), (-5, -7.0)], 3.6, h=1.0)
    for front, hd in (((5.5, 7.5), 90), ((2.5, 7.5), 270)):
        m.quarter_pipe("mini_ramp", front, hd, 7, 1.8, deck=1.5)
    m.quarter_pipe("spine", (14.5, -8), 90, 8, 1.5, deck=0.05)
    m.quarter_pipe("spine", (17.6, -8), 270, 8, 1.5, deck=0.05, coping=False)
    m.prism("funbox", [(0, 0), (1.8, 0.6), (6.8, 0.6), (8.6, 0)], 3.5, (-0.3, -6, 0), 90, "wood_ramp")
    m.rail("funbox_rail", (1.5, -6, 0.95), (6.5, -6, 0.95), drop=0.35)
    m.quarter_pipe("wall_qp", (20.5, 7.5), 90, 9, 2.2, vert=0.3, deck=1.0)
    m.box_h("ledge", 12, 2, 6, 0.5, 0, 0.45, "concrete_smooth")
    m.box_h("manual_pad", 10, -11.5, 5, 2, 0, 0.3, "concrete_smooth")

    # Mezzanine café with rooms underneath, stairs up
    m.slab("mezzanine", -23.85, -13.85, -17.5, 13.85, 3.2, 0.25, "concrete_structure")
    for y in (-13.5, -9, -4.5, 0, 4.5, 9, 13.5):
        m.box_h("mezz_column", -17.7, y, 0.25, 0.25, 0, 2.95, "steel_grey")
    m.railing("mezz_rail", [(-17.6, -7.0), (-17.6, 13.8)], 3.2)
    for y in (-4, 4):
        m.wall("room_wall", (-23.85, y), (-17.5, y), 0, 2.95, 0.15, "concrete_structure", openings=[(2.6, 3.6, 0, 2.1)])
    m.box_h("counter", -18.2, 0, 0.6, 6, 0, 1.1, "wood_ramp")
    m.stairs("mezz_stairs", (-16.8, -12, 0), 0, 1.2, 16, 0.2, 0.3, "concrete_structure")

    # Yard: containers, poles; context
    m.container("container", -20, -24, 90)
    m.container("container", -20, -24, 90, z=2.59, mat="container_red")
    m.container("container", 18, -26, 20, mat="container_red")
    for (x, y) in ((-32, -32), (0, -36), (32, -32)):
        m.light_pole("light_pole", x, y, 8.0)
    for x in range(-30, 31, 8):
        m.tree("tree", x, 22, 9.0)
    for args in ((-62, 0, 30, 40, 9), (62, 8, 30, 30, 11), (0, 58, 60, 20, 10), (-10, -62, 40, 15, 8)):
        m.backdrop_block("unit", *args)

    f = m.feature
    f("Vert ramp", "10 m wide, 3.6 m tall (3 m transition plus 0.6 m vertical), 4 m flat, decks with railings.",
      (-10, 0, 3.6))
    f("Mini ramp", "7 m wide, 1.8 m tall, 3 m flat.", (4, 7.5, 1.8))
    f("Spine", "Two 1.5 m transitions back to back, 8 m wide.", (16, -8, 1.5))
    f("Funbox and rail", "8.6 × 3.5 m, 0.6 m high, banks both ends, rail on top.", (4, -6, 0.6))
    f("Wall quarter pipe", "9 m wide, 2.5 m tall, against the east wall.", (22, 7.5, 2.5))
    f("Mezzanine café", "+3.2 m deck along the west wall; three rooms underneath with 1 × 2.1 m doors.",
      (-20.5, 0, 3.2))
    f("Roof trusses", "Steel trusses every 6 m from 7 m (eaves) to 10 m (ridge); triangles about 3 m across.",
      (0, -7, 8.0))
    f("Skylights", "Three open 3 × 3 m holes in the south roof: a way out to the sky.", (-12.8, -7.6, 8.4))
    f("Roller doors", "Open: 5 × 5 m (south, to the yard) and 4 × 4.5 m (east).", (0, -14, 2.5))
    f("High windows", "3 × 1.5 m openings at 4.5 m all round.", (-24, 4.5, 5.2))

    m.spot("L1", -20.5, 2, 90, "On the mezzanine, +3.2 m, facing the whole hall.", z_hint=3.2, recommended=True)
    m.spot("L2", 0, -11.5, 0, "Hall floor just inside the big roller door, facing in.")
    m.spot("L3", 0, -24, 0, "Outside in the yard, facing the open roller door.")

    m.fly_shot("whoop_trusses", "Whoop at 8 m, threading the roof trusses along the hall.", (-21, 1.75, 8.0), 90, -3, "whoop")
    m.fly_shot("five_vert", "5\" dropping into the vert ramp from above the north deck.", (-10, 9.5, 5.6), 180, -32, "five")

    m.plans = [dict(name="plan", caption="Floor plan, cut at 6.5 m (roof and trusses removed)", cut=6.5, hide=["roof", "trusses"]),
               dict(name="plan_roof", caption="Roof plan from above", cut=None)]
    m.three_quarter.update(az=205, elev=38, dist=62, hide=("roof", "wall_s", "wall_w"),
                           target=(0, 0, 1.5))
    m.three_quarter["caption"] = "Cutaway three-quarter view (roof, south and west walls hidden)"
    m.extra_views.append(dict(name="exterior", caption="Exterior three-quarter view", az=215, elev=30, dist=85,
                              lens=28.0, target=(0, -6, 3)))
    for p in ((-8, 0.5, 0), (3, 4.5, 0), (-20.5, -2, 3.2), (12, -2, 0), (2, -22, 0)):
        m.figure(*p)
    m.facts += ["Indoors with a roof at 7–10 m: the ceiling is part of the Map.",
                "Exits: two roller doors, three skylights, a ring of high windows."]
    return m
