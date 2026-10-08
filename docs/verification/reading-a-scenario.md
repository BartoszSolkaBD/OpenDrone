# Reading a Scenario and its Results

A **Scenario** is a test you can read without reading code: a starting state, the inputs, and Expectations about what the Simulation does. The terms are in the [Verification deep dive](../context/verification.md). This page walks through one, [`scenarios/physics/free-fall-is-exactly-g.toml`](../../scenarios/physics/free-fall-is-exactly-g.toml), and the Results file the runner writes beside it.

## Where they live

- `scenarios/<topic>/<plain words>.toml`: the Scenarios, grouped by topic: `flight-controller/` for the Flight Controller (alone, or flying a Quad), `physics/`, and `quads/<quad>/` for one Quad's own, such as its Thrust Stand Scenarios.
- `<name>.results.toml`, beside each Scenario: what the last run measured. The runner writes it; never edit it by hand.
- `scenarios/test-quads/`: Test Quads, such as `whoop-65-no-drag.toml`, the Whoop 65 with its drag set to zero, `whoop-65-bench-supply.toml`, the Whoop 65 on the 4.0 V bench supply BetaFPV measured its motor on, `whoop-65-rotor-drag-only.toml`, the Whoop 65 with rotor drag as its only drag, and `freestyle-5-filters-and-shaping-off.toml`, the Freestyle 5″ with the Flight Controller's filters and loop shaping off in its Tune, which the Flight Controller doesn't simulate yet (#49, #50), so Flight Controller Scenarios can check Betaflight's PID loop and mixer on their own. Each says at its top what it changes and why.

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
| `kind` | `"physics"` | One of `"flight"`, `"thrust stand"`, `"flight controller"`, `"physics"`: see [The four kinds](#the-four-kinds) below. |
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

### The four kinds

| `kind` | What runs | Its inputs |
|---|---|---|
| `"flight"` | Our Flight Controller and the physics fly the Quad together. | The pilot's sticks and Arm switch. |
| `"flight controller"` | The Flight Controller alone. | The pilot's sticks and Arm switch, and the sensor readings: a Timeline, or a table of cases. |
| `"physics"` | The physics alone, with scripted motors in place of the Flight Controller. | Motor commands. |
| `"thrust stand"` | The physics, with the Quad held still: its `speed` and `rotation` must be zero, while its motors, ESCs and battery work as in flight. | Motor commands. |

Where our Flight Controller runs:

- **The Quad's Tune must spell out every setting the Flight Controller reads** ([ADR-0015](../adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)). Both built-in Quads' do: the Freestyle 5″'s holds Betaflight 2026.6.2's defaults, and the Whoop 65's is imported from the Meteor65 Pro's `diff all` ([Importing a Tune from Betaflight](importing-a-tune.md)). A Scenario that flies a Quad whose Tune lacks one is refused, naming what it lacks.
- **It flies Acro so far.** `flight_mode = "Angle"` or `"Horizon"` waits for #51, and Input smoothing (#56) and Endless Battery (#57) must be `"off"` until their tickets. Auto-arm runs in Flight Scenarios (see [Auto-arm](#auto-arm) below); a Flight Controller Scenario leaves it out, so there it must be `"off"`.
- **A Flight Scenario starts mid-air or landed.** Mid-air, its motors are `"settled"`. Landed, a "fresh" Flight Controller is exactly Reset: its motors start `"powering up"`, so its ESCs play their start-up tones and arming waits for their ready beep, about 1.7 s, and it starts as Reset leaves the Quad, with `armed = false` and no `speed` or `rotation`. That start is the Scenario's Launch Spot, where Reset puts the Quad back.
- **`armed = true`** starts the Flight Controller armed, as a mid-air start needs; the Timeline's Arm switch then holds it armed, or disarms it.

A Flight Controller Scenario runs the Flight Controller alone, so its `[start]` leaves out `map`, `position`, `speed`, `motors` and `battery`: it has no Map, place, motors or battery. Its `attitude` and `rotation` are the sensor readings it starts with: the attitude, and the gyro.

### How the motors start

| `motors` | The motors | Their ESCs |
|---|---|---|
| `"powering up"` | Stopped. | Just powered, as when a pack is plugged in: they play Bluejay's start-up melody, a "signal found" beep and a ready beep, and answer no command until they are ready, about 1.66 s later. They beep ready only once the throttle has been at 0% for ten counts of their 32 ms timer, so a command above 0% before then holds them back. |
| `"stopped"` | Stopped. | Already powered up and ready: each starts its motor on its first command above 0%, after the start wait. |
| `"settled"` | Spinning, all at the speed that holds the Quad's height at the stated motion: the thrust's upward part, with the air's push at that motion, carries the weight. Tilted, that takes more thrust than the weight, and the thrust's sideways part holds the speed only at the speed where the air's drag matches it, so a tilted start that should hold its speed states that speed. Never more than full drive (tilted further than full drive can hold, full drive), and none upside down or past its side. | Running. |

`"stopped"` is only for Physics and Thrust Stand Scenarios, which script their motors, and a Quad held still on the thrust stand has no motion for `"settled"` motors to hold. Where the Flight Controller runs, a start at rest with a "fresh" Flight Controller is exactly Reset, whose ESCs power up first: a Flight Scenario writes it `"powering up"`, disarmed and still.

Each ESC copies Bluejay v0.21.0 ([`crates/physics/src/esc.rs`](../../crates/physics/src/esc.rs) has the timing, with the firmware's file and line for each step):

- **Starting a stopped motor:** on the first command above 0%, a ready ESC waits the Quad's start wait (0.1 s), then starts the motor with its drive held at the Quad's start-up power limit (1.96%) for 15 electrical turns (Bluejay's 24 start-up commutations, then its initial-run countdown of 12 turns, which starts on the fourth), and then runs it as commanded.
- **Stopping:** at 0% it brakes the motor, and below Bluejay's minimum speed, about 1,330 electrical RPM, switches it off and is ready again.
- **A stall and its restarts:** a running motor told to spin that falls below the minimum speed, or is turned backwards, has stalled: a Prop Strike can stop it, or a jammed prop hold it. The ESC switches it off for the start wait (0.1 s), then restarts it as it starts a stopped one, capped at the start-up power limit. A start whose prop doesn't turn steps through its 15 turns at Bluejay's timeouts' pace, waits once more for its prop to answer, and fails after 1.711 s. That takes Bluejay's comparator, which has no back-voltage to read, never to show the level Bluejay waits for; what a real one shows is unknown, and a try could then take anywhere from about 1.0 s to 1.724 s (the esc module has the working), so Scenarios check the moments around a failed start with room for all of that. Bluejay counts failed starts in a row: after 3 (the Quad's `restart_tries`) it plays its "motor stalled" beeps and keeps the motor off, "stopped after failed restarts", until the command has been 0% for ten counts of its 32 ms timer and it has beeped ready again, as a disarm does. A running motor that stalls gets 3 restarts; a motor jammed from a standstill fails its first start, so it gets 2.

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
| `test/block` | The ground and a small block, 2 cm square and 2.5 cm tall, its middle 11 cm east and 11 cm north of the origin: where a 5″ landed level at the origin, nose north, has its front right prop. |

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

A Flight or Flight Controller Scenario scripts the pilot's sticks and the Arm switch instead, as in [`full-right-roll-reaches-the-max-rate.toml`](../../scenarios/flight-controller/full-right-roll-reaches-the-max-rate.toml):

```toml
timeline = [
  { at = "0 s",   roll = "0%", pitch = "0%", yaw = "0%", throttle = "30%", arm = "on" },
  { at = "1 s",   roll = "100%" },
  { at = "1.5 s", roll = "0%" },
]
```

- **The sticks are in percent:** `roll`, `pitch` and `yaw` from -100% to 100% (right, forward and right are positive, so pitch forward asks for nose down), `throttle` from 0% to 100%.
- **`arm`** is the Arm switch on AUX1: `"on"` (high, 2012 µs) or `"off"` (988 µs). Flight Mode on AUX2 comes from `flight_mode` in `[start]`, as the pilot's setting drives it when no switch is bound; Crash Flip on AUX3 is off.
- **The first moment, at 0 s, sets every stick and the Arm switch,** so the run starts from values the file states.
- **`"ramp to 60%"`** moves a stick in a straight line, step by step, from the value and moment an earlier entry set it to this value at this moment. A ramp needs an earlier value to ramp from.
- **A Flight Controller Scenario's Timeline** may also change the sensor readings from a moment on: `rotation` (the gyro, written as in `[start]`) and `attitude`. Its ESCs count as ready: it has none to wait for.

Besides the Channels, a Timeline sends the other Flight Inputs, as in [`failsafe.toml`](../../scenarios/flight-controller/failsafe.toml) and [`arming-waits-for-the-escs-after-reset.toml`](../../scenarios/flight-controller/arming-waits-for-the-escs-after-reset.toml):

```toml
timeline = [
  { at = "0 s",   roll = "0%", pitch = "0%", yaw = "0%", throttle = "0%", arm = "off" },
  { at = "1 s",   input_device = "lost" },
  { at = "4 s",   input_device = "back" },
  { at = "6 s",   radio_link = "drops out for 0.2 s" },
  { at = "8 s",   reset = true },
]
```

- **`input_device = "lost"`** and **`"back"`**: the Flying Input Device unplugged and plugged back in. While it is lost the Radio Link sends no frames, and the Flight Controller's Failsafe follows; once it is back, the next frame on the link's own beat carries the newest Channels. Lost and back take turns, starting with lost.
- **`radio_link = "drops out for 0.2 s"`**: a Radio Link drop-out, the frames stopping for a while. It is the device lost at that moment and back that long after, so it is written in whole physics steps.
- **`reset = true`**, in a Flight Scenario: Reset, as the pilot presses it. The Quad goes back on the Launch Spot, landed and disarmed, with a full battery, a fresh Flight Controller and its ESCs starting up, so arming waits for their ready beep again. A Scenario's Launch Spot is where it starts, so only one that starts as Reset leaves the Quad (motors `"powering up"`) may press it. The Radio Link is the pilot's radio, so it keeps its beat.

#### Auto-arm

With `auto_arm = "on"` in a Flight Scenario, as in [`auto-arm.toml`](../../scenarios/flight-controller/auto-arm.toml), the Quad arms on the first throttle raise from low (below `min_check`) that passes Betaflight's arming checks, and only Reset or a Failsafe drop disarms it. No Arm switch is bound then (it would take over), so `arm` stays `"off"` all through the Timeline. Auto-arm drives the Arm switch in each Radio Link frame itself, through Betaflight's own arming, which arms only on a frame with the throttle low, one frame after the switch goes on: so it turns Arm on with the frame that brings the raise and holds the throttle at the bottom in that frame and the next. The raise reaches the Flight Controller two frames late, 8 ms at 250 Hz.

How the sticks reach the Flight Controller, as on a real quad on ExpressLRS:

1. **Each stick becomes a whole-number Channel:** the step an ELRS receiver hands over CRSF, from 172 (-100%, 988 µs) through 992 (centre, 1500 µs) to 1811 (+100%, 2012 µs). The runner rounds each percent to the nearest step: 50% is 1500 + 512 × 0.5 = 1756 µs, step 1401. Whenever a Channel changes, it enters the Simulation as a Flight Input, stamped with Simulation Time.
2. **The Radio Link** sends a frame with the newest Channels at the Packet Rate (`radio_link`), the first at 0 s: every 4 ms at 250 Hz. A frame due between two physics steps leaves on the later one.
3. **The Flight Controller** reads each step as Betaflight 2026.6 reads CRSF: 0.62477 × step + 881 µs. So a centred stick reads 1500.77 µs, not 1500 µs, and asks for a little rotation (0.11 °/s on Actual 70/670), and 50% stick reads 1756.30 µs, a deflection of 0.5126. Full stick reads 2012.46 µs, beyond 500 µs from centre, so it is exactly full. [`actual-rates.toml`](../../scenarios/flight-controller/actual-rates.toml) has the table.
4. **One Flight Controller loop runs per physics step,** reading the gyro (the Quad's true rotation) and the true attitude at the step's start.

### A table of cases, `[[case]]`

A Flight Controller Scenario may be fed a table of cases instead of a Timeline, as in [`mixer-and-airmode.toml`](../../scenarios/flight-controller/mixer-and-airmode.toml). Each case is a fresh Flight Controller (armed or not, as `[start]` says), one Radio Link frame with the case's sticks and Arm switch, and one loop with its sensor readings; each `[[case.expect]]` is measured after that loop:

```toml
[[case]]
roll     = "0%"
pitch    = "0%"
yaw      = "0%"
throttle = "0%"
arm      = "on"
rotation = "roll -100 °/s, pitch 0 °/s, yaw 0 °/s"

[[case.expect]]
what  = "roll PID sum"
value = "144.53 ± 0.01"
basis = "source: ..."
```

A case sets every stick and the Arm switch; `rotation` and `attitude` are optional, and otherwise `[start]`'s. A case is one loop, so nothing in it ramps. Its Expectations say only `what`, `value` and `basis`.

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
  - **how the Quad moves:** height, distance east, distance north, vertical speed, speed east, speed north, horizontal speed, speed, vertical acceleration, acceleration east, acceleration north, roll rate, pitch rate, yaw rate, roll, pitch, heading. Up, east, north, rolling right, pitching nose up and yawing nose right are positive. Vertical acceleration is how much the vertical speed changed over the last step, divided by the step's length, and acceleration east and north the same for the speeds east and north; none of the three can be measured at 0 s.
  - **what the gyro reads:** gyro roll rate, gyro pitch rate and gyro yaw rate: the roll, pitch and yaw rates, each clipped at the board's gyro range (±2,000 °/s on both alpha Quads).
  - **each motor,** written "motor 1 speed" to "motor 4 speed" in Betaflight's motor order, and the same for the rest: **speed** (written in RPM, positive the normal way), **thrust** (along the Quad's up axis, in N or gf, grams of thrust as makers' tables give it, over the last step: in flight, in the air the rotor moved through, so it falls in a climb and rises in a descent and at speed; on the thrust stand, in still air), **torque** (the air's drag on its prop, in N·m), **current** (through the motor itself, which sets its torque; at part throttle its ESC draws less than this from the battery, about the drive times this) and **drive** (the share of the battery's voltage its ESC puts across it, in %) and **restarts** (how many times its ESC has restarted it since it last ran properly or stopped at 0%, a plain number). **Total thrust** is all four motors' thrust.
  - **each prop,** written "prop 1 rub" to "prop 4 rub": how hard it rubbed the Map during the last step (in N), the friction at its disc; 0 when it touches nothing.
  - **the battery:** **battery voltage** (at its terminals, past the connector), **battery current** (drawn from it; negative while braking motors give some back), **battery charge used** (since the start, in mAh) and **battery sag** (how far the voltage sits below the pack's resting voltage at its charge).
  - **what our Flight Controller's loop did** (only where it runs, and not at 0 s, before its first loop): **roll setpoint**, **pitch setpoint** and **yaw setpoint**, the rotation speed the Rates ask for, before any smoothing (Betaflight's raw setpoint), in °/s; **roll P term**, **roll I term**, **roll D term** and **roll PID sum** (and the same for pitch and yaw), plain numbers on Betaflight's scale, where 1000 is the whole motor range, as Blackbox shows them; **motor 1 DShot** to **motor 4 DShot**, the DShot value it sends each motor's ESC: 0 is "stop", 48 to 2047 the throttle, 158 the Freestyle 5″'s idle; and **mixer throttle**, the throttle the mixer starts from, from 0% to 100%, before Airmode moves it (Blackbox's throttle), which shows the pilot's throttle through the throttle curve whatever the PID loop does. Setpoints and terms are signed the pilot's way: rolling right, pitching nose up and yawing nose right are positive, so a positive pitch term pushes the nose up. A Flight Controller Scenario measures only these.
  - **something that happens** (only where our Flight Controller runs, and not in a table of cases): **the Quad arms**, **the Quad disarms**, **Failsafe's stage 2 starts** (with DROP, the Quad disarms), **Failsafe ends**, **the ESCs are ready** (every ESC has played its ready beep; only in a Flight Scenario), and each of Betaflight's reasons for refusing to arm, **FAILSAFE**, **RXLOSS**, **NOT_DISARMED**, **THROTTLE**, **ANGLE**, **BOOTGRACE** and **ARM_SWITCH**, written "RXLOSS blocks arming" or "RXLOSS stops blocking arming". See [When something happens](#when-something-happens) below.
- **`at`** a moment, with **`value`**; or **`over`** a stretch, with one of **`mean`**, **`lowest`**, **`highest`** or **`final`** (or **`first`**, for something that happens). A stretch covers the state after each step from just after its start up to its end.
- **The value** always has a tolerance: `"± amount"`, `"± percent"` (a share of the value; for a value in percent, percentage points) or `"between X and Y"`.
- **Angles** (roll, pitch, heading) are compared the short way round, so 359.9° and 0.1° are 0.2° apart. So no two angles are more than half a turn apart, and a tolerance a whole turn wide, such as `"0° ± 180°"` or `"between 0° and 360°"`, would accept every angle: it is refused, because it checks nothing.
  - Over a stretch, each angle is taken the short way round from the expected value before the lowest, highest, mean or final is worked out, so a heading that passes north or a roll that passes upside down still reads right ([`slow-turn-through-north.toml`](../../scenarios/physics/slow-turn-through-north.toml) shows it).
  - That only works while the angle moves smoothly and stays less than half a turn from the expected value. If it jumps, or reaches half a turn away, its lowest, highest and mean have no single answer, and the Expectation fails saying so. That happens when the angle sweeps round, as in a roll or a pirouette, and when the nose passes straight up or down, where roll and heading jump by half a turn. Check such an angle at moments, over a shorter stretch, or check its rate; its `final` value still works.
  - Pitch reads from -90° (nose straight down) to 90° (nose straight up). With the nose straight up or down, roll and heading turn about the same line, so only their difference (nose up) or sum (nose down) says anything: then roll reads 0° and heading carries the whole turn. A Quad at roll 30°, nose straight up, heading 45° reads roll 0°, heading 15°, which is the same attitude.
  - "Straight up or down" means within about 0.00000006° of vertical. Turned back into an attitude, the three angles point every part of the Quad the same way to within 1e-12, exactly vertical included. The one exception is a nose inside that band but not exactly vertical: roll still reads 0° there, so the angles are off by up to twice the nose's distance from vertical, at most about 2e-9 (0.0000001°). No tolerance can tell the difference.
  - A stretch in which the nose is inside that band for some steps and outside it for others mixes the two ways of reading roll and heading, for example a Quad that starts there and leaves very slowly. Then roll's and heading's lowest, highest and mean have no single answer either, and the Expectation fails saying so; their `final` value still works. A stretch wholly inside the band, such as a spin about the nose pointing straight up, keeps all four.
- **Compared with another run:** an Expectation may compare this run with the same Scenario run again with one or two starting-state items changed, written in `against` as in `[start]`: `physics_rate`, `battery`, or both. `compare` says how: `"difference"` (this run's value minus the other's, in the measure's unit) or `"ratio"` (this run's value as a share of the other's, in %). The moment or stretch is the same in both runs, so it must be a whole number of steps at both physics rates. Angles can't be compared yet, because they wrap round.
  - A Flight Scenario may also compare with the same flight on mirrored sticks: `against = { sticks = "mirrored" }` turns roll and yaw the other way. The start must be its own mirror image (no roll, no roll or yaw rotation, no speed sideways to the heading), as in [`mirrored-sticks.toml`](../../scenarios/flight-controller/mirrored-sticks.toml).

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

### When something happens

Something that happens, such as the Quad disarming, is checked over a stretch with **`first`**: how long after the stretch's start it first happens, or **`"never"`** if it mustn't happen in the stretch at all, as in [`failsafe.toml`](../../scenarios/flight-controller/failsafe.toml):

```toml
[[expect]]
what  = "the Quad disarms"
over  = "1 s to 3 s"
first = "between 1.497 s and 1.5082 s"
basis = "source: ..."

[[expect]]
what  = "the Quad arms"
over  = "4 s to 5.4 s"
first = "never"
basis = "source: ..."
```

- It happens between two steps: on the first step whose state shows it, so the stretch's start itself never counts. A Flight Controller loop's state is the one after its 125 µs step (at 8 kHz), so a frame that arms the Quad at 2.004 s shows at 2.004125 s, 0.004125 s after a stretch starting at 2 s.
- If it doesn't happen in the stretch, a time fails, measured "never"; if it mustn't and does, `"never"` fails, measured "after" the time it did.
- The reasons for refusing to arm are raised and cleared as Betaflight does it: RXLOSS and FAILSAFE whether armed or not, the others only while disarmed. So while the Quad is armed, THROTTLE, ANGLE, BOOTGRACE, NOT_DISARMED and ARM_SWITCH stay as they were when it armed: clear.

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
  - `quad` and `map`: what the Simulation received from the Quad and the Map. If `quad` moved, the Quad definition or its Tune changed; if `map` moved, the Map's world values or its solid shapes did. A Flight Controller Scenario has no Map: `map = "none"`.
  - `run`: the whole state after every step, in order, the Radio Link and the Flight Controller's memory included. Any change to the flight changes it.
  - `[fingerprints.checkpoints]`: the whole state after each tenth of the run, so you can see how far into the run nothing changed. For a table of cases, the Flight Controller's whole state after each case.

## Running them

- `cargo scenarios run` runs every Scenario (or the files you name), prints each check as passed or FAILED, and writes the Results files. Commit them with your change.
- `cargo scenarios check` is what CI runs on macOS, Windows and Linux. It runs every Scenario twice and blocks on a failed Expectation, on two runs that differ (the **repeat check**) and on a Results file that is out of date. It writes nothing except, with `--fingerprints <folder>`, the fingerprint after every step.
- `cargo scenarios agree <folder> <folder> …` is the **agreement check**: CI gives it each OS's fingerprints, and on a mismatch it names the Scenario and the first step where the OSes split.

## Units

Every number in a Scenario or Pack file is text with its unit, such as `"9.81 m/s²"`, read by one shared unit list. Our tools always write symbols, but plain-keyboard spellings read the same. [The unit list](../units.md) shows every unit, its plain spellings, how tolerances and ranges are written, and what is refused.
