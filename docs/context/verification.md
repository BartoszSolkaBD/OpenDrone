# Verification

How we prove the simulation behaves correctly, and keeps behaving correctly. Back to the [map](../../CONTEXT.md).

## Language

**Scenario**:
A repeatable test case: a starting state, a scripted sequence of inputs, and the expected resulting simulation state, such as position, attitude or battery charge.
_Avoid_: E2E test, replay test

## Rules

- A Scenario is readable by a non-programmer. Its expectations are written as plain values with tolerances, for example battery voltage ≈ 3.71 ± 0.02 at t = 30 s.
- A change to simulation behaviour shows up as changed Scenario expectations. The change is reviewed by reading which expectations moved and which didn't.
- A Scenario's starting state names the active Rates and every active Assist that acts on inputs or inside the Flight Controller, because both are part of the simulation.
