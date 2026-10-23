"""The Factory's first slice (#295, spec #290, ADR-0024): built in Blender.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/maps/factory/build.py -- --out <file.glb> [--previews <folder>]

`cargo xtask maps` does that. One 30 x 30 m bay of an abandoned brick hall,
the first piece of `opendrone/factory`: brick walls with arched iron windows,
a corrugated roof on steel trusses with skylights broken or gone and one bay
fallen in, a hole broken through the west wall, plaster peeling off the north
wall with a faded sign, a little graffiti, rubble, burnt timber and fallen
beams on a worn concrete floor, weeds at the foot of the walls, and a yard of
dirt with self-sown trees. Every surface is a material made from scratch.

East is +x, north is +y, up is +z; the origin is the hall's south-west corner
on the floor (0 m). The walls' centre lines run along x = 0 and 30 (the
gables) and y = 0 and 30; the ridge runs east-west over y = 15. The sun is in
the south-west, 37 degrees up (`map.toml`), so it shines in through the south
slope's holes, the south windows and the west wall's breach.

Gaps, by size (world.md): missing panes (0.56 m) and a vent hole (0.8 x 0.6 m)
only the whoop fits; the breach (2.8 x 2.6 m), the loading door (3.6 x 4.4 m),
the man door and the open skylights for both Quads; and the bay where the roof
fell in (3.8 x 7 m) to dive through from 20 m and more.
"""
import math
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
import odplants as plants  # noqa: E402
import odruin as ruin  # noqa: E402
from odmap import MapBuilder  # noqa: E402

SEED = 295
LAUNCH = (12.5, -14.0, 0.0, 0.0)  # x, y, ground height, compass heading: facing the loading door

HALL = 30.0  # wall centre line to centre line
T = 0.5  # wall thickness, two leaves of brick
LEAF = T / 2.0
EAVE = 8.3  # top of the long walls
RISE = 4.0  # ridge above the eaves
SLOPE = math.degrees(math.atan2(RISE, HALL / 2.0))
SHEET = 0.02  # roof sheet thickness
BAYS = [5.0 * k for k in range(1, 6)]  # truss lines across x

BRICK = "brick_red_weathered"
STEEL = "steel_beam_rusted"
GLASS = "glass_wired_grimy"

PREVIEW = {
    "core": (-25.0, -25.0, 45.0, 45.0),
    "three_quarter": {"azimuth": 215.0, "elevation": 28.0, "distance": 70.0, "target": (15.0, 15.0, 5.0)},
    "figures": [(12.5, -5.0, 0.0), (5.0, 10.0, 0.0), (20.0, 20.0, 0.0)],
    "landed": [("whoop-65", 0.035), ("freestyle-5", 0.07)],
    "flights": [
        ("freestyle-5-into-the-loading-door", (12.5, -6.0, 2.0), 0.0, 0.0),
        ("whoop-65-through-the-breach", (-3.0, 14.5, 1.6), 90.0, 0.0),
        ("freestyle-5-diving-through-the-fallen-bay", (12.5, 6.5, 24.0), 0.0, -85.0),
        ("whoop-65-under-the-trusses", (2.0, 25.0, 6.5), 120.0, -5.0),
    ],
}


def roof_under(y):
    """The underside of the roof sheets over a point `y` north of the south
    wall's centre line: rising from the eaves to the ridge and down again."""
    return EAVE + 0.1 + RISE * (1.0 - abs(y - HALL / 2.0) / (HALL / 2.0))


class Face:
    """A wall's own frame: `u` metres along it from its start, `v` metres up,
    `n` metres out of its centre line towards `outward`."""

    def __init__(self, start, along, outward):
        self.start, self.along, self.outward = start, along, outward

    def p(self, u, v, n=0.0):
        s = self.start
        return (
            s[0] + self.along[0] * u + self.outward[0] * n,
            s[1] + self.along[1] * u + self.outward[1] * n,
            v + self.along[2] * u,
        )

    def plate(self, outline, n_from, n_to):
        """An outline in `(u, v)` laid in the wall's plane between `n_from`
        and `n_to` out of the centre line."""
        a, b = self.along, (0.0, 0.0, 1.0)
        thick_dir = g.cross(a, b)
        lo, hi = min(n_from, n_to), max(n_from, n_to)
        # `plate` thickens along a x b; start from whichever side that leaves.
        start = hi if g.dot(thick_dir, self.outward) < 0 else lo
        return ruin.plate(self.p(0.0, 0.0, start), a, b, outline, hi - lo)


