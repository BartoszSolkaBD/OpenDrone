# SDL3 input probe: pocket-usb-rf-off

> Re-analysed on 2026-10-04 08:39:31 UTC from this folder's raw events with the updated estimator (busiest 100 ms and grid test). The poll-loop and frame tables are copied from the original run. The original output is in git history.

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

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 235210 | 0 /s | 2993 /s | NaN% / NaN% / NaN% | 0.334 | 0.259 | 0.342 | 0.401 | 4.601 | 0.043 | 0 |
| poll loop, get-ready | 44874 | 0 /s | 2916 /s | NaN% / NaN% / NaN% | 0.343 | 0.259 | 0.350 | 0.450 | 0.636 | 0.048 | 0 |
| poll loop, connect | 9103 | 0 /s | 2784 /s | NaN% / NaN% / NaN% | 0.359 | 0.309 | 0.346 | 0.462 | 0.813 | 0.045 | 0 |
| poll loop, rest | 12540 | 0 /s | 2514 /s | NaN% / NaN% / NaN% | 0.398 | 0.315 | 0.371 | 0.477 | 1.424 | 0.052 | 0 |
| poll loop, circles | 33314 | 0 /s | 2780 /s | NaN% / NaN% / NaN% | 0.360 | 0.305 | 0.360 | 0.381 | 0.520 | 0.013 | 0 |
| poll loop, yaw | 16613 | 0 /s | 2771 /s | NaN% / NaN% / NaN% | 0.361 | 0.308 | 0.360 | 0.382 | 0.482 | 0.012 | 0 |
| poll loop, extremes | 35237 | 0 /s | 2941 /s | NaN% / NaN% / NaN% | 0.340 | 0.262 | 0.352 | 0.378 | 4.601 | 0.043 | 0 |
| poll loop, switches | 50096 | 0 /s | 3342 /s | NaN% / NaN% / NaN% | 0.299 | 0.263 | 0.299 | 0.306 | 0.500 | 0.004 | 0 |
| poll loop, unfocused | 33433 | 0 /s | 3343 /s | NaN% / NaN% / NaN% | 0.299 | 0.262 | 0.299 | 0.306 | 0.395 | 0.004 | 0 |

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

- Report rate: expected about 1000 Hz with RF off. Busiest 100 ms: 980 updates per second. Gaps between updates on a 1 ms grid: 100.0%, 2 ms: 36.7%, 4 ms: 4.3%. Average while moving: 477 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Stick axes at start: centred [0, 1, 3], at the bottom [2]. EdgeTX's AETR order expects roll 0, pitch 1, throttle 2 (rests at the bottom), yaw 3.
- Axes: expected 8 (CH1-CH8); SDL reports 8; 8 of them moved during the run (axes [0, 1, 2, 3, 4, 5, 6, 7]). MATCHES
- Buttons: expected 24 (CH9-CH32); SDL reports 24; pressed during the run: [0, 1].
- Resolution: expected about 11 bits (0-2048); measured about 11.0 bits on the busiest axes. MATCHES
- Yaw-only step: the sticks were not moved in this step, so it can't single out the yaw axis.
- Channels 5-8 (axes 4-7): moved = [4, 5, 6, 7]. They only move if the radio model mixes switches or the pot onto CH5-CH8.
- Window-not-focused step: no stick movement recorded, so focus independence is untested in this run.
- Hands-off step: 0 axis changes. SDL only reports changes, so a still stick sends nothing; silence alone can't tell a resting stick from a stalled device.
- Disconnected: SDL reported the removal 45.41 s into the run (the probe can't see when the cable was actually pulled). In the same poll it also delivered last-moment value changes that the pilot didn't make: axis 5 -> 15, axis 6 -> 15.

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

**Update rate.** Counts the moments when any axis changed. A new value can only appear when the device sends a report, but a slow stick doesn't change on every report, so averages understate the report rate. Two measures don't depend on stick speed: the **busiest 100 ms**, and the **grid test** (a device reporting every N ms only produces gaps that are whole multiples of N). SDL stamps a change when the input thread polls (about every 0.33 ms here), so single gaps wobble by up to one poll period.

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| any axis, all steps | 2381 | 980 /s | 477 /s | 100% / 37% / 4% | 2.096 | 0.685 | 1.768 | 4.094 | 49.191 | 2.842 | 31 |
| any axis, circles | 1643 | 980 /s | 527 /s | 100% / 33% / 3% | 1.898 | 0.685 | 1.111 | 3.907 | 49.191 | 2.690 | 18 |
| axis 0, circles | 524 | 940 /s | 564 /s | 100% / 36% / 4% | 1.774 | 0.685 | 1.106 | 3.877 | 18.750 | 1.557 | 8 |
| axis 1, circles | 581 | 790 /s | 480 /s | 100% / 38% / 4% | 2.084 | 0.696 | 1.823 | 4.800 | 36.850 | 2.042 | 6 |
| axis 2, circles | 99 | 300 /s | 206 /s | 100% / 42% / 8% | 4.854 | 0.696 | 2.118 | 15.311 | 44.188 | 7.577 | 3 |
| axis 3, circles | 503 | 970 /s | 674 /s | 100% / 29% / 1% | 1.484 | 0.692 | 1.081 | 2.907 | 9.099 | 0.860 | 4 |

Gaps between updates, any axis, all steps:

| Interval | Count | Share |
|---|---|---|
| 0.50-0.75 ms | 241 | 10.3% |
| 0.75-1.25 ms | 877 | 37.3% |
| 1.25-1.75 ms | 6 | 0.3% |
| 1.75-2.50 ms | 733 | 31.2% |
| 2.50-3.50 ms | 303 | 12.9% |
| 3.50-4.50 ms | 88 | 3.7% |
| 4.50-6.00 ms | 40 | 1.7% |
| 6.00-8.00 ms | 24 | 1.0% |
| 8.00-12.00 ms | 16 | 0.7% |
| 12.00-20.00 ms | 11 | 0.5% |
| 20.00-50.00 ms | 10 | 0.4% |

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

