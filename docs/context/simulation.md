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

**Snapshot**:
A copy of the Simulation's whole state, taken between two ticks and put back as often as wanted. It is what a rollback or a lockstep game steps back to. It is a copy of the state only, not of the Map.
_Avoid_: Save state, checkpoint (a Scenario's checkpoints are fingerprints, not copies)

**Fingerprint**:
A short code for the Simulation's whole state, the same on every computer: if two fingerprints match, the two Simulations are in exactly the same state, and if one bit differs, the codes differ. The repeat check and the agreement check compare them. What it holds is listed under "What the Fingerprint holds" below.
_Avoid_: Checksum, hash (informal only)

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
- The Simulation can hold several Quads, stepped in a fixed order, the order of the set-up's list. The alpha flies one. Quads never touch each other, so each flies exactly as it would alone: the same Flight Inputs give the same state at every step, its Flight Controller's memory included. Each Quad's Prop Wash flicker comes from its own seed, taken in turn from the Simulation's, so a Quad alone gets the same flicker as it has in company only when it is given the seed that Quad is given there.
- A Snapshot holds the Simulation's whole state: Simulation Time, every Quad, the random generators and the Flight Inputs that have not arrived yet (see the Fingerprint below for the full list). Restoring it puts the Simulation exactly where it was, and the same Flight Inputs from there give a bit-identical Fingerprint at every step. Flight Inputs given after the Snapshot was taken are forgotten by the restore, and ones that were already waiting wait again, so a caller that replays its Flight Inputs gives only those from after the Snapshot. A Snapshot fits the Simulation it came from, and any other built from the same set-up (the same Map above all, which is not in it): it brings its own seed, Quads' numbers and world values with it.
- After each tick, the Simulation hands out every Quad's state and its contacts with the Map: which part of the Quad touched which part of the Map, where, which way the Map pushed and how hard, how hard friction held or dragged it, and, on a prop's disc, how hard the prop rubbed. It also hands out what the gyro reads (the true rotation, clipped at the board's range) and each ESC's state (starting up, ready, start wait, starting, running, restart wait, restarting, or stopped after failed restarts) with how many times it has restarted its motor.
- Its only randomness is Prop Wash's flicker, drawn from the random seed the caller gives it, each Quad's from its own seed taken from that one in turn. The generator is part of the Simulation's state: it is fingerprinted and copied with it, and goes on through Reset. The same seed gives the same flights; another seed gives other flicker.
- The Simulation answers one read-only question about the Map, the line question: which surfaces does a straight line pass through, and where does it go into and come out of each? The Video Signal and the Where-you-stand sound ask it between ticks. Asking never changes the Simulation.

## What the Fingerprint holds

The Fingerprint covers the state, in this fixed order. Two Simulations that match are in the same state: they give the same flights from here on, for the same Flight Inputs.

- **The Simulation:** Simulation Time (whole ticks), the physics rate, the world values (gravity and air density), the random seed and the number of Quads.
- **Then each Quad, in the set-up's order,** with its own Fingerprint, which anyone can ask for alone:
  - **Its body:** position, velocity, attitude and rotation; each motor's speed, drive, current and power; each ESC's whole state (starting up, ready, start wait, starting, running, restart wait, restarting or stopped, with its timers and restart count); the battery's charge used, starting charge, slow sag, voltage and current; and each rotor's Prop Wash flicker with the place its random generator has reached.
  - **The motor commands** the Flight Controller gave on the last tick.
  - **Its Radio Link:** the Packet Rate, whether the device reports at rest, the clock (its own, or the beat learnt from the device), the newest Channels with the tick they arrived at, how many reports have come since the last frame, when the device last reported, and whether it counts as unplugged or silent.
  - **Input smoothing,** if it is on: the smoothed roll, pitch and yaw, and the newest ones the pilot's hands gave it.
  - **The Flight Controller's memory:** the Tune and the Rates, the loop count, the receiver and Failsafe state, RC smoothing, feedforward, every filter, the Dynamic Throttle and throttle shaping, arming and why it is blocked, the PID terms, yaw spin recovery, runaway takeoff prevention, the DShot command state, the battery and the beeper, the last frame's time and the last motor commands, and Auto-arm, if it is on. The scripted-motors stand-in holds how far along its Timeline it has got.
  - **Flight Inputs that have not arrived yet,** each with its moment, but only when there are any. The game and the runner give each Flight Input for the tick that is about to run, so after a tick there are none, and the Fingerprint is the same as if this line did not exist.
- **What it leaves out, because it never changes during a flight or comes from the state:**
  - the Map's solid parts, which the Results' Map fingerprint covers;
  - the Quad's own numbers, its Launch Spot and its Assist settings, which the Results' Quad fingerprint covers;
  - what the last tick reported: contacts, how hard each prop rubbed, thrust and torque, and the Flight Controller's debug record. They are worked out from the state, and every push a contact reports is already in the Quad's speeds.

A Snapshot holds all of that and the parts the Fingerprint leaves out except the Map, so a restored Simulation reports the last tick as it was.
