"""PROTOTYPE (#17). Skate Park A: Street Plaza. Flat, open, two levels, street obstacles."""
import math
from blockout_lib import Map


def build():
    m = Map("skate_park_a", "A · Street Plaza", "Skate Park",
            "An open city plaza on two levels: stairs, rails, ledges and a few big sculptural gaps. "
            "Mostly low obstacles under open sky.")
    m.core = (-25, -16, 25, 16)
    m.ground()
    m.slab("plaza", -25, -16, 25, 16, 0.0, 0.3, "concrete_plaza")

    # Upper terrace, stairs, hubba and bank
    m.box_h("terrace", 0, 12, 50, 8, 0.0, 1.2, "concrete_plaza")
    m.stairs("stair_set", (-3, 5.9, 0), 0, 6.0, 6, 0.2, 0.35, "concrete_plaza")
    for x in (-4.6, -1.4):
        m.rail("handrail", (x, 5.5, 0.871), (x, 8.0, 2.30), drop=0.95, posts=3)
    m.prism("hubba", [(-0.3, 0), (-0.3, 0.45), (2.1, 1.7), (2.1, 0)], 0.7, (0.35, 5.9, 0), 0, "concrete_smooth")
    m.bank("bank_to_terrace", (12, 4.4), 0, 8.0, 3.6, 1.2)
    m.railing("terrace_railing", [(-25, 8.05), (-6.3, 8.05)], 1.2)
    m.railing("terrace_railing", [(0.9, 8.05), (7.8, 8.05)], 1.2)
    m.railing("terrace_railing", [(16.2, 8.05), (25, 8.05)], 1.2)
    for x in (-20, -12, 4, 11):
        m.box_h("planter", x, 13.5, 2, 2, 1.2, 1.7, "concrete_smooth")
        m.tree("plaza_tree", x, 13.5, 7.0, z=1.7, core=True)
    m.box_h("kiosk", 19, 12.5, 6, 4, 1.2, 4.2, "concrete_structure")

    # Lower plaza: ledges, manual pad, flat rail, kicker
    m.box_h("ledge", -15, 1, 6, 0.6, 0, 0.45, "concrete_smooth")
    m.box_h("ledge_edge", -15, 0.72, 6, 0.06, 0.40, 0.46, "steel_painted")
    m.box_h("ledge", -11, -6, 5, 0.5, 0, 0.5, "concrete_smooth", heading=30)
    m.box_h("manual_pad", 5, -4.5, 7, 2.5, 0, 0.3, "concrete_smooth")
    m.rail("flat_rail", (12, -1, 0.35), (18, -1, 0.35), posts=2)
    m.bank("kicker", (16.5, -9.5), 90, 1.6, 1.2, 0.5)

    # Pergola: columns, edge beams and slats with 0.6 m gaps
    for x in (-19.7, -15, -10.3):
        for y in (-13.7, -8.3):
            m.box_h("pergola_column", x, y, 0.35, 0.35, 0, 3.3, "concrete_structure")
    for y in (-13.7, -8.3):
        m.box_h("pergola_beam", -15, y, 10, 0.35, 3.3, 3.6, "concrete_structure")
    x = -19.85
    while x <= -10.1:
        m.box_h("pergola_slat", x, -11, 0.3, 6.4, 3.6, 3.85, "concrete_structure")
        x += 0.9
    m.box_h("bench", -15, -11, 4, 0.5, 0, 0.45, "concrete_smooth")

    # Ring sculpture on a plinth: opening 3.1 m across
    R, cz = 1.75, 2.15
    m.box_h("ring_plinth", 0, -8, 1.2, 0.6, 0, 0.35, "concrete_structure")
    m.tube("ring_sculpture", [(R * math.cos(t), -8, cz + R * math.sin(t))
                              for t in (2 * math.pi * i / 48 for i in range(48))], 0.2, "steel_painted",
           seg=10, closed_loop=True)

    # Wall with two windows
    m.wall("window_wall", (8, -13), (16, -13), 0, 3.0, 0.3, "concrete_structure",
           openings=[(1.2, 2.6, 0.9, 1.9), (4.6, 6.4, 0.6, 2.0)])

    # Step seating on the east edge
    m.stairs("step_seating", (21.4, -4, 0), 90, 20, 3, 0.45, 1.2, "concrete_plaza")

    for (x, y, z) in ((-24, -15, 0), (24, -15, 0), (0, -15.5, 0), (-24, 15, 1.2), (24, 15, 1.2)):
        m.light_pole("light_pole", x, y, 8.0, z=z, heading=0 if y < 0 else 180)

    # Context: road to the south, city blocks, trees on the grass
    m.box("road", (400, 8, 0.2), (0, -26, -0.14), "asphalt", core=False)
    for args in ((-40, 46, 26, 16, 22), (-5, 50, 20, 14, 30), (30, 45, 28, 16, 18),
                 (-35, -45, 30, 14, 14), (22, -48, 24, 18, 25), (65, 5, 16, 30, 12), (-65, 0, 14, 40, 16)):
        m.backdrop_block("city_block", *args)
    for y in range(-14, 16, 6):
        m.tree("tree", -30, y, 9.0)
        m.tree("tree", 30, y + 3, 9.0)

    f = m.feature
    f("Stair set and handrails", "6 steps, 1.2 m drop, two handrails 0.9 m above the steps.", (-3, 7, 1.2))
    f("Hubba ledge", "A sloped ledge beside the stairs, 0.45–0.5 m above them.", (0.35, 6.5, 1.3))
    f("Bank to the terrace", "Flat bank, 3.6 m run up to the +1.2 m terrace.", (12, 6, 0.6))
    f("Upper terrace", "50 × 8 m at +1.2 m, railing, planter trees (7 m), kiosk roof at +4.2 m.", (-12, 12, 1.2))
    f("Pergola", "10 × 6 m. Slats at 3.6 m with 0.6 m gaps (a whoop gap, a tight 5\" gap); 3.3 m clear underneath.",
      (-15, -11, 3.6))
    f("Ring sculpture", "A steel ring with a 3.1 m opening, centre at 2.2 m.", (0, -8, 2.2))
    f("Wall with windows", "8 × 3 m wall; openings 1.4 × 1.0 m and 1.8 × 1.4 m.", (12, -13, 1.5))
    f("Ledges, manual pad, kicker", "Ledges 0.45–0.5 m, a 7 × 2.5 m manual pad, a 0.5 m kicker.", (-15, 1, 0.45))
    f("Flat rail", "6 m long, 0.35 m high.", (15, -1, 0.35))
    f("Step seating", "Three 0.45 m steps along the east edge, 20 m long.", (23, -4, 0.9))
    f("Light poles", "8 m poles at the corners: something tall to dive around.", (-24, -15, 8))

    m.spot("L1", -3, 10.5, 180, "Top of the stairs on the terrace, facing the whole plaza.", z_hint=1.2, recommended=True)
    m.spot("L2", -22, -3, 90, "Ground level at the west edge, facing east along the plaza.")
    m.spot("L3", 19, 12.5, 225, "Kiosk roof, +4.2 m, facing south-west over the plaza.", z_hint=4.2)

    m.fly_shot("whoop_rails", "Whoop at 0.7 m, cruising toward the stairs between the handrails.",
               (-3, 2.5, 0.7), 0, 2, "whoop")
    m.fly_shot("five_ring", "5\" at 3 m, lining up the ring sculpture from the road.", (0, -21, 3.0), 0, -4, "five")
    for p in ((-6.5, 4.5, 0), (13.5, 2, 0), (-13, -10, 0), (2, -9, 0)):
        m.figure(*p)
    m.facts += ["Open sky everywhere: no ceilings except the pergola.", "Two levels, 1.2 m apart."]
    return m
