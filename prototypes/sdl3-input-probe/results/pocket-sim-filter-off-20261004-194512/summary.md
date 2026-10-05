# SDL3 input probe: pocket-sim-filter-off

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 19:45:12 (UTC), lasted 84.5 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 39.0 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Busiest 100 ms | Average (1 / mean) | On 1 / 2 / 4 ms grid | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 236524 | 3180 /s | 2810 /s | 0% / 0% / 0% | 0.356 | 0.279 | 0.352 | 0.386 | 0.530 | 0.016 | 0 |
| poll loop, get-ready | 41923 | 3000 /s | 2796 /s | 0% / 0% / 0% | 0.358 | 0.309 | 0.353 | 0.388 | 0.484 | 0.015 | 0 |
| poll loop, connect | 9310 | 3150 /s | 2856 /s | 0% / 0% / 0% | 0.350 | 0.310 | 0.354 | 0.377 | 0.461 | 0.018 | 0 |
| poll loop, rest | 13964 | 2910 /s | 2795 /s | 0% / 0% / 0% | 0.358 | 0.311 | 0.353 | 0.386 | 0.516 | 0.015 | 0 |
| poll loop, flicks | 28090 | 2910 /s | 2812 /s | 0% / 0% / 0% | 0.356 | 0.317 | 0.352 | 0.385 | 0.495 | 0.014 | 0 |
| poll loop, circles | 33538 | 2900 /s | 2797 /s | 0% / 0% / 0% | 0.358 | 0.333 | 0.353 | 0.391 | 0.508 | 0.014 | 0 |
| poll loop, slow-circles | 42505 | 3180 /s | 2837 /s | 0% / 0% / 0% | 0.353 | 0.279 | 0.352 | 0.385 | 0.528 | 0.019 | 0 |
| poll loop, small-slow | 33663 | 2910 /s | 2808 /s | 0% / 0% / 0% | 0.356 | 0.321 | 0.352 | 0.385 | 0.498 | 0.014 | 0 |
| poll loop, extremes | 33531 | 2910 /s | 2798 /s | 0% / 0% / 0% | 0.357 | 0.322 | 0.353 | 0.388 | 0.530 | 0.014 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 2477 | 165 | 17.7 | 100% |
| connect | 552 | 167 | 12.1 | 100% |
| rest | 826 | 165 | 7.4 | 100% |
| flicks | 1650 | 165 | 12.2 | 100% |
| circles | 1982 | 165 | 7.3 | 100% |
| slow-circles | 2476 | 165 | 11.9 | 100% |
| small-slow | 1982 | 165 | 7.0 | 100% |
| extremes | 1980 | 165 | 11.7 | 100% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

### EdgeTX Radiomaster Pocket Joystick — Radio (EdgeTX USB joystick, e.g. Radiomaster Pocket)

**Compared with the research**

