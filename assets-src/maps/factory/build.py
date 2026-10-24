"""The Factory's hall corner (#295, reworked by #324; spec #290, ADR-0024): built in Blender.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/maps/factory/build.py -- --out <file.glb> [--previews <folder>]

`cargo xtask maps` does that. One 30 x 30 m bay of an abandoned brick hall,
the first piece of `opendrone/factory`, and its yard. Nothing in it is flat or
square: the walls bulge and their broken openings are stepped brick by brick
(`odopening`), the roof sags between its trusses with sheets lifted, curled,
torn and gone (`odroof`), the floor is a patchwork of concrete slabs that have
each settled their own way over dips where water stands, and the yard rolls,
with grass that thins into dirt and a path worn through it (`odground`,
`odedge`, a vertex-colour blend). Every surface is a material made from
scratch, worn by the rules of `odwear` where water, shade, fire and people
would wear it.

Its history is in layers (docs/research/lived-in-places.md, "The Factory's
story"), each with a place you can point at:

1. the works: machine plinths with cut anchor bolts and oil rings, the crane
   rail on its brackets, the empty cable tray, walkway lines down the aisle;
2. closure and stripping: the breach knocked through the west wall and the
   drag marks to it, cut cable, the open fuse box, the scrap pile sorted and
   left by the south wall, the tipped filing cabinet and office chair;
3. decay: the fallen bay, water runs under every sill, damp and algae under
   the roof's holes, moss and salt on the shaded walls, rust under the
   brackets, droppings under the trusses, weeds and a young birch;
4. squatters and the fire: the north-east corner, a burnt bed frame and
   trolley in the ash, a melted chair, a pallet windbreak, soot up the walls
   and under the roof, old tags under the soot;
5. the pilots: a fire pit and a BBQ under the skylight inside the loading
   door, pallet benches, crates, a cable-drum table with a charging spot,
   bottles and cans, a bin bag, new tags, the repaired yard gate, a path worn
   to the Launch Spot and a launch pad, and crashed-quad bits where crashes
   would leave them.

East is +x, north is +y, up is +z; the origin is the hall's south-west corner
on the floor (0 m). The walls' centre lines run along x = 0 and 30 (the
gables) and y = 0 and 30; the ridge runs east-west over y = 15. The sun is in
the south-west, 37 degrees up (`map.toml`), so it shines in through the south
slope's holes, the south windows and the west wall's breach.

Gaps, by size (world.md): missing panes (0.56 m) and a vent hole (0.8 x 0.6 m)
only the whoop fits; the breach (2.8 x 2.6 m), the loading door (3.6 x 4.4 m),
the man door, the open skylights and the yard gate for both Quads; and the bay
where the roof fell in (3.8 x 7 m) to dive through from 20 m and more.
"""
import math
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odgeom as g  # noqa: E402
import odplants as plants  # noqa: E402
import odprops as props  # noqa: E402
import odruin as ruin  # noqa: E402
from odedge import WanderingEdge  # noqa: E402
from odground import Dip, Flat, Ground  # noqa: E402
from odmap import MapBuilder  # noqa: E402
from odnoise import Random, fbm2, noise2, smoothstep  # noqa: E402
from odopening import Opening, broken_wall  # noqa: E402
from odroof import Roof, Skylight, Tear  # noqa: E402
from odrelief import Relief  # noqa: E402
from odscatter import Scatter  # noqa: E402
from odwear import Face as WearFace  # noqa: E402
from odwear import Floor, Wear  # noqa: E402

SEED = 295
LAUNCH = (12.5, -14.0, 0.0, 0.0)  # x, y, ground height, compass heading: facing the loading door

HALL = 30.0  # wall centre line to centre line
T = 0.45  # wall thickness: two leaves of brick and their joint, two bricks' length
FOOT = 0.3  # the walls reach this far below the floor, so uneven ground never shows under them
EAVE = 8.325  # top of the long walls: 111 courses of 75 mm
RISE = 4.0  # ridge above the eaves
SHEET = 0.02  # a loose roof sheet's thickness
BAYS = [5.0 * k for k in range(1, 6)]  # truss lines across x
IN = T / 2  # the inside faces: y = IN and HALL - IN, x = IN and HALL - IN

BRICK = "brick_red_weathered"
STEEL = "steel_beam_rusted"
GLASS = "glass_wired_grimy"
ROOF = "roof_corrugated_rusted"
FLOOR = "concrete_floor_worn"
DIRT = "ground_dirt_rubble"
GRASS = "grass_weeds"
WOOD = "wood_weathered"
CHAR = "wood_charred"

# The direction to the sun, (east, north): `map.toml`'s azimuth of 215 degrees.
SUN = (math.sin(math.radians(215.0)), math.cos(math.radians(215.0)))

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

# Holes in the roof, in plan (x0, y0, x1, y1). The fallen bay is the one the
# Quads dive through; the hangout's fire is under the open skylight east of it,
# so its smoke goes up through it; the fire corner's skylight shattered in the
# squatters' fire and is gone.
FALLEN_BAY = (10.6, 3.0, 14.4, 10.0)
SOUTH_GONE = [(5.9, 4.0, 7.1, 10.0), FALLEN_BAY, (20.9, 4.0, 22.1, 10.0)]
SOUTH_GLASS = [((0.9, 4.0, 2.1, 10.0), "intact"), ((15.9, 4.0, 17.1, 10.0), "broken"), ((25.9, 4.0, 27.1, 10.0), "broken")]
NORTH_GONE = [(5.9, 20.0, 7.1, 26.0), (20.9, 19.0, 22.1, 24.0)]
NORTH_GLASS = [((15.9, 20.0, 17.1, 26.0), "intact")]
NORTH_MISSING_SHEET = 21  # the sheet over x 8 to 9 on the north slope, eave to ridge
# Rust holes, and tears round the fallen bay so its edge is ragged, not a
# rectangle: slope, x, y, radius.
ROOF_TEARS = [("south", 26.5, 12.0, 0.6), ("north", 1.5, 17.0, 0.5),
              ("south", 10.7, 3.2, 0.75), ("south", 14.3, 3.1, 0.6), ("south", 10.8, 9.8, 0.65), ("south", 14.2, 9.9, 0.8),
              ("south", 12.4, 10.1, 0.55), ("south", 10.55, 6.6, 0.5), ("south", 14.45, 7.4, 0.6)]

# The pilots' hangout, and the squatters' fire corner.
FIRE_PIT = (21.5, 6.5)
ASH = (24.8, 24.6)


def roof_under(y):
    """The underside of the roof sheets over a point `y` north of the south
    wall's centre line, before they sag: rising from the eaves to the ridge
    and down again."""
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
        start = hi if g.dot(thick_dir, self.outward) < 0 else lo
        return ruin.plate(self.p(0.0, 0.0, start), a, b, outline, hi - lo)


# The four walls, each starting on a whole brick (a multiple of 0.225 m) so
# the stepped edges of `odopening` fall on the picture's joints. The long walls
# run past the corners, the gables between them.
SOUTH = Face((-T / 2, 0.0), (1.0, 0.0, 0.0), (0.0, -1.0, 0.0))
NORTH = Face((-T / 2, HALL), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0))
WEST = Face((0.0, T / 2), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0))
EAST = Face((HALL, T / 2), (0.0, 1.0, 0.0), (1.0, 0.0, 0.0))
LONG_RUN = HALL + T
GABLE_RUN = HALL - T

# Arched windows: sill, springing, radius.
SILL, SPRING, R = 1.2, 4.2, 1.2
WIN_S = [2.5, 7.5, 17.5, 22.5, 27.5]
WIN_N = [2.5, 7.5, 22.5, 27.5]
WIN_W = [7.5, 22.5]
WIN_E = [22.5]


def u_of(face, x_or_y):
    """A Map coordinate along the wall (x for the long walls, y for the
    gables) as the wall's own `u`."""
    return x_or_y + T / 2 if face in (SOUTH, NORTH) else x_or_y - T / 2


class Parts:
    """Batches by material, so a hundred small solids are a few parts to
    draw. `solid` ones become trimesh colliders, the rest are drawn only."""

    def __init__(self):
        self.batches = {}

    def add(self, key, mesh, pose=None):
        self.batches.setdefault(key, ruin.Batch()).add(mesh, pose)

    def put(self, m, key, name, material, collider, **kw):
        batch = self.batches.get(key)
        if batch is None or batch.empty():
            return None
        return m.solid(name, batch.mesh(), None, material, collider, **kw)


# ---------------------------------------------------------------- walls

def hall_wall(m, name, face, run, openings, seed, bulge, ledge, ground_at):
    """One wall as a closed mesh of bricks, from `FOOT` under the floor to the
    eaves, its broken openings stepped and its face bulging; a plain wall of
    boxes round the openings is what the Quad meets."""
    p0 = face.p(0.0, 0.0)
    p1 = face.p(run, 0.0)
    wall = broken_wall(run, FOOT + EAVE, T, seed, openings=openings, bulge=bulge, ledge=ledge)
    pose = wall.pose((p0[0], p0[1]), (p1[0], p1[1]), -FOOT)
    ob = m.solid(name, wall.mesh, pose, BRICK, "none")
    m.wall(name + "_solid", (p0[0], p0[1]), (p1[0], p1[1]), -FOOT, FOOT + EAVE, T, BRICK,
           openings=wall.collision_openings(), collision_only=True)

    def floor(u, y):
        x, yy, _ = pose.apply((u, y, 0.0))
        return FOOT + ground_at(x, yy)
    bricks = [b for b in wall.loose_bricks(Random(seed, "bricks"), floor=floor) if b[1]]
    return ob, wall, pose, bricks


def window_parts(parts, face, uc, rnd, broken=0.3):
    """An arched window in an opening already cut from `uc - R` to `uc + R`
    and `SILL` to `SPRING + R`: brick spandrels round the arch, a ring of
    header bricks, a concrete sill, an iron grid of 0.6 m panes and wired
    glass, some panes broken or gone."""
    top = SPRING + R
    arc_l = [(uc + R * math.cos(math.radians(180 - k * 10)), SPRING + R * math.sin(math.radians(180 - k * 10))) for k in range(10)]
    arc_r = [(uc + R * math.cos(math.radians(90 - k * 10)), SPRING + R * math.sin(math.radians(90 - k * 10))) for k in range(10)]
    span = (-T / 2, T / 2)
    for outline in (arc_l + [(uc - R, top)], arc_r + [(uc + R, top)]):
        parts.add("brick", face.plate(outline, *span))
    ring_out = [(uc + (R + 0.23) * math.cos(math.radians(k * 10)), SPRING + (R + 0.23) * math.sin(math.radians(k * 10))) for k in range(19)]
    ring_in = [(uc + R * math.cos(math.radians(180 - k * 10)), SPRING + R * math.sin(math.radians(180 - k * 10))) for k in range(19)]
    parts.add("brick", face.plate(ring_out + ring_in, -T / 2 - 0.03, T / 2 + 0.03))
    parts.add("sill", face.plate([(uc - R - 0.1, SILL - 0.12), (uc + R + 0.1, SILL - 0.12), (uc + R + 0.1, SILL), (uc - R - 0.1, SILL)],
                                 -T / 2 - 0.04, T / 2 + 0.07))

    def arch_top(u):
        d = u - uc
        return SPRING + math.sqrt(max(0.0, R * R - d * d))

    us = [uc - R + 0.6 * k for k in range(5)]
    vs = [SILL + 0.6 * k for k in range(9)]
    for u in us[1:-1]:
        parts.add("grid", face.plate([(u - 0.02, SILL), (u + 0.02, SILL), (u + 0.02, arch_top(u + 0.02)), (u - 0.02, arch_top(u + 0.02))], -0.03, 0.03))
    for v in vs[1:]:
        if v + 0.02 >= top - 0.05:
            break
        half = R if v <= SPRING else math.sqrt(R * R - (v + 0.02 - SPRING) ** 2)
        parts.add("grid", face.plate([(uc - half, v - 0.02), (uc + half, v - 0.02), (uc + half, v + 0.02), (uc - half, v + 0.02)], -0.03, 0.03))
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
                        parts.add("glass", face.plate([(u0 + s, v0 + t) for s, t in shard], -0.004, 0.004))
                continue
            if min(tops) >= v1:
                outline = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
            else:
                top_pts = [p for k, p in enumerate(kept) if not (0 < k < len(kept) - 1 and kept[k - 1][1] == p[1] == kept[k + 1][1])]
                outline = [(kept[0][0], v0), (kept[-1][0], v0)] + list(reversed(top_pts))
            parts.add("glass", face.plate(outline, -0.004, 0.004))


def framed(face, x):
    """The opening an arched window's frame sits in, as `broken_wall` takes
    it: straight edges, from `FOOT` under the floor."""
    return Opening(u_of(face, x) - R, u_of(face, x) + R, FOOT + SILL, FOOT + SPRING + R, "framed")


