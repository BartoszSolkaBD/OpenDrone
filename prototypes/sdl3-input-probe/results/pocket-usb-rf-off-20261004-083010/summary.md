# SDL3 input probe: pocket-usb-rf-off

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 08:30:10 (UTC), lasted 78.9 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 39.5 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Rate (1 / mean) | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 235210 | 2993 Hz | 0.334 | 0.259 | 0.342 | 0.401 | 4.601 | 0.043 | 0 |
| poll loop, get-ready | 44874 | 2916 Hz | 0.343 | 0.259 | 0.350 | 0.450 | 0.636 | 0.048 | 0 |
| poll loop, connect | 9103 | 2784 Hz | 0.359 | 0.309 | 0.346 | 0.462 | 0.813 | 0.045 | 0 |
| poll loop, rest | 12540 | 2514 Hz | 0.398 | 0.315 | 0.371 | 0.477 | 1.424 | 0.052 | 0 |
| poll loop, circles | 33314 | 2780 Hz | 0.360 | 0.305 | 0.360 | 0.381 | 0.520 | 0.013 | 0 |
| poll loop, yaw | 16613 | 2771 Hz | 0.361 | 0.308 | 0.360 | 0.382 | 0.482 | 0.012 | 0 |
| poll loop, extremes | 35237 | 2941 Hz | 0.340 | 0.262 | 0.352 | 0.378 | 4.601 | 0.043 | 0 |
| poll loop, switches | 50096 | 3342 Hz | 0.299 | 0.263 | 0.299 | 0.306 | 0.500 | 0.004 | 0 |
| poll loop, unfocused | 33433 | 3343 Hz | 0.299 | 0.262 | 0.299 | 0.306 | 0.395 | 0.004 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 1847 | 126 | 250.0 | 84% |
| connect | 363 | 144 | 250.0 | 97% |
| rest | 534 | 106 | 23.9 | 69% |
| circles | 1977 | 165 | 13.0 | 100% |
| yaw | 991 | 165 | 7.3 | 100% |
| extremes | 1981 | 165 | 7.3 | 100% |
| switches | 901 | 60 | 19.2 | 0% |
| unfocused | 601 | 60 | 19.3 | 0% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

### EdgeTX Radiomaster Pocket Joystick — Radio (EdgeTX USB joystick, e.g. Radiomaster Pocket)

**Compared with the research**

- Update rate while moving: expected about 1000 Hz with RF off; measured 527 Hz (mean interval 1.898 ms; 90% of single intervals between 0.71 and 3.91 ms). DIFFERS
- Axes: expected 8 (CH1-CH8); SDL reports 8; 8 of them moved during the run (axes [0, 1, 2, 3, 4, 5, 6, 7]). MATCHES
- Buttons: expected 24 (CH9-CH32); SDL reports 24; pressed during the run: [0, 1].
- Resolution: expected about 11 bits (0-2048); measured about 11.0 bits on the busiest axes. MATCHES
- Yaw: during the yaw-only step the most active axis was Some(6) (EdgeTX AETR puts yaw/rudder on CH4, which is axis 3).
- Channels 5-8 (axes 4-7): moved = [4, 5, 6, 7]. They only move if the radio model mixes switches or the pot onto CH5-CH8.

**Identity**

| Field | Value |
|---|---|
| Name | EdgeTX Radiomaster Pocket Joystick |
| Vendor : Product | 0x1209 : 0x4F54 |
| Product version / firmware | 0x0200 / 0x0000 |
| Bus / connection | USB / unknown |
| SDL driver | platform driver (IOKit on macOS) |
| SDL joystick type | unknown |
| Opened as SDL gamepad | false  |
| Axes / buttons / hats | 8 / 24 / 0 |
| Motion sensors | gyro false, accel false, SDL rate None |
| Serial |  |
| GUID | 0300fcb609120000544f000000020000 |
| Path |  |

**Update rate** (instants when any axis changed; a new value can only appear when the device sends a report, so while the sticks move fast this tracks the report rate). SDL stamps a change when the input thread polls (about every 0.33 ms here), so single intervals snap to whole poll periods; the mean gives the true rate.

| What | Samples | Rate (1 / mean) | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|
| any axis, circles | 1643 | 527 Hz | 1.898 | 0.685 | 1.111 | 3.907 | 49.191 | 2.690 | 18 |
| axis 0, circles | 524 | 564 Hz | 1.774 | 0.685 | 1.106 | 3.877 | 18.750 | 1.557 | 8 |
| axis 1, circles | 581 | 480 Hz | 2.084 | 0.696 | 1.823 | 4.800 | 36.850 | 2.042 | 6 |
| axis 2, circles | 99 | 206 Hz | 4.854 | 0.696 | 2.118 | 15.311 | 44.188 | 7.577 | 3 |
| axis 3, circles | 503 | 674 Hz | 1.484 | 0.692 | 1.081 | 2.907 | 9.099 | 0.860 | 4 |

Interval histogram, any axis, circles step:

| Interval | Count | Share |
|---|---|---|
| 0.50-0.75 ms | 193 | 11.9% |
| 0.75-1.25 ms | 709 | 43.7% |
| 1.25-1.75 ms | 3 | 0.2% |
| 1.75-2.50 ms | 474 | 29.2% |
| 2.50-3.50 ms | 147 | 9.1% |
| 3.50-4.50 ms | 41 | 2.5% |
| 4.50-6.00 ms | 21 | 1.3% |
| 6.00-8.00 ms | 15 | 0.9% |
| 8.00-12.00 ms | 9 | 0.6% |
| 12.00-20.00 ms | 5 | 0.3% |
| 20.00-50.00 ms | 7 | 0.4% |

**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)

| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |
|---|---|---|---|---|---|---|---|
| 0 | -32768 | 32767 | 366 | 31 | 32 | 2049 | 11.0 |
| 1 | -32768 | 32767 | 461 | 31 | 32 | 2049 | 11.0 |
| 2 | -32768 | 32767 | 591 | 32 | 32 | 2049 | 11.0 |
| 3 | -32768 | 32767 | 405 | 31 | 32 | 2049 | 11.0 |
| 4 | -32768 | 32767 | 2 | 65535 | 65535 | 2 | 1.0 |
| 5 | -32768 | 32767 | 3 | 32752 | 32783 | 3 | 1.6 |
| 6 | -32768 | 32767 | 3 | 32752 | 32783 | 3 | 1.6 |
| 7 | -32768 | 32767 | 2 | 65535 | 65535 | 2 | 1.0 |

**What moved in each step** (axis: number of changes)

| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |
|---|---|---|---|---|
| connect | {} | {} | 0 | None |
| rest | {} | {} | 0 | None |
| circles | {0: 525, 1: 582, 2: 99, 3: 503, 4: 3, 7: 3} | {0} | 0 | Some(1) |
| yaw | {5: 4, 6: 4} | {} | 0 | Some(6) |
| extremes | {4: 2, 5: 1, 6: 1, 7: 1} | {} | 0 | Some(4) |
| switches | {} | {} | 0 | None |
| unfocused | {} | {} | 0 | None |
| sensors | {} | {} | 0 | None |

