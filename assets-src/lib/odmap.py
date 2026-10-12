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

The `.glb` carries no normals: the game works those out, and a Pack's `.glb`
may leave them out. It has no compressed meshes (Bevy refuses them), no
animations and no pictures.
"""
import os
import sys
import tomllib

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odgeom as g  # noqa: E402

COLLIDERS = ("box", "convex", "trimesh", "none")


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

    def solid(self, name, mesh, pose, material, collider, collision_only=False):
        """Adds a shape: `mesh` is `(corners, faces)` from `odgeom`, in its own
        axes; `pose` puts it in the Map (None: it is there already)."""
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
        ob = bpy.data.objects.new(name, me)
        self.scene.collection.objects.link(ob)
        ob["od_collider"] = collider
        if collision_only:
            ob["od_collision_only"] = 1
        self.objects.append((ob, collider, collision_only))
        return ob

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
    def export(self, path):
        """Writes the `.glb`: every object and the Launch Spot, nothing else."""
        assert self.launch is not None, "a Map needs a Launch Spot"
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        view_layer = bpy.context.view_layer
        view_layer.update()
        for ob in bpy.data.objects:
            ob.select_set(False)
        for ob, _, _ in self.objects:
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
            export_vertex_color="NONE",
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

    def stats(self):
        kinds = {}
        triangles = 0
        for ob, collider, only in self.objects:
            kinds[collider] = kinds.get(collider, 0) + 1
            triangles += len(ob.data.polygons)
        return {"objects": len(self.objects), "triangles": triangles, "colliders": kinds}
