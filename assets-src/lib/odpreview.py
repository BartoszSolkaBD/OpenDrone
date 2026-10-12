"""Preview pictures of a Map, for people (MIT OR Apache-2.0).

Run after `MapBuilder.export`, in the same Blender: it adds a sky, a sun, a
metre grid on every surface, scale figures and cameras, then renders with
Cycles on the processor, because CI has no graphics card. Nothing it adds
reaches the `.glb`. The pictures are for the Review Report, the Hub's Map
picker and the pilot guide; they are not part of the "same bytes" rule.

Pictures, in the folder it is given:

- `plan.png`: straight down from above;
- `three-quarter.png`: the whole park from the south-south-west, high up;
- `fpv-<quad>-landed.png`: what each Quad's camera sees landed on the Launch
  Spot, 30 degrees of Camera Tilt;
- `fpv-<flight>.png`: a few places a pilot will fly through.

The FPV pictures use a plain 110 degree view, wider than a person's but not
the Quad's fisheye Lens, which only the game draws.
"""
import math
import os

import bpy
from mathutils import Vector

FPV_HFOV = 110.0
CAMERA_TILT = 30.0
SAMPLES = 24


def _setup(scene):
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = SAMPLES
    scene.cycles.use_denoising = True
    scene.cycles.denoiser = "OPENIMAGEDENOISE"
    scene.cycles.max_bounces = 4
    scene.view_settings.view_transform = "AgX"
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    world = bpy.data.worlds.new("sky")
    scene.world = world
    nt = world.node_tree
    bg = nt.nodes["Background"]
    coords = nt.nodes.new("ShaderNodeTexCoord")
    split = nt.nodes.new("ShaderNodeSeparateXYZ")
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    nt.links.new(coords.outputs["Generated"], split.inputs[0])
    nt.links.new(split.outputs[2], ramp.inputs[0])
    ramp.color_ramp.elements[0].position = 0.0
    ramp.color_ramp.elements[0].color = (0.62, 0.66, 0.70, 1)
    ramp.color_ramp.elements[1].position = 0.6
    ramp.color_ramp.elements[1].color = (0.20, 0.36, 0.75, 1)
    nt.links.new(ramp.outputs[0], bg.inputs["Color"])
    bg.inputs["Strength"].default_value = 1.0
    # Surfaces are lit by a softer, near-neutral sky than the one the camera sees.
    ambient = nt.nodes.new("ShaderNodeBackground")
    ambient.inputs["Color"].default_value = (0.62, 0.66, 0.74, 1)
    ambient.inputs["Strength"].default_value = 0.75
    path = nt.nodes.new("ShaderNodeLightPath")
    mix = nt.nodes.new("ShaderNodeMixShader")
    nt.links.new(path.outputs["Is Camera Ray"], mix.inputs[0])
    nt.links.new(ambient.outputs[0], mix.inputs[1])
    nt.links.new(bg.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], nt.nodes["World Output"].inputs["Surface"])
    # Sunny early afternoon: the sun about 50 degrees up, to the south-south-east.
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    scene.collection.objects.link(sun)
    sun.data.energy = 3.2
    sun.data.angle = math.radians(1.5)
    sun.rotation_euler = (math.radians(40), 0, math.radians(-35))


def _grid_on_materials(builder):
    """A faint line every metre, and a stronger one every 5 m, over every
    surface, so a picture shows how big things are."""
    for name, material in builder.materials.items():
        nt = material.node_tree
        bsdf = nt.nodes["Principled BSDF"]
        colour = builder.library[name]["colour"]
        geo = nt.nodes.new("ShaderNodeNewGeometry")
        position = nt.nodes.new("ShaderNodeSeparateXYZ")
        normal = nt.nodes.new("ShaderNodeSeparateXYZ")
        nt.links.new(geo.outputs["Position"], position.inputs[0])
        nt.links.new(geo.outputs["Normal"], normal.inputs[0])

        def maths(op, a, b=None):
            node = nt.nodes.new("ShaderNodeMath")
            node.operation = op
            for i, x in enumerate((a, b)):
                if x is None:
                    continue
                if isinstance(x, (int, float)):
                    node.inputs[i].default_value = x
                else:
                    nt.links.new(x, node.inputs[i])
            return node.outputs[0]

        def lines(scale, width):
            total = None
            for axis in range(3):
                p = maths("MULTIPLY", position.outputs[axis], 1.0 / scale)
                dist = maths("ABSOLUTE", maths("SUBTRACT", maths("FRACT", maths("ADD", p, 0.5003)), 0.5))
                on = maths("LESS_THAN", dist, width / scale)
                facing = maths("LESS_THAN", maths("ABSOLUTE", normal.outputs[axis]), 0.9)
                term = maths("MULTIPLY", on, facing)
                total = term if total is None else maths("MAXIMUM", total, term)
            return total

        fine, coarse = lines(1.0, 0.025), lines(5.0, 0.06)
        fac = maths("SUBTRACT", maths("SUBTRACT", 1.0, maths("MULTIPLY", fine, 0.22)), maths("MULTIPLY", coarse, 0.22))
        scale = nt.nodes.new("ShaderNodeVectorMath")
        scale.operation = "SCALE"
        scale.inputs[0].default_value = colour
        nt.links.new(fac, scale.inputs["Scale"])
        nt.links.new(scale.outputs[0], bsdf.inputs["Base Color"])


