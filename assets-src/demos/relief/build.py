"""A relief test Map (#340): not a Map of the Pack.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/demos/relief/build.py -- --out <file.glb> [--flat] [--light-cm 20]

The kit's look-test hall (`assets-src/demos/kit/build.py`, #339), 20 x 14 m,
given real relief (`assets-src/lib/odrelief.py`, `odbrick.py`,
`odframe.py`):

- the yard has wheel ruts along a track, potholes, a slope of spoil under
  the rubble heap, and fine cells (0.3 m) round the aisle, the doorway and
  the heap and coarse ones (1.2 m) elsewhere;
- the hall's floor is under a thick layer of crumbled plaster and debris,
  thickest along the north and west walls, worn smooth round the columns
  and a machine's plinth, on 0.2 m cells;
- the south wall, which the aisle flies through, is brick laid brick by brick
  for 9.45 m round its doorway and window: sunken mortar, bricks missing in
  patches and one by one, faces gone, islands of render, the doorway's and
  the window's broken edges, a window cill and two pilasters; the rest of it
  is the picture on a plain face;
- a gable roof of buckled sheets over a steel frame: most of the south
  slope's sheets and a few of the north's are gone, and the purlins under
  them are bent, hanging by one end or gone, so the roof's line against the
  sky is broken.

The scatter (#339) puts the kit down as in the look-test hall, on the relief.
A 4 m aisle from the Launch Spot through the doorway is a keep-clear zone for
the scatter and for the relief alike. `--flat` builds the same hall with the
look-test hall's flat ground and floor, box walls and no roof: the "before".

Nothing builds this in CI and nothing commits its `.glb`: to fly or render it,
build it into `packs/opendrone/maps/relief-demo/` on a checkout of your own,
give that folder a `map.toml` like any Map's (copy Bando's, with `model =
"relief-demo.glb"`) and add `("relief-demo", "opendrone/relief-demo")` to
`MAPS` in `crates/opendrone/src/main.rs`. East is +x, north is +y, up is +z.
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odcut  # noqa: E402
import odgeom as g  # noqa: E402
from odbrick import LEAF, brick_face, cill  # noqa: E402
from odframe import BuckledRoof, Frame  # noqa: E402
from odground import Flat, FlatRect, Ground  # noqa: E402
from odmap import MapBuilder  # noqa: E402
from odopening import BRICK, Opening, broken_wall  # noqa: E402
from odrelief import Relief, graded_mesh  # noqa: E402
from odscatter import Scatter  # noqa: E402

YARD = (-22.0, -22.0, 26.0, 16.0)
HALL = (-10.0, -6.0, 10.0, 8.0)
WALL_H, WALL_T = 5.0, 0.34
LAUNCH = (0.0, -16.0)
FLOOR_Z = 0.05
AISLE = (-2.0, YARD[1], 2.0, HALL[3] - 1.0)
COLUMNS = (-8.0, -4.0, 4.0, 8.0)
PLINTH = (-6.6, -3.4, -4.4, -2.2)  # a machine's base, 0.25 m tall
HEAP = (15.0, 1.0)


def tris(mesh):
    return odcut.triangle_count(mesh)


def build(out, seed=7, flat=False, light_cm=None):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    if light_cm is not None:
        m.lightmap(light_cm)
    cost = {}
    x0, y0, x1, y1 = HALL

    # The flying lines: the Launch Spot and the aisle through the doorway.
    # Their own Scatter holds only them, for the relief (#339's keep-clear
    # zones); the scatter of the kit below adds the walls and columns too.
    fly = Scatter(seed)
    fly.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.5)
    fly.keep_clear_rect(*AISLE)

    # ---------- the yard ----------
    plain_yard = Ground(5, amplitude=0.12, wavelength=9.0, slope=(0.004, 0.003),
                        flats=[Flat(LAUNCH[0], LAUNCH[1], 3.5), FlatRect(x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3, blend=0.8, level=0.0)])
    yard = Relief(plain_yard.height, seed=5, clear=fly.clearance, bumps=0.02)
    if not flat:
        yard.rut((-21.0, -10.5), (25.0, -9.0), gauge=1.8, depth=0.08)
        yard.rut((12.0, -21.0), (14.5, 14.0), gauge=1.8, depth=0.06)
        for x, y, r, d in ((-8.0, -12.0, 0.7, 0.10), (6.5, -13.5, 0.9, 0.08), (11.5, -6.0, 0.6, 0.12),
                           (-14.0, 4.0, 1.1, 0.07), (19.0, -15.0, 0.8, 0.09), (1.2, -9.0, 0.5, 0.06)):
            yard.pothole(x, y, r, d)
        yard.under_heap(HEAP[0], HEAP[1], 4.2, 0.35)
        yard.crumble((x1 + 0.4, -14.0, 22.0, 12.0), depth=0.06, grain=0.6, cover=0.45)
    yard_fine = [AISLE, (-4.0, y0 - 3.0, 4.0, y0 + 0.5), (HEAP[0] - 5.0, HEAP[1] - 5.0, HEAP[0] + 5.0, HEAP[1] + 5.0),
                 (LAUNCH[0] - 5.0, LAUNCH[1] - 5.0, LAUNCH[0] + 5.0, LAUNCH[1] + 5.0)]
    if flat:
        yard_mesh = plain_yard.mesh(YARD, 0.5)
    else:
        yard_mesh = graded_mesh(YARD, yard.height, fine=yard_fine, fine_step=0.3, coarse_step=1.2)
    yard_shape = m.solid("yard", yard_mesh, None, "ground_dirt_rubble", "trimesh")
    cost["yard ground"] = tris(yard_mesh)
    m.launch_spot(LAUNCH[0], LAUNCH[1], yard.height(*LAUNCH), 0.0)

    # ---------- the floor ----------
    plain_floor = Ground(8, amplitude=0.012, wavelength=5.0, base=FLOOR_Z)
    floor = Relief(plain_floor.height, seed=8, clear=fly.clearance, bumps=0.01)
    inside_n, inside_w, inside_s = y1 - WALL_T / 2, x0 + WALL_T / 2, y0 + WALL_T / 2
    if not flat:
        floor.crumble((inside_w, inside_s, x1, inside_n), depth=0.14, grain=0.35, cover=0.7,
                      walls=[((x0, inside_n), (x1, inside_n)), ((inside_w, y0), (inside_w, y1))])
        for x in COLUMNS:
            floor.worn(x, 3.0, 0.25, band=0.9)
        floor.worn_rect(PLINTH, band=1.2)
        floor.under_heap(-5.5, inside_n - 0.4, 2.8, 0.18)
    if flat:
        floor_mesh = plain_floor.mesh(HALL, 0.5)
    else:
        floor_mesh = graded_mesh(HALL, floor.height, fine=[HALL], fine_step=0.2, coarse_step=0.8)
    # The crumbled layer shows the rubble picture through the concrete's, as
    # thick as it lies (the vertex-colour blend, #321).
    blend = None if flat else ("ground_dirt_rubble", floor.blend_weights(floor_mesh[0]))
    floor_shape = m.solid("floor", floor_mesh, None, "concrete_floor_worn", "trimesh", blend=blend)
    cost["hall floor"] = tris(floor_mesh)
    m.box_h("plinth", *((PLINTH[0] + PLINTH[2]) / 2.0, (PLINTH[1] + PLINTH[3]) / 2.0), PLINTH[2] - PLINTH[0],
            PLINTH[3] - PLINTH[1], FLOOR_Z - 0.1, FLOOR_Z + 0.25, "concrete_wall_rough")

    # ---------- the south wall ----------
    door = (8.0, 12.0, 0.0, 4.2)  # 4 m wide, from x = 2 to -2
    window = (6.0, 7.5, 1.5, 3.3)  # from x = 4.0 to 2.5
    if flat:
        m.wall("south_wall", (x1, y0), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered", openings=[door])
    else:
        # A core, LEAF thinner than the whole wall, its front face where the
        # outer leaf stands; the leaf is laid brick by brick over 9.45 m round
        # the doorway and the window, and is a plain face elsewhere.
        face = y0 - WALL_T / 2.0 + LEAF  # the core's front face, facing south
        core_line = y0 + LEAF / 2.0
        ops = [Opening(*door, "doorway"), Opening(*window, "window", sill_gone=False)]
        core = broken_wall(x1 - x0, WALL_H, WALL_T - LEAF, seed=11, openings=ops, bulge=0.0, lean=0.0)
        core_pose = core.pose((x1, core_line), (x0, core_line))
        m.solid("south_wall", core.mesh, core_pose, "brick_red_weathered", "none")
        m.wall("south_wall_solid", (x1, y0), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered",
               openings=core.collision_openings(), collision_only=True)
        cost["south wall core"] = tris(core.mesh)
        # The picture's bricks start at x = 0 (a multiple of 0.225 m), so
        # the leaf starts on one: u = 10 - x.
        u_start = 10.0 - 21 * BRICK  # x = 4.725
        u_end = 10.0 + 21 * BRICK  # x = -4.725
        run = u_end - u_start
        holes = []
        for op in ops:
            for k, (l, r) in enumerate(op.edges):
                z = (op.first_course + k) * 0.075
                holes.append((l - u_start, r - u_start, z, z + 0.075))
        leaf = brick_face(run, WALL_H, seed=12, holes=holes,
                          patches=[(1.0, 0.5, 0.45), (8.9, 3.0, 0.55), (4.4, 4.5, 0.35), (2.4, 2.6, 0.3)],
                          render=[(8.3, 1.3, 1.6, 1.1), (1.4, 3.9, 1.3, 0.9)], loose=0.03, faced=0.05,
                          stagger_odd=False)  # this wall runs west: its even courses are the picture's shifted ones
        leaf_pose = g.aiming((x1 - u_start, face, 0.0), (x0, face, 0.0))
        m.solid("south_leaf", leaf.brick, leaf_pose, "brick_red_weathered", "none")
        m.solid("south_render", leaf.render, leaf_pose, "concrete_smooth", "none")
        cost["south wall leaf, bricks"] = tris(leaf.brick)
        cost["south wall leaf, render"] = tris(leaf.render)
        cost["  leaf stats"] = leaf.stats()
        # The rest of the outer leaf: a plain face with the picture.
        for name, a, b in (("east", 0.0, u_start), ("west", u_end, x1 - x0)):
            m.wall(f"south_leaf_{name}", (x1 - a, face - LEAF / 2.0), (x1 - b, face - LEAF / 2.0), 0.0, WALL_H, LEAF,
                   "brick_red_weathered", collider="none")
        # Pilasters where the brick-by-brick leaf meets the plain one, a cill
        # under the window and a string course along the top.
        out_face = y0 - WALL_T / 2.0
        for u in (u_start, u_end):
            m.box_h("pilaster", x1 - u, out_face - 0.06, 0.45, 0.12, 0.0, WALL_H, "brick_red_weathered", collider="none")
        cill_mesh = cill(window[1] - window[0] + 0.3, depth=0.14)
        cill_pose = g.aiming((x1 - (window[0] + window[1]) / 2.0, out_face, window[2] - 0.06),
                             (x1 - (window[0] + window[1]) / 2.0 - 1.0, out_face, window[2] - 0.06))
        m.solid("cill", cill_mesh, cill_pose, "concrete_wall_rough", "none")
        m.box_h("string_course", 0.0, out_face - 0.04, x1 - x0, 0.08, WALL_H - 0.5, WALL_H - 0.36,
                "concrete_wall_rough", collider="none")
    m.wall("north_wall", (x0, y1), (x1, y1), 0.0, WALL_H, WALL_T, "concrete_wall_rough")
    m.wall("west_wall", (x0, y1), (x0, y0), 0.0, WALL_H, WALL_T, "brick_red_weathered")
    for x in COLUMNS:
        m.box_h("column", x, 3.0, 0.3, 0.3, FLOOR_Z, WALL_H, "steel_beam_rusted")
    m.beam("tie", (x0 + WALL_T / 2, 3.0, WALL_H - 0.2), (x1, 3.0, WALL_H - 0.2), 0.25, 0.35, "steel_beam_rusted")

    # ---------- the roof and its frame ----------
    if not flat:
        south, north = BuckledRoof.gable(x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3, eave_z=WALL_H + 0.25, ridge_z=WALL_H + 1.9,
                                         seed=4, buckle=0.07, dip=0.03, sag=0.12, step=0.3,
                                         south=dict(missing=list(range(5, 16)), bays={3: 2.5}),
                                         north=dict(missing=[2, 3, 12, 13, 14], bays={5: 3.0}))
        roof_tris = 0
        for tag, roof in (("south", south), ("north", north)):
            iron = roof.iron()
            m.solid(f"roof_{tag}", iron, None, "roof_corrugated_rusted", "none")
            m.solid(f"roof_{tag}_solid", roof.collision_mesh(), None, "roof_corrugated_rusted", "trimesh", collision_only=True)
            roof_tris += tris(iron)
        cost["roof sheets"] = roof_tris
        frame = Frame(south, north, seed=4, clear=fly.clearance)
        frame_mesh = frame.mesh()
        m.solid("frame", frame_mesh, None, "steel_beam_rusted", "none")
        for p0, p1, w, h in frame.colliders():
            m.beam("frame_solid", p0, p1, w, h, "steel_beam_rusted", collision_only=True)
        cost["frame"] = tris(frame_mesh)
        cost["  frame stats"] = frame.stats()["members"]

    # ---------- the kit (#339), on the relief ----------
    def ground(x, y):
        inside = x0 + 0.01 < x < x1 - 0.01 and y0 + 0.01 < y < y1 - 0.01
        return floor.height(x, y) if inside else yard.height(x, y)

    s = Scatter(seed, ground)
    s.keep_clear_circle(LAUNCH[0], LAUNCH[1], 3.5)
    s.keep_clear_rect(*AISLE)
    s.keep_clear_rect(x0 - 1.0, y0 - 0.4, x1 + 1.0, y0 + 0.2)  # the south wall and its pilasters
    for x in COLUMNS:
        s.keep_clear_rect(x - 0.2, 2.8, x + 0.2, 3.2)
    s.keep_clear_rect(*PLINTH)
    s.keep_clear_rect(x0 - 1.0, inside_n, x1 + 1.0, y1 + 1.0)
    s.keep_clear_rect(x0 - 1.0, y0 - 1.0, inside_w, y1 + 1.0)
    s.heap(HEAP, 2.8, 1.4)
    s.heap((-5.5, inside_n - 0.4), 2.2, 1.1, toward=(0.0, 1.0))
    s.wall_foot((x1, inside_n), (x0, inside_n), depth=1.1, height=0.14, core="rubble")
    s.wall_foot((inside_w, y1), (inside_w, y0), depth=0.9, height=0.12, core="rubble")
    s.wall_foot((x0, inside_s), (x1, inside_s), depth=0.7, height=0.08, per_metre=25.0, core="rubble")
    s.strew((x0, y0, x1, y1), 2.0, bed="rubble")
    s.strew((x1 + 0.5, -14.0, 22.0, 12.0), 0.25)
    s.strew((-18.0, -14.0, x0 - 0.5, 12.0), 0.15)
    s.anchor_row((-8.0, -1.5), (8.0, -1.5), 4.0, missing=0.0)
    s.anchor_row((-6.0, 6.0), (6.0, 6.0), 3.0)
    s.pallets(-8.4, -4.4, 0.0)
    s.pallets(8.4, 6.6, 0.0, stacks=(2, 3))
    s.boards(-7.6, 1.2, 75.0)
    s.boards(6.5, -4.4, 10.0)
    s.place("plank", 5.6, 0.6, 100.0)
    s.place("pipe_run", 4.5, 4.6, 8.0)
    s.place("drum", 9.0, -2.6, 0.0, k=0)
    s.place("drum_lying", 7.9, -2.2, 70.0)
    s.place("tyre", 12.6, -1.8, 0.0)
    s.patches((x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5), 9, on=[floor_shape])
    s.patches((x1 + 1.0, -12.0, 20.0, 10.0), 5, on=[yard_shape], kinds=("damp", "moss", "moss"))
    s.apply(m)
    cost["kit"] = s.stats()

    m.export(out)
    return m, cost


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    light_cm = float(argv[argv.index("--light-cm") + 1]) if "--light-cm" in argv else None
    builder, cost = build(argv[argv.index("--out") + 1], flat="--flat" in argv, light_cm=light_cm)
    print("relief-demo:", builder.stats())
    for name, value in cost.items():
        print(f"  {name}: {value}")