# The four walls. The long walls run past the corners, the gables between them.
SOUTH = Face((-T / 2, 0.0), (1.0, 0.0, 0.0), (0.0, -1.0, 0.0))
NORTH = Face((-T / 2, HALL), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0))
WEST = Face((0.0, T / 2), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0))
EAST = Face((HALL, T / 2), (0.0, 1.0, 0.0), (1.0, 0.0, 0.0))

# Arched windows: centre (in the wall's own u), sill, springing, radius.
SILL, SPRING, R = 1.2, 4.2, 1.2


def u_of(face, x_or_y):
    """A Map coordinate along the wall (x for the long walls, y for the
    gables) as the wall's own `u`."""
    return x_or_y + T / 2 if face in (SOUTH, NORTH) else x_or_y - T / 2


def wall_leaves(m, name, face, length, height, openings, rnd, pockets_out=10, pockets_in=4):
    """One wall in two leaves, each cut round the openings, and each with a
    few pockets where single bricks have fallen out of its face."""
    for leaf, n, count in (("outer", LEAF / 2, pockets_out), ("inner", -LEAF / 2, pockets_in)):
        holes = list(openings) + ruin.missing_bricks(0.4, length - 0.4, 0.1, height - 0.3, count, rnd)
        # On a 5 mm grid, so two edges that nearly meet do meet, and no box
        # round the holes is a sliver.
        holes = [tuple(round(c / 0.005) * 0.005 for c in h) for h in holes]
        p0 = face.p(0.0, 0.0, n)
        p1 = face.p(length, 0.0, n)
        m.wall(f"{name}_{leaf}", (p0[0], p0[1]), (p1[0], p1[1]), 0.0, height, LEAF, BRICK, openings=holes)


def arched_window(m, name, face, uc, rnd, broken=0.3):
    """An arched window in an opening already cut from `uc - R` to `uc + R`
    and `SILL` to `SPRING + R`: brick spandrels round the arch, a ring of
    header bricks, a concrete sill, an iron grid of 0.6 m panes and wired
    glass, some panes broken or gone."""
    top = SPRING + R
    arc_l = [(uc + R * math.cos(math.radians(180 - k * 10)), SPRING + R * math.sin(math.radians(180 - k * 10))) for k in range(10)]
    arc_r = [(uc + R * math.cos(math.radians(90 - k * 10)), SPRING + R * math.sin(math.radians(90 - k * 10))) for k in range(10)]
    span = (-T / 2, T / 2)
    # The arcs run from 180 to 90 degrees and from 90 to 0, ends included.
    left = arc_l + [(uc - R, top)]
    right = arc_r + [(uc + R, top)]
    for outline in (left, right):
        m.solid(name + "_spandrel", face.plate(outline, *span), None, BRICK, "trimesh")
    ring_out = [(uc + (R + 0.23) * math.cos(math.radians(k * 10)), SPRING + (R + 0.23) * math.sin(math.radians(k * 10))) for k in range(19)]
    ring_in = [(uc + R * math.cos(math.radians(180 - k * 10)), SPRING + R * math.sin(math.radians(180 - k * 10))) for k in range(19)]
    m.solid(name + "_arch", face.plate(ring_out + ring_in, -T / 2 - 0.03, T / 2 + 0.03), None, BRICK, "trimesh")
    sill = face.plate([(uc - R - 0.1, SILL - 0.12), (uc + R + 0.1, SILL - 0.12), (uc + R + 0.1, SILL), (uc - R - 0.1, SILL)], -T / 2 - 0.02, T / 2 + 0.07)
    m.solid(name + "_sill", sill, None, "concrete_wall_rough", "trimesh")

    def arch_top(u):
        d = u - uc
        return SPRING + math.sqrt(max(0.0, R * R - d * d))

    bars, panes = ruin.Batch(), ruin.Batch()
    us = [uc - R + 0.6 * k for k in range(5)]
    vs = [SILL + 0.6 * k for k in range(9)]
    for u in us[1:-1]:
        bars.add(face.plate([(u - 0.02, SILL), (u + 0.02, SILL), (u + 0.02, arch_top(u + 0.02)), (u - 0.02, arch_top(u + 0.02))], -0.03, 0.03))
    for v in vs[1:]:
        if v + 0.02 >= top - 0.05:
            break
        half = R if v <= SPRING else math.sqrt(R * R - (v + 0.02 - SPRING) ** 2)
        bars.add(face.plate([(uc - half, v - 0.02), (uc + half, v - 0.02), (uc + half, v + 0.02), (uc - half, v + 0.02)], -0.03, 0.03))
    for i in range(4):
        for j in range(8):
            u0, u1 = us[i] + 0.02, us[i + 1] - 0.02
            v0, v1 = vs[j] + 0.02, vs[j + 1] - 0.02
            if v0 >= top - 0.1:
                continue
            steps = [u0 + (u1 - u0) * k / 6 for k in range(7)]
            tops = [min(v1, arch_top(u) - 0.02) for u in steps]
            kept = [(u, t) for u, t in zip(steps, tops) if t > v0 + 0.05]
            if len(kept) < 2:
                continue
            if rnd.chance(broken):
                if min(tops) >= v1:  # a whole rectangle: shards are left in its corners
                    for shard in ruin.shards(u1 - u0, v1 - v0, rnd):
                        panes.add(face.plate([(u0 + s, v0 + t) for s, t in shard], -0.004, 0.004))
                continue
            if min(tops) >= v1:
                outline = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
            else:
                # Under the arch: the cell's bottom, then its top along the
                # arch, with runs along the cell's own top cut to their ends.
                top_pts = [p for k, p in enumerate(kept) if not (0 < k < len(kept) - 1 and kept[k - 1][1] == p[1] == kept[k + 1][1])]
                outline = [(kept[0][0], v0), (kept[-1][0], v0)] + list(reversed(top_pts))
            panes.add(face.plate(outline, -0.004, 0.004))
    m.solid(name + "_grid", bars.mesh(), None, STEEL, "trimesh", light_cm=3)
    if not panes.empty():
        m.solid(name + "_glass", panes.mesh(), None, GLASS, "trimesh")


