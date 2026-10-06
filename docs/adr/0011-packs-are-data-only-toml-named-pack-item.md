# Packs are data only, written in TOML, and every item is named `<pack>/<item>`

A Pack is a folder of data and assets: a `pack.toml` manifest, then `quads/<id>/`, `maps/<id>/` and `input-devices/<id>.toml`. Every data file is TOML 1.1, the same format as Scenarios, with each number written as text with its unit. Every Quad, Map and Input Device profile is named by its Pack's id plus its folder name, such as `opendrone/whoop-65`. That id never changes, and it's separate from the on-screen name ("Whoop 65"). The game's own content is an ordinary Pack (`packs/opendrone/`), read through the same checker as a Pack a pilot drops in.

A Pack never carries or runs code. It supplies numbers and shapes, never rules. We chose this so that a non-programmer can read and review every Quad and Map, so that Scenarios can name content that never gets renamed under them, and so that a dropped-in Pack can't break "physics first" or the same-results guarantee ([ADR-0001](0001-bit-exact-determinism-with-ordinary-floats.md)). Decided in [#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16).

## What a Pack may never do

- Carry or run code: no scripts, programs or shaders. The game reads only the files the format names.
- Change physics rules. It supplies numbers for Quads and a Map's world values, never new effects or switches.
- Touch a pilot's settings or saved Input Device profile: Presets, Rates, Assists, Packet Rate, bindings or calibration.
- Replace or edit another Pack's items, built-in ones included.
- Reach outside its own folder, or the network.
- Hide where it comes from. The manifest always names an author and a licence.

## Considered options

- **RON:** Rust's own notation, with comments, but brackets everywhere and few people outside Rust know it.
- **JSON:** universal, but it has no comments, so a Quad file couldn't say why a number is what it is.
- **The folder name alone as the id (`whoop-65`):** shorter, but two Packs could clash.
- **The on-screen name as the id:** readable, but renaming or translating it would break every Scenario.
- **Built-in content compiled in or exempt from checks:** the checker would then go untested day to day, and Pack authors would have no real example to copy.

## Consequences

- **Fixing #11's sample:** the sample Scenario in [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11) named `quad = "Meteor65 Pro"`. It now reads `quad = "opendrone/whoop-65"`. Test Quads and built-in Test Maps use the `test/` prefix.
- **Format versions:** every file starts with `format = N`.
  - A newer format than the game knows is refused with "needs a newer OpenDrone".
  - An older one is upgraded in memory, using the same steps the dev tool uses to rewrite the repo's own Packs.
- **Fingerprints for multiplayer:** the game fingerprints what the Simulation receives from each Quad and Map: numbers, Tune, collision shapes, Launch Spot and world values. Comments and pictures don't count. Two players can then check that they fly identical content, and Results record the fingerprints they ran on.
- **The checker:**
  - It reports every problem with its file, line and a plain sentence.
  - In a dropped Pack, a broken item is skipped and the rest loads; a broken manifest skips the whole Pack.
  - CI runs the same checker on the repo's Packs and blocks a merge on any problem.
- **Game Modes and Modifiers** can arrive later as new kinds of item. Per [ADR-0003](0003-crate-split-and-flight-inputs.md), they act only through Flight Inputs, settings, Assists and the choice of Map. A game that meets a kind of item it doesn't know refuses that item instead of ignoring it.
- **Mobile:** for now a Pack is a folder. A single zipped file with the same layout can come later, because Android doesn't let pilots reach an app's folder easily.
- **Input Device profiles:** a Pack's profile is a starting point for a device model. The pilot's calibrated copy lives with their settings and is never written back into a Pack.
