# Flying OpenDrone

OpenDrone is a free FPV simulator for pilots who want to practise, or who can't fly their real quads right now. It aims to fly like the real thing: the props, the battery, the crashes and the Flight Controller behave as they do on a real quad, so what you practise here carries over.

**There's a first test flight, and not much more yet.** OpenDrone is being built towards its first playable version, the 0.1.0 alpha, for macOS, Windows and Linux. This guide grows with it. [Your first flight](#your-first-flight) says how to fly what there is; the rest says what the alpha will have.

## Your first flight

The first flight is a test: either Quad on the Skate Park, through the Quad's FPV Camera. There are screens to get you into the air (see [The screens](#the-screens)), but not yet the whole Settings drawer, no OSD and no Breakup. You hear the Quad's motors, but no beeps yet.

**Starting it.** From the OpenDrone folder, with Rust installed (see [Contributing](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/CONTRIBUTING.md)):

```sh
cargo run --release -p opendrone                        # First Launch the first time, then the Hub
cargo run --release -p opendrone -- --quad freestyle-5  # pick the Freestyle 5″, and keep the choice
cargo run --release -p opendrone -- --no-screens        # developer command: fly at once, no screens
cargo run --release -p opendrone -- --map flat-floor    # developer command: fly at once over a grid floor, nothing to hit
```

The first build takes a few minutes. `--release` matters: without it the picture is slow.

### The screens

**First Launch** is one screen, shown once, on a new install. Text size and Reduce motion are at the top, then a notice about flashing images (FPV video static and Breakup can flash), then "Have you flown FPV before?". Your answer picks your starting Preset: No, never is Beginner; A little is Intermediate; Yes is Pro; Skip is Beginner. A second step is about your Input Device: it shows the Radio or Gamepad it found, or says "Plug in your Radio or Gamepad" and offers "Continue without". Setting up an Input Device (Calibration and switches) is a later version's job: **Set up** takes you on to the Hub and says so, and your device flies on its profile's starting values. A new install starts with the Whoop 65 on the Skate Park.

**The Hub** is where you land after that, and every time after. FLY is selected when you arrive, so a returning pilot is in the air in one press; a new one needs three (answer the question, Set up later, FLY). Under FLY are the Map, the Quad and the Preset, each changed in place with left and right; Settings; Quit; and a line, "Flying with: …", that says which Input Device will fly. A **Pre-flight Warning** (a yellow ! and a sentence) tells you when something would stop you flying well: no Input Device connected, or Arm not bound on your device while Auto-arm is off. A warning never stops FLY. A short notice says when an Input Device is plugged in or unplugged, and, once, when there's no sound device. Presets don't change anything yet: the Hub remembers which one you chose, and what each one sets (Flight Mode, Rates, Assists, camera values) arrives with the Settings drawer.

**The loading screen** follows FLY and every swap of the Map or the Quad. It shows the Map, the Quad and the Preset, a progress bar, and a reminder of the keys, with a yellow ! and "not bound" if nothing arms the Quad. The Map is drawn behind it and the graphics card builds its shaders there, so the first seconds of the flight don't stutter.

**The Pause Menu** opens with Esc, or the Pause button of your Input Device (Options on a DualSense). It freezes the Simulation and the Quad's sound goes quiet. It offers Resume, Reset, Map, Quad, Settings, Back to the Hub and Quit. Resume counts 3, 2, 1 before the Simulation runs again, but only if the Quad was in the air. Picking another Map or Quad puts the Quad on that Map's Launch Spot, powered up afresh, through the loading screen.

**Settings** is, for now, a short page: Text size, Reduce motion and Auto-arm. Every setting is one stop, changed with left and right, and takes effect and is saved the moment you change it. Flying, Camera, Graphics, Input Devices, Sound and Packs arrive later.

**Moving around.** On a Radio the right stick moves the cursor, a flick of yaw to the right selects, and a flick to the left goes back; the throttle and the switches are never read, so a Radio alone does everything. On a Gamepad the D-pad or the left stick moves, ✕ selects, ○ goes back, and L1 and R1 will switch tabs. On the keyboard the arrows move, Enter or Space selects, Backspace goes back, and the mouse works too. The Quad is never flown from a menu, and a menu is never driven from a flight: the sticks fly in Free Flight and move the cursor everywhere else.