def walls(m, rnd):
    """The four walls, their windows, doors, breach and pilasters."""
    long_len = HALL + T
    gable_len = HALL - T
    # South: five arched windows and the loading door in the middle bay.
    win_s = [2.5, 7.5, 17.5, 22.5, 27.5]
    door = (u_of(SOUTH, 10.7), u_of(SOUTH, 14.3), 0.0, 4.4)
    notch = [(u_of(SOUTH, 18.6), u_of(SOUTH, 19.5), EAVE - 0.45, EAVE), (u_of(SOUTH, 19.5), u_of(SOUTH, 20.1), EAVE - 0.22, EAVE)]
    ops = [(u_of(SOUTH, x) - R, u_of(SOUTH, x) + R, SILL, SPRING + R) for x in win_s] + [door] + notch
    wall_leaves(m, "wall_south", SOUTH, long_len, EAVE, ops, rnd.child("south"), 14, 5)
    for x in win_s:
        arched_window(m, "window_south", SOUTH, u_of(SOUTH, x), rnd.child("ws", x), 0.25)
    # A steel lintel over the door.
    m.box_h("door_lintel", 12.5, 0.0, 4.2, T + 0.04, 4.4, 4.7, STEEL)

    # North: four windows; the middle bays plastered inside, peeling to brick.
    win_n = [2.5, 7.5, 22.5, 27.5]
    ops = [(u_of(NORTH, x) - R, u_of(NORTH, x) + R, SILL, SPRING + R) for x in win_n]
    wall_leaves(m, "wall_north", NORTH, long_len, EAVE, ops, rnd.child("north"), 10, 0)
    for x in win_n:
        arched_window(m, "window_north", NORTH, u_of(NORTH, x), rnd.child("wn", x), 0.35)
    plaster(m, rnd.child("plaster"))

    # The gables: the west has two windows and the breach, the east a man
    # door, a vent hole and a window. Each runs up to the roof's slope.
    breach = ruin.breach(u_of(WEST, 14.5), 0.45, 2.8, 2.6, rnd.child("breach"))
    ops = [(u_of(WEST, y) - R, u_of(WEST, y) + R, SILL, SPRING + R) for y in (7.5, 22.5)] + breach
    wall_leaves(m, "wall_west", WEST, gable_len, EAVE, ops, rnd.child("west"), 16, 8)
    for y in (7.5, 22.5):
        arched_window(m, "window_west", WEST, u_of(WEST, y), rnd.child("ww", y), 0.4)
    ops = [
        (u_of(EAST, 7.0), u_of(EAST, 8.1), 0.0, 2.2),
        (u_of(EAST, 12.1), u_of(EAST, 12.9), 0.9, 1.5),
        (u_of(EAST, 22.5) - R, u_of(EAST, 22.5) + R, SILL, SPRING + R),
    ]
    wall_leaves(m, "wall_east", EAST, gable_len, EAVE, ops, rnd.child("east"), 10, 3)
    arched_window(m, "window_east", EAST, u_of(EAST, 22.5), rnd.child("we"), 0.3)
    for name, x in (("gable_west", 0.0), ("gable_east", HALL)):
        profile = [(-T / 2, EAVE), (HALL + T / 2, EAVE), (HALL + T / 2, roof_under(HALL + T / 2))]
        profile += [(HALL / 2, roof_under(HALL / 2)), (-T / 2, roof_under(-T / 2))]
        # `prism` wants its outline clockwise, or its ends face inwards.
        m.prism(name, list(reversed(profile)), T, (x, 0.0, 0.0), 0.0, BRICK)

    # Brick pilasters under each truss on the long walls' inside faces.
    for x in BAYS:
        for y, sgn in ((T / 2, 1.0), (HALL - T / 2, -1.0)):
            m.box_h("pilaster", x, y + sgn * 0.14, 0.7, 0.28, 0.0, EAVE - 0.4, BRICK)


