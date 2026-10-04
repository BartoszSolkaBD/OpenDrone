# Flying

What gets simulated in the air, and how pilots tune the experience. Back to the [map](../../CONTEXT.md).

## Language

**Quad**:
The simulated aircraft: its frame, motors, props, battery and the physics that govern it. A Quad is defined as data, so a new Quad is a new definition, not new physics code.
_Avoid_: Drone (in code and specs), aircraft, model

**Flight Controller**:
The simulated on-board firmware that turns stick input into motor commands through rates, a stabilisation loop and a flight mode. Its reference is Betaflight.
_Avoid_: Controller (ambiguous), FC firmware

**Assist**:
An optional aid layered on top of faithful physics, for example a self-levelling flight mode or reduced input noise. An Assist never changes the underlying physics.
_Avoid_: Arcade mode, easy mode, simplified physics

**Preset**:
A named bundle of settings and Assists, such as amateur, semi-pro or pro, that a pilot can pick as a starting point.
_Avoid_: Difficulty, profile

## Rules

- There is no arcade mode. Every Preset runs the same physics and differs only in settings and Assists.
- A Quad should feel like its real-world counterpart to a pilot who flies one. Feel is judged against real quads running Betaflight.
