# Reading a Scenario and its Results

A **Scenario** is a test you can read without reading code: a starting state, the inputs, and Expectations about what the Simulation does. The terms are in the [Verification deep dive](../context/verification.md). This page walks through one, [`scenarios/physics/free-fall-is-exactly-g.toml`](../../scenarios/physics/free-fall-is-exactly-g.toml), and the Results file the runner writes beside it.

## Where they live

- `scenarios/<topic>/<plain words>.toml`: the Scenarios, grouped by topic: `physics/`, and `quads/<quad>/` for one Quad's own, such as its Thrust Stand Scenarios.
- `<name>.results.toml`, beside each Scenario: what the last run measured. The runner writes it; never edit it by hand.
- `scenarios/test-quads/`: Test Quads, such as `whoop-65-no-drag.toml`, the Whoop 65 with its drag set to zero, and `whoop-65-bench-supply.toml`, the Whoop 65 on the 4.0 V bench supply BetaFPV measured its motor on.

## The Scenario file

```toml
format = 1
name   = "Free fall is exactly g"
```

`format` is the file format's version, so an older OpenDrone refuses a newer file. When a new starting-state item arrives, one command writes it into every Scenario and bumps this number ([Changing a file format](../format-migration.md)). `name` says what the Scenario proves.

### The starting state, `[start]`

It spells out every item that affects the Simulation, every time, with no hidden defaults ([ADR-0002](../adr/0002-scenario-starting-state-spells-out-everything.md)). A file that leaves one out is refused.