def plaster(m, rnd):
    """The north wall's inside face between x = 9.6 and 20.4: a skin of
    painted plaster up to a ragged edge that the damp has peeled back to the
    brick, with a faded sign board on it."""
    y = HALL - T / 2  # the inside face
    edge = [(20.4, 3.4)] + [(20.4 - k * 0.6, 3.4 + rnd.uniform(-0.5, 0.9)) for k in range(1, 18)] + [(9.6, 4.2)]
    outline = [(9.6, 0.15), (20.4, 0.15)] + edge
    skin = ruin.plate((0.0, y, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0), outline, 0.02)
    m.solid("plaster", skin, None, "brick_painted_peeling", "none")
    # Higher up, patches the damp left: islands of plaster.
    for cx, cz, w, h in ((11.0, 5.6, 1.6, 1.0), (17.6, 6.3, 2.2, 1.2), (14.0, 7.3, 1.0, 0.5)):
        ring = [(cx + w / 2 * math.cos(math.radians(a)) * rnd.uniform(0.7, 1.1), cz + h / 2 * math.sin(math.radians(a)) * rnd.uniform(0.7, 1.1)) for a in range(0, 360, 30)]
        m.solid("plaster", ruin.plate((0.0, y, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0), ring, 0.02), None, "brick_painted_peeling", "none")
    # The sign: one repeat of `sign_faded` is 4 m, here from x = 16 to 20 and
    # from z = 4 to 8; the board covers the part that holds it, between the
    # pilasters.
    m.box("sign_board", (3.2, 0.02, 1.6), (18.0, y - 0.03, 6.0), "sign_faded")


def graffiti(m):
    """A scrawl on the west wall's inside face: one repeat of `graffiti_brick`
    (3.6 m, from y = 0 to 3.6 and z = 0 to 3.6) holds it, and a skin 3 mm
    proud of the bricks shows the part with the paint."""
    x = T / 2 + 0.003
    m.box("graffiti", (0.004, 3.024, 1.908), (x, 1.8, 1.746), "graffiti_brick", collider="none")


