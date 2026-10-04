# Verification

How we prove the simulation behaves correctly, and keeps behaving correctly. Back to the [map](../../CONTEXT.md).

## Language

**Scenario**:
A repeatable test case: a starting state, a scripted sequence of inputs, and the expected resulting simulation state, such as position, attitude or battery charge.
_Avoid_: E2E test, replay test

**Test Quad**:
A Quad definition made only for Scenarios, such as one with a single effect's numbers set to zero so that effect can be tested alone. Pilots never fly one.
_Avoid_: Mock Quad, debug switch

**Feel Test**:
A session in which a pilot who flies the real Quad flies the simulated one against their memory of it, following a written checklist of manoeuvres. Whatever the pilot signs off is pinned in Scenarios.
_Avoid_: Playtest, vibe check

## Rules

- A Scenario is readable by a non-programmer. Its expectations are written as plain values with tolerances, for example battery voltage ≈ 3.71 ± 0.02 at t = 30 s.
- A change to simulation behaviour shows up as changed Scenario expectations. The change is reviewed by reading which expectations moved and which didn't.
- A Scenario's starting state names the active Rates and every active Assist that acts on inputs or inside the Flight Controller, because both are part of the simulation.
- To test one effect alone, a Scenario uses a Test Quad. The physics has no hidden switches that turn effects off.