def walls(m, rnd, floor_h, yard_h):
    """The four walls, their windows, doors, breach and pilasters. Returns
    the wall objects and their openings, in the Map's own terms, for the
    wear rules."""
    def ground_at(x, y):
        return floor_h(x, y) if 0.2 < x < HALL - 0.2 and 0.2 < y < HALL - 0.2 else yard_h(x, y)

    parts = Parts()
    out = {}
    door = Opening(u_of(SOUTH, 10.45), u_of(SOUTH, 14.55), 0.0, FOOT + 4.55, "doorway", ragged=0.35)
    ops = [framed(SOUTH, x) for x in WIN_S] + [door]
    out["south"] = hall_wall(m, "wall_south", SOUTH, LONG_RUN, ops, 324 * 3 + 1, 0.025, 0.004, ground_at)
    for x in WIN_S:
        window_parts(parts, SOUTH, u_of(SOUTH, x), rnd.child("ws", x), 0.25)
    m.box_h("door_lintel", 12.5, 0.0, 4.7, T + 0.04, 4.45, 4.75, STEEL)

    ops = [framed(NORTH, x) for x in WIN_N]
    out["north"] = hall_wall(m, "wall_north", NORTH, LONG_RUN, ops, 324 * 3 + 2, 0.025, 0.0, ground_at)
    for x in WIN_N:
        window_parts(parts, NORTH, u_of(NORTH, x), rnd.child("wn", x), 0.35)

    breach = Opening(u_of(WEST, 13.1), u_of(WEST, 15.9), FOOT + 0.45, FOOT + 3.05, "breach")
    ops = [framed(WEST, y) for y in WIN_W] + [breach]
    out["west"] = hall_wall(m, "wall_west", WEST, GABLE_RUN, ops, 324 * 3 + 3, 0.0, 0.004, ground_at)
    for y in WIN_W:
        window_parts(parts, WEST, u_of(WEST, y), rnd.child("ww", y), 0.4)

    ops = [
        Opening(u_of(EAST, 7.0), u_of(EAST, 8.1), 0.0, FOOT + 2.2, "doorway", ragged=0.3),
        Opening(u_of(EAST, 12.1), u_of(EAST, 12.9), FOOT + 0.9, FOOT + 1.5, "window", ragged=0.25),
        framed(EAST, 22.5),
    ]
    out["east"] = hall_wall(m, "wall_east", EAST, GABLE_RUN, ops, 324 * 3 + 4, 0.02, 0.0, ground_at)
    window_parts(parts, EAST, u_of(EAST, 22.5), rnd.child("we"), 0.3)

    for name, x in (("gable_west", 0.0), ("gable_east", HALL)):
        # A little under the roof's line: the sheets sag onto it.
        profile = [(-T / 2, EAVE), (HALL + T / 2, EAVE), (HALL + T / 2, roof_under(HALL + T / 2) - 0.02)]
        profile += [(HALL / 2, roof_under(HALL / 2) - 0.04), (-T / 2, roof_under(-T / 2) - 0.02)]
        m.prism(name, list(reversed(profile)), T, (x, 0.0, 0.0), 0.0, BRICK)

    # Brick pilasters under each truss on the long walls' inside faces, set
    # into the wall so its bulge never opens a gap behind them.
    for x in BAYS:
        for y, sgn in ((IN, 1.0), (HALL - IN, -1.0)):
            parts.add("pilaster", g.box_mesh(0.7, 0.34, EAVE - 0.4 + FOOT), g.Pose(at=(x, y + sgn * 0.11, (EAVE - 0.4 - FOOT) / 2)))
    parts.put(m, "brick", "window_brick", BRICK, "trimesh")
    parts.put(m, "sill", "window_sill", "concrete_wall_rough", "trimesh")
    parts.put(m, "grid", "window_grid", STEEL, "trimesh", light_cm=3)
    parts.put(m, "glass", "window_glass", GLASS, "trimesh")
    out["pilasters"] = parts.put(m, "pilaster", "pilasters", BRICK, "trimesh")
    bricks = [b for k in ("south", "north", "west", "east") for b in [(out[k][2], mesh) for mesh in out[k][3]]]
    loose = ruin.Batch()
    for pose, mesh in bricks:
        loose.add(mesh, pose)
    m.solid("loose_bricks", loose.mesh(), None, BRICK, "none")
    return out


def plaster(m, rnd):
    """The north wall's inside face between x = 9.6 and 20.4: a skin of
    painted plaster up to a ragged edge that the damp has peeled back to the
    brick, with a faded sign board on it, and the pale lines where two pipes
    ran before they were stripped."""
    y = HALL - IN + 0.025  # into the wall a little, so its bulge never shows behind the skin
    edge = [(20.4, 3.4)] + [(20.4 - k * 0.6, 3.4 + rnd.uniform(-0.5, 0.9)) for k in range(1, 18)] + [(9.6, 4.2)]
    outline = [(9.6, 0.15), (20.4, 0.15)] + edge
    skins = ruin.Batch()
    skins.add(ruin.plate((0.0, y, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0), outline, 0.045))
    for cx, cz, w, h in ((11.0, 5.6, 1.6, 1.0), (17.6, 6.3, 2.2, 1.2), (14.0, 7.3, 1.0, 0.5)):
        ring = [(cx + w / 2 * math.cos(math.radians(a)) * rnd.uniform(0.7, 1.1), cz + h / 2 * math.sin(math.radians(a)) * rnd.uniform(0.7, 1.1)) for a in range(0, 360, 30)]
        skins.add(ruin.plate((0.0, y, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0), ring, 0.045))
    ob = m.solid("plaster", skins.mesh(), None, "brick_painted_peeling", "none")
    front = y - 0.045
    # The sign: one repeat of `sign_faded` is 4 m, here from x = 16 to 20 and
    # from z = 4 to 8; the board covers the part that holds it.
    m.box("sign_board", (3.2, 0.02, 1.6), (18.0, front - 0.012, 6.0), "sign_faded")
    # Pipe ghosts: two pale lines where the paint was kept from the dirt, and
    # the stubs of the brackets that held the pipes.
    ghosts, stubs = ruin.Batch(), ruin.Batch()
    for z in (0.4, 0.62):
        ghosts.add(g.box_mesh(10.6, 0.004, 0.05), g.Pose(at=(15.0, front - 0.002, z)))
        for x in (10.2, 11.7, 13.2, 14.7, 16.2, 17.7, 19.2):
            stubs.add(g.box_mesh(0.03, 0.06, 0.04), g.Pose(at=(x + rnd.uniform(-0.1, 0.1), front - 0.03, z)))
    m.solid("pipe_ghosts", ghosts.mesh(), None, "plastic_white", "none", light_cm=3)
    m.solid("pipe_stubs", stubs.mesh(), None, STEEL, "none", light_cm=3)
    return ob


def graffiti(m):
    """An old scrawl on the west wall's inside face (no bulge there): one
    repeat of `graffiti_brick` (3.6 m, from y = 0 to 3.6 and z = 0 to 3.6)
    holds it, and a skin 3 mm proud of the bricks shows the part with the
    paint."""
    x = T / 2 + 0.003
    m.box("graffiti", (0.004, 3.024, 1.908), (x, 1.8, 1.746), "graffiti_brick", collider="none")


# ---------------------------------------------------------------- roof

def slope_frame(side):
    """For a slope: its plan coordinates as the `Roof`'s own `(a, s)`."""
    k = math.sqrt(15.6 * 15.6 + (RISE * 15.6 / 15.0) ** 2) / 15.6
    if side == "south":
        return lambda x, y: (x, (y + 0.6) * k)
    return lambda x, y: (HALL - x, (HALL + 0.6 - y) * k)


def skylight(side, rect, state):
    f = slope_frame(side)
    a0, s0 = f(rect[0], rect[1])
    a1, s1 = f(rect[2], rect[3])
    return Skylight(min(a0, a1), max(a0, a1), min(s0, s1), max(s0, s1), state)


def roof(m, rnd):
    """The trusses, sagging purlins and the corrugated iron: skylights gone,
    broken or whole, one bay fallen in, a sheet missing, rust holes. Returns
    the two slopes and their iron for the wear rules."""
    for x in BAYS:
        truss = ruin.Batch()
        y0, y1 = IN, HALL - IN

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
                t0, t1 = (k - 1) / panels, k / panels
                ya = y0 + (HALL / 2 - y0) * t1 if side == 0 else y1 - (y1 - HALL / 2) * t1
                yb = y0 + (HALL / 2 - y0) * t0 if side == 0 else y1 - (y1 - HALL / 2) * t0
                member((x, ya, low + 0.08), (x, ya, chord(ya) - 0.08), 0.09)
                member((x, yb, low + 0.08), (x, ya, chord(ya) - 0.08), 0.08)
        m.solid("truss", truss.mesh(), None, STEEL, "trimesh")

    # The iron: two slopes from gable to gable that sag between the trusses
    # (every 5 m, on the truss lines), the fallen bay's slope three times as
    # much; the fire corner's bay twice.
    eave_z = roof_under(-0.6) + 0.012
    ridge_z = eave_z + RISE * 15.6 / 15.0
    holes = {"south": [], "north": []}
    for side, gone, glass in (("south", SOUTH_GONE, SOUTH_GLASS), ("north", NORTH_GONE, NORTH_GLASS)):
        holes[side] = [skylight(side, r, "gone") for r in gone] + [skylight(side, r, st) for r, st in glass]
    tears = {"south": [], "north": []}
    for side, x, y, r in ROOF_TEARS:
        a, s = slope_frame(side)(x, y)
        tears[side].append(Tear(a, s, r))
    south, north = Roof.gable(
        0.0, -0.6, HALL, HALL + 0.6, eave_z, ridge_z, 324, truss_spacing=5.0, sag=0.12, hole_step=0.25,
        south=dict(skylights=holes["south"], tears=tears["south"], bays={2: 3.0}),
        north=dict(skylights=holes["north"], tears=tears["north"], missing=[NORTH_MISSING_SHEET], bays={0: 2.0}),
    )
    iron = {}
    for side, slope in (("south", south), ("north", north)):
        iron[side] = m.solid(f"roof_{side}", slope.iron(), None, ROOF, "none")
        glass = slope.glass()
        if glass[1]:
            m.solid(f"roof_{side}_glass", glass, None, GLASS, "none")
        # A 0.5 m grid: on the default 1.5 m one a hole narrower than a cell
        # (the missing 1 m sheet, a rust hole) falls between two corners and
        # the collider stays shut over it.
        m.solid(f"roof_{side}_solid", slope.collision_mesh(step=0.5), None, ROOF, "trimesh", collision_only=True)
    # The eaves' overhang past each gable, flat: the trusses there hold it.
    for side, tilt, (ya, yb) in (("south", math.degrees(math.atan2(RISE, HALL / 2.0)), (-0.6, HALL / 2)),
                                 ("north", -math.degrees(math.atan2(RISE, HALL / 2.0)), (HALL / 2, HALL + 0.6))):
        for xa, xb in ((-0.4, 0.04), (HALL - 0.04, HALL + 0.4)):
            sheet(m, f"roof_{side}_verge", xa, ya, xb, yb, tilt, ROOF)

    def sag(side, x):
        return (south if side == "south" else north).sag_at(x if side == "south" else HALL - x)

    # Purlins every 1.47 m down each slope, on the trusses, gable to gable,
    # sagging with the iron they hold.
    purlins = ruin.Batch()

    def purlin(side, y, xa, xb):
        z = roof_under(y) - 0.06
        n = max(2, int((xb - xa) / 1.0))
        pts = [(xa + (xb - xa) * i / n, y, z - sag(side, xa + (xb - xa) * i / n)) for i in range(n + 1)]
        for p, q in zip(pts, pts[1:]):
            purlins.add(g.box_mesh(g.length(g.sub(q, p)) + 0.004, 0.1, 0.12), g.Pose(g.aiming(p, q).rows, g.lerp(p, q, 0.5)))

    lines = [(k, 0.3 + 1.47 * k, "south") for k in range(11)] + [(k, HALL - 0.3 - 1.47 * k, "north") for k in range(10)]
    for k, y, side in lines:
        if side == "south" and k == 5:
            continue  # the bent one, below
        if side == "south" and k in (3, 4):
            purlin(side, y, IN, 10.0)  # torn out over the fallen bay: a stub each side
            purlin(side, y, 15.0, HALL - IN)
            continue
        purlin(side, y, IN, HALL - IN)
    m.solid("purlin", purlins.mesh(), None, STEEL, "trimesh")
    y = 0.3 + 1.47 * 5
    z = roof_under(y) - 0.06
    bent = ruin.bent_bar([(10.0, y, z - sag("south", 10.0)), (11.4, y + 0.1, z - 0.6), (12.6, y + 0.2, z - 1.5), (13.6, y + 0.1, z - 0.7),
                          (15.0, y, z - sag("south", 15.0))], 0.1, 0.12)
    m.solid("purlin_bent", bent.mesh(), None, STEEL, "trimesh")

    # Torn flaps hang into the fallen bay all round, so its edge is never a
    # straight line, and one hangs into the broken skylight at x = 16.5.
    flaps = ruin.Batch()
    for x0, x1, y_edge, side, angle, length in (
        (15.9, 17.1, 10.0, -1.0, 25.0, 0.9),
        (10.6, 11.5, 10.0, -1.0, 55.0, 1.4),
        (11.8, 13.1, 10.0, -1.0, 70.0, 1.1),
        (13.3, 14.4, 10.0, -1.0, 38.0, 0.8),
        (10.6, 11.7, 3.0, 1.0, 30.0, 0.7),
        (12.0, 12.8, 3.0, 1.0, 75.0, 1.0),
        (13.4, 14.4, 3.0, 1.0, 60.0, 1.2),
    ):
        b = (0.0, side * math.cos(math.radians(angle)), -math.sin(math.radians(angle)))
        tear = ruin.jagged((x1, length), (x0, length), rnd, 3, 0.35)
        outline = [(x0, 0.0), (x1, 0.0), (x1, length)] + tear + [(x0, length)]
        xm = (x0 + x1) / 2
        flaps.add(ruin.plate((0.0, y_edge, roof_under(y_edge) - sag("south", xm)), (1.0, 0.0, 0.0), b, outline, SHEET))
    # Along the bay's west and east edges, sheets bent down along the slope.
    up = g.unit((0.0, HALL / 2.0, RISE))
    for x_edge, side, y0, y1, angle in ((10.6, 1.0, 3.6, 6.2, 50.0), (10.6, 1.0, 7.0, 9.4, 70.0), (14.4, -1.0, 3.2, 5.6, 65.0),
                                        (14.4, -1.0, 6.4, 9.6, 40.0)):
        b = (side * math.cos(math.radians(angle)), 0.0, -math.sin(math.radians(angle)))
        length = 0.6 + 0.6 * rnd.unit()
        run = (y1 - y0) / up[1]
        tear = ruin.jagged((run, length), (0.0, length), rnd, 3, 0.3)
        outline = [(0.0, 0.0), (run, 0.0), (run, length)] + tear + [(0.0, length)]
        flaps.add(ruin.plate((x_edge, y0, roof_under(y0) - sag("south", x_edge)), up, b, outline, SHEET))
    m.solid("roof_flap", flaps.mesh(), None, ROOF, "trimesh")
    return {"south": south, "north": north, "iron": iron}


