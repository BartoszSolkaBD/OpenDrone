# OpenDrone

An open-source FPV drone flight simulator. Its flight physics stay faithful to real-world quads, with optional Assists layered on top.

This file is the **map**: read it first, then open only the deep dive for the topic you're working on. Each deep dive holds that topic's full glossary entries, terms to avoid, and domain rules.

## Guiding principles

- **Physics first.** Faithful flight physics is the top priority and the optimisation lighthouse. Every other area may compromise to protect it.
- **Assists, never arcade.** Easier experiences come from Assists layered on top. The physics underneath never changes.
- **Leave doors open.** Multiplayer, scripting and mobile are on the roadmap. Don't make choices that rule them out.
- **Behaviour is reviewable without reading code.** Scenarios are the contract.

## Topics

| Topic | Terms | Deep dive |
|---|---|---|
| Flying | Quad, Flight Controller, Flight Mode, Arm, Rates, Assist, Endless Battery, Preset | [docs/context/flying.md](docs/context/flying.md) |
| Input | Input Device, Radio, Gamepad, Action | [docs/context/input.md](docs/context/input.md) |
| World and content | Map, Skate Park, Bando, Gap, Backdrop, Launch Spot, Reset, Drift Event, Pack, Free Flight | [docs/context/world.md](docs/context/world.md) |
| Camera and video | Camera Tilt | [docs/context/camera.md](docs/context/camera.md) |
| Verification | Scenario (Flight, Thrust Stand, Flight Controller, Physics), Expectation, Basis, Results, Timeline, Input Track, Test Pilot, Test Map | [docs/context/verification.md](docs/context/verification.md) |

## Terms to avoid

| Avoid | Say instead |
|---|---|
| drone (in code or specs) | Quad |
| controller | Input Device, or Flight Controller (always say which) |
| arcade mode, easy mode, difficulty | Assist or Preset |
| level, track, scene | Map |
| gate (for an opening on a Map) | Gap (a gate is a racing object) |
| mod, plugin, DLC | Pack |
| e2e test, replay test | Scenario |
| assertion | Expectation |
| camera angle, uptilt | Camera Tilt (Angle is a Flight Mode) |
| hotkey, shortcut | Action |
