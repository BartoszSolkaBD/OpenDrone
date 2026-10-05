# SDL3 input probe: pocket-sim-filter-on

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 19:42:53 (UTC), lasted 84.8 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 39.1 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 235788 | 3200 /s | 2793 /s | 0% / 0% / 0% | 0.358 | 0.288 | 0.355 | 0.384 | 0.532 | 0.016 | 0 |
| poll loop, get-ready | 41478 | 3010 /s | 2768 /s | 0% / 0% / 0% | 0.361 | 0.297 | 0.362 | 0.384 | 0.492 | 0.013 | 0 |
| poll loop, connect | 10893 | 3200 /s | 3103 /s | 0% / 0% / 0% | 0.322 | 0.288 | 0.321 | 0.344 | 0.408 | 0.010 | 0 |
| poll loop, rest | 13755 | 2870 /s | 2753 /s | 0% / 0% / 0% | 0.363 | 0.320 | 0.363 | 0.387 | 0.485 | 0.014 | 0 |
| poll loop, flicks | 27762 | 2900 /s | 2780 /s | 0% / 0% / 0% | 0.360 | 0.323 | 0.358 | 0.383 | 0.499 | 0.013 | 0 |
| poll loop, circles | 33530 | 2910 /s | 2798 /s | 0% / 0% / 0% | 0.357 | 0.307 | 0.353 | 0.387 | 0.498 | 0.014 | 0 |
| poll loop, slow-circles | 41743 | 2890 /s | 2786 /s | 0% / 0% / 0% | 0.359 | 0.319 | 0.354 | 0.387 | 0.523 | 0.014 | 0 |
| poll loop, small-slow | 33309 | 2860 /s | 2778 /s | 0% / 0% / 0% | 0.360 | 0.323 | 0.357 | 0.383 | 0.532 | 0.013 | 0 |
| poll loop, extremes | 33318 | 2890 /s | 2780 /s | 0% / 0% / 0% | 0.360 | 0.323 | 0.356 | 0.385 | 0.512 | 0.013 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 2310 | 154 | 18.5 | 96% |
| connect | 3600 | 1102 | 250.0 | 98% |
| rest | 826 | 165 | 7.3 | 100% |
| flicks | 1651 | 165 | 7.5 | 100% |
| circles | 1979 | 165 | 11.3 | 100% |
| slow-circles | 2476 | 165 | 11.9 | 100% |
| small-slow | 1981 | 165 | 12.2 | 100% |
| extremes | 1981 | 165 | 7.9 | 100% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

### EdgeTX Radiomaster Pocket Joystick — Radio (EdgeTX USB joystick, e.g. Radiomaster Pocket)

**Compared with the research**