def sheet(m, name, x0, y0, x1, y1, tilt, material):
    """A flat run of roof over the plan rectangle, on the roof's slope."""
    cy = (y0 + y1) / 2.0
    z = roof_under(cy) + SHEET / 2.0
    long = (y1 - y0) / math.cos(math.radians(math.degrees(math.atan2(RISE, HALL / 2.0))))
    m.box(name, (x1 - x0, long, SHEET), ((x0 + x1) / 2.0, cy, z), material, tilt=(tilt, 0.0), collider="trimesh")


# ---------------------------------------------------------------- floor

STRIPS = [IN, 4.4, 9.05, 13.1, 17.6, 21.9, 26.2, HALL - IN]  # the pours, west to east
JOINT = 0.008  # half a joint: a saw cut gone ragged and filled with dirt


def floor_ground():
    """The floor's own unevenness and the dips where water stands under the
    roof's holes (none under the fire pit, which the pilots keep dry)."""
    dips = [Dip(6.5, 7.0, 1.3, 0.04), Dip(6.5, 23.0, 1.2, 0.035), Dip(8.6, 17.5, 1.0, 0.03), Dip(26.5, 11.5, 0.9, 0.03),
            Dip(21.3, 21.0, 1.0, 0.03)]
    return Ground(SEED + 11, amplitude=0.012, wavelength=6.0, base=0.0, dips=dips), dips


def slab_layout(rnd):
    """Slabs poured in strips of their own widths, each strip cut where its
    own pour stopped, so no joint runs on from one strip to the next; each
    slab has settled by a few millimetres and tilts its own way."""
    slabs = []
    for i in range(len(STRIPS) - 1):
        x0, x1 = STRIPS[i], STRIPS[i + 1]
        r = rnd.child("strip", i)
        y = IN
        while y < HALL - IN - 0.01:
            y1 = min(HALL - IN, y + r.uniform(3.4, 6.4))
            if HALL - IN - y1 < 2.0:
                y1 = HALL - IN
            slabs.append((x0, y, x1, y1, r.uniform(-0.004, 0.004), r.uniform(-0.0012, 0.0012), r.uniform(-0.0012, 0.0012)))
            y = y1
    return slabs


def floor_and_clutter(m, rnd, floor, slabs, kit, relief):
    """The floor's slabs over a dark bed that shows in the joints, the
    rubble, the fallen roof, burnt timber and fallen beams."""
    def slab_h(s):
        x0, y0, x1, y1, off, tx, ty = s
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        return lambda x, y: floor.height(x, y) + off + tx * (x - cx) + ty * (y - cy)

    mesh = ruin.Batch()
    for s in slabs:
        x0, y0, x1, y1 = s[:4]
        mesh.add(g.heightfield_mesh(x0 + JOINT, y0 + JOINT, x1 - JOINT, y1 - JOINT, 0.25, relief.height))
    mesh = mesh.mesh()
    # The crumbled layer (#340) shows the rubble picture through the
    # concrete's, as thick as it lies.
    floor_ob = m.solid("floor", mesh, None, FLOOR, "none", blend=(DIRT, relief.blend_weights(mesh[0])))
    m.box_h("floor_bed", HALL / 2, HALL / 2, HALL - T, HALL - T, -0.14, -0.1, FLOOR, collider="none", light_cm=40)
    m.solid("floor_solid", g.heightfield_mesh(IN, IN, HALL - IN, HALL - IN, 0.5, relief.height), None, FLOOR, "trimesh", collision_only=True)

    # Under the fallen bay: a heap of rubble from the kit (#339), the roof's
    # sheets and a purlin on it.
    heap = kit.heap((12.5, 6.5), 3.3, 0.85, core=DIRT)
    fallen = ruin.Batch()
    for cx, cy, yaw, pitch, roll in ((11.6, 5.0, 12.0, 8.0, 14.0), (13.6, 7.8, -30.0, -12.0, 6.0), (10.6, 8.6, 70.0, 4.0, -18.0)):
        fallen.add(g.box_mesh(1.0, 2.6, SHEET), g.Pose(ruin.turned(yaw, pitch, roll), (cx, cy, max(0.05, heap(cx, cy)) + 0.15)))
    m.solid("roof_fallen", fallen.mesh(), None, ROOF, "trimesh")
    m.beam("purlin_fallen", (8.6, 3.4, 0.06), (13.9, 7.6, 0.75), 0.1, 0.12, STEEL, collider="box")
    m.beam("beam_fallen", (21.0, 13.0, 0.11), (26.5, 17.5, 0.11), 0.16, 0.22, STEEL, collider="box")

    # Both sides of the breach: what fell out of the wall.
    for name, cx, rx in (("rubble_breach_in", 1.6, 1.4), ("rubble_breach_out", -1.6, 1.4)):
        h = ruin.mound(cx, 14.5, rx, 2.2, 0.45, rnd.child(name))
        m.heightfield(name, cx - rx, 12.3, cx + rx, 16.7, 0.2, h, DIRT)
        b = ruin.Batch()
        ruin.scatter(b, rnd.child(name, "bricks"), (cx, 14.5), 2.0, 70, ruin.brick, z=lambda x, y, h=h: max(0.0, h(x, y)))
        m.solid(name + "_bricks", b.mesh(), None, BRICK, "trimesh")

    # Drifts of dirt and leaves in the corners, deepest in the south-east,
    # downwind of the breach and the loading door.
    drifts = ruin.Batch()
    for k, (cx, cy, rx, ry, h) in enumerate(((29.2, 0.9, 1.6, 1.3, 0.16), (0.9, 0.9, 0.9, 0.8, 0.08), (0.9, 29.1, 1.0, 0.9, 0.09),
                                            (15.0, 0.75, 1.4, 0.5, 0.06), (29.3, 12.0, 0.5, 1.2, 0.05))):
        d = ruin.mound(cx, cy, rx, ry, h, rnd.child("drift", k))
        drifts.add(g.heightfield_mesh(max(IN, cx - rx), max(IN, cy - ry), min(HALL - IN, cx + rx), min(HALL - IN, cy + ry), 0.2, d))
    m.solid("drifts", drifts.mesh(), None, DIRT, "none")

    # Burnt timber in the north-east corner: what is left of the squatters'
    # fire, on a low bed of ash and char.
    ax, ay = ASH
    ash = ruin.mound(ax, ay, 2.3, 2.0, 0.18, rnd.child("ash"))
    m.heightfield("ash", ax - 2.3, ay - 2.0, ax + 2.3, ay + 2.0, 0.25, ash, CHAR)
    planks = ruin.Batch()
    q = rnd.child("planks")
    for _ in range(22):
        length = q.uniform(0.8, 2.2)
        x, y = ax + q.uniform(-1.7, 1.7), ay + q.uniform(-1.4, 1.4)
        pose = g.Pose(ruin.turned(q.uniform(0, 360), q.uniform(-25, 25), q.uniform(-10, 10)), (x, y, max(0.0, ash(x, y)) + q.uniform(0.05, 0.3)))
        planks.add(g.box_mesh(length, q.uniform(0.08, 0.15), q.uniform(0.03, 0.06)), pose)
    m.solid("burnt_timber", planks.mesh(), None, CHAR, "trimesh")

    # Loose bricks and chunks along the foot of the walls.
    b = ruin.Batch()
    q = rnd.child("loose")
    for x, y in ((19.0, 0.8), (29.0, 5.0), (8.0, 29.0), (0.9, 26.0), (0.8, 4.5)):
        ruin.scatter(b, q, (x, y), 0.7, 8, ruin.brick)
        ruin.scatter(b, q, (x, y), 0.6, 4, lambda r: ruin.lump(r.uniform(0.1, 0.3), r))
    m.solid("loose_rubble", b.mesh(), None, "concrete_wall_rough", "trimesh")
    return floor_ob, heap, ash


def aisle_lines(m, height):
    """The old walkway lines down the aisle from the loading door, 3 m apart,
    painted yellow and worn away in the middle of the way, running on under
    the rubble heap (the roof fell after the works closed), on the floor's
    `height`."""

    lines = ruin.Batch()
    rnd = Random(SEED, "aisle")
    for x in (11.0, 14.0):
        y = 0.6
        while y < 26.0:
            length = rnd.uniform(0.6, 2.4)
            gap = rnd.uniform(0.05, 0.6) if rnd.chance(0.35) else 0.0
            ya, yb = y, min(26.0, y + length)
            w = 0.1
            lines.add(g.heightfield_mesh(x - w / 2, ya, x + w / 2, yb, 0.5, lambda xx, yy: height(xx, yy) + 0.003))
            y = yb + gap
    m.solid("aisle_lines", lines.mesh(), None, "paint_yellow", "none")


# ---------------------------------------------------------------- the kit (#341)

# The kit's own materials (#339) are the library's downloaded ones; the
# Factory draws only pictures made from scratch, so each becomes the
# Factory's nearest own material.
KIT_MATERIALS = {
    "steel_rusty": STEEL, "steel_painted": STEEL, "wood_ramp": WOOD, "concrete_smooth": "concrete_wall_rough",
    "concrete_dark": "plastic_black", "steel_grey": "can_painted", "container_blue": "plastic_blue",
    "container_red": "plastic_red", "rubble": DIRT,
}


class KitOnFactory:
    """The MapBuilder as the kit's `apply` sees it: every material put
    through `KIT_MATERIALS`, and the shapes named in `grow` given a scatter
    tag, so a heap's core grows grass and weeds out of the rubble."""

    def __init__(self, m, grow=None):
        self.m, self.grow = m, grow or {}

    def solid(self, name, mesh, pose, material, collider, **kw):
        if name in self.grow:
            kw["scatter"] = self.grow[name]
        return self.m.solid(name, mesh, pose, KIT_MATERIALS.get(material, material), collider, **kw)

    def box(self, name, size, centre, material, **kw):
        return self.m.box(name, size, centre, KIT_MATERIALS.get(material, material), **kw)

    def __getattr__(self, name):
        return getattr(self.m, name)


