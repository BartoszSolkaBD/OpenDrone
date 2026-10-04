# PROTOTYPE #28 latency run, 20261004-144735

- Machine: Mac mini M4 (10-core GPU), macOS, Bevy 0.19.1, Metal. Window 2560x1440, VSync off (AutoNoVsync).
- Pipelined rendering: **OFF**. Frames allowed in flight on the GPU: **1**. Quad: Whoop 65. FOV 160 deg, Camera Tilt 30 deg, Breakup Realistic.
- Output: OFFSCREEN 2560x1440 image, no window (no present, no compositor): the GPU work is the same except the final copy to the screen.
- Values are median / p95 / p99 in ms. Frame time = wall clock between frames (at most 3 frames in flight, VSync off). GPU times are Metal timestamps at the END of each pass: mip chain = Map render done -> last mip done; merged pass = last mip done -> merged pass done.

Synthetic stick steps at random moments (150-350 ms apart), 12 s, camera hovering at the Launch Spot.

| Measured from the input sample to... | Samples | median ms | p95 ms | p99 ms |
|---|---|---|---|---|
| the frame that used it was handed to the GPU and presented (CPU side) | 48 | 9.70 | 13.37 | 14.10 |
| the CPU saw the GPU finish that frame (upper bound) | 48 | 15.95 | 20.31 | 20.37 |

Frame time during the test: median 7.17 ms, p95 8.17 ms (139 fps). The step is applied in the next frame's update, so the expected cost of pipelining is about one extra frame.

Not included: USB and radio, the Radio Link emulation (#21), the physics step, the compositor and the display's scan-out (about half to one refresh, 3-6 ms at 165 Hz).

Raw (present/gpu-done ms): 11.48/17.87 9.32/16.11 12.11/18.91 11.17/17.40 12.46/17.85 11.61/18.00 10.91/16.84 12.23/18.89 9.59/15.67 11.37/17.44 14.10/20.37 11.48/17.73 8.25/13.87 12.71/19.14 7.57/12.70 13.37/20.33 8.81/13.76 11.01/16.61 5.65/10.39 8.34/12.93 6.78/11.36 7.50/12.13 9.75/14.44 9.48/14.46 7.14/13.17 8.35/14.53 7.16/13.29 6.95/13.22 6.83/12.37 12.87/18.94 8.68/14.30 9.70/15.86 9.85/16.29 8.31/14.57 7.54/14.02 11.85/17.71 9.39/15.68 13.21/20.31 8.28/14.26 10.51/16.80 11.34/17.42 9.31/15.95 8.21/14.29 13.41/19.79 8.55/14.83 7.15/13.23 13.19/19.53 13.32/19.67

> Agent run on 2026-10-04, with the Mac locked and its display asleep, so it ran windowless (--offscreen). Indicative only: the maintainer's run with the window on screen is the real measurement.