- Report rate: expected about 1000 Hz with RF off. Busiest 100 ms: 1040 updates per second. Gaps between updates on a 1 ms grid: 99.7%, 2 ms: 22.2%, 4 ms: 2.0%. Average while moving: 640 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Stick axes at start: centred [0, 1, 3], at the bottom [2]. EdgeTX's AETR order expects roll 0, pitch 1, throttle 2 (rests at the bottom), yaw 3.
- Axes: expected 8 (CH1-CH8); SDL reports 8; 4 of them moved during the run (axes [0, 1, 2, 3]). MATCHES
- Buttons: expected 24 (CH9-CH32); SDL reports 24; pressed during the run: [].
- Resolution: expected about 11 bits (0-2048); measured about 11.0 bits on the busiest axes. MATCHES
- Yaw-only step: the sticks were not moved in this step, so it can't single out the yaw axis.
- Channels 5-8 (axes 4-7): moved = []. They only move if the radio model mixes switches or the pot onto CH5-CH8.
- Window-not-focused step: no stick movement recorded, so focus independence is untested in this run.
- Hands-off step: 0 axis changes. SDL only reports changes: a perfectly still stick sends nothing, and only sensor noise or a touch shows up. Silence alone can't tell a resting stick from a stalled device.
- Hands-off step, stick noise (issue #30): axis 0: 0 changes, spread 0 counts; axis 1: 0 changes, spread 0 counts; axis 2: 0 changes, spread 0 counts; axis 3: 0 changes, spread 0 counts.

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

**Update rate.** Counts the moments when any axis changed. A new value can only appear when the device sends a report, but a slow stick doesn't change on every report, so averages understate the report rate. Two measures don't depend on stick speed: the **busiest 100 ms**, and the **grid test** (a device reporting every N ms only produces gaps that are whole multiples of N). SDL stamps a change when the input thread polls (about every 0.36 ms here), so single gaps wobble by up to one poll period.

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| any axis, all steps | 30102 | 1040 /s | 640 /s | 100% / 22% / 2% | 1.562 | 0.367 | 1.080 | 3.185 | 49.213 | 1.928 | 85 |
| any axis, flicks | 2064 | 990 /s | 694 /s | 100% / 20% / 2% | 1.441 | 0.678 | 1.077 | 3.073 | 25.927 | 1.238 | 13 |
| any axis, circles | 9597 | 1040 /s | 903 /s | 100% / 9% / 0% | 1.108 | 0.368 | 1.060 | 2.102 | 37.955 | 0.580 | 0 |
| any axis, slow-circles | 10657 | 1020 /s | 727 /s | 100% / 23% / 1% | 1.376 | 0.367 | 1.083 | 2.826 | 47.901 | 1.118 | 3 |
| any axis, small-slow | 2470 | 620 /s | 319 /s | 100% / 47% / 10% | 3.132 | 0.694 | 2.141 | 7.747 | 49.213 | 3.865 | 33 |
| any axis, extremes | 4779 | 990 /s | 529 /s | 100% / 34% / 4% | 1.890 | 0.394 | 1.147 | 3.908 | 49.016 | 2.097 | 11 |
| axis 0, circles | 4203 | 990 /s | 649 /s | 100% / 33% / 2% | 1.540 | 0.677 | 1.098 | 2.966 | 48.911 | 1.144 | 22 |
| axis 1, circles | 3913 | 990 /s | 679 /s | 100% / 32% / 1% | 1.473 | 0.662 | 1.088 | 2.849 | 20.823 | 0.961 | 23 |
| axis 2, circles | 3537 | 1010 /s | 760 /s | 100% / 18% / 1% | 1.315 | 0.672 | 1.066 | 2.211 | 47.856 | 1.317 | 26 |
| axis 3, circles | 4345 | 960 /s | 644 /s | 100% / 36% / 1% | 1.552 | 0.678 | 1.100 | 2.875 | 37.776 | 1.146 | 23 |

Gaps between updates, any axis, all steps:

| Interval | Count | Share |
|---|---|---|
| 0.25-0.50 ms | 83 | 0.3% |
| 0.50-0.75 ms | 4470 | 14.9% |
| 0.75-1.25 ms | 16696 | 55.6% |
| 1.25-1.75 ms | 112 | 0.4% |
| 1.75-2.50 ms | 5775 | 19.2% |
| 2.50-3.50 ms | 1694 | 5.6% |
| 3.50-4.50 ms | 495 | 1.6% |
| 4.50-6.00 ms | 275 | 0.9% |
| 6.00-8.00 ms | 175 | 0.6% |
| 8.00-12.00 ms | 94 | 0.3% |
| 12.00-20.00 ms | 73 | 0.2% |
| 20.00-50.00 ms | 74 | 0.2% |

**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)

| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |
|---|---|---|---|---|---|---|---|
| 0 | -32768 | 32767 | 1638 | 31 | 32 | 2049 | 11.0 |
| 1 | -32768 | 32767 | 1616 | 31 | 32 | 2049 | 11.0 |
| 2 | -32768 | 32767 | 1700 | 31 | 32 | 2049 | 11.0 |
| 3 | -32768 | 32767 | 1633 | 31 | 32 | 2049 | 11.0 |
| 4 | 0 | 0 | 0 | - | - | - | - |
| 5 | 0 | 0 | 0 | - | - | - | - |
| 6 | 0 | 0 | 0 | - | - | - | - |
| 7 | 0 | 0 | 0 | - | - | - | - |

