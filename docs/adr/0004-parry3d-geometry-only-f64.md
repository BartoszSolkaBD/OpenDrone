# Collisions use parry3d as a geometry library only, and the core counts in 64-bit

Collision against the Map lives inside `opendrone-physics`, where the same-results house rules apply. It uses parry3d's 64-bit edition (`parry3d-f64`) in its `enhanced-determinism` mode, and only to answer geometry questions:
- what the Quad hits when it moves from here to there, and where
- how far the nearest surface is above or below each rotor

We write the contact response ourselves: bounce and slide, plus Prop Strikes, in which a spinning prop disc rubs against what it touches and brakes its motor. That replaces the 5" Quad's solid prop discs ([ADR-0012](0012-crashes-behave-like-a-real-quad.md)). We keep our own flight integrator. Every core crate counts in 64-bit floats (f64) through `opendrone-maths`, and the game converts to 32-bit only for drawing. Settled in [#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12).

## Considered options

- **Rapier, a full physics engine built on parry3d.** It offers a ready-made contact solver, sweeps for fast-moving objects, and would help with Drift Event cars later. Rejected for now:
  - Rapier's integrator would replace ours, and the flight dynamics research recommends a specific attitude update.
  - The Quad's state would live inside Rapier's world, so rollback snapshots would have to go through Rapier.

  Rapier stays possible later, behind the same collision interface inside `opendrone-physics`.
- **All collision code written ourselves.** It would avoid sharing glam with Bevy (below), but it's the most work: a spatial index and triangle-mesh sweeps. It gains nothing over parry3d.
- **32-bit numbers in the core.** Rejected on arithmetic:
  - 100 m from a Map's origin, f32 can only place the Quad in steps of about 7.6 µm.
  - At 8 kHz, a slow 0.1 m/s drift moves the Quad 12.5 µm per step, less than two of those steps.
  - Whoop values are also tiny: inertia is about 1e-5 kg·m² and the thrust coefficient about 2e-8.
  - f64 costs almost nothing, because flight is about 1 µs per step. Our Flight Controller rounds slightly differently from Betaflight's 32-bit maths, far below anything a pilot can feel.

## Consequences

- **Bevy and parry3d share one copy of glam from Bevy 0.20 on.** Checked on 2026-10-04:
  - parry3d 0.31.1 and Rapier 0.36 depend on glam 0.33 (through glamx 0.3).
  - Bevy 0.19.1 uses glam 0.32, so the two don't touch today. Bevy 0.20.0-rc.2 uses glam 0.33.
  - Cargo merges the features of a shared library, so parry3d's determinism mode switches Bevy's own maths to glam's `scalar-math` (no parallel maths instructions) and `libm`.
  - `cargo check` of Bevy 0.20.0-rc.2 with default features, `serialize` and parry3d 0.31.1 `enhanced-determinism` succeeded, so the build failure noted in bevy_rapier's changelog doesn't occur with these versions. A running window wasn't tested.
- **The frame-time cost is unmeasured.** The 1440p frame test should run with parry3d's determinism mode on. _Update: measured in [#33](https://github.com/BartoszSolkaBD/OpenDrone/issues/33): no visible cost at 1440p, and at most about 0.07 ms of CPU per frame on the M4. The alpha starts on Bevy 0.20 and accepts the shared glam; see [ADR-0021](0021-alpha-starts-on-bevy-0-20.md)._
- **A guard in CI.** Bevy's side could change glam's settings and so change collision results in the game build only. CI checks that one reference Scenario gives the same fingerprint in the game build as in the headless runner.
- **Our own number types.** Our types in `opendrone-maths` don't use glam. The conversion to and from parry3d's types happens inside `opendrone-physics`.
- **Ray questions for the FPV camera.** The read-only ray question the game asks for the FPV camera's signal model uses the same Map collision.