- Report rate: expected about 1000 Hz with RF off. Busiest 100 ms: 1050 updates per second. Gaps between updates on a 1 ms grid: 99.6%, 2 ms: 7.7%, 4 ms: 0.7%. Average while moving: 871 per second (lower whenever a stick moves too slowly to change value on every report). MATCHES
- Stick axes at start: centred [0, 1, 3], at the bottom []. EdgeTX's AETR order expects roll 0, pitch 1, throttle 2 (rests at the bottom), yaw 3.
- Axes: expected 8 (CH1-CH8); SDL reports 8; 5 of them moved during the run (axes [0, 1, 2, 3, 7]). MATCHES
- Buttons: expected 24 (CH9-CH32); SDL reports 24; pressed during the run: [].
- Resolution: expected about 11 bits (0-2048); measured about 11.0 bits on the busiest axes. MATCHES
- Yaw-only step: the sticks were not moved in this step, so it can't single out the yaw axis.
- Channels 5-8 (axes 4-7): moved = [7]. They only move if the radio model mixes switches or the pot onto CH5-CH8.
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
| any axis, all steps | 60844 | 1050 /s | 871 /s | 100% / 8% / 1% | 1.147 | 0.329 | 1.060 | 2.126 | 40.229 | 0.651 | 10 |
| any axis, flicks | 7961 | 1030 /s | 796 /s | 100% / 13% / 1% | 1.256 | 0.361 | 1.062 | 2.829 | 9.250 | 0.728 | 0 |
| any axis, circles | 11817 | 1050 /s | 984 /s | 99% / 1% / 0% | 1.016 | 0.364 | 1.059 | 1.134 | 40.229 | 0.455 | 0 |
| any axis, slow-circles | 13649 | 1050 /s | 934 /s | 99% / 4% / 0% | 1.071 | 0.329 | 1.057 | 1.363 | 28.242 | 0.557 | 4 |
| any axis, small-slow | 10505 | 1030 /s | 875 /s | 100% / 9% / 0% | 1.143 | 0.370 | 1.061 | 2.121 | 8.117 | 0.512 | 0 |
| any axis, extremes | 9871 | 1050 /s | 904 /s | 99% / 6% / 0% | 1.107 | 0.360 | 1.060 | 2.052 | 38.935 | 0.697 | 6 |
| axis 0, circles | 5826 | 1010 /s | 953 /s | 100% / 3% / 0% | 1.050 | 0.680 | 1.060 | 1.153 | 7.815 | 0.328 | 21 |
| axis 1, circles | 4992 | 1010 /s | 955 /s | 100% / 2% / 0% | 1.047 | 0.678 | 1.060 | 1.149 | 36.958 | 0.603 | 22 |
| axis 2, circles | 4658 | 1010 /s | 943 /s | 100% / 3% / 0% | 1.061 | 0.678 | 1.061 | 1.154 | 43.128 | 0.707 | 21 |
| axis 3, circles | 6474 | 1010 /s | 951 /s | 100% / 2% / 0% | 1.052 | 0.674 | 1.060 | 1.146 | 40.229 | 0.603 | 21 |

Gaps between updates, any axis, all steps:

| Interval | Count | Share |
|---|---|---|
| 0.25-0.50 ms | 248 | 0.4% |
| 0.50-0.75 ms | 10891 | 17.9% |
| 0.75-1.25 ms | 43502 | 71.5% |
| 1.25-1.75 ms | 329 | 0.5% |
| 1.75-2.50 ms | 4058 | 6.7% |
| 2.50-3.50 ms | 1103 | 1.8% |
| 3.50-4.50 ms | 408 | 0.7% |
| 4.50-6.00 ms | 181 | 0.3% |
| 6.00-8.00 ms | 78 | 0.1% |
| 8.00-12.00 ms | 25 | 0.0% |
| 12.00-20.00 ms | 4 | 0.0% |
| 20.00-50.00 ms | 6 | 0.0% |