def roof(m, rnd):
    """The trusses, purlins and corrugated sheets, with skylights broken or
    gone and one bay fallen in."""
    for x in BAYS:
        truss = ruin.Batch()
        y0, y1 = T / 2, HALL - T / 2

        def chord(y):
            return roof_under(y) - 0.12 - 0.08

        def member(p, q, w):
            pose = g.aiming(p, q, (1.0, 0.0, 0.0))
            truss.add(g.box_mesh(g.length(g.sub(q, p)), w, w), g.Pose(pose.rows, g.lerp(p, q, 0.5)))

        low = EAVE - 0.35
        member((x, y0, low), (x, y1, low), 0.16)
        for y in (y0 + 0.08, y1 - 0.08):
            member((x, y, low), (x, y, chord(y)), 0.12)
        member((x, y0, chord(y0)), (x, HALL / 2, chord(HALL / 2)), 0.16)
        member((x, HALL / 2, chord(HALL / 2)), (x, y1, chord(y1)), 0.16)
        panels = 6
        for side in (0, 1):
            for k in range(1, panels + 1):
                t0 = (k - 1) / panels
                t1 = k / panels
                ya = y0 + (HALL / 2 - y0) * t1 if side == 0 else y1 - (y1 - HALL / 2) * t1
                yb = y0 + (HALL / 2 - y0) * t0 if side == 0 else y1 - (y1 - HALL / 2) * t0
                member((x, ya, low + 0.08), (x, ya, chord(ya) - 0.08), 0.09)
                member((x, yb, low + 0.08), (x, ya, chord(ya) - 0.08), 0.08)
        m.solid("truss", truss.mesh(), None, STEEL, "trimesh")

    # Purlins: every 1.5 m down each slope, on the trusses, gable to gable.
    lines = [(k, 0.3 + 1.47 * k, "south") for k in range(11)] + [(k, HALL - 0.3 - 1.47 * k, "north") for k in range(10)]
    for k, y, side in lines:
        z = roof_under(y) - 0.06
        if side == "south" and k == 5:
            continue  # the bent one, below
        if side == "south" and k in (3, 4):
            # Torn out over the fallen bay: a stub each side of the hole.
            m.beam("purlin", (T / 2, y, z), (10.0, y, z), 0.1, 0.12, STEEL)
            m.beam("purlin", (15.0, y, z), (HALL - T / 2, y, z), 0.1, 0.12, STEEL)
            continue
        m.beam("purlin", (T / 2, y, z), (HALL - T / 2, y, z), 0.1, 0.12, STEEL)
    # One purlin over the hole has buckled and sags into it.
    y = 0.3 + 1.47 * 5
    z = roof_under(y) - 0.06
    sag = ruin.bent_bar([(10.0, y, z), (11.4, y + 0.1, z - 0.6), (12.6, y + 0.2, z - 1.5), (13.6, y + 0.1, z - 0.7), (15.0, y, z)], 0.1, 0.12)
    m.solid("purlin_bent", sag.mesh(), None, STEEL, "trimesh")

    # The sheets. Holes in plan, (x0, y0, x1, y1): skylights in every bay of
    # the south slope and two of the north, some gone, one bay fallen in.
    south = [(5.9, 4.0, 7.1, 10.0), (10.6, 3.0, 14.4, 10.0), (15.9, 4.0, 17.1, 10.0), (20.9, 4.0, 22.1, 10.0), (25.9, 11.0, 27.1, 13.0)]
    glazed_s = [(0.9, 4.0, 2.1, 10.0), (25.9, 4.0, 27.1, 10.0)]
    north = [(5.9, 20.0, 7.1, 26.0), (20.9, 19.0, 22.1, 24.0), (1.0, 16.0, 2.0, 18.0)]
    glazed_n = [(15.9, 20.0, 17.1, 26.0)]
    for name, plan, holes, glazed, tilt in (
        ("roof_south", (-0.4, -0.6, HALL + 0.4, HALL / 2), south, glazed_s, SLOPE),
        ("roof_north", (-0.4, HALL / 2, HALL + 0.4, HALL + 0.6), north, glazed_n, -SLOPE),
    ):
        for x0, y0, x1, y1 in g.rect_subtract(plan, holes + glazed):
            sheet(m, name, x0, y0, x1, y1, tilt, "roof_corrugated_rusted")
        for x0, y0, x1, y1 in glazed:
            sheet(m, name + "_skylight", x0, y0, x1, y1, tilt, GLASS)
    # Torn flaps hang into the holes: the skylight at x = 16.5 kept jagged
    # shards, and the fallen bay a few sheets bent down at its edges.
    flaps = ruin.Batch()
    # (x0, x1, the hole's edge it hangs from, which way the hole lies, degrees
    # below level, length)
    for x0, x1, y_edge, side, angle, length in (
        (15.9, 17.1, 10.0, -1.0, 25.0, 0.9),
        (10.6, 11.6, 10.0, -1.0, 55.0, 1.4),
        (12.0, 13.2, 10.0, -1.0, 70.0, 1.1),
        (13.4, 14.4, 3.0, 1.0, 60.0, 1.2),
    ):
        b = (0.0, side * math.cos(math.radians(angle)), -math.sin(math.radians(angle)))
        tear = ruin.jagged((x1, length), (x0, length), rnd, 3, 0.35)
        outline = [(x0, 0.0), (x1, 0.0), (x1, length)] + tear + [(x0, length)]
        flaps.add(ruin.plate((0.0, y_edge, roof_under(y_edge)), (1.0, 0.0, 0.0), b, outline, SHEET))
    m.solid("roof_flap", flaps.mesh(), None, "roof_corrugated_rusted", "trimesh")


