# Simulation

The world every Quad flies in, and the one way into it. Back to the [map](../../CONTEXT.md).

## Language

**Simulation**:
The world that moves every Quad, its Flight Controller and the Map's collisions forward in fixed steps. The same Flight Inputs give the same results on every computer.
_Avoid_: Game loop, engine, physics engine

**Simulation Time**:
Time as the Simulation counts it: whole steps, never the computer's clock. Times in a Scenario are Simulation Time.
_Avoid_: Game time, frame time, real time

**Flight Input**:
Anything that enters the Simulation and can change a flight: Channels, an Input Device being lost or coming back, and Reset. Each one is stamped with Simulation Time.
_Avoid_: Front door (informal only), event, command

## Rules

- Only Flight Inputs change a flight. They are recorded and replayed exactly as they arrived.
- Flight Inputs enter before any Assist, the Rates or the Radio Link. All of those run inside the Simulation, so a recording captures what the pilot's hands did.
- The Simulation never sees the computer's clock and never opens a file.
- Camera Tilt, FOV, Pause and anything purely visual never enter the Simulation.
- An Input Device counts as lost when the computer reports it removed. A device that only reports changes is never counted as lost just because it goes quiet. Values that arrive in the same instant as the removal are thrown away.
- World values, such as wind, come only from the Map.
- Game modes and scripts change a flight only through Flight Inputs, settings, Assists and the choice of Map. They never change physics rules.
- The Simulation can hold several Quads, stepped in a fixed order. The alpha flies one.
