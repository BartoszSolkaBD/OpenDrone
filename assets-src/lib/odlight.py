"""Baked light for a Map: the second UV set, and the Cycles bake (MIT OR Apache-2.0).

Two halves, so that the part that reaches the `.glb` can't be touched by
Blender's own maths:

- **The unwrap** (`unwrap`, plain Python, no Blender). It cuts each mesh into
  flat charts, scales each chart to the Map's texel density and packs every
  chart of every mesh into one atlas. The result is a list of picture
  coordinates, which goes into the `.glb` as `TEXCOORD_1`. It is written in
  64-bit Python, like `odgeom`, so the `.glb` is the same bytes on every
  computer. Blender's `smart_project` and `pack_islands` are never used: their
  output order isn't kept from run to run (docs/research/asset-pipeline.md).
- **The bake** (`bake`, Blender's Cycles on the processor). It bakes the Map's
  sun and sky light, with bounces and ambient occlusion, into an image on that
  second UV set. A bake may differ a little from run to run, so the atlas is a
  committed build output and CI never rebuilds it.

Units are metres in the Map and centimetres per texel for the density. The
atlas's first row is the bottom of the picture (v = 0), as Blender's own.
"""
import hashlib
import math
import re
import time

# A chart takes in a triangle only when its face points within 20 degrees of
# the chart's first triangle: the flattening then stays within 6% of true size
# and a connected patch can't fold over itself.
CHART_COS = math.cos(math.radians(20.0))

MARGIN = 2  # empty texels round every chart, which the bake fills outwards
DEFAULT_SAMPLES = 512  # Cycles samples per texel: 128 is a quick look, 1024 is clean
DEFAULT_MAX_CHART = 512  # most texels one chart may span along a side
DEFAULT_MAX_SIZE = 8192  # biggest atlas side, in texels

# The values of `[light]` in a Map's `map.toml`, which the Pack checker reads too.
LIGHT_KEYS = ("sun_elevation", "sun_azimuth", "sun_colour", "sun_strength", "sky_colour", "sky_strength")


# ---------- the unwrap: plain Python ----------
def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def _unit(a):
    n = math.sqrt(_dot(a, a))
    return (a[0] / n, a[1] / n, a[2] / n)


def _triangle_normal(corners, tri):
    a, b, c = (corners[i] for i in tri)
    return _cross(_sub(b, a), _sub(c, a))


def _frame(n):
    """Two flat axes `(u, v)` for a face pointing along the unit `n`: `v` is
    up for a wall, and `u` is east for a floor or a ceiling, so a chart of a
    building's face is a tight upright rectangle."""
    if abs(n[2]) >= 0.9:
        x = (1.0, 0.0, 0.0)
        k = _dot(x, n)
        u = _unit((x[0] - n[0] * k, x[1] - n[1] * k, x[2] - n[2] * k))
        v = _cross(n, u)
    else:
        z = (0.0, 0.0, 1.0)
        k = _dot(z, n)
        v = _unit((z[0] - n[0] * k, z[1] - n[1] * k, z[2] - n[2] * k))
        u = _cross(v, n)
    return u, v


def charts_of(corners, triangles):
    """Cuts a mesh into charts: lists of triangle numbers that share edges and
    face nearly the same way. Fixed order: a chart starts at the lowest
    triangle not yet taken and grows through its neighbours in number order."""
    normals = []
    for tri in triangles:
        n = _triangle_normal(corners, tri)
        normals.append(_unit(n))
    by_edge = {}
    for t, (a, b, c) in enumerate(triangles):
        for e in ((a, b), (b, c), (c, a)):
            by_edge.setdefault((min(e), max(e)), []).append(t)
    neighbours = [[] for _ in triangles]
    for e in sorted(by_edge):
        sharing = by_edge[e]
        for t in sharing:
            for s in sharing:
                if s != t:
                    neighbours[t].append(s)
    for row in neighbours:
        row.sort()
    taken = [False] * len(triangles)
    charts = []
    for seed in range(len(triangles)):
        if taken[seed]:
            continue
        taken[seed] = True
        chart = [seed]
        queue = [seed]
        head = 0
        while head < len(queue):
            t = queue[head]
            head += 1
            for s in neighbours[t]:
                if not taken[s] and _dot(normals[s], normals[seed]) >= CHART_COS:
                    taken[s] = True
                    chart.append(s)
                    queue.append(s)
        charts.append((sorted(chart), normals[seed]))
    return charts


