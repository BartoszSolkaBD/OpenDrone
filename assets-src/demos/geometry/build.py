"""A test Map for the geometry helpers (#322): not a Map of the Pack.

  blender --background --factory-startup --python-exit-code 1 \
    --python assets-src/demos/geometry/build.py -- --out <file.glb>

It puts each helper of `assets-src/lib/` in one small scene, to look at with
`--render-views` (docs/building-a-map.md, "Geometry helpers"): uneven yard
ground with a level Launch Spot, dips and rubble heaped against a wall; a brick
wall with a window that has lost its sill, a doorway and a breach, and the
bricks that fell out; a gable roof of rusted iron that sags between its
trusses, with sheets lifted and curled, one missing, a tear and three
skylights; and a grass-to-dirt boundary that wanders.

Nothing builds this in CI and nothing commits its `.glb`: it is a way to see
the helpers, and the numbers it prints are the cost of each. To fly or render
it, build it into a folder of your own, give that folder a `map.toml` like any
Map's (copy Bando's) and add the Map's name to `MAPS` in
`crates/opendrone/src/main.rs` on your own checkout. East is +x, north is +y,
up is +z; the origin is the middle of the site, at ground level.
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import odcut  # noqa: E402
import odgeom as g  # noqa: E402
from odedge import WanderingEdge  # noqa: E402
from odground import Dip, Flat, FlatRect, Ground, Mound  # noqa: E402
from odmap import MapBuilder  # noqa: E402
from odnoise import Random  # noqa: E402
from odopening import Opening, broken_wall  # noqa: E402
from odroof import Roof, Skylight, Tear  # noqa: E402

SEED = 3
YARD = (-16.0, -14.0, 16.0, 10.0)
HALL = (-7.0, -4.0, 7.0, 4.0)  # the hall's walls stand on these lines
WALL_H, WALL_T = 4.5, 0.34
LAUNCH = (0.0, -12.0, 0.0)  # x, y; the ground's own height there is the third number


def build(out, yard_amplitude=0.15):
    m = MapBuilder(os.path.join(here, "..", "..", "materials.toml"))
    cost = {}

    # ---------- the yard ----------
    # Ground is level under the hall (below its floor, out of sight), level for
    # 3.5 m round the Launch Spot, with six dips and rubble heaped on the
    # south wall's outside, more under the breach where bricks fell out.
    x0, y0, x1, y1 = HALL
    wall_p0, wall_p1 = (x1, y0), (x0, y0)  # walking west along the south wall: its front is the south face
    dips = [Dip(-9.0, -8.0, 1.8, 0.09), Dip(8.5, -9.5, 2.2, 0.07), Dip(-12.0, 3.0, 1.4, 0.06), Dip(11.0, 5.0, 1.6, 0.08)]
    yard = Ground(
        SEED, amplitude=yard_amplitude, wavelength=10.0, slope=(0.008, 0.004),
        flats=[Flat(LAUNCH[0], LAUNCH[1], 3.5), FlatRect(x0 - 0.3, y0 - 0.3, x1 + 0.3, y1 + 0.3, blend=0.6, level=-0.12)],
        mounds=[Mound(wall_p0, wall_p1, 0.40, 1.8, side=1, spills=[(10.4, 1.4, 0.45)])],
        dips=dips,
    )
    launch_z = yard.height(LAUNCH[0], LAUNCH[1])

    ground_mesh = yard.mesh(YARD, 0.25)
    cost["yard ground, 0.25 m grid"] = (odcut.triangle_count(ground_mesh), (YARD[2] - YARD[0]) * (YARD[3] - YARD[1]))

    # Beyond the yard the same ground runs on at 2 m a corner, in grass, so the
    # pictures have no cliff edge to look at. Its four pieces share one grid,
    # so no crack shows between them, and reach 2 m in under the yard, 2 cm
    # lower, so none shows where they meet it.
    (X0, Y0, X1, Y1) = YARD
    low = lambda x, y: yard.height(x, y) - 0.02
    for tag, rect in (("west", (-60.0, -60.0, X0 + 2.0, 60.0)), ("east", (X1 - 2.0, -60.0, 60.0, 60.0)),
                      ("south", (X0 + 2.0, -60.0, X1 - 2.0, Y0 + 2.0)), ("north", (X0 + 2.0, Y1 - 2.0, X1 - 2.0, 60.0))):
        m.solid(f"far_ground_{tag}", g.heightfield_mesh(*rect, 2.0, low), None, "grass_weeds", "trimesh")

    # ---------- the grass-to-dirt boundary ----------
    # Dirt on the north (left of a line running east), grass to the south. The
    # weights are for the vertex-colour blend of #321; until it merges the
    # picture draws each triangle in the material its corners weigh most for,
    # which shows the shape of the edge at the triangle's size.
    edge = WanderingEdge([(-16.0, -7.0), (16.0, -7.5)], seed=5, wander=1.8, band=0.6, wavelength=6.0)
    weights = edge.weights(ground_mesh[0])
    grass, dirt = [], []
    for face in ground_mesh[1]:
        (grass if sum(weights[i] for i in face) / 3.0 < 0.5 else dirt).append(face)
    for name, faces, material in (("yard_grass", grass, "grass_weeds"), ("yard_dirt", dirt, "ground_dirt_rubble")):
        used = sorted({i for f in faces for i in f})
        remap = {old: new for new, old in enumerate(used)}
        mesh = ([ground_mesh[0][i] for i in used], [tuple(remap[i] for i in f) for f in faces])
        m.solid(name, mesh, None, material, "trimesh")

    # ---------- the floor ----------
    floor = Ground(
        8, amplitude=0.012, wavelength=5.0, base=0.0,
        dips=[Dip(2.0, 0.5, 1.4, 0.025)],
        mounds=[Mound(wall_p0, wall_p1, 0.30, 1.4, side=-1, spills=[(10.4, 1.1, 0.40)], seed=19)],
    )
    floor_rect = (x0, y0, x1, y1)
    floor_mesh = floor.mesh(floor_rect, 0.35)
    m.solid("hall_floor", floor_mesh, None, "concrete_floor_worn", "trimesh")
    cost["hall floor, 0.35 m grid"] = (odcut.triangle_count(floor_mesh), (x1 - x0) * (y1 - y0))

    # ---------- the south wall ----------
    ops = [
        Opening(2.2, 3.8, 1.2, 2.8, "window", sill_gone=True),
        Opening(5.4, 7.2, 0.0, 2.4, "doorway"),
        Opening(9.6, 11.2, 0.4, 2.0, "breach"),
    ]
    wall = broken_wall(14.0, WALL_H, WALL_T, seed=2, openings=ops, bulge=0.03, lean=0.05, ledge=0.004)
    pose = wall.pose(wall_p0, wall_p1, 0.0)
    m.solid("south_wall", wall.mesh, pose, "brick_red_weathered", "none")
    m.wall("south_wall_solid", wall_p0, wall_p1, 0.0, WALL_H, WALL_T, "brick_red_weathered",
           openings=wall.collision_openings(), collision_only=True)
    # The wall's own y runs to the south, so a brick at y > 0 is outside, on the yard.
    ground_at = lambda u, y: (yard if y > 0 else floor).height(wall_p0[0] - u, wall_p0[1] - y)
    bricks = wall.loose_bricks(Random(2, "bricks"), floor=ground_at)
    for mesh in bricks:
        m.solid("loose_bricks", mesh, pose, "brick_red_weathered", "none")
    cost["south wall, 14 x 4.5 m, three openings, 4 mm ledges"] = (odcut.triangle_count(wall.mesh), 14.0 * WALL_H)
    flat_wall = broken_wall(14.0, WALL_H, WALL_T, seed=2, openings=ops, bulge=0.03, lean=0.05, ledge=0.0)
    cost["  the same with no ledges"] = (odcut.triangle_count(flat_wall.mesh), 14.0 * WALL_H)
    cost["  bricks that fell out (3 openings)"] = (sum(odcut.triangle_count(b) for b in bricks), 14.0 * WALL_H)

    # The other three walls are plain boxes.
    m.wall("north_wall", (x0, y1), (x1, y1), 0.0, WALL_H, WALL_T, "brick_red_weathered")
    m.wall("west_wall", (x0, y0), (x0, y1), 0.0, WALL_H, WALL_T, "brick_red_weathered")
    m.wall("east_wall", (x1, y0), (x1, y1), 0.0, WALL_H, WALL_T, "brick_red_weathered",
           openings=[(1.5, 3.5, 0.0, 2.4)])

    # ---------- the roof ----------
    south, north = Roof.gable(
        x0 - 0.5, y0 - 0.5, x1 + 0.5, y1 + 0.5, eave_z=WALL_H, ridge_z=6.2, seed=4,
        south=dict(missing=[3], skylights=[Skylight(5.0, 6.0, 2.2, 3.8, "broken"), Skylight(8.0, 9.0, 2.2, 3.8, "gone"),
                                           Skylight(11.0, 12.0, 2.2, 3.8, "intact")], bays={4: 3.0}),
        north=dict(missing=[10], tears=[Tear(6.0, 1.8, 0.55), Tear(9.5, 3.0, 0.45)]),
    )
    roof_tris, roof_area = 0, 0.0
    for tag, roof in (("south", south), ("north", north)):
        iron = roof.iron()
        m.solid(f"roof_{tag}", iron, None, "roof_corrugated_rusted", "none")
        glass = roof.glass()
        if glass[0]:
            m.solid(f"roof_{tag}_glass", glass, None, "steel_grey", "none")
        m.solid(f"roof_{tag}_solid", roof.collision_mesh(), None, "roof_corrugated_rusted", "trimesh", collision_only=True)
        roof_tris += odcut.triangle_count(iron) + odcut.triangle_count(glass)
        roof_area += roof.length * roof.slope_length
        # A rafter at every truss, from the eave to the ridge.
        for k, a in enumerate(roof.trusses()):
            m.beam(f"rafter_{tag}", roof.point(a, 0.0, -0.1), roof.point(a, roof.slope_length, -0.1), 0.14, 0.2, "steel_beam_rusted", collider="none")
    cost["roof, 15 x 2 slopes of 5.2 m, solid sheets"] = (roof_tris, roof_area)
    open_south = Roof((0, 0, 0), 90.0, south.length, south.run, south.rise, 4, thick=0.0)
    cost["  the same, open sheets (no thickness)"] = (odcut.triangle_count(open_south.iron()), south.length * south.slope_length)

    m.launch_spot(LAUNCH[0], LAUNCH[1], launch_z, 0.0)
    m.export(out)
    return m, cost


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    # `--yard-amplitude 0.6` exaggerates the ground, to see its shape.
    amplitude = float(argv[argv.index("--yard-amplitude") + 1]) if "--yard-amplitude" in argv else 0.15
    builder, cost = build(argv[argv.index("--out") + 1], amplitude)
    print("geometry-test:", builder.stats())
    for name, (tris, area) in cost.items():
        print(f"  {name}: {tris} triangles, {tris / area:.1f} a square metre")
