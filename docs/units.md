# The unit list

Every number in a Scenario or a Pack file is written as short text with its unit, such as `"9.81 m/s²"`, `"23.0 g"` or `"670 °/s ± 3%"`. This one list of units is shared by Scenarios and Packs, so a number means the same thing in both ([#11 §2](https://github.com/BartoszSolkaBD/OpenDrone/issues/11), [#16 §3](https://github.com/BartoszSolkaBD/OpenDrone/issues/16)). The Scenario runner and the Pack reader both read numbers through the same code, [`crates/pack/src/units.rs`](../crates/pack/src/units.rs), so the two kinds of file always agree.

The one exception is a Quad's Tune, which keeps Betaflight's own CLI form and units, with no unit text ([ADR-0015](adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)). Counts and choices, such as a prop's blade count or its direction, are plain values with no unit.

## How a number is written

- **A number, then its unit.** The space between them is optional: `"30°"` and `"670 °/s"` both read.
- **A decimal point only.** Write `"31.2 g"`, never `"31,2 g"`, and no thousands separators: `"19500 KV"`, never `"19,500 KV"`.
- **Symbols or plain-keyboard spellings.** Both read the same, so `"30 deg"` is `"30°"` and `"+-"` is `"±"`. OpenDrone's tools always write symbols.
- **Everyday prefixes,** so a number stays ordinary: `"140 g·cm²"`, not `"1.4 × 10⁻⁵ kg·m²"`.
- **Never radians.** Angles are in degrees, and spin speeds in degrees per second or RPM.
- **Percent** is its own kind of number, written with `%`. In Scenarios, sticks are in percent: roll, pitch and yaw from −100% to +100%, and throttle from 0% to 100%.
- **Several parts carry their labels inline.** When every part has the same unit, it may be written once, at the end: `"roll 70, pitch 90, yaw 140 g·cm²"`. If any other part has a unit of its own, each part keeps the unit it was written with.
- **A curve is one line of points:** `"4.35 V at 100 %, 4.10 V at 80 %, 3.85 V at 50 %"`.
- **Prop coefficients are plain numbers,** the unitless thrust and power coefficients that propeller test data publishes, so a Map's air density can change thrust.

## The units

| What it measures | Symbol | Also accepted | Prefixes | Example |
|---|---|---|---|---|
| Length | `m` | | `km`, `cm`, `mm` | `"2 m"`, `"35 mm"` |
| Mass | `g` | | `kg`, `mg` | `"23.0 g"` |
| Time | `s`, `min`, `h` | `us` for `µs` | `ms`, `µs` (on `s`) | `"1.5 s"`, `"35 ms"` |
| Rate, frequency | `Hz`, `s⁻¹` | `hz`, `s^-1`, `1/s` | `kHz`, `MHz` | `"250 Hz"`, `"8 kHz"`, `"0.3 s⁻¹"` |
| Angle | `°` | `deg` | | `"30°"` |
| Rotation speed | `°/s`, `RPM` | `deg/s`, `rpm` | | `"670 °/s"`, `"22700 RPM"` |
| Voltage | `V` | | `mV` | `"4.35 V"` |
| Current | `A` | | `mA` | `"3.4 A"` |
| Resistance | `Ω` | `ohm`, `ohms` | `mΩ`, `kΩ` | `"29 mΩ"` |
| Charge | `Ah` | | `mAh` | `"320 mAh"` |
| Power | `W` | | `mW`, `kW` | `"25 mW"` |
| Energy | `Wh` | | | `"1.22 Wh"` |
| Motor KV (RPM per volt) | `KV` | `kv`, `Kv` | | `"19500 KV"` |
| Force | `N` | | `mN` | `"0.3 N"` |
| Share | `%` | `percent` | | `"50%"` |
| Sound level | `dB` | `db` | | `"-12 dB"` |
| An FPV Camera's Dynamic Range | `stops` | `stop` | | `"7 stops"` |
| An FPV Camera's picture height | `lines` | | | `"480 lines"` |
| An FPV Camera's sharpness | `TVL` | `tvl` | | `"300 TVL"` |

Units combine into others: products with `·` (or `*`, or a space), quotients with one `/`, and powers with `²`, `³` and `⁻¹` (or `^2`, `^3` and `^-1`). For example `"9.81 m/s²"`, `"670 °/s"`, `"140 g·cm²"`, `"25 cm²"`, `"1.225 kg/m³"` and `"0.3 s⁻¹"`.

Other symbols and their plain-keyboard spellings: `µ` or `u` for micro, `±` or `+-` or `+/-`, `−` or `-` for minus, `×` or `x` for times, and `–` or `-` between the two ends of a range.

## Tolerances

An Expectation in a Scenario gives its value with a tolerance, always with units:

- **± an amount:** `"3.71 V ± 0.02 V"`
- **± a percentage:** `"670 °/s ± 3%"`, a share of the value (for a value in percent, percentage points)
- **between two values:** `"between 40% and 50%"`

## Ranges

An Estimate in a Quad definition carries the range a Feel Test may move it within, written with the same units. A range is absolute, such as `range = "20–50 ms"`, or relative to the value, such as `range = "×0.5–×2"`.

## What is refused

A number that breaks these rules is refused with a plain sentence naming the file and the line, with the fix when there's only one: `"31,2 g" isn't a number OpenDrone can read: numbers take a decimal point, so write "31.2 g"`. Refused are:

- radians, in any spelling;
- decimal commas and thousands separators;
- a unit that isn't on this list, or a number in the wrong kind of unit, such as a mass written in metres.