def kit_on_factory(kit):
    """Turns every part the kit has put down into the Factory's materials
    before `apply` merges them, so one material in one cell is one draw."""
    for p in kit.placements:
        p.parts = [(KIT_MATERIALS.get(mat, mat), mesh) for mat, mesh in p.parts]
    kit.cores = [(rule, KIT_MATERIALS.get(mat, mat), mesh) for rule, mat, mesh in kit.cores]


SKY_DROPS = [(6.5, 7.0), (21.5, 8.5), (6.5, 23.0), (21.5, 20.0), (8.5, 22.0)]  # crates/pack/tests/factory.rs


def hall_kit(floor_h):
    """The hall's mess, from the kit's rules (#339): keep-clear zones round
    every line a pilot flies and every prop the story already placed."""
    s = Scatter(SEED + 60, floor_h, cell=15.0)
    for x0, y0, x1, y1 in ((-1.0, -1.0, IN + 0.02, HALL + 1.0), (HALL - IN - 0.02, -1.0, HALL + 1.0, HALL + 1.0),
                           (-1.0, -1.0, HALL + 1.0, IN + 0.02), (-1.0, HALL - IN - 0.02, HALL + 1.0, HALL + 1.0)):
        s.keep_clear_rect(x0, y0, x1, y1)  # the walls themselves
    s.keep_clear_rect(11.0, 10.2, 14.0, HALL)  # the aisle north of the heap
    s.keep_clear_rect(10.2, -1.0, 14.8, 2.6)  # the loading door's way in
    s.keep_clear_rect(-1.0, 12.6, 3.2, 16.4)  # the breach and its rubble
    s.keep_clear_rect(27.6, 6.6, 31.0, 8.6)  # the man door
    s.keep_clear_rect(28.4, 11.9, 31.0, 13.1)  # the vent hole
    for x, y in SKY_DROPS:
        s.keep_clear_circle(x, y, 0.7)
    for x, y, sx, sy in ((6.0, 20.0, 3.0, 1.6), (8.9, 23.2, 2.4, 1.4), (18.0, 22.5, 2.0, 1.2)):
        s.keep_clear_rect(x - sx / 2 - 0.3, y - sy / 2 - 0.3, x + sx / 2 + 0.3, y + sy / 2 + 0.3)  # the plinths
    s.keep_clear_circle(21.6, 7.2, 3.3)  # the pilots' hangout
    s.keep_clear_circle(ASH[0], ASH[1], 2.5)  # the fire corner's ash
    s.keep_clear_rect(1.2, IN, 4.8, 2.4)  # the scrap pile
    s.keep_clear_rect(6.4, 24.6, 10.8, 28.6)  # the filing cabinet and the chair
    s.keep_clear_rect(20.6, 12.6, 27.0, 18.0)  # the fallen beam
    return s


def hall_mess(m, kit, floor_ob):
    """Thickness and mess on the hall's floor (references 3 and 4 of #341):
    crumbled plaster and brick along every wall's foot, a floor strewn in
    clusters on low beds of fallen render, bolt plates and anchor stubs where
    a column line was cut, pallets and boards piled in corners, a plank and
    a pipe run, puddles and moss."""
    kit.wall_foot((0.4, IN), (HALL - 0.4, IN), depth=1.0, height=0.12, core="concrete_wall_rough")
    kit.wall_foot((HALL - IN, 0.4), (HALL - IN, HALL - 0.4), depth=0.9, height=0.12, core="concrete_wall_rough")
    kit.wall_foot((HALL - 0.4, HALL - IN), (0.4, HALL - IN), depth=1.3, height=0.16, core="concrete_wall_rough")
    kit.wall_foot((IN, HALL - 0.4), (IN, 0.4), depth=1.1, height=0.14, core="concrete_wall_rough")
    kit.strew((IN, IN, HALL - IN, HALL - IN), 1.1, bed="concrete_wall_rough", bed_height=(0.04, 0.09))
    kit.anchor_row((16.5, 11.4), (28.5, 11.4), 4.0, missing=0.0)
    kit.anchor_row((3.6, 3.6), (3.6, 11.4), 3.9)
    kit.pallets(2.4, 27.4, 0.0, stacks=(2, 3))
    kit.pallets(26.2, 1.7, 0.0)
    kit.boards(4.9, 9.2, 75.0)
    kit.boards(16.6, 27.2, 8.0)
    kit.place("plank", 17.4, 14.6, 22.0)
    kit.place("plank", 8.2, 13.2, 100.0)
    kit.place("pipe_run", 16.2, 18.0, 84.0)
    kit.place("drum", 28.5, 28.3, 0.0, k=0)
    kit.place("drum_lying", 27.6, 3.9, 70.0)
    kit.place("cable_tangle", 9.4, 4.0, 0.0)
    kit.patches((IN + 0.5, IN + 0.5, HALL - IN - 0.5, HALL - IN - 0.5), 14, on=[floor_ob])


def yard_kit(m, yard_h, yard_pieces):
    """The yard's mess: a rubble heap in front of the hall with tall grass
    and weeds growing out of it (reference 2), a smaller one by the west
    trees, rubble fallen from the south wall, odd bits in the grass and
    puddles on the track."""
    s = Scatter(SEED + 70, yard_h, cell=24.0)
    s.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.8)
    for a, b in zip(PATH.line, PATH.line[1:]):
        n = max(1, int(math.hypot(b[0] - a[0], b[1] - a[1]) / 0.8))
        for k in range(n):
            s.keep_clear_circle(a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n, 0.75)
    s.keep_clear_rect(9.0, -6.0, 16.0, 0.0)  # the hard standing before the loading door
    s.keep_clear_rect(-1.0, -0.2, HALL + 1.0, HALL + 1.0)  # the hall
    s.keep_clear_rect(-2.6, 12.4, 0.0, 16.6)  # the breach's way in
    s.keep_clear_rect(-15.0, -31.0, -7.0, -26.0)  # the gate
    for x, y, r in ((-6.0, -6.5, 0.7), (-9.5, 9.0, 0.7), (27.0, -8.0, 0.7), (-4.5, 26.0, 0.7), (24.4, -14.8, 2.2),
                    (6.0, -9.0, 0.7), (-4.2, -3.2, 1.0), (31.6, -1.8, 0.6), (15.6, -12.0, 0.9)):
        s.keep_clear_circle(x, y, r)  # the yard's trees and the story's props
    tall = s.heap((22.4, -6.6), 2.7, 1.3, toward=(0.0, 1.0))
    s.heap((-7.2, -13.0), 2.0, 0.8)
    s.wall_foot((HALL + 0.2, -IN), (-0.2, -IN), depth=0.9, height=0.1, core=DIRT)
    s.strew((-12.0, -24.0, 40.0, 40.0), 0.06)
    s.place("tyre", 19.6, -8.4, 0.0)
    s.place("drum_lying", 25.6, -5.0, 120.0)
    s.place("pipe", 4.0, -4.6, 30.0)
    kit_on_factory(s)
    s.apply(KitOnFactory(m, {"yard_kit_heap": ("grass", 9, (0.25, 0.9))}), name="yard_kit", light_cm=20)
    track = Scatter(SEED + 71, yard_h)
    track.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.0)
    track.patches((4.0, -20.0, 16.0, -1.0), 7, on=yard_pieces, kinds=("damp", "damp", "algae"), size=(0.9, 2.2))
    track.apply(m, name="yard_puddles")
    return s.stats(), tall


# ---------------------------------------------------------------- yard

def grassiness(x, y):
    """How much grass shows over the yard's dirt at `(x, y)`, from 0 (bare
    dirt) to 1, a whole number of 255ths: grass along the sunny south and
    west walls' feet and in patches, thickening away from the hall and the
    hard standing in front of the loading door, and none on the path the
    pilots have worn from the gate to the Launch Spot and on to the door."""
    seed = SEED + 41
    # Away from the hall, the grass takes over.
    dx = max(-x, 0.0, x - HALL)
    dy = max(-y, 0.0, y - HALL)
    far = math.sqrt(dx * dx + dy * dy) + 8.0 * (fbm2(seed, x, y, 9.0, 3))
    w = 0.9 * smoothstep((far - 18.0) / 16.0) * smoothstep(fbm2(seed + 1, x, y, 7.0, 3) * 2.5 + 0.6)
    # #341: the yard has gone back to grass everywhere but the track, in
    # patches of thinner grass over gravel where something stood or drove.
    w = max(w, 0.7 * smoothstep(fbm2(seed + 2, x, y, 6.0, 3) * 3.0 + 0.45))
    # Strips along the walls' feet where the sun falls and nothing drives.
    for edge in (SOUTH_STRIP, WEST_STRIP, NORTH_STRIP):
        w = max(w, edge.weight(x, y))
    for patch in PATCHES:
        w = max(w, patch.weight(x, y))
    # The hard standing before the loading door and the path: bare.
    hard = HARD.weight(x, y)
    path = PATH.weight(x, y)
    w = w * (1.0 - hard) * (1.0 - path)
    return round(min(1.0, max(0.0, w)) * 255.0) / 255.0


def _ring(cx, cy, rx, ry, n=16):
    """An anticlockwise ellipse: a `WanderingEdge` line with grass inside."""
    return [(cx + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n)) for k in range(n + 1)]


class Band:
    """A soft-edged band along a polyline: 1 within `half` metres of it,
    falling to 0 over `fade`, its edges wandering by `wander` metres."""

    def __init__(self, line, half, fade, wander, seed):
        self.line, self.half, self.fade, self.wander, self.seed = line, half, fade, wander, seed

    def weight(self, x, y):
        best = 1e9
        for (ax, ay), (bx, by) in zip(self.line, self.line[1:]):
            vx, vy = bx - ax, by - ay
            ll = vx * vx + vy * vy
            t = max(0.0, min(1.0, ((x - ax) * vx + (y - ay) * vy) / ll))
            px, py = ax + vx * t, ay + vy * t
            best = min(best, math.sqrt((x - px) * (x - px) + (y - py) * (y - py)))
        d = best - self.half - self.wander * noise2(self.seed, x / 2.2, y / 2.2)
        return 1.0 - smoothstep(d / self.fade + 0.5)


# Grass on the left of each line (WanderingEdge): along the walls' outside
# feet, from the corner outward.
SOUTH_STRIP = WanderingEdge([(31.0, -2.4), (-2.6, -1.6)], seed=SEED + 1, wander=0.9, band=1.2, wavelength=5.0, patches=0.6)
WEST_STRIP = WanderingEdge([(-2.0, -2.0), (-2.4, 31.0)], seed=SEED + 2, wander=1.0, band=1.3, wavelength=5.0, patches=0.6)
NORTH_STRIP = WanderingEdge([(-2.0, 32.4), (31.0, 31.8)], seed=SEED + 3, wander=1.2, band=1.6, wavelength=6.0, patches=0.6)
PATCHES = [WanderingEdge(_ring(cx, cy, rx, ry), seed=SEED + 10 + k, wander=1.0, band=1.4, wavelength=4.0, patches=0.7)
           for k, (cx, cy, rx, ry) in enumerate(((-7.5, -8.0, 4.5, 3.5), (-10.0, 8.0, 4.0, 4.5), (2.0, -6.0, 2.5, 1.6),
                                                 (27.0, -8.0, 3.0, 2.4), (6.0, -22.0, 5.0, 3.0), (34.0, 12.0, 3.0, 5.0)))]
HARD = Band([(9.0, -2.5), (16.0, -2.5)], 1.6, 1.8, 1.0, SEED + 30)
PATH = Band([(-11.0, -31.0), (-8.5, -26.0), (-2.0, -21.5), (5.0, -17.8), (10.5, -14.8), (12.6, -11.0), (12.2, -6.0), (12.5, -3.0)],
            0.35, 0.9, 0.35, SEED + 31)

YARD = (-18.0, -32.0, 48.0, 46.0)  # the yard's near ground: fine light, grass blended into dirt
MID = (-70.0, -70.0, 110.0, 110.0)  # beyond it, coarser, fading to level
GATE = (-14.0, -8.0)  # the boundary wall's opening on y = -30, west to east


# Real grass (#338) by how grassy the ground is under each triangle's middle:
# (least grassiness, the scatter tag). The yard's near ground grows it thick
# and tall; the ground beyond, which the camera sees from further off, thinner.
YARD_GRASS = [(0.5, ("grass", 7, (0.14, 0.5))), (0.15, ("grass", 2.5, (0.08, 0.3)))]
MID_GRASS = [(0.5, ("grass", 2.0, (0.15, 0.45))), (0.15, ("grass", 0.8, (0.08, 0.3)))]


GRASS_REACH = (-26.0, -42.0, 56.0, 56.0)  # no real grass past this: the camera never sees it near


