# Building a Map

A Map is built by a script, not drawn by hand. An agent writes a Blender script in `assets-src/maps/<id>/build.py`; Blender runs it with no window and writes the Map's `.glb` (binary glTF, the standard 3D file) into the built-in Pack, at `packs/opendrone/maps/<id>/`. Both the script and the `.glb` are committed: the script is the source, and the `.glb` is a build output, like a compiled program. [Checking a Pack](verification/checking-a-pack.md) says what the Pack checker demands of the file. The terms are in [World and content](context/world.md).

The Maps are CC0, like everything agents make, and the scripts are MIT or Apache-2.0 ([ADR-0014](adr/0014-licences-for-libraries-and-assets.md)). The first Map built this way is the Skate Park, and the second is Bando.

## Building and checking

You need Blender, the exact version [`assets-src/blender-version.toml`](../assets-src/blender-version.toml) pins (5.2.2 today). That file is the one place the version is written: this command and CI both read it, and the command refuses any other Blender. It looks for `blender` on the path, in `/Applications/Blender.app` and in Program Files, or wherever the `BLENDER` variable points.

```sh
cargo xtask maps                                   # rebuild every Map's .glb into the Pack
cargo xtask maps --check                           # build into target/maps-check/ and say whether the committed .glb is the same bytes
cargo xtask maps --previews target/map-previews    # also render pictures of every Map
cargo xtask maps --previews target/map-previews --update-preview   # and copy the new preview.png into the Pack
```

