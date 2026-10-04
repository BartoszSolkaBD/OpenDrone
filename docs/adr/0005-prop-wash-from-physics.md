# Prop Wash comes from physics, not an artificial effect or a slider

Commercial sims fake Prop Wash. Liftoff adds an artificial shake (42 % by default) because its own Flight Controller had cancelled the real effect, and both Liftoff and Velocidrone give pilots a strength slider. We decided that Prop Wash comes only from the physics plus the Flight Controller's real delays. Each rotor that descends into its own air loses some thrust and its thrust flickers, differently on each rotor, and forward speed escapes it. How strong it is and how fast it flickers are Quad parameters, tuned in Feel Tests. No pilot setting changes them, because under our Assist rule a slider would be an arcade knob. Settled in [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10).

## Considered options

- **An artificial shake, Liftoff-style:** rejected, because it breaks "physics first".
- **A prop wash prototype before the spec is final:** rejected, because it needs physics, the Flight Controller, input and a camera all working. The build's first Feel Test does the same job.

## Consequences

- The Flight Controller must keep Betaflight's real filter and motor-lag delays. An idealised one would erase Prop Wash, which is what happened to Liftoff.
- The flicker comes from a seeded random generator kept in the simulation state, so Scenarios and replays repeat exactly (ADR-0001, bit-exact determinism).
- The simulated gyro is clean in the alpha, with no motor vibration. Prop Wash shake still reaches it, because the Quad really moves.
