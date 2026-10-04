# Verification

How we prove the simulation behaves correctly, and keeps behaving correctly. Back to the [map](../../CONTEXT.md).

## Language

**Scenario**:
A repeatable test case: a starting state, inputs from a Timeline or an Input Track, and Expectations about how the simulation behaves. Every Scenario is one of four kinds: Flight, Thrust Stand, Flight Controller or Physics.
_Avoid_: E2E test, replay test

**Flight Scenario**:
A Scenario in which the Flight Controller and the physics fly the Quad together.
_Avoid_: rig, setup (pilots use both words for their gear)

**Thrust Stand Scenario**:
A Scenario in which the Quad is held still and its motors are scripted, to check motors and props against makers' thrust sheets.

**Flight Controller Scenario**:
A Scenario that runs the Flight Controller alone, fed recorded stick and sensor readings or a table of cases.

**Physics Scenario**:
A Scenario that runs the physics alone, fed scripted motor commands, for example from a real flight log.

**Expectation**:
One check in a Scenario: what is measured, the expected value with its tolerance, and its Basis.
_Avoid_: assertion

**Basis**:
Where an Expectation's number comes from. **Source** means a cited outside reference. **Rule** means it was worked out from physics, with the working shown. **Observed** means it's what the sim did when the Expectation was written.
_Avoid_: provenance, origin

**Results**:
The file the Scenario runner writes next to each Scenario, holding the measured value of every Expectation.
_Avoid_: snapshot (a whole-state save is something else), golden file

**Timeline**:
The hand-written inputs inside a Scenario file: stick positions, switches and events at given times.
_Avoid_: script

**Input Track**:
A separate file of recorded inputs that a Scenario plays back, such as a flight saved in the sim or a Betaflight log.
_Avoid_: recording, replay

**Test Pilot**:
A helper, used only in Scenarios, that moves the sticks in reaction to the Quad. It has named jobs such as hold height, hold level and hold heading.
_Avoid_: autopilot, altitude hold

**Test Map**:
A simple Map built into the code for Scenarios only, such as empty air, a flat floor or a wall.
_Avoid_: test level

## Rules

- A Scenario is readable by a non-programmer. Every number carries its unit, sticks are written in percent, and Expectations are plain values with tolerances, for example battery voltage 3.71 V ± 0.02 V at 30 s.
- The whole simulation is bit-exact on every platform. The same starting state and the same inputs give exactly the same flight, to the last bit, on every OS ([ADR-0001](../adr/0001-bit-exact-determinism-with-ordinary-floats.md)). Picture, camera, sound and analog noise are outside it.
- A Scenario's starting state spells out everything, every time, with no hidden defaults ([ADR-0002](../adr/0002-scenario-starting-state-spells-out-everything.md)). That includes the active Rates and every Assist, because both are part of the simulation. State nobody can write by hand is named in one word: "settled" motors and a "fresh" Flight Controller.
- A Scenario names its Quad and its Map but never copies their numbers. So changing a Quad definition moves every Scenario that uses it.
- World values such as wind come from the Map. A Scenario that needs wind uses a Test Map that has wind.
- Every Expectation has a Basis. Source and Rule Expectations are locked: if the sim disagrees, the sim is fixed. Observed Expectations may be updated to match the sim, each with a one-line reason.
- A change to simulation behaviour shows up in the Results, which show every measured value that moved, even inside its tolerance. The change is reviewed by reading which values moved and which didn't.
- The Test Pilot moves only the sticks, as a human would, and never touches the physics. Pilots never get it.
- Rust and maths-library updates count as behaviour changes, and each one gets its own PR.
