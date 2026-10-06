# Sound

What the pilot hears while flying, and where they hear it from. Back to the [map](../../CONTEXT.md).

## Language

**Listening Position**:
Where the pilot hears the Quad from: On the Quad, as a camera's microphone on board hears it (the default), or Where you stand, at the Map's Launch Spot.
_Avoid_: Audio perspective, POV audio, line-of-sight audio

**Background Sound**:
A Map's looped recording of its surroundings, such as birds, distant traffic and wind. It plays for as long as the pilot is on that Map.
_Avoid_: Ambience, ambient sound, environment sound

## Rules

### The Quad's sound

- The Quad's sound is made live from each motor's simulated speed and load, and from its Quad definition: blade count, pole count, prop size, and a sound block of values set by ear ([ADR-0023](../adr/0023-quad-sound-made-live-on-firewheel.md)). No recording of a Quad is ever played.
- Each motor has its own voice. You hear Prop Wash and the Flight Controller's corrections as the motors warble, never as a sound of their own. Nothing is heard that the Simulation didn't do.
- A sound is never fixed by moving a physics number. If the pitch in a hover sounds wrong while the sound block is right, that's raised as a possible physics problem for a Feel Test.
- The sound block carries no Confidence, because it isn't physics.
- **The sound block describes only its Quad**:
  - the motor and prop voice
  - frame hum
  - beep levels
  - Prop Strike ticks
  - hit level
  - how loud the Quad is Where you stand

  Everything about the listener is the game's, the same for every Quad: the camera mic, wind on the mic, the compressor, and distance, air and walls. A Pack's Quad never carries listener settings.
- **A small sound-side wobble in each motor's speed** widens each motor's tone. It stands in for turbulence faster than the Simulation's tick, which the Simulation doesn't model. It's checked again once the real physics exists. It changes nothing in a flight.
- **The voice isn't final.** It's reworked and tuned by ear during the build. The starting point is the [#34 prototype](https://github.com/BartoszSolkaBD/OpenDrone/issues/34)'s second round ([research](../research/quad-sound.md)), and the maintainer's own recording of their Meteor65 Pro is the reference.

### Listening Positions

- **On the Quad**, the default, sounds like a camera recording made on board:
  - the motors and props, close and loud
  - wind on the microphone, growing steeply with airspeed and with the props' downwash
  - a camera-style compressor, so a punch-out swells and everything else ducks, with the first instant of the swell slipping through. Its attack is about 20 ms.
  - a frame hum that follows the motors
  - no clipping: it was tried, and dropped
- **Where you stand**, at the Launch Spot:
  - The sound fades with distance.
  - It arrives late, at the speed of sound: about 0.15 s at 50 m.
  - Its pitch shifts as the Quad flies past.
  - The Map's walls muffle it, along the line from the Quad to the pilot.
    - **Walls muffle and never silence.** They cut the highs and lower the level a little, and the total cut is capped, because sound bends round a building.
    - **Walls are counted along five parallel lines 1 m apart**, not one, in the spirit of the Video Signal's averaging over the wave's width ([ADR-0019](../adr/0019-video-signal-averages-walls-over-the-wave-width.md)). So a column or a rail blocks only part of the sound, and the muffling builds up gradually as the Quad goes behind a wall.
  - There's no wind on a microphone, and beeps and crashes fade with distance like the motors.
  - Echo off the Map comes later.

### Beeps

- Beeps come from only two places:
  - the Flight Controller's beeper, with the Quad's Tune setting its `beeper` and `beacon` options
  - the ESCs, doing what Bluejay does

  The sim never adds a beep of its own, for an OSD warning or anything else.
- Every Betaflight beep also flashes Betaflight's visual beeper, `* * * *`, on the OSD. Nothing is told by sound alone.
- A Quad definition says whether the Quad has a buzzer.
  - The Freestyle 5″ has one. It plays Betaflight's beeps: arming and disarming, a chirp on each Flight Mode change, LOW BATTERY and LAND NOW, Crash Flip, and RX lost.
  - The RX lost beep starts when Failsafe drops the Quad, 1.5 s after the link goes, and lasts until the link comes back.
  - The Whoop 65, like the real Meteor65 Pro, has no buzzer and keeps its motor beacon off, so it is silent after a Failsafe drop.
- **Powering up.** Reset, a new Map and a new Quad all power the Quad up, as a new battery does.
  - The ESCs play their start-up melody, then their "signal found" and "ready" beeps, about 1.7 s in all. The motors answer only after the ready beep, and arming is blocked until then.
  - A Quad with a buzzer also plays Betaflight's power-up chirp and its three "gyro calibrated" beeps, because the fresh Flight Controller counts as calibrated at once.
- **The ESCs also beep:**
  - three falling tones when a motor stays off after its restarts fail following a Prop Strike (a restart that works is silent)
  - Bluejay's own beacon, after the Quad has sat idle for 10 minutes

### Other sounds

- Prop Strikes are made live: one tick for each blade that hits, as fast as the slowing prop and as hard as the rub.
- Hits and menu sounds are CC0 recordings.
- Each Map carries its Background Sound in its Pack, as a CC0 loop with its own level in dB. The file stays as recorded, and the Map's level sets how loud it plays.
- Every outside sound file is CC0 and is listed in `CREDITS.md` with its source and checksum.
- There's no music. Pilots who want music play their own.

### Settings and screens

- **The Sound tab in Settings holds:**
  - the Listening Position
  - the master volume, where 0 means off
  - a volume for each group:
    - Quad: motors, props, wind and beeps
    - Crashes: Prop Strikes and hits
    - Background
    - Menus

  These settings belong to the pilot, and no Preset changes them. There's no Mute Action.
- **Pause Menu:**
  - The Quad goes silent within about 0.1 s, because the Simulation is frozen.
  - The Background Sound carries on, quieter.
  - Menus click.
  - Resume brings the Quad back along with the Simulation.
- **The Hub and the loading screen** play menu sounds only.
- **The sound device:** the game asks for a 256-frame block, about 5 ms at 48 kHz. If the device can't do that, it takes the device's default. Firewheel's own default is 1024 frames, about 23 ms, which is too slow.
- **No sound device at start:** the game runs silent and says so once on the Hub.
- **Sound device lost in flight:** the sound moves to the new default device, and nothing pauses.
- **Window in the background:** sound keeps playing.

### Where sound sits

- Sound never enters the Simulation, and it sits outside the bit-exact guarantee. It reads each tick's state and never changes a flight.
- Sound is judged by ear. The maintainer's sign-off fixes a Quad's sound block.
- No CI check listens to the sound. CI only checks that the game starts and runs with no sound device.