| Item | Example | Meaning |
|---|---|---|
| `kind` | `"physics"` | One of `"flight"`, `"thrust stand"`, `"flight controller"`, `"physics"`. So far Physics and Thrust Stand Scenarios run: scripted motors stand in for the Flight Controller. On the thrust stand the Quad is held still, so its `speed` and `rotation` must be zero, while its motors, ESCs and battery work as in flight. |
| `quad` | `"test/whoop-65-no-drag"` | The Quad, by id. Its numbers come from its Quad definition, never from the Scenario. |
| `map` | `"test/empty-air"` | The Map, by id. Gravity, air density and everything solid come from the Map. The Test Maps are built into the code: see [Test Maps](#test-maps) below. |
| `position` | `"0 m east, 0 m north, 0 m up"` | From the Map's origin. |
| `attitude` | `"level, heading 0°"` or `"roll 0°, pitch 30°, heading 45°"` | Heading is a compass heading (0° north, 90° east). Pitch is nose up, roll is right side down. |
| `speed` | `"0 m/s"` | East, north and up, such as `"5 m/s north, 0 m/s east, 0 m/s up"`. A single number must be zero. |
| `rotation` | `"roll 2000 °/s, pitch 0 °/s, yaw 0 °/s"` | Rolling right, pitching nose up and yawing nose right are positive, as in Betaflight. |
| `armed` | `false` | Whether the Quad is armed. |
| `motors` | `"stopped"` | How the motors and their ESCs start: see [How the motors start](#how-the-motors-start) below. |
| `flight_controller` | `"fresh"` | As right after Reset powers it up. |
| `battery` | `"100%"` | The charge. |
| `flight_mode` | `"Acro"` | `"Acro"`, `"Angle"` or `"Horizon"`. |
| `assists` | `{ input_smoothing = "off", endless_battery = "off", auto_arm = "off" }` | Every Assist, on or off. |
| `radio_link` | `"250 Hz"` | The Packet Rate: 50, 100, 150, 250, 333, 500 or 1000 Hz. |
| `physics_rate` | `"8 kHz"` | Physics steps a second. |
| `random_seed` | `1` | The seed for the Simulation's random numbers. |
| `[start.rates]` | `type = "Actual"`, `roll = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"`, … | Every field of a Betaflight 2026.6 rate profile, written as the Betaflight App shows it: see [The Rates](#the-rates) below. |

In Physics and Thrust Stand Scenarios the Flight Controller doesn't run, so `armed`, `flight_controller`, the Flight Mode, the Assists, the Radio Link and the Rates change nothing. They are written down all the same, so the format never needs them added later.

### How the motors start

| `motors` | The motors | Their ESCs |
|---|---|---|
| `"powering up"` | Stopped. | Just powered, as when a pack is plugged in: they play Bluejay's start-up melody, a "signal found" beep and a ready beep, and answer no command until they are ready, about 1.66 s later. They beep ready only once the throttle has been at 0% for ten counts of their 32 ms timer, so a command above 0% before then holds them back. |
| `"stopped"` | Stopped. | Already powered up and ready: each starts its motor on its first command above 0%, after the start wait. |
| `"settled"` | Spinning, all at the speed whose thrust carries the Quad's weight along the motors' axis (none upside down). Level, that holds the stated motion; tilted, only drag could, and drag arrives with the air ticket (#42). | Running. |

`"powering up"` and `"stopped"` are only for Physics and Thrust Stand Scenarios, which script their motors, and a Quad held still on the thrust stand has no motion for `"settled"` motors to hold. Where the Flight Controller runs, a landed start with a "fresh" Flight Controller is exactly Reset, whose ESCs power up first; the arming and power-up ticket (#52) names how its motors are written.

Each ESC copies Bluejay v0.21.0 ([`crates/physics/src/esc.rs`](../../crates/physics/src/esc.rs) has the timing, with the firmware's file and line for each step):

- **Starting a stopped motor:** on the first command above 0%, a ready ESC waits the Quad's start wait (0.1 s), then starts the motor with its drive held at the Quad's start-up power limit (1.96%) for 15 electrical turns (Bluejay's 24 start-up commutations, then its initial-run countdown of 12 turns, which starts on the fourth), and then runs it as commanded.
- **Stopping:** at 0% it brakes the motor, and below Bluejay's minimum speed, about 1,330 electrical RPM, switches it off and is ready again.

### Test Maps

The Test Maps are simple Maps built into the code for Scenarios only, each with the `test/` prefix. Each one's `map.toml` is a file in [`crates/pack/test-maps/`](../../crates/pack/test-maps/), so `cargo xtask migrate` keeps its format up, and its solid parts are written in [`crates/pack/src/test_maps.rs`](../../crates/pack/src/test_maps.rs). All have standard gravity (9.81 m/s²) and sea-level air (1.225 kg/m³). Every one but empty air stands on the same ground: a box 200 m square whose top is the origin's height, 0 m. Positions are from the Map's origin.

| Id | What is in it |
|---|---|
| `test/empty-air` | Nothing: no floor, no walls. |
| `test/flat-floor` | The ground. |
| `test/wall` | The ground and a wall 20 m wide, 5 m tall and 0.2 m thick, its face 5 m east, facing west. |
| `test/thin-rail` | The ground, a round rail 6 cm across running north–south at 3 m east, 1 m up, from 5 m south to 5 m north, and a rebar stub 3 cm across standing 2 m tall at 3 m east, 10 m north: the alpha Maps' thinnest parts. |
| `test/floor-and-ceiling` | A floor and a concrete ceiling 3.1 m above it, as on the Bando's floors: the floor a sheet at 0 m, the ceiling a slab from 3.1 m to 3.3 m, both triangle meshes 40 m square. |
| `test/ledge` | The ground and a platform 1 m high covering everything west of the origin, its edge running north–south through the origin. |

Between them they use all three kinds of solid shape a Map's `.glb` gives: boxes, convex shapes (the rail, the stub and the ledge) and triangle meshes (the floor and ceiling). A Quad resting on a floor has its centre half its body box's height above it: 10 mm for the Whoop 65, 17.5 mm for the Freestyle 5″.

### The Rates

`[start.rates]` holds every field of a Betaflight 2026.6 rate profile, each written as the Betaflight App's Rates tab shows it. The runner keeps each as Betaflight's CLI stores it, so the Flight Controller reads exactly what Betaflight would. A number between two stored steps is refused.

```toml
[start.rates]
type               = "Actual"
roll               = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
pitch              = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
yaw                = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"
rate_limit         = "1998 °/s"
throttle           = "mid 0.50, hover 0.50, expo 0.00, limit off"
quickrates_rc_expo = "off"
```

Each axis's three numbers depend on `type` (`rates_type`). The CLI names for roll are `roll_rc_rate`, `roll_srate` and `roll_expo`; pitch and yaw are the same.

| `type` | First: `rc_rate` | Second: `srate` | Third: `expo` |
|---|---|---|---|
| `"Betaflight"` | `rc rate 1.00` (CLI 100: App ÷ 100, steps of 0.01) | `rate 0.70` (CLI 70) | `rc expo 0.00` (CLI 0) |
| `"Raceflight"` | `rate 370 °/s` (CLI 37: App ÷ 10, steps of 10 °/s) | `acro+ 80%` (CLI 80) | `expo 50%` (CLI 50) |
| `"KISS"` | `rc rate 1.00` (CLI 100) | `rate 0.70` (CLI 70) | `rc curve 0.00` (CLI 0) |
| `"Actual"` | `center sensitivity 70 °/s` (CLI 7: App ÷ 10) | `max rate 670 °/s` (CLI 67: App ÷ 10) | `expo 0.54` (CLI 54: App × 100) |
| `"Quick"` | `rc rate 1.00` (CLI 100) | `max rate 670 °/s` (CLI 67) | `expo 0.00` (CLI 0) |

The rest:

- `rate_limit`: `roll_rate_limit`, `pitch_rate_limit` and `yaw_rate_limit`, in whole °/s from 200 to 1998. Write one for all three axes, or `"roll 1998, pitch 1998, yaw 1998 °/s"`.
- `throttle`: the App's Throttle MID, Hover Point and Throttle EXPO, from 0.00 to 1.00 (CLI `thr_mid`, `thr_hover` and `thr_expo`, × 100), and the Throttle Limit: `limit off`, `limit scale 80%` or `limit clip 80%` (CLI `throttle_limit_type` and `throttle_limit_percent`, 25% to 100%).
- `quickrates_rc_expo`: `"on"` or `"off"`, as in the CLI.

Angle and Horizon strengths, and the other Flight Mode settings, aren't Rates: in Betaflight 2026.6 they are part of the Tune.

### The inputs, `[inputs]`

```toml
timeline = [
  { at = "0 s", motors = "0%" },
]
```

A Timeline: what happens when. Each value holds until it changes. A Physics or Thrust Stand Scenario scripts the four motor commands, either one for all four (`"50%"`) or four in Betaflight's motor order (rear right, front right, rear left, front left), such as `"100%, 0%, 0%, 0%"`. Each is from 0% to 100%: the ESC's drive, the share of the battery's voltage it puts across the motor, as a DShot throttle value is to Bluejay. (A thrust stand's "throttle" can mean something else: T-Motor's, for one, is a share of its stand's own signal.)

### The Expectations, `[[expect]]`

Each `[[expect]]` is one check, either at a moment or over a stretch of time:

```toml
[[expect]]
what  = "vertical speed"
at    = "1 s"
value = "-9.81 m/s ± 0.00001 m/s"
basis = "rule: speed = g × t = 9.81 m/s² × 1 s, downward"

[[expect]]
what   = "vertical acceleration"
over   = "0 s to 1 s"
lowest = "-9.81 m/s² ± 0.00001 m/s²"
basis  = "rule: ..."
```

- **`what`** is one of:
  - **how the Quad moves:** height, distance east, distance north, vertical speed, speed east, speed north, horizontal speed, speed, vertical acceleration, roll rate, pitch rate, yaw rate, roll, pitch, heading. Up, east, north, rolling right, pitching nose up and yawing nose right are positive. Vertical acceleration is how much the vertical speed changed over the last step, divided by the step's length.
  - **each motor,** written "motor 1 speed" to "motor 4 speed" in Betaflight's motor order, and the same for the rest: **speed** (written in RPM, positive the normal way), **thrust** (along the Quad's up axis, in N or gf, grams of thrust as makers' tables give it), **torque** (the air's drag on its prop, in N·m), **current** (through the motor itself, which sets its torque; at part throttle its ESC draws less than this from the battery, about the drive times this) and **drive** (the share of the battery's voltage its ESC puts across it, in %). **Total thrust** is all four motors' thrust.
  - **the battery:** **battery voltage** (at its terminals, past the connector), **battery current** (drawn from it; negative while braking motors give some back), **battery charge used** (since the start, in mAh) and **battery sag** (how far the voltage sits below the pack's resting voltage at its charge).
- **`at`** a moment, with **`value`**; or **`over`** a stretch, with one of **`mean`**, **`lowest`**, **`highest`** or **`final`**. A stretch covers the state after each step from just after its start up to its end.
- **The value** always has a tolerance: `"± amount"`, `"± percent"` (a share of the value; for a value in percent, percentage points) or `"between X and Y"`.
- **Angles** (roll, pitch, heading) are compared the short way round, so 359.9° and 0.1° are 0.2° apart. So no two angles are more than half a turn apart, and a tolerance a whole turn wide, such as `"0° ± 180°"` or `"between 0° and 360°"`, would accept every angle: it is refused, because it checks nothing.
  - Over a stretch, each angle is taken the short way round from the expected value before the lowest, highest, mean or final is worked out, so a heading that passes north or a roll that passes upside down still reads right ([`slow-turn-through-north.toml`](../../scenarios/physics/slow-turn-through-north.toml) shows it).
  - That only works while the angle moves smoothly and stays less than half a turn from the expected value. If it jumps, or reaches half a turn away, its lowest, highest and mean have no single answer, and the Expectation fails saying so. That happens when the angle sweeps round, as in a roll or a pirouette, and when the nose passes straight up or down, where roll and heading jump by half a turn. Check such an angle at moments, over a shorter stretch, or check its rate; its `final` value still works.
  - Pitch reads from -90° (nose straight down) to 90° (nose straight up). With the nose straight up or down, roll and heading turn about the same line, so only their difference (nose up) or sum (nose down) says anything: then roll reads 0° and heading carries the whole turn. A Quad at roll 30°, nose straight up, heading 45° reads roll 0°, heading 15°, which is the same attitude.
  - "Straight up or down" means within about 0.00000006° of vertical. Turned back into an attitude, the three angles point every part of the Quad the same way to within 1e-12, exactly vertical included. The one exception is a nose inside that band but not exactly vertical: roll still reads 0° there, so the angles are off by up to twice the nose's distance from vertical, at most about 2e-9 (0.0000001°). No tolerance can tell the difference.
  - A stretch in which the nose is inside that band for some steps and outside it for others mixes the two ways of reading roll and heading, for example a Quad that starts there and leaves very slowly. Then roll's and heading's lowest, highest and mean have no single answer either, and the Expectation fails saying so; their `final` value still works. A stretch wholly inside the band, such as a spin about the nose pointing straight up, keeps all four.
- **Compared with another run:** an Expectation may compare this run with the same Scenario run again with one or two starting-state items changed, written in `against` as in `[start]`: `physics_rate`, `battery`, or both. `compare` says how: `"difference"` (this run's value minus the other's, in the measure's unit) or `"ratio"` (this run's value as a share of the other's, in %). The moment or stretch is the same in both runs, so it must be a whole number of steps at both physics rates. Angles can't be compared yet, because they wrap round.

  ```toml
  [[expect]]
  what    = "total thrust"
  at      = "1.9 s"
  against = { battery = "100%" }
  compare = "ratio"
  value   = "between 75.2% and 86.7%"
  basis   = "rule: ..."
  ```

- **`basis`** says where the number comes from: `source:` a cited outside reference, `rule:` worked out from physics with the working shown, or `observed:` what the Simulation did when the Expectation was written. Source and Rule Expectations are locked: if the Simulation disagrees, the Simulation is fixed.

Times are Simulation Time, counted in whole physics steps: at 8 kHz, `"1 s"` is the state after step 8000, and a moment between two steps is refused. The run lasts until the last moment the file mentions.

Every Scenario also gets one automatic check: **no broken numbers**. After every step, every number in the state must be a real number, never "not a number" or endless.

## The Results file

```toml
[[expect]]
what     = "height at 1 s"
basis    = "Rule"
expected = "-4.905 m ± 0.001 m"
measured = "-4.91 m"
```

- Each Expectation's **measured** value, to 3 significant figures, in the unit its expected value uses. The check itself uses the full number: -4.9056 m passes "-4.905 m ± 0.001 m", and the Results show it as -4.91 m.
- The Results are the same on every computer, so a pull request's diff shows every value that moved, even inside its tolerance.
- `[fingerprints]`: short codes that change if anything changes by even one bit.
  - `quad` and `map`: what the Simulation received from the Quad and the Map. If `quad` moved, the Quad definition changed; if `map` moved, the Map's world values or its solid shapes did.
  - `run`: the whole state after every step, in order. Any change to the flight changes it.
  - `[fingerprints.checkpoints]`: the whole state after each tenth of the run, so you can see how far into the run nothing changed.

## Running them

- `cargo scenarios run` runs every Scenario (or the files you name), prints each check as passed or FAILED, and writes the Results files. Commit them with your change.
- `cargo scenarios check` is what CI runs on macOS, Windows and Linux. It runs every Scenario twice and blocks on a failed Expectation, on two runs that differ (the **repeat check**) and on a Results file that is out of date. It writes nothing except, with `--fingerprints <folder>`, the fingerprint after every step.
- `cargo scenarios agree <folder> <folder> …` is the **agreement check**: CI gives it each OS's fingerprints, and on a mismatch it names the Scenario and the first step where the OSes split.

## Units

Every number in a Scenario or Pack file is text with its unit, such as `"9.81 m/s²"`, read by one shared unit list. Our tools always write symbols, but plain-keyboard spellings read the same. [The unit list](../units.md) shows every unit, its plain spellings, how tolerances and ranges are written, and what is refused.