A Map builds in about a second. The pictures take a minute or two, because they are rendered on the processor (CI has no graphics card): a plan, a three-quarter view (and a smaller one, which becomes the Pack's `preview.png`), and what each Quad's camera sees landed on the Launch Spot and from a few places a pilot will fly. A metre grid and 1.8 m figures are drawn on them for scale, and they use a plain 110° view, not the Quad's fisheye Lens, which only the game draws.

## What a script does

Each script makes a `MapBuilder` ([`assets-src/lib/odmap.py`](../assets-src/lib/odmap.py)), adds shapes to it, places the Launch Spot and exports. The builder takes the shapes the Maps need: boxes, straight walls of boxes round their openings (doorways, windows and gaps), posts and cones, rails, steps and banks (a side outline cut out across a width), a pipe, a sheet over a rectangle with a height at every corner (the bowls), and trees and light poles. Each shape is given a **material** by name and a **collider** tag:

- `box` for a box, found again from its corners by the Pack checker;
- `convex` for a post or cone that stands straight up (its corners are rounded flat, so a tilted one can't be convex);
- `trimesh` for everything else solid: a closed mesh is a solid and an open one is a surface;
- `none` for a part that is only drawn.

The tags and the Launch Spot rules are in [Checking a Pack](verification/checking-a-pack.md#a-map-mapsid) and the [World and content](context/world.md) rules: the Launch Spot is on open flat ground, with at least 3 m clear all round and open sky above, facing the Map's main feature. The Skate Park's script, [`assets-src/maps/skate-park/build.py`](../assets-src/maps/skate-park/build.py), and Bando's, [`assets-src/maps/bando/build.py`](../assets-src/maps/bando/build.py), keep each size of #17's table of must-have features, and [`crates/pack/tests/skate_park.rs`](../crates/pack/tests/skate_park.rs) and [`crates/pack/tests/bando.rs`](../crates/pack/tests/bando.rs) measure them in the finished Map. Bando's thin parts are solid at their real thickness (6 cm rails, a 5 cm hook cable, 3 cm rebar), and its ceilings and floors are colliders, so the ceiling effect and the continuous collision check have something to work on.

### The same bytes on every computer

CI rebuilds each Map with Blender on Linux and requires the very same `.glb` as the committed one, so a script has to give the same bytes on a Mac, on Windows and on Linux. Blender's own maths is 32-bit, and its last digit can differ between processors, so the scripts never use it for anything that reaches the file:

- every shape is worked out in plain Python with 64-bit numbers ([`assets-src/lib/odgeom.py`](../assets-src/lib/odgeom.py)), placed in the Map there, and handed to Blender with every object at the origin and unturned;
- every corner is rounded to 0.1 mm, then to the 32-bit number the file stores;
- every solid is cut into triangles by a fixed rule, never by Blender's own triangulation, and a convex solid's four-cornered faces are cut along the diagonal that leaves no dent;
- no Blender unwrap, no Blender lightmap pack and no bevel or boolean operation, whose output order Blender doesn't keep from run to run ([the research](research/asset-pipeline.md) measured that); the second picture-coordinate set that [baked light](#baked-light) needs is made in plain Python;
- the picture coordinates are the Map's own metres divided by the material's repeat size, so a picture runs on from one object to the next;
- the file has no normals (the game works them out) and no pictures, compressed meshes or animations.

If you change a script, run `cargo xtask maps` twice and `--check`: the bytes must not change between runs.

If a Map built on a Mac or Windows differs from what CI builds on Linux, the `maps` download of the **Maps** check holds the one CI built: commit that, and tell the Reviewer which step differed.

## Materials and textures

Every object's material is named after an entry in the **material library**, [`assets-src/materials.toml`](../assets-src/materials.toml): its flat colour, its roughness and, if it has one, the CC0 texture behind it. The `.glb` carries only the name and the flat colour. The game gives each name its pictures.

- The textures are from [Poly Haven](https://polyhaven.com/license) and [ambientCG](https://docs.ambientcg.com/license/), both CC0, at 1K, as JPG: a colour picture and a normal map each. The manifest lists each file's address, who made it and the SHA-256 of the file.
- `cargo xtask textures` downloads them into `assets-src/textures/`, which git ignores: pictures are fetched, never committed. It refuses any file whose checksum differs from the manifest's and keeps nothing it refused. `cargo xtask textures --pin` writes the checksum of a texture you just added; look at the pictures before committing it, and check they have no brand logos or graffiti.
- `cargo xtask credits` writes [`CREDITS.md`](../CREDITS.md) from the manifest, and CI checks it is up to date.
- Adding a material: add it to `[materials]` (and its texture to `[textures]`), run `cargo xtask textures --pin`, then `cargo xtask credits`. A texture no material uses is refused, so nothing is credited for nothing.

The game reads the library from the repo, so it draws a Map with pictures only when it runs from a repo where `cargo xtask textures` has run. You don't have to remember that: `./play` (`play.cmd` on Windows), or `cargo xtask play`, runs it for you, then builds and starts the game ([Starting the game from any checkout](pilot-guide.md#starting-the-game-from-any-checkout)). Without the pictures, every material is drawn in its flat colour, and the Map flies the same. A packaged game finds its pictures once the release ticket lays them out.

## Baked light

A detailed Map is lit by **baked light** ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)): Blender works out, once, how much sun and sky light reaches every spot of every surface, with bounces off the walls and the shade that crowding makes, and keeps the answer in a picture, the **light atlas**. The game then lights the Map by looking the answer up (the game's part is [#294](https://github.com/BartoszSolkaBD/OpenDrone/issues/294)). The Skate Park and Bando don't ask for it and are lit as before.

### Asking for it, in the script

```python
m = MapBuilder(...)
m.lightmap(texel_cm=20)                       # one texel of the atlas is 20 cm of the Map
m.box_h("pillar", ..., light_cm=10)           # this one shape gets finer light
```

- `lightmap(texel_cm)` gives every drawn mesh a second set of picture coordinates (`TEXCOORD_1` in the `.glb`) that lays all of them flat, side by side, in one atlas. A smaller `texel_cm` means sharper shade and a bigger atlas: 10 is fine detail, 25 is coarse.
- A shape's own `light_cm=` overrides the density for that shape, for a part that deserves finer light than the rest, or far-off ground that needs none.
- A flat piece longer than `max_chart` texels (512 by default) is made coarser to fit, so a 2 km ground costs the same as a 50 m floor. The atlas must fit in `max_size` texels a side (8192 by default); if it doesn't, the script stops and says to raise `texel_cm`. There is one atlas per Map, so a Map that needs more asks for coarser light, or for its far parts to be coarser.
- Parts that are `od_collision_only` get no picture coordinates and cast no shade: nothing draws them.
- The unwrap is plain Python, written the way the rest of the Map is ([`odlight.py`](../assets-src/lib/odlight.py)): it cuts each mesh into flat charts of faces that point nearly the same way, scales each chart to the density, leaves 2 empty texels all round each chart, and packs them in a fixed order. It never uses Blender's `smart_project` or island packing, so the `.glb` is the same bytes on every computer, as [above](#the-same-bytes-on-every-computer).

### The light, in `map.toml`

```toml
[light]
sun_elevation = "37 °"       # how high the sun is: 0 ° on the horizon, 90 ° overhead
sun_azimuth   = "225 °"      # the compass direction the sun is in: 0 north, 90 east, 225 south-west
sun_colour    = "#fff2d6"    # sRGB, as in a paint program
sun_strength  = "3.0"        # a plain number, from 0 up to 1000
sky_colour    = "#a5c4f0"
sky_strength  = "0.5"
```

Every value is needed, and [the Pack checker](verification/checking-a-pack.md#a-map-mapsid) refuses a wrong one in a sentence. The strengths are in the atlas's own units: `sky_strength` is how bright a white surface facing up under an open sky is, and `sun_strength` how bright a white surface square-on to the sun is, and the sky is one even colour all round (measured on Bando: flat ground under a sun 50 ° up with `sun_strength` 3.0 and `sky_strength` 0.5 bakes to about 2.5 in the red channel, which is 3.0 × sin 50 ° = 2.3 from the sun plus the sky's share). The picture holds light only, and the game multiplies it by each surface's own colour. A sun of about 3 against a sky of about 0.5 gives sunlit ground about 6 times as bright as its shade.

### Baking it

```sh
cargo xtask maps --bake <id>                  # bakes one Map's light, at 512 samples a texel
cargo xtask maps --bake <id> --samples 128    # a quick look
cargo xtask maps --bake <id> --samples 1024   # the one to commit
```

It rebuilds the Map's `.glb` and, with Blender's Cycles on the processor, bakes the `[light]` into `packs/opendrone/maps/<id>/<id>.light.exr`. It also writes `<id>.bake.toml` beside it, which says which `.glb` and which `[light]` values the atlas was baked from, plus the size, the samples and how long it took. Commit all three.

- **What gets baked.** Sun and sky light, with up to 6 bounces, from a sun half a degree wide (so shade has the soft edge a real shadow has). Bounced light takes the colour of what it bounces off, from each material's flat colour in the `.glb`: a brick wall's red spills onto the floor beside it, but the texture's pattern doesn't. Shade from crowding (the dark corner where a wall meets the floor) is part of it.
- **Noise.** Cycles can't denoise a bake, so the atlas has a little noise, which shows most indoors, where all the light has bounced. It falls with the square root of the samples and the time rises with them. 256 samples is fine for the sunlit outside and grainy inside; 1024 is clean on Bando. Bake with plenty before committing.
- **Time.** It grows with the texels in the atlas times the samples. The table below has the times on the maintainer's M4 Mac mini (10 cores).
- **The format, `.exr`.** The atlas is an OpenEXR picture of half-size floats (16 bits a channel), ZIP-compressed. Sunlit and shaded parts differ by a factor of ten and more, and the Factory's halls will differ by 5 to 6 stops, which an 8-bit picture can't hold without banding or a scale that `map.toml` would have to carry (a PNG of the same atlas is a third of the size, but sun above 1.0 would be cut off). EXR holds the real numbers, so the game needs no decoding rule, and it is what Blender writes without help. It costs size (about 1.7 to 14 MB for Bando, below; ZIP gives about a fifth of the raw size, DWAA a tenth, but the game's reader may not open it) and, in the graphics card, 8 bytes a texel if the game keeps it in half floats, or 16 if the loader makes full floats: the game's ticket measures the load time and the memory, and may turn the atlas into a compressed form at load time.
- **The atlas isn't rebuilt in CI**, because Cycles is allowed to differ a little from run to run. So that the committed atlas can't quietly go out of date, `cargo xtask maps --check` reads `<id>.bake.toml` and fails if the atlas file is missing, if the `.glb` Blender builds now isn't the one it was baked from (the shapes or the second UV set changed), or if the `[light]` values aren't the ones it was baked with. The fix is the `--bake` command above.
- **The unwrap is checked everywhere.** `assets-src/lightmap-uv.toml` holds the SHA-256 of each Map's `.glb` built with a second UV set at 25 cm per texel, whether the Map asks for one or not. `cargo xtask maps` writes it and `--check` demands it, so CI proves on Linux that the unwrap gives the same bytes as on a Mac. The unwrap's own rules (charts, margins, density, no overlap) are checked in plain Python by `python3 -m unittest discover -s assets-src/lib`, which CI runs too.

### What it costs: Bando

A throwaway copy of Bando (a 70 × 50 m site with a five-floor building, 745 shapes, 13,248 triangles, ground to 2 km), baked with the `[light]` above at 256 samples. The atlas is EXR, half floats, ZIP. Times are on the maintainer's M4 Mac mini and vary by half again with what else the computer is doing.

| Texel | Atlas | Atlas file | In the graphics card (half floats) | Bake time |
|---|---|---|---|---|
| 40 cm | 4096 × 460 | 1.7 MB | 15 MB | 47–61 s |
| 25 cm | 2048 × 1260 | 3.4 MB | 21 MB | 88 s |
| 20 cm | 4096 × 772 | 4.8 MB | 25 MB | 79–159 s |
| 10 cm | 2048 × 4076 | 14.3 MB | 67 MB | 7 to 8 minutes |

At 1024 samples the 40 cm atlas took 258 s. About 85 to 90% of the atlas is covered by charts. The rest is the gaps between them, and the undersides of boxes (which nothing sees, but which can't be told apart from other faces).

## What CI checks

When anything under `assets-src/` or a Pack's `maps/` changes, the **Maps** check downloads the pinned Blender (checking its SHA-256, also pinned in the version file), rebuilds every Map and fails if a committed `.glb` differs, or a Map's baked light is out of date, or the second UV set's hash differs (see [Baked light](#baking-it)). It first runs the unwrap's plain-Python checks. It also renders the pictures above and keeps them as the `map-previews` download, which the [Review Report](review-report.md) points to. `cargo xtask packs` runs the Pack checker over the built-in Pack, the Map included, in every Rust check.

The pictures don't show baked light yet: they are drawn with a plain sun, and a Map's look under its baked light is judged in the game until the screenshot command exists.

## Not here yet

- Baked light for a Map in the built-in Pack: Bando and the Skate Park don't use it, and the game doesn't read it yet ([#294](https://github.com/BartoszSolkaBD/OpenDrone/issues/294)).
- A denoising pass for the atlas, a second atlas for a Map too big for one, and a smarter cut of the undersides of boxes: later, if the Factory needs them.
- A material on collider surfaces, which the Video Signal can use to tell concrete from glass.
- The Quad models: they have their own ticket.
