"""PROTOTYPE, throwaway (OpenDrone #17: "What do Skate Park and Bando look like?").

Shared helpers for the Map blockout variants. Runs inside Blender 5.2 LTS, headless.
Blender is Z-up and the glTF export converts to Y-up. Units are metres.
Compass headings: 0 = north (+Y), 90 = east (+X).

Follows the asset-pipeline research (#8): explicit vertex and face lists, two UV sets
(UVMap and Lightmap, unwrapped with smart_project), `od_collider` tags in node extras,
library material names, one .glb per Map. Launch Spot candidates are exported as empty
nodes tagged `od_launch_spot`. Everything render-only (grid shader, sky, sun, scale
figures, cameras) is added after the export, so it never reaches the .glb.
"""
import bpy, bmesh, math, json, os, time, random
from mathutils import Vector, Matrix
from bpy_extras.object_utils import world_to_camera_view

# Library material name -> blockout colour (linear RGB). The game swaps these by name.
COLORS = {
    "concrete_plaza": (0.40, 0.40, 0.38),
    "concrete_smooth": (0.36, 0.40, 0.46),   # rideable transitions: a cooler grey
    "concrete_structure": (0.42, 0.42, 0.42),
    "concrete_dark": (0.30, 0.30, 0.30),
    "wood_ramp": (0.60, 0.40, 0.20),
    "steel_painted": (0.80, 0.42, 0.04),     # rails, coping, handrails
    "steel_rusty": (0.36, 0.17, 0.09),
    "steel_grey": (0.30, 0.32, 0.35),
    "brick_old": (0.42, 0.18, 0.12),
    "rubble": (0.33, 0.29, 0.25),
    "grass": (0.16, 0.26, 0.09),
    "dirt": (0.35, 0.27, 0.17),
    "asphalt": (0.12, 0.12, 0.13),
    "foliage": (0.10, 0.24, 0.07),
    "bark": (0.22, 0.14, 0.08),
    "backdrop": (0.55, 0.57, 0.62),
    "metal_sheet": (0.38, 0.44, 0.50),
    "container_blue": (0.05, 0.16, 0.42),
    "container_red": (0.45, 0.06, 0.04),
    "timber_old": (0.30, 0.20, 0.12),
}

QUADS = {
    # Camera Tilt default: 30 deg on both Quads (decided in the FPV-camera ticket, #14).
    # Landed camera heights are estimates.
    "whoop": dict(label="Whoop (65 mm)", cam_h=0.035, tilt=30.0),
    "five": dict(label='5" freestyle', cam_h=0.07, tilt=30.0),
}
FPV_HFOV = 110.0  # degrees, horizontal, plain (rectilinear) lens: a placeholder; #14 chose a fisheye warp


def _rad(d):
    return math.radians(d)


def heading_vec(h):
    return Vector((math.sin(_rad(h)), math.cos(_rad(h)), 0.0))


def rect_subtract(outer, holes):
    """Split rectangle `outer` (x0, y0, x1, y1) minus `holes` into a few rectangles."""
    x0, y0, x1, y1 = outer
    hs = [(max(a, x0), max(b, y0), min(c, x1), min(d, y1)) for a, b, c, d in holes]
    hs = [h for h in hs if h[2] > h[0] + 1e-6 and h[3] > h[1] + 1e-6]
    xs = sorted({x0, x1, *[h[0] for h in hs], *[h[2] for h in hs]})
    ys = sorted({y0, y1, *[h[1] for h in hs], *[h[3] for h in hs]})

    def solid(cx, cy):
        return not any(a < cx < c and b < cy < d for a, b, c, d in hs)

    rows = []
    for j in range(len(ys) - 1):
        cy = (ys[j] + ys[j + 1]) / 2
        run = None
        for i in range(len(xs) - 1):
            cx = (xs[i] + xs[i + 1]) / 2
            if solid(cx, cy):
                run = [run[0], xs[i + 1]] if run else [xs[i], xs[i + 1]]
            elif run:
                rows.append((run[0], ys[j], run[1], ys[j + 1])); run = None
        if run:
            rows.append((run[0], ys[j], run[1], ys[j + 1]))
    # merge vertically where x-spans match
    merged = []
    for r in rows:
        for k, m in enumerate(merged):
            if abs(m[0] - r[0]) < 1e-6 and abs(m[2] - r[2]) < 1e-6 and abs(m[3] - r[1]) < 1e-6:
                merged[k] = (m[0], m[1], m[2], r[3]); break
        else:
            merged.append(r)
    return merged


