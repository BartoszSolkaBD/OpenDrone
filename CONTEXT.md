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
| Flying | Quad, Quad definition, Flight Controller, Tune, Flight Mode, Arm, Failsafe, Rates, Assist, Endless Battery, Preset, Prop Wash, Confidence | [docs/context/flying.md](docs/context/flying.md) |
| Input | Input Device, Radio, Gamepad, Channel, Action, Radio Link, Packet Rate, Input Device profile | [docs/context/input.md](docs/context/input.md) |
| World and content | Map, Skate Park, Bando, Gap, Backdrop, Launch Spot, Reset, Drift Event, Pack, Free Flight | [docs/context/world.md](docs/context/world.md) |
| Camera and video | FPV Camera, Camera Tilt, Lens, FOV, Video Look, Video Signal, Breakup | [docs/context/camera.md](docs/context/camera.md) |
| Simulation | Simulation, Simulation Time, Flight Input | [docs/context/simulation.md](docs/context/simulation.md) |
| Verification | Scenario (Flight, Thrust Stand, Flight Controller, Physics), Expectation, Basis, Results, Timeline, Input Track, Test Pilot, Test Map, Test Quad, Feel Test | [docs/context/verification.md](docs/context/verification.md) |
| Screens and navigation | Hub, First Launch, Pause Menu, Pre-flight Warning | [docs/context/screens.md](docs/context/screens.md) |

## Terms to avoid

| Avoid | Say instead |
|---|---|
| drone (in code or specs) | Quad |
| controller | Input Device, or Flight Controller (always say which) |
| arcade mode, easy mode, difficulty | Assist or Preset |
| level, track, scene | Map |
| gate (for an opening on a Map) | Gap (a gate is a racing object) |
| mod, plugin, DLC | Pack |
| quad config, quad profile | Quad definition |
| input profile, controller config | Input Device profile |
| e2e test, replay test | Scenario |
| assertion | Expectation |
| camera angle, uptilt | Camera Tilt (Angle is a Flight Mode) |
| analog noise, static (as a setting) | Breakup |
| video mode, video system | Video Look |
| hotkey, shortcut | Action |
| game loop, game time | Simulation, Simulation Time |
| front door, sim input | Flight Input |
| RC link, link rate | Radio Link, Packet Rate |
| PID profile, FC settings | Tune |
| main menu, lobby, home screen | Hub |
| onboarding, tutorial | First Launch |
