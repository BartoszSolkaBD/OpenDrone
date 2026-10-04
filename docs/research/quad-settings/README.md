# Quad settings: the maintainer's real quads

Betaflight `diff all` exports from the maintainer's two real quads, captured over USB (ticket [#20](https://github.com/BartoszSolkaBD/OpenDrone/issues/20)). They seed the alpha Quad definitions and the Flight Controller defaults. `mcu_id` and `expresslrs_uid` are redacted; neither affects simulation.

| | Meteor65 Pro | Cetus X |
|---|---|---|
| Export | [meteor65-pro.diff-all.txt](meteor65-pro.diff-all.txt) | [cetus-x.diff-all.txt](cetus-x.diff-all.txt) |
| Betaflight | 4.3.0 (June 2022 build, config dated January 2023) | 4.4.0 (October 2023 build) |
| Board | BETAFPVF411 (STM32F411) | BETAFPVF4SX1280 (STM32F411, ELRS 2.4 GHz receiver on board) |
| Radio link | ELRS receiver over serial (CRSF) | Built-in ELRS receiver, `expresslrs_rate_index = 1` |
| PID loop | Full gyro rate (`pid_process_denom = 1`) | Full gyro rate (`pid_process_denom = 1`) |
| Motors | DShot300 bidirectional, 12 poles (RPM data available) | DShot300 bidirectional, 12 poles (RPM data available) |
| Motor idle | 6 % (`dshot_idle_value = 600`) | 12 % (`dshot_idle_value = 1200`) |
| Prop direction | Default (props-in) | `yaw_motors_reversed = ON` (props-out) |
| Battery | LiHV, 4.35 V max per cell | LiHV, 4.35 V max per cell |
| Rates | Betaflight 4.3 defaults (Actual: centre 70°/s, max 670°/s, expo 0) | Actual, max rate 650°/s on all axes, other values default |
| TPA | `tpa_rate = 70` | `tpa_rate = 70` |
| PIDs (roll / pitch / yaw) | P 40/39/35, I 64/64/50, D 48/52/–, F 125/124/100, D-min 45/49 | P 58/54/55, I 67/63/70, D 52/60/–, F 156/146/118, D-min 48/55 |
| Feedforward | `averaging = 2_POINT`, `jitter_factor = 9` | `smooth_factor = 30`, `jitter_factor = 12` |

## Switch layout (aux modes)

| Mode | Meteor65 Pro | Cetus X |
|---|---|---|
| Arm | AUX1 high | AUX1 high |
| Angle (self-levelling) | AUX2 high | AUX2 **low** |
| Horizon | AUX2 middle | AUX2 middle |
| Acro (no mode active) | AUX2 low | AUX2 high |
| Turtle (flip over after crash) | AUX3 high | AUX3 high |
| Beeper | – | AUX4 high |

Both quads arm at any angle (`small_angle = 180`).

## Notes for later tickets

- **Model variant:** the Meteor is the **Meteor65 Pro**. Per the [flight dynamics research](../flight-dynamics.md), that's the 35 mm-prop variant (thrust-to-weight about 3.9), not the original 31 mm one (about 3.2).
- **Setting names:** both quads run Betaflight 4.3 or 4.4, which still use the `d_min_*` names. Betaflight 2025.12 renamed them (see the [Betaflight research](../betaflight-flight-controller.md)), so a `diff all` importer must handle both naming schemes.
- **Factory Angle mode:** the Cetus X ships with Angle mode on the switch's *low* position, a beginner-friendly factory setup. That's relevant for Assists and Presets.
- **Unknowns:** the Cetus X's exact prop size, motor KV and battery aren't in the export. Its ELRS packet rate for index 1 needs mapping against Betaflight's ELRS rate table.
