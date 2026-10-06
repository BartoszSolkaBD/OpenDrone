# The Quad's sound is made live from the simulated motors, on Firewheel in its own crate

The Quad's sound is never played from recordings. It's made live, on the sound's own thread, from:

- each motor's simulated speed and load
- the Quad definition's blade count, pole count and prop size
- a sound block set by ear

Four voices running at the motors' own speeds let the pilot hear the throttle, the Flight Controller's corrections and Prop Wash, and a new Quad sounds right from its own data.

The sound runs on Firewheel. We use it directly, not through a Bevy add-on, in a new edge crate, `opendrone-sound`, which doesn't use Bevy. The maintainer chose live sound, tuned by ear in a prototype. Firewheel was chosen because it never holds up a Bevy upgrade, and it's the engine Bevy plans to adopt. Decided in [#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32).

## Considered options

- **Recordings pitched up and down with motor speed,** as car games do. Rejected.
  - No CC0 recordings exist of a 65 mm whoop or a 5″ at several speeds. Freesound's CC0 section has one indoor Tiny Hawk 2 flight and a few toy-quad clips.
  - Sonniss, BBC and Pixabay sounds aren't CC0, so they can't enter the repo ([ADR-0014](0014-licences-for-libraries-and-assets.md)).
  - Looped recordings are the top complaint in other sims' reviews ("loopy", "a mosquito").
- **Live sound plus a recorded texture layer.** Not needed unless the prototype shows otherwise.
- **`bevy_seedling`** (Firewheel inside Bevy). Rejected.
  - It has no release for Bevy 0.20 yet. It has lagged Bevy by 2–7 weeks before.
  - Every later Bevy upgrade would wait for it ([ADR-0021](0021-alpha-starts-on-bevy-0-20.md)).
  - By default it crashes the game when no sound device opens.
- **Bevy's built-in audio.** Rejected. Live changes are limited to volume and speed, there are no filters, and its delay is fixed at about 43 ms.
- **`bevy_kira_audio`.** Rejected. It only plays recordings.
- **kira, used directly.** Mature, with a test backend, but its makers aim it mainly at desktop, and mobile is on the roadmap.

## Consequences

- **What sound reads.** Each tick's state: each motor's speed, thrust and current, each ESC's state, contacts (including how hard each prop rubs), the speeds, and the Flight Controller's beeper ([ADR-0022](0022-osd-worked-out-beside-the-simulation.md)).
- **Outside the Simulation.** Sound never enters the Simulation and sits outside the bit-exact guarantee ([ADR-0001](0001-bit-exact-determinism-with-ordinary-floats.md)). [ADR-0003](0003-crate-split-and-flight-inputs.md)'s crate list grows to twelve.
- **Cost.** About 1% of one M4 core for four motors, measured in a small benchmark. It runs on the sound's own thread, so it doesn't touch the frame or the physics.
- **Firewheel is pinned.** Its API still changes often, so we upgrade it on purpose. It borrows Bevy's small `bevy_platform` crate, which doesn't have to match the game's Bevy version.
- **Linux.** Every Rust sound library there loads the system's `alsa-lib`, which is LGPL. [ADR-0014](0014-licences-for-libraries-and-assets.md) now allows it. Linux builds need `libasound2-dev`.
- **No sound device.** The game must start and run without one. CI's machines have none, so every CI run checks this. No other CI check listens to sound; it's judged by ear.
- **New data.** Quad definitions gain a buzzer flag, an ESC start-up melody and a sound block, with no Confidence. Maps gain a Background Sound.
- **Mobile.** Firewheel has iOS and Android backends. On iOS the app must set its own audio session, or the silent switch mutes it.
