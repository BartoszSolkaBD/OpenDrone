# Crate split: a deterministic core, entered only through Flight Inputs

The code is one Cargo workspace of twelve unpublished crates (eleven until [#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32) added `opendrone-sound`) that share one version number. Five core crates make up the Simulation. They follow the same-results house rules ([ADR-0001](0001-bit-exact-determinism-with-ordinary-floats.md), bit-exact determinism with ordinary floats), use no Bevy, and never read a clock or open a file. Everything that can change a flight enters through one front door, as Flight Inputs stamped with Simulation Time. We chose this so that the game, the Scenario runner and, later, multiplayer all drive exactly the same Simulation. It also means the build tools enforce the walls, so a maintainer who doesn't read Rust doesn't have to police them in review. Settled in [#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12).

## The crates

| Crate | Group | What it is |
|---|---|---|
| `opendrone-maths` | core | Shared 64-bit number types and the house-rule maths ([ADR-0004](0004-parry3d-geometry-only-f64.md)) |
| `opendrone-physics` | core | Quad physics and Map collisions, including a "held on a thrust stand" set-up |
| `opendrone-flight-controller` | core | Our Betaflight-style Flight Controller, plus the Betaflight CLI translator (Rates paste, `diff all` import), which takes text in and opens no files, and Betaflight's OSD logic, which runs beside the Simulation ([ADR-0022](0022-osd-worked-out-beside-the-simulation.md)) |
| `opendrone-sim` | core | The Simulation: Flight Inputs, Radio Link, Assists, Flight Controller, physics and session state |
| `opendrone-test-pilot` | core | The Test Pilot: reads the state and moves only the sticks, from outside the front door |
| `opendrone-input` | edge | Input Devices on their own thread, on SDL 3.4 ([ADR-0018](0018-input-through-sdl3-on-its-own-thread.md)) |
| `opendrone-pack` | edge | Reads and checks Pack files, and pulls colliders and the Launch Spot out of each Map's `.glb` |
| `opendrone-scenario` | edge | The Scenario runner: headless, used in CI |
| `opendrone-blackbox` | edge | Writes `.bbl` files from the flight log stream |
| `opendrone-sound` | edge | Makes and plays the Quad's sound, Background Sound and menu sounds on Firewheel, on the sound's own thread ([ADR-0023](0023-quad-sound-made-live-on-firewheel.md)) |
| `opendrone` | shell | The game, and the only crate that uses Bevy |
| `xtask` | dev | Asset generators, texture fetch, Scenario format migration, an input monitor. Never shipped |

Folders: `crates/<short name>` (for example `crates/physics`), `scenarios/`, `assets-src/` for Blender Python scripts, and the generated `.glb` files, which are committed.

```
OUTSIDE WORLD (devices, clock, files, screen)  |  CORE: no clock, no files, no Bevy;
                                               |  same results on every computer
 Radio / Gamepad                               |
     | device reports                          |
 +---v-----+                                   |
 | input   |  own thread, SDL                  |
 +---+-----+                                   |
     | Channels, lost / back, stamped by the   |   +--------------------- sim ---------------------+
     | computer's clock                        |   | Radio Link, Assists on input, Rates           |
 +---v--------+                                |   |        v                                      |
 | opendrone  |-- Flight Inputs, stamped ------+-->| flight-controller <-- sensor readings ---+    |
 | (the game, |   with Simulation Time         |   |        v 4 motor commands                |    |
 |  Bevy)     |                                |   | physics: Quad + Map collisions ----------+    |
 |            |<-- every Quad's state each ----+---| session: battery top-up, Reset, seed          |
 +---^--------+    tick; flight log stream     |   +-----------------------------------------------+
     |                                         |          ^ set-up: Quad definitions and Tune, Map
     | Map visuals (.glb)                      |          | shapes, Launch Spot, world values,
 +---+-----+-- checked plain data -------------+----------+ settings, random seed
 |  pack   |                                   |
 +---------+                                   |   test-pilot: reads state, writes Flight Inputs
 +----------+ drives sim the same way,         |   maths: shared by every core crate
 | scenario | checks expectations, writes -----+--> sim
 +----------+ results and .bbl (blackbox)      |
```

## Who may use whom

- `physics` and `flight-controller` each use only `maths`, and **never each other**. The Flight Controller knows what a real board knows: its sensor readings and its Channels. The true attitude its Angle and Horizon modes level against arrives as a sensor reading.
- `sim` is the only place the two meet. It runs the 8 kHz steps, with one Flight Controller loop per step.
- `test-pilot` uses `sim` from the outside, through Flight Inputs.
- `scenario` uses `sim`, `flight-controller`, `test-pilot`, `pack` and `blackbox`.
- `sound` uses no other OpenDrone crate. The game hands it each frame's state.
- The game uses everything except `xtask`.

## What crosses each wall

| Wall | What crosses |
|---|---|
| device → `input` | raw device reports |
| `input` → the game | Channels at the device's full resolution, with every button's state, and lost / back; all stamped with the computer's clock |
| the game, `scenario` or `test-pilot` → `sim` | Flight Inputs: Channels (Arm, Flight Mode and Crash Flip are switch Channels with fixed meanings, [ADR-0017](0017-switches-reach-the-flight-controller-with-fixed-meanings.md)), an Input Device lost or back, and Reset; each stamped with Simulation Time |
| `pack` → `sim` (set-up) | Quad definitions with their Tune, Map collision shapes, the Launch Spot, world values; plus the pilot's settings and a random seed from the caller |
| `sim` → its caller | after each tick, every Quad's state: position, attitude, speeds, each motor's speed, thrust, torque and current, each ESC's state (starting up, ready, running, or stopped after failed restarts), battery voltage and charge, armed state, Flight Mode, Failsafe state and contacts, including how hard each prop rubs, plus the Flight Controller's readings for the OSD ([ADR-0022](0022-osd-worked-out-beside-the-simulation.md)) |
| `sim` → its caller, when asked | the flight log stream: setpoint, gyro, P, I, D and F terms, motor commands and battery voltage at the loop rate. It's off by default and costs nothing while off |
| `sim` → the game (read only) | "which Map surfaces does this line pass through, and where?", for the FPV camera's signal model. It never changes state |
| `sim` ↔ `flight-controller` | in: sensor readings, Channels and the time step. Out: 4 motor commands, each with its spin direction, a debug record, and what Betaflight's OSD reads: why arming is blocked, the Failsafe phase, the battery state, Crash Flip state and the beeper |
| the game or `scenario` → the OSD, beside `sim` | in: each tick's Flight Controller readings and the pilot's OSD layout. Out: the OSD screen, 12 times a second in Simulation Time. Nothing flows back into `sim` ([ADR-0022](0022-osd-worked-out-beside-the-simulation.md)) |
| the game → `sound` | each frame's Quad states and the Flight Controller's beeper, the Map's Background Sound, the Listening Position and the volumes. Out: sound to the device. Nothing flows back into `sim` ([ADR-0023](0023-quad-sound-made-live-on-firewheel.md)) |
| `sim` ↔ `physics` | in: motor commands, each with its spin direction for Crash Flip ([ADR-0012](0012-crashes-behave-like-a-real-quad.md)), and the time step. Out: the new Quad state and sensor readings |

## Rules at the front door

- **What enters, and what never does.** Only Flight Inputs enter. Camera Tilt, FOV, Pause and anything purely visual never do, including the digital camera's extra latency.
- **Where the clock is converted.** The game turns the computer's clock into Simulation Time.
- **Inside, never before the door:** the Radio Link, input smoothing, Auto-arm and Rates.
- **Lost and back:**
  - `input` reports a device lost when the operating system says it was removed.
  - A device that only reports changes, such as a resting Pocket through SDL, is never counted as lost just because it goes quiet. Whether a device that reports all the time may count as lost after a silence is for the input tickets.
  - `input` throws away values that arrive in the same instant as a removal. SDL sends false centre values on switch Channels at unplug.
- **Actions belong to the game.** `input` knows only devices and Channels, and the game turns keys, buttons and switches into Actions.

## Doors left open

- **Multiplayer:**
  - `sim` holds a list of Quads, stepped in a fixed order; the alpha has one.
  - Its whole state can be copied, restored and fingerprinted, including the Flight Controller's memory and the random seed.
  - Networking code will live in the game and move only Flight Inputs and states.
- **Scripting:**
  - Game modes live in the game.
  - They act only through Flight Inputs, settings, Assists and the choice of Map.
  - World values come only from the Map.
- **Mobile:** only `input`, `sound` and the game touch the operating system. SDL runs on iOS and Android, and Firewheel has backends for both.
- **SITL Betaflight:**
  - The Flight Controller seam is in `sim`. What plugs into it: our Flight Controller, a scripted-motors stand-in, and later a SITL bridge in its own crate.
  - The shipped game never depends on the SITL bridge.
  - Runs that use it fall outside the same-results guarantee and can't roll back.

## Scenario set-ups

- **In flight:** through Flight Inputs.
- **Physics alone:** `sim` with the scripted-motors stand-in.
- **Thrust stand:** `sim` with scripted motors and the Quad held in place.
- **Flight Controller alone:** straight into `flight-controller`.
- **Sticks in Scenario files:** `scenario` converts the sticks-in-percent values into whole-number Channels.
- **Mid-air starts:** `sim` can build one with the motors "settled" and the Flight Controller "fresh".
- **Fresh after Reset:** Reset, a new Map or a new Quad also starts a fresh Flight Controller, and powers the ESCs up, so the motors answer only after their ready beep, about 1.7 s later ([#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32)). A mid-air start's "settled" motors have their ESCs already running.

## Files and settings

- **Who reads and writes files:**
  - The core never touches files.
  - `pack` reads Pack files.
  - The game writes the settings file, a Blackbox folder next to it, and saved flights (a Scenario file plus an Input Track).
  - `scenario` reads Scenarios and Input Tracks, and writes results files and `.bbl` files.
  - `xtask` writes generated assets and runs Scenario format migrations.
- **Where settings live:**
  - `flight-controller`: Rates and Flight Mode
  - `sim`: input smoothing, Endless Battery, Auto-arm and Packet Rate
  - the game: camera, graphics, accessibility and sound settings, plus the settings file itself
  - `input`: bindings and calibration, in Input Device profiles

## Checks in CI

- Nothing in the core may depend on Bevy or the operating system.
- The house-rule lints run on the core crates only. Clippy reads its settings for each crate separately.
- One reference Scenario must give the same fingerprint in the game build as in the headless runner.
- The game starts and runs with no sound device. GitHub's machines have none.

## Considered options

- **Fewer crates (physics, Flight Controller and maths inside `sim`).** Rejected. The physics / Flight Controller wall would become a convention an agent could quietly break.
- **More crates now (separate collision or Assist crates; several Bevy crates).** Rejected for now. That's more walls than reasons. The game crate can split later when an area grows.
- **Recording before calibration, or after the input-side Assists.** Rejected. Recordings would depend on the device, or Scenarios couldn't test Assists.
- **Arm and Flight Mode as commands.** Rejected. Betaflight's arming and Failsafe would have to be reinvented.
- **The Test Pilot inside `scenario`.** Rejected. Its stick values feed the Simulation, so it must follow the house rules.