def sheet(m, name, x0, y0, x1, y1, tilt, material):
    """A flat run of roof over the plan rectangle, on the roof's slope."""
    cy = (y0 + y1) / 2.0
    z = roof_under(cy) + SHEET / 2.0
    long = (y1 - y0) / math.cos(math.radians(SLOPE))
    # A thin box on a slope isn't quite a box once its corners are rounded to
    # 0.1 mm, so it is a closed triangle mesh.
    m.box(name, (x1 - x0, long, SHEET), ((x0 + x1) / 2.0, cy, z), material, tilt=(tilt, 0.0), collider="trimesh")


def floor_and_clutter(m, rnd):
    """The floor, the rubble, the fallen roof, burnt timber, fallen beams."""
    m.box_h("floor", HALL / 2, HALL / 2, HALL + T, HALL + T, -0.3, 0.0, "concrete_floor_worn")

    # Under the fallen bay: a heap of rubble, sheets and a purlin on it.
    heap = ruin.mound(12.5, 6.5, 3.2, 3.8, 0.7, rnd.child("heap"))
    m.heightfield("rubble_heap", 9.3, 2.7, 15.7, 10.3, 0.25, heap, "ground_dirt_rubble")
    lumps = ruin.Batch()
    ruin.scatter(lumps, rnd.child("heap_lumps"), (12.5, 6.5), 3.4, 60, lambda q: ruin.lump(q.uniform(0.15, 0.45), q), z=lambda x, y: max(0.0, heap(x, y)))
    ruin.scatter(lumps, rnd.child("heap_bricks"), (12.5, 6.5), 3.8, 50, ruin.brick, z=lambda x, y: max(0.0, heap(x, y)))
    m.solid("rubble_heap_chunks", lumps.mesh(), None, "concrete_wall_rough", "trimesh")
    fallen = ruin.Batch()
    for cx, cy, yaw, pitch, roll in ((11.6, 5.0, 12.0, 8.0, 14.0), (13.6, 7.8, -30.0, -12.0, 6.0), (10.6, 8.6, 70.0, 4.0, -18.0)):
        size = (1.0, 2.6, SHEET)
        fallen.add(g.box_mesh(*size), g.Pose(ruin.turned(yaw, pitch, roll), (cx, cy, max(0.05, heap(cx, cy)) + 0.15)))
    m.solid("roof_fallen", fallen.mesh(), None, "roof_corrugated_rusted", "trimesh")
    # The purlin that fell, one end on the heap.
    m.beam("purlin_fallen", (8.6, 3.4, 0.06), (13.9, 7.6, 0.75), 0.1, 0.12, STEEL, collider="box")
    # A rolled steel beam from somewhere, lying across the floor.
    m.beam("beam_fallen", (21.0, 13.0, 0.11), (26.5, 17.5, 0.11), 0.16, 0.22, STEEL, collider="box")

    # Both sides of the breach: what fell out of the wall.
    for name, cx, rx in (("rubble_breach_in", 1.6, 1.4), ("rubble_breach_out", -1.6, 1.4)):
        h = ruin.mound(cx, 14.5, rx, 2.2, 0.45, rnd.child(name))
        m.heightfield(name, cx - rx, 12.3, cx + rx, 16.7, 0.2, h, "ground_dirt_rubble")
        b = ruin.Batch()
        ruin.scatter(b, rnd.child(name, "bricks"), (cx, 14.5), 2.0, 70, ruin.brick, z=lambda x, y, h=h: max(0.0, h(x, y)))
        m.solid(name + "_bricks", b.mesh(), None, BRICK, "trimesh")

    # Burnt timber in the north-east corner: a fire of old pallets and planks,
    # on a low bed of ash and char.
    ash = ruin.mound(24.0, 24.5, 2.4, 2.0, 0.18, rnd.child("ash"))
    m.heightfield("ash", 21.6, 22.5, 26.4, 26.5, 0.25, ash, "wood_charred")
    planks = ruin.Batch()
    q = rnd.child("planks")
    for _ in range(26):
        length = q.uniform(0.8, 2.4)
        x, y = 24.0 + q.uniform(-1.8, 1.8), 24.5 + q.uniform(-1.5, 1.5)
        pose = g.Pose(ruin.turned(q.uniform(0, 360), q.uniform(-25, 25), q.uniform(-10, 10)), (x, y, max(0.0, ash(x, y)) + q.uniform(0.05, 0.35)))
        planks.add(g.box_mesh(length, q.uniform(0.08, 0.15), q.uniform(0.03, 0.06)), pose)
    for k in range(5):  # what is left of a pallet
        planks.add(g.box_mesh(1.2, 0.1, 0.022), g.Pose(ruin.turned(20.0, 0.0, 0.0), (21.2 + 0.25 * k * math.cos(math.radians(110)), 20.6 + 0.25 * k * math.sin(math.radians(110)), 0.13)))
    m.solid("burnt_timber", planks.mesh(), None, "wood_charred", "trimesh")

    # Loose bricks and chunks along the foot of the walls.
    b = ruin.Batch()
    q = rnd.child("loose")
    for x, y in ((4.0, 0.9), (19.0, 0.8), (29.0, 5.0), (8.0, 29.0), (26.0, 29.2), (0.9, 26.0)):
        ruin.scatter(b, q, (x, y), 0.7, 8, ruin.brick)
        ruin.scatter(b, q, (x, y), 0.6, 4, lambda r: ruin.lump(r.uniform(0.1, 0.3), r))
    m.solid("loose_rubble", b.mesh(), None, "concrete_wall_rough", "trimesh")