def _figure(scene, x, y, z):
    """A 1.8 m person, for scale."""
    material = bpy.data.materials.new("scale_figure")
    material.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.9, 0.9, 0.92, 1)
    for name, size, height in (("legs", (0.32, 0.2, 0.85), 0.425), ("body", (0.44, 0.24, 0.65), 1.2), ("head", (0.2, 0.2, 0.24), 1.68)):
        hx, hy, hz = size[0] / 2, size[1] / 2, size[2] / 2
        corners = [(sx * hx, sy * hy, sz * hz) for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)]
        faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        mesh = bpy.data.meshes.new("figure_" + name)
        mesh.from_pydata(corners, [], faces)
        mesh.materials.append(material)
        ob = bpy.data.objects.new("figure_" + name, mesh)
        ob.location = (x, y, z + height)
        scene.collection.objects.link(ob)


def _camera(scene, name):
    cam = bpy.data.objects.new(name, bpy.data.cameras.new(name))
    scene.collection.objects.link(cam)
    scene.camera = cam
    return cam


def _shoot(scene, folder, name, size):
    scene.render.resolution_x, scene.render.resolution_y = size
    scene.render.resolution_percentage = 100
    scene.render.filepath = os.path.join(folder, name + ".png")
    bpy.ops.render.render(write_still=True)


def render(builder, preview, launch, folder):
    os.makedirs(folder, exist_ok=True)
    scene = builder.scene
    _grid_on_materials(builder)
    _setup(scene)
    for x, y, z in preview.get("figures", ()):
        _figure(scene, x, y, z)
    x0, y0, x1, y1 = preview["core"]
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    width, depth = x1 - x0, y1 - y0

    cam = _camera(scene, "plan")
    cam.data.type = "ORTHO"
    size = (1280, 880)
    cam.data.ortho_scale = max(width * 1.08, depth * 1.08 * size[0] / size[1])
    cam.location = (cx, cy, 300.0)
    cam.rotation_euler = (0, 0, 0)
    cam.data.clip_start, cam.data.clip_end = 1.0, 1000.0
    _shoot(scene, folder, "plan", size)

    tq = preview["three_quarter"]
    cam = _camera(scene, "three-quarter")
    cam.data.lens = 28.0
    target = Vector(tq["target"])
    az, el = math.radians(tq["azimuth"]), math.radians(tq["elevation"])
    cam.location = target + tq["distance"] * Vector((math.sin(az) * math.cos(el), math.cos(az) * math.cos(el), math.sin(el)))
    cam.rotation_mode = "QUATERNION"
    cam.rotation_quaternion = (target - cam.location).to_track_quat("-Z", "Y")
    cam.data.clip_end = 3000.0
    _shoot(scene, folder, "three-quarter", (1280, 720))

    def fpv(name, position, heading, pitch):
        cam = _camera(scene, name)
        cam.data.sensor_fit = "HORIZONTAL"
        cam.data.angle = math.radians(FPV_HFOV)
        cam.data.clip_start, cam.data.clip_end = 0.005, 3000.0
        cam.location = position
        cam.rotation_euler = (math.radians(90 + pitch), 0, math.radians(-heading))
        _shoot(scene, folder, name, (960, 720))

    lx, ly, lz, heading = launch
    for quad, height in preview["landed"]:
        fpv(f"fpv-{quad}-landed", (lx, ly, lz + height), heading, CAMERA_TILT)
    for name, position, heading, pitch in preview["flights"]:
        fpv("fpv-" + name, position, heading, pitch)
