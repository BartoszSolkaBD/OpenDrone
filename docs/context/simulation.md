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
- An Input Device counts as lost when the computer reports it removed. A device that keeps reporting at rest also counts as lost after 1 s without a report, as the Radio Link counts it from the Flight Inputs' stamps, so a recording replays it exactly. A device that only reports changes is never counted as lost just because it goes quiet. Values that arrive in the same instant as the removal are thrown away.
- While the Flying Input Device is lost, the Radio Link sends no frames, and the Flight Controller's Failsafe follows. When it is back, the next frame on the link's own beat carries the newest Channels.
- The Flying Input Device's facts (its Report Rate, and whether it reports at rest) come with the set-up. Scripted sticks, as in a Scenario's Timeline, have no device, and get plain regular frames on the Radio Link's own clock.
- Reset puts the Quad on the Launch Spot, landed, still and disarmed, and powers it up fresh: a full battery, a fresh Flight Controller, and ESCs starting up. The Radio Link belongs to the pilot's radio, so it keeps its beat.
- World values, such as wind, come only from the Map.
- Game modes and scripts change a flight only through Flight Inputs, settings, Assists and the choice of Map. They never change physics rules.
- The Simulation can hold several Quads, stepped in a fixed order. The alpha flies one.
- After each tick, the Simulation hands out every Quad's state and its contacts with the Map: which part of the Quad touched which part of the Map, where, which way the Map pushed and how hard, how hard friction held or dragged it, and, on a prop's disc, how hard the prop rubbed. It also hands out what the gyro reads (the true rotation, clipped at the board's range) and each ESC's state (starting up, ready, start wait, starting, running, restart wait, restarting, or stopped after failed restarts) with how many times it has restarted its motor.
- Its only randomness is Prop Wash's flicker, drawn from the random seed the caller gives it, each Quad's from its own seed taken from that one in turn. The generator is part of the Simulation's state: it is fingerprinted and copied with it, and goes on through Reset. The same seed gives the same flights; another seed gives other flicker.
- After each tick, the Simulation hands out every Quad's state and its contacts with the Map: which part of the Quad touched which part of the Map, where, which way the Map pushed and how hard, and how hard friction held or dragged it.
- The Simulation answers one read-only question about the Map, the line question: which surfaces does a straight line pass through, and where does it go into and come out of each? The Video Signal and the Where-you-stand sound ask it between ticks. Asking never changes the Simulation.
