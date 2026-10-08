# Flying

What gets simulated in the air, and how pilots tune the experience. Back to the [map](../../CONTEXT.md).

## Language

**Quad**:
The simulated aircraft: its frame, motors, props, battery and the physics that govern it. A Quad is defined as data, so a new Quad is a new definition, not new physics code.
_Avoid_: Drone (in code and specs), aircraft, model

**Quad definition**:
The data that defines one Quad in a Pack: its physics numbers and its FPV Camera's limits, each with a Confidence, its Tune, its camera defaults and its on-screen name. It is named by a fixed id, such as `opendrone/whoop-65`, which never changes even if the on-screen name does.
_Avoid_: Quad config, quad profile, airframe file

**Flight Controller**:
The simulated on-board firmware that turns stick input into motor commands through Rates, a stabilisation loop and a Flight Mode. Its reference is Betaflight 2026.6.
_Avoid_: Controller (ambiguous), FC firmware

**Tune**:
A Quad's Flight Controller settings, such as PIDs, filters, feedforward, TPA, Angle and Horizon strength and Failsafe, stored under Betaflight's CLI names as part of the Quad's definition. Pilots can't edit it.
_Avoid_: PID profile, config, FC settings

**Flight Mode**:
How the Flight Controller reads the sticks: Acro, where the sticks command rotation speed, or Angle and Horizon, which level the Quad on their own.
_Avoid_: Mode (on its own), stabilisation mode

**Arm**:
To let the Flight Controller spin the motors. As in Betaflight, the pilot arms with a switch Channel, and only with the throttle low and the Quad tilted no further than its Tune allows.
_Avoid_: Start, enable motors

**Failsafe**:
What the Flight Controller does when the Radio Link is lost, copied from Betaflight: it holds the last Channels, then centres the sticks with the throttle low, then disarms and drops the Quad.
_Avoid_: Signal-loss mode, RX loss

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

**Prop Wash**:
The shaking a Quad suffers when it descends into its own disturbed air, for example after a chop-and-dive. It comes from the physics and the Flight Controller's real delays, never from an artificial effect or a setting.
_Avoid_: Propwash effect, prop wash strength (as a setting)

**Prop Strike**:
A spinning prop touching the Map. The prop rubs against what it touches, which brakes its motor and pushes and twists the Quad. Nothing breaks.
_Avoid_: Prop hit, prop clip

**Crash Flip**:
A Betaflight mode that spins the motors backwards so an upside-down Quad can flip itself back over. It is chosen with a switch Channel at the moment of arming.
_Avoid_: Turtle mode, flip over after crash

**Confidence**:
How well a physics number in a Quad definition is known: Measured, Manufacturer, Derived (arithmetic on Measured or Manufacturer numbers) or Estimate. Every physics number carries one, with its source, and so does each of the FPV Camera's limits: its Dynamic Range, lines and sharpness. Counts and choices, such as blade count or prop direction, don't, and neither do the Tune or the camera defaults (FOV and Camera Tilt). An Estimate also carries the range it may move within.
_Avoid_: Accuracy, certainty

## Rules

- There is no arcade mode. Every Preset runs the same physics and differs only in settings and Assists.
- An Assist acts in only three places: between the Input Device and the Flight Controller, inside the Flight Controller, or on session state such as battery charge. It never touches the physics rules: forces, motors, props, air or how the battery behaves.
- The battery is always simulated. No setting turns it off.
- A Quad should feel like its real-world counterpart to a pilot who flies one. Feel is judged against real quads running Betaflight.
- The Flight Controller copies Betaflight 2026.6. A Tune from an older Betaflight is translated into 2026.6 settings, never emulated.
- Reset, loading a Map and switching Quad all power the Quad up fresh, as a new battery does.
  - The Flight Controller is ready at once, skipping Betaflight's own power-up waits.
  - The ESCs play their start-up tones first, as Bluejay does. The motors answer only after the ready beep, about 1.7 s later.
  - Until that beep, arming is blocked, and the OSD shows Betaflight's `BOOTGRACE` reason. An Arm switch that was already on must be flipped off and on again, and Auto-arm arms on the first throttle raise after the beep ([#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32)).
- No pilot setting changes a physics effect, Prop Wash strength included. How strong each effect is comes from the Quad's definition.
- A Quad collides with the Map as a few simple shapes from its definition: its body box, its pack box where the pack really sits, a disc per prop (solid until Prop Strikes arrive) and, on a whoop, a ring round each prop. It bounces and slides with its own bounce and friction, and never passes through anything between two physics steps, a 3 cm rebar stub at 30 m/s included. A landed Quad that nothing pushes stays exactly still: when the Map holds it and less than 0.01 mm/s of speed and 0.006 °/s of turn is left after a physics step, it is held still. So a push too small to move it by that much in one step is held too: at 8 kHz, anything under 0.08 m/s² (at 1 kHz, 0.01 m/s²), which is why a Quad on a slope within about half a degree of the steepest its friction holds stays put instead of creeping off.
- Only Estimate numbers move in a Feel Test, and only inside their range. Measured, Manufacturer and Derived numbers are locked: if only a locked number would fix a feel, an effect is missing or wrong. A locked number changes only with a new source.
- A Tune spells out every setting the Flight Controller reads, each marked with where its value came from: the real quad, its Betaflight version's default, or a translation rule ([ADR-0015](../adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)).
- Which switch arms the Quad or picks the Flight Mode belongs to the pilot's Input Device profile, never to the Tune. It's the same on every Quad.
- Whether a Quad measures its current is its Tune's `current_meter` setting, as in Betaflight. `ADC` or `ESC` means the Flight Controller reads the battery current from the physics, so the OSD can show current and mAh. Any other value reads blank. The Whoop 65 has `ADC`, its board's default; the Freestyle 5″ has `ADC` set by hand, because a typical 5″ stack measures current.
- Whoop numbers come from whoop data, never from scaled-down 5" numbers.
- A crash never disarms or resets the Quad on its own. As in Betaflight, the pilot disarms, Failsafe drops the Quad, or the pilot presses Reset.
- Failsafe runs on Betaflight 2026.6's timeline, from the Tune (`failsafe_delay`, `failsafe_recovery_delay`, `rx_min_usec`, `mid_rc`), counted from the last Radio Link frame: the Quad flies on its last values, arming is blocked (`RXLOSS`) at 150 ms, the Channels take their stage 1 values at 300 ms (sticks centred, throttle low, AUX switches held), and DROP disarms at 1.5 s (`FAILSAFE`). To fly again, frames must arrive for 0.5 s before `RXLOSS` clears, and Failsafe ends 0.5 s after that, so about 1 s after the Input Device is back; then the Arm switch must go off and on again, with the throttle low. DROP is the only procedure: a Tune set to AUTO-LAND or GPS-RESCUE flies DROP and says it isn't simulated yet.
- Auto-arm drives the Arm switch for a pilot who has none, through Betaflight's own arming: it arms on the first throttle raise from low that passes the arming checks (so not before the ESCs' ready beep, nor during Failsafe), holding the throttle low for the two frames Betaflight needs, and then only Reset or a Failsafe drop disarms. After a drop, the throttle must come low and go up again.
- Nothing on a Quad breaks or wears out in the alpha. A Prop Strike only brakes the motor, and the ESC restarts it as Bluejay does.
- The gyro reads the Quad's true rotation up to the real sensor's limit of ±2000 °/s, with no vibration.
