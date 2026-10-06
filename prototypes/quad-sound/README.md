# PROTOTYPE: the live Quad sound (#34)

Throwaway. It answers one question: **does the live Quad sound feel right, On the Quad and Where you stand?** Nothing here is production code, and the flight model is a rough stand-in that only exists to drive the sound.

- Ticket: [#34](https://github.com/BartoszSolkaBD/OpenDrone/issues/34). Spec: [#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32), `docs/context/sound.md`, ADR-0023.
- Sound: **Firewheel 0.14.0, used directly and pinned**, with no Bevy anywhere. Two custom nodes make the Quad's sound, and Firewheel's own sampler and volume nodes do the clips and the Sound tab's groups.

## Run

```
./run.sh                    # the app (first run: fetches the CC0 clips, builds SDL from source; a few minutes)
./run.sh render             # the listening files: renders/*.wav and renders/m4a/*.m4a
./run.sh bench              # CPU cost + live block size and output delay (silent), into results/bench.md
./run.sh --no-device        # the app as if there were no sound device
./run.sh nodevice           # the same path, headless
./run.sh analyze FILE 3 7   # spectrum of FILE (wav or ogg) between 3 and 7 s
./run.sh --load NAME.toml   # start from a saved tuning (tuning/NAME.toml)
```

Needs CMake (for SDL, already on the dev machine). On macOS only, `render` also makes the M4A copies with `afconvert`.

## The window

- **Left: what to fly and hear.**
  - Pick the Quad, the Map and the Listening Position.
  - Pick the sticks: Keyboard, or a Radio or Gamepad that SDL sees.
    - Radio: AETR on axes 0–3, and CH5 arms.
    - Gamepad: Mode 2, throttle on the left stick (it rests at 50%) or on R2.
  - Keys:
    - W/S throttle, Space punch, X chop
    - arrows roll/pitch, Q/E yaw
    - Enter arm, Backspace Reset, Esc Pause Menu
  - Scripted flights replay exactly the same flight, so a change can be judged on it. The Where-you-stand passes fly fixed lines past you.
  - The events buttons:
    - Reset (power-up), Settle mid-air
    - Prop Strike, Jam a prop, Hit
    - Lose the link
    - Skip 10 min idle (the beacon)
    - Pause Menu
    - pack charge
- **Middle:**
  - the Map from above, with the Launch Spot, you and the Quad. A red line means there are walls between you.
  - the four motors' speeds over the last 3 s, which is the warble you should hear.
- **Right: the tuning panel.** It holds:
  - the Sound tab's group volumes
  - the current Quad's **sound block**
  - the On the Quad and Where you stand settings
  - the CC0 clip choices, with ▶ to preview

  **Save tuning** writes `tuning/current.toml`, which loads next time, plus a dated copy. **Please send the file back.**

The top bar shows the live sample rate, block size, output delay, how old the newest tick is when the sound reads it, and the sound's peak CPU per block. "block request" changes the block size live.

## What makes the sound (all in `src/synth.rs`)

Round 2 rebuilt the voice against real recordings; see `results/reference-spectra.md`.

- **Each motor's voice** comes from its simulated speed and current:
  - **The blade-pass tone** (blades × rotation rate) dominates. Its overtones fall off steeply.
  - **Weaker shaft-rate tones** sit between them.
  - **Broadband whoosh**, pulsing at the blade rate, and a high **hiss** carry about half the energy, as in the Tiny Hawk 2 recording.
  - **The tones aren't steady.** Each motor's speed wobbles a little, each tone's strength flutters, and each prop's overtones differ slightly.
  - **Load (current)** makes it louder and brighter.
  - **The motor's electrical whine** stays weak, as measured.
- **The rest of the Quad:**
  - **Frame hum.**
  - **Bluejay's ESC tones**, made as its two current pulses per period ringing the motor bell.
  - **Betaflight's active buzzer**, gated by the real beeper patterns.
  - **Prop Strike ticks**, one for each blade passing while it rubs.
- **On the Quad:** the camera mic, with:
  - a low cut and a high cut
  - wind on the mic from airspeed and the props' downwash
  - a camera-style compressor with a 20 ms attack, so the first instant of a punch-out gets through

  Clipping was tried in round 1 and dropped.
- **Where you stand:**
  - **Travel time** at 343 m/s through a variable delay. Doppler comes from that delay, not from a separate effect.
  - **Distance:** the sound falls off as 1/distance beyond a "full level" radius, plus air absorption.
  - **Walls muffle, never silence.** They cut the highs and the level, at most about 15 dB for the highs plus 9 dB overall. The prototype counts walls along five parallel lines 1 m apart, so a column or a rail blocks only part of the sound.
  - **A safety limiter** stops a punch-out right next to you from clipping.
  - **No wind on the mic, and no echo yet.**

## Where the numbers come from

- **Motor model** (`src/quads.rs`, *not* tunable here, because a sound is never fixed by moving a physics number): `docs/research/flight-dynamics.md` §8–9 and the `diff all` exports.
  - **Whoop:** 0802SE 19500KV, 35 mm 3-blade, 1S, 12 poles, DShot300, idle 6%. It hovers at ≈22,700 RPM (blade-pass ≈1,140 Hz) and tops out at ≈45,000 RPM. Round 2 corrected the thrust coefficient for 35 mm props; see `results/reference-spectra.md` §1.
  - **5″:** 1750KV, 5.1″ 3-blade, 6S, 14 poles, idle 5.5%. It hovers at ≈9,400 RPM (≈470 Hz) and tops out at ≈29,600 RPM.
- **Beeps** (`src/beeps.rs`): Bluejay v0.21.0 and Betaflight 2026.6.2 source. Every figure is traced, with file and line, in `results/beeps-facts.md`.
- **Clips** (`assets/clips.toml`): all CC0 1.0, each checked on its source page, with SHA-256. `assets/fetch-clips.sh` downloads and verifies them; the files themselves aren't committed.

## Results

- `results/reference-spectra.md`: the physics pitch check, and real recordings against the synth.
- `results/bench.md`: CPU cost and live delay on the M4.
- `renders/README.md`: what each rendered file is.
- `results/ui-screenshot.png`: the window.
