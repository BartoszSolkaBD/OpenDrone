# Reference spectra: real quads against the synth (#34, round 2)

Round 1 sounded synthetic to the maintainer, and its pitch sounded too high. This page checks the pitch against the physics, then measures real recordings and compares them with the synth using the same analyser (`./run.sh analyze FILE t0 t1`).

The reference recordings were downloaded only to be analysed. They aren't committed. The page links each one.

## 1. The whoop's pitch from the physics

Hover thrust per motor = 31.2 g × 9.81 / 4 = 0.0765 N, which is 7.8 g ([#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10): 31.2 g all-up).

The speed that gives this thrust depends on the prop's thrust coefficient, `T = k_f·ω²`.

**No BetaFPV table gives RPM for the Meteor65 Pro's 35 mm props.** Its table only pairs thrust with current: 30.6 g at 3.4 A and 4 V, and 8.8 g at 1.0 A. The only published whoop RPM point is the 0802 (2026) motor on 40 mm props: 55 g at 46,481 RPM (`docs/research/flight-dynamics.md` §8.1).

From that point:
- `C_T = T / (ρ n² D⁴)`, which gives 0.29.
- With the same `C_T` on 35 mm props: `k_f = C_T·ρ·D⁴ / 4π²` = 0.29 × 1.225 × 0.035⁴ / 39.48 ≈ 1.35 × 10⁻⁸ N/(rad/s)².

| | Speed | Shaft rate | Blade-pass (3 blades) | Motor electrical (6 pole pairs) |
|---|---|---|---|---|
| Hover | √(0.0765 / 1.35e-8) = 2,380 rad/s ≈ **22,700 RPM** | 379 Hz | **≈ 1,140 Hz** | ≈ 2,270 Hz |
| Full throttle (30.6 g) | √(0.300 / 1.35e-8) = 4,714 rad/s ≈ **45,000 RPM** | 750 Hz | ≈ 2,250 Hz | ≈ 4,500 Hz |

**Checks:**
- At 45,000 RPM, the back-EMF is 45,000 / 19,500 = 2.31 V of the 4 V supply. That leaves a lumped resistance of (4 − 2.31) / 3.4 ≈ 0.50 Ω, which is plausible.
- The hover current comes out at about 1.0 A. BetaFPV's table gives 8.8 g at 1.0 A, so this agrees.
- The hover duty comes out at about 41%, inside #10's expected 40–50%.

**Round 1 was wrong here, and in the other direction.**
- It used k_f = 2.15 × 10⁻⁸ from the research's draft table (§9.1). That is the 40 mm figure, not scaled down to 35 mm.
- That value would need C_T ≈ 0.46, above even the 0.29 the research already calls high for a free prop.
- So round 1 hovered at ≈ 18,000 RPM (blade-pass ≈ 900 Hz). The physics says ≈ 22,700 RPM (≈ 1,140 Hz): **about 4 semitones higher, not lower.**
- The prototype now uses the corrected numbers.
- **To raise as a physics question:** the research's §9.1 whoop `k_f`. The maintainer's phone recording would settle it, because the blade-pass line in a hover can be read off directly.

**Idle** (6%, `dshot_idle_value = 600`) comes out at ≈ 2,700 RPM in the rough model. That number rests on two unpublished Estimates: winding resistance and no-load current. It isn't verified.

**The 5″** is anchored by T-Motor's RPM data: 29,447 RPM at full throttle.
- Hover ≈ 9,400 RPM: blade-pass ≈ 470 Hz, shaft ≈ 157 Hz.
- Full throttle: blade-pass ≈ 1,470 Hz.
- The motor's electrical frequency is 7 pole pairs × shaft rate, ≈ 1,100 Hz at hover.

## 2. What real recordings show

| Recording | What it is | Use |
|---|---|---|
| [Freesound #677833](https://freesound.org/people/Sadiquecat/sounds/677833/), Sadiquecat, CC0 | Emax Tiny Hawk 2 whoop hovering indoors, Zoom H5 at waist level, 45 s | the whoop, from where you stand |
| [Freesound #854466](https://freesound.org/people/qubodup/sounds/854466/), qubodup, CC0 | on-board audio of a military FPV race drone (larger than 5″, from a US Army video) | On the Quad, a big FPV quad |
| [Freesound #854352](https://freesound.org/people/qubodup/sounds/854352/), qubodup, CC0 | an FPV drone flying past (US Marine Corps video) | a fly-by |

### Tiny Hawk 2 hover: 5 s windows across 45 s

- **The dominant line is the blade-pass, about 1,450 Hz.** The shaft rate is about 365 Hz (≈ 21,900 RPM), and the line falls at 4 × shaft, so the props are 4-blade.
- **That line is a wide cluster, not a pure tone.** It spreads over 1,320–1,610 Hz, about ±10%: the four motors run at different speeds and keep moving.
- **Other tones, relative to the blade-pass line:**

  | Tone | Level |
  |---|---|
  | 1× shaft | −12 to −16 dB |
  | 2× and 3× shaft | about −23 dB |
  | 2nd blade-pass harmonic | −7 to −11 dB |
  | 3rd blade-pass harmonic | −13 to −19 dB |

- **The motor's electrical frequency (6× or 7× shaft) is weak**, at −11 to −25 dB. The blade-pass dominates, not the motor whine.
- **Broadband noise carries about half the energy.** Only 25–58% is tonal; the rest is broadband.

  | Octave band | Level, relative to the 1–2 kHz band |
  |---|---|
  | 2–4 kHz | −4 to −7 dB |
  | 4–8 kHz | −4 to −7 dB |
  | 8–16 kHz | −6 to −11 dB |
  | 250–500 Hz | −16 to −20 dB |
  | below 250 Hz | about −30 dB |

  The spectral centroid is 2.8–3.9 kHz.
- **A 23.9 kHz line** is the ESC's 24 kHz PWM, which most adults can't hear. Bluejay "96k" on the Meteor65 Pro switches at 96 kHz, so it's silent.

### Big FPV quad, on board (#854466)

- The blade-pass dominates at about 260–310 Hz, with the shaft rate (about 87 Hz) at −4 to −19 dB.
- A second strong cluster sits around 900–1,000 Hz.
- Heavy low end: the 63–250 Hz bands are only 3–14 dB down.
- Highs are rolled off, as camera audio is: 8–16 kHz is 21–36 dB down.
- Tonal share is 22–74%, depending on the moment.

### FPV fly-by (#854352)

- A line at about 640 Hz with strong harmonics (1,290, 1,930, 3,220 Hz), plus a weak subharmonic.
- The pitch drops about 9% through the pass. That's Doppler at roughly 16 m/s, which matches what the prototype does.

## 3. What round 1 got wrong, and what changed

Measured the same way, round 1's whoop hover (on board) was:
- **89% tonal**, against about 45% in the real recording.
- **Razor-thin lines.** The four motors were within ±1%, and the "jitter" knob was mis-scaled ten times too small, so it did almost nothing.
- **A bright, regular comb of overtones:** blade-pass harmonics at −4, −6, −9, −12 dB.
- **Too little broadband**, and no high hiss.

That is the "synthetic" sound.

**Round 2:**

1. **Physics (the rough model, not the sound block):**
   - The whoop's k_f is corrected for 35 mm props, so it hovers at ≈ 22,700 RPM.
   - Both Quads now have an off-centre centre of gravity that an I-term holds, as a real quad does. That gives steady differences between the motors.
   - More hover buffeting (5% on the whoop, 3% on the 5″). The four whoop motors now spread 21,400–24,100 RPM in a hover, like the reference.
2. **The voice:**
   - Speed jitter is now properly scaled (sd 1.5% on the whoop, 1.2% on the 5″ at about 30 Hz), which widens each line.
   - A new **tone roughness**: random flutter of each tone's strength.
   - Fixed per-prop differences in the overtones.
   - The overtones fall off more steeply (whoop 1.5, 5″ 1.3).
   - Less shaft tone and growl; the whoop's hum is nearly off.
   - Much more broadband whoosh, centred lower (2× blade-pass).
   - A new **high hiss** band (tip and trailing-edge noise) above 2 kHz on the whoop and 1.5 kHz on the 5″.
   - Broadband noise now grows with speed at the same rate as the tones, so a hover isn't left as bare tones.
3. **On the Quad:** a camera-mic high cut (11 kHz). The compressor attack is 20 ms, and clipping is removed.
4. **Result for the whoop hover, from 1.5 m (`02s`), against the Tiny Hawk 2:**

   | | Synth (`02s`) | Tiny Hawk 2 |
   |---|---|---|
   | Tonal share | 35% | 25–58% |
   | Shaft tone | −13 dB | −12 to −16 dB |
   | 2nd blade-pass harmonic | −12 dB | −7 to −11 dB |
   | 250 Hz / 500 Hz bands | −17 / −17 dB | −16…−20 / −19…−25 dB |
   | 2 / 4 / 8 kHz bands | −5 / −7 / −7 dB | −4…−7 / −4…−7 / −6…−11 dB |
   | Centroid | 3.8 kHz | 2.8–3.9 kHz |

   On board (`02`): 48% tonal, the same balance, with highs a little lower.

   **5″:** about 61% tonal in a hover and 30% at full throttle, with a −7 dB shaft line in a hover. There's no true 5″ reference recording; the on-board reference is a bigger quad.

Matching the spectrum's shape isn't the same as sounding right. Only the maintainer's ears can judge that, and their own recording is the best reference.
