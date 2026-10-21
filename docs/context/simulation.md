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
A short code for the Simulation's whole state, the same on every computer: if two fingerprints match, the two Simulations are almost surely in exactly the same state, and a different state almost surely gives a different code (it is a 64-bit code, so no code can promise more). It is not the Results' Quad, Map or run fingerprints, which are other codes: those cover a Quad definition's numbers, a Map's shapes and a whole Scenario run, and the Scenario's run fingerprint is made from the Simulation's Fingerprint after every step. The repeat check and the agreement check compare them. What it holds is listed under "What the Fingerprint holds" below.
_Avoid_: Checksum, hash (informal only)

## Rules

- Only Flight Inputs change a flight. They are saved in an Input Track and play back exactly as they arrived.
- Flight Inputs enter before any Assist, the Rates or the Radio Link. All of those run inside the Simulation, so an Input Track captures what the pilot's hands did.
- The Simulation never sees the computer's clock and never opens a file.
- Camera Tilt, FOV, Pause and anything purely visual never enter the Simulation.
- An Input Device counts as lost when the computer reports it removed. A device that keeps reporting at rest also counts as lost after 1 s without a report, as the Radio Link counts it from the Flight Inputs' stamps, so the same Input Track gives it exactly. A device that only reports changes is never counted as lost just because it goes quiet. Values that arrive in the same instant as the removal are thrown away.
- While the Flying Input Device is lost, the Radio Link sends no frames, and the Flight Controller's Failsafe follows. When it is back, the next frame on the link's own beat carries the newest Channels.
- The Flying Input Device's facts (its Report Rate, and whether it reports at rest) come with the set-up. Scripted sticks, as in a Scenario's Timeline, have no device, and get plain regular frames on the Radio Link's own clock.
- Reset puts the Quad on the Launch Spot, landed, still and disarmed, and powers it up fresh: a full battery, a fresh Flight Controller, and ESCs starting up. The Radio Link belongs to the pilot's radio, so it keeps its beat.
- World values, such as wind, come only from the Map.
- Game modes and scripts change a flight only through Flight Inputs, settings, Assists and the choice of Map. They never change physics rules.
- The Simulation can hold several Quads, stepped in a fixed order, the order of the set-up's list. The alpha flies one. Quads never touch each other, so each flies exactly as it would alone: the same Flight Inputs give the same state at every step, its Flight Controller's memory included. Each Quad's Prop Wash flicker comes from its own seed, taken in turn from the Simulation's, so a Quad flying alone has the flicker it has in company only when the lone Simulation's seed is the pair's seed moved on, for the second Quad, by one step of the generator (SplitMix64: the third Quad's by two, and so on). The first Quad needs the same seed as in the pair.
- A Snapshot holds the Simulation's whole state: Simulation Time, every Quad, the random generators and the Flight Inputs that have not arrived yet (what the Fingerprint holds is listed below; the Snapshot holds that and more). Restoring it puts the Simulation exactly where it was, and the same Flight Inputs from there give a bit-identical Fingerprint at every step. Flight Inputs given after the Snapshot was taken are forgotten by the restore, and ones that were already waiting wait again, so a caller that gives its Flight Inputs again from the Snapshot gives only those from after it. A Snapshot fits the Simulation it came from, and any other built from the same set-up (the same Map above all, which is not in it): it brings its own seed, Quads' numbers and world values with it.
- After each tick, the Simulation hands out every Quad's state and its contacts with the Map: which part of the Quad touched which part of the Map, where, which way the Map pushed and how hard, how hard friction held or dragged it, and, on a prop's disc, how hard the prop rubbed. It also hands out what the gyro reads (the true rotation, clipped at the board's range) and each ESC's state (starting up, ready, start wait, starting, running, restart wait, restarting, or stopped after failed restarts) with how many times it has restarted its motor.
- Its only randomness is Prop Wash's flicker, drawn from the random seed the caller gives it, each Quad's from its own seed taken from that one in turn. Each rotor of each Quad has its own generator, and they are part of the Simulation's state: they are fingerprinted and held in its Snapshot, and go on through Reset. The same seed gives the same flights; another seed gives other flicker.
- The Simulation answers one read-only question about the Map, the line question: which surfaces does a straight line pass through, and where does it go into and come out of each? The Video Signal and the Where-you-stand sound ask it between ticks. Asking never changes the Simulation.

## What the Fingerprint holds

The Fingerprint covers the state, in this fixed order. Two Simulations that match are, apart from what is left out below, in the same state: they give the same flights from here on, for the same Flight Inputs.

- **The Simulation:** Simulation Time (whole ticks), the physics rate, the world values (gravity and air density), the random seed and the number of Quads.
- **Then each Quad, in the set-up's order,** with its own Fingerprint, which anyone can ask for alone:
  - **Its body:** position, velocity, attitude and rotation; each motor's speed, drive, current and power; each ESC's whole state (starting up, ready, start wait, starting, running, restart wait, restarting or stopped, with its timers and counters, and how many times it has restarted its motor); the battery's charge used, starting charge, slow sag, voltage and current; and each rotor's Prop Wash flicker: where its random generator has got to, and the glide it is on.
  - **The motor commands** the Flight Controller gave on the last tick.
  - **Its Radio Link:** the Packet Rate, whether the device reports at rest, the clock (its own, or the beat learnt from the device), the newest Channels with the tick they arrived at, how many reports have come since the last frame, on the beat which report the newest Channels came with, when the device last reported, and whether it counts as unplugged or silent.
  - **Input smoothing:** whether it is on, and when it is, its state: the smoothed roll, pitch and yaw (once Channels have arrived), and the newest ones the pilot's hands gave it.
  - **The Flight Controller's memory,** among other things. The exact list is `OurFlightController::write_fingerprint` and the functions it calls, in `crates/flight-controller`; this follows its order and names the main parts: the Tune and the Rates; the loop rate and the loop count; the receiver and Failsafe state; the stick command and the setpoint; the Packet Rate it has detected; RC smoothing; feedforward; the gyro filters; the Dynamic Throttle (when its filters last moved, and to which throttle) and throttle shaping; arming and why it is blocked; whether the throttle has been raised, whether it is stabilising and whether the I term is to be reset; the Flight Mode (Acro, Angle or Horizon) and the levelling state (its target angles, the yaw setpoint and its filters); the PID memory (the I terms; the D-term and yaw filters; whether its filters have settled; the last gyro; the I-term relax and D Max filters, whether D Max has settled on each axis, and its factor; the setpoint; and the last P, I, D and F terms and their sum for each axis); yaw spin recovery; runaway takeoff prevention; the tilt at start; the DShot command state; the battery and the beeper; the time of the last frame; and the last motor commands. Then whether Auto-arm is on and, when it is, where the Assist has got to (waiting for the throttle to go low, counting frames to arm, or armed). The scripted-motors stand-in holds how far along its Timeline it has got.
- **What it leaves out, because it never changes during a flight or comes from the state:**
  - Flight Inputs that have not arrived yet: when the caller hands one over is not part of the flight, only the tick it arrives at is, and the game hands them over between ticks where the runner does it just before one. Two Simulations with the same Fingerprint are in the same state, but can have been told different things about the future;
  - the Map's solid parts, which the Results' Map fingerprint covers;
  - the Quad's own numbers, which the Results' Quad fingerprint covers;
  - the Launch Spot and whether Endless Battery is on, which come with the set-up and are fingerprinted nowhere. Their effects still show in the state: where Reset puts the Quad and the topped-up battery. (Input smoothing and Auto-arm are not left out: the Fingerprint says whether each is on, and holds its state when it is.);
  - what the last tick reported: contacts, how hard each prop rubbed, thrust and torque, what the Radio Link did on it, and the Flight Controller's debug record (though the PID's last terms, which it shows, are in the PID memory above), with whether the Flight Controller has run a loop yet. They are worked out from the state, or, like the last Channels the Radio Link sent, only kept to report; they never change what the Quad does next. Every push a contact reports is already in the Quad's speeds.

A Snapshot holds all of that and the parts the Fingerprint leaves out except the Map, so a restored Simulation reports the last tick as it was.
