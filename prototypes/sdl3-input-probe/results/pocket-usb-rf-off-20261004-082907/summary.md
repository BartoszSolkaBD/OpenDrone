# SDL3 input probe: pocket-usb-rf-off

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 08:29:07 (UTC), lasted 10.2 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 7.1 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Rate (1 / mean) | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 34500 | 3444 Hz | 0.290 | 0.257 | 0.290 | 0.295 | 0.365 | 0.004 | 0 |
| poll loop, connect | 34500 | 3444 Hz | 0.290 | 0.257 | 0.290 | 0.295 | 0.365 | 0.004 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| connect | 1634 | 163 | 135.6 | 100% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

No Input Device was connected during this run.