def yard(m, rnd):
    """Outside: dirt and rubble, grass and weeds along the walls, and
    self-sown trees."""
    # The yard round the hall gets light at 20 cm; the ground beyond it, to the
    # horizon, coarse light (a chart is at most 512 texels across).
    yard_area = (-45.0, -45.0, 80.0, 85.0)
    m.slab("yard", *yard_area, 0.0, 0.3, "ground_dirt_rubble", holes=[(-T / 2, -T / 2, HALL + T / 2, HALL + T / 2)], light_cm=20)
    m.slab("ground", -1000.0, -1000.0, 1000.0, 1000.0, 0.0, 0.3, "ground_dirt_rubble", holes=[yard_area])
    # Grass in a strip along the south and west walls and in patches.
    # Each is a thin skin on the ground with a ragged edge, drawn only.
    q = rnd.child("grass")
    south = [(x, 0.0) for x in (30.25, -0.25)] + [(-0.25 + k * 1.0, -q.uniform(0.6, 2.4)) for k in range(31)]
    west = [(0.0, 30.25), (0.0, -0.25)] + [(-q.uniform(0.6, 2.6), -0.25 + k * 1.0) for k in range(31)]
    grass = ruin.Batch()
    grass.add(ruin.plate((0.0, -0.25, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), south, 0.015))
    grass.add(ruin.plate((-0.25, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), west, 0.015))
    for cx, cy, rx, ry in ((-7.5, -8.0, 4.5, 3.5), (-10.0, 8.0, 4.0, 4.5), (2.0, -5.0, 2.5, 1.6), (27.0, -8.0, 3.0, 2.4)):
        ring = [(cx + rx * math.cos(math.radians(a)) * q.uniform(0.6, 1.1), cy + ry * math.sin(math.radians(a)) * q.uniform(0.6, 1.1)) for a in range(0, 360, 20)]
        grass.add(ruin.plate((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), ring, 0.015))
    m.solid("grass", grass.mesh(), None, "grass_weeds", "none")
    weeds = ruin.Batch()
    q = rnd.child("weeds")
    spots = [(x, -0.6) for x in (1.0, 3.6, 5.2, 9.4, 15.2, 19.0, 20.6, 24.4, 26.0, 29.2)]
    spots += [(-0.6, y) for y in (2.0, 5.0, 9.6, 11.8, 17.4, 19.6, 25.0, 28.0)]
    for cx, cy, rx, ry in ((-7.5, -8.0, 4.5, 3.5), (-10.0, 8.0, 4.0, 4.5), (2.0, -5.0, 2.5, 1.6), (27.0, -8.0, 3.0, 2.4)):
        p = rnd.child("patch", cx)
        spots += [(cx + rx * p.uniform(-0.6, 0.6), cy + ry * p.uniform(-0.6, 0.6)) for _ in range(7)]
    for x, y in spots:
        plants_batch = plants.weeds(q, (x + q.uniform(-0.3, 0.3), y + q.uniform(-0.3, 0.3)), q.uniform(0.2, 0.4), q.pick((10, 14, 18)), q.uniform(0.35, 0.8), 0.015)
        weeds.add(plants_batch.mesh())
    # And inside, where light and dirt get in: by the breach, on the heap,
    # along the south wall under the windows.
    for x, y, z in ((1.2, 12.6, 0.0), (1.0, 16.6, 0.0), (0.7, 19.0, 0.0), (11.0, 3.4, 0.2), (14.4, 9.6, 0.15), (7.5, 0.6, 0.0), (22.5, 0.6, 0.0), (17.0, 0.7, 0.0)):
        weeds.add(plants.weeds(q, (x, y), 0.25, 12, q.uniform(0.25, 0.55), z).mesh())
    # Fine light for the thin blades: a triangle that covers no texel's middle
    # bakes black.
    m.solid("weeds", weeds.mesh(), None, "leaves_mixed", "none", light_cm=2)

    for k, (x, y, h) in enumerate(((-6.0, -6.5, 8.5), (-9.5, 9.0, 9.5), (27.0, -8.0, 7.0), (-4.5, 26.0, 6.0))):
        wood, crown = plants.tree(rnd.child("tree", k), (x, y, 0.0), h, leaves=1800)
        m.solid("tree", wood.mesh(), None, "bark_rough", "trimesh")
        m.solid("tree_leaves", crown.mesh(), None, "leaves_mixed", "none", light_cm=7)