**How the values move** (issue #30). Every change on stick axes 0-3, sized in EdgeTX counts (1 count = 32 SDL units). EdgeTX's ADC filter lets through only 1-2 count creeps and jumps of about 10 counts or more, so a filtered stick shows almost nothing in the 3-9 column. With the filter off, or RF on, changes should spread over all sizes. Gaps are in report periods (1.00 ms).

| Step | Axis | Changes | Per moving s | 1-2 counts | 3-9 counts | 10+ counts | Median jump | Gap 1 / 2 / 3 / 4+ reports | On report grid |
|---|---|---|---|---|---|---|---|---|---|
| circles | 0 | 4202 | 653 | 1269 | 49 | 2884 | 16 | 2526 / 1309 / 238 / 107 | 99% |
| circles | 1 | 3912 | 683 | 922 | 49 | 2941 | 17 | 2494 / 1169 / 146 / 80 | 99% |
| circles | 2 | 3536 | 766 | 577 | 16 | 2943 | 16 | 2768 / 609 / 88 / 45 | 99% |
| circles | 3 | 4344 | 648 | 1440 | 53 | 2851 | 16 | 2529 / 1507 / 196 / 89 | 99% |
| extremes | 0 | 1481 | 410 | 975 | 6 | 500 | 14 | 421 / 563 / 287 / 203 | 99% |
| extremes | 1 | 1387 | 480 | 740 | 12 | 635 | 14 | 434 / 567 / 267 / 114 | 100% |
| extremes | 2 | 1195 | 459 | 510 | 2 | 683 | 14 | 501 / 471 / 143 / 70 | 99% |
| extremes | 3 | 1689 | 379 | 1190 | 13 | 486 | 14 | 451 / 621 / 326 / 284 | 100% |
| flicks | 0 | 2018 | 716 | 468 | 51 | 1499 | 37 | 1480 / 364 / 118 / 44 | 99% |
| flicks | 1 | 64 | 194 | 57 | 0 | 7 | 14 | 4 / 17 / 9 / 29 | 92% |
| slow-circles | 0 | 4323 | 493 | 2180 | 37 | 2106 | 14 | 1642 / 1687 / 657 / 321 | 100% |
| slow-circles | 1 | 3501 | 553 | 1281 | 34 | 2186 | 16 | 1564 / 1395 / 380 / 143 | 99% |
| slow-circles | 2 | 2379 | 705 | 523 | 11 | 1845 | 17 | 1873 / 361 / 76 / 50 | 99% |
| slow-circles | 3 | 4715 | 465 | 2573 | 48 | 2094 | 14 | 1613 / 1843 / 851 / 393 | 100% |
| small-slow | 0 | 1078 | 340 | 877 | 15 | 186 | 13 | 183 / 406 / 237 / 239 | 99% |
| small-slow | 1 | 891 | 302 | 774 | 3 | 114 | 14 | 115 / 292 / 218 / 258 | 99% |
| small-slow | 2 | 603 | 210 | 603 | 0 | 0 | - | 203 / 102 / 58 / 187 | 91% |

**What moved in each step** (axis: number of changes)

| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |
|---|---|---|---|---|
| connect | {} | {} | 0 | None |
| rest | {} | {} | 0 | None |
| circles | {0: 4203, 1: 3913, 2: 3538, 3: 4346} | {} | 0 | Some(3) |
| yaw | {} | {} | 0 | None |
| extremes | {0: 1482, 1: 1388, 2: 1196, 3: 1690} | {} | 0 | Some(3) |
| switches | {} | {} | 0 | None |
| unfocused | {} | {} | 0 | None |
| sensors | {} | {} | 0 | None |
| sensors-rest | {} | {} | 0 | None |
| flicks | {0: 2020, 1: 66} | {} | 0 | Some(0) |
| slow-circles | {0: 4324, 1: 3502, 2: 2380, 3: 4716} | {} | 0 | Some(3) |
| small-slow | {0: 1079, 1: 892, 2: 604} | {} | 0 | Some(0) |

