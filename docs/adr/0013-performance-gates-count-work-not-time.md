# Performance gates count work, not time

CI guards performance by counting work, not by timing it. On every PR, GitHub's Linux machine builds main and the PR in the same job. It flies the same recorded flights through both and compares their Work Counts:

- **Physics:**
  - instructions run, counted with Gungraun (Valgrind-based)
  - for one second of whoop flight inside Bando and one second of 5" flight through the Skate Park bowls, each 8,000 Simulation steps with collisions and parry3d's determinism mode on
  - and, to locate a slowdown, for the physics alone and the Flight Controller alone
- **Rendering:** draw calls, triangles, render passes and the pixels they cover at 1440p, shader pipelines and GPU memory, per frame on the same flights. It runs with nothing on screen, on Mesa's software GPU.

A Work Count more than 2% above main fails the gate, unless the PR names the change and a reason the Reviewer accepts. A tidy-up is never a reason. The maintainer asked for a gate that rejects PRs that lower performance blindly, and counts are the only CI measure steady enough for a 2% line. Decided in [#15](https://github.com/BartoszSolkaBD/OpenDrone/issues/15).

## Considered options

- **Time the benchmarks on GitHub's machines** with criterion and github-action-benchmark. Rejected:
  - Shared runners vary about 2.7% between runs (CodSpeed, 2025) and can only detect changes of about 4–5% (arXiv 2411.05491).
  - github-action-benchmark's own README reports swings of 10–20%. Only big slowdowns could be flagged.
- **A CI machine on the maintainer's Mac mini M4.** It would give real target-machine times. Rejected:
  - It's noisy while the maintainer uses the Mac.
  - GitHub warns that self-hosted runners "should almost never be used for public repositories".
- **CodSpeed or Bencher as a hosted service.** Not needed. Counting and comparing in our own CI gives the same signal with no outside account.
- **The Frame Check as a CI gate,** with agents pasting their M4 result into the PR. Rejected by the maintainer as not a good CI benchmark: the number can't be checked, and it depends on the machine and its load.

## Consequences

- **Counts can't be turned into milliseconds on the M4.**
  - They catch structural regressions: more draw calls, an extra full-screen pass, unshared materials, more shader pipelines. The last one predicts the first-launch compile stall: about 52 Metal pipelines and several seconds in the camera prototype (#28).
  - They miss a costlier shader with the same counts, overdraw, GPU cache effects, and driver or Bevy changes.
- **Real time stays on the M4.**
  - `cargo xtask bench` times the physics there, and agents paste the figures beside any slowdown they explain.
  - The Frame Check measures the 45 fps promise: an average, with the slowest 1% reported but not promised. The maintainer runs it before each release, and no release is tagged below the promise.
- **Small steps can't add up unseen.** The Review Report shows each Work Count's total change since the last release.
- **The render counts aren't proven yet.** Whether Bevy renders on GitHub's Linux machine with a software GPU, and which counts it can report, is a task ticket. The physics counts don't depend on it. _Update: proven in [#29](https://github.com/BartoszSolkaBD/OpenDrone/issues/29); see [ADR-0016](0016-render-work-counts-from-a-counting-wgpu-on-one-lavapipe-thread.md)._
- **Faster results need nothing.** Main's numbers simply move.
