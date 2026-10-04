# Agent-driven asset pipeline: Blender scripts, procedural Rust and glTF in Bevy

Research for [#8](https://github.com/BartoszSolkaBD/OpenDrone/issues/8), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1).
Checked on 2026-10-03 against Blender **5.2.2 LTS** (bundled glTF exporter "Khronos glTF Blender I/O v5.2.40"), Bevy **0.19.1** (latest stable; 0.20 is at rc.2), Avian 0.7.0, parry3d 0.31.1 and bevy_skein 0.6.0. The claims marked **measured** come from throwaway probes run on the dev machine (Apple M4) on the same day.

## The answer in plain language

**Recommended pipeline: Blender as a build tool, with glTF as the only hand-off into the game.**

1. **Every Map and every Quad's look is a Python script in the repo**, for example `assets-src/maps/skate_park/build.py`. The script is the source of truth. The 3D file it produces is a build output, like a compiled program.
2. **Blender 5.2 LTS runs the script with no window.** The script builds the shapes and lays out two sets of texture coordinates: one for repeating surface textures, one for baked lighting. It also tags which parts are solid (colliders) and where pilots spawn, bakes the light and shadow into an image (a lightmap), and exports one `.glb` file (binary glTF, the standard 3D interchange format).
3. **Bevy loads the `.glb`.** A small Map loader in our code reads the tags. It attaches the lightmap, places spawn points, and swaps placeholder materials for our shared library of real textures.
4. **The physics crate reads the same `.glb` without Bevy.** It takes only the parts tagged as colliders, so physics and graphics always agree about where the walls are.
5. **Procedural Rust is a complement, not the main road.** Use it for test Maps in Scenarios (a flat floor, a wall at a known distance), for scattering objects, and later for Mountains terrain. Anything that ships as a Map still goes through the same glTF hand-off.
6. **Textures come from Poly Haven and ambientCG.** Both are CC0: free for any use, with no credit required. A manifest records each file's address and checksum. A build step downloads and verifies them, so large image files never go into git.
7. **No Git LFS for the alpha.** The generated Map geometry is tiny (the test Skate Park blockout is 14 KB). Textures are fetched from the manifest, and large build outputs can go into GitHub Releases, which have no bandwidth limit. A free account gets 10 GiB of LFS downloads a month, and every CI run and every fork's download counts against it.

**What we proved on the dev machine (measured)**

- A blockout script (30 m ground, quarter pipe, ledge, rail) runs in Blender 5.2.2 with no window in **0.8 s**, including Blender start-up. With a 1024×1024 lightmap baked on the CPU it takes **3.3 s**.
- The `.glb` loads in an engine-free Rust probe (the `gltf` crate Bevy itself uses, plus parry3d). The probe reads the collider tags and both texture-coordinate sets, builds colliders, and a ray dropped from 10 m hits the ground at exactly y = 0.
- The same `.glb` loads in **Bevy 0.19.1** (run without a window or GPU). Every tagged node arrived with its tag as a component, every mesh had the second texture-coordinate set, and a `Lightmap` attached to all four meshes.
- **The same script doesn't automatically give byte-identical files.** Blender's `lightmap_pack` tool and its "extrude" geometry operation gave a different but equivalent result on every run. After switching to explicitly listed faces and the `smart_project` unwrap, three runs produced byte-identical files.
- **Bevy refuses compressed meshes.** A `.glb` exported with Draco compression failed to load in Bevy 0.19.1 (and in the engine-free probe) with `"KHR_draco_mesh_compression": Unsupported extension`. Leave mesh compression off.

**Key facts**

- Pin **Blender 5.2 LTS** (supported until July 2028). The pip-installable `bpy` 5.2.2 needs exactly Python 3.13.
- GitHub's standard runners are free and unlimited for public repos, but have **no GPU**. Lighting bakes in CI run on the CPU, so asset jobs should run only when asset sources change.
- Bevy can't bake lightmaps. It applies them per mesh through a `Lightmap` component that our code attaches; nothing loads them from glTF automatically. It reads them through the second texture-coordinate set, which Blender's exporter writes as `TEXCOORD_1`.
- Bevy has no LOD system that reads glTF. Its `VisibilityRange` (distance-based swapping) is attached by code. Skate Park and Bando are small enough that LOD is probably unnecessary.
- **Licences are compatible.** CC0 sources can ship in an MIT/Apache repo. Our `bpy` scripts must be under a GPL-compatible licence, and MIT and Apache-2.0 both are. Blender's output files belong to us. The US Copyright Office says purely machine-determined output isn't copyrightable, so licence the generated assets permissively and don't rely on copyright to protect them.

## 1. Headless Blender: scripts, version pinning, CI

### 1.1 Running a script with no window

Blender's command line (5.2 LTS manual) gives everything an agent-run build needs ([arguments](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html)):

- `-b, --background`: "Run in background (often used for UI-less rendering)."
- `--factory-startup`: "Skip reading the startup.blend in the users home directory." This keeps a developer's personal settings out of the build.
- `-P, --python <filepath>`: "Run the given Python script file."
- `--python-exit-code <code>`: "Set the exit-code in [0..255] to exit if a Python exception is raised (only for scripts executed from the command line), zero disables." Without it, a crashing script still exits with 0 and CI would go green.
- `--`: "End option processing, following arguments passed unchanged. Access via Python's sys.argv."
- `--cycles-device <device>`: "Valid options are: CPU CUDA OPTIX HIP ONEAPI METAL."

The canonical call is:

```sh
blender --background --factory-startup --python-exit-code 1 \
  --python assets-src/maps/skate_park/build.py -- --out assets/maps/skate_park.glb --bake
```

### 1.2 Pinning the Blender version

- **Current versions.** Blender 5.2 LTS was released on 14 July 2026. "It will get fixes for up to two years, until July 2028" ([5.2 LTS](https://www.blender.org/download/lts/5-2/)). The latest patch is 5.2.2, from 15 September 2026 ([LTS page](https://www.blender.org/download/lts/)). 4.5 LTS is also still maintained (4.5.14).
- **Option A, the official build (recommended).** `download.blender.org/release/Blender5.2/` has `blender-5.2.2-linux-x64.tar.xz`, `-macos-arm64.dmg` and `-windows-x64.zip`, plus a `blender-5.2.2.sha256` checksum file ([release directory](https://download.blender.org/release/Blender5.2/)). CI downloads the tarball, checks the SHA-256 and caches it. It's the same program developers run, and every command-line flag works.
- **Option B, the `bpy` Python module.** `pip install bpy==5.2.2` (released 15 September 2026) has wheels for Windows x64 and arm64, Linux x86-64 (glibc 2.28+) and macOS 11+ arm64. It "requires Python 3.13.x exclusively", because "each Blender release supports one Python version, and the package is only compatible with that version" ([PyPI](https://pypi.org/project/bpy/)). The caveats come from Blender's docs: it "behaves as if `--factory-startup` was passed", command-line-only settings such as `--threads` and `--log` aren't accessible, and "a crash log is not written in the event of a crash" ([Blender as a Python module](https://docs.blender.org/api/current/info_advanced_blender_as_bpy.html)). That's fine for unit-testing helper code, but option A is simpler to reason about.
- Keep the version in one file (for example `tools/blender-version`) that CI, the local build command and the toolchain ticket all read. The dev machine already has 5.2.2 installed through Homebrew.

### 1.3 CI runners and run time

- Standard GitHub-hosted runners are "free and unlimited on public repositories". Linux and Windows runners have 4 CPUs and 16 GB of RAM. macOS arm64 runners have "3 (M1)" CPUs and 7 GB, and "nested-virtualization is not supported" ([runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)). GPUs only come with paid "larger runners", so **lighting bakes in CI are CPU-only**. A single job may run for up to 6 hours, and the Actions cache holds 10 GB per repository ([Actions limits](https://docs.github.com/en/actions/reference/limits)).
- **Measured on the M4** (Blender 5.2.2, CPU): building and exporting the 4-object blockout took under 0.2 s inside the script, 0.8 s with start-up. Baking a 1024×1024 lightmap at 64 samples took 3.2 s. A real Map with more geometry, a 2048² atlas and more samples will take minutes, and a 4-CPU Linux runner will be several times slower than the M4. **Estimate, not measured:** expect single-digit minutes per Map in CI.
- Recommended CI shape: one Linux job, triggered only when `assets-src/` or the Blender version file changes. It caches the Blender tarball and downloaded textures, rebuilds the Maps, checks a fingerprint (section 1.4), runs a load test (section 4), and uploads the outputs.

### 1.4 Reproducibility: measured, with a fix

Running the same script twice should give the same file, so CI can tell whether a committed `.glb` is out of date. **Measured** with the blockout probe, all on one machine:

| Variant | Same bytes across runs? | What differed |
|---|---|---|
| `lightmap_pack` for the lightmap UV, `bmesh.ops.extrude_face_region` for the quarter pipe | No | The lightmap UV layout (`TEXCOORD_1`) on every object, and the quarter pipe's triangle order |
| Same, with `PYTHONHASHSEED=0` | No | Same as above |
| `smart_project` for both UV sets, extrude kept | No | Only the quarter pipe's triangle order: the same triangles and winding in a different order (`extrude_face_region` already gives a different polygon order inside Blender) |
| `smart_project` and explicit face lists, no extrude | **Yes, 3 of 3 runs** | Nothing |

What this means:

- The glTF exporter itself was stable. The instability came from two Blender operations whose output order varies between runs: the Python `lightmap_pack` operator and the bmesh extrude.
- Rules for agent-written scripts: build geometry from explicit vertex and face lists, or from operations proven stable; unwrap lightmap UVs with `smart_project` (stable in our test), not `lightmap_pack`; and add a CI check that rebuilds and compares.
- **Not tested:** whether macOS, Linux and Windows produce the same bytes. Until that's proven, compare a canonical fingerprint in CI (positions, sorted triangles, UVs, extras) rather than raw bytes, and build the committed outputs on one platform (Linux CI).
- A UV-export bug in Blender 5.1 ("Missing UVs on export") was closed on 5 April 2026 ([glTF-Blender-IO#2671](https://github.com/KhronosGroup/glTF-Blender-IO/issues/2671)). Another reason to pin 5.2.

### 1.5 Licences for scripts and output

- Output: "What you create with Blender is your sole property. All your artwork – images or movie files – including the .blend files and other data files Blender can write, is free for you to use as you like" ([Blender licence](https://www.blender.org/about/license/)).
- Scripts: "Blender's Python API is an integral part of the software… The GNU GPL license therefore requires that such scripts (if published) are being shared under a GPL compliant license." Blender binaries are distributed under "GNU GPL Version 3 or later" (same page).
- MIT is GPL-compatible, and the Apache Software Foundation states Apache-2.0 is "compatible with version 3 of the GPL" ([ASF](https://www.apache.org/licenses/GPL-compatibility.html)). So our MIT/Apache-2.0 `bpy` scripts are fine. We never ship Blender itself.

## 2. Exporting glTF from Blender

The bundled exporter's settings that matter for us ([manual](https://docs.blender.org/manual/en/latest/addons/scene_gltf2.html), [operator API](https://docs.blender.org/api/current/bpy.ops.export_scene.html)):

- `export_format="GLB"`: one binary file.
- `export_extras=True`: "Export custom properties as glTF extras". Custom properties on objects (for example `obj["od_collider"] = "box"`) arrive in the glTF node's `extras`. This is how a script tags colliders, spawn points and lightmap groups. **Measured:** the tags round-tripped intact.
- `export_texcoords=True` exports **every** UV layer as `TEXCOORD_0`, `TEXCOORD_1` and so on. The exporter counts `len(self.blender_mesh.uv_layers)` ([source](https://github.com/KhronosGroup/glTF-Blender-IO/blob/main/addons/io_scene_gltf2/blender/exp/primitive_extract.py)), so a second UV layer named `Lightmap` comes out as `TEXCOORD_1` even though no material uses it. **Measured:** both sets present.
- `export_apply=True` applies modifiers (bevels, arrays, booleans) before export.
- `export_yup=True`: "Export using glTF convention, +Y up". Blender is Z-up, and glTF and Bevy are Y-up, both in metres. We set it explicitly.
- Collection exporters: "This exporter can be used as a collection exporter… Custom Properties of the collection are exported as Scene glTF extras." This is handy for writing one `.glb` per Map module from a single `.blend`.
- **Leave off:** Draco (`export_draco_mesh_compression_enable`), meshopt compression (`export_meshopt_compression_enable`), gltfpack (`export_use_gltfpack`) and GPU instances (`EXT_mesh_gpu_instancing`). Bevy's loader doesn't support them (section 4.1). **Measured:** the Draco export was rejected.
- Bake target: `bpy.ops.object.bake(type=..., pass_filter=..., uv_layer='', margin=16, ...)` takes a `uv_layer` argument, so it bakes straight into the lightmap UV ([API](https://docs.blender.org/api/current/bpy.ops.object.html)). In the probe, `type="DIFFUSE"` with `pass_filter={"DIRECT","INDIRECT"}` (lighting without surface colour) baked into a float image saved as EXR.

## 3. Procedural Rust

### 3.1 What it's good for

- **Bevy builds meshes in code.** You create a mesh with `Mesh::new`, add attributes and indices (example [`generate_custom_mesh`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/generate_custom_mesh.rs)), or use the built-in primitive shapes.
- **Rust can also write glTF.** The `gltf-json` crate serialises a document, and `gltf::binary::Glb::to_writer` writes `.glb` (official [`export` example](https://github.com/gltf-rs/gltf/blob/main/examples/export/main.rs)). A Rust generator can therefore emit `.glb` files just like Blender and use the same hand-off. Note: the `gltf` crate's last release, 1.4.1, dates from May 2024, though its repo had commits in May 2026 ([crates.io](https://crates.io/crates/gltf)).
- **Bevy 0.19 added asset saving at runtime.** Its release notes name "a procedurally generated mesh, a baked lightmap" as the use case ([0.19 notes](https://bevy.org/news/bevy-0-19/)). It saves in Bevy's formats, not glTF.
- **Supporting crates (latest version and date from crates.io):** `noise` 0.9.0 (Mar 2024) and `fastnoise-lite` 1.1.1 (Mar 2024) for terrain noise; `csgrs` 0.20.1 (Jul 2025, MIT) and `manifold3d` 0.4.1 (Sep 2026) for boolean operations; `meshopt` 0.6.2 (Oct 2025) for mesh simplification (LODs); `parry3d` 0.31.1 (Sep 2026, Apache-2.0) for colliders, with `trimesh`, `convex_hull`, `convex_decomposition` and `heightfield` constructors ([SharedShape](https://docs.rs/parry3d/0.31.1/parry3d/shape/struct.SharedShape.html)).

### 3.2 Why it isn't the main road for Map art

Rust has no UV unwrapping, no lightmap baking, no bevel or boolean modifiers that come with good UVs, and no visual check. Blender provides all of these in one scripted tool. Agents write `bpy` Python fluently, and everything they produce can be opened and inspected in Blender's window by a human.

### 3.3 Where Rust generation belongs

- **Scenario test Maps.** A flat floor, a wall 5 m ahead, a gap of known width. Define these as colliders in Rust inside the engine-free physics crate, with no assets at all. Physics-rule Scenarios shouldn't depend on art files.
- **Scatter and repeated geometry** (cones, pallets, debris), placed with a seeded random generator.
- **Mountains terrain** (later), as a heightfield.
- **Rule:** generate once, offline, and ship the bytes. Don't regenerate collision geometry on each player's machine at runtime. Floating-point results can differ between platforms, and different colliders on two machines would break replays, Scenarios and future multiplayer.

## 4. glTF in Bevy 0.19

### 4.1 Loading, tags and supported extensions

- **Spawning a Map.** `commands.spawn(WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("maps/skate_park.glb"))))`. Bevy 0.19 renamed scene spawning (`WorldAssetRoot`, crate `bevy_world_serialization`) because its new scene system, BSN, landed. The team plans "porting Bevy's glTF loader to the new scene system", so expect API churn in the next releases ([0.19 notes](https://bevy.org/news/bevy-0-19/), [bevy_gltf docs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/lib.rs)). Keep our Map loader small.
- **Tags and names arrive as components:** `GltfExtras` on nodes; `GltfMeshExtras`, `GltfMaterialExtras` and `GltfSceneExtras` for the others; and `Name`, `GltfMeshName`, `GltfMaterialName` ([assets.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/assets.rs)).
- **Supported extensions.** `bevy_gltf` 0.19.1 enables these `gltf` crate features: `KHR_lights_punctual`, `KHR_materials_transmission`, `_ior`, `_volume`, `_unlit`, `_emissive_strength`, `KHR_texture_transform` and `extras` ([Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/Cargo.toml)). It handles `KHR_materials_clearcoat`, `_anisotropy` and `_specular` itself ([loader/extensions](https://github.com/bevyengine/bevy/tree/v0.19.1/crates/bevy_gltf/src/loader/extensions)).
- **Unsupported required extensions are rejected.** The loader validates by default (`validate: true`, [loader](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/loader/mod.rs)). The `gltf` crate flags any `extensionsRequired` entry outside its short supported list as `Unsupported` ([gltf-json 1.4.1](https://docs.rs/crate/gltf-json/1.4.1/source/src/root.rs)). That list has no Draco, meshopt, `KHR_mesh_quantization` or `KHR_texture_basisu` ([list](https://docs.rs/crate/gltf-json/1.4.1/source/src/extensions/mod.rs)). gltfpack, for example, uses `KHR_mesh_quantization` "by default unless disabled via `-noq`" ([gltfpack](https://github.com/zeux/meshoptimizer/blob/master/gltf/README.md)). So don't run gltfpack or glTF-Transform compression on Bevy-bound files. **Measured:** Bevy 0.19.1 logged `Failed to load asset 'sp_draco.glb' … "KHR_draco_mesh_compression": Unsupported extension` and spawned nothing.
- **Measured: the plain export loads.** A headless Bevy 0.19.1 app (`RenderPlugin` with `backends: None`, no `WinitPlugin`) spawned the blockout. It found `GltfExtras` such as `{"od_collider":"trimesh"}` on each node, `Mesh::ATTRIBUTE_UV_1` on each mesh, and accepted a `Lightmap` on all four. The same setup works as a CI load test.
- **Escape hatch.** Since 0.19 a `GltfExtensionHandler` trait gives hooks (`on_gltf_node`, `on_spawn_mesh_and_material`, `on_gltf_primitive`…) to "participate in processing glTF files as they load" ([extensions/mod.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/loader/extensions/mod.rs)). The Map loader can be one of these instead of a system that waits for the scene to spawn.
- **Coordinates.** glTF faces +Z forward and Bevy faces −Z. Bevy offers an experimental `GltfConvertCoordinates` option ([convert_coordinates.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/convert_coordinates.rs)). Pick one convention for Maps and spawn points and write it down.

### 4.2 Materials and textures

- glTF metal/rough PBR maps onto Bevy's `StandardMaterial`. Bevy's docs note the common packed texture: red channel for occlusion, green and blue for metallic-roughness, "use the same image handle for both fields" ([pbr_material.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/pbr_material.rs)). Poly Haven ships exactly that packed map as `arm` (AO, roughness, metal; [API sample](https://api.polyhaven.com/files/brushed_concrete)).
- **Recommendation: a material library, matched by name.** The Blender script assigns placeholder materials named after library entries (`concrete_brushed`, `steel_painted`). Bevy swaps them by `GltfMaterialName` for library materials built from the manifest's textures. The Map files then stay small and textures are shared across Maps. Skein ships a "replace materials from Blender with materials defined in Bevy" example for the same idea ([skein](https://github.com/rust-adventure/skein)).
- **Texture compression** happens in Bevy's asset processor, not in glTF. 0.19 has `asset_processor` and `compressed_image_saver` ("compressed KTX2 UASTC texture output", [features](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/cargo_features.md)). In 0.20 (rc) the saver "compresses textures into BCn formats (for desktop GPUs) or ASTC formats (for mobile GPUs)" with automatic mipmaps, and keeps Basis Universal as `compressed_image_saver_universal` ([0.20 note](https://github.com/bevyengine/bevy/blob/v0.20.0-rc.2/_release-content/release-notes/compressed_image_saver.md)). That keeps iOS and Android open.

### 4.3 Baked lighting

- "Bevy doesn't currently have any way to actually bake lightmaps, but they can be baked in an external tool like Blender" ([lightmap module](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/lightmap/mod.rs)).
- A `Lightmap { image, uv_rect, bicubic_sampling }` component goes on a mesh entity. "If the mesh has a second UV layer (`ATTRIBUTE_UV_1`), then the lightmap will render using those UVs"; brightness comes from `lightmap_exposure` on the material.
- **Nothing attaches lightmaps automatically from glTF.** The official example finds meshes by `GltfMeshName` and inserts `Lightmap` in a system ([lightmaps.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/lightmaps.rs)).
- Batching: "multiple meshes can't be drawn in a single drawcall if they use different lightmap textures, unless bindless textures are in use… combine the lightmap textures into a single atlas, and set the `uv_rect` field". Bevy 0.19 enabled partial bindless on Metal ([0.19 notes](https://bevy.org/news/bevy-0-19/)). **Plan: one lightmap atlas per Map.** **Measured:** one `smart_project` across all four selected objects put their lightmap UVs into one shared 0–1 square, with no texel claimed by two objects. Islands are sized by real-world area, though: the 30 m ground took about half the atlas and the rail only 42 texels at 512². Detailed small pieces may need a separate atlas or scaled islands. The blockout ticket (#17) should tune this.
- Lightmaps only light static geometry. The moving Quad is lit by the sun and the environment map. Bevy also has irradiance volumes and parallax-corrected reflection probes if the Quad looks wrong in Bando's shadows. That decision belongs to the FPV-look ticket (#14).
- Real-time ray-traced light (Solari) is experimental. Bevy 0.20 rc says "Solari now runs on Apple Silicon Macs", but "denoising is not available on Metal yet" ([note](https://github.com/bevyengine/bevy/blob/v0.20.0-rc.2/_release-content/release-notes/solari_metal.md)). Don't plan the alpha around it. Baked light is the safe path to 45 fps on the M4.

### 4.4 LOD

glTF has no LOD concept Bevy reads. Bevy's example says "we need to add the `VisibilityRange` components manually, as glTF currently has no way to specify visibility ranges" ([visibility_range.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/visibility_range.rs)). 0.19 moved these checks to the GPU. Meshlets ("virtual geometry") are still marked experimental. For Skate Park and Bando, skip LOD. If a later Map needs it, have Blender export `name_lod1` meshes with a Decimate modifier, and the Map loader attaches `VisibilityRange` by name.

### 4.5 Colliders authored alongside meshes

- **Authoring.** The Blender script tags each solid object with a custom property, for example `od_collider = "box" | "convex" | "trimesh"`. Where the visual mesh is too detailed, it adds a simplified, hidden collision-only object. The tag vocabulary belongs to the Pack-format ticket (#16).
- **Reading in the engine-free physics crate (measured).** The probe used `gltf` 1.4 and `parry3d` 0.31 with no Bevy. It read the tags, built a trimesh for the quarter pipe, boxes as trimeshes and a convex hull for the rail, and ray-cast the ground correctly. Both crates are permissively licensed (MIT/Apache-2.0 and Apache-2.0).
- **Reading on the Bevy side, if Map collisions end up there:** Avian 0.7's `ColliderConstructorHierarchy` "will automatically generate Colliders on its descendants at runtime", waits for a `WorldAssetRoot` scene to load, and takes per-name overrides (`with_constructor_for_name`, [docs](https://docs.rs/avian3d/0.7.0/avian3d/collision/collider/struct.ColliderConstructorHierarchy.html)). bevy_rapier3d 0.36 has `AsyncSceneCollider` ([docs](https://docs.rs/bevy_rapier3d/0.36.0/bevy_rapier3d/geometry/index.html)).
- **Skein** (bevy_skein 0.6 for Bevy 0.19; the Blender add-on needs at least 0.1.16 on Blender 5.2) stores real Bevy components in extras as `{"skein": [{"crate::Type": {...}}]}` ([README](https://github.com/rust-adventure/skein), [without_blender example](https://github.com/rust-adventure/skein/blob/main/examples/without_blender.rs)). It's good for hand-authoring in Blender's UI. For agent-written scripts, our own small tag names are simpler and don't break when a Rust type is renamed. **Blenvy** hasn't published since August 2024 and has no stable release ([crates.io](https://crates.io/crates/blenvy)), so avoid it.

### 4.6 Runtime-loaded Packs and the agent loop

- `register_asset_source("name", AssetSourceBuilder::platform_default(path, None))` loads assets from any folder at runtime ([extra_source.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/asset/extra_source.rs)). A user-dropped Map Pack is a folder containing a `.glb` and a data file.
- The `dev` feature turns on `file_watcher` hot reload. An agent re-runs the Blender script, and the running game reloads the Map.

## 5. CC0 and permissive sources, and what the licences mean

| Source | Licence (quoted) | Notes for us |
|---|---|---|
| [Poly Haven](https://polyhaven.com/license) (textures, HDRIs, models) | CC0: "You can use our assets for any purpose, including commercial work… You do not need to give credit" | The API is "free to access and use by anyone… including commercial use". Calls need "a unique 'Referer' header or user-agent". A visible credit is needed **only** when surfacing the *live* API inside our product, not for downloaded assets ([API ToS](https://github.com/Poly-Haven/Public-API/blob/master/ToS.md)). File entries list `size`, `url` and `md5` ([example](https://api.polyhaven.com/files/brushed_concrete)), which is ideal for a pinned manifest. |
| [ambientCG](https://docs.ambientcg.com/license/) (PBR textures) | CC0 1.0: "You can include the raw files in your project, for example a video game." | Its [API](https://docs.ambientcg.com/api/) (v3) warns it is "operated… by just one person" and "potentially not as reliable", so cache what we fetch. |
| [Kenney](https://kenney.nl/support) (models) | "public domain licensed (CC0)… even in commercial projects" | "Do not use our logo". |
| [Quaternius](https://quaternius.com/faq.html) (models) | "All models are under the CC0 License." | Low-poly style. |
| [ShareTextures](https://www.sharetextures.com/about) | "licensed as CC0" but refers to a "Limited CC0 Application" policy | **Unverified** (terms page not reachable). Don't use until someone reads those terms. |

What the licences mean for an MIT/Apache-2.0 repo:

- **CC0 assets can ship in the repo.** The CC0 fallback licence covers use "for any purpose whatsoever" ([CC0 legal code](https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt)). But "no trademark or patent rights held by Affirmer are waived", and the author "disclaims responsibility for clearing rights of other persons". Avoid photo textures showing brand logos or someone's graffiti.
- **License assets separately from code.** MIT and Apache-2.0 are written for software. Bevy's own repo dual-licenses its code and lists each asset's source and licence in a [CREDITS.md](https://github.com/bevyengine/bevy/blob/v0.19.1/CREDITS.md) (Kenney, Poly Haven formerly HDRIHaven, CC-BY items with attribution). We should do the same. The [REUSE spec 3.3](https://reuse.software/spec-3.3/) gives a machine-checkable form: a `LICENSES/` folder, plus `REUSE.toml` annotations or `.license` sidecar files for binary files.
- **CC-BY assets** (much of Sketchfab and OpenGameArt) are usable, but every one needs a credit line kept with it. For the alpha, stay CC0-only so the credits file stays trivial.
- **Agent-generated assets.** The US Copyright Office (Part 2 report, 29 January 2025) concluded that "the outputs of generative AI can be protected by copyright only where a human author has determined sufficient expressive elements… but not the mere provision of prompts". It also found that AI "to assist in the process of creation… does not bar copyrightability" ([NewsNet 1060](https://www.copyright.gov/newsnet/2025/1060.html), [report](https://www.copyright.gov/ai/Copyright-and-Artificial-Intelligence-Part-2-Copyrightability-Report.pdf)). For us, the human-shaped parts (the maintainer's layout decisions, edits and review) may be protectable, and purely machine-determined geometry may not be. Practical consequence: **licence generated assets permissively** (CC0 matches our sources and is the honest choice, or the same MIT/Apache dual licence) and don't count on copyright to stop copying. This is a summary of an official source, not legal advice. **Maintainer decision:** CC0 or MIT/Apache for generated assets.

## 6. Git LFS and storage

GitHub's limits ([large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github), [LFS](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-git-large-file-storage), [LFS billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs)):

- **Plain git.** Git warns above 50 MiB, GitHub "blocks files larger than 100 MiB", and browser uploads are capped at 25 MiB. Repos should stay "ideally less than 1 GB, and less than 5 GB is strongly recommended".
- **LFS quota.** GitHub Free and Pro accounts get **10 GiB storage and 10 GiB bandwidth** a month. The largest single file is 2 GB on Free.
- **Who pays for LFS bandwidth.** "If GitHub Actions downloads a 500 MB file that is tracked with Git LFS, it will use 500 MB of the repository owner's bandwidth." "Forking and pulling a repository counts against the parent repository's bandwidth usage."
- **What happens over quota.** Every re-push of a changed LFS file stores a full new copy. Over the bandwidth quota with no payment method, "Git LFS support is disabled on your account until the next month".
- **Releases.** "We don't limit the total size of the binary files in the release or the bandwidth used to deliver them", but each file must be under the LFS per-file limit (2 GB on Free).

**Recommendation: no LFS for the alpha.**

- **Commit:** generator scripts, the texture manifest, and the generated `.glb` geometry. These are kilobytes to low megabytes, and stable byte output (section 1.4) keeps them from churning.
- **Don't commit:** downloaded CC0 textures. A build step fetches them from the manifest, verifies each md5 into a git-ignored cache, and CI caches them. Large processed bundles (compressed textures, high-resolution lightmaps) are published to GitHub Releases and fetched by the same hash-checked step.
- **Revisit** if the asset history passes about 500 MB, or when artists start committing hand-made `.blend` files.

## 7. Worked path: from script to in-game Map

1. **Script.** `assets-src/maps/skate_park/build.py` (Python, runs inside Blender). It starts from an empty file, builds each piece from explicit vertex and face lists, assigns library material names, adds `UVMap` and `Lightmap` UV layers, runs `smart_project` on both, and tags objects (`od_collider`, `od_spawn`, …).
2. **Textures.** `assets-src/materials.toml` lists each library material's Poly Haven or ambientCG asset id, resolution, URL and md5. A fetch step downloads and verifies them.
3. **Build.** `blender --background --factory-startup --python-exit-code 1 --python build.py -- --out assets/maps/skate_park.glb --bake`. This writes `skate_park.glb` (geometry, two UV sets, tags) and `skate_park_lightmap.exr`. Bevy can load EXR (`exr` feature), or the asset processor compresses it to KTX2.
4. **Check.** CI rebuilds on Linux and compares the fingerprint. A headless Bevy test loads the `.glb` with validation on and asserts that every tagged node has the expected collider and every mesh has `UV_1`.
5. **Game.** A `MapPlugin` spawns `WorldAssetRoot` for the Map. As nodes arrive, it swaps materials by name, inserts `Lightmap { image: lightmap_atlas, .. }` on meshes, and turns `od_spawn` nodes into spawn points.
6. **Physics.** The engine-free physics crate opens the same `.glb` with `gltf` and builds parry3d colliders from the `od_collider` nodes. It never sees Bevy, and both sides use the same bytes.
7. **Review for the maintainer.** CI attaches a Cycles CPU render of each changed Map as a PNG. Visual changes are reviewable without reading code. Cycles runs on CPU, but **we haven't tested this on a runner**.

The probe script used for the measurements (blockout geometry, both UV sets, collider tags, CPU bake, export) is in the appendix. It's a ready starting point for the blockout ticket (#17).

## 8. Surprises for other tickets

- **#2 and #14 (rendering on the M4).** The Bevy 0.20 release candidate says Solari now runs on Apple Silicon, without a denoiser. That contradicts #2's note that it doesn't run on Macs, though it doesn't change the plan: baked lightmaps for the alpha.
- **#7 and #11 (determinism, Scenarios).** Blender output isn't byte-identical by default, and Map collision built at runtime could differ between platforms. Treat a Map's identity as the hash of its shipped `.glb`. Ship generated bytes, never regenerate collision per machine, and build Scenario test Maps in Rust inside the physics crate.
- **#10 and #12 (physics scope, crate split).** The engine-free physics crate needs Map collision geometry without Bevy. That means depending on `gltf` (or a small collision export) plus a collision library such as `parry3d`. Collision against the Map is a physics-crate concern, not a Bevy one.
- **#16 (Pack format).** glTF plus a small tag vocabulary in `extras`, plus a data file, is a natural Map Pack, and Bevy can load it from a user folder at runtime. Prefer our own stable tag names over Skein's Rust-type-path tags. Bevy plans to move its glTF loader onto BSN, so keep our Bevy-facing loader thin.
- **#9 (toolchain).** Pin Blender **5.2.2 LTS** (the installed version). If anyone uses the `bpy` wheel, it needs Python 3.13 exactly.
- **#15 (CI).** Asset jobs are CPU-only and should be path-filtered. LFS would drain the 10 GiB/month bandwidth through CI and forks, so don't use it. The Actions cache is 10 GB per repo.
- **#17 (blockouts).** Units are metres; the exporter converts Blender's Z-up to glTF's Y-up. Use `smart_project`, not `lightmap_pack`, and explicit faces, not extrude, if the output must be stable.

## Appendix: probe code

Throwaway code behind the **measured** claims. It isn't committed as source; it's kept here so #17 can start from it.

<details>
<summary>Blender 5.2 blockout script (byte-stable: 3 of 3 runs identical, 14 KB <code>.glb</code>)</summary>

```python
"""Skate Park blockout probe (OpenDrone #8). Runs inside Blender 5.2 LTS:
  blender --background --factory-startup --python-exit-code 1 \
    --python blockout.py -- OUT.glb [--bake]
"""
import bpy, bmesh, sys, math

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
out, do_bake = argv[0], "--bake" in argv

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def mesh_object(name, verts, faces):
    """Explicit vertex and face lists keep the output byte-stable between runs."""
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.update()
    ob = bpy.data.objects.new(name, me)
    scene.collection.objects.link(ob)
    return ob


def box(name, size, center):
    (sx, sy, sz), (cx, cy, cz) = [s / 2 for s in size], center
    v = [(cx + x * sx, cy + y * sy, cz + z * sz) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)]
    f = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    return mesh_object(name, v, f)


def quarter_pipe(name, radius=2.0, width=4.0, deck=1.0, seg=16, y0=6.0):
    prof = [(radius * math.sin(a), radius - radius * math.cos(a))
            for a in (i * (math.pi / 2) / seg for i in range(seg + 1))]
    prof += [(radius + deck, radius), (radius + deck, 0.0)]
    n = len(prof)
    v = [(-width / 2, y0 + y, z) for y, z in prof] + [(width / 2, y0 + y, z) for y, z in prof]
    f = [tuple(reversed(range(n))), tuple(range(n, 2 * n))]
    f += [(i, (i + 1) % n, n + (i + 1) % n, n + i) for i in range(n)]
    return mesh_object(name, v, f)


def rail(name, length=5.0, height=0.4, r=0.025, seg=12, at=(5.0, -3.0)):
    ring = [(r * math.cos(2 * math.pi * i / seg), r * math.sin(2 * math.pi * i / seg)) for i in range(seg)]
    v = [(at[0] + x, at[1] + c, height + s) for x in (-length / 2, length / 2) for c, s in ring]
    f = [tuple(reversed(range(seg))), tuple(range(seg, 2 * seg))]
    f += [(i, (i + 1) % seg, seg + (i + 1) % seg, seg + i) for i in range(seg)]
    return mesh_object(name, v, f)


def material(name, rgb):
    m = bpy.data.materials.new(name)  # name = entry in the game's material library
    m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (*rgb, 1.0)
    return m


concrete, steel = material("concrete_brushed", (0.55, 0.55, 0.52)), material("steel_painted", (0.3, 0.3, 0.32))
parts = [
    (box("ground", (30, 30, 0.2), (0, 0, -0.1)), concrete, "box"),
    (quarter_pipe("quarter_pipe"), concrete, "trimesh"),
    (box("ledge", (4, 0.5, 0.4), (-6, 0, 0.2)), concrete, "box"),
    (rail("rail"), steel, "convex"),
]
for ob, mat, collider in parts:
    ob.data.materials.append(mat)
    ob.data.uv_layers.new(name="UVMap")     # TEXCOORD_0: tiling surface textures
    ob.data.uv_layers.new(name="Lightmap")  # TEXCOORD_1: Bevy's ATTRIBUTE_UV_1
    ob["od_collider"] = collider            # glTF node extras

# Unwrap with smart_project (stable between runs; lightmap_pack was not).
bpy.ops.object.select_all(action="DESELECT")
for ob, _, _ in parts:
    ob.select_set(True)
bpy.context.view_layer.objects.active = parts[0][0]
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
for layer, margin in (("UVMap", 0.01), ("Lightmap", 0.02)):
    for ob, _, _ in parts:
        ob.data.uv_layers.active = ob.data.uv_layers[layer]
    bpy.ops.uv.smart_project(island_margin=margin)  # all selected objects share one 0-1 atlas
bpy.ops.object.mode_set(mode="OBJECT")

if do_bake:
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    scene.collection.objects.link(sun)
    sun.rotation_euler = (math.radians(50), 0, math.radians(30))
    scene.world = bpy.data.worlds.new("sky")
    scene.render.engine, scene.cycles.device, scene.cycles.samples = "CYCLES", "CPU", 64
    img = bpy.data.images.new("lightmap", 1024, 1024, float_buffer=True)
    for m in (concrete, steel):
        node = m.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = img
        m.node_tree.nodes.active = node  # the bake writes into the active image node
    bpy.ops.object.bake(type="DIFFUSE", pass_filter={"DIRECT", "INDIRECT"},
                        uv_layer="Lightmap", margin=4, use_clear=True)
    img.filepath_raw, img.file_format = out.replace(".glb", "_lightmap.exr"), "OPEN_EXR"
    img.save()
    for m in (concrete, steel):
        m.node_tree.nodes.remove(m.node_tree.nodes.active)

bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_extras=True,
                          export_texcoords=True, export_apply=True, export_yup=True,
                          export_cameras=False, export_lights=False)
```

</details>

<details>
<summary>Engine-free Rust probe: <code>gltf = "1.4"</code> (features <code>import, utils, names, extras</code>), <code>parry3d = "0.31"</code>, <code>serde_json = "1"</code></summary>

```rust
use parry3d::math::{Pose, Vector};
use parry3d::query::Ray;
use parry3d::shape::SharedShape;

fn main() {
    for path in std::env::args().skip(1) {
        println!("== {path}");
        // gltf::import validates, like Bevy's loader with `validate: true` (the default).
        let (doc, buffers, _images) = match gltf::import(&path) {
            Ok(x) => x,
            Err(e) => {
                println!("   LOAD ERROR: {e}");
                continue;
            }
        };
        let mut shapes: Vec<(String, SharedShape)> = Vec::new();
        for node in doc.nodes() {
            let (Some(mesh), Some(extras)) = (node.mesh(), node.extras()) else { continue };
            let extras: serde_json::Value = serde_json::from_str(extras.get()).unwrap();
            let kind = extras["od_collider"].as_str().unwrap_or("none").to_string();
            let mut verts = Vec::new();
            let mut tris = Vec::new();
            for prim in mesh.primitives() {
                let r = prim.reader(|b| Some(&buffers[b.index()]));
                let base = verts.len() as u32;
                verts.extend(r.read_positions().unwrap().map(|p| Vector::new(p[0], p[1], p[2])));
                let idx: Vec<u32> = r.read_indices().unwrap().into_u32().collect();
                tris.extend(idx.chunks(3).map(|t| [base + t[0], base + t[1], base + t[2]]));
                let uv_sets = (0..4).filter(|&s| r.read_tex_coords(s).is_some()).count();
                println!("   node {:<13} collider={:<8} verts={:<4} tris={:<4} uv_sets={uv_sets}",
                    node.name().unwrap_or("?"), kind, verts.len(), tris.len());
            }
            let shape = match kind.as_str() {
                "trimesh" | "box" => SharedShape::trimesh(verts, tris).unwrap(),
                "convex" => SharedShape::convex_hull(&verts).unwrap(),
                _ => continue,
            };
            shapes.push((node.name().unwrap_or("?").to_string(), shape));
        }
        // glTF is +Y up: drop a ray from 10 m straight down at the origin.
        let ray = Ray::new(Vector::new(0.0, 10.0, 0.0), Vector::new(0.0, -1.0, 0.0));
        for (name, shape) in &shapes {
            if let Some(toi) = shape.cast_ray(&Pose::IDENTITY, &ray, 100.0, true) {
                println!("   ray hit {name} at y = {:.3}", 10.0 - toi);
            }
        }
    }
}
```

Output on the blockout:

```text
node ground        collider=box      verts=24   tris=12   uv_sets=2
node ledge         collider=box      verts=24   tris=12   uv_sets=2
node quarter_pipe  collider=trimesh  verts=114  tris=72   uv_sets=2
node rail          collider=convex   verts=72   tris=44   uv_sets=2
ray hit ground at y = 0.000
```

</details>

The Bevy 0.19.1 load test was a `DefaultPlugins` app with `AssetPlugin { file_path }` pointed at the output folder, `RenderPlugin { render_creation: WgpuSettings { backends: None, .. }.into(), .. }`, `WindowPlugin { primary_window: None, .. }`, `WinitPlugin` disabled and a `ScheduleRunnerPlugin` loop. It spawned `WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("sp_clean.glb")))`, then queried `(&Name, &GltfExtras, &Children)` and checked `mesh.attribute(Mesh::ATTRIBUTE_UV_1)` on each child's `Mesh3d`. A clean debug build of Bevy 0.19.1 took 3 min 52 s on the M4.

## Sources

Blender
- [Blender LTS](https://www.blender.org/download/lts/) and [5.2 LTS](https://www.blender.org/download/lts/5-2/)
- [Command-line arguments (5.2 manual)](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html)
- [Blender as a Python module](https://docs.blender.org/api/current/info_advanced_blender_as_bpy.html), [bpy on PyPI](https://pypi.org/project/bpy/)
- [Release directory 5.2](https://download.blender.org/release/Blender5.2/)
- [glTF 2.0 add-on manual](https://docs.blender.org/manual/en/latest/addons/scene_gltf2.html), [export_scene operators](https://docs.blender.org/api/current/bpy.ops.export_scene.html), [object.bake](https://docs.blender.org/api/current/bpy.ops.object.html)
- [glTF-Blender-IO primitive_extract.py](https://github.com/KhronosGroup/glTF-Blender-IO/blob/main/addons/io_scene_gltf2/blender/exp/primitive_extract.py), [issue #2671](https://github.com/KhronosGroup/glTF-Blender-IO/issues/2671)
- [Blender licence](https://www.blender.org/about/license/), [ASF GPL compatibility](https://www.apache.org/licenses/GPL-compatibility.html)

Bevy and Rust
- [bevy on crates.io](https://crates.io/crates/bevy), [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/), [cargo features (0.19.1)](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/cargo_features.md)
- bevy_gltf 0.19.1: [Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/Cargo.toml), [lib.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/lib.rs), [loader](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/loader/mod.rs), [extension handlers](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/loader/extensions/mod.rs), [assets.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/assets.rs), [convert_coordinates.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gltf/src/convert_coordinates.rs)
- [Lightmap module](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/lightmap/mod.rs), [StandardMaterial](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/pbr_material.rs)
- Examples: [lightmaps](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/lightmaps.rs), [visibility_range](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/visibility_range.rs), [extra_source](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/asset/extra_source.rs), [generate_custom_mesh](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/generate_custom_mesh.rs)
- Bevy 0.20 rc.2 notes: [compressed image saver](https://github.com/bevyengine/bevy/blob/v0.20.0-rc.2/_release-content/release-notes/compressed_image_saver.md), [Solari on Metal](https://github.com/bevyengine/bevy/blob/v0.20.0-rc.2/_release-content/release-notes/solari_metal.md)
- gltf-json 1.4.1: [supported extensions](https://docs.rs/crate/gltf-json/1.4.1/source/src/extensions/mod.rs), [root validation](https://docs.rs/crate/gltf-json/1.4.1/source/src/root.rs); [gltf export example](https://github.com/gltf-rs/gltf/blob/main/examples/export/main.rs)
- [gltfpack README](https://github.com/zeux/meshoptimizer/blob/master/gltf/README.md)
- [Avian ColliderConstructorHierarchy](https://docs.rs/avian3d/0.7.0/avian3d/collision/collider/struct.ColliderConstructorHierarchy.html), [bevy_rapier3d geometry](https://docs.rs/bevy_rapier3d/0.36.0/bevy_rapier3d/geometry/index.html), [parry3d SharedShape](https://docs.rs/parry3d/0.31.1/parry3d/shape/struct.SharedShape.html)
- [Skein](https://github.com/rust-adventure/skein), [Blenvy on crates.io](https://crates.io/crates/blenvy)
- crates.io: [noise](https://crates.io/crates/noise), [fastnoise-lite](https://crates.io/crates/fastnoise-lite), [csgrs](https://crates.io/crates/csgrs), [manifold3d](https://crates.io/crates/manifold3d), [meshopt](https://crates.io/crates/meshopt), [gltf](https://crates.io/crates/gltf)

Assets and licences
- [Poly Haven licence](https://polyhaven.com/license), [Poly Haven API ToS](https://github.com/Poly-Haven/Public-API/blob/master/ToS.md), [Poly Haven API files endpoint](https://api.polyhaven.com/files/brushed_concrete)
- [ambientCG licence](https://docs.ambientcg.com/license/), [ambientCG API](https://docs.ambientcg.com/api/)
- [Kenney](https://kenney.nl/support), [Quaternius FAQ](https://quaternius.com/faq.html), [ShareTextures](https://www.sharetextures.com/about)
- [CC0 1.0 legal code](https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt), [REUSE 3.3](https://reuse.software/spec-3.3/), [Bevy CREDITS.md](https://github.com/bevyengine/bevy/blob/v0.19.1/CREDITS.md)
- [US Copyright Office AI hub](https://www.copyright.gov/ai/), [NewsNet 1060](https://www.copyright.gov/newsnet/2025/1060.html), [Part 2 report](https://www.copyright.gov/ai/Copyright-and-Artificial-Intelligence-Part-2-Copyrightability-Report.pdf)

GitHub
- [About large files](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github), [About Git LFS](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-git-large-file-storage), [Git LFS billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs)
- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [Actions limits](https://docs.github.com/en/actions/reference/limits)
