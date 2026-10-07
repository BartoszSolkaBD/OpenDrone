# Reading a Scenario and its Results

A **Scenario** is a test you can read without reading code: a starting state, the inputs, and Expectations about what the Simulation does. The terms are in the [Verification deep dive](../context/verification.md). This page walks through one, [`scenarios/physics/free-fall-is-exactly-g.toml`](../../scenarios/physics/free-fall-is-exactly-g.toml), and the Results file the runner writes beside it.

## Where they live

- `scenarios/<topic>/<plain words>.toml`: the Scenarios, grouped by topic, such as `physics/`.
- `<name>.results.toml`, beside each Scenario: what the last run measured. The runner writes it; never edit it by hand.
- `scenarios/test-quads/`: Test Quads, such as `whoop-65-no-drag.toml`, the Whoop 65 with its drag set to zero.

## The Scenario file

```toml
format = 1
name   = "Free fall is exactly g"
```

`format` is the file format's version, so an older OpenDrone refuses a newer file. `name` says what the Scenario proves.

### The starting state, `[start]`

It spells out every item that affects the Simulation, every time, with no hidden defaults ([ADR-0002](../adr/0002-scenario-starting-state-spells-out-everything.md)). A file that leaves one out is refused.

| Item | Example | Meaning |
|---|---|---|
| `kind` | `"physics"` | One of `"flight"`, `"thrust stand"`, `"flight controller"`, `"physics"`. So far only Physics Scenarios run: scripted motors stand in for the Flight Controller. |
| `quad` | `"test/whoop-65-no-drag"` | The Quad, by id. Its numbers come from its Quad definition, never from the Scenario. |
| `map` | `"test/empty-air"` | The Map, by id. Gravity and air density come from the Map. `test/empty-air` is built into the code: 9.81 m/s² and 1.225 kg/m³, with nothing to hit. |
| `position` | `"0 m east, 0 m north, 0 m up"` | From the Map's origin. |
| `attitude` | `"level, heading 0°"` or `"roll 0°, pitch 30°, heading 45°"` | Heading is a compass heading (0° north, 90° east). Pitch is nose up, roll is right side down. |
| `speed` | `"0 m/s"` | East, north and up, such as `"5 m/s north, 0 m/s east, 0 m/s up"`. A single number must be zero. |
| `rotation` | `"roll 2000 °/s, pitch 0 °/s, yaw 0 °/s"` | Rolling right, pitching nose up and yawing nose right are positive, as in Betaflight. |
| `armed` | `false` | Whether the Quad is armed. |
| `motors` | `"stopped"` | `"stopped"`: at rest, with the ESCs already powered up and ready, so a motor starts on its first command; only for Physics and Thrust Stand Scenarios, which script their motors. Or `"settled"`: spinning at the speed that holds the stated motion, with the ESCs running (needs the motor model, which comes later). Neither is Reset: a landed start with a "fresh" Flight Controller is exactly Reset, so its ESCs play their start-up first, about 1.7 s. |
| `flight_controller` | `"fresh"` | As right after Reset powers it up. |
| `battery` | `"100%"` | The charge. |
| `flight_mode` | `"Acro"` | `"Acro"`, `"Angle"` or `"Horizon"`. |
| `assists` | `{ input_smoothing = "off", endless_battery = "off", auto_arm = "off" }` | Every Assist, on or off. |
| `radio_link` | `"250 Hz"` | The Packet Rate: 50, 100, 150, 250, 333, 500 or 1000 Hz. |
| `physics_rate` | `"8 kHz"` | Physics steps a second. |
| `random_seed` | `1` | The seed for the Simulation's random numbers. |
| `[start.rates]` | `type = "Actual"`, `roll = "center sensitivity 70 °/s, max rate 670 °/s, expo 0.00"`, … | Every field of a Betaflight 2026.6 rate profile, written as the Betaflight App shows it: see [The Rates](#the-rates) below. |

In a Physics Scenario the Flight Controller doesn't run, so `armed` down to the Rates change nothing. They are written down all the same, so the format never needs them added later.

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

A Timeline: what happens when. Each value holds until it changes. A Physics Scenario scripts the four motor commands, either one for all four (`"0%"`) or four in Betaflight's motor order. Until the motor model arrives, every command must be 0%.

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

- **`what`** is one of: height, distance east, distance north, vertical speed, horizontal speed, speed, vertical acceleration, roll rate, pitch rate, yaw rate, roll, pitch, heading. Up, rolling right, pitching nose up and yawing nose right are positive. Vertical acceleration is how much the vertical speed changed over the last step, divided by the step's length.
- **`at`** a moment, with **`value`**; or **`over`** a stretch, with one of **`mean`**, **`lowest`**, **`highest`** or **`final`**. A stretch covers the state after each step from just after its start up to its end.
- **The value** always has a tolerance: `"± amount"`, `"± percent"` (a share of the value; for a value in percent, percentage points) or `"between X and Y"`.
- **Angles** (roll, pitch, heading) are compared the short way round, so 359.9° and 0.1° are 0.2° apart. So no two angles are more than half a turn apart, and a tolerance a whole turn wide, such as `"0° ± 180°"` or `"between 0° and 360°"`, would accept every angle: it is refused, because it checks nothing.
  - Over a stretch, each angle is taken the short way round from the expected value before the lowest, highest, mean or final is worked out, so a heading that passes north or a roll that passes upside down still reads right ([`slow-turn-through-north.toml`](../../scenarios/physics/slow-turn-through-north.toml) shows it).
  - That only works while the angle moves smoothly and stays less than half a turn from the expected value. If it jumps, or reaches half a turn away, its lowest, highest and mean have no single answer, and the Expectation fails saying so. That happens when the angle sweeps round, as in a roll or a pirouette, and when the nose passes straight up or down, where roll and heading jump by half a turn. Check such an angle at moments, over a shorter stretch, or check its rate; its `final` value still works.
  - Pitch reads from -90° (nose straight down) to 90° (nose straight up). With the nose straight up or down, roll and heading turn about the same line, so only their difference (nose up) or sum (nose down) says anything: then roll reads 0° and heading carries the whole turn. A Quad at roll 30°, nose straight up, heading 45° reads roll 0°, heading 15°, which is the same attitude.
  - "Straight up or down" means within about 0.00000006° of vertical. Turned back into an attitude, the three angles point every part of the Quad the same way to within 1e-12, exactly vertical included. The one exception is a nose inside that band but not exactly vertical: roll still reads 0° there, so the angles are off by up to twice the nose's distance from vertical, at most about 2e-9 (0.0000001°). No tolerance can tell the difference.
  - A stretch in which the nose is inside that band for some steps and outside it for others mixes the two ways of reading roll and heading, for example a Quad that starts there and leaves very slowly. Then roll's and heading's lowest, highest and mean have no single answer either, and the Expectation fails saying so; their `final` value still works. A stretch wholly inside the band, such as a spin about the nose pointing straight up, keeps all four.
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
  - `quad` and `map`: what the Simulation received from the Quad and the Map. If `quad` moved, the Quad definition changed.
  - `run`: the whole state after every step, in order. Any change to the flight changes it.
  - `[fingerprints.checkpoints]`: the whole state after each tenth of the run, so you can see how far into the run nothing changed.

## Running them

- `cargo scenarios run` runs every Scenario (or the files you name), prints each check as passed or FAILED, and writes the Results files. Commit them with your change.
- `cargo scenarios check` is what CI runs on macOS, Windows and Linux. It runs every Scenario twice and blocks on a failed Expectation, on two runs that differ (the **repeat check**) and on a Results file that is out of date. It writes nothing except, with `--fingerprints <folder>`, the fingerprint after every step.
- `cargo scenarios agree <folder> <folder> …` is the **agreement check**: CI gives it each OS's fingerprints, and on a mismatch it names the Scenario and the first step where the OSes split.

## Units

Every number in a Scenario or Pack file is text with its unit, such as `"9.81 m/s²"`, read by one shared unit list. Our tools always write symbols, but plain-keyboard spellings read the same. [The unit list](../units.md) shows every unit, its plain spellings, how tolerances and ranges are written, and what is refused.
