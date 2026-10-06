# Bit-exact determinism on every platform, with ordinary floats

The whole simulation gives bit-identical results on every platform we ship: macOS on ARM, Windows and Linux on x86, and later iOS and Android. "The whole simulation" means Quad physics, the Flight Controller, Map collisions, the Radio Link, input Assists, the battery, the Test Pilot and seeded randomness. We get this with ordinary hardware floats plus house rules, not with software floats or fixed-point maths:

- `libm` for every maths function
- our own non-SIMD maths types
- a fixed tick and a fixed order of work
- no `HashMap` iteration where order matters
- seeded randomness
- a pinned toolchain

Tolerance-only checking would have been cheaper today. But it rules out exact replays of saved flights, long Scenarios that include crashes, and rollback or lockstep multiplayer with cross-play. The [determinism research](../research/determinism.md) found the house rules cost little at runtime. Decided in [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11).

## Considered options

- **Tolerance only:** results drift by OS and even by CPU model. Scenarios would have to stay short, avoid touching anything, or use loose tolerances.
- **Bit-exact for flight but not collisions:** a grazing touch of a wall can hit on one machine and miss on another, which effectively rules out rollback multiplayer.
- **Same machine only:** it needs nearly the same rules, so it saves nothing.
- **Software floats or fixed-point:** roughly 10 to 100 times slower, with no extra guarantee on our platforms. Some netcode guides recommend fixed-point, but that advice assumes nobody controls the maths functions and SIMD.

## Consequences

- Rust and maths-library updates count as behaviour changes. Each one gets its own PR.
- CI runs a repeat check and a cross-OS agreement check on Scenario fingerprints, and both block merges.
- Picture, camera, sound, analog noise and the SITL Betaflight bridge are outside the guarantee.
