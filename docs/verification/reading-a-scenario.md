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
| `motors` | `"stopped"` | `"stopped"`, or `"settled"`: spinning at the speed that holds the stated motion (needs the motor model, which comes later). |
| `flight_controller` | `"fresh"` | As right after Reset powers it up. |
| `battery` | `"100%"` | The charge. |
| `flight_mode` | `"Acro"` | `"Acro"`, `"Angle"` or `"Horizon"`. |
| `assists` | `{ input_smoothing = "off", endless_battery = "off", auto_arm = "off" }` | Every Assist, on or off. |
| `radio_link` | `"250 Hz"` | The Packet Rate: 50, 100, 150, 250, 333, 500 or 1000 Hz. |
| `physics_rate` | `"8 kHz"` | Physics steps a second. |
| `random_seed` | `1` | The seed for the Simulation's random numbers. |
| `[start.rates]` | `type = "Actual"`, `roll = "70 / 670 / 0"`, … | Every Rates field: the type, each axis's three numbers as Betaflight's rate profile holds them (for Actual: centre °/s, max °/s, expo), the rate limit and the throttle curve (`"mid 50%, expo 0, limit off"`). |

In a Physics Scenario the Flight Controller doesn't run, so `armed` down to the Rates change nothing. They are written down all the same, so the format never needs them added later.

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
- **The value** always has a tolerance: `"± amount"`, `"± percent"` (a share of the value; for a value in percent, percentage points) or `"between X and Y"`. Angles are compared the short way round, so 359.9° and 0.1° are 0.2° apart.
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

Every number in a Scenario or Pack file is text with its unit, read by one shared unit list (`crates/pack/src/units.rs`). Our tools always write symbols, but plain-keyboard spellings read the same.

| Kind | Units | Plain spellings |
|---|---|---|
| Length | `m`, `km`, `cm`, `mm` | |
| Mass | `g`, `kg`, `mg` | |
| Time | `s`, `ms`, `µs`, `min`, `h` | `us` for `µs` |
| Angle | `°` | `deg` |
| Rotation speed | `°/s`, `RPM` | `deg/s`, `rpm` |
| Frequency, rate | `Hz`, `kHz`, `MHz`, `s⁻¹` | `hz`, `s^-1`, `1/s` |
| Electrical | `V`, `mV`, `A`, `mA`, `Ω`, `mΩ`, `kΩ`, `mAh`, `Ah`, `W`, `mW`, `kW`, `Wh`, `KV` | `ohm` for `Ω`, `kv` for `KV` |
| Force | `N`, `mN` | |
| Camera and sound | `stops`, `lines`, `TVL`, `dB` | `db` |
| Percent | `%` | |
| Combinations | such as `m/s`, `m/s²`, `kg·m²`, `g·cm²`, `cm²`, `kg/m³` | `*` or a space for `·`, `^2` for `²` |

- Use whichever everyday prefix keeps the number ordinary: "140 g·cm²", not "0.000014 kg·m²".
- A decimal point only: "31,2 g" and "2,000 °/s" are refused, with the fix.
- Never radians: angles are degrees and spin speeds are °/s or RPM.
- A value with labelled parts writes its unit once at the end, if it likes: "roll 70, pitch 90, yaw 140 g·cm²".
