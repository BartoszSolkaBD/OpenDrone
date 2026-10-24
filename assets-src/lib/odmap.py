"""Builds an OpenDrone Map in Blender and writes its `.glb` (MIT OR Apache-2.0).

This runs inside Blender 5.2, with no window (`cargo xtask maps` starts it).
A Map's script (`assets-src/maps/<id>/build.py`) makes a `MapBuilder`, adds
shapes to it and calls `export`. What comes out follows the Pack's Map rules
(docs/verification/checking-a-pack.md):

- every mesh object carries an `od_collider` tag: `box`, `convex`, `trimesh`
  or `none` (drawn, but nothing solid);
- an object that is solid but never drawn also carries `od_collision_only`;
- one empty carries `od_launch_spot`, the compass heading the Quad faces;
- every object's material is named after an entry in `assets-src/materials.toml`,
  and carries only that entry's flat colour: the game swaps in the pictures;
- corners are worked out in `odgeom` and arrive placed in the Map, so every
  object sits at the origin with no turn: nothing in Blender's 32-bit maths
  can differ between computers, and the same script gives the same bytes.

A shape may blend a second material into its own (`solid(..., blend=(material,
weights))`): its mesh then carries one grey vertex colour per corner
(`COLOR_0`), how much of the second material shows there, and an `od_blend`
tag naming it. A script may put **decals** on its shapes (`decal(...)`): thin
meshes that hug a surface 2 mm above it, with one picture each, tagged
`od_decal` (their layer) and `od_collider = none` (docs/building-a-map.md,
"Decals and blended ground").

A script may ask for baked light with `lightmap(texel_cm)`: every drawn mesh
then gets a second picture-coordinate set (`TEXCOORD_1`, from a deterministic
unwrap in `odlight`), and `cargo xtask maps --bake <id>` bakes the Map's sun
and sky light into an atlas for it (docs/building-a-map.md, "Baked light").

The `.glb` carries no normals: the game works those out, and a Pack's `.glb`
may leave them out. It has no compressed meshes (Bevy refuses them), no
animations and no pictures.
"""
import hashlib
import os
import sys
import tomllib

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402
import odlight  # noqa: E402
from oddecal import Decal, Host  # noqa: E402

COLLIDERS = ("box", "convex", "trimesh", "none")
TOP_DECAL_LAYER = 15  # the Pack checker's highest decal layer
BLEND_ATTRIBUTE = "od_blend"  # the colour attribute the exporter writes as COLOR_0


def _arguments():
    """The `--name value` pairs after `--` on the command line, which
    `cargo xtask maps` passes. A flag with no value is `True`."""
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = {}
    for i, a in enumerate(argv):
        if a.startswith("--"):
            nxt = argv[i + 1] if i + 1 < len(argv) else None
            out[a[2:]] = nxt if nxt is not None and not nxt.startswith("--") else True
    return out


def _write_toml(path, values):
    """A flat TOML file of strings and numbers, in the order given."""
    lines = []
    for k, v in values.items():
        lines.append(f"{k} = {v!r}" if isinstance(v, (int, float)) else f'{k} = "{v}"')
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def _units(text):
    """`"3 m"` or `"1.12 m"` in metres."""
    number, unit = text.split()
    assert unit == "m", f"a texture size is written in metres: {text!r}"
    return float(number)


