# SDL3 input probe: dualsense-usb-sensors

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 16:55:21 (UTC), lasted 101.5 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 20.7 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 346775 | 3500 /s | 3429 /s | 0% / 0% / 0% | 0.292 | 0.255 | 0.291 | 0.297 | 1.887 | 0.005 | 0 |
| poll loop, get-ready | 68512 | 3460 /s | 3428 /s | 0% / 0% / 0% | 0.292 | 0.259 | 0.291 | 0.296 | 0.627 | 0.004 | 0 |
| poll loop, connect | 11199 | 3500 /s | 3445 /s | 0% / 0% / 0% | 0.290 | 0.260 | 0.290 | 0.295 | 1.887 | 0.015 | 0 |
| poll loop, rest | 17160 | 3460 /s | 3435 /s | 0% / 0% / 0% | 0.291 | 0.266 | 0.290 | 0.295 | 0.346 | 0.003 | 0 |
| poll loop, circles | 41098 | 3460 /s | 3431 /s | 0% / 0% / 0% | 0.291 | 0.257 | 0.291 | 0.296 | 0.349 | 0.003 | 0 |
| poll loop, sensors | 41085 | 3460 /s | 3429 /s | 0% / 0% / 0% | 0.292 | 0.261 | 0.291 | 0.296 | 0.349 | 0.003 | 0 |
| poll loop, sensors-rest | 20544 | 3450 /s | 3429 /s | 0% / 0% / 0% | 0.292 | 0.265 | 0.291 | 0.296 | 0.358 | 0.003 | 0 |
| poll loop, yaw | 20555 | 3450 /s | 3430 /s | 0% / 0% / 0% | 0.292 | 0.271 | 0.291 | 0.296 | 0.335 | 0.003 | 0 |
| poll loop, extremes | 41089 | 3450 /s | 3430 /s | 0% / 0% / 0% | 0.292 | 0.261 | 0.291 | 0.296 | 0.347 | 0.003 | 0 |
| poll loop, switches | 51427 | 3500 /s | 3433 /s | 0% / 0% / 0% | 0.291 | 0.255 | 0.291 | 0.296 | 0.353 | 0.004 | 0 |
| poll loop, unfocused | 34106 | 3470 /s | 3409 /s | 0% / 0% / 0% | 0.293 | 0.259 | 0.292 | 0.303 | 0.376 | 0.005 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 3301 | 165 | 12.1 | 100% |
| connect | 539 | 165 | 15.3 | 100% |
| rest | 826 | 165 | 7.3 | 100% |
| circles | 1981 | 165 | 7.9 | 100% |
| sensors | 1981 | 165 | 7.4 | 100% |
| sensors-rest | 991 | 165 | 7.2 | 100% |
| yaw | 991 | 165 | 7.4 | 100% |
| extremes | 1981 | 165 | 7.4 | 100% |
| switches | 2476 | 165 | 17.0 | 100% |
| unfocused | 1148 | 115 | 18.3 | 75% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

### DualSense Wireless Controller — Gamepad (DualSense) over USB

**Compared with the research**

- Stick report rate, sensors off (circles step): expected 250 Hz over USB. Busiest 100 ms: 260 updates per second. Gaps between updates on a 1 ms grid: 100.0%, 2 ms: 100.0%, 4 ms: 100.0%. Average while moving: 246 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Stick report rate, sensors on (sensors step): expected 250 Hz over USB. Busiest 100 ms: 260 updates per second. Gaps between updates on a 1 ms grid: 100.0%, 2 ms: 100.0%, 4 ms: 100.0%. Average while moving: 242 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Report rate from the controller's own clock (gyro timestamps): 250 Hz (mean 4.000 ms, median 4.005 ms). SDL claims Some(250.0) Hz. MATCHES
- Reports at rest (sensors on, controller untouched): 250 gyro readings per second, longest gap 4.4 ms. A steady rate here means a connected DualSense keeps reporting while the sticks rest, which a stall check could use.
- Resolution: expected 8 bits (hardware limit); measured about 8.0 bits on the sticks. MATCHES
- With the probe window NOT focused: busiest 100 ms 260 updates per second, average 195. Input keeps flowing without focus if this is close to the focused steps.
- Hands-off step: 2 axis changes. SDL only reports changes: a perfectly still stick sends nothing, and only sensor noise or a touch shows up. Silence alone can't tell a resting stick from a stalled device.

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
| Motion sensors | gyro true, accel true, SDL rate Some(250.0) |
| Serial | 4c-b9-9b-09-a5-de |
| GUID | 030057564c050000e60c000000016800 |
| Path | DevSrvsID:4295190984 |