class Chart:
    """One flat piece of a mesh, sized in texels, waiting for a place."""

    __slots__ = ("mesh", "triangles", "corners_uv", "w", "h", "x", "y", "scale")

    def __init__(self, mesh, triangles, corners_uv, w, h, scale):
        self.mesh = mesh
        self.triangles = triangles
        self.corners_uv = corners_uv  # {corner number: (u, v) in texels from the chart's own corner}
        self.w = w  # texels, margin included
        self.h = h
        self.x = 0
        self.y = 0
        self.scale = scale  # texels per metre


def flatten(meshes, texel_cm, margin=MARGIN, max_chart=DEFAULT_MAX_CHART):
    """Every mesh's charts, flattened and sized. `meshes` is a list of
    `(corners, triangles, cm)`, with `cm` the mesh's own centimetres per texel
    or None for the Map's `texel_cm`. A chart wider than `max_chart` texels is
    scaled down to it, so a 2 km ground costs the same as a 50 m floor.

    A mesh may carry a fourth number, `least` (#339): each of its charts is
    then stretched to at least that many texels each way. The bake lights a
    texel only where a triangle covers its middle, so a chart smaller than a
    texel (a brick's face at 20 cm a texel) would be left black; the kit's
    small things ask for 2."""
    out = []
    for m, mesh in enumerate(meshes):
        corners, triangles, cm = mesh[:3]
        least = mesh[3] if len(mesh) > 3 else None
        per_metre = 100.0 / (cm if cm is not None else texel_cm)
        for chart, normal in charts_of(corners, triangles):
            u, v = _frame(normal)
            used = sorted({i for t in chart for i in triangles[t]})
            flat = {i: (_dot(corners[i], u), _dot(corners[i], v)) for i in used}
            u0 = min(p[0] for p in flat.values())
            v0 = min(p[1] for p in flat.values())
            wm = max(p[0] for p in flat.values()) - u0
            hm = max(p[1] for p in flat.values()) - v0
            scale = min(per_metre, max_chart / max(wm, hm, 1e-9))
            su = sv = scale
            if least:
                su, sv = max(scale, least / max(wm, 1e-9)), max(scale, least / max(hm, 1e-9))
            w = max(1, math.ceil(wm * su - 1e-9))
            h = max(1, math.ceil(hm * sv - 1e-9))
            local = {i: ((p[0] - u0) * su, (p[1] - v0) * sv) for i, p in flat.items()}
            out.append(Chart(m, chart, local, w + 2 * margin, h + 2 * margin, scale))
    return out


def _shelves(charts, width):
    """Places the charts, tall ones first, in rows across `width`. Returns the
    height used, or None if a chart is wider than `width`. Sets each chart's
    `x` and `y`."""
    order = sorted(range(len(charts)), key=lambda i: (-charts[i].h, -charts[i].w, charts[i].mesh, i))
    x = y = row = 0
    for i in order:
        c = charts[i]
        if c.w > width:
            return None
        if x + c.w > width:
            y += row
            x = row = 0
        c.x, c.y = x, y
        x += c.w
        row = max(row, c.h)
    return y + row


def pack(charts, max_size=DEFAULT_MAX_SIZE):
    """Chooses the atlas's width and height and places every chart. The width
    is a power of two and the height the least that holds the rows (a multiple
    of 4); of the widths that fit, the least area wins, the narrower first.
    Returns `(width, height)`."""
    best = None
    width = 64
    while width <= max_size:
        height = _shelves(charts, width)
        if height is not None:
            height = (height + 3) // 4 * 4
            if height <= max_size and (best is None or width * height < best[0] * best[1]):
                best = (width, height)
        width *= 2
    if best is None:
        used = sum(c.w * c.h for c in charts)
        raise ValueError(
            f"the light atlas doesn't fit in {max_size} x {max_size} texels: its charts need about {used} texels. "
            "Raise `texel_cm` (fewer, bigger texels), or give the big far-off parts a `light_cm` of their own."
        )
    _shelves(charts, best[0])
    return best