class MapBuilder:
    def __init__(self, materials_file):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.scene = bpy.context.scene
        with open(materials_file, "rb") as f:
            library = tomllib.load(f)
        self.library = library["materials"]
        self.textures = library["textures"]
        self.materials = {}
        self.objects = []  # (object, collider, collision_only)
        self.counts = {}
        self.launch = None
        self.light = None  # what `lightmap()` asked for
        self.light_cm = {}  # object name -> its own centimetres per texel
        self.light_least = {}  # object name -> the fewest texels a chart of it spans each way
        self.light_result = None  # what the unwrap made, once exported
        self.shapes = {}  # object name -> (corners, triangles), as handed to Blender
        self.light_loops = {}  # object name -> its light coordinates, two numbers a triangle corner
        self.decals = []  # what `decal()` asked for, made at export
        self.decal_objects = []
        self.batched_decals = False

    def batch_decals(self):
        """Puts every decal of one material and one layer into one part of the
        `.glb`, so the game draws them at once: a Map with hundreds of
        decals is otherwise hundreds of draws, and the frame pays for each
        (#341). Each decal is still cut from its own shapes as before; only
        the parts are joined, in the order the decals were asked for."""
        self.batched_decals = True

    # ---------- materials ----------
    def material(self, name):
        if name not in self.materials:
            entry = self.library[name]
            m = bpy.data.materials.new(name)
            bsdf = m.node_tree.nodes["Principled BSDF"]
            bsdf.inputs["Base Color"].default_value = (*entry["colour"], 1.0)
            bsdf.inputs["Roughness"].default_value = entry["roughness"]
            bsdf.inputs["Metallic"].default_value = 0.0
            self.materials[name] = m
        return self.materials[name]

    def tile(self, name):
        """How many metres one repeat of the material's picture covers."""
        texture = self.library[name].get("texture")
        return _units(self.textures[texture]["size"]) if texture else 2.0

    # ---------- objects ----------
    def _name(self, name):
        n = self.counts.get(name, 0)
        self.counts[name] = n + 1
        return name if n == 0 else f"{name}_{n:03d}"

    def lightmap(self, texel_cm, max_chart=odlight.DEFAULT_MAX_CHART, max_size=odlight.DEFAULT_MAX_SIZE):
        """Asks for baked light: every drawn mesh gets a second picture
        coordinate set, packed into one atlas at `texel_cm` centimetres per
        texel (10 is fine, 25 is coarse). A chart longer than `max_chart`
        texels is made coarser to fit, so far-off ground costs little; the
        atlas must fit in `max_size` texels a side. A shape's own `light_cm=`
        overrides the density for that shape."""
        assert self.light is None, "lightmap() is asked for once"
        self.light = {"texel_cm": float(texel_cm), "max_chart": max_chart, "max_size": max_size}

    def solid(self, name, mesh, pose, material, collider, collision_only=False, light_cm=None, blend=None,
              scatter=None, light_least=None):
        """Adds a shape: `mesh` is `(corners, faces)` from `odgeom`, in its own
        axes; `pose` puts it in the Map (None: it is there already).
        `light_cm` is the shape's own centimetres per texel in the light atlas,
        if the Map asks for one (see `lightmap`). `blend=(material_b,
        weights)` shows a second material through this one: `weights` is one
        number from 0 (only `material`) to 1 (only `material_b`) for each
        corner of `mesh`, in its order, such as `WanderingEdge.weights`
        gives. `scatter=(kind, density, (shortest, tallest))` makes the shape
        a ground that grows a plant, such as `("grass", 6, (0.12, 0.38))`:
        clumps a square metre and their heights in metres (#338); the game
        grows them on its upward-facing triangles. `light_least` stretches each of the shape's charts in the atlas
        to at least that many texels each way, so a small thing's faces are
        lit and not left black (`odlight.flatten`)."""
        assert collider in COLLIDERS, collider
        corners, faces = g.placed(mesh, pose or g.Pose())
        triangles = g.triangulate(corners, faces, convex=collider == "convex")
        name = self._name(name)
        for a, b, c in triangles:
            area = g.length(g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a])))
            assert area > 0.0, f"{name} has a triangle with no area after rounding"
        me = bpy.data.meshes.new(name)
        me.from_pydata(corners, [], triangles)
        # One picture coordinate per corner of each triangle: a metre of the
        # Map is a metre of picture, from the face's flattest view, so a
        # picture runs on unbroken from one object to the next.
        tile = self.tile(material)
        flat = []
        for a, b, c in triangles:
            n = g.cross(g.sub(corners[b], corners[a]), g.sub(corners[c], corners[a]))
            ax, ay, az = abs(n[0]), abs(n[1]), abs(n[2])
            for i in (a, b, c):
                x, y, z = corners[i]
                if az >= ax and az >= ay:
                    u, v = x, y
                elif ax >= ay:
                    u, v = y, z
                else:
                    u, v = x, z
                flat += [round(u / tile * 1e4) / 1e4 + 0.0, round(v / tile * 1e4) / 1e4 + 0.0]
        layer = me.uv_layers.new(name="UVMap")
        layer.uv.foreach_set("vector", flat)
        me.materials.append(self.material(material))
        if blend is not None:
            self._blend(name, me, len(corners), *blend)
        ob = bpy.data.objects.new(name, me)
        self.scene.collection.objects.link(ob)
        ob["od_collider"] = collider
        if collision_only:
            ob["od_collision_only"] = 1
        if blend is not None:
            ob["od_blend"] = blend[0]
        if scatter is not None:
            kind, density, (shortest, tallest) = scatter
            ob["od_scatter"] = kind
            ob["od_scatter_density"] = float(density)
            ob["od_scatter_height"] = [float(shortest), float(tallest)]
        self.objects.append((ob, collider, collision_only))
        self.shapes[ob.name] = (corners, triangles)
        if light_cm is not None:
            self.light_cm[ob.name] = float(light_cm)
        if light_least is not None:
            self.light_least[ob.name] = float(light_least)
        return ob

    def _blend(self, name, me, count, material_b, weights):
        """The mesh's vertex colours: each corner's weight of `material_b`, as
        grey. Blender holds them as 32-bit floats, and its glTF exporter
        writes `COLOR_0` as 16-bit whole numbers that stand for 0 to 1
        (normalised unsigned shorts, #324 checked the Factory's `.glb`); a
        weight that is a whole number of 255ths, as `WanderingEdge` gives,
        comes through exactly on every computer."""
        assert material_b in self.library, f"{name}: no material {material_b!r} to blend with"
        assert len(weights) == count, f"{name}: {len(weights)} blend weights for {count} corners"
        flat = []
        for w in weights:
            w = float(w)
            assert 0.0 <= w <= 1.0, f"{name}: a blend weight of {w}"
            flat += [w, w, w, 1.0]
        attribute = me.color_attributes.new(name=BLEND_ATTRIBUTE, type="FLOAT_COLOR", domain="POINT")
        attribute.data.foreach_set("color", flat)

    def decal(self, name, material, on, centre, facing, up, size, layer=0, depth=0.4, max_angle=75.0):
        """Puts a decal on shapes made already: `material` is a decal material
        of the library (its picture, its mask, how wet it makes what is under
        it), `on` the shapes it may lie on (what `solid` and the other shapes
        return, or their names). It is a `size = (width, height)` metre
        rectangle round `centre`, facing `facing` (out of the surface), the
        picture's top toward `up`, reaching `depth` metres into and out of the
        surface, onto faces turned less than `max_angle` degrees from it
        (`oddecal.Decal`). Where decals overlap, a higher `layer` (0 to 15) is
        drawn over a lower one. The mesh is made at export, after the light
        atlas's coordinates, so it can take the surface's own."""
        assert material in self.library, f"decal {name}: no material {material!r} in the library"
        assert int(layer) == layer and 0 <= layer <= TOP_DECAL_LAYER, f"decal {name}: layer {layer} isn't 0 to {TOP_DECAL_LAYER}"
        hosts = [o if isinstance(o, str) else o.name for o in on]
        for h in hosts:
            assert h in self.shapes, f"decal {name} is put on {h!r}, which isn't a shape of this Map"
        name = self._name(name)
        self.decals.append(dict(
            name=name, material=material, hosts=hosts, layer=int(layer),
            projector=Decal(centre, facing, up, size, depth=depth, max_angle=max_angle),
        ))
        return name

    def _make_decals(self):
        """Each decal's mesh, cut from the shapes it lies on, one part each,
        or one part for each material and layer if `batch_decals` was asked."""
        made = []
        for d in self.decals:
            hosts = []
            for h in d["hosts"]:
                corners, triangles = self.shapes[h]
                flat = self.light_loops.get(h)
                light = None
                if flat is not None:
                    light = [tuple((flat[6 * k + 2 * i], flat[6 * k + 2 * i + 1]) for i in range(3)) for k in range(len(triangles))]
                hosts.append(Host(corners, triangles, light))
            corners, triangles, uvs, light = d["projector"].project(hosts)
            if not triangles:
                raise SystemExit(f"decal {d['name']} lies on nothing: no face of {', '.join(d['hosts'])} faces it inside its box")
            made.append((d, corners, triangles, uvs, light))
        if self.batched_decals:
            groups = {}
            for d, corners, triangles, uvs, light in made:
                key = (d["material"], d["layer"])
                if key not in groups:
                    groups[key] = ({"name": f"decals_{d['material']}_{d['layer']}", "material": d["material"],
                                    "layer": d["layer"]}, [], [], [], [] if light is not None else None)
                _, cs, ts, us, ls = groups[key]
                first = len(cs)
                cs.extend(corners)
                ts.extend(tuple(first + i for i in t) for t in triangles)
                us.extend(uvs)
                if ls is not None:
                    ls.extend(light)
            made = list(groups.values())
        for d, corners, triangles, uvs, light in made:
            me = bpy.data.meshes.new(d["name"])
            me.from_pydata(corners, [], triangles)
            # Corners aren't shared, so each triangle corner is its own.
            me.uv_layers.new(name="UVMap").uv.foreach_set("vector", [x for uv in uvs for x in uv])
            if light is not None:
                me.uv_layers.new(name="UVLight").uv.foreach_set("vector", [x for uv in light for x in uv])
            me.materials.append(self.material(d["material"]))
            ob = bpy.data.objects.new(d["name"], me)
            self.scene.collection.objects.link(ob)
            ob["od_collider"] = "none"
            ob["od_decal"] = d["layer"]
            self.decal_objects.append(ob)

    def launch_spot(self, x, y, z, heading):
        """The one Launch Spot: where the Quad starts, landed, facing the
        compass `heading` (0 north, 90 east). `z` is the height of the ground
        under it."""
        assert self.launch is None, "a Map has one Launch Spot"
        ob = bpy.data.objects.new("launch_spot", None)
        self.scene.collection.objects.link(ob)
        ob.location = (g.snap(x), g.snap(y), g.snap(z))
        ob["od_launch_spot"] = float(heading)
        self.launch = ob
        return ob

    # ---------- shapes ----------
    def box(self, name, size, centre, material, heading=0.0, tilt=(0.0, 0.0), collider="box", **kw):
        """A box `size` (x, y, z), turned clockwise to a compass `heading`
        after being tilted about x then y."""
        rows = g.mat_mul(g.rot_z(-heading), g.mat_mul(g.rot_y(tilt[1]), g.rot_x(tilt[0])))
        return self.solid(name, g.box_mesh(*size), g.Pose(rows, centre), material, collider, **kw)

    def box_h(self, name, x, y, sx, sy, z0, z1, material, heading=0.0, **kw):
        """A box standing on `z0` with its top at `z1`, `sx` by `sy` seen from
        above, turned to a compass heading."""
        return self.box(name, (sx, sy, z1 - z0), (x, y, (z0 + z1) / 2.0), material, heading=heading, **kw)

    def wall(self, name, p0, p1, z0, height, thick, material, openings=(), **kw):
        """A straight wall of boxes along the line from `p0` to `p1` (x, y),
        which runs north-south or east-west, standing on `z0`, centred on the
        line. Each opening is `(u0, u1, v0, v1)`: from `u0` to `u1` metres
        along the wall from `p0`, and from `v0` to `v1` metres above `z0`.
        The boxes round the openings are cut by `odgeom.rect_subtract`, in a
        fixed order."""
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        assert dx == 0 or dy == 0, f"{name}: a wall runs along x or along y"
        run = abs(dx) + abs(dy)
        sign = (1.0 if dx >= 0 else -1.0, 1.0 if dy >= 0 else -1.0)
        holes = [(u0, v0, u1, v1) for u0, u1, v0, v1 in openings]
        for a0, b0, a1, b1 in g.rect_subtract((0.0, 0.0, run, height), holes):
            mid = (a0 + a1) / 2.0
            if dx != 0:
                self.box_h(name, p0[0] + sign[0] * mid, p0[1], a1 - a0, thick, z0 + b0, z0 + b1, material, **kw)
            else:
                self.box_h(name, p0[0], p0[1] + sign[1] * mid, thick, a1 - a0, z0 + b0, z0 + b1, material, **kw)

    def beam(self, name, p0, p1, w, h, material, collider="box", **kw):
        """A square bar from one point to another."""
        pose = g.aiming(p0, p1)
        centre = g.lerp(p0, p1, 0.5)
        pose = g.Pose(pose.rows, centre)
        return self.solid(name, g.box_mesh(g.length(g.sub(p1, p0)), w, h), pose, material, collider, **kw)

    def cylinder(self, name, p0, p1, r, material, segments=12, r1=None, collider=None, **kw):
        """A round post, rail or cone from one point to another. One that
        stands straight up is a convex solid; one laid at any other angle is a
        triangle mesh, because its corners can't be rounded flat."""
        if collider is None:
            collider = "convex" if p0[0] == p1[0] and p0[1] == p1[1] else "trimesh"
        mesh = g.cylinder_mesh(g.length(g.sub(p1, p0)), r, r1, segments)
        return self.solid(name, mesh, g.along(p0, p1), material, collider, **kw)

    def prism(self, name, profile, width, origin, heading, material, collider="trimesh", **kw):
        """A closed side outline, `(along, up)` points, cut out `width` wide,
        centred on `origin`, its `along` running on the compass `heading`."""
        return self.solid(name, g.prism_mesh(profile, width), g.facing(origin, heading), material, collider, **kw)

    def tube(self, name, path, radius, material, segments=8, closed_loop=False, collider="trimesh", **kw):
        return self.solid(name, g.tube_mesh(path, radius, segments, closed_loop), None, material, collider, **kw)

    def annulus(self, name, p0, p1, r_in, r_out, material, segments=28, collider="trimesh", **kw):
        mesh = g.annulus_mesh(g.length(g.sub(p1, p0)), r_in, r_out, segments)
        return self.solid(name, mesh, g.along(p0, p1), material, collider, **kw)

    def heightfield(self, name, x0, y0, x1, y1, step, height, material, collider="trimesh", **kw):
        return self.solid(name, g.heightfield_mesh(x0, y0, x1, y1, step, height), None, material, collider, **kw)

    def slab(self, name, x0, y0, x1, y1, z_top, thick, material, holes=(), **kw):
        """A flat slab over a rectangle, a few boxes round the `holes`."""
        for a0, b0, a1, b1 in g.rect_subtract((x0, y0, x1, y1), holes):
            self.box_h(name, (a0 + a1) / 2.0, (b0 + b1) / 2.0, a1 - a0, b1 - b0, z_top - thick, z_top, material, **kw)

    def stairs(self, name, base, heading, width, n, rise, run, material, **kw):
        profile = [(0.0, 0.0)]
        for i in range(n):
            profile += [(i * run, (i + 1) * rise), ((i + 1) * run, (i + 1) * rise)]
        profile += [(n * run, 0.0)]
        return self.prism(name, profile, width, base, heading, material, **kw)

    def bank(self, name, front, heading, width, run, height, material, deck=0.0, z=0.0, **kw):
        profile = [(0.0, 0.0), (run, height)]
        if deck > 0:
            profile.append((run + deck, height))
        profile.append((run + deck, 0.0))
        return self.prism(name, profile, width, (front[0], front[1], z), heading, material, **kw)

    def rail(self, name, p0, p1, material="steel_painted", r=0.03, posts=2, drop=None):
        """A round rail on posts. The posts reach `drop` metres down, or to
        z = 0."""
        self.cylinder(name, p0, p1, r, material, segments=10)
        for k in range(posts):
            t = 0.12 + 0.76 * k / max(1, posts - 1)
            p = g.lerp(p0, p1, t)
            z0 = 0.0 if drop is None else p[2] - drop
            self.cylinder(name + "_post", (p[0], p[1], z0), (p[0], p[1], p[2] - r), r * 1.4, material, segments=8)

    def railing(self, name, points, z0, h=1.0, spacing=2.0, material="steel_painted"):
        pts = [(p[0], p[1], z0) for p in points]
        for a, b in zip(pts, pts[1:]):
            n = max(1, round(g.length(g.sub(b, a)) / spacing))
            for k in range(n + (1 if b is pts[-1] else 0)):
                p = g.lerp(a, b, k / n)
                self.cylinder(name + "_post", p, (p[0], p[1], p[2] + h), 0.025, material, segments=6)
            self.cylinder(name + "_top", (a[0], a[1], a[2] + h), (b[0], b[1], b[2] + h), 0.025, material, segments=6)
            self.cylinder(name + "_mid", (a[0], a[1], a[2] + h / 2), (b[0], b[1], b[2] + h / 2), 0.015, material, segments=6)

    # ---------- scenery ----------
    def tree(self, name, x, y, h=8.0, z=0.0, kind="round"):
        self.cylinder(name + "_trunk", (x, y, z), (x, y, z + h * 0.45), 0.14 + h * 0.012, "bark", segments=7)
        if kind == "conifer":
            self.cylinder(name + "_crown", (x, y, z + h * 0.25), (x, y, z + h), h * 0.22, "foliage", segments=8, r1=0.05)
        else:
            c = h * 0.3
            self.cylinder(name + "_crown", (x, y, z + h * 0.35), (x, y, z + h * 0.7), c * 0.55, "foliage", segments=8, r1=c)
            self.cylinder(name + "_crown_top", (x, y, z + h * 0.7), (x, y, z + h), c, "foliage", segments=8, r1=c * 0.35)

    def light_pole(self, name, x, y, h=8.0, z=0.0, heading=0.0):
        self.cylinder(name, (x, y, z), (x, y, z + h), 0.09, "steel_grey", segments=8, r1=0.06)
        hv = g.heading_vec(heading)
        self.box(name + "_lamp", (0.35, 0.9, 0.18), (x + hv[0] * 0.45, y + hv[1] * 0.45, z + h), "steel_grey", heading=heading)

    # ---------- the file ----------
    def _light_uvs(self, request):
        """Gives every drawn mesh its second picture-coordinate set, from the
        deterministic unwrap in `odlight`. Returns the atlas's size and what
        the unwrap did."""
        meshes = []
        drawn = [ob for ob, _, only in self.objects if not only]
        for ob in drawn:
            me = ob.data
            corners = [tuple(v.co) for v in me.vertices]
            triangles = [tuple(p.vertices) for p in me.polygons]
            if ob.name in self.light_least:
                meshes.append((corners, triangles, self.light_cm.get(ob.name), self.light_least[ob.name]))
            else:
                meshes.append((corners, triangles, self.light_cm.get(ob.name)))
        uvs, size, info = odlight.unwrap(
            meshes, request["texel_cm"], max_chart=request["max_chart"], max_size=request["max_size"]
        )
        for ob, flat in zip(drawn, uvs):
            layer = ob.data.uv_layers.new(name="UVLight")
            layer.uv.foreach_set("vector", flat)
            self.light_loops[ob.name] = list(flat)
        return size, info

    def export(self, path):
        """Writes the `.glb`: every object and the Launch Spot, nothing else.
        If the script asked for baked light (or the command line asks for a
        proof of the unwrap), every drawn mesh also has `TEXCOORD_1`; the
        command line may then ask for the bake (`cargo xtask maps --bake`)."""
        assert self.launch is not None, "a Map needs a Launch Spot"
        args = _arguments()
        request = self.light
        if request is None and "light-uv-proof" in args:
            request = {
                "texel_cm": float(args["light-uv-proof"]),
                "max_chart": odlight.DEFAULT_MAX_CHART,
                "max_size": odlight.DEFAULT_MAX_SIZE,
            }
        if "bake" in args and request is None:
            raise SystemExit("--bake: this Map's script never calls lightmap(texel_cm), so it has no second picture-coordinate set to bake into")
        if request is not None:
            size, info = self._light_uvs(request)
            self.light_result = dict(texel_cm=request["texel_cm"], width=size[0], height=size[1], **info)
        self._make_decals()
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        view_layer = bpy.context.view_layer
        view_layer.update()
        for ob in bpy.data.objects:
            ob.select_set(False)
        for ob, _, _ in self.objects:
            ob.select_set(True)
        for ob in self.decal_objects:
            ob.select_set(True)
        self.launch.select_set(True)
        bpy.ops.export_scene.gltf(
            filepath=path,
            export_format="GLB",
            use_selection=True,
            export_extras=True,
            export_texcoords=True,
            export_normals=False,
            export_tangents=False,
            export_materials="EXPORT",
            # Only a blend's weights: a mesh without them has no COLOR_0.
            export_vertex_color="NAME",
            export_vertex_color_name=BLEND_ATTRIBUTE,
            export_all_vertex_colors=False,
            export_attributes=False,
            export_image_format="NONE",
            export_cameras=False,
            export_lights=False,
            export_apply=False,
            export_yup=True,
            export_animations=False,
            export_skins=False,
            export_morph=False,
            export_draco_mesh_compression_enable=False,
            export_meshopt_compression_enable=False,
            export_use_gltfpack=False,
            export_gpu_instances=False,
        )
        if "light-info" in args and self.light_result is not None:
            _write_toml(args["light-info"], self.light_result)
        if "bake" in args:
            self._bake(path, args)

    def _bake(self, glb_path, args):
        """`--bake <atlas.exr> --map-toml <map.toml> --bake-report <file>`:
        bakes the Map's `[light]` into the atlas and writes the report that
        `cargo xtask maps` turns into the Map's `<id>.bake.toml`."""
        with open(args["map-toml"], "rb") as f:
            table = tomllib.load(f).get("light")
        if table is None:
            raise SystemExit(f"--bake: {args['map-toml']} has no [light] table to bake with")
        light = odlight.read_light(table)
        drawn = [ob for ob, _, only in self.objects if not only]
        for ob, _, only in self.objects:
            if only:
                ob.hide_render = True  # solid but never drawn: it casts no shade
        for ob in self.decal_objects:
            ob.hide_render = True  # a picture on a surface: it casts no shade on it
        result = odlight.bake(
            drawn,
            light,
            args["bake"],
            (self.light_result["width"], self.light_result["height"]),
            samples=int(args.get("samples", odlight.DEFAULT_SAMPLES)),
        )
        with open(glb_path, "rb") as f:
            glb_sha256 = hashlib.sha256(f.read()).hexdigest()
        _write_toml(
            args["bake-report"],
            {
                "atlas": os.path.basename(args["bake"]),
                "glb_sha256": glb_sha256,
                "light_sha256": odlight.light_key(table),
                "texel_cm": self.light_result["texel_cm"],
                "width": self.light_result["width"],
                "height": self.light_result["height"],
                "samples": result["samples"],
                "bounces": result["bounces"],
                "bake_seconds": result["seconds"],
            },
        )

    def stats(self):
        kinds = {}
        triangles = 0
        for ob, collider, only in self.objects:
            kinds[collider] = kinds.get(collider, 0) + 1
            triangles += len(ob.data.polygons)
        stats = {"objects": len(self.objects), "triangles": triangles, "colliders": kinds}
        if self.decal_objects:
            stats["decals"] = len(self.decal_objects)
            stats["decal_triangles"] = sum(len(ob.data.polygons) for ob in self.decal_objects)
        return stats