**Your settings** are kept in one file, `settings.toml`, saved the moment a setting changes: in `~/Library/Application Support/OpenDrone` on macOS, `%APPDATA%\OpenDrone` on Windows, and `~/.config/opendrone` on Linux. Saved flights go beside it, in `saved-flights/`. `--settings-folder DIR` uses another folder. If the file can't be read, the game keeps it aside as `settings.broken.toml`, tells you, and starts fresh.

**The Skate Park.** An outdoor concrete park, 62 × 40 m, sunk into the ground: a pool bowl, a round cradle, a snake run that winds along the south side, a full pipe, and on the east a street corner with stairs, handrails, a ring and a pergola with slats to fly between, all at real-world size. The Quad starts on the flat deck between the two bowls, facing the pool. Everything solid is solid: there are no invisible walls, and the ground runs to the horizon. Its surfaces are real concrete, steel, plywood and grass pictures (CC0, from Poly Haven and ambientCG), which `cargo xtask textures` downloads the first time; without them the Map is drawn in flat colours, and flies the same. [Building a Map](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/building-a-map.md) says where they come from.

**Picking your Input Device.** Plug it in before or after starting. The device that selects FLY is the one that flies: a Radio with a flick of yaw to the right, a Gamepad with ✕. If you press FLY with the keyboard or the mouse, the device you last moved a menu with flies, or the only one plugged in. If there are several and you used none, the Quad waits on the Launch Spot until you move a stick on the device you want to fly with, and the flight starts afresh with it. Either way it is the Flying Input Device, and it never changes in flight. During a flight the window's title says which device flies, whether the Quad is armed or what stops it arming, and Failsafe.

**Arming.** Wait about 2 s after the flight starts: the ESCs play their start-up tones first, as on a real quad, and the title says `BOOTGRACE` until they're ready. Then, with the throttle at the bottom, flip Arm.

**On the Radiomaster Pocket** (over USB, in EdgeTX's Classic USB joystick mode, with the RF module off and the sim model's ADC filter Off):

- CH1 to CH4 are roll, pitch, throttle and yaw.
- CH5 high arms. CH6 is the Flight Mode: low Acro, middle Horizon, high Angle. CH7 high is Crash Flip.
- SE (CH9) resets the Quad.

**Angle and Horizon** level the Quad for you, as Betaflight 2026.6 does. In Angle the roll and pitch sticks ask for a tilt instead of a spin: full stick leans the Quad to its Tune's angle limit (55° on the Whoop 65, 60° on the Freestyle 5″), half stick about 17°, and letting go levels it. Horizon levels like Angle near the centre of the sticks and flies like Acro towards full stick or far from level, so you can still flip and roll; after a flick its levelling comes back over about half a second. Yaw is Acro in both. Both level against the Quad's true attitude, so they never drift.

**On the DualSense** (over USB):

- Mode 2: the left stick is throttle and yaw, the right stick pitch and roll. The throttle is zero with the left stick at rest in the middle, and rises as you push it up.
- R1 arms and disarms. It turns itself off whenever the Quad disarms, so the next press always arms. Hold L1 for Crash Flip.
- Options pauses, and Create resets the Quad.
- It has no Flight Mode switch, so it flies the Flight Mode setting: Acro, for now.

