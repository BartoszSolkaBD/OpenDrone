# SDL3 input probe: run

> Re-analysed on 2026-10-04 11:19:41 UTC from this folder's raw events with the updated estimator (busiest 100 ms and grid test). The poll-loop and frame tables are copied from the original run. The original output is in git history.

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 11:06:11 (UTC), lasted 44.7 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 21.1 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 152487 | 3510 /s | 3435 /s | 0% / 0% / 0% | 0.291 | 0.257 | 0.290 | 0.296 | 9.010 | 0.023 | 0 |
| poll loop, get-ready | 34304 | 3490 /s | 3433 /s | 0% / 0% / 0% | 0.291 | 0.258 | 0.291 | 0.296 | 0.337 | 0.004 | 0 |
| poll loop, connect | 11207 | 3510 /s | 3436 /s | 0% / 0% / 0% | 0.291 | 0.261 | 0.290 | 0.295 | 9.010 | 0.082 | 0 |
| poll loop, rest | 17149 | 3460 /s | 3433 /s | 0% / 0% / 0% | 0.291 | 0.266 | 0.290 | 0.296 | 0.341 | 0.004 | 0 |
| poll loop, circles | 41153 | 3480 /s | 3433 /s | 0% / 0% / 0% | 0.291 | 0.258 | 0.291 | 0.296 | 0.354 | 0.004 | 0 |
| poll loop, yaw | 20578 | 3480 /s | 3434 /s | 0% / 0% / 0% | 0.291 | 0.267 | 0.290 | 0.296 | 0.355 | 0.004 | 0 |
| poll loop, extremes | 28096 | 3500 /s | 3439 /s | 0% / 0% / 0% | 0.291 | 0.257 | 0.290 | 0.296 | 0.360 | 0.004 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 1651 | 165 | 12.7 | 100% |
| connect | 543 | 165 | 14.1 | 100% |
| rest | 826 | 165 | 9.5 | 100% |
| circles | 1981 | 165 | 12.4 | 100% |
| yaw | 989 | 165 | 12.4 | 100% |
| extremes | 1351 | 165 | 15.6 | 100% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

### DualSense Wireless Controller — Gamepad (DualSense) over USB

**Compared with the research**

- Stick report rate, sensors off (circles step): expected 250 Hz over USB. Busiest 100 ms: 260 updates per second. Gaps between updates on a 1 ms grid: 100.0%, 2 ms: 100.0%, 4 ms: 100.0%. Average while moving: 127 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Resolution: expected 8 bits (hardware limit); measured about 8.0 bits on the sticks. MATCHES
- Window-not-focused step: no stick movement recorded, so focus independence is untested in this run.
- Hands-off step: 548 axis changes. SDL only reports changes: a perfectly still stick sends nothing, and only sensor noise or a touch shows up. Silence alone can't tell a resting stick from a stalled device.

**Identity**

| Field | Value |
|---|---|
| Name | DualSense Wireless Controller |
| Vendor : Product | 0x054C : 0x0CE6 |
| Product version / firmware | 0x0100 / 0x0630 |
| Bus / connection | USB / wired |
| SDL driver | HIDAPI (SDL's own HID driver) |
| SDL joystick type | gamepad |
| Opened as SDL gamepad | true ps5 |
| Axes / buttons / hats | 6 / 13 / 1 |
| Motion sensors | gyro true, accel true, SDL rate None |
| Serial | 4c-b9-9b-09-a5-de |
| GUID | 030057564c050000e60c000000016800 |
| Path | DevSrvsID:4295181314 |

**Update rate.** Counts the moments when any axis changed. A new value can only appear when the device sends a report, but a slow stick doesn't change on every report, so averages understate the report rate. Two measures don't depend on stick speed: the **busiest 100 ms**, and the **grid test** (a device reporting every N ms only produces gaps that are whole multiples of N). SDL stamps a change when the input thread polls (about every 0.29 ms here), so single gaps wobble by up to one poll period.

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| any axis, all steps | 2127 | 260 /s | 146 /s | 100% / 100% / 100% | 6.851 | 3.726 | 4.085 | 20.001 | 48.167 | 6.632 | 97 |
| any axis, circles | 451 | 260 /s | 127 /s | 100% / 100% / 100% | 7.853 | 3.751 | 4.093 | 20.120 | 43.943 | 6.618 | 18 |
| any axis, extremes | 137 | 190 /s | 190 /s | 100% / 100% / 100% | 5.251 | 3.758 | 4.074 | 8.100 | 48.167 | 5.978 | 14 |
| axis 0, circles | 98 | 250 /s | 143 /s | 100% / 100% / 100% | 7.001 | 3.751 | 4.088 | 16.047 | 27.964 | 4.942 | 13 |
| axis 1, circles | 50 | 150 /s | 125 /s | 100% / 100% / 100% | 8.002 | 3.776 | 4.089 | 20.149 | 40.192 | 7.223 | 8 |
| axis 2, circles | 316 | 240 /s | 104 /s | 100% / 100% / 100% | 9.580 | 3.766 | 7.845 | 32.020 | 47.985 | 8.776 | 16 |
| axis 3, circles | 80 | 180 /s | 115 /s | 100% / 100% / 100% | 8.706 | 3.780 | 4.109 | 28.068 | 44.276 | 8.132 | 5 |

Gaps between updates, any axis, all steps:

| Interval | Count | Share |
|---|---|---|
| 3.50-4.50 ms | 1443 | 71.1% |
| 6.00-8.00 ms | 177 | 8.7% |
| 8.00-12.00 ms | 194 | 9.6% |
| 12.00-20.00 ms | 113 | 5.6% |
| 20.00-50.00 ms | 102 | 5.0% |

**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)

| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |
|---|---|---|---|---|---|---|---|
| 0 | -32768 | 32767 | 169 | 128 | 257 | 256 | 8.0 |
| 1 | -3984 | 32767 | 117 | 128 | 257 | 256 | 8.0 |
| 2 | -32511 | 32767 | 102 | 128 | 257 | 256 | 8.0 |
| 3 | -32768 | 32767 | 133 | 128 | 257 | 256 | 8.0 |
| 4 | -32768 | 32767 | 175 | 257 | 257 | 256 | 8.0 |
| 5 | -32768 | 32767 | 181 | 257 | 257 | 256 | 8.0 |

**What moved in each step** (axis: number of changes)

| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |
|---|---|---|---|---|
| connect | {0: 90, 1: 66, 2: 11, 3: 12} | {} | 0 | Some(0) |
| rest | {0: 32, 1: 14, 2: 259, 3: 225, 5: 18} | {10} | 0 | Some(2) |
| circles | {0: 98, 1: 50, 2: 316, 3: 80} | {7, 8} | 34 | Some(2) |
| yaw | {2: 10, 4: 251} | {} | 0 | Some(4) |
| extremes | {0: 15, 2: 2, 5: 120} | {10} | 0 | Some(5) |
| switches | {} | {} | 0 | None |
| unfocused | {} | {} | 0 | None |
| sensors | {} | {} | 0 | None |