def backdrop(m, rnd):
    """What stands round the yard, all solid, and nothing worth flying to past
    about 100 m: the rest of the works, a boundary wall and a tree line."""
    for x, y, sx, sy, h, mat in (
        (62.0, 15.0, 30.0, 60.0, 12.0, BRICK),
        (15.0, 68.0, 60.0, 22.0, 9.0, BRICK),
        (-40.0, 40.0, 18.0, 30.0, 7.0, "concrete_wall_rough"),
        (45.0, -45.0, 24.0, 18.0, 6.0, "concrete_wall_rough"),
    ):
        m.box_h("works", x, y, sx, sy, 0.0, h, mat, light_cm=40)
        m.box_h("works_roof", x, y, sx + 0.6, sy + 0.6, h, h + 0.15, "roof_corrugated_rusted", light_cm=40)
    m.wall("boundary_wall", (-30.0, -30.0), (40.0, -30.0), 0.0, 2.6, 0.3, "concrete_wall_rough", openings=[(16.0, 22.0, 0.0, 2.6)], light_cm=30)
    m.wall("boundary_wall", (-30.0, -30.0), (-30.0, 22.0), 0.0, 2.6, 0.3, "concrete_wall_rough", light_cm=30)
    for k, (x, y, h) in enumerate(((-22.0, -36.0, 10.0), (-8.0, -38.0, 12.0), (8.0, -37.0, 9.0), (28.0, -38.0, 11.0), (-38.0, -14.0, 10.0), (-37.0, 6.0, 12.0))):
        wood, crown = plants.tree(rnd.child("backdrop_tree", k), (x, y, 0.0), h, leaves=900)
        m.solid("backdrop_tree", wood.mesh(), None, "bark_rough", "trimesh", light_cm=30)
        m.solid("backdrop_tree_leaves", crown.mesh(), None, "leaves_mixed", "none", light_cm=12)


def build(out):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    m.lightmap(texel_cm=10)
    rnd = ruin.Random(SEED, "factory")
    walls(m, rnd.child("walls"))
    graffiti(m)
    roof(m, rnd.child("roof"))
    floor_and_clutter(m, rnd.child("floor"))
    yard(m, rnd.child("yard"))
    backdrop(m, rnd.child("backdrop"))
    # Every surface here is a picture made from scratch: no flat colours,
    # nothing downloaded (ADR-0024).
    for name in m.materials:
        texture = m.library[name].get("texture")
        assert texture and "generator" in m.textures[texture], f"{name} has no picture made from scratch"
    m.launch_spot(*LAUNCH[:3], LAUNCH[3])
    m.export(out)
    return m


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    builder = build(out)
    print("factory:", builder.stats())
    if "--previews" in argv:
        import odpreview

        odpreview.render(builder, PREVIEW, LAUNCH, argv[argv.index("--previews") + 1])
