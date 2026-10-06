# What a real quad sounds like, and what the sound prototype measured

Findings from the sound prototype, [#34](https://github.com/BartoszSolkaBD/OpenDrone/issues/34). Its rules are in [docs/context/sound.md](../context/sound.md) and [ADR-0023](../adr/0023-quad-sound-made-live-on-firewheel.md). The prototype itself is on branch [`prototype/quad-sound`](https://github.com/BartoszSolkaBD/OpenDrone/tree/prototype/quad-sound/prototypes/quad-sound), in `prototypes/quad-sound/`. It includes:
- the app and its tuning panel
- 24 renders, and round 1's renders
- `results/reference-spectra.md` and `results/beeps-facts.md`

**Where it stands:** the approach is confirmed, but **the voice isn't convincing yet.** It gets reworked and tuned by ear during the build, starting from the prototype's round 2. The maintainer's phone recording of their Meteor65 Pro is still wanted as the reference.

## 1. The whoop's pitch from the physics

Hover thrust per motor is 31.2 g × 9.81 / 4 = 0.0765 N, which is 7.8 g ([#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10)).

The speed that gives this thrust depends on the prop's thrust coefficient, `T = k_f·ω²`.

**The data:**
- BetaFPV gives no RPM for the Meteor65 Pro's 35 mm props.
- The only published whoop RPM point is the 0802 (2026) motor on 40 mm props: 55 g at 46,481 RPM. That gives `C_T` ≈ 0.29.
- The same `C_T` on 35 mm props gives `k_f` ≈ 1.35 × 10⁻⁸.

| | Speed | Shaft rate | Blade-pass (3 blades) | Motor electrical (6 pole pairs) |
|---|---|---|---|---|
| Hover | ≈ 22,700 RPM | 379 Hz | ≈ 1,140 Hz | ≈ 2,270 Hz |
| Full thrust | ≈ 45,000 RPM | 750 Hz | ≈ 2,250 Hz | ≈ 4,500 Hz |

**Cross-checks:**
- Hover current comes out at ≈ 1.0 A. BetaFPV gives 8.8 g at 1.0 A.
- Hover duty comes out at ≈ 41%. #10 expects 40–50%.
- At full thrust, the back-EMF is 2.31 V of 4 V, so the lumped resistance is ≈ 0.50 Ω.

**Not verified:** idle at 6% (`dshot_idle_value = 600`) gives ≈ 2,700 RPM, but that rests on two Estimates (winding resistance and no-load current).

**A correction for the research.** `flight-dynamics.md` §9.1 lists the 40 mm `k_f` unscaled. The prototype's first round used it and hovered at ≈ 18,000 RPM (≈ 900 Hz). That correction is noted in §9.1 as a physics question.

**The 5″** rests on T-Motor's RPM data:

| | Speed | Blade-pass |
|---|---|---|
| Hover | ≈ 9,400 RPM | ≈ 470 Hz |
| Full thrust | ≈ 29,600 RPM | ≈ 1,470 Hz |

## 2. Real recordings

All three are CC0 on Freesound. They were downloaded only to be analysed, and aren't committed.

**[#677833](https://freesound.org/people/Sadiquecat/sounds/677833/): a Tiny Hawk 2 whoop hovering indoors, mic at waist level.**
- **The blade-pass line dominates.** It sits at ≈ 1,450 Hz (4-blade props, shaft ≈ 365 Hz).
- **That line is a wide cluster.** It spans ±10%, because the four motors differ and keep moving.
- **The other tones are weaker**, relative to the blade-pass line:

  | Tone | Level |
  |---|---|
  | Shaft tone | −12 to −16 dB |
  | 2nd blade-pass harmonic | −7 to −11 dB |
  | 3rd blade-pass harmonic | −13 to −19 dB |

- **The motor's electrical whine is weak**, at −11 to −25 dB.
- **About half the energy is broadband.** 25–58% is tonal; the rest is noise.
- **The noise reaches high.** The 2–16 kHz bands are only 4–11 dB below the main band, and the centroid sits at 2.8–3.9 kHz.

**[#854466](https://freesound.org/people/qubodup/sounds/854466/): on-board audio from a military FPV quad, larger than a 5″.**
- The blade-pass line dominates, with a strong shaft tone and a heavy low end.
- The highs are rolled off, as camera audio is.

**[#854352](https://freesound.org/people/qubodup/sounds/854352/): an FPV fly-by.**
- The pitch drops about 9% through the pass: Doppler at roughly 16 m/s.

**What this meant for round 1.** It measured 89% tonal, with razor-thin lines, a bright and regular comb of overtones, and little broadband noise. That is why it sounded synthetic.

**Round 2 brought it close to the Tiny Hawk 2's spectral shape:**
- a wide line, from motors that really differ
- speed jitter and tone flutter
- steeper overtones
- strong broadband noise and a high hiss

The maintainer still judged the voice not convincing. Matching a spectrum's shape isn't enough by itself.

**No true 5″ reference was found.**

## 3. Measured on the Mac mini M4

**CPU:** the whole graph (four motor voices, the Listening Position, volumes and samplers) costs **1.1–1.5% of one core**, measured offline. Live at 256 frames, the busiest block used about 5% of its time.

**Output delay** is what CoreAudio reports through cpal: the device buffer plus the device's latency and safety offset. It was measured on wired headphones at 48 kHz. It isn't acoustic, because the Mac mini has no microphone.

| Block | Output delay | Worst case, tick to speaker (one block more) |
|---|---|---|
| 128 frames | 4.9 ms | 7.5 ms |
| **256 frames** | **7.5 ms** | **12.9 ms** |
| 1024 frames (Firewheel's default) | 23.5 ms | 44.9 ms |

Bluetooth output is unmeasured, and is far slower.

**No sound device:** the prototype ran silent, said so once, and skipped every sound call.

## 4. Firewheel 0.14.0 notes

- **`Volume::Linear` squares its value.** It's a slider curve: amplitude = value². Gains that are already amplitudes go in as the square root.
- **Turning on per-node profiling (`profile_nodes`) panicked** on the audio thread (`profiling.rs:226`, index out of bounds). The overall `profiling_data().overall_cpu_usage` worked.
- **The graph runs offline.** `FirewheelContext::activate` hands back the processor, so a test or a renderer can drive it with no device. The prototype's WAV renders are made this way, through the same graph that plays live.
- **The cpal backend asks for 1024 frames by default.** The block request has to be set explicitly.
- **Stream errors arrive through `CpalStream::poll_status`.** After an error, `output_stream_ok()` says whether to reopen the device.
- **The two Quad nodes are custom.** They read each tick's state through a wait-free triple buffer, and take tuning values as Diff/Patch parameter events.
- **Clips use the stock nodes.** Hit, menu and Background Sound clips play on `SamplerNode`, and the groups use `VolumeNode`.

## 5. Choices made in the prototype

**Clips** are all CC0 1.0, each checked on its source page, with SHA-256. They're listed in the prototype's `assets/clips.toml`.

| Use | Clip |
|---|---|
| Hit, whoop | Kenney Impact Sounds `impactGeneric_light_000` |
| Hit, 5″ | Freesound [#854351](https://freesound.org/people/qubodup/sounds/854351/), a real FPV quad hitting a window frame, trimmed to the hit (the maintainer didn't choose; this is the default) |
| Menus | Kenney Interface Sounds |
| Skate Park Background Sound | Freesound [#640600](https://freesound.org/people/gokalp_gonen/sounds/640600/), city birds and distant vehicles |
| Bando Background Sound | Freesound [#545035](https://freesound.org/people/gecop/sounds/545035/), wind, plastic sheeting and a squeaky crane on a building site |

Rejected backgrounds:
- [#580646](https://freesound.org/people/njlyczko/sounds/580646/): nearly silent as recorded.
- [#711196](https://freesound.org/people/Rashpil/sounds/711196/): likely voices.

The Map levels that put both chosen backgrounds at about −30 dBFS RMS are **Skate Park −10 dB and Bando +9 dB**. The files themselves stay as recorded.

**Beeps** come straight from the firmware:
- Bluejay v0.21.0: the default melody, the "signal found" and "ready" beeps (ready at ≈ 1.66 s), the three falling stall tones, and the ≈ 10-minute beacon every ≈ 3.1 s.
- Betaflight 2026.6.2: every `beeper.c` pattern and priority.

The prototype's `results/beeps-facts.md` traces each one, with file and line.
