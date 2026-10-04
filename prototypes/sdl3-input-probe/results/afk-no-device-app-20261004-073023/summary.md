# SDL3 input probe: afk-no-device-app

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 07:30:23 (UTC), lasted 15.2 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as its own .app bundle (macOS treats it as a separate app for permissions)
- QUICK mode (short steps, for a no-device smoke test)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 5.2 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Rate (1 / mean) | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 45467 | 3026 Hz | 0.330 | 0.265 | 0.330 | 0.337 | 0.381 | 0.004 | 0 |
| poll loop, get-ready | 9156 | 3022 Hz | 0.331 | 0.274 | 0.330 | 0.337 | 0.364 | 0.003 | 0 |
| poll loop, connect | 8985 | 3037 Hz | 0.329 | 0.280 | 0.330 | 0.337 | 0.360 | 0.005 | 0 |
| poll loop, rest | 4572 | 3022 Hz | 0.331 | 0.273 | 0.330 | 0.337 | 0.360 | 0.004 | 0 |
| poll loop, circles | 4526 | 3022 Hz | 0.331 | 0.265 | 0.330 | 0.337 | 0.359 | 0.004 | 0 |
| poll loop, yaw | 4527 | 3023 Hz | 0.331 | 0.268 | 0.330 | 0.337 | 0.350 | 0.004 | 0 |
| poll loop, extremes | 4536 | 3028 Hz | 0.330 | 0.312 | 0.330 | 0.338 | 0.381 | 0.005 | 0 |
| poll loop, switches | 4533 | 3025 Hz | 0.331 | 0.269 | 0.330 | 0.338 | 0.359 | 0.004 | 0 |
| poll loop, unfocused | 4632 | 3023 Hz | 0.331 | 0.286 | 0.330 | 0.337 | 0.364 | 0.003 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 182 | 60 | 19.2 | 0% |
| connect | 181 | 62 | 19.3 | 0% |
| rest | 91 | 60 | 20.2 | 0% |
| circles | 90 | 60 | 19.3 | 0% |
| yaw | 90 | 60 | 18.9 | 0% |
| extremes | 90 | 60 | 27.6 | 0% |
| switches | 90 | 60 | 20.1 | 0% |
| unfocused | 92 | 60 | 19.0 | 0% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Unknown (never asked)**, at end **Unknown (never asked)**.
- What it means: Nothing asked macOS for Input Monitoring during the run, so no prompt can have come from this app.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

No Input Device was connected during this run.