def window_row(length, width, height, sill, spacing, margin=1.0, skip=()):
    """Evenly spaced openings (u0, u1, v0, v1) along a wall of `length`."""
    n = max(1, int((length - 2 * margin + spacing - width) // spacing))
    used = (n - 1) * spacing + width
    start = (length - used) / 2
    return [(start + i * spacing, start + i * spacing + width, sill, sill + height)
            for i in range(n) if i not in skip]


class Map:
    def __init__(self, key, title, map_name, pitch):
        self.t_start = time.time()
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.scene = bpy.context.scene
        self.key, self.title, self.map_name, self.pitch = key, title, map_name, pitch
        self.mats, self.names = {}, {}
        self.parts, self.features, self.spots, self.shots = [], [], [], []
        self.groups = {}
        self.core_names = set()   # objects inside the flyable core (stats only)
        self.core = None          # (x0, y0, x1, y1) of the flyable core, set by the variant
        self.recommended_spot = None
        self.spot_tag = None      # e.g. "Chosen in Round 1"; the gallery shows "my pick" otherwise
        self.round = 1
        self.plans = [dict(name="plan", caption="Plan from above", cut=None)]
        self.three_quarter = dict(az=215.0, elev=32.0, dist=None, lens=28.0, hide=(), target=None)
        self.extra_views = []
        self.figures = []
        self.facts = []

    # ---------- materials ----------
    def mat(self, key):
        if key not in self.mats:
            m = bpy.data.materials.new(key)
            bsdf = m.node_tree.nodes["Principled BSDF"]
            bsdf.inputs["Base Color"].default_value = (*COLORS[key], 1.0)
            bsdf.inputs["Roughness"].default_value = 0.85
            self.mats[key] = m
        return self.mats[key]

    # ---------- object creation ----------
    def _name(self, name):
        n = self.names.get(name, 0)
        self.names[name] = n + 1
        return name if n == 0 else f"{name}_{n:03d}"

    def _add(self, name, verts, faces, mat, collider, loc=(0, 0, 0), rot=None, closed=True,
             smooth_faces=None, group=None, core=True):
        name = self._name(name)
        me = bpy.data.meshes.new(name)
        me.from_pydata([tuple(v) for v in verts], [], [tuple(f) for f in faces])
        me.validate()
        if closed:
            bm = bmesh.new(); bm.from_mesh(me)
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            bm.to_mesh(me); bm.free()
        if smooth_faces is not None:
            for i, p in enumerate(me.polygons):
                p.use_smooth = i in smooth_faces
        me.update()
        ob = bpy.data.objects.new(name, me)
        self.scene.collection.objects.link(ob)
        ob.location = loc
        if rot is not None:
            ob.rotation_mode = "QUATERNION"
            ob.rotation_quaternion = rot
        me.materials.append(self.mat(mat))
        if collider:
            ob["od_collider"] = collider
        if core:
            self.core_names.add(ob.name)
        self.parts.append(ob)
        if group:
            self.groups.setdefault(group, []).append(ob)
        return ob

    @staticmethod
    def _box_mesh(sx, sy, sz):
        hx, hy, hz = sx / 2, sy / 2, sz / 2
        v = [(x * hx, y * hy, z * hz) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)]
        f = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        return v, f

    def box(self, name, size, center, mat, rot=(0, 0, 0), collider="box", **kw):
        v, f = self._box_mesh(*size)
        q = Matrix.Rotation(_rad(rot[2]), 4, "Z") @ Matrix.Rotation(_rad(rot[1]), 4, "Y") @ Matrix.Rotation(_rad(rot[0]), 4, "X")
        return self._add(name, v, f, mat, collider, loc=center, rot=q.to_quaternion(), **kw)

    def box_h(self, name, x, y, sx, sy, z0, z1, mat, heading=0.0, **kw):
        """Box standing on z0, top at z1, rotated to a compass heading."""
        return self.box(name, (sx, sy, z1 - z0), (x, y, (z0 + z1) / 2), mat, rot=(0, 0, -heading), **kw)

    def beam(self, name, p0, p1, w, h, mat, collider="box", **kw):
        p0, p1 = Vector(p0), Vector(p1)
        d = p1 - p0
        q = d.to_track_quat("X", "Y" if abs(d.normalized().z) > 0.999 else "Z")
        v, f = self._box_mesh(d.length, w, h)
        return self._add(name, v, f, mat, collider, loc=(p0 + p1) / 2, rot=q, **kw)

    def cyl(self, name, p0, p1, r, mat, seg=12, r1=None, collider="convex", smooth=True, **kw):
        p0, p1 = Vector(p0), Vector(p1)
        d = p1 - p0
        r1 = r if r1 is None else r1
        ring = [(math.cos(2 * math.pi * i / seg), math.sin(2 * math.pi * i / seg)) for i in range(seg)]
        v = [(r * c, r * s, 0.0) for c, s in ring] + [(r1 * c, r1 * s, d.length) for c, s in ring]
        f = [tuple(reversed(range(seg))), tuple(range(seg, 2 * seg))]
        f += [(i, (i + 1) % seg, seg + (i + 1) % seg, seg + i) for i in range(seg)]
        q = d.to_track_quat("Z", "X" if abs(d.normalized().x) < 0.9 else "Y")
        sm = set(range(2, 2 + seg)) if smooth else None
        return self._add(name, v, f, mat, collider, loc=p0, rot=q, smooth_faces=sm, **kw)

    def annulus(self, name, p0, p1, r_in, r_out, mat, seg=28, collider="trimesh", **kw):
        """A thick-walled tube: a full pipe or a culvert."""
        p0, p1 = Vector(p0), Vector(p1)
        d = p1 - p0
        L = d.length
        ring = [(math.cos(2 * math.pi * i / seg), math.sin(2 * math.pi * i / seg)) for i in range(seg)]
        v = []
        for z in (0.0, L):
            v += [(r_out * c, r_out * s, z) for c, s in ring]
            v += [(r_in * c, r_in * s, z) for c, s in ring]
        o0, i0, o1, i1 = 0, seg, 2 * seg, 3 * seg
        f = []
        for i in range(seg):
            j = (i + 1) % seg
            f.append((o0 + i, o0 + j, o1 + j, o1 + i))      # outside
            f.append((i0 + j, i0 + i, i1 + i, i1 + j))      # inside
            f.append((o0 + j, o0 + i, i0 + i, i0 + j))      # end ring 0
            f.append((o1 + i, o1 + j, i1 + j, i1 + i))      # end ring 1
        q = d.to_track_quat("Z", "X" if abs(d.normalized().x) < 0.9 else "Y")
        sm = set(range(0, 4 * seg, 4)) | set(range(1, 4 * seg, 4))
        return self._add(name, v, f, mat, collider, loc=p0, rot=q, smooth_faces=sm, **kw)

    def tube(self, name, pts, r, mat, seg=8, closed_loop=False, collider="trimesh", **kw):
        """Sweep a circle along a polyline (coping, curved rails, ring sculptures)."""
        pts = [Vector(p) for p in pts]
        n = len(pts)
        v, f = [], []
        up = Vector((0, 0, 1))
        for k, p in enumerate(pts):
            if closed_loop:
                t = (pts[(k + 1) % n] - pts[(k - 1) % n]).normalized()
            else:
                t = (pts[min(k + 1, n - 1)] - pts[max(k - 1, 0)]).normalized()
            ref = up if abs(t.dot(up)) < 0.9 else Vector((1, 0, 0))
            a = t.cross(ref).normalized(); b = t.cross(a).normalized()
            for i in range(seg):
                ang = 2 * math.pi * i / seg
                v.append(p + r * (math.cos(ang) * a + math.sin(ang) * b))
        rings = n if closed_loop else n - 1
        for k in range(rings):
            k2 = (k + 1) % n
            for i in range(seg):
                j = (i + 1) % seg
                f.append((k * seg + i, k * seg + j, k2 * seg + j, k2 * seg + i))
        if not closed_loop:
            f.append(tuple(reversed(range(seg))))
            f.append(tuple(range((n - 1) * seg, n * seg)))
        return self._add(name, v, f, mat, collider, smooth_faces=set(range(rings * seg)), **kw)

    def prism(self, name, profile, width, origin, heading, mat, collider="trimesh", smooth_edges=(), **kw):
        """Extrude a closed side profile [(a, z)...] across `width`.
        `a` runs along the compass `heading`; the extrusion is centred on `origin`."""
        n = len(profile)
        v = [(x, a, z) for x in (-width / 2, width / 2) for a, z in profile]
        f = [tuple(reversed(range(n))), tuple(range(n, 2 * n))]
        f += [(i, (i + 1) % n, n + (i + 1) % n, n + i) for i in range(n)]
        sm = {2 + i for i in smooth_edges}
        q = Matrix.Rotation(_rad(-heading), 4, "Z").to_quaternion()
        return self._add(name, v, f, mat, collider, loc=origin, rot=q, smooth_faces=sm, **kw)

    def panel(self, name, origin, u, v, U, V, thick, holes, mat, collider="box", **kw):
        """A flat slab or wall in any plane, with rectangular openings (u0, u1, v0, v1)."""
        u, v = Vector(u).normalized(), Vector(v).normalized()
        n = u.cross(v)
        rot = Matrix((u, v, n)).transposed().to_quaternion()
        out = []
        for a0, b0, a1, b1 in rect_subtract((0, 0, U, V), [(h[0], h[2], h[1], h[3]) for h in holes]):
            c = Vector(origin) + ((a0 + a1) / 2) * u + ((b0 + b1) / 2) * v
            bv, bf = self._box_mesh(a1 - a0, b1 - b0, thick)
            out.append(self._add(name, bv, bf, mat, collider, loc=c, rot=rot, **kw))
        return out

    def slab(self, name, x0, y0, x1, y1, z_top, thick, mat, holes=(), **kw):
        hl = [(h[0] - x0, h[2] - x0, h[1] - y0, h[3] - y0) for h in holes]  # holes: (x0,y0,x1,y1)
        return self.panel(name, (x0, y0, z_top - thick / 2), (1, 0, 0), (0, 1, 0), x1 - x0, y1 - y0, thick, hl, mat, **kw)

    def wall(self, name, p0, p1, z0, height, thick, mat, openings=(), **kw):
        """Wall from p0 to p1 (x, y), centred on that line. Openings are (u0, u1, v0, v1)."""
        p0, p1 = Vector((*p0, z0)), Vector((*p1, z0))
        d = p1 - p0
        return self.panel(name, p0, d, (0, 0, 1), d.length, height, thick, openings, mat, **kw)

    def walls_rect(self, name, x0, y0, x1, y1, z0, h, thick, mat, sides, **kw):
        """Walls around a rectangle, centred on its edges. `sides` maps 's', 'e', 'n', 'w' to a list of
        openings (u0, u1, v0, v1). u runs anticlockwise from the corner: s west->east, e south->north,
        n east->west, w north->south. Leave a side out for no wall."""
        t = thick / 2
        segs = {"s": ((x0 - t, y0), (x1 + t, y0)), "e": ((x1, y0 - t), (x1, y1 + t)),
                "n": ((x1 + t, y1), (x0 - t, y1)), "w": ((x0, y1 + t), (x0, y0 - t))}
        for k in "senw":
            if k in sides and sides[k] is not None:
                ops = [(a + t, b + t, c, d) for a, b, c, d in sides[k]]
                self.wall(f"{name}_{k}", *segs[k], z0, h, thick, mat, openings=ops, **kw)

    def heightfield(self, name, x0, y0, x1, y1, step, fn, mat, collider="trimesh", **kw):
        nx, ny = max(1, round((x1 - x0) / step)), max(1, round((y1 - y0) / step))
        v = []
        for j in range(ny + 1):
            y = y0 + (y1 - y0) * j / ny
            for i in range(nx + 1):
                x = x0 + (x1 - x0) * i / nx
                v.append((x, y, fn(x, y)))
        f = [(j * (nx + 1) + i, j * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i)
             for j in range(ny) for i in range(nx)]
        return self._add(name, v, f, mat, collider, closed=False, smooth_faces=set(range(len(f))), **kw)

    def stairs(self, name, base, heading, width, n, rise, run, mat, **kw):
        pts = [(0.0, 0.0)]
        for i in range(n):
            pts += [(i * run, (i + 1) * rise), ((i + 1) * run, (i + 1) * rise)]
        pts += [(n * run, 0.0)]
        return self.prism(name, pts, width, base, heading, mat, collider="trimesh", **kw)

    # ---------- skate pieces ----------
    def quarter_pipe(self, name, front, heading, width, r, vert=0.0, deck=1.0, mat="wood_ramp", z=0.0,
                     coping=True, seg=14, **kw):
        """Transition rising toward `heading`. `front` (x, y) is where it meets the flat."""
        prof = [(r * math.sin(a), r - r * math.cos(a)) for a in (i * (math.pi / 2) / seg for i in range(seg + 1))]
        top = r + vert
        if vert > 0:
            prof.append((r, top))
        prof += [(r + deck, top), (r + deck, 0.0)]
        ob = self.prism(name, prof, width, (front[0], front[1], z), heading, mat, smooth_edges=range(seg), **kw)
        if coping:
            h = heading_vec(heading); p = Vector((front[0], front[1], z)) + h * r
            side = Vector((h.y, -h.x, 0))
            self.cyl(name + "_coping", p - side * width / 2 + Vector((0, 0, top)), p + side * width / 2 + Vector((0, 0, top)),
                     0.032, "steel_painted", seg=8)
        return ob

    def bank(self, name, front, heading, width, run, height, mat="concrete_smooth", deck=0.0, z=0.0, **kw):
        prof = [(0.0, 0.0), (run, height)]
        if deck > 0:
            prof.append((run + deck, height))
        prof.append((run + deck, 0.0))
        return self.prism(name, prof, width, (front[0], front[1], z), heading, mat, **kw)

    def rail(self, name, p0, p1, mat="steel_painted", r=0.03, posts=2, drop=None):
        """A round rail from p0 to p1 on posts. Posts reach `drop` metres down (default: to z = 0)."""
        p0, p1 = Vector(p0), Vector(p1)
        self.cyl(name, p0, p1, r, mat, seg=10)
        for k in range(posts):
            t = 0.12 + 0.76 * k / max(1, posts - 1)
            p = p0.lerp(p1, t)
            z0 = 0.0 if drop is None else p.z - drop
            self.cyl(name + "_post", (p.x, p.y, z0), (p.x, p.y, p.z - r), r * 1.4, mat, seg=8)

    def railing(self, name, pts, z0, h=1.0, spacing=2.0, mat="steel_painted"):
        pts = [Vector((p[0], p[1], z0)) for p in pts]
        for a, b in zip(pts, pts[1:]):
            L = (b - a).length
            n = max(1, round(L / spacing))
            for k in range(n + (1 if b is pts[-1] else 0)):
                p = a.lerp(b, k / n)
                self.cyl(name + "_post", p, p + Vector((0, 0, h)), 0.025, mat, seg=6)
            self.cyl(name + "_top", a + Vector((0, 0, h)), b + Vector((0, 0, h)), 0.025, mat, seg=6)
            self.cyl(name + "_mid", a + Vector((0, 0, h / 2)), b + Vector((0, 0, h / 2)), 0.015, mat, seg=6)

    # ---------- scenery ----------
    def tree(self, name, x, y, h=8.0, z=0.0, kind="round", core=False):
        self.cyl(name + "_trunk", (x, y, z), (x, y, z + h * 0.45), 0.14 + h * 0.012, "bark", seg=7, core=core)
        if kind == "conifer":
            self.cyl(name + "_crown", (x, y, z + h * 0.25), (x, y, z + h), h * 0.22, "foliage", seg=8, r1=0.05, core=core)
        else:
            c = h * 0.3
            self.cyl(name + "_crown", (x, y, z + h * 0.35), (x, y, z + h * 0.7), c * 0.55, "foliage", seg=8, r1=c, core=core)
            self.cyl(name + "_crown_top", (x, y, z + h * 0.7), (x, y, z + h), c, "foliage", seg=8, r1=c * 0.35, core=core)

    def light_pole(self, name, x, y, h=8.0, z=0.0, heading=0.0, core=True):
        self.cyl(name, (x, y, z), (x, y, z + h), 0.09, "steel_grey", seg=8, r1=0.06, core=core)
        hv = heading_vec(heading)
        self.box(name + "_lamp", (0.35, 0.9, 0.18), (x + hv.x * 0.45, y + hv.y * 0.45, z + h), "steel_grey",
                 rot=(0, 0, -heading), core=core)

    def fence(self, name, pts, h=2.0, spacing=2.5, z0=0.0, core=True):
        pts = [Vector((p[0], p[1], z0)) for p in pts]
        for a, b in zip(pts, pts[1:]):
            n = max(1, round((b - a).length / spacing))
            for k in range(n):
                p = a.lerp(b, k / n)
                self.cyl(name + "_post", p, p + Vector((0, 0, h)), 0.035, "steel_grey", seg=6, core=core)
            self.cyl(name + "_rail", a + Vector((0, 0, h)), b + Vector((0, 0, h)), 0.025, "steel_grey", seg=6, core=core)
        self.cyl(name + "_post", pts[-1], pts[-1] + Vector((0, 0, h)), 0.035, "steel_grey", seg=6, core=core)

    def rubble(self, name, x, y, radius, n, seed, z=0.0, size=0.8, mat="rubble"):
        rng = random.Random(seed)
        for k in range(n):
            a, d = rng.uniform(0, 2 * math.pi), radius * math.sqrt(rng.random())
            s = size * (1 - 0.6 * d / max(radius, 1e-3))
            sx, sy, sz = (s * rng.uniform(0.4, 1.2) for _ in range(3))
            self.box(name, (sx, sy, sz), (x + d * math.cos(a), y + d * math.sin(a), z + sz * 0.3),
                     mat, rot=(rng.uniform(-25, 25), rng.uniform(-25, 25), rng.uniform(0, 180)))

    def container(self, name, x, y, heading, z=0.0, mat="container_blue"):
        return self.box_h(name, x, y, 2.44, 6.06, z, z + 2.59, mat, heading=heading)

    def backdrop_block(self, name, x, y, sx, sy, h, heading=0.0):
        return self.box_h(name, x, y, sx, sy, 0.0, h, "backdrop", heading=heading, core=False)

    def ground(self, size=400.0, mat="grass", z_top=-0.05, holes=()):
        """The surrounding ground, with rectangular holes (x0, y0, x1, y1) where the Map's own
        ground (or anything sunken) takes over."""
        h = size / 2
        for x0, y0, x1, y1 in rect_subtract((-h, -h, h, h), holes):
            self.box("far_ground", (x1 - x0, y1 - y0, 0.3), ((x0 + x1) / 2, (y0 + y1) / 2, z_top - 0.15), mat, core=False)

    # ---------- annotations ----------
    def feature(self, name, desc, pos, suits="both"):
        self.features.append(dict(n=len(self.features) + 1, name=name, desc=desc, pos=list(pos), suits=suits))

    def spot(self, sid, x, y, heading, note, z_hint=0.0, recommended=False):
        self.spots.append(dict(id=sid, x=x, y=y, z_hint=z_hint, heading=heading, note=note))
        if recommended:
            self.recommended_spot = sid

    def fly_shot(self, name, caption, pos, heading, pitch, quad):
        self.shots.append(dict(kind="flight", name=name, caption=caption, pos=list(pos), heading=heading,
                               pitch=pitch, quad=quad))

    def figure(self, x, y, z=0.0):
        self.figures.append((x, y, z))

    # ---------- build, export, render ----------
    def finish(self, out_dir, render=True, quick=False):
        t0 = time.time()
        os.makedirs(os.path.join(out_dir, "glb"), exist_ok=True)
        os.makedirs(os.path.join(out_dir, "img"), exist_ok=True)
        meta = dict(key=self.key, title=self.title, map=self.map_name, pitch=self.pitch, facts=self.facts,
                    features=self.features, recommended_spot=self.recommended_spot, spot_tag=self.spot_tag,
                    round=self.round)
        vl = bpy.context.view_layer
        vl.update()

        # Launch Spots: find the surface under each one, measure headroom and nearby clearance.
        dg = bpy.context.evaluated_depsgraph_get()
        spots_out, spot_objs = [], []
        for s in self.spots:
            hit, loc, *_ = self.scene.ray_cast(dg, Vector((s["x"], s["y"], s["z_hint"] + 0.6)), Vector((0, 0, -1)), distance=5.0)
            z = loc.z if hit else s["z_hint"]
            hit_up, loc_up, *_ = self.scene.ray_cast(dg, Vector((s["x"], s["y"], z + 0.05)), Vector((0, 0, 1)), distance=500.0)
            head = (loc_up.z - z) if hit_up else None
            near = 99.0
            for k in range(16):
                d = Vector((math.cos(k * math.pi / 8), math.sin(k * math.pi / 8), 0))
                h2, l2, *_ = self.scene.ray_cast(dg, Vector((s["x"], s["y"], z + 0.3)), d, distance=99.0)
                if h2:
                    near = min(near, (l2 - Vector((s["x"], s["y"], z + 0.3))).length)
            e = bpy.data.objects.new(f"launch_spot_{s['id']}", None)
            self.scene.collection.objects.link(e)
            e.location = (s["x"], s["y"], z)
            e.rotation_euler = (0, 0, _rad(-s["heading"]))
            e["od_launch_spot"] = s["id"]
            spot_objs.append(e)
            spots_out.append(dict(s, z=round(z, 3), headroom=None if head is None else round(head, 2),
                                  nearest=round(near, 1)))
        meta["spots"] = spots_out

        # Two UV sets, smart_project for both (byte-stable in the #8 probe).
        meshes = [o for o in self.parts]
        for ob in meshes:
            ob.data.uv_layers.new(name="UVMap")
            ob.data.uv_layers.new(name="Lightmap")
        bpy.ops.object.select_all(action="DESELECT")
        for ob in meshes:
            ob.select_set(True)
        vl.objects.active = meshes[0]
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        for layer, margin in (("UVMap", 0.01), ("Lightmap", 0.002)):
            for ob in meshes:
                ob.data.uv_layers.active = ob.data.uv_layers[layer]
            bpy.ops.uv.smart_project(island_margin=margin)
        bpy.ops.object.mode_set(mode="OBJECT")

        # Export: meshes plus Launch Spot empties, nothing render-only.
        for ob in spot_objs:
            ob.select_set(True)
        glb = os.path.join(out_dir, "glb", f"{self.key}.glb")
        bpy.ops.export_scene.gltf(filepath=glb, export_format="GLB", use_selection=True, export_extras=True,
                                  export_texcoords=True, export_apply=True, export_yup=True,
                                  export_cameras=False, export_lights=False)
        t_build = time.time() - self.t_start   # from an empty scene to a written .glb

        # Stats
        tris, area, area_core = 0, 0.0, 0.0
        colliders = {}
        lo, hi = Vector((1e9, 1e9, 1e9)), Vector((-1e9, -1e9, -1e9))
        for ob in meshes:
            me = ob.data
            me.calc_loop_triangles()
            tris += len(me.loop_triangles)
            mw = ob.matrix_world
            rot = mw.to_3x3()
            # Lightmap area: skip hidden undersides that rest on the ground (downward faces near z = 0).
            a = sum(p.area for p in me.polygons
                    if not ((rot @ p.normal).z < -0.7 and (mw @ p.center).z < 0.05))
            area += a
            if ob.name in self.core_names:
                area_core += a
                for c in ob.bound_box:
                    w = mw @ Vector(c)
                    lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
            colliders[ob.get("od_collider", "none")] = colliders.get(ob.get("od_collider", "none"), 0) + 1
        cx0, cy0, cx1, cy1 = self.core
        diag = math.hypot(cx1 - cx0, cy1 - cy0)
        texel_cm = 100 * math.sqrt(area_core / (0.65 * 2048 * 2048))
        meta["stats"] = dict(
            core=[cx0, cy0, cx1, cy1], core_w=round(cx1 - cx0, 1), core_d=round(cy1 - cy0, 1),
            top=round(hi.z, 1), low=round(lo.z, 1), diag=round(diag, 1),
            whoop_cross_s=round(diag / 5.0, 1), five_cross_s=round(diag / 20.0, 1),
            tris=tris, objects=len(meshes), colliders=colliders, area_core=round(area_core),
            lightmap_cm_2048=round(texel_cm, 1), lightmap_cm_4096=round(texel_cm / 2, 1),
            glb_kb=round(os.path.getsize(glb) / 1024), build_s=round(t_build, 1))

        if render:
            self._render(out_dir, meta, quick)
        meta["stats"]["total_s"] = round(time.time() - self.t_start, 1)
        with open(os.path.join(out_dir, f"{self.key}.json"), "w") as fh:
            json.dump(meta, fh, indent=1)
        print(f"[{self.key}] done in {time.time() - t0:.1f}s: {tris} tris, {len(meshes)} objects")

    # ---------- rendering (render-only additions from here on) ----------
    def _grid_materials(self):
        for key, m in self.mats.items():
            nt = m.node_tree
            bsdf = nt.nodes["Principled BSDF"]
            geo = nt.nodes.new("ShaderNodeNewGeometry")
            sp, sn = nt.nodes.new("ShaderNodeSeparateXYZ"), nt.nodes.new("ShaderNodeSeparateXYZ")
            nt.links.new(geo.outputs["Position"], sp.inputs[0])
            nt.links.new(geo.outputs["Normal"], sn.inputs[0])

            def M(op, a, b=None):
                n = nt.nodes.new("ShaderNodeMath"); n.operation = op
                for i, x in enumerate((a, b)):
                    if x is None:
                        continue
                    if isinstance(x, (int, float)):
                        n.inputs[i].default_value = x
                    else:
                        nt.links.new(x, n.inputs[i])
                return n.outputs[0]

            def lines(scale, width):
                acc = None
                for ax in range(3):
                    p = M("MULTIPLY", sp.outputs[ax], 1.0 / scale)
                    dist = M("ABSOLUTE", M("SUBTRACT", M("FRACT", M("ADD", p, 0.5003)), 0.5))
                    on = M("LESS_THAN", dist, width / scale)
                    mask = M("LESS_THAN", M("ABSOLUTE", sn.outputs[ax]), 0.9)
                    term = M("MULTIPLY", on, mask)
                    acc = term if acc is None else M("MAXIMUM", acc, term)
                return acc

            l1, l5 = lines(1.0, 0.025), lines(5.0, 0.06)
            fac = M("SUBTRACT", M("SUBTRACT", 1.0, M("MULTIPLY", l1, 0.22)), M("MULTIPLY", l5, 0.22))
            vm = nt.nodes.new("ShaderNodeVectorMath"); vm.operation = "SCALE"
            vm.inputs[0].default_value = COLORS[key]
            nt.links.new(fac, vm.inputs["Scale"])
            nt.links.new(vm.outputs[0], bsdf.inputs["Base Color"])

    def _setup_render(self, quick):
        sc = self.scene
        sc.render.engine = "BLENDER_EEVEE"
        ee = sc.eevee
        for attr, val in (("taa_render_samples", 16 if quick else 48), ("use_raytracing", True),
                          ("use_shadows", True), ("shadow_ray_count", 2), ("shadow_step_count", 8),
                          ("use_fast_gi", True), ("fast_gi_distance", 6.0)):
            try:
                setattr(ee, attr, val)
            except Exception:
                pass
        try:
            ee.ray_tracing_options.resolution_scale = "2"
        except Exception:
            pass
        sc.view_settings.view_transform = "AgX"
        try:
            sc.view_settings.look = "AgX - Medium High Contrast"
        except Exception:
            pass
        sc.view_settings.exposure = 0.0
        sc.render.image_settings.file_format = "JPEG"
        sc.render.image_settings.quality = 86
        # Sky: a simple gradient, horizon to zenith.
        w = bpy.data.worlds.new("sky"); sc.world = w
        nt = w.node_tree
        bg = nt.nodes["Background"]
        tc = nt.nodes.new("ShaderNodeTexCoord"); sx = nt.nodes.new("ShaderNodeSeparateXYZ")
        ramp = nt.nodes.new("ShaderNodeValToRGB")
        nt.links.new(tc.outputs["Generated"], sx.inputs[0]); nt.links.new(sx.outputs[2], ramp.inputs[0])
        ramp.color_ramp.elements[0].position = 0.0; ramp.color_ramp.elements[0].color = (0.62, 0.66, 0.70, 1)
        ramp.color_ramp.elements[1].position = 0.6; ramp.color_ramp.elements[1].color = (0.20, 0.36, 0.75, 1)
        nt.links.new(ramp.outputs[0], bg.inputs["Color"])
        bg.inputs["Strength"].default_value = 1.0
        # The camera sees the blue gradient; surfaces are lit by a softer, near-neutral sky.
        amb = nt.nodes.new("ShaderNodeBackground")
        amb.inputs["Color"].default_value = (0.62, 0.66, 0.74, 1)
        amb.inputs["Strength"].default_value = 0.75
        lp = nt.nodes.new("ShaderNodeLightPath")
        mix = nt.nodes.new("ShaderNodeMixShader")
        nt.links.new(lp.outputs["Is Camera Ray"], mix.inputs[0])
        nt.links.new(amb.outputs[0], mix.inputs[1])
        nt.links.new(bg.outputs[0], mix.inputs[2])
        nt.links.new(mix.outputs[0], nt.nodes["World Output"].inputs["Surface"])
        sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
        sc.collection.objects.link(sun)
        sun.data.energy = 2.4
        sun.data.angle = _rad(1.5)
        sun.rotation_euler = (_rad(40), 0, _rad(-35))   # light from the south-south-east, 50 deg up
        dg = bpy.context.evaluated_depsgraph_get()
        for (x, y, z) in self.figures:   # 1.8 m human silhouettes for scale (render only)
            hit, loc, *_ = sc.ray_cast(dg, Vector((x, y, z + 0.6)), Vector((0, 0, -1)), distance=10.0)
            self._figure(x, y, loc.z if hit else z)

    def _figure(self, x, y, z):
        m = bpy.data.materials.get("scale_figure") or bpy.data.materials.new("scale_figure")
        m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.9, 0.9, 0.92, 1)
        for nm, size, c in (("legs", (0.32, 0.2, 0.85), 0.425), ("body", (0.44, 0.24, 0.65), 1.2), ("head", (0.2, 0.2, 0.24), 1.68)):
            v, f = self._box_mesh(*size)
            me = bpy.data.meshes.new("fig_" + nm); me.from_pydata(v, [], f); me.materials.append(m)
            ob = bpy.data.objects.new("fig_" + nm, me); ob.location = (x, y, z + c)
            self.scene.collection.objects.link(ob)

    def _camera(self, name):
        cam = bpy.data.objects.new(name, bpy.data.cameras.new(name))
        self.scene.collection.objects.link(cam)
        self.scene.camera = cam
        return cam

    def _project(self, cam, dg):
        out = []
        for f in self.features:
            p = world_to_camera_view(self.scene, cam, Vector(f["pos"]))
            if 0 <= p.x <= 1 and 0 <= p.y <= 1 and p.z > 0:
                out.append(dict(kind="feature", n=f["n"], x=round(p.x, 4), y=round(1 - p.y, 4)))
        for s in self.spots_meta:
            p = world_to_camera_view(self.scene, cam, Vector((s["x"], s["y"], s["z"])))
            if 0 <= p.x <= 1 and 0 <= p.y <= 1 and p.z > 0:
                out.append(dict(kind="spot", n=s["id"], x=round(p.x, 4), y=round(1 - p.y, 4)))
        return out

    def _shoot(self, out_dir, name, res):
        sc = self.scene
        sc.render.resolution_x, sc.render.resolution_y = res
        sc.render.resolution_percentage = 100
        rel = f"img/{self.key}_{name}.jpg"
        sc.render.filepath = os.path.join(out_dir, rel)
        t = time.time()
        bpy.ops.render.render(write_still=True)
        print(f"  render {name}: {time.time() - t:.1f}s")
        return rel

    def _render(self, out_dir, meta, quick):
        self.spots_meta = meta["spots"]
        self._grid_materials()
        self._setup_render(quick)
        dg = bpy.context.evaluated_depsgraph_get()
        shots = []
        k = 0.5 if quick else 1.0
        cx0, cy0, cx1, cy1 = self.core
        cx, cy = (cx0 + cx1) / 2, (cy0 + cy1) / 2
        W, D = cx1 - cx0, cy1 - cy0

        # Plans: orthographic, straight down. A `cut` height puts the camera inside the Map,
        # which slices off everything above it (a floor plan).
        for pl in self.plans:
            cam = self._camera(pl["name"])
            cam.data.type = "ORTHO"
            res = (int(1600 * k), int(1100 * k))
            aspect = res[0] / res[1]
            cam.data.ortho_scale = max(W * 1.08, D * 1.08 * aspect)
            cam.location = (cx, cy, pl["cut"] if pl["cut"] is not None else 300.0)
            cam.rotation_euler = (0, 0, 0)
            cam.data.clip_start = 0.01 if pl["cut"] is not None else 1.0
            cam.data.clip_end = 1000.0
            for g in pl.get("hide", ()):
                for ob in self.groups.get(g, []):
                    ob.hide_render = True
            rel = self._shoot(out_dir, pl["name"], res)
            for g in pl.get("hide", ()):
                for ob in self.groups.get(g, []):
                    ob.hide_render = False
            shots.append(dict(kind="plan", name=pl["name"], caption=pl["caption"], img=rel,
                              m_per_px_full=cam.data.ortho_scale / res[0], width_m=cam.data.ortho_scale,
                              markers=self._project(cam, dg)))

        # Three-quarter views
        views = [dict(dict(caption="Three-quarter view"), **self.three_quarter, name="three_quarter")] + self.extra_views
        for tq in views:
            cam = self._camera(tq["name"])
            cam.data.lens = tq.get("lens", 28.0)
            res = (int(1600 * k), int(900 * k))
            tgt = Vector(tq["target"]) if tq.get("target") else Vector((cx, cy, 2.0))
            R = 0.5 * math.hypot(W, D)
            dist = tq.get("dist") or R * 2.3
            az, el = _rad(tq.get("az", 215.0)), _rad(tq.get("elev", 32.0))
            cam.location = tgt + dist * Vector((math.sin(az) * math.cos(el), math.cos(az) * math.cos(el), math.sin(el)))
            cam.rotation_mode = "QUATERNION"
            cam.rotation_quaternion = (tgt - cam.location).to_track_quat("-Z", "Y")
            cam.data.clip_end = 2000.0
            for g in tq.get("hide", ()):
                for ob in self.groups.get(g, []):
                    ob.hide_render = True
            rel = self._shoot(out_dir, tq["name"], res)
            dg = bpy.context.evaluated_depsgraph_get()
            shots.append(dict(kind="view", name=tq["name"], caption=tq["caption"], img=rel, markers=self._project(cam, dg)))
            for g in tq.get("hide", ()):
                for ob in self.groups.get(g, []):
                    ob.hide_render = False

        # FPV: landed at every Launch Spot candidate, from both Quads.
        def fpv(name, pos, heading, pitch):
            cam = self._camera(name)
            cam.data.sensor_fit = "HORIZONTAL"
            cam.data.angle = _rad(FPV_HFOV)
            cam.data.clip_start = 0.005
            cam.data.clip_end = 2000.0
            cam.location = pos
            cam.rotation_euler = (_rad(90 + pitch), 0, _rad(-heading))
            return self._shoot(out_dir, name, (int(1280 * k), int(720 * k)))

        for s in self.spots_meta:
            for q, qd in QUADS.items():
                nm = f"fpv_{s['id']}_{q}"
                rel = fpv(nm, (s["x"], s["y"], s["z"] + qd["cam_h"]), s["heading"], qd["tilt"])
                shots.append(dict(kind="landed", name=nm, spot=s["id"], quad=q, img=rel,
                                  caption=f"{qd['label']} landed on {s['id']}, camera {qd['cam_h'] * 100:.1f} cm up, "
                                          f"Camera Tilt {qd['tilt']:.0f}°, facing {s['heading']:.0f}°"))
        for sh in self.shots:
            rel = fpv("fly_" + sh["name"], sh["pos"], sh["heading"], sh["pitch"])
            shots.append(dict(kind="flight", name=sh["name"], quad=sh["quad"], img=rel, caption=sh["caption"]))
        meta["shots"] = shots
