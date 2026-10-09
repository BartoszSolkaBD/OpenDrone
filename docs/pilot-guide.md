# Flying OpenDrone

OpenDrone is a free FPV simulator for pilots who want to practise, or who can't fly their real quads right now. It aims to fly like the real thing: the props, the battery, the crashes and the Flight Controller behave as they do on a real quad, so what you practise here carries over.

**There's nothing to fly yet.** OpenDrone is being built towards its first playable version, the 0.1.0 alpha, for macOS, Windows and Linux. This guide grows with it. For now, it says what the alpha will have.

## What the alpha will have

- **Free Flight** on two Maps at real-world scale, with no objectives, timers or scoring and no invisible walls:
  - the **Skate Park**, a sunken concrete park with a pool bowl, a full pipe and a street corner;
  - **Bando**, a five-floor unfinished tower with brick rooms, an open lift shaft and a tower crane.
- **Two Quads:** the **Whoop 65**, modelled on the BetaFPV Meteor65 Pro, and the **Freestyle 5″**, a typical 5″ freestyle quad.
- **A Flight Controller that copies Betaflight 2026.6.** Each Quad flies a real quad's Tune, and you can paste your own Rates straight from the Betaflight CLI, so your muscle memory works.
- **Your Radio or Gamepad.** The Radiomaster Pocket and the DualSense are checked by hand; other EdgeTX and OpenTX radios and other gamepads work too. Every Input Device flies through an emulated ELRS Radio Link, so each one feels like a real link.
- **An FPV Camera** with a fisheye Lens, an Analog or a Digital Video Look, Breakup when walls weaken your signal, and Betaflight's OSD.
- **Live Quad sound,** made from the simulated motors, heard On the Quad or from Where you stand.
- **Presets and Assists.** The Beginner, Intermediate and Pro Presets pick your starting settings. Assists such as Angle and Horizon, Input smoothing and Endless Battery make flying easier without touching the physics.

## What stays real

- When a prop touches something, it rubs, brakes its motor and pushes the Quad, as a Prop Strike does. Nothing breaks in the alpha.
- After a crash, nothing disarms or resets on its own. An upside-down Quad comes back with Betaflight's Crash Flip, or with Reset.
  - **Crash Flip** works as on Betaflight 2026.6: turn the Crash Flip switch on, then arm. Upside down, it arms anyway. Move the stick the way you want the Quad to tip over: pitch forward lifts the front, roll right the right side, and a diagonal one corner. Let go as it comes over, then turn the Crash Flip switch off, which disarms the Quad; flip the Arm switch off and on to fly. Turning Crash Flip on while you're armed does nothing.
  - **Runaway takeoff prevention** is on, as it is on a real quad: right after arming, until you've flown about half a second normally, a Quad that can't turn the way its Flight Controller asks, because it's pinned against a wall, say, disarms after 75 ms. Flip the Arm switch off and on to fly again.
  - **Yaw spin recovery** is on: a hit that spins the Quad faster than your Rates ever ask makes the Flight Controller brake the spin before you get control back.
- Prop Wash comes from the Quad descending into its own air, never from an artificial shake or a slider.
- A tired pack sags and fades until the Quad can't hover, with no hard cutoff.
- No setting changes the physics. Every Preset flies the same physics.

## Following along

The plan for the alpha is [the spec](https://github.com/BartoszSolkaBD/OpenDrone/issues/37), and its tickets show what's being built. The rest of this book explains how OpenDrone works and how its physics is proved.