def unwrap(meshes, texel_cm, margin=MARGIN, max_chart=DEFAULT_MAX_CHART, max_size=DEFAULT_MAX_SIZE):
    """The second UV set of every mesh, from `meshes` as `flatten` takes them.
    Returns `(uvs, (width, height), info)`. `uvs[m]` is a flat list of
    `u, v` per corner of each triangle of mesh `m`, in the order of its
    triangles."""
    assert texel_cm > 0.0, "texel_cm is centimetres per texel, above zero"
    charts = flatten(meshes, texel_cm, margin, max_chart)
    width, height = pack(charts, max_size)
    uvs = [[0.0] * (6 * len(mesh[1])) for mesh in meshes]
    for c in charts:
        for t in c.triangles:
            tri = meshes[c.mesh][1][t]
            for k, i in enumerate(tri):
                px, py = c.corners_uv[i]
                uvs[c.mesh][6 * t + 2 * k] = round((c.x + margin + px) / width * 1e6) / 1e6 + 0.0
                uvs[c.mesh][6 * t + 2 * k + 1] = round((c.y + margin + py) / height * 1e6) / 1e6 + 0.0
    used = sum(c.w * c.h for c in charts)
    info = {"charts": len(charts), "texels_in_charts": used, "fill": used / (width * height)}
    return uvs, (width, height), info


# ---------- the Map's light values ----------
def _number(text, what):
    if not re.fullmatch(r"\d+(\.\d+)?", text.strip()):
        raise ValueError(f"[light] {what} is a plain number such as \"3.0\", not {text!r}")
    return float(text)


def _angle(text, what):
    m = re.fullmatch(r"\s*(-?\d+(?:\.\d+)?)\s*(?:°|deg)\s*", text)
    if not m:
        raise ValueError(f"[light] {what} is an angle in degrees such as \"37 °\", not {text!r}")
    return float(m.group(1))


def _srgb_to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def _colour(text, what):
    m = re.fullmatch(r"#([0-9a-fA-F]{6})", text.strip())
    if not m:
        raise ValueError(f"[light] {what} is a colour such as \"#fff2d6\" (sRGB, as in a paint program), not {text!r}")
    h = m.group(1)
    return tuple(_srgb_to_linear(int(h[i : i + 2], 16) / 255.0) for i in (0, 2, 4))


def read_light(table):
    """The `[light]` table of a `map.toml`, checked and in plain numbers: the
    sun's elevation and compass azimuth in degrees, and the colours as linear
    red, green and blue."""
    missing = [k for k in LIGHT_KEYS if k not in table]
    if missing:
        raise ValueError("[light] needs " + ", ".join(missing))
    unknown = [k for k in table if k not in LIGHT_KEYS]
    if unknown:
        raise ValueError("[light] doesn't know " + ", ".join(unknown))
    light = {
        "sun_elevation": _angle(table["sun_elevation"], "sun_elevation"),
        "sun_azimuth": _angle(table["sun_azimuth"], "sun_azimuth"),
        "sun_colour": _colour(table["sun_colour"], "sun_colour"),
        "sun_strength": _number(table["sun_strength"], "sun_strength"),
        "sky_colour": _colour(table["sky_colour"], "sky_colour"),
        "sky_strength": _number(table["sky_strength"], "sky_strength"),
    }
    if not 0.0 <= light["sun_elevation"] <= 90.0:
        raise ValueError("[light] sun_elevation is from 0 ° (on the horizon) to 90 ° (overhead)")
    if not 0.0 <= light["sun_azimuth"] <= 360.0:
        raise ValueError("[light] sun_azimuth is a compass direction from 0 ° to 360 °")
    return light


def light_key(table):
    """What the bake was made from, as one fingerprint: the `[light]` values as
    written, in key order. A different sun, or colour, is another bake."""
    text = "".join(f"{k} = {table[k]}\n" for k in sorted(table))
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def sun_direction(elevation, azimuth):
    """The unit vector from the Map towards the sun: east is +x, north is +y."""
    el, az = math.radians(elevation), math.radians(azimuth)
    return (math.sin(az) * math.cos(el), math.cos(az) * math.cos(el), math.sin(el))


