# World and content

Where pilots fly, what they do there, and how content is added. Back to the [map](../../CONTEXT.md).

## Language

**Map**:
A flyable environment, such as Mountains, Skate Park, Bando or Drift Event.
_Avoid_: Level, track (a track is a racing concept), scene

**Skate Park**:
An outdoor Map of a concrete skate park: sunken bowls and transitions to carve, plus street pieces such as stairs, handrails and sculptures to fly through.
_Avoid_: Skatepark (one word)

**Bando**:
A Map of an abandoned, unfinished building and its site, flown inside and out. The name is FPV slang for an abandoned building.
_Avoid_: Ruin, abandoned building (as the Map's name)

**Factory**:
A Map of an abandoned industrial site of about 200 × 100 m, made from scratch and lit by baked light (`opendrone/factory`, [ADR-0024](../adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)). Today it is one 30 × 30 m bay of a brick hall and its yard, worn and with the history of an abandoned works in it ([Building a Map](../building-a-map.md#the-factorys-first-slice), [its second slice](../building-a-map.md#the-factorys-second-slice-wear-uneven-shapes-and-history)).
_Avoid_: Warehouse, plant

**Gap**:
An opening on a Map that a Quad can fly through, such as a doorway, a window, a ring or the space between slats.
_Avoid_: Gate (a gate is a racing object), hole

**Backdrop**:
The simple, solid scenery around a Map's flyable area: ground running to the horizon, buildings and trees. A Quad can crash into it, but there is nothing in it worth flying to.
_Avoid_: Boundary, skybox (a skybox is only the sky)

**Launch Spot**:
The place on a Map where a Quad starts, landed. Every Map has one.
_Avoid_: Spawn point, start position

**Reset**:
An Action that returns the Quad to the Map's Launch Spot, landed and disarmed, with a full battery.
_Avoid_: Restart

**Drift Event**:
A Map in which simulated cars drift around a circuit and the pilot follows them on camera, like an FPV pilot filming at a real drifting event.
_Avoid_: Drifting contest

**Pack**:
A bundle of content made only of data and assets, never code: Quad definitions, Maps and Input Device profiles. The game's own content is a Pack too. A Pack can be added to the repository or dropped in by a pilot at runtime.
_Avoid_: Mod, plugin, DLC

**Pack checker**:
The part of OpenDrone that reads a Pack and refuses anything that breaks the Pack rules, listing every problem with its file, its line and a plain sentence. It checks the game's own Pack in CI and a dropped Pack when the game loads it ([Checking a Pack](../verification/checking-a-pack.md)).
_Avoid_: Validator, linter

**Baked light**:
The sun and sky light of a detailed Map, worked out once by Blender's Cycles from the Map's `[light]` values in `map.toml` (where the sun is, the colour and strength of sun and sky) and stored in a light atlas: a picture that lays every drawn surface of the Map flat, so the game lights the Map's parts by looking each spot up on a second set of picture coordinates, with no real-time sun or shadows on them. The Quad isn't in the atlas: the game lights it with the Map's sky values as ambient light and a share of its sun, with no shadows. A Map at another time of day is another Map with its own bake. The atlas holds light and no reflections, so a surface that is wet, glossy or metal (it says so in the material library) also shows the Map's sky, as much as the atlas says the spot is open to it ([Wet, rough and metal surfaces](../building-a-map.md#wet-rough-and-metal-surfaces)). How one is made: [Building a Map](../building-a-map.md#baked-light).
_Avoid_: Static lighting

**Sky**:
What a baked Map shows behind everything else: a gradient from its sky colour overhead to a paler haze at the horizon, a sun where the Map's `[light]` puts it, a few soft clouds and a dull ground below the horizon, all made by the game from the Map's `[light]` values when the Map loads, with no picture files. A Map with no baked light keeps a flat light blue. How it is made: [Building a Map](../building-a-map.md#what-the-game-does-with-it).
_Avoid_: Skybox (that is a way to draw one)

**Decal**:
A picture put once on a Map's surface, over the surface's own repeating picture: a stain, a damp patch, moss at the foot of a wall, a water streak, a crack or a tag. It is a thin mesh of its own that the Map's script cuts from the surfaces under it and lifts 2 mm off them, with a mask that fades it out before its edges, so its rectangle never shows; it is only drawn, never solid, and is lit by the light baked for the spot it lies on. A decal can show its own colour, make the surface under it wet, or both, and overlapping decals are drawn in the order of their layer. How one is made: [Building a Map](../building-a-map.md#decals).
_Avoid_: Sticker, overlay

**Wear**:
The marks of age and use on a Map's surfaces (water runs, moss, rust, soot, scuffs, droppings, cracks and the like), put as decals (see Decal above) by rules that give each mark a cause: water runs below a sill, moss grows low on a shaded wall, soot rises above a fire. A rule is given a wear level from 0 to 1 and a seed, so the same seed gives the same Map, and a Map's author can add a decal by hand or lock a box against the rules. How a script asks for it: [Building a Map](../building-a-map.md#wear).
_Avoid_: Dirt layer, grunge

**Prop**:
A loose thing on a Map that tells its history, built from shapes by the Map's script: a pallet, a bottle crate, a bottle, an oil drum, a bed frame, the bits of a crashed quad. A prop bigger than about 20 cm that a Quad can hit is solid; a smaller one is only drawn. Few strong groups of props, each placed for a reason, not an even sprinkle ([the lived-in places research](../research/lived-in-places.md), and how a script builds them: [Building a Map](../building-a-map.md#props)).
_Avoid_: Clutter, set dressing, asset

**Scatter**:
Small plants the game grows over a Map's ground, thousands of them, each a real little mesh rather than part of the ground's picture: today grass, a clump of 6 to 12 blades. A ground part asks for it with a tag that names the plant, how many clumps a square metre and how tall; the game places the clumps when the Map is drawn, from a fixed seed, so the same Map always grows the same grass. Scatter is only drawn, never solid: a Quad flies through it. How a Map asks for it: [Building a Map](../building-a-map.md#grass).
_Avoid_: Foliage, vegetation layer

**Kit**:
The loose things a Map's script lays on its floors and yards, made from scratch and solid, not pictures: bricks and brick chunks, concrete lumps with rebar, plaster chips, roof-sheet pieces, planks, boards and pallets, pipes, drums, cable, a tyre, cans and bottles, and the bolt plates and anchor stubs left in a hall floor. Each **kit item** has a few variants, and rules scatter them from a seed: a heap of rubble, debris along a wall's foot, a strewn floor, a row of bolt plates on an old column line, piles of pallets and boards. How a script asks for it: [Building a Map](../building-a-map.md#the-kit-and-the-scatter).
_Avoid_: Props (a prop is a propeller), clutter, asset pack

**Keep-clear zone**:
A rectangle or circle on a Map that the kit's rules leave empty, so a line the pilot flies stays open: an aisle, a Gap's approach, the Launch Spot's 3 m. An item with any corner in it is left out, and a heap is cut away at its edge.
_Avoid_: Exclusion zone, no-spawn area

**Relief**:
The depth of a Map's surfaces made as real geometry by its script, not as a picture: ground and floors that rise and fall (ruts, potholes, a swell under a heap, a thick layer of crumbled plaster, worn patches round columns), brick walls laid brick by brick with sunken mortar and missing bricks, and roofs whose sheets buckle over a bent and broken frame. It keeps out of keep-clear zones as the kit does. How a script asks for it: [Building a Map](../building-a-map.md#relief-floors-walls-and-roofs-with-depth).
_Avoid_: Displacement, terrain detail

**Free Flight**:
The game mode with no objectives, timers or scoring: the pilot just flies a Map.
_Avoid_: Sandbox, practice mode

## Rules

- A Map is built at real-world scale. Nothing is enlarged to suit the 5" Quad.
- Every Map mixes Gap sizes: a few that only the whoop fits (0.5–1 m), most at 2–5 m for both Quads, and a few big 5" dives (15 m or more).
- A Map has no invisible walls. Its ground runs to the horizon inside a solid Backdrop, with nothing worth flying to beyond about 100 m.
- A Launch Spot is on open, flat ground, with at least 3 m clear all round and open sky above, facing the Map's main feature.
- A Map's solid parts and its Launch Spot come from tags in its `.glb`, which a Blender script makes ([Building a Map](../building-a-map.md)); every object says whether it is solid.
- A Map looks realistic: real-world materials such as concrete, brick, rust and plywood. Graffiti and signs are fine when an agent made them and they're generic: no real brand, person or artist's work ([ADR-0024](../adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)).
- Each Map has one fixed time of day, which its sun values in `map.toml` set. The alpha's Skate Park and Bando are both in sunny early afternoon. A Map at another time of day is a copy with its own sun and its own baked light, so it's another Map ([ADR-0024](../adr/0024-detailed-maps-are-made-from-scratch-and-lit-by-a-baked-atlas.md)).
- A Map sets the world's physical values: gravity, air density and, later, wind. Pilot settings never change them.
- Every part of a Map states whether it is solid. Nothing is solid, or passable, by accident.
- Every Quad, Map and Input Device profile has a fixed id made of its Pack's id and its own, such as `opendrone/skate-park`. Scenarios and settings use the id. The on-screen name is separate and may change.
- A Pack never runs code, changes physics rules, touches a pilot's settings or replaces another Pack's items ([ADR-0011](../adr/0011-packs-are-data-only-toml-named-pack-item.md)).
- A broken item in a dropped Pack is skipped and reported in plain words. The rest of the Pack still loads. What the Pack checker refuses: [Checking a Pack](../verification/checking-a-pack.md).