def grass_split(mesh, bands):
    """Cuts a ground's `(corners, faces)` into a mesh for each band of
    `bands` (and one for the bare rest) by the grassiness under each face's
    middle. Returns `[(scatter tag or None, mesh)]`, in a fixed order, leaving
    out empty ones. Ground outside `GRASS_REACH` grows none: every 8 m
    square of grass is one more thing the game handles each frame, drawn or
    not, and from the yard the grass that far off is gone by distance."""
    corners, faces = mesh
    groups = [[] for _ in range(len(bands) + 1)]
    for f in faces:
        cx = sum(corners[i][0] for i in f) / len(f)
        cy = sum(corners[i][1] for i in f) / len(f)
        w = grassiness(cx, cy)
        r0, r1, r2, r3 = GRASS_REACH
        if (not (r0 <= cx <= r2 and r1 <= cy <= r3) or PATH.weight(cx, cy) > 0.25 or HARD.weight(cx, cy) > 0.25
                or math.hypot(cx - LAUNCH[0], cy - LAUNCH[1]) < 3.2):
            w = 0.0  # out of reach, the worn track, the hard standing or the Launch Spot's trodden ground: bare
        k = next((i for i, (least, _) in enumerate(bands) if w >= least), len(bands))
        groups[k].append(f)
    out = []
    for k, fs in enumerate(groups):
        if not fs:
            continue
        index = {}
        for f in fs:
            for i in f:
                if i not in index:
                    index[i] = len(index)
        own = [None] * len(index)
        for i, j in index.items():
            own[j] = corners[i]
        out.append((bands[k][1] if k < len(bands) else None, (own, [[index[i] for i in f] for f in fs])))
    return out


def flying_lines(seed):
    """The lines a pilot flies, as keep-clear zones (#339) for the relief
    (#340): it keeps them within 2 cm of the plain ground."""
    fly = Scatter(seed)
    fly.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.8)
    fly.keep_clear_rect(11.0, 10.2, 14.0, HALL - IN)  # the aisle
    fly.keep_clear_rect(10.2, -1.5, 14.8, 2.6)  # the loading door
    fly.keep_clear_rect(-2.0, 12.6, 2.0, 16.4)  # the breach
    fly.keep_clear_rect(28.0, 6.6, 31.5, 8.6)  # the man door
    for x, y in SKY_DROPS:
        fly.keep_clear_circle(x, y, 0.6)
    return fly


def yard_relief(plain):
    """The yard's relief (#340): ruts where the scrappers' vans came in from
    the east and stood in front of the loading door, potholes in the hard
    standing, and a swell under each rubble heap."""
    fly = flying_lines(SEED + 81)
    yard = Relief(plain.height, seed=SEED + 82, clear=fly.clearance, bumps=0.02)
    yard.rut((21.0, -29.0), (15.8, -3.0), gauge=1.8, depth=0.08)
    yard.rut((15.8, -3.0), (6.0, -4.2), gauge=1.8, depth=0.06)
    for x, y, r, d in ((9.6, -3.6, 0.55, 0.08), (15.6, -1.6, 0.7, 0.1), (17.8, -9.5, 0.5, 0.07), (8.2, -6.8, 0.45, 0.06)):
        yard.pothole(x, y, r, d)
    yard.under_heap(22.4, -6.6, 3.4, 0.25)
    yard.under_heap(-7.2, -13.0, 2.6, 0.15)
    return yard


def yard(m, rnd):
    """Outside: uneven yard ground of dirt with grass thinning into it, a
    path worn through it, trees, weeds and the boundary wall's gate. Returns
    the ground's height and its parts."""
    plain = Ground(SEED + 21, amplitude=0.12, wavelength=13.0, slope=(0.002, -0.002), base=-0.02,
                   flats=[Flat(LAUNCH[0], LAUNCH[1], 3.5, blend=1.5, level=0.0)])
    ground = yard_relief(plain)
    pieces = []
    x0, y0, x1, y1 = YARD
    # The yard in four pieces round the hall (the hall's floor is its own),
    # on one grid so no crack shows between them.
    # Each piece is cut by how grassy it is (#341): real grass grows thick
    # where the yard has gone back to grass, thin over the gravel patches,
    # and none on the track the pilots have worn.
    for tag, rect in (("south", (x0, y0, x1, 0.0)), ("north", (x0, HALL, x1, y1)), ("west", (x0, 0.0, 0.0, HALL)), ("east", (HALL, 0.0, x1, HALL))):
        for grow, mesh in grass_split(g.heightfield_mesh(*rect, 0.5, ground.height), YARD_GRASS):
            pieces.append(m.solid("yard", mesh, None, DIRT, "trimesh", light_cm=20, scatter=grow,
                                  blend=(GRASS, [grassiness(c[0], c[1]) for c in mesh[0]])))
    # Beyond, to 70 m out: the same ground on a 2 m grid, easing to level at
    # its edge, reaching 2 m in under the yard and 2 cm lower so no seam shows.
    def mid_h(x, y):
        edge = min(x - MID[0], MID[2] - x, y - MID[1], MID[3] - y)
        return (plain.height(x, y) - 0.02) * smoothstep(edge / 12.0) - 0.02 * (1.0 - smoothstep(edge / 12.0))
    for tag, rect in (("west", (MID[0], MID[1], x0 + 2.0, MID[3])), ("east", (x1 - 2.0, MID[1], MID[2], MID[3])),
                      ("south", (x0 + 2.0, MID[1], x1 - 2.0, y0 + 2.0)), ("north", (x0 + 2.0, y1 - 2.0, x1 - 2.0, MID[3]))):
        for grow, mesh in grass_split(g.heightfield_mesh(*rect, 2.0, mid_h), MID_GRASS):
            m.solid("ground_near", mesh, None, DIRT, "trimesh", light_cm=40, scatter=grow,
                    blend=(GRASS, [max(grassiness(c[0], c[1]), smoothstep((min(c[0] - MID[0], MID[2] - c[0], c[1] - MID[1], MID[3] - c[1]) - 0.0) / -1.0 + 1.0)) for c in mesh[0]]))
    m.slab("ground", -1000.0, -1000.0, 1000.0, 1000.0, -0.02, 0.3, GRASS, holes=[MID])

    weeds = ruin.Batch()
    q = rnd.child("weeds")
    spots = [(x, -0.6) for x in (1.0, 3.6, 5.2, 9.4, 15.2, 19.0, 20.6, 24.4, 26.0, 29.2)]
    spots += [(-0.6, y) for y in (2.0, 5.0, 9.6, 11.8, 17.4, 19.6, 25.0, 28.0)]
    spots += [(x, 30.7) for x in (3.0, 11.0, 18.5, 26.5)]
    for cx, cy, rx, ry in ((-7.5, -8.0, 4.5, 3.5), (-10.0, 8.0, 4.0, 4.5), (2.0, -6.0, 2.5, 1.6), (27.0, -8.0, 3.0, 2.4), (6.0, -22.0, 5.0, 3.0)):
        p = rnd.child("patch", cx)
        spots += [(cx + rx * p.uniform(-0.6, 0.6), cy + ry * p.uniform(-0.6, 0.6)) for _ in range(6)]
    # Along the path's edges, where feet don't quite reach.
    for k in range(10):
        t = (k + 0.5) / 10
        a, b = PATH.line[int(t * (len(PATH.line) - 1))], PATH.line[int(t * (len(PATH.line) - 1)) + 1]
        f = t * (len(PATH.line) - 1) % 1.0
        px, py = a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f
        side = 1.0 if k % 2 else -1.0
        spots.append((px + side * 0.9, py - side * 0.5))
    for x, y in spots:
        x += q.uniform(-0.3, 0.3)
        y += q.uniform(-0.3, 0.3)
        if math.hypot(x - LAUNCH[0], y - LAUNCH[1]) < 3.6:
            continue
        clump = plants.weeds(q, (x, y), q.uniform(0.2, 0.4), q.pick((10, 14, 18)), q.uniform(0.35, 0.8), ground.height(x, y))
        weeds.add(clump.mesh())
    # Inside, where light and rain get in: by the breach, on the heap, under
    # the windows, under the roof's holes and in the slab joints there.
    for x, y, z in ((1.2, 12.6, 0.0), (1.0, 16.6, 0.0), (0.7, 19.0, 0.0), (11.0, 3.4, 0.2), (14.4, 9.6, 0.15), (7.5, 0.6, 0.0),
                    (22.5, 0.6, 0.0), (17.0, 0.7, 0.0), (5.6, 6.2, 0.0), (7.2, 8.1, 0.0), (6.4, 22.6, 0.0), (8.6, 18.6, 0.0),
                    (9.05, 16.1, 0.0), (26.2, 11.0, 0.0), (21.9, 9.4, 0.0)):
        weeds.add(plants.weeds(q, (x, y), 0.25, 12, q.uniform(0.25, 0.55), z).mesh())
    m.solid("weeds", weeds.mesh(), None, "leaves_mixed", "none", light_cm=2)

    for k, (x, y, h) in enumerate(((-6.0, -6.5, 8.5), (-9.5, 9.0, 9.5), (27.0, -8.0, 7.0), (-4.5, 26.0, 6.0))):
        wood, crown = plants.tree(rnd.child("tree", k), (x, y, ground.height(x, y) - 0.05), h, leaves=1800)
        m.solid("tree", wood.mesh(), None, "bark_rough", "trimesh")
        m.solid("tree_leaves", crown.mesh(), None, "leaves_mixed", "none", light_cm=7)
    # A young birch inside, in the rubble that fell from the breach, under the
    # rust hole in the roof: the rain and the light reach it there.
    wood, crown = plants.tree(rnd.child("birch"), (2.3, 17.6, 0.0), 3.4, leaves=420)
    m.solid("tree", wood.mesh(), None, "bark_rough", "trimesh")
    m.solid("tree_leaves", crown.mesh(), None, "leaves_mixed", "none", light_cm=7)
    return ground, pieces


CHIMNEY = (38.5, 36.0)  # the works' chimney, north-east of the hall, beyond the yard


def silhouette(m):
    """What makes the hall corner an object against the sky (references 1 and
    2 of #341): purlins that came down with the fallen bay hang bent from its
    trusses, and the works' tall chimney, banded in pale render, stands
    behind the hall."""
    for name, pts in (("purlin_hanging", [(10.0, 3.6, 9.4), (11.4, 3.7, 8.3), (12.2, 3.75, 6.9)]),
                      ("purlin_hanging", [(15.0, 9.4, 10.8), (13.9, 9.3, 9.6), (13.4, 9.25, 8.1)])):
        for a, b in zip(pts, pts[1:]):
            m.beam(name, a, b, 0.1, 0.12, STEEL, collider="box")
    x, y = CHIMNEY
    m.box_h("chimney_base", x, y, 4.2, 4.2, -0.3, 2.4, BRICK, light_cm=30)
    z, r = 2.4, 1.7
    for k, (rise, material) in enumerate(((7.0, BRICK), (0.8, "concrete_wall_rough"), (6.0, BRICK), (0.8, "concrete_wall_rough"),
                                          (6.0, BRICK), (0.8, "concrete_wall_rough"), (4.6, BRICK))):
        top = r - 0.035 * rise
        m.solid("chimney", g.cylinder_mesh(rise, r, top, 16), g.Pose(at=(x, y, z)), material, "convex", light_cm=30)
        z, r = z + rise, top


def backdrop(m, rnd):
    """What stands round the yard, all solid, and nothing worth flying to past
    about 100 m: the rest of the works, a boundary wall with its gate and a
    tree line."""
    for x, y, sx, sy, h, mat in (
        (62.0, 15.0, 30.0, 60.0, 12.0, BRICK),
        (15.0, 68.0, 60.0, 22.0, 9.0, BRICK),
        (-40.0, 40.0, 18.0, 30.0, 7.0, "concrete_wall_rough"),
        (45.0, -45.0, 24.0, 18.0, 6.0, "concrete_wall_rough"),
    ):
        m.box_h("works", x, y, sx, sy, -0.3, h, mat, light_cm=40)
        m.box_h("works_roof", x, y, sx + 0.6, sy + 0.6, h, h + 0.15, ROOF, light_cm=40)
    g0, g1 = GATE
    m.wall("boundary_wall", (-30.0, -30.0), (40.0, -30.0), -0.3, 2.9, 0.3, "concrete_wall_rough", openings=[(g0 + 30.0, g1 + 30.0, 0.0, 2.9)], light_cm=30)
    m.wall("boundary_wall", (-30.0, -30.0), (-30.0, 22.0), -0.3, 2.9, 0.3, "concrete_wall_rough", light_cm=30)
    for k, (x, y, h) in enumerate(((-22.0, -36.0, 10.0), (-8.0, -38.0, 12.0), (8.0, -37.0, 9.0), (28.0, -38.0, 11.0), (-38.0, -14.0, 10.0), (-37.0, 6.0, 12.0))):
        wood, crown = plants.tree(rnd.child("backdrop_tree", k), (x, y, -0.05), h, leaves=900)
        m.solid("backdrop_tree", wood.mesh(), None, "bark_rough", "trimesh", light_cm=30)
        m.solid("backdrop_tree_leaves", crown.mesh(), None, "leaves_mixed", "none", light_cm=12)