# ---------- the bake: Blender ----------
# A white surface square-on to a sun of strength S, in Blender's units, bakes
# to S / PI (measured, tests in docs/building-a-map.md); the Map's own
# `sun_strength` is that value, and the sky's `sky_strength` is what a white
# surface under an open sky bakes to.
BLENDER_SUN = math.pi


def bake(meshes, light, atlas_path, size, samples=DEFAULT_SAMPLES, margin=MARGIN + 2, bounces=6):
    """Bakes sun and sky light into `atlas_path` (an OpenEXR picture, half
    floats, no alpha). `meshes` are the Blender mesh objects to bake, each with
    its second UV set active. Returns what happened, for the sidecar file."""
    import bpy
    from mathutils import Vector

    started = time.time()
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    cy = scene.cycles
    cy.device = "CPU"
    cy.samples = samples
    cy.use_denoising = False  # Cycles doesn't denoise a bake
    cy.max_bounces = bounces
    cy.diffuse_bounces = bounces
    cy.glossy_bounces = 0
    cy.transmission_bounces = 0
    cy.transparent_max_bounces = 0
    cy.sample_clamp_indirect = 20.0
    cy.use_adaptive_sampling = False
    cy.seed = 1
    bake_settings = scene.render.bake
    bake_settings.margin = margin
    bake_settings.margin_type = "EXTEND"
    bake_settings.use_clear = True

    # The sky: one even colour all round. The sun: a Cycles sun the width of
    # the real one (half a degree), so shadow edges are soft the way they are.
    world = bpy.data.worlds.new("od_light")
    world.use_nodes = True
    background = world.node_tree.nodes["Background"]
    sky = light["sky_colour"]
    background.inputs["Color"].default_value = (sky[0], sky[1], sky[2], 1.0)
    background.inputs["Strength"].default_value = light["sky_strength"]
    scene.world = world
    sun = bpy.data.lights.new("od_sun", "SUN")
    sun.color = light["sun_colour"]
    sun.energy = light["sun_strength"] * BLENDER_SUN
    sun.angle = math.radians(0.53)
    sun_ob = bpy.data.objects.new("od_sun", sun)
    scene.collection.objects.link(sun_ob)
    towards = Vector(sun_direction(light["sun_elevation"], light["sun_azimuth"]))
    sun_ob.rotation_euler = towards.to_track_quat("Z", "Y").to_euler()

    # The atlas, and an Image Texture node in every material to bake into.
    image = bpy.data.images.new("od_light_atlas", size[0], size[1], alpha=False, float_buffer=True)
    image.colorspace_settings.name = "Non-Color"
    image.generated_color = (0.0, 0.0, 0.0, 1.0)
    for ob in meshes:
        for slot in ob.material_slots:
            tree = slot.material.node_tree
            node = tree.nodes.get("od_light_atlas")
            if node is None:
                node = tree.nodes.new("ShaderNodeTexImage")
                node.name = "od_light_atlas"
            node.image = image
            tree.nodes.active = node
            node.select = True
    # One object to bake: Cycles sets the scene up again for every object it
    # bakes, which takes a second or so each, so copies of the Map's hundreds
    # of shapes are joined into one, and the shapes themselves stay out of
    # the render.
    bpy.ops.object.select_all(action="DESELECT")
    copies = []
    for ob in meshes:
        copy = ob.copy()
        copy.data = ob.data.copy()
        copy.data.uv_layers.active_index = 1
        scene.collection.objects.link(copy)
        copy.select_set(True)
        copies.append(copy)
        ob.hide_render = True
    bpy.context.view_layer.objects.active = copies[0]
    bpy.ops.object.join()
    bpy.ops.object.bake(type="DIFFUSE", pass_filter={"DIRECT", "INDIRECT"}, margin=margin, margin_type="EXTEND", use_clear=True, uv_layer="UVLight")

    scene.render.image_settings.file_format = "OPEN_EXR"
    scene.render.image_settings.color_depth = "16"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.image_settings.exr_codec = "ZIP"
    image.save_render(atlas_path, scene=scene)
    return {"seconds": round(time.time() - started, 1), "samples": samples, "bounces": bounces}

