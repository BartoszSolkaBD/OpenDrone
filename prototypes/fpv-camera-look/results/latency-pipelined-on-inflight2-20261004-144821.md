# PROTOTYPE #28 latency run, 20261004-144821

- Machine: Mac mini M4 (10-core GPU), macOS, Bevy 0.19.1, Metal. Window 2560x1440, VSync off (AutoNoVsync).
- Pipelined rendering: **ON**. Frames allowed in flight on the GPU: **2**. Quad: Whoop 65. FOV 160 deg, Camera Tilt 30 deg, Breakup Realistic.
- Output: OFFSCREEN 2560x1440 image, no window (no present, no compositor): the GPU work is the same except the final copy to the screen.
- Values are median / p95 / p99 in ms. Frame time = wall clock between frames (at most 3 frames in flight, VSync off). GPU times are Metal timestamps at the END of each pass: mip chain = Map render done -> last mip done; merged pass = last mip done -> merged pass done.

Synthetic stick steps at random moments (150-350 ms apart), 12 s, camera hovering at the Launch Spot.

| Measured from the input sample to... | Samples | median ms | p95 ms | p99 ms |
|---|---|---|---|---|
| the frame that used it was handed to the GPU and presented (CPU side) | 47 | 12.95 | 18.16 | 18.35 |
| the CPU saw the GPU finish that frame (upper bound) | 47 | 22.07 | 26.95 | 28.21 |

Frame time during the test: median 6.78 ms, p95 8.31 ms (147 fps). The step is applied in the next frame's update, so the expected cost of pipelining is about one extra frame.

Not included: USB and radio, the Radio Link emulation (#21), the physics step, the compositor and the display's scan-out (about half to one refresh, 3-6 ms at 165 Hz).

Raw (present/gpu-done ms): 18.16/28.21 18.35/27.77 11.40/20.32 15.04/24.50 10.11/19.22 10.99/19.89 16.83/26.30 11.93/21.13 14.35/23.59 15.43/24.69 13.61/23.31 14.02/23.27 15.71/25.02 14.08/23.16 18.18/26.95 11.19/19.78 11.28/18.63 10.11/17.50 12.63/22.07 11.23/18.51 11.44/18.62 13.06/22.30 15.67/24.80 12.48/21.81 12.95/21.99 14.28/23.33 11.28/19.41 12.68/21.92 14.92/25.05 14.96/24.37 10.96/20.27 15.54/25.03 10.74/17.98 15.00/24.49 10.75/18.26 17.55/26.72 9.90/19.18 15.68/24.69 11.23/18.59 11.06/18.28 15.62/24.85 9.52/16.84 11.59/20.84 11.31/20.67 11.21/20.43 16.50/26.55 16.95/26.37

> Agent run on 2026-10-04, with the Mac locked and its display asleep, so it ran windowless (--offscreen). Indicative only: the maintainer's run with the window on screen is the real measurement.