# ---------------------------------------------------------------- the story's props

def story(m, rnd, heap, ash, yard_h):
    """The props of each layer of the Factory's history, few and placed for
    a reason (docs/research/lived-in-places.md, "The Factory's story").
    Returns what the wear rules need to know: where machines stood, where the
    plinths and the crane rail's brackets are."""
    solid, drawn = Parts(), Parts()
    out = {}

    # ---- 1. The works: plinths, the crane rail, the cable tray.
    plinths = [(6.0, 20.0, 3.0, 1.6, 0.25), (8.9, 23.2, 2.4, 1.4, 0.2), (18.0, 22.5, 2.0, 1.2, 0.3)]
    for k, (x, y, sx, sy, h) in enumerate(plinths):
        r = rnd.child("plinth", k)
        solid.add("concrete", props.plinth(r, sx, sy, h), g.Pose(props.turned(r.uniform(-1.5, 1.5)), (x, y, -0.02)))
        drawn.add("steel", props.bolts(sx, sy, h), g.Pose(props.turned(), (x, y, -0.02)))
    out["plinths"] = [(x, y) for x, y, *_ in plinths]
    brackets = []
    for y_wall, sgn in ((IN, 1.0), (HALL - IN, -1.0)):
        y_rail = y_wall + sgn * 0.58
        for x in BAYS:
            solid.add("steel", g.box_mesh(0.16, 0.42, 0.18), g.Pose(at=(x, y_wall + sgn * 0.48, 6.42)))
            brackets.append((x, y_wall + sgn * 0.28, 6.36, sgn))
        if sgn > 0:
            # The south rail was cut at x = 4 and its end hangs down.
            solid.add("steel", g.box_mesh(HALL - IN - 4.0, 0.12, 0.15), g.Pose(at=((4.0 + HALL - IN) / 2, y_rail, 6.585)))
            for p, q in zip([(4.0, y_rail, 6.585), (3.2, y_rail - 0.05, 6.0), (2.7, y_rail - 0.12, 5.1)],
                            [(3.2, y_rail - 0.05, 6.0), (2.7, y_rail - 0.12, 5.1), (2.55, y_rail - 0.15, 4.5)]):
                solid.add("steel", g.box_mesh(g.length(g.sub(q, p)) + 0.02, 0.12, 0.15), g.Pose(g.aiming(p, q).rows, g.lerp(p, q, 0.5)))
        else:
            solid.add("steel", g.box_mesh(HALL - T, 0.12, 0.15), g.Pose(at=(HALL / 2, y_rail, 6.585)))
    out["brackets"] = brackets
    # The empty cable tray along the north wall at 3.2 m, sagging in one span
    # where a bracket gave, cut cable ends hanging from it.
    ty = HALL - IN - 0.17
    pts = [(0.5, ty, 3.2), (3.0, ty, 3.2), (5.4, ty, 3.2), (6.3, ty, 2.95), (7.2, ty, 2.82), (8.2, ty, 3.0), (9.5, ty, 3.2)]
    for p, q in zip(pts, pts[1:]):
        rows = g.aiming(p, q).rows
        c = g.lerp(p, q, 0.5)
        length = g.length(g.sub(q, p)) + 0.01
        drawn.add("steel", g.box_mesh(length, 0.3, 0.006), g.Pose(rows, c))
        for side in (-1, 1):
            drawn.add("steel", g.box_mesh(length, 0.006, 0.08), g.Pose(rows, g.add(c, (0.0, side * 0.147, 0.04))))
    for x, drop in ((1.6, 0.5), (4.1, 0.8), (8.8, 0.35)):
        drawn.add("cable", g.tube_mesh([(x, ty, 3.2), (x + 0.05, ty - 0.02, 3.2 - drop * 0.6), (x + 0.12, ty - 0.05, 3.2 - drop)], 0.012, 6))

    # ---- 2. Closure and stripping: the fuse box, the scrap pile, the
    # filing cabinet and the office chair the thieves left.
    solid.add("steel_painted", props.fuse_box(110.0), g.Pose(props.turned(-90.0), (HALL - IN, 9.5, 1.15)))
    solid.add("steel", props.metal_pile(rnd.child("pile")), g.Pose(props.turned(4.0), (3.0, 1.15, 0.0)))
    solid.add("steel_painted", props.filing_cabinet(rnd.child("cabinet")), g.Pose(props.turned(-30.0, 0.0, 90.0), (8.0, 27.2, 0.235)))
    for k, (x, y, yaw, roll) in enumerate(((8.9, 26.0, 12.0, 0.0), (7.0, 26.1, 70.0, 180.0), (9.3, 27.8, -40.0, 0.0))):
        drawer = props.drawer()
        z = 0.0 if roll == 0.0 else 0.32
        solid.add("steel_painted", drawer, g.Pose(props.turned(yaw, 0.0, roll), (x, y, z)))
    solid.add("plastic_black", props.office_chair(), g.Pose(props.turned(140.0, 0.0, 90.0), (10.0, 25.2, 0.31)))

    # ---- 4. The squatters and their fire, in the north-east corner.
    ax, ay = ASH
    bed_z = max(0.0, ash(ax + 0.8, ay + 1.0)) - 0.06
    solid.add("steel", props.bed_frame(rnd.child("bed"), 0.16), g.Pose(props.turned(2.0, 2.0, 0.0), (ax + 0.8, ay + 1.0, bed_z)))
    solid.add("steel", props.trolley(rnd.child("trolley")), g.Pose(props.turned(30.0, 0.0, 88.0), (ax - 2.1, ay - 1.6, 0.28)))
    for k in range(5):
        r = rnd.child("tin", k)
        x, y = ax + r.uniform(-1.5, 1.2), ay + r.uniform(-1.2, 1.0)
        drawn.add("steel", props.can(r, crushed=r.uniform(0.3, 0.9)), g.Pose(props.turned(r.uniform(0, 360), r.uniform(-20, 20), 90.0), (x, y, max(0.0, ash(x, y)) + 0.02)))
    solid.add("chair", props.plastic_chair(rnd.child("chair"), melt=0.75), g.Pose(props.turned(200.0), (ax - 1.6, ay + 1.6, 0.0)))
    # The windbreak: two pallets stood on edge against the east wall, the
    # north one charred.
    for k, (y, key) in enumerate(((20.4, WOOD), (21.7, CHAR))):
        p = props.pallet(rnd.child("windbreak", k), 0.2, 0.3)
        solid.add(key, p, g.Pose(props.turned(90.0, 0.0, -78.0), (HALL - IN - 0.18, y, 0.6)))

    # ---- 5. The pilots' hangout under the skylight inside the loading door.
    fx, fy = FIRE_PIT
    ring = ruin.Batch()
    for k in range(13):
        a = 360.0 * k / 13 + rnd.uniform(-6, 6)
        r = 0.42 + rnd.uniform(-0.03, 0.04)
        ring.add(g.box_mesh(0.215, 0.1, 0.065), g.Pose(props.turned(a + 90.0 + rnd.uniform(-12, 12), rnd.uniform(-6, 6)),
                                                       (fx + r * math.cos(math.radians(a)), fy + r * math.sin(math.radians(a)), 0.035)))
    solid.add("brick", ring.mesh())
    pit_ash = ruin.mound(fx, fy, 0.38, 0.38, 0.06, rnd.child("pit_ash"))
    drawn.add("char", g.heightfield_mesh(fx - 0.38, fy - 0.38, fx + 0.38, fy + 0.38, 0.1, pit_ash))
    for a, length in ((20.0, 0.9), (110.0, 0.7)):
        drawn.add("char", g.box_mesh(length, 0.1, 0.022), g.Pose(props.turned(a, 8.0), (fx + 0.05, fy - 0.05, 0.08)))
    bx, by = 19.8, 6.8  # the BBQ: a grate on two stacks of three bricks
    for sx in (-1, 1):
        for k in range(3):
            solid.add("brick", g.box_mesh(0.215, 0.1, 0.065), g.Pose(props.turned(90.0 + rnd.uniform(-5, 5)), (bx, by + sx * 0.2, 0.0325 + 0.065 * k)))
    drawn.add("steel", props.grill(rnd.child("grill")), g.Pose(props.turned(92.0), (bx, by, 0.195)))
    cx, cy = 23.2, 8.3
    solid.add("wood", props.cable_drum(), g.Pose(at=(cx, cy, 0.0)))
    out["table"] = (cx, cy, 0.62)
    for k, (x, y, yaw) in enumerate(((fx, fy - 1.65, 3.0), (fx + 1.65, fy - 0.3, 87.0))):
        for level in range(2):
            p = props.pallet(rnd.child("bench", k, level))
            solid.add("wood", p, g.Pose(props.turned(yaw + rnd.uniform(-6, 6)), (x, y, 0.144 * level)))
    for k, (x, y, key, cracked) in enumerate(((20.35, 7.75, "blue", False), (20.4, 5.35, "red", False), (19.2, 8.5, "blue", True))):
        r = rnd.child("crate", k)
        solid.add("plastic_" + key, props.crate(r, cracked), g.Pose(props.turned(r.uniform(0, 90)), (x, y, 0.0)))
    # A crate of empties by the table.
    r = rnd.child("empties")
    solid.add("plastic_red", props.crate(r), g.Pose(props.turned(15.0), (24.25, 9.05, 0.0)))
    for k in range(6):
        kx, ky = (k % 3 - 1) * 0.12, (k // 3 - 0.5) * 0.13
        p = g.Pose(props.turned(15.0), (24.25, 9.05, 0.012)).apply((kx, ky, 0.0))
        drawn.add("glass_" + ("brown" if k % 2 else "green"), props.bottle(r), g.Pose(props.turned(r.uniform(0, 360)), p))
    # Bottles and cans in a loose ring round the seats, a few on the table.
    r = rnd.child("litter")
    for k in range(22):
        a = r.uniform(0, 360)
        d = r.uniform(1.4, 3.0)
        x, y = fx + 0.3 + d * math.cos(math.radians(a)), fy + 0.4 + d * math.sin(math.radians(a))
        if math.hypot(x - cx, y - cy) < 0.6 or y < IN + 0.1:
            continue
        lying = r.chance(0.7)
        if r.chance(0.5):
            drawn.add("can", props.can(r, crushed=r.uniform(0.0, 0.9) if r.chance(0.5) else 0.0),
                      g.Pose(props.turned(r.uniform(0, 360), 0.0, 90.0 if lying else 0.0), (x, y, 0.033 if lying else 0.0)))
        else:
            key = "glass_brown" if r.chance(0.5) else "glass_green"
            drawn.add(key, props.bottle(r), g.Pose(props.turned(r.uniform(0, 360), 0.0, 90.0 if lying else 0.0), (x, y, 0.032 if lying else 0.0)))
    for k, (dx, dy) in enumerate(((-0.2, 0.15), (0.25, -0.1), (0.1, 0.3))):
        drawn.add("can" if k != 1 else "glass_brown", props.can(r) if k != 1 else props.bottle(r), g.Pose(at=(cx + dx, cy + dy, 0.62)))
    # The charging spot on the table: a power bank, a roll of tape, cut zip ties.
    drawn.add("plastic_black", g.box_mesh(0.15, 0.07, 0.022), g.Pose(props.turned(20.0), (cx - 0.1, cy - 0.25, 0.631)))
    drawn.add("plastic_white", g.annulus_mesh(0.025, 0.025, 0.05, 12), g.Pose(at=(cx + 0.2, cy - 0.3, 0.62)))
    for k in range(5):
        drawn.add("plastic_black", g.box_mesh(0.1, 0.004, 0.002), g.Pose(props.turned(r.uniform(0, 180)), (cx + r.uniform(-0.3, 0.3), cy + r.uniform(-0.3, 0.0), 0.621)))
    # The bin bag against the pilaster at x = 20 on the south wall, and the
    # litter blown along the wall's foot.
    solid.add("plastic_black", props.bin_bag(rnd.child("bag")), g.Pose(props.turned(30.0, 6.0), (19.45, 0.62, 0.0)))
    r = rnd.child("wall_litter")
    for k in range(9):
        x, y = r.uniform(15.0, 24.0), IN + r.uniform(0.1, 0.5)
        if 19.5 < x < 20.5:
            continue
        if r.chance(0.6):
            drawn.add("can", props.can(r, crushed=r.uniform(0.3, 1.0)), g.Pose(props.turned(r.uniform(0, 360), 0.0, 90.0), (x, y, 0.03)))
        else:
            drawn.add("plastic_red", g.box_mesh(0.18, 0.12, 0.004), g.Pose(props.turned(r.uniform(0, 360), r.uniform(-8, 8)), (x, y, 0.006)))

    # ---- Crashed-quad bits, each where a crash would leave it.
    drawn.add("plastic_black", props.quad_frame(rnd.child("whoop"), 0.075), g.Pose(props.turned(10.0, 0.0, 90.0), (2.5, HALL - 0.02, 4.05)))
    drawn.add("plastic_red", props.prop_blades(rnd.child("prop5")), g.Pose(props.turned(40.0, 14.0), (12.3, 6.2, heap(12.3, 6.2) + 0.06)))
    drawn.add("plastic_black", props.quad_frame(rnd.child("arm"), 0.25, arms_gone=3), g.Pose(props.turned(220.0), (21.5, 13.75, 0.0)))
    drawn.add("plastic_black", props.antenna(), g.Pose(props.turned(70.0), (21.9, 13.4, 0.0)))
    drawn.add("plastic_black", props.quad_frame(rnd.child("lost"), 0.25), g.Pose(props.turned(30.0), (15.0, 19.0, EAVE - 0.35 + 0.08)))

    # ---- The yard: the repaired gate, the launch pad, a skip, a tyre, a
    # stack of pallets grass grows through, an oil drum.
    g0, g1 = GATE
    for x in (g0, g1):
        solid.add("steel", g.box_mesh(0.12, 0.12, 2.5), g.Pose(at=(x, -30.0, 1.15)))
    # The west leaf hangs from its hinges, dropped at its free end and strapped
    # to its post; the east one is propped open against a block.
    leaf_a = props.gate_leaf(rnd.child("leaf_a"), 2.9, 2.0)
    solid.add("steel", leaf_a, g.Pose(props.turned(-14.0, -2.5), (g0 + 0.08, -30.0, 0.06)))
    leaf_b = props.gate_leaf(rnd.child("leaf_b"), 2.9, 2.0)
    solid.add("steel", leaf_b, g.Pose(props.turned(100.0), (g1 - 0.08, -29.95, 0.05)))
    solid.add("concrete", g.box_mesh(0.44, 0.22, 0.2), g.Pose(props.turned(-8.0), (g1 - 0.75, -27.0, yard_h(g1 - 0.75, -27.0) + 0.08)))
    drawn.add("plastic_red", g.annulus_mesh(0.05, 0.075, 0.085, 10), g.Pose(at=(g0, -30.0, 1.85)))
    drawn.add("plastic_red", g.annulus_mesh(0.05, 0.075, 0.085, 10), g.Pose(at=(g0, -30.0, 0.5)))
    # A bright new chain and padlock on the rusty leaf.
    lx, ly = g.Pose(props.turned(-14.0), (g0 + 0.08, -30.0, 0.0)).apply((2.75, 0.0, 0.0))[:2]
    for k in range(7):
        drawn.add("can", g.annulus_mesh(0.01, 0.012, 0.022, 8), g.Pose(props.turned(0.0, 0.0, 90.0 if k % 2 else 0.0), (lx, ly - 0.03, 1.1 - 0.035 * k)))
    drawn.add("can", g.box_mesh(0.05, 0.025, 0.06), g.Pose(at=(lx, ly - 0.03, 0.82)))
    pad = (15.6, -12.0)
    drawn.add("wood", g.box_mesh(1.0, 1.0, 0.018), g.Pose(props.turned(12.0), (pad[0], pad[1], yard_h(*pad) + 0.012)))
    for a in (45.0, -45.0):
        drawn.add("paint", g.box_mesh(0.7, 0.08, 0.002), g.Pose(props.turned(12.0 + a), (pad[0], pad[1], yard_h(*pad) + 0.022)))
    solid.add("steel", props.skip(rnd.child("skip")), g.Pose(props.turned(20.0, 0.0, 84.0), (24.0, -15.5, 0.86)))
    spill = ruin.mound(24.8, -14.0, 1.4, 1.0, 0.35, rnd.child("skip_spill"))
    drawn.add("dirt", g.heightfield_mesh(23.4, -15.0, 26.2, -13.0, 0.25, lambda x, y: spill(x, y) + yard_h(x, y)))
    lumps = ruin.Batch()
    ruin.scatter(lumps, rnd.child("skip_lumps"), (24.9, -14.0), 1.3, 14, lambda q: ruin.lump(q.uniform(0.15, 0.35), q), z=lambda x, y: spill(x, y) + yard_h(x, y))
    solid.add("concrete", lumps.mesh())
    solid.add("black", props.tyre(), g.Pose(props.turned(0.0, 12.0), (6.0, -9.0, yard_h(6.0, -9.0) + 0.02)))
    for k in range(5):
        p = props.pallet(rnd.child("stack", k), 0.1, 0.1)
        solid.add("wood", p, g.Pose(props.turned(25.0 + rnd.uniform(-7, 7)), (-4.2, -3.2, yard_h(-4.2, -3.2) + 0.144 * k)))
    solid.add("steel", props.drum(rnd.child("drum"), 1.0), g.Pose(props.turned(30.0), (31.6, -1.8, yard_h(31.6, -1.8) - 0.02)))
    out["drums"] = [(31.6, -1.8)]

    names = {
        "concrete": ("concrete_wall_rough", "story_concrete"), "steel": (STEEL, "story_steel"), "steel_painted": (STEEL, "story_steel_painted"),
        "brick": (BRICK, "story_brick"), "wood": (WOOD, "story_wood"), CHAR: (CHAR, "story_char"), WOOD: (WOOD, "story_windbreak"),
        "chair": ("plastic_white", "story_chair"), "plastic_black": ("plastic_black", "story_plastic_black"),
        "plastic_blue": ("plastic_blue", "story_plastic_blue"), "plastic_red": ("plastic_red", "story_plastic_red"),
        "black": ("plastic_black", "story_tyre"),
    }
    for key, (material, name) in names.items():
        out[key] = solid.put(m, key, name, material, "trimesh")
    drawn_names = {
        "steel": STEEL, "cable": "plastic_black", "char": CHAR, "glass_brown": "glass_brown", "glass_green": "glass_green",
        "can": "can_painted", "plastic_black": "plastic_black", "plastic_white": "plastic_white", "plastic_red": "plastic_red",
        "wood": WOOD, "paint": "paint_yellow", "dirt": DIRT,
    }
    for key, material in drawn_names.items():
        drawn.put(m, key, "litter_" + key, material, "none", light_cm=3)
    return out


def ivy(m, rnd):
    """Ivy up the shaded north and east walls' outsides: leaf cards flat to
    the brick, densest at the foot."""
    batch = ruin.Batch()
    for k, (face, u0, width, height) in enumerate(((NORTH, 14.0, 3.5, 4.5), (NORTH, 27.0, 2.5, 3.0), (EAST, 3.0, 3.0, 5.5))):
        r = rnd.child("ivy", k)
        out_n = T / 2 + 0.03
        for i in range(380):
            h = height * (r.unit() ** 1.6)
            u = u0 + (r.unit() - 0.5) * width * (1.0 - 0.6 * h / height)
            if face is EAST and 6.5 < u + T / 2 < 8.5:
                continue
            p = face.p(u, h + 0.05, out_n + r.uniform(0.0, 0.06))
            size = r.uniform(0.06, 0.11)
            along = face.along
            n = face.outward
            a = r.uniform(0, 360)
            dirn = g.unit(g.add(g.mul(along, math.cos(math.radians(a))), (0.0, 0.0, math.sin(math.radians(a)))))
            side = g.cross(n, dirn)
            tilt = g.unit(g.add(n, g.mul(dirn, -0.4)))
            rows = ((dirn[0], side[0], tilt[0]), (dirn[1], side[1], tilt[1]), (dirn[2], side[2], tilt[2]))
            batch.add(plants.leaf(size, size * 0.8, 0.003), g.Pose(rows, p))
    m.solid("ivy", batch.mesh(), None, "leaves_mixed", "none", light_cm=4)


# ---------------------------------------------------------------- wear

class Op:
    """An opening as `odwear.Face` reads it."""

    def __init__(self, u0, u1, v0, v1, kind):
        self.u0, self.u1, self.v0, self.v1, self.kind = u0, u1, v0, v1, kind


def wear_face(name, p0, p1, hosts, openings, painted=False):
    """An `odwear.Face` from `p0` to `p1` (front on the left), its openings
    given in the Map's own terms: `(a0, a1, z0, z1, kind)` with `a` the x or y
    the wall runs along."""
    face = WearFace(name, p0, p1, 0.0, EAVE, hosts, painted=painted)
    ops = []
    for a0, a1, z0, z1, kind in openings:
        pts = [(a, p0[1]) if p0[1] == p1[1] else (p0[0], a) for a in (a0, a1)]
        us = sorted(g.dot((pt[0] - p0[0], pt[1] - p0[1], 0.0), (face.along[0], face.along[1], 0.0)) for pt in pts)
        ops.append(Op(us[0], us[1], z0, z1, kind))
    face.openings = ops
    return face


def wear(m, walls_out, roof_out, floor_ob, floor_h, yard_pieces, yard_h, plaster_ob, story_out):
    """Wear where something put it: water under every sill and roof edge,
    moss and salt where it is shaded and damp, rust under the steel, soot from
    the two fires, droppings under the trusses, scuffs where people walk,
    tags where they stand."""
    w = Wear(seed=324, level=0.85, sun=SUN)
    ws, wn, ww, we = (walls_out[k][0] for k in ("south", "north", "west", "east"))
    win = lambda xs: [(x - R, x + R, SILL, SPRING + R, "window") for x in xs]
    s_ops = win(WIN_S) + [(10.45, 14.55, 0.0, 4.4, "doorway")]
    n_ops = win(WIN_N)
    w_ops = win(WIN_W) + [(13.1, 15.9, 0.45, 3.05, "breach")]
    e_ops = [(7.0, 8.1, 0.0, 2.2, "doorway"), (12.1, 12.9, 0.9, 1.5, "window")] + win(WIN_E)
    o, i = HALL + T / 2, T / 2
    faces = {
        "south_out": wear_face("south_out", (HALL + i, -i), (-i, -i), [ws], s_ops),
        "south_in": wear_face("south_in", (i, i), (HALL - i, i), [ws], s_ops),
        "north_out": wear_face("north_out", (-i, HALL + i), (o, HALL + i), [wn], n_ops),
        "north_in": wear_face("north_in", (HALL - i, HALL - i), (i, HALL - i), [wn], n_ops),
        "west_out": wear_face("west_out", (-i, i), (-i, HALL - i), [ww], w_ops),
        "west_in": wear_face("west_in", (i, HALL - i), (i, i), [ww], w_ops),
        "east_out": wear_face("east_out", (o, HALL - i), (o, i), [we], e_ops),
        "east_in": wear_face("east_in", (HALL - i, i), (HALL - i, HALL - i), [we], e_ops),
    }
    yard_floor = Floor(yard_pieces, yard_h)
    inside = Floor([floor_ob], floor_h)
    # Outside: the weather side gets the water; the shaded north and east get
    # moss, salt and lichen; tags where people come in from the gate.
    gutters = {"south_out": [5.5, 21.0], "north_out": [9.0, 24.5], "west_out": [3.0], "east_out": [26.0]}
    for name in ("south_out", "north_out", "west_out", "east_out"):
        f = faces[name]
        w.runs_below_openings(f)
        w.runs_from_edge(f, gutters=gutters[name])
        w.moss_at_foot(f, yard_floor, damp=gutters[name])
        w.salt_at_foot(f, yard_floor, damp=gutters[name])
        w.lichen_on(f)
        w.cracks_at_openings(f)
    w.marks_near_openings(faces["south_out"])
    w.marks_near_openings(faces["west_out"])
    # Inside: runs under the sills on the weather side, scratches by the way
    # in, cracks over the openings, flakes under the plaster.
    for name in ("south_in", "west_in"):
        w.runs_below_openings(faces[name])
    # The roof leaks at the eaves and the gables' tops: runs down every inside
    # face, and scratches low on every face people pass.
    for name in ("south_in", "north_in", "west_in", "east_in"):
        w.runs_from_edge(faces[name], gutters={"east_in": [16.0, 4.5], "west_in": [21.0], "north_in": [12.0]}.get(name, []))
        w.scratches_low(faces[name])
    w.salt_at_foot(faces["east_in"], inside, damp=[4.5, 16.0])
    w.salt_at_foot(faces["north_in"], inside, damp=[12.0])
    plaster_face = WearFace("plaster", (20.4, HALL - i - 0.05), (9.6, HALL - i - 0.05), 0.0, 3.4, [plaster_ob, wn], painted=True)
    w.flakes_at_foot(plaster_face, inside)
    h = 0.7071
    w.cobwebs_in_corners([
        (i, HALL - i, EAVE, (h, -h), [ww, wn]), (HALL - i, i, EAVE, (-h, h), [we, ws]), (i, i, EAVE, (h, h), [ww, ws]),
    ])
    w.moss_in_corners([(i + 0.0, HALL - i, (h, -h), [ww, wn]), (HALL - i, i, (-h, h), [we, ws])], inside)
    # Water under the roof's holes, where the floor dips.
    w.damp_at([(d.x, d.y, d.radius) for d in FLOOR_DIPS], inside)
    w.cracks_on_floor((IN, IN, HALL - IN, HALL - IN), inside, 5)
    # Steel: rust under the crane rail's brackets and the trusses' feet.
    pilasters = [walls_out["pilasters"]]
    for x, y, z, sgn in story_out["brackets"]:
        w.rust_below([(x, y, z)], (0.0, sgn, 0.0), pilasters)
        w.rust_below([(x, y, EAVE - 0.5)], (0.0, sgn, 0.0), pilasters)
    # Birds on the trusses over the aisle and the fire corner.
    for x, ya, yb in ((10.0, 12.0, 19.0), (15.0, 14.0, 22.0), (25.0, 3.0, 9.0)):
        w.droppings_under((x, ya), (x, yb), inside)
    # Oil where the machines stood and round the drum.
    w.oil_at([(x + 1.0, y - 1.1) for x, y in story_out["plinths"]] + [(x - 1.3, y + 0.2) for x, y in story_out["plinths"][:2]], inside)
    w.oil_at(story_out["drums"], yard_floor)
    # People: tyre marks up the aisle, a path from the door to the hangout.
    w.scuff_paths([(12.5, 0.6), (12.5, 2.6), (19.5, 5.0)], inside)
    w.scuff_paths([(12.2, 11.0), (12.8, 25.5)], inside, width=1.4)

    # Water stains under the sills: years of rain off each sill, more on the
    # weather side, left where a sill still throws it clear.
    r = Random(SEED, "sill_stains")
    for name, xs, f, wall, along_x in (("south_out", WIN_S, (0.0, -1.0, 0.0), ws, True), ("south_in", WIN_S, (0.0, 1.0, 0.0), ws, True),
                                       ("north_out", WIN_N, (0.0, 1.0, 0.0), wn, True), ("west_out", WIN_W, (-1.0, 0.0, 0.0), ww, False),
                                       ("west_in", WIN_W, (1.0, 0.0, 0.0), ww, False), ("east_out", WIN_E, (1.0, 0.0, 0.0), we, False)):
        for x in xs:
            if not r.chance(0.7):
                continue
            a = x + r.uniform(-0.6, 0.6)
            h = r.uniform(1.0, 1.25)
            if along_x:
                c = (a, (0.0 if name.startswith("south") else HALL) + f[1] * T / 2, SILL - 0.12 - h / 2)
            else:
                c = ((0.0 if name.startswith("west") else HALL) + f[0] * T / 2, a, SILL - 0.12 - h / 2)
            w.hand(f"sill_stain_{name}_{x}", "decal_stain", [wall], c, f, (0.0, 0.0, 1.0), (r.uniform(1.0, 1.6), h), layer=0, depth=0.6)

    # By hand: the story's own marks.
    north, south_f, east_f, west_f = (0.0, -1.0, 0.0), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0), (1.0, 0.0, 0.0)
    up = (0.0, 0.0, 1.0)
    ax, ay = ASH
    iron_n, iron_s = roof_out["iron"]["north"], roof_out["iron"]["south"]
    # The fire corner: old tags first (layer 0), soot over them (1), the roof
    # blackened over the ash.
    w.hand("old_tag_east_a", "decal_tag", [we], (HALL - i, 19.0, 1.3), east_f, up, (1.6, 1.3), layer=0, depth=0.6)
    w.hand("old_tag_east_b", "decal_tag", [we], (HALL - i, 21.4, 1.6), east_f, up, (1.3, 1.1), layer=0, depth=0.6)
    for k, (dx, s, z) in enumerate(((0.2, (5.0, 7.0), 3.3), (-0.5, (3.2, 4.4), 2.1), (0.9, (2.6, 3.4), 1.6), (0.0, (4.2, 6.0), 2.9))):
        w.hand(f"soot_north_fire_{k}", "decal_soot", [wn], (ax + dx, HALL - i, z), north, up, s, layer=1, depth=0.8)
        w.hand(f"soot_east_fire_{k}", "decal_soot", [we], (HALL - i, ay + 0.3 - dx, z), east_f, up, s, layer=1, depth=0.8)
    w.hand("soot_east_low", "decal_soot", [we], (HALL - i, ay - 2.0, 1.5), east_f, up, (2.6, 3.2), layer=1, depth=0.8)
    w.hand("soot_plaster", "decal_soot", [plaster_ob, wn], (19.9, HALL - i, 3.4), north, up, (1.6, 1.8), layer=1, depth=0.8)
    w.hand("scorch_floor", "decal_soot", [floor_ob], (ax - 0.4, ay - 0.2, floor_h(ax, ay)), up, (-0.7071, -0.7071, 0.0), (6.4, 6.4), layer=1, depth=0.6)
    for k, (x, y, s) in enumerate(((ax, ay, 5.5), (ax + 1.8, ay + 2.6, 4.0), (ax - 2.2, ay + 0.8, 3.6))):
        w.hand(f"soot_roof_{k}", "decal_soot", [iron_n], (x, y, roof_under(y) - 0.1), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), (s, s), layer=1, depth=1.2)
    # The hangout: a light soot fan round the skylight over the fire pit, a
    # bright new tag over a faded old one, the drag marks to the breach.
    fx, fy = FIRE_PIT
    w.hand("soot_roof_hangout", "decal_soot", [iron_s], (fx, fy, roof_under(fy) - 0.1), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), (3.2, 3.2), layer=1, depth=1.4)
    w.hand("old_tag_south", "decal_tag", [ws], (18.6, i, 1.7), south_f, up, (1.2, 1.0), layer=0, depth=0.6)
    w.hand("new_tag_south", "decal_tag", [ws], (17.4, i, 1.3), south_f, up, (1.8, 1.5), layer=2, depth=0.6)
    w.hand("new_tag_door", "decal_tag", [ws], (9.4, -i, 1.4), (0.0, -1.0, 0.0), up, (1.5, 1.3), layer=2, depth=0.6)
    w.hand("new_tag_gate_side", "decal_tag", [ws], (4.2, -i, 1.5), (0.0, -1.0, 0.0), up, (1.9, 1.5), layer=2, depth=0.6)
    w.hand("old_tag_west", "decal_tag", [ww], (-i, 5.0, 1.4), (-1.0, 0.0, 0.0), up, (1.6, 1.3), layer=0, depth=0.6)
    d = g.unit((0.2 - 6.0, 14.5 - 20.0, 0.0))
    for k in range(4):
        t = (k + 0.5) / 4
        x, y = 6.0 + (0.6 - 6.0) * t, 19.2 + (14.5 - 19.2) * t
        w.hand(f"drag_{k}", "decal_scuffs", [floor_ob], (x, y, floor_h(x, y)), up, (d[0], d[1], 0.0), (0.6, 1.9), layer=4, depth=0.5)
    # Bay marks stencilled on the pilasters, 2.5 m up.
    for x, y, f in ((5.0, IN + 0.28, south_f), (15.0, IN + 0.28, south_f), (20.0, HALL - IN - 0.28, north), (10.0, HALL - IN - 0.28, north)):
        w.hand(f"bay_mark_{int(x)}_{int(y)}", "decal_stencil", [walls_out["pilasters"]], (x, y, 2.5), f, up, (0.62, 0.62), layer=2, depth=0.2)
    # Scuffs at forklift height on the loading door's jambs and the pilasters
    # beside the aisle.
    for x, y, f in ((10.0, IN + 0.28, south_f), (15.0, IN + 0.28, south_f)):
        w.hand(f"scuff_pilaster_{int(x)}", "decal_scratches", [walls_out["pilasters"]], (x, y, 0.8), f, up, (0.7, 0.9), layer=4, depth=0.2)
    # #341: every decal is a draw, and the frame had to fit 8 ms with the
    # grass and the kit; the marks that read least at FPV distance (runs,
    # cracks, scratches and droppings, by the slice v2 report) are thinned.
    thin = {"runs": 3, "cracks": 3, "scratches": 2, "droppings": 2}
    seen = {}
    kept = []
    for spec in w.specs:
        n = seen.get(spec.kind, 0)
        seen[spec.kind] = n + 1
        if spec.hand or n % thin.get(spec.kind, 1) == 0:
            kept.append(spec)
    w.specs = kept
    w.apply(m)
    return w