**Resolution** (SDL scales every axis to -32768..32767; the step between neighbouring values shows the device's real resolution)

| Axis | Min | Max | Distinct values | Smallest step | Usual step | Levels over full range | Bits |
|---|---|---|---|---|---|---|---|
| 0 | -32768 | 32767 | 1640 | 31 | 32 | 2049 | 11.0 |
| 1 | -32768 | 32767 | 1618 | 31 | 32 | 2049 | 11.0 |
| 2 | -32768 | 32767 | 1727 | 31 | 32 | 2049 | 11.0 |
| 3 | -32768 | 32767 | 1634 | 31 | 32 | 2049 | 11.0 |
| 4 | 0 | 0 | 0 | - | - | - | - |
| 5 | 0 | 0 | 0 | - | - | - | - |
| 6 | 0 | 0 | 0 | - | - | - | - |
| 7 | -32768 | 32767 | 2 | 65535 | 65535 | 2 | 1.0 |

**How the values move** (issue #30). Every change on stick axes 0-3, sized in EdgeTX counts (1 count = 32 SDL units). EdgeTX's ADC filter lets through only 1-2 count creeps and jumps of about 10 counts or more, so a filtered stick shows almost nothing in the 3-9 column. With the filter off, or RF on, changes should spread over all sizes. Gaps are in report periods (1.00 ms).

| Step | Axis | Changes | Per moving s | 1-2 counts | 3-9 counts | 10+ counts | Median jump | Gap 1 / 2 / 3 / 4+ reports | On report grid |
|---|---|---|---|---|---|---|---|---|---|
| circles | 0 | 5825 | 956 | 725 | 3306 | 1794 | 11 | 5586 / 176 / 25 / 17 | 100% |
| circles | 1 | 4991 | 960 | 426 | 2398 | 2167 | 12 | 4827 / 109 / 22 / 11 | 100% |
| circles | 2 | 4657 | 947 | 737 | 1662 | 2258 | 14 | 4466 / 123 / 26 / 21 | 100% |
| circles | 3 | 6473 | 954 | 727 | 4657 | 1089 | 11 | 6258 / 137 / 31 / 26 | 100% |
| extremes | 0 | 4143 | 766 | 2224 | 1819 | 100 | 26 | 3359 / 555 / 148 / 77 | 100% |
| extremes | 1 | 3106 | 875 | 1181 | 1888 | 37 | 10 | 2766 / 286 / 29 / 18 | 100% |
| extremes | 2 | 3327 | 852 | 1566 | 1620 | 141 | 15 | 2895 / 320 / 74 / 33 | 100% |
| extremes | 3 | 4427 | 785 | 2573 | 1847 | 7 | 10 | 3571 / 639 / 143 / 67 | 100% |
| flicks | 0 | 3515 | 928 | 710 | 1727 | 1078 | 30 | 3284 / 187 / 17 / 10 | 99% |
| flicks | 1 | 783 | 612 | 650 | 133 | 0 | - | 544 / 143 / 46 / 40 | 99% |
| flicks | 2 | 6748 | 675 | 6086 | 662 | 0 | - | 4681 / 1357 / 425 / 285 | 100% |
| slow-circles | 0 | 5260 | 881 | 1346 | 3869 | 45 | 10 | 4873 / 270 / 55 / 52 | 100% |
| slow-circles | 1 | 5490 | 917 | 1402 | 4033 | 55 | 10 | 5124 / 308 / 26 / 21 | 100% |
| slow-circles | 2 | 4745 | 887 | 1274 | 2781 | 690 | 12 | 4329 / 289 / 62 / 54 | 100% |
| slow-circles | 3 | 6631 | 878 | 2508 | 4097 | 26 | 10 | 5970 / 531 / 72 / 44 | 100% |
| small-slow | 0 | 4235 | 711 | 3208 | 1027 | 0 | - | 3064 / 785 / 266 / 102 | 100% |
| small-slow | 1 | 4377 | 714 | 3044 | 1333 | 0 | - | 3266 / 779 / 204 / 118 | 100% |
| small-slow | 2 | 7979 | 665 | 5703 | 2276 | 0 | - | 5504 / 1558 / 535 / 382 | 100% |

**What moved in each step** (axis: number of changes)

| Step | Axis changes | Buttons pressed | Hat changes | Most active axis |
|---|---|---|---|---|
| connect | {} | {} | 0 | None |
| rest | {} | {} | 0 | None |
| circles | {0: 5826, 1: 4992, 2: 4658, 3: 6475, 7: 5} | {} | 0 | Some(3) |
| yaw | {} | {} | 0 | None |
| extremes | {0: 4144, 1: 3107, 2: 3328, 3: 4428} | {} | 0 | Some(3) |
| switches | {} | {} | 0 | None |
| unfocused | {} | {} | 0 | None |
| sensors | {} | {} | 0 | None |
| sensors-rest | {} | {} | 0 | None |
| flicks | {0: 3517, 1: 785, 2: 6749} | {} | 0 | Some(2) |
| slow-circles | {0: 5261, 1: 5491, 2: 4746, 3: 6632} | {} | 0 | Some(3) |
| small-slow | {0: 4236, 1: 4378, 2: 7980} | {} | 0 | Some(2) |

