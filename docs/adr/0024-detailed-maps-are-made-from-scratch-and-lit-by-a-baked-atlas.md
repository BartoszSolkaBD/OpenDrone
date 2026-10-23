# Detailed Maps are made from scratch and lit by a baked atlas

Skate Park and Bando look flat: boxes, plain colours and a handful of downloaded pictures under one sun and a flat ambient light. The next Map, the **Factory** (`opendrone/factory`), is an abandoned industrial site of about 200 × 100 m that should look believable at FPV speed: corrugated roof with broken skylights, weathered brick, rusty beams, rubble, overgrowth, light falling into a dark hall. Four decisions make that possible, and the guide for future detailed Maps rests on them.

- **A detailed Map is made from scratch.** Its meshes, textures and materials come from scripts that agents write, run with free tools such as Blender and Python. Nothing is downloaded, and no AI image generator or other third-party tool takes part. The maintainer's own photos would be allowed as a source, but the Factory doesn't use any. The rule is for new detailed Maps only: Skate Park and Bando keep their Poly Haven and ambientCG textures, and [ADR-0014](0014-licences-for-libraries-and-assets.md) still decides their licence. What agents make is CC0, as ADR-0014 says.
- **Generator scripts and their output are both committed.** A generated texture is a compressed JPG, at most 2K for a hero surface and 1K otherwise. A check regenerates the pictures and fails if the committed ones differ, as `cargo xtask maps --check` does for `.glb` files. No Git LFS. Each Map has a size budget for its pictures and its light atlas.
- **The byte-identical rebuild rule stays.** Broken windows, bent beams, bevels, noise and UV unwrapping are written as deterministic Python in `assets-src/lib/`. Blender's booleans, bevels and unwrap are still not used. Cycles is used only to bake light.
- **The light is baked, all of it, direct sun included.** Each Map says where its sun is in `map.toml` (elevation, azimuth and colour, plus a sky colour). Cycles bakes sun and sky light, with bounces and ambient occlusion, into a lightmap atlas on a second UV set, and the game draws the Map's static parts from that atlas with no real-time sun shadows. A Quad isn't baked and gets simple ambient light from the Map's sky values. A Map at another time of day is a copy with its own sun values and its own bake, so it's another Map with its own id. The Factory's sun is about 35–40° up, so light falls through the holes in the roof, and the inside of a building stays 5–6 stops under the outside, as [the camera deep dive](../context/camera.md) asks.

The engine grows in stages, only when a Map looks wrong without a feature: first an in-game screenshot command, then lightmaps, then metallic and ambient-occlusion materials, and later fog, glass or SSAO if the Factory needs them.

## Considered options

- **Keep downloading CC0 pictures.** Rejected for detailed Maps. The same few pictures tile across every Map, and the maintainer wants Maps whose look is made for them. Skate Park and Bando keep them, since redoing them would be work for no new result.
- **Bake only indirect light and keep a real-time sun.** Rejected. The static Map doesn't change, so a real-time sun costs a 4-cascade shadow pass for nothing, and baked direct light is richer and cheaper. The price is that a new sun needs a new bake, and the answer to that is a separate Map.
- **Real-time light only** (SSAO, fog, probes). Rejected. It can't give a dark hall with shafts of light at a cost the M4's 45 fps allows, and Bevy has no cheap global illumination.
- **Relax byte-identical builds for the Factory.** Rejected for now. Reproducibility is what lets a later agent trust and extend a Map. If the Factory proves impossible without Blender's booleans, the maintainer decides again.
- **Commit generator scripts only, make pictures on each build.** Rejected for now. It would make every build and every CI run generate dozens of pictures, and the game would have nothing to draw without a build step. A better arrangement is a later decision, once this one has been tried.

## Consequences

- A Map made from scratch is named as such in the material library, with a generator in place of a download, and `CREDITS.md` lists nothing for it. `cargo xtask textures` ignores generated pictures.
- The lightmap atlas of a 200 × 100 m site may be several 4K–8K pictures. The first slice measures the size and sets the budget the guide gives future Maps.
- The Quad's look under a Map's baked light isn't the same as under a real-time sun. Its shadow on the ground is dropped or faked.
- The Review Report's renders and the previews don't show the baked light until the screenshot command exists. Until then a Map's look is judged in the game.
- `docs/context/world.md` changes two rules. A Map may set its own sun, and graffiti and signs are allowed when an agent made them and they're generic: no real brand, person or artist's work.
- The existing Maps and their tests don't change, and the render counts of the other Maps shouldn't move.
- Performance isn't tuned yet. The Factory is built first, then benchmarked against the 45 fps promise on the M4.
