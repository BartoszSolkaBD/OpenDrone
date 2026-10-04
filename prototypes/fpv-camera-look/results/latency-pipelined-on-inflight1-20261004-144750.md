# PROTOTYPE #28 latency run, 20261004-144750

- Machine: Mac mini M4 (10-core GPU), macOS, Bevy 0.19.1, Metal. Window 2560x1440, VSync off (AutoNoVsync).
- Pipelined rendering: **ON**. Frames allowed in flight on the GPU: **1**. Quad: Whoop 65. FOV 160 deg, Camera Tilt 30 deg, Breakup Realistic.
- Output: OFFSCREEN 2560x1440 image, no window (no present, no compositor): the GPU work is the same except the final copy to the screen.
- Values are median / p95 / p99 in ms. Frame time = wall clock between frames (at most 3 frames in flight, VSync off). GPU times are Metal timestamps at the END of each pass: mip chain = Map render done -> last mip done; merged pass = last mip done -> merged pass done.

Synthetic stick steps at random moments (150-350 ms apart), 12 s, camera hovering at the Launch Spot.

| Measured from the input sample to... | Samples | median ms | p95 ms | p99 ms |
|---|---|---|---|---|
| the frame that used it was handed to the GPU and presented (CPU side) | 48 | 16.40 | 19.35 | 19.82 |
| the CPU saw the GPU finish that frame (upper bound) | 48 | 21.88 | 25.09 | 25.73 |

Frame time during the test: median 6.35 ms, p95 7.23 ms (158 fps). The step is applied in the next frame's update, so the expected cost of pipelining is about one extra frame.

Not included: USB and radio, the Radio Link emulation (#21), the physics step, the compositor and the display's scan-out (about half to one refresh, 3-6 ms at 165 Hz).

Raw (present/gpu-done ms): 14.62/19.84 16.40/21.70 16.83/21.90 12.53/18.03 18.03/23.60 13.79/19.41 16.81/22.01 16.54/22.07 18.11/23.17 15.02/20.40 13.39/19.04 16.95/22.51 13.63/20.18 16.70/22.26 16.95/22.60 15.16/20.80 19.35/25.09 15.40/21.00 16.49/21.92 16.72/22.11 17.90/23.60 14.78/20.38 16.29/21.77 14.85/20.41 15.56/21.38 16.31/22.62 17.35/22.88 14.59/20.19 12.15/17.60 13.81/18.94 15.56/21.15 18.46/24.00 14.99/21.11 16.76/23.36 13.84/19.37 17.30/22.98 16.66/22.74 13.51/19.07 16.59/22.03 15.04/20.54 13.01/18.01 19.18/24.75 16.46/21.88 13.01/18.24 12.27/17.90 19.76/25.73 16.52/22.13 19.82/25.31

> Agent run on 2026-10-04, with the Mac locked and its display asleep, so it ran windowless (--offscreen). Indicative only: the maintainer's run with the window on screen is the real measurement.
