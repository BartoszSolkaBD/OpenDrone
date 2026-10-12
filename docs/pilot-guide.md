# Flying OpenDrone

OpenDrone is a free FPV simulator for pilots who want to practise, or who can't fly their real quads right now. It aims to fly like the real thing: the props, the battery, the crashes and the Flight Controller behave as they do on a real quad, so what you practise here carries over.

**There's a first test flight, and not much more yet.** OpenDrone is being built towards its first playable version, the 0.1.0 alpha, for macOS, Windows and Linux. This guide grows with it. [Your first flight](#your-first-flight) says how to fly what there is; the rest says what the alpha will have.

## Your first flight

The first flight is a test: either Quad over a flat floor with a grid, through a plain camera on the Quad's frame. There are no menus, no sound, no OSD and no fisheye yet, and the settings are fixed.

**Starting it.** From the OpenDrone folder, with Rust installed (see [Contributing](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/CONTRIBUTING.md)):

```sh
cargo run --release -p opendrone                        # the Whoop 65
cargo run --release -p opendrone -- --quad freestyle-5  # the Freestyle 5″
```

The first build takes a few minutes. `--release` matters: without it the picture is slow.

**Picking your Input Device.** Plug it in before or after starting. Then move a stick on the device you want to fly with: it becomes the Flying Input Device, and the flight starts afresh, with the Quad landed in the middle of the floor. The window's title says which device flies, whether the Quad is armed or what stops it arming, Failsafe, and Pause.

**Arming.** Wait about 2 s after the flight starts: the ESCs play their start-up tones first, as on a real quad, and the title says `BOOTGRACE` until they're ready. Then, with the throttle at the bottom, flip Arm.

**On the Radiomaster Pocket** (over USB, in EdgeTX's Classic USB joystick mode, with the RF module off and the sim model's ADC filter Off):

- CH1 to CH4 are roll, pitch, throttle and yaw.
- CH5 high arms. CH6 is the Flight Mode: low Acro, middle Horizon, high Angle. CH7 high is Crash Flip.
- SE (CH9) resets the Quad.

**On the DualSense** (over USB):

- Mode 2: the left stick is throttle and yaw, the right stick pitch and roll. The throttle is zero with the left stick at rest in the middle, and rises as you push it up.
- R1 arms and disarms. It turns itself off whenever the Quad disarms, so the next press always arms. Hold L1 for Crash Flip.
- Options pauses, and Create resets the Quad.
- It has no Flight Mode switch, so it flies Acro.

**On the keyboard:** R resets the Quad, and Esc pauses and goes on.

**The fixed settings, for now:** a 250 Hz Packet Rate, Betaflight 2026.6's default Rates (Actual, 70 °/s at centre and 670 °/s at full stick), Acro for a device with no Flight Mode switch, every Assist off (so the battery runs down: Reset gives a fresh one), and your device uncalibrated, on its profile's starting values.

**What's a placeholder:** the camera is a plain wide lens, 90° top to bottom, at the Quad's Camera Tilt. The Quad's own fisheye Lens, its Video Look and the OSD come later. On the Whoop 65 you see the front ducts at the sides.

**If something's wrong:** the terminal you started it from lists the devices it finds and why one might not fly. `cargo run -p opendrone -- --help` lists the other ways to start it.

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
- The Flight Controller has Betaflight's filters, RC smoothing and feedforward, from each Quad's Tune. The filters' small delays are part of how a Quad handles Prop Wash, as on a real quad. RC smoothing and feedforward tune themselves to your Packet Rate, as Betaflight does: the same stick move gives a little less feedforward at 500 Hz than at 250 Hz, so pick the Packet Rate your real radio link uses.
- Prop Wash comes from the Quad descending into its own air, never from an artificial shake or a slider.
- A tired pack sags and fades until the Quad can't hover, with no hard cutoff.
- No setting changes the physics. Every Preset flies the same physics.

## Following along

The plan for the alpha is [the spec](https://github.com/BartoszSolkaBD/OpenDrone/issues/37), and its tickets show what's being built. The rest of this book explains how OpenDrone works and how its physics is proved.
