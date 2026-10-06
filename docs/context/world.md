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

**Free Flight**:
The game mode with no objectives, timers or scoring: the pilot just flies a Map.
_Avoid_: Sandbox, practice mode

## Rules

- A Map is built at real-world scale. Nothing is enlarged to suit the 5" Quad.
- Every Map mixes Gap sizes: a few that only the whoop fits (0.5–1 m), most at 2–5 m for both Quads, and a few big 5" dives (15 m or more).
- A Map has no invisible walls. Its ground runs to the horizon inside a solid Backdrop, with nothing worth flying to beyond about 100 m.
- A Launch Spot is on open, flat ground, with at least 3 m clear all round and open sky above, facing the Map's main feature.
- A Map looks realistic: real-world materials such as concrete, brick, rust and plywood. No real graffiti or brand logos.
- Each Map has one fixed time of day. The alpha's Skate Park and Bando are both in sunny early afternoon.
- A Map sets the world's physical values: gravity, air density and, later, wind. Pilot settings never change them.
- Every part of a Map states whether it is solid. Nothing is solid, or passable, by accident.
- Every Quad, Map and Input Device profile has a fixed id made of its Pack's id and its own, such as `opendrone/skate-park`. Scenarios and settings use the id. The on-screen name is separate and may change.
- A Pack never runs code, changes physics rules, touches a pilot's settings or replaces another Pack's items ([ADR-0011](../adr/0011-packs-are-data-only-toml-named-pack-item.md)).
- A broken item in a dropped Pack is skipped and reported in plain words. The rest of the Pack still loads.