FLOOR_DIPS = []
KIT_STATS = []


def build(out):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    m.lightmap(texel_cm=10)
    m.batch_decals()  # #341: hundreds of decals are a few dozen draws
    rnd = Random(SEED, "factory")
    floor, dips = floor_ground()
    FLOOR_DIPS[:] = dips
    slabs = slab_layout(rnd.child("slabs"))
    yard_ground, yard_pieces = yard(m, rnd.child("yard"))

    def floor_h(x, y):
        for x0, y0, x1, y1, off, tx, ty in slabs:
            if x0 <= x <= x1 and y0 <= y <= y1:
                return floor.height(x, y) + off + tx * (x - (x0 + x1) / 2) + ty * (y - (y0 + y1) / 2)
        return floor.height(x, y)

    # The floor's relief (#340): a thick layer of crumbled plaster and
    # debris, deepest along the walls it fell from, worn round the plinths,
    # a swell under the fallen bay's heap; the flying lines kept low.
    relief = Relief(floor_h, seed=SEED + 83, clear=flying_lines(SEED + 84).clearance, bumps=0.01)
    relief.crumble((IN, IN, HALL - IN, HALL - IN), depth=0.13, grain=0.35, cover=0.7,
                   walls=[((0.0, HALL - IN), (HALL, HALL - IN)), ((IN, 0.0), (IN, HALL)), ((HALL - IN, 0.0), (HALL - IN, HALL)),
                          ((0.0, IN), (HALL, IN))])
    for x, y, sx, sy in ((6.0, 20.0, 3.0, 1.6), (8.9, 23.2, 2.4, 1.4), (18.0, 22.5, 2.0, 1.2)):
        relief.worn_rect((x - sx / 2, y - sy / 2, x + sx / 2, y + sy / 2), band=1.2)
    relief.under_heap(12.5, 6.5, 3.8, 0.15)
    floor_h = relief.height

    walls_out = walls(m, rnd.child("walls"), floor_h, yard_ground.height)
    plaster_ob = plaster(m, rnd.child("plaster"))
    graffiti(m)
    roof_out = roof(m, rnd.child("roof"))
    silhouette(m)
    kit = hall_kit(floor_h)
    floor_ob, heap, ash = floor_and_clutter(m, rnd.child("floor"), floor, slabs, kit, relief)
    aisle_lines(m, floor_h)
    story_out = story(m, rnd.child("story"), heap, ash, yard_ground.height)
    hall_mess(m, kit, floor_ob)
    kit_on_factory(kit)
    kit.apply(KitOnFactory(m, {"kit_heap": ("grass", 2.0, (0.12, 0.45))}))
    yard_stats, _ = yard_kit(m, yard_ground.height, yard_pieces)
    KIT_STATS[:] = [kit.stats(), yard_stats]
    ivy(m, rnd.child("ivy"))
    backdrop(m, rnd.child("backdrop"))
    worn = wear(m, walls_out, roof_out, floor_ob, floor_h, yard_pieces, yard_ground.height, plaster_ob, story_out)
    # Every surface here is a picture made from scratch: no flat colours,
    # nothing downloaded (ADR-0024).
    for name in m.materials:
        texture = m.library[name].get("texture")
        assert texture and "generator" in m.textures[texture], f"{name} has no picture made from scratch"
    m.launch_spot(*LAUNCH[:3], LAUNCH[3])
    m.export(out)
    return m, worn


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = argv[argv.index("--out") + 1]
    builder, worn = build(out)
    print("factory:", builder.stats())
    print("  decals by kind:", ", ".join(f"{k} {v}" for k, v in sorted(worn.counts().items())))
    for where, st in zip(("hall", "yard"), KIT_STATS):
        print(f"  kit in the {where}:", st)
    if builder.light_result:
        print("  light atlas:", builder.light_result.get("width"), "x", builder.light_result.get("height"))
    if "--previews" in argv:
        import odpreview

        odpreview.render(builder, PREVIEW, LAUNCH, argv[argv.index("--previews") + 1])
