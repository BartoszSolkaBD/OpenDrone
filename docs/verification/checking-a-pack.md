# Checking a Pack

A **Pack** is a folder of data and assets, never code: Quads, Maps and Input Device profiles ([ADR-0011](../adr/0011-packs-are-data-only-toml-named-pack-item.md)). The **Pack checker** reads a Pack the way the game will, and refuses anything that breaks the rules on this page. The game's own content, `packs/opendrone/`, goes through the same checker as a Pack a pilot drops in. The terms are in the [World and content](../context/world.md), [Flying](../context/flying.md) and [Verification](../context/verification.md) deep dives.

So far the checker reads manifests and Quads. Maps and Input Device profiles arrive with their own tickets.

## Running it

```sh
cargo xtask packs                           # every Pack in packs/ and every Test Quad
cargo xtask feel-tests --base origin/main   # the Feel Test log rules, against main
```

CI runs both on every pull request, in the Rust job on each OS, and any problem blocks the merge. There, the base is `HEAD^1`: the pull request's base branch.

Every problem names its file, its line and a plain sentence, and all of them are listed at once:

```text
- packs/opendrone/quads/whoop-65/quad.toml line 20: "23,0 g" isn't a number OpenDrone can read: numbers take a decimal point, so write "23.0 g"
```

A broken item is skipped and the rest of its Pack still loads. A broken manifest skips the whole Pack.

## A Pack's folder

```text
packs/opendrone/                 the Pack; its id comes from pack.toml
  pack.toml                      the manifest
  quads/whoop-65/                one Quad: its id is opendrone/whoop-65
    quad.toml                    the Quad definition
    tune.txt                     its Tune
    picture.png                  for the Quad picker
    feel-tests.md                its Feel Test log
  maps/…                         Maps (not read yet)
  input-devices/…                Input Device profiles (not read yet)
```

- Items are found by their folder, so there's no list to keep in step. Each is named `<pack>/<item>`, such as `opendrone/whoop-65`.
- Ids are lowercase words joined by dashes. The id `test` is kept for Test Quads and Test Maps.
- Two Packs can't share an id: the second is skipped.
- A folder of a kind the game doesn't know, such as `modifiers/`, is skipped and reported.

## The manifest, `pack.toml`

```toml
format      = 1
id          = "opendrone"
name        = "OpenDrone"
description = "The Quads, Maps and Input Devices that come with the game."
version     = "0.1"
author      = "OpenDrone contributors"
licence     = "CC0-1.0"

[licences]   # optional: files under another licence, by path
"maps/harbour/textures/**" = { licence = "CC-BY-4.0", credit = "Rust texture by Jane Doe, example.com" }
```

Every key is required except `[licences]`, and nothing else may appear. Licences are standard SPDX names. Anything under a CC BY licence needs a credit line.

## A Quad definition, `quad.toml`

The two built-in ones are full examples: [the Whoop 65](../../packs/opendrone/quads/whoop-65/quad.toml) and [the Freestyle 5″](../../packs/opendrone/quads/freestyle-5/quad.toml). Where each of their numbers comes from is in [the research note](../research/quad-definitions.md).

**Physics numbers and the FPV Camera's limits** carry a Confidence and a source:

```toml
dry_mass   = { value = "23.0 g", confidence = "Manufacturer", source = "betafpv" }
rotor_drag = { value = "0.3 s⁻¹", confidence = "Estimate", range = "0.1–0.6 s⁻¹", source = "first-guess" }
```

- **Confidence** is Measured, Manufacturer, Derived (arithmetic on Measured or Manufacturer numbers) or Estimate.
- **An Estimate needs a range,** absolute (`"0.1–0.6 s⁻¹"`) or relative to where it started (`"×0.5–×2"`), and its value must lie inside an absolute one. Only an Estimate has a range.
- **The source** is a key from the file's `[sources]` list at the end, which says where the number comes from: a web page with the date it was read, a repo file, a research note's section, or the Feel Test log.

**Counts, choices, camera defaults, the sound block and on-screen text carry no Confidence.** A count is a bare whole number, such as `blades = 3`; everything else is text in quotes, numbers with their units.

Every number has a unit from the [shared unit list](../units.md). A few values have a form of their own:

- labelled parts: `"roll 70, pitch 90, yaw 140 g·cm²"`, with directions as forward, left and up from the centre of mass;
- a box: `"box 64 × 10 × 6 mm"`, front to back, side to side, top to bottom;
- a value at a condition: `"1.2 A at 10 V"`;
- a curve, as one line of points: `"4.35 V at 100%, 3.92 V at 50%, 3.30 V at 0%"`.

### What a Quad definition holds

At the top: `format`, the on-screen `name`, `description` and `spec_line`, and the `picture`, a PNG in the Quad's own folder. Then these sections, in this order. Unknown keys and sections are refused, and so is `based_on`, which only Test Quads use.

| Section | Keys | Notes |
|---|---|---|
| `[camera]` | `lens`, `position`, `camera_tilt` (0–80°), `fov` (90–170°, corner to corner), `vtx_power`; and the limits `analog_dynamic_range`, `analog_lines`, `analog_sharpness` | The first five are camera defaults, with no Confidence. The limits carry one. |
| `[frame]` | `dry_mass`, `diagonal`, `inertia` (roll, pitch, yaw), `rotor_height` (the props' plane above the centre of mass), `drag_area` (front, side, top) | The dry mass excludes the battery. |
| `[collision]` | `body` and `pack` (boxes), `pack_height` (the pack's centre above the centre of mass), `duct_rings` (inside, wall, tall), `bounce` (0–1), `friction` | `duct_rings` goes with `[ducts]`, and only with it. |
| `[props]` | `diameter`, `blades` (a count), `direction` (`"props-in"` or `"props-out"`), `thrust_coefficient`, `power_coefficient`, `rotor_drag`, `rotor_inertia`, `reverse_thrust`, `reverse_torque` (shares of forward), `grip` | The coefficients are unitless, as propeller test data publishes them. |
| `[motors]` | `kv`, `poles` (a count, even), `winding_resistance`, `no_load_current` (`"… A at … V"`), `spin_up`, `slow_down`, and the ESC's `start_wait`, `restart_tries` (a count) and `startup_power_limit` | |
| `[battery]` | `cells` (a count), `chemistry` (`"LiPo"`, `"LiHV"` or `"Li-ion"`), `full`, `empty`, `capacity`, `mass`, `voltage_curve`, `resistance`, `recovery`, `connector` | Voltages are per cell. The pack's mass is stored apart from the dry mass, and the checker adds them, so a heavier pack is never counted twice. The curve runs from `full` at 100% down to `empty` at 0%. |
| `[ducts]` | `ram_drag`, `nose_up_offset` | Only on a Quad with ducts. |
| `[feel]` | `prop_wash_strength`, `prop_wash_flicker`, `ground_effect_body` | |
| `[board]` | `gyro_range` | The gyro reads up to ± this. |
| `[sound]` | `buzzer` (`true` or `false`), `esc_melody` (`"Bluejay default"`) | |
| `[sound_block]` | the motor and prop voice, frame hum, beep levels, Prop Strike ticks, hit level and how loud the Quad is Where you stand; `buzzer_pitch` and `buzzer_level` only on a Quad with a buzzer | Set by ear, with no Confidence ([Sound deep dive](../context/sound.md)). |
| `[sources]` | one line per source key | |

## The Tune, `tune.txt`

Betaflight CLI text, with 2026.6 names and Betaflight's own units ([ADR-0015](../adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)). It stays pasteable into a real quad, so it has no `format` line; its opening comments say which Betaflight it follows, the quad and the date.

```text
set motor_poles = 12                   # diff; must match [motors] poles
set yaw_motors_reversed = OFF          # 4.3 default; must match [props] direction, props-in
```

- Only `set` lines and comments. `aux` lines and rate profiles belong to the pilot.
- Every line carries a mark: `diff`, a version's default such as `4.3 default` or `2026.6 default`, `ADR-0008`, or `hand-set: <reason>`, optionally followed by `(was <old name>)`. A note may follow a `;`.
- No setting twice.
- `motor_poles` must equal `[motors] poles`, and `yaw_motors_reversed` must be `OFF` for props-in and `ON` for props-out. Both lines are required.

For now each Tune holds only those two lines; the Flight Controller tickets spell out every other setting.

## Test Quads

A Test Quad is a real Quad with a few numbers changed, so a Scenario can test one effect alone. It lives in `scenarios/test-quads/<id>.toml`, its id starts with `test/`, and only the Scenario runner reads it.

```toml
format   = 1
based_on = "opendrone/whoop-65"
why      = "No drag of any kind, so free fall follows the rigid-body rules alone"

[props]
rotor_drag = "0 s⁻¹"   # a deliberate test value: no Confidence or source
```

- It lists only what it changes, so a change to the real Quad carries into it.
- Its changes carry no Confidence, and each must name a setting the real Quad has.
- It builds on a real Quad, never on another Test Quad.
- The result is checked like any Quad, Tune cross-checks included.

## The Feel Test log, `feel-tests.md`

Each Quad keeps a log of every Estimate a Feel Test moves, oldest first:

```text
| Date | Number | Old → new | Why |
|---|---|---|---|
| 2026-11-02 | [props] rotor_drag | 0.3 s⁻¹ → 0.35 s⁻¹ | carved too wide after a sprint |
```

The values are written with their units, as `quad.toml` writes them; they're compared as numbers, so "300 ms" matches "0.3 s". The Pack checker refuses a row whose values don't read as its number.

`cargo xtask feel-tests` compares each Quad with the same Quad before the change. Quads are paired by id (the Pack's id from its `pack.toml`, and the Quad's folder), so moving or renaming a Pack's folder changes nothing, whatever its new name (accents and all). It refuses:

- an Estimate that moved without a new row naming it, its old value and its new one, even when its range was re-sourced in the same change;
- a new row that doesn't record a move this change makes: the same Estimate, its value before the change as the old value, and its value after as the new one;
- an Estimate that moved outside its range. A relative range is measured from the value the Estimate started at: the new value of its latest "New source" row already in the log, or else the old value of its earliest row there, or else its value before this change. A row added in the same change never sets the start, so a made-up earlier row can't stretch a range.
- a Measured, Manufacturer or Derived number that changed without a new source, meaning a different source key, or a changed line for its key in `[sources]`;
- a change to what's known about a number without a new source: its Confidence, its range, or where its numbers hold (a curve's places or number of points, or the condition of a value written "at" one, such as the 4 V in "0.3 A at 4 V"). So a Feel Test can't widen its own range, or move a curve's points sideways instead of up and down;
- an Estimate moved with a new source whose row's why doesn't start with "New source", and a "New source" row with no real new source. A "New source" row starts the range afresh from its new value. One may also record a re-sourced range whose value stayed put, with the same old and new value; it still moves where the range is measured from, so CI lists it;
- a number taken out, such as a whoop's `[ducts]` section, without a new source;
- a change that removes a Quad and adds one, unless the added Quad keeps every setting of a removed one: a pure rename or move. Rename or move a Quad in a change of its own, and change its numbers in another. **Retiring a Quad and adding a different one takes two changes:** take the old one out in one, and add the new one in the next;
- an earlier row changed or removed: the log only grows.

"A new source" is easy to write, so CI lists every number that passes on its source alone, and the Reviewer judges whether each source is real:

- a number that changed with a new source;
- a "New source" row, even one whose value stayed put, with the number's Confidence, range and source: from then on its range is measured from there;
- a number taken out;
- a number added to a Quad that already existed, such as a 5″ given `[ducts]`, with its Confidence and source.

Counts, choices, camera defaults and the sound block carry no Confidence and move freely. CI lists the counts and choices the Simulation receives that changed too; camera defaults and the sound block don't reach the Simulation, so they aren't listed. CI also says how many Quads it compared with their version before the change, and names every new Quad, so a Quad left out of the comparison stands out. A change under `packs/` always runs these checks in CI, even when only Markdown changed.

## Fingerprints

The checker gives each Quad a fingerprint of what the Simulation receives from it: every physics number, count and choice, the ESC melody and the Tune's settings. Its on-screen text, picture, camera, sound block, Confidences, sources, marks and comments don't count, and the same number written two ways ("23.0 g" or "0.023 kg") gives the same fingerprint. Each Scenario's Results file records the fingerprints of the Quad and Map it ran on, so a moved Result can be traced to a changed Quad.

## File formats

Every Pack data file starts with `format = N`, before anything else but comments. A newer format than this OpenDrone knows is refused as needing a newer OpenDrone. An older one is upgraded in memory, one format at a time, by the same steps the format migration tool, `cargo xtask migrate`, runs over the repo's own files. Format 1 is the first, so there are no steps yet. Pack files, Test Quads and the built-in Test Maps share one format number; Scenarios have their own. [Changing a file format](../format-migration.md) says how a step is written and run.
