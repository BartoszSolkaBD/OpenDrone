# Crashes behave like a real quad: Prop Strikes brake motors, nothing breaks, nothing disarms or resets on its own

A crash in OpenDrone goes the way it goes on a real quad running Betaflight's defaults.

- **A Prop Strike rubs.** Each prop is a thin disc in the Quad's collision shape. While it touches the Map, it rubs: the harder it's pressed, the harder its motor is braked, and the same rub pushes and twists the Quad. A hard hit stops the motor at once, and a graze only slows it.
- **The ESC restarts a stopped motor as Bluejay does.** It waits 0.1 s, then restarts with limited start-up power. After 3 failed tries the motor stays off until the pilot disarms.
- **Nothing breaks or wears out in the alpha.**
- **A crash never disarms the Quad or resets it on its own.** The pilot disarms, Failsafe drops the Quad, or the pilot presses Reset.
- **Betaflight 2026.6's Crash Flip is in.**

Every commercial sim we checked does it differently. Liftoff, Uncrashed and TRYP break props. Liftoff and VelociDrone reset the quad by themselves after some crashes. Liftoff turns turtle mode into a single key press. We chose to copy the real quad, because physics comes first and easier play comes only from Assists. Decided in [#26](https://github.com/BartoszSolkaBD/OpenDrone/issues/26).

## Considered options

- **Props as solid discs,** the stand-in from [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10) and [ADR-0004](0004-parry3d-geometry-only-f64.md). A 5" that clips a rail just bounces off, and an upside-down armed 5" spins its props through the ground. Rejected.
- **A speed threshold:** a touch faster than a set speed stops the motor, and the kick is a fixed jolt. It's easier to tune, but a graze and a hard hit feel the same, and the kick ignores which way the prop spins. Rejected.
- **Lasting prop damage now,** as in Liftoff, Uncrashed and TRYP. Our gyro has no vibration in the alpha, so a chipped prop would only feel weaker. The shaking that pilots know from a chipped prop wouldn't be there. Deferred until gyro vibration noise arrives.
- **Disarm on crash.** That could be Betaflight's `crash_recovery = DISARM`, its `landing_disarm_threshold`, or a sim-only Assist like VelociDrone's auto-arming reset. All of Betaflight's are off by default and on both alpha Tunes. Deferred. A Tune that sets one imports with a "not simulated yet" note.
  - _Update: one of Betaflight's is on by default, and on both alpha Tunes: runaway takeoff prevention, which disarms a Quad whose PID sum stays high with the gyro moving, from arming until the flight counts as stable, such as one pinned against a wall right after arming. It is simulated from [#54](https://github.com/BartoszSolkaBD/OpenDrone/issues/54), as the maintainer decided in [#126](https://github.com/BartoszSolkaBD/OpenDrone/issues/126). The two off by default stay deferred._
- **Automatic reset after a crash.** It takes the decision away from the pilot. Not planned.

## Consequences

- **New numbers in each Quad file:**
  - prop grip, an Estimate with a range
  - the pack as its own collision box
  - reverse thrust and reverse torque, for Crash Flip
  - the ESC's start wait, restart tries and start-up power limit
  - a gyro limit of ±2000 °/s
- **The wall between `sim` and `physics`** in [ADR-0003](0003-crate-split-and-flight-inputs.md): motor commands now carry each motor's spin direction, because Crash Flip reverses the motors.
- **The gyro reads the true rotation up to ±2000 °/s,** like the real sensors. Only then can collisions trigger Betaflight's yaw spin recovery, which comes from the Tune.
- **When lasting damage arrives,** a prop's condition is session state, like battery charge. An Assist could then keep the props repaired without touching the physics rules.
- **Later, none of it blocked by this decision:** lasting damage of every kind (props, motors, frames, a camera knocked down, a pack thrown out), Prop Strikes against another Quad, Pick Up (the walk of shame), rewind and a saved reset point.