**Update rate.** Counts the moments when any axis changed. A new value can only appear when the device sends a report, but a slow stick doesn't change on every report, so averages understate the report rate. Two measures don't depend on stick speed: the **busiest 100 ms**, and the **grid test** (a device reporting every N ms only produces gaps that are whole multiples of N). SDL stamps a change when the input thread polls (about every 0.29 ms here), so single gaps wobble by up to one poll period.

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| any axis, all steps | 11027 | 260 /s | 200 /s | 100% / 100% / 100% | 5.008 | 0.003 | 4.081 | 11.892 | 48.110 | 4.079 | 115 |
| any axis, circles | 2913 | 260 /s | 246 /s | 100% / 100% / 100% | 4.063 | 3.755 | 4.077 | 4.112 | 23.893 | 0.779 | 0 |
| any axis, sensors | 2715 | 260 /s | 242 /s | 100% / 100% / 100% | 4.140 | 3.755 | 4.079 | 4.117 | 47.722 | 1.532 | 7 |
| any axis, extremes | 2037 | 260 /s | 201 /s | 100% / 100% / 100% | 4.978 | 3.759 | 4.083 | 8.198 | 44.019 | 3.236 | 7 |
| any axis, unfocused | 1143 | 260 /s | 195 /s | 100% / 100% / 100% | 5.129 | 3.550 | 4.055 | 11.886 | 47.977 | 4.408 | 15 |
| gyro, controller clock (= report rate) | 19011 | 260 /s | 250 /s | 99% / 99% / 99% | 4.000 | 3.503 | 4.005 | 4.005 | 4.007 | 0.048 | 0 |
| gyro, arrival in SDL | 19011 | 270 /s | 250 /s | 100% / 100% / 100% | 4.000 | 0.003 | 4.076 | 4.123 | 9.420 | 0.149 | 0 |
| axis 0, circles | 1950 | 260 /s | 212 /s | 100% / 100% / 100% | 4.723 | 3.755 | 4.078 | 7.858 | 48.240 | 4.380 | 29 |
| axis 1, circles | 1942 | 260 /s | 221 /s | 100% / 100% / 100% | 4.520 | 3.755 | 4.078 | 4.143 | 47.877 | 3.502 | 29 |
| axis 2, circles | 2334 | 260 /s | 218 /s | 100% / 100% / 100% | 4.589 | 3.755 | 4.079 | 7.886 | 47.962 | 3.486 | 17 |
| axis 3, circles | 2108 | 260 /s | 238 /s | 100% / 100% / 100% | 4.207 | 3.755 | 4.076 | 4.117 | 47.958 | 2.401 | 42 |

Gaps between updates, any axis, all steps:

| Interval | Count | Share |
|---|---|---|
| 0.00-0.25 ms | 1 | 0.0% |
| 0.75-1.25 ms | 1 | 0.0% |
| 2.50-3.50 ms | 1 | 0.0% |
| 3.50-4.50 ms | 9772 | 89.6% |
| 4.50-6.00 ms | 1 | 0.0% |
| 6.00-8.00 ms | 335 | 3.1% |
| 8.00-12.00 ms | 393 | 3.6% |
| 12.00-20.00 ms | 206 | 1.9% |
| 20.00-50.00 ms | 201 | 1.8% |

Interval histogram, gyro on the controller's clock:

| Interval | Count | Share |
|---|---|---|
| 3.50-4.50 ms | 19010 | 100.0% |

**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)

| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |
|---|---|---|---|---|---|---|---|
| 0 | -32768 | 32767 | 257 | 128 | 257 | 256 | 8.0 |
| 1 | -32768 | 32767 | 257 | 128 | 257 | 256 | 8.0 |
| 2 | -32768 | 32767 | 253 | 128 | 257 | 256 | 8.0 |
| 3 | -32768 | 32767 | 256 | 128 | 257 | 256 | 8.0 |
| 4 | -32768 | 32767 | 24 | 257 | 1028 | 65 | 6.0 |
| 5 | -32768 | 32767 | 14 | 514 | 6682 | 11 | 3.4 |

**What moved in each step** (axis: number of changes)

| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |
|---|---|---|---|---|
| connect | {2: 2, 3: 2} | {} | 0 | Some(3) |
| rest | {2: 2} | {} | 0 | Some(2) |
| circles | {0: 1951, 1: 1943, 2: 2334, 3: 2108} | {} | 0 | Some(2) |
| yaw | {0: 627, 1: 298, 2: 71} | {} | 0 | Some(0) |
| extremes | {0: 1020, 1: 866, 2: 829, 3: 835} | {} | 0 | Some(0) |
| switches | {0: 64, 1: 31, 2: 505, 3: 65, 4: 25, 5: 15} | {0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11} | 8 | Some(2) |
| unfocused | {0: 693, 1: 574, 2: 805, 3: 590} | {} | 0 | Some(2) |
| sensors | {0: 2144, 1: 1583, 2: 1938, 3: 1955} | {} | 0 | Some(0) |
| sensors-rest | {1: 2, 2: 181} | {} | 0 | Some(2) |

