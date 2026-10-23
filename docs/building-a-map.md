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

A Map builds in about a second. The pictures take a minute or two, because they are rendered on the processor (CI has no graphics card): a plan, a three-quarter view (and a smaller one, which becomes the Pack's `preview.png`), and what each Quad's camera sees landed on the Launch Spot and from a few places a pilot will fly. A metre grid and 1.8 m figures are drawn on them for scale, and they use a plain 110° view with none of the game's textures or lighting, not the Quad's fisheye Lens, which only the game draws. [Seeing the Map as the game draws it](#seeing-the-map-as-the-game-draws-it) says how to see it with the game's own textures and lighting.

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
- no unwrap, no lightmap pack and no bevel or boolean operation, whose output order Blender doesn't keep from run to run ([the research](research/asset-pipeline.md) measured that);
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

## Seeing the Map as the game draws it

The Blender pictures above don't show what the game shows: they have no textures, no shadows from the sun and no Video Look. To see the Map as the game draws it, ask the game itself. It starts the real renderer, with the same textures, materials and lighting as a flight, puts a camera where you say, saves one PNG for each view and quits:

```sh
cargo run -p opendrone -- --map bando --render-views target/map-views \
  --view "name=launch" \
  --view "name=front at=0,-12,2 toward=0,0,3 fov=100" \
  --view "name=what-the-pilot-sees look=analog"
```

That writes `bando-01-launch.png`, `bando-02-front.png` and `bando-03-what-the-pilot-sees.png` into `target/map-views/`: the Map's short id, the view's place in the list, and its name. It prints each file as it saves it. A Map is one run, because the game opens one Map at a time: do the other with a second command, or a loop (`for map in bando skate-park; do ...; done`). The game takes a few seconds to start, and a Video Look view waits about three seconds for Auto-exposure to settle, so pay the start-up once by putting every view of a Map in one run.

A view is a line of `key=value` words, and every key is optional:

| Key | Says | If left out |
|---|---|---|
| `name` | part of the file's name (letters, digits, `-` and `_`) | `view` |
| `at` | where the camera is: `east,north,up` in metres from the Map's origin, as in the Map's script | the Quad's camera, landed on the Launch Spot |
| `toward` | a point to look at, `east,north,up` in metres | see `heading` and `pitch` |
| `heading` | the compass heading to look along, in degrees: 0 north, 90 east | the Launch Spot's heading |
| `pitch` | how far to look up, in degrees: -90 straight down, 90 straight up | the Quad's Camera Tilt |
| `fov` | the FOV in degrees across the picture's diagonal, as the game's FOV is measured | the Quad's own FOV in a Video Look, 110 in a plain view |
| `look` | `plain`, `analog` or `digital` | `plain` |

`toward` doesn't go with `heading` or `pitch`. With no view at all, you get one plain view from the Launch Spot, which is what the Quad's camera sees there. The FOV runs from 90° to 170° in a Video Look, as it does in the game, and from 20° to 170° in a plain view.

A **plain** view is a flat lens at a fixed exposure with the Quad hidden: the Map as the sun lights it, for looking at a wall, a stair or a room from wherever you like. A **Video Look** view is the Quad's own FPV Camera, through its fisheye Lens, Auto-exposure, grain and Look, with its lens put where the view says: it is what the pilot would see from there. Use it for anything about whether the Map flies and reads well on the pilot's screen, and plain for the Map itself. A Video Look view is drawn in the largest 4:3 picture that fits the image, so keep the default size, 1280x960, or another 4:3 one; `--size WxH` sets it. The camera settings go in each view, so `--look`, `--fov`, `--aspect` and `--tilt` aren't taken with this command. `--quad whoop-65` or `--quad freestyle-5` picks the Quad whose camera the Video Look views use.

To keep a list of views, write them one a line in a file, with `#` for notes, and pass it with `--views FILE`. `--view` and `--views` can be mixed, in the order given.

Pictures are drawn with the Map's textures when `cargo xtask textures` has run, and in flat colours when it hasn't: the same as the game ([Materials and textures](#materials-and-textures)). The game's Auto-exposure takes a few seconds to settle after a flight starts, so a Video Look picture matches what the pilot sees once the picture has settled, not in the first second.

**Running it.** The command needs a GPU and opens a window for a moment, as `--screenshot` does, so run it on the maintainer's Mac or another computer with a screen; it draws into an image of its own, not the window. CI has no GPU, so nothing in CI runs it, and there is no golden-image comparison: you look at the PNGs, with the Read tool or an image viewer. It stops after a minute if a view is never drawn, and says so.

**Checking it by hand.** The window part has no automated check. To see that a picture is what the game shows, fly to the same camera and compare: `cargo run -p opendrone -- --map bando --no-screens --screenshot target/game.bmp --seconds 20 --size 1280x960` saves the game's own picture from the Launch Spot, and `--render-views` with `--view "look=analog"` should show the same picture, apart from the noise.

## Materials made from scratch

A detailed Map (the Factory is the first) doesn't use downloaded pictures: its surfaces are drawn by scripts that agents write, and the pictures they make are committed ([ADR-0024](adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)). Nothing is downloaded, no outside picture goes in, and the pictures are CC0 like everything agents make. Skate Park and Bando keep their downloaded textures. The first eight materials, which are the Factory slice's, look like this (the colour picture of each, the way [`cargo xtask generate-textures`](#adding-one) makes it):

![The eight materials made from scratch: painted brick with the plaster peeling, weathered red brick, worn concrete floor and rough concrete wall in the top row; grass and weeds, dirt and rubble ground, rusted corrugated roof and a rusted steel beam in the bottom row](images/materials-made-from-scratch.jpg)

| Material | One repeat | Picture | Cost |
|---|---|---|---|
| `brick_red_weathered` | 1.8 m | 1K | 638 KB |
| `brick_painted_peeling` (paint over plaster over brick, a faded safety band, generic graffiti) | 1.8 m | 1K | 475 KB |
| `concrete_floor_worn` (the hero surface) | 3 m | 2K | 1.8 MB |
| `concrete_wall_rough` (board-formed, tie holes, a spalled rebar) | 2 m | 1K | 490 KB |
| `steel_beam_rusted` | 1 m | 1K | 566 KB |
| `roof_corrugated_rusted` | 1.2 m | 1K | 495 KB |
| `ground_dirt_rubble` | 2 m | 1K | 936 KB |
| `grass_weeds` | 2 m | 1K | 1.3 MB |

Each material has three pictures in `assets-src/generated/<name>/`: `diffuse.jpg` (the colour), `normal.jpg` (the normal map, in the same convention as Poly Haven's) and `roughness.jpg`. The game reads the colour and the normal map today, and draws a part with them as it does a downloaded texture; the roughness picture waits for the ticket that reads it. All eight together are about 6.7 MB, and the contact sheet above is 0.4 MB. Pictures are 2K at most for a hero surface and 1K otherwise. A one-off mark (the graffiti, the spalled rebar, the stencilled block) repeats with every repeat of the picture, so put such a material where its repeats are not side by side in plain view, or cover the repeats with other things.

### In the material library

A generated material is a material like any other, plus a texture that names a generator instead of a download:

```toml
[materials.brick_red_weathered]
colour    = [0.30, 0.10, 0.06]      # the picture's average: what the Map shows without the pictures
roughness = 0.90
texture   = "brick_red_weathered"

[textures.brick_red_weathered]
generator = "brick_red_weathered"   # the script assets-src/generators/brick_red_weathered.py
seed      = 1
pixels    = 1024                    # 512, 1024 or 2048; the picture is square
size      = "1.8 m"                 # what one repeat covers
```

`cargo xtask textures` and `cargo xtask credits` leave such a texture alone: there is nothing to fetch, no licence to credit and no checksum to pin. A texture that has neither a download (`source`, `diffuse`, `normal`) nor a generator, or has both, is refused.

### Making and checking them

```sh
cargo xtask generate-textures          # make every generated texture and the contact sheet
cargo xtask generate-textures --check  # make them into target/textures-check/ and say whether the committed ones are the very same bytes
```

The command runs the pinned Blender (the one [`cargo xtask maps`](#building-and-checking) uses) with no window, and takes about half a minute. It prints what each picture costs. CI runs the check on Linux, with the generators' readable checks, in the **Maps** check; if a picture made on a Mac differs from Linux's, the `textures` download of that check holds the ones CI made: commit those.

The same bytes on every computer is the hard part, as it is for the `.glb` files. The generators run in Blender's Python because its numpy is the same everywhere, use only whole-number maths and the four basic operations on 64-bit numbers (never `sin`, `exp` or a matrix product, which processors may round differently), take their randomness from a hash of the position and the seed (never a random generator), and write the JPG with a writer of their own, [`odjpeg.py`](../assets-src/lib/odjpeg.py), instead of Blender's. [`odtex.py`](../assets-src/lib/odtex.py) explains the rules at its top.

### Adding one

1. Write `assets-src/generators/<generator>.py` with a `make(seed, pixels)` that returns three `uint8` pictures: colour `(pixels, pixels, 3)`, normal map `(pixels, pixels, 3)` and roughness `(pixels, pixels)`. Build it from the blocks of [`odtex.py`](../assets-src/lib/odtex.py): seeded multi-scale noise (`fbm`), cellular noise (`voronoi`), cracks, brick and plank layouts, edge wear, streaks and drips, stains, rust that spreads from edges and seams, peeling paint, moss and dirt in low places (`cavity`), loose stones, blades of grass, generic graffiti and faded sign marks. Lay marks out in tile units (0 to 1), so the picture is the same at any size, and keep to the rules at the top of `odtex.py`: **the picture has to repeat**, so a mark placed with a gradient from top to bottom, or any `Y > 0.9`, leaves a seam. [`brick_red_weathered.py`](../assets-src/generators/brick_red_weathered.py) is a short example.
2. Add the material and its texture to `assets-src/materials.toml` as above, run `cargo xtask generate-textures` and commit the pictures with the script.
3. Look at the pictures: open `assets-src/generated/<name>/diffuse.jpg` and the contact sheet, and, from 1 to 2 m, ask the game for views of a Map that uses the material ([Seeing the Map as the game draws it](#seeing-the-map-as-the-game-draws-it)). Nothing in the picture may be a real brand, person or artist's work.
4. Run the generators' readable checks, which also run in CI:

```sh
blender --background --factory-startup --python-exit-code 1 --python assets-src/generators/test_generators.py
```

They prove that a seed gives the same bytes and another seed doesn't, that each picture repeats without a seam, and that each normal map points out of the surface. The size is yours to watch: a normal map of a busy surface is the biggest picture, so keep fine detail out of the height if the normal map grows past about half a megabyte.

## What CI checks

When anything under `assets-src/` or a Pack's `maps/` changes, the **Maps** check downloads the pinned Blender (checking its SHA-256, also pinned in the version file), rebuilds every Map and fails if a committed `.glb` differs. It also renders the pictures above and keeps them as the `map-previews` download, which the [Review Report](review-report.md) points to. `cargo xtask packs` runs the Pack checker over the built-in Pack, the Map included, in every Rust check.

## Not here yet

- Baked lighting: the Skate Park and Bando are lit by a plain sun with shadows, and the baked light that gives real shade and Auto-exposure something to work with comes later. Pictures drawn with `--render-views` will show it when it arrives.
- A material on collider surfaces, which the Video Signal can use to tell concrete from glass.
- The Quad models: they have their own ticket.
