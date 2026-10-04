# Flying

What gets simulated in the air, and how pilots tune the experience. Back to the [map](../../CONTEXT.md).

## Language

**Quad**:
The simulated aircraft: its frame, motors, props, battery and the physics that govern it. A Quad is defined as data, so a new Quad is a new definition, not new physics code.
_Avoid_: Drone (in code and specs), aircraft, model

**Flight Controller**:
The simulated on-board firmware that turns stick input into motor commands through Rates, a stabilisation loop and a Flight Mode. Its reference is Betaflight.
_Avoid_: Controller (ambiguous), FC firmware

**Flight Mode**:
How the Flight Controller reads the sticks: Acro, where the sticks command rotation speed, or Angle and Horizon, which level the Quad on their own.
_Avoid_: Mode (on its own), stabilisation mode

**Arm**:
To let the Flight Controller spin the motors. As in Betaflight, the pilot arms with an Action, and only with the throttle low and the Quad roughly level.
_Avoid_: Start, enable motors

**Rates**:
The curves that turn stick movement into rotation speed and throttle, copied field for field from a Betaflight rate profile. A pilot has one set of Rates for every Quad.
_Avoid_: Sensitivity, stick curves

**Assist**:
An optional aid layered on top of faithful physics, such as the Angle Flight Mode, input smoothing or Endless Battery. An Assist never changes the underlying physics.
_Avoid_: Arcade mode, easy mode, simplified physics

**Endless Battery**:
An Assist that keeps the battery charge topped up. Voltage still sags under load, but the pack never runs flat.
_Avoid_: Infinite battery, battery off

**Preset**:
A named starting point, Beginner, Intermediate or Pro, that sets the pilot's Flying and camera settings. Changing any of those afterwards makes the selection Custom.
_Avoid_: Difficulty, profile

## Rules

- There is no arcade mode. Every Preset runs the same physics and differs only in settings and Assists.
- An Assist acts in only three places: between the Input Device and the Flight Controller, inside the Flight Controller, or on session state such as battery charge. It never touches the physics rules: forces, motors, props, air or how the battery behaves.
- The battery is always simulated. No setting turns it off.
- A Quad should feel like its real-world counterpart to a pilot who flies one. Feel is judged against real quads running Betaflight.
