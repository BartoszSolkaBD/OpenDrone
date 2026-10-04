# SDL3 input probe: afk-no-device-terminal

PROTOTYPE output for [issue #18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18). Raw data: `events.csv` (every change SDL reported) and `stats.json`.

- Started: 2026-10-04 07:30:05 (UTC), lasted 15.3 s, macOS 26.6.2 (aarch64)
- SDL 3.4.18 (SDL-3.4.18), built from source, statically linked, joystick + HIDAPI only
- Bevy 0.19.1 window open on the main thread; Bevy's own gamepad plugin (gilrs) ON (reading the same devices at the same time)
- Launched as a plain binary from a terminal (macOS attributes permissions to the terminal app)
- QUICK mode (short steps, for a no-device smoke test)

## 1. Does SDL run on its own thread while Bevy/winit owns the main thread?

- SDL_Init(GAMEPAD) on thread `sdl-input`: OK in 6.3 ms
- `pthread_main_np()` on that thread = 0 (not the main thread).
- Thread priority (QoS user-interactive) requested: result 0 (OK).
- The input thread slept 250 µs between polls. Its loop timing (how often it actually ran):

| What | Samples | Rate (1 / mean) | Mean ms | Min ms | Median ms | p95 ms | Max ms | Jitter (std) ms | Pauses >50 ms skipped |
|---|---|---|---|---|---|---|---|---|---|
| poll loop, whole run | 46095 | 3061 Hz | 0.327 | 0.263 | 0.329 | 0.336 | 0.378 | 0.006 | 0 |
| poll loop, get-ready | 9318 | 3058 Hz | 0.327 | 0.276 | 0.329 | 0.336 | 0.355 | 0.005 | 0 |
| poll loop, connect | 8995 | 3076 Hz | 0.325 | 0.302 | 0.325 | 0.335 | 0.378 | 0.006 | 0 |
| poll loop, rest | 4620 | 3051 Hz | 0.328 | 0.302 | 0.329 | 0.336 | 0.351 | 0.006 | 0 |
| poll loop, circles | 4630 | 3061 Hz | 0.327 | 0.267 | 0.329 | 0.336 | 0.348 | 0.006 | 0 |
| poll loop, yaw | 4630 | 3057 Hz | 0.327 | 0.263 | 0.329 | 0.336 | 0.347 | 0.006 | 0 |
| poll loop, extremes | 4582 | 3056 Hz | 0.327 | 0.302 | 0.329 | 0.336 | 0.347 | 0.006 | 0 |
| poll loop, switches | 4630 | 3057 Hz | 0.327 | 0.293 | 0.329 | 0.336 | 0.345 | 0.006 | 0 |
| poll loop, unfocused | 4690 | 3063 Hz | 0.327 | 0.280 | 0.329 | 0.335 | 0.354 | 0.006 | 0 |

Main thread (Bevy rendering the window) at the same time:

| Step | Frames | Avg fps | Longest frame ms | Window focused |
|---|---|---|---|---|
| get-ready | 183 | 60 | 19.3 | 0% |
| connect | 179 | 62 | 19.3 | 0% |
| rest | 91 | 60 | 19.0 | 0% |
| circles | 91 | 60 | 18.8 | 0% |
| yaw | 91 | 60 | 19.1 | 0% |
| extremes | 90 | 60 | 19.3 | 0% |
| switches | 91 | 60 | 19.0 | 0% |
| unfocused | 92 | 60 | 19.4 | 0% |

## 2. Input Monitoring permission

- macOS `IOHIDCheckAccess(ListenEvent)` (reads the status, never prompts): at start **Denied**, at end **Denied**.
- What it means: The app macOS holds responsible (the terminal, unless launched with run.sh) is refused Input Monitoring. If the devices below still delivered input, Input Monitoring is not needed to read them.
- Also record whether any macOS permission dialog appeared during the run.

## 3. Devices

No Input Device was connected during this run.
