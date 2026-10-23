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

## What CI checks

When anything under `assets-src/` or a Pack's `maps/` changes, the **Maps** check downloads the pinned Blender (checking its SHA-256, also pinned in the version file), rebuilds every Map and fails if a committed `.glb` differs. It also renders the pictures above and keeps them as the `map-previews` download, which the [Review Report](review-report.md) points to. `cargo xtask packs` runs the Pack checker over the built-in Pack, the Map included, in every Rust check.

## Not here yet

- Baked lighting: the Skate Park and Bando are lit by a plain sun with shadows, and the baked light that gives real shade and Auto-exposure something to work with comes later. Pictures drawn with `--render-views` will show it when it arrives.
- A material on collider surfaces, which the Video Signal can use to tell concrete from glass.
- The Quad models: they have their own ticket.