**On the keyboard:** in flight, R resets the Quad, and Esc opens the Pause Menu (and, in it, goes on). The up and down arrows change the Camera Tilt, and the right and left arrows change the FOV (see [The camera](#the-camera)).

**The settings the screens don't change yet:** a 250 Hz Packet Rate, Betaflight 2026.6's default Rates (Actual, 70 °/s at centre and 670 °/s at full stick), Acro for a device with no Flight Mode switch, every Assist off but Auto-arm, which Settings turns on (so the battery runs down: Reset gives a fresh one), and your device uncalibrated, on its profile's starting values. The settings file already holds the Flight Mode, the Assists and the Packet Rate, so editing them in the file works until the Settings drawer arrives.

**The sound** is made live from the simulated motors, never played from recordings: each of the four motors has its own voice, so you hear the throttle, the Flight Controller's corrections and Prop Wash as the motors warble. You hear it On the Quad, as a camera's microphone on board would: wind on the mic grows with your speed and with the props' downwash, and a punch-out swells for an instant before the camera's compressor pulls it back. Pause silences the Quad, and going on brings it back. It keeps playing when the window is in the background. The voice isn't final: it's being tuned by ear. If you have no sound device, the game runs silent and the Hub says so once, in a short notice (a game started with `--no-screens` or `--scenario` has no Hub, and just runs silent); if your headphones go, the sound moves to your computer's new default device without pausing. The ESCs' start-up tones and the buzzer's beeps, Where you stand, crash sounds and the volume settings come later.

**What's a placeholder:** the light. The Skate Park is lit by a plain sun with shadows and some sky light, not yet by the baked light that gives it real shade. The OSD and Breakup, the picture falling apart as the Video Signal weakens, come later too. The Camera Tilt and FOV readout shows in the window's title for a moment, until the OSD arrives. The screens' look is a placeholder too: plain dark panels in a plain font.

### The camera

You fly through the Quad's FPV Camera, bolted to its frame, as on a real quad.

- **The Lens.** Every Quad's camera is a fisheye: straight lines bend towards the edges. The Whoop 65's is 160° and the Freestyle 5″'s 155°, measured corner to corner of the 4:3 picture, like the number on a camera's box. That is about 128° across on the Whoop.
- **Camera Tilt and FOV.** Both start at the Quad's own values (30° on both, and 160° or 155°), and you can change them in flight: the up and down arrows tilt the camera between 0° and 80°, and the right and left arrows widen and narrow the FOV between 90° and 170°. A press moves 1°. Hold the key and it repeats after a second, then five times a second. A Gamepad or Radio button bound to the same Actions does the same. The window's title shows the new value for a moment. Each Quad keeps its own values, and they are kept in the settings file the moment they change.
- **Analog or Digital.** `--look analog` (the default) is a soft picture with a little grain and colour that bleeds sideways, like analog goggles. Each Quad's own camera sets how soft it is: the Whoop 65's is the softest. `--look digital` is clean and sharp, with a gentler fisheye, and it shows the picture 15 ms later than Analog, as real digital systems do. That delay is in the picture only: the sticks and the flight are untouched.
- **4:3 or 16:9.** `--aspect 16:9` crops the top and bottom of the picture and never stretches it. 4:3 is the default.
- **Light.** The camera adapts to light as a real one does, over about a second: flying into a dark room starts near black, then the room shows within a second, with its windows blown out; flying back out blows out to white before the picture recovers. Each camera holds only so many stops of light between black and white (7 on the Whoop 65, 8.5 on the Freestyle 5″, 10 on Digital), so bright things blow out and dark things sink to black.
- **Your own Quad.** The Whoop 65's camera sits between its front ducts, a few millimetres behind their outer edge, so its picture shows no duct walls, and when the Whoop hits a wall nose first you see the wall, not through it. The props of both Quads show only where the Camera Tilt and FOV let you see them.

Pick the Video Look and the picture's shape when you start the game: `cargo run --release -p opendrone -- --look digital --aspect 16:9`. The choice is kept in the settings file. The Settings drawer adds menus for them.

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
- **Betaflight's OSD**, once the goggles show it, tells you only what a real quad could: the battery, the timers, Link Quality, the Flight Mode and Betaflight's warnings, in Betaflight's order. A punch-out on a tired pack warns LOW BATTERY. Lose the link with the Arm switch on and it shows RXLOSS at once, then FAILSAFE when the Quad drops at 1.5 s, as Betaflight does; FAIL SAFE itself shows only if you've switched the arming warnings off. Your own OSD layout will paste straight from the Betaflight CLI.
- No setting changes the physics. Every Preset flies the same physics.

## Following along

The plan for the alpha is [the spec](https://github.com/BartoszSolkaBD/OpenDrone/issues/37), and its tickets show what's being built. The rest of this book explains how OpenDrone works and how its physics is proved.
