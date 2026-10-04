# Scenario starting states spell out everything

A Scenario's starting state lists, every time and with no hidden defaults, every item that affects the simulation:

- the Quad and the Map, by name
- position, attitude, speed and rotation
- whether the Quad is armed
- the battery
- the Flight Mode
- every Rates field
- every Assist, on or off
- the Radio Link rate, the physics rate and the random seed

State nobody can write by hand gets one word each:

- **motors `"settled"`:** spinning at the speed that holds the stated motion.
- **flight_controller `"fresh"`:** as right after Reset powers it up, with filters starting from the current sensor readings.

The maintainer chose this over written-down defaults so that a reviewer who doesn't read code sees, in the file itself, everything a Scenario depends on. Longer files are the accepted cost. Decided in [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11).

## Consequences

- Don't "simplify" Scenarios by adding defaults.
- When a new starting-state item appears, such as several Quads per Scenario or a new Assist, the PR that adds it runs a tool that writes it into every Scenario, with the value that keeps today's behaviour. The same PR bumps the format version number at the top of each file.
- The Quad and the Map are named, not copied. Changing a Quad definition moves every Scenario that uses it, and the Results show it.
- World values such as wind and air density come from the Map, not from the starting state.
