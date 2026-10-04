# PROTOTYPE #28 latency run, 20261004-144806

- Machine: Mac mini M4 (10-core GPU), macOS, Bevy 0.19.1, Metal. Window 2560x1440, VSync off (AutoNoVsync).
- Pipelined rendering: **OFF**. Frames allowed in flight on the GPU: **2**. Quad: Whoop 65. FOV 160 deg, Camera Tilt 30 deg, Breakup Realistic.
- Output: OFFSCREEN 2560x1440 image, no window (no present, no compositor): the GPU work is the same except the final copy to the screen.
- Values are median / p95 / p99 in ms. Frame time = wall clock between frames (at most 3 frames in flight, VSync off). GPU times are Metal timestamps at the END of each pass: mip chain = Map render done -> last mip done; merged pass = last mip done -> merged pass done.

Synthetic stick steps at random moments (150-350 ms apart), 12 s, camera hovering at the Launch Spot.

| Measured from the input sample to... | Samples | median ms | p95 ms | p99 ms |
|---|---|---|---|---|
| the frame that used it was handed to the GPU and presented (CPU side) | 47 | 7.56 | 9.47 | 10.05 |
| the CPU saw the GPU finish that frame (upper bound) | 47 | 14.09 | 18.56 | 18.74 |

Frame time during the test: median 6.07 ms, p95 8.22 ms (165 fps). The step is applied in the next frame's update, so the expected cost of pipelining is about one extra frame.

Not included: USB and radio, the Radio Link emulation (#21), the physics step, the compositor and the display's scan-out (about half to one refresh, 3-6 ms at 165 Hz).

Raw (present/gpu-done ms): 8.97/18.74 10.05/16.47 7.46/13.58 8.29/14.60 7.96/16.97 5.26/11.66 2.78/8.84 8.83/15.18 8.61/15.02 9.27/16.65 5.41/11.77 8.24/17.18 5.14/11.29 8.98/17.63 7.58/13.94 8.42/14.56 9.12/15.63 4.03/11.58 7.18/16.16 7.56/16.45 7.67/13.96 9.85/18.74 2.58/8.95 9.00/17.90 5.10/11.46 3.86/10.28 4.39/11.79 7.77/13.34 4.03/10.38 7.85/14.23 9.26/15.62 8.13/14.52 3.49/10.02 4.54/10.96 6.52/12.84 5.03/12.67 8.76/15.11 9.47/18.49 7.15/16.10 3.14/9.39 7.29/13.45 5.03/11.40 9.46/15.76 7.39/14.09 9.06/18.56 6.11/12.42 6.01/12.40

> Agent run on 2026-10-04, with the Mac locked and its display asleep, so it ran windowless (--offscreen). Indicative only: the maintainer's run with the window on screen is the real measurement.
