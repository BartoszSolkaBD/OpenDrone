# Betaflight and our Flight Controller: what to replicate, and is SITL practical?

Research for [#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). Researched 2026-10-03.

- **Reference version:** Betaflight **2026.6.2** (released 2026-09-16, tag commit `e0b7bb0`). Every source link below points at that tag. Older Betaflight versions differ in places, and those places are flagged.
- **Vocabulary:** see [`CONTEXT.md`](../../CONTEXT.md) and [`docs/context/flying.md`](../context/flying.md). "Flight Controller" means our simulated firmware. A **Quad** is the simulated aircraft.
- **Licensing sections are not legal advice.** They summarise what the licence text and the Free Software Foundation (FSF) say. Get a lawyer's view before you ship anything that bundles or links Betaflight.
- **Code is not copied here.** Betaflight is GPL-3.0. This note describes its behaviour in maths and plain words, not in its C source (see [§8.4](#84-reimplementing-betaflight-behaviour-in-rust)).
- **Corrected 2026-10-04.** [#21](https://github.com/BartoszSolkaBD/OpenDrone/issues/21) checked the source again at 4.3.0, 4.4.0, 2025.12.1 and 2026.6.2, and fixed §3, §3.1, §4 and §6.3. The fixes cover version scoping, the I-term hold, the throttle-curve date, low TPA, the failsafe timeline and Dynamic D. The decisions that rest on this note are in [ADR 0007](../adr/0007-emulated-radio-link.md) and [ADR 0008](../adr/0008-copy-betaflight-2026-6-translate-older-tunes.md).

## Answer in brief

1. **What our Flight Controller must copy, in order of how much the pilot feels it:**
   1. the stick-to-rotation-speed curves (rates)
   2. the PID loop, with Betaflight's exact gain scaling
   3. the motor mixer with Airmode and motor idle
   4. feedforward with RC smoothing
   5. the gyro and D-term low-pass filters, because they add delay
   6. then the extras: throttle curve and boost, anti-gravity, I-term relax, TPA and Dynamic D

   All of this is a few hundred lines of maths and cheap to run. The notch filters (RPM filter and dynamic notch) only matter if we simulate motor vibration noise, so they can wait. The full ranked list is in [§1](#1-flight-controller-features-ranked-by-importance-to-feel).

   One catch: feedforward and RC smoothing tune themselves to how often stick updates arrive. So the rate at which each Input Device feeds the Flight Controller changes the feel ([§3.1](#31-radio-link-rate-how-rc-smoothing-and-feedforward-adapt)).
2. **Running real Betaflight in software (SITL) works as a developer tool, not as something we ship to players.**
   - It builds and runs on Linux and macOS. I built and ran it on this Mac mini M4 in about 5 seconds.
   - It has no native Windows build. Windows only works through WSL2.
   - It talks to a simulator over local network messages (UDP), so it runs as a separate program.
   - Its clock is the wall clock, not our simulation clock, and its virtual gyro is fixed at 1 kHz. Real hardware runs at 8 kHz. So it can't run lock-step with our physics and isn't deterministic.
   - It is built with `-Ofast` and fast-math, uses threads and calls the platform maths library. That puts it outside our bit-identical determinism guarantee ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7)) unless we keep a patched GPL fork.
   - It has no DShot, no RPM filter and no dynamic idle.
3. **Licensing (not legal advice):**
   - **Linking Betaflight's code into our app** makes the shipped app one combined program, and the whole of it must be released under GPL-3.0. That would override our MIT/Apache intent for the app.
   - **Running SITL as a separate program** that talks to ours over UDP keeps our code MIT/Apache, by the FSF's own reading of the GPL.
   - **Not shipping SITL at all**, and letting a developer build it locally or in CI, creates no GPL duties for us.
   - **Hand-translating Betaflight's C code into Rust** counts as making a modified version, so the result would be GPL. **Re-implementing the same behaviour from its maths and docs** is not copying, because copyright protects code text, not ideas or algorithms.

## 1. Flight Controller features ranked by importance to feel

"Feel" means what a pilot of the real Meteor 65 or a 5" quad notices in the first minutes of flying. Each row says whether the feature matters when the simulated gyro is perfectly clean, which is the alpha's default.

| # | Feature | What it does, in plain words | Matters without simulated sensor noise? | Betaflight default (2026.6.2) |
|---|---|---|---|---|
| 1 | **Rates**: Actual, Betaflight, Quick, KISS and Raceflight, plus the rate limit | Turns stick position into a target spin speed in degrees per second. This is the pilot's "muscle memory". Pilots will type their real numbers in. | **Yes, essential.** | Actual: centre 70 °/s, max 670 °/s, expo 0, limit 1998 °/s ([rc.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L190-L268), [controlrate_profile.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/controlrate_profile.c#L42-L67)) |
| 2 | **PID loop (rate mode)**: P, I and D (with D on the gyro reading), Betaflight's scaling constants, PID-sum limits and the I-term limit | Compares target spin speed with measured spin speed and corrects the error. The scaling constants make a pilot's "P 45" mean the same thing here as on their quad. | **Yes, essential.** | Roll 45/80/30, pitch 47/84/34, yaw 45/80/0 (P/I/D). PID-sum limit 500, or 400 on yaw ([pid.h](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.h#L37-L71)) |
| 3 | **Mixer, Airmode, motor idle and output range** | Spreads the PID output across the four motors. Airmode keeps full control at zero throttle (flips, dives). Idle stops motors falling below a floor. | **Yes, essential.** Low-throttle tricks depend on it. | Airmode on. Legacy mixer. Idle 5.5 %. Airmode engages once throttle first passes 25 % ([mixer.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L676-L845), [core.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L841-L866)) |
| 4 | **Feedforward and RC smoothing** | Pushes the Quad into a move as soon as the stick moves fast, before an error builds up. Smoothing hides the steps between radio packets. Both tune themselves to the radio's packet rate. | **Yes.** It shapes stick response, and the packet rate comes from the Input Device. | F gain 120/125/120. Smoothing auto at about 0.375 × packet rate (see [§3.1](#31-radio-link-rate-how-rc-smoothing-and-feedforward-adapt); [pid.h](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.h#L64-L66), [pid.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L198-L202)) |
| 5 | **Gyro and D-term low-pass filters, and the yaw P low-pass** | Smooth the gyro signal. Even on a clean signal they add a few milliseconds of delay, which changes how D damps a move and how propwash feels. | **Yes, for latency.** Without them a pilot's real gains would behave differently. | Gyro: PT1 dynamic 250–500 Hz, then PT1 500 Hz. D-term: PT1 dynamic 75–150 Hz, then PT1 150 Hz. Yaw P: 100 Hz ([gyro.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro.c#L121-L150), [pid.h](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.h#L69-L71)) |
| 6 | **Throttle path**: throttle curve (mid, expo, hover), throttle limit, throttle boost | Shapes the throttle stick and briefly boosts sharp throttle moves. | Yes. Hover feel and punch-outs. | Linear curve. Limit off. Boost 5 ([rc.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L829-L873), [mixer.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L508-L521)) |
| 7 | **Anti-gravity** | Briefly strengthens I (and a little P) on fast throttle changes, so the nose doesn't dip on a punch. | Yes, but only if our physics produces the real attitude upset on throttle changes. | Gain 80 (= 8.0), 5 Hz, P boost 100 ([pid.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L465-L485)) |
| 8 | **I-term relax** | Stops I building up during fast stick moves, which prevents bounce-back at the end of flips and rolls. | Yes. Flip and roll endings. | Roll and pitch, setpoint mode, cutoff 15 ([pid.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L846-L879)) |
| 9 | **TPA** (throttle PID attenuation), including low-throttle TPA | Lowers D (and optionally P) at high throttle, where motors are more powerful. | Yes, but only once motor authority rises with throttle in our physics. | D only, 65 % above 1350 µs. Low TPA 20 % below 1050 µs ([pid.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L407-L462)) |
| 10 | **Dynamic D (D max)** | Runs a lower D in calm flight and boosts it on fast moves. | Mostly yes. Its gyro-driven half reacts to noise. | Base D 30/34, max 40/46. Gain 0, advance 35 in source (see [§6.3](#63-documentation-disagrees-with-the-source)) |
| 11 | **Battery-related**: vbat sag compensation, thrust linearisation, motor output limit | Evens out power as the battery sags, and linearises thrust. | Only if we simulate battery sag and nonlinear thrust (physics ticket [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10)). | All off or 100 % by default |
| 12 | **Yaw extras**: feedforward yaw hold, I-term rotation, integrated yaw | Small yaw refinements. | Minor. | Yaw hold on (gain 15). The others are off |
| 13 | **Dynamic idle** | Raises idle to keep motors above a minimum RPM. It needs RPM telemetry. | Minor in the alpha. Our physics knows RPM anyway. | Off |
| 14 | **RPM filter, dynamic notch, static notches, D-term notch** | Narrow filters that remove motor-vibration noise at 100–600 Hz. | **No.** They only matter when we simulate gyro noise. | RPM: 3 harmonics, min 100 Hz, Q 5.0, needs bidirectional DShot. Dynamic notch: 3 notches, 100–600 Hz, Q 3.0 ([rpm_filter.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/rpm_filter.c#L32-L39), [dyn_notch.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/dyn_notch.c#L32-L37)) |
| 15 | **DShot wire protocol** | The digital link from the Flight Controller to each motor's ESC (electronic speed control). | **No.** We only need its effect: the idle floor and about 2000 throttle steps. | DShot600, bidirectional off, 14 motor poles ([motor.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/motor.c#L58-L122)) |

**Assist candidates.** These are Betaflight features that work like Assists. They go to the Assist and Preset ticket ([#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13)):

- Angle mode and Horizon mode (self-levelling; angle limit 60°)
- Acro Trainer
- Crash recovery (off by default)
- Yaw spin recovery (auto)
- Launch control
- Crash-flip, also called "turtle mode"
- The EzLanding mixer

The stock Meteor 65 ships with Angle and Horizon on a switch (see [§6.2](#62-meteor-65-factory-settings)), so a self-levelling Assist is something real whoop pilots already know.

**Not needed for Free Flight:** GPS Rescue, position and altitude hold, the autopilot and flight plans, OSD, blackbox recording, VTX control and failsafe stages.

## 2. Rate models

All five models take a stick deflection `x` from −1 to +1, with `a = |x|`, and return a target rate in °/s. The result is then clamped to `rate_limit`, which defaults to 1998 °/s ([rc.c L681](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L681), [rc_controls.h L81](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc_controls.h#L81)). Below, `rc`, `sr` and `ex` are the configured RC rate, super rate and expo, as the Betaflight App shows them (stored as integers in the CLI). Formulas are from [rc.c L190–L268](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L190-L268).

- **Actual** (default since 4.3, per the [Rate Calculator doc](https://www.betaflight.com/docs/wiki/guides/current/Rate-Calculator)):
  - Let `centre = rc·10` (°/s) and `extra = max(0, sr·10 − centre)`.
  - Then `rate = centre·x + extra · a · (e·x⁵ + (1−e)·x)`, where `e = ex/100`.
  - Centre sensitivity and max rate are set directly. Expo only moves the "kink" of the curve.
- **Betaflight**:
  - First `x' = e·x·a³ + (1−e)·x`.
  - Then `rate = 200·r·x' / clamp(1 − a·s, 0.01, 1)`, where `r = rc/100` and `s = sr/100`.
  - Above `r = 2`, `r` grows by an extra 14.54 per unit.
  - The "super rate" term is what makes the stick ends steep.
- **Quick**:
  - The centre rate is `R = 2·rc` (rc stored ×100) and the max is `M = max(sr·10, R)`.
  - With `k = (M/R − 1)/(M/R)`, the rate is `x·R / clamp(1 − k·(e·a³ + (1−e)·a), 0.01, 1)`.
  - A second variant, behind `quick_rates_rc_expo`, applies expo to the signed stick instead.
- **KISS**: `rate = 2000 · (rc/1000) · (c·x³ + (1−c)·x) / clamp(1 − a·s, 0.01, 1)`, where `c = ex/100` and `s = sr/100`. The result is clamped to ±1998.
- **Raceflight**: `rate = 10·rc · (1 + 0.01·ex·(x² − 1))·x · (1 + a·sr/100)`.

**For the spec:** every model is a pure function of three numbers per axis. Each can be checked with a table-driven Scenario such as "full stick right, Actual 70/670/0, gives 670 °/s". Pilots expect the exact numbers they use in the Betaflight App.

## 3. PID loop structure

In rate mode ("acro"), one loop tick does the following per axis ([pid.c L1048–L1476](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L1048-L1476)):

1. **Target.** The target is the smoothed setpoint from the rates (§2). In Angle or Horizon mode, the self-level code replaces it.
2. **Error.** `error = target − filtered gyro`.
3. **P.** `P = 0.032029 · P_gain · error`. TPA scales it only if TPA mode is "PD". On yaw, P passes through a 100 Hz low-pass.
4. **I.** Each tick, `I += (0.244381 · I_gain + anti-gravity boost) · dT · error'`.
   - `error'` is the error after I-term relax.
   - I is clamped to `iterm_windup % × PID-sum limit`. That is ±400 on roll and pitch and ±320 on yaw by default ([pid_init.c L423](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L423)). *This is 2025.12 and later. In 4.3–4.5, `iterm_limit` (400) was the clamp, and `iterm_windup` (85) instead slowed I growth once the mixer range passed 85 %: on yaw only in 4.3, on all axes from 4.4.*
   - On yaw, the I gain is multiplied by 2.5 internally, unless `use_integrated_yaw` is on ([pid_init.c L378–L383](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L378-L383)).
   - Yaw gets no anti-gravity boost.
5. **D.** `D = 0.000529 · D_gain · (−rate of change of the D-filtered gyro)`.
   - Dynamic D then multiplies it by between 1 and D_max ÷ D, and TPA scales it.
   - D acts on the measurement, not the error, so stick moves don't kick it.
   - The loop uses a fixed `dT`, not a measured one ([pid.c L1307–L1384](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L1307-L1384)).
6. **F.** `F = 0.013754 · (F_gain/100) · feedforward`. The feedforward term comes from the speed of the stick, computed in [rc.c L432 onwards](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L432).
7. **Anti-gravity P boost** (roll and pitch only). It fades out above 50 °/s of commanded rotation ([pid.c L1423–L1434](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L1423-L1434)).
8. **Sum.** `sum = P + I + D + F`. The S term is for fixed-wing only.
9. **Mixer input.** The mixer clamps the sum to the PID-sum limit and divides by 1000 ([mixer.c L700–L714](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L700-L714)). So `sum = 100` means "10 % of the motor range".

The scaling constants are in [pid.h L46–L52](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.h#L46-L52), and the gain conversion is in [pid_init.c L373](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L373).

**Gains don't depend on loop rate.** I is multiplied by `dT` and D is divided by it, so Betaflight gains mean the same at 4 kHz and 8 kHz. Only the filter discretisation changes. Our Flight Controller can therefore run at any fixed rate of a few kHz and still accept a pilot's real numbers. Real F4, F7 and H7 boards default to an 8 kHz gyro and PID loop ([STM32 platform.h L409–L411](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/STM32/include/platform/platform.h#L409-L411)).

**The details below change feel in specific moves:**

- **I-term relax** ([pid.c L846–L879](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L846-L879)): a 15 Hz low-pass of the setpoint is subtracted from the setpoint, giving a high-pass. I growth is scaled by `max(0, 1 − |high-pass| / 40 °/s)`. The 40 °/s threshold drops to 8 °/s in Angle mode.
- **Anti-gravity** ([pid.c L465–L485](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L465-L485)): it takes the throttle's rate of change, weights it towards low throttle, and smooths it with a 5 Hz PT2 filter. That value boosts I by ×0.34 per unit of gain, and P by a smaller amount.
- **TPA** ([pid.c L407–L422](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L407-L422), [pid_init.c L534–L540](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L534-L540)):
  - The factor falls linearly from 1 at the breakpoint (1350 µs, or 35 % throttle) to `1 − rate` at full throttle.
  - Low TPA is separate. It cuts D by up to 20 % below 1050 µs, until throttle first passes that point after power-up. The latch resets only at power-up, not when re-arming. Low TPA arrived in 4.5.
  - 2026.x also adds a "hyperbolic" TPA curve and speed-based TPA, mainly for wings. Both are off for Quads.
- **Dynamic D** ([pid.c L1347–L1371](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L1347-L1371)): D is multiplied by up to `D_max/D`. The boost is driven by the larger of two signals: gyro acceleration (scaled by `d_max_gain`) or stick speed (scaled by `d_max_advance`).
- **Naming changed in 2025.12** ([Dynamic-D doc](https://www.betaflight.com/docs/wiki/guides/current/Dynamic-D)):
  - In 4.3–4.5, `d_roll` was the *peak* and `d_min_roll` the *base*.
  - From 2025.12, `d_roll` is the *base* and `d_max_roll` the *peak*.
  - A settings importer must map old diffs.

### 3.1 Radio link rate: how RC smoothing and feedforward adapt

Real Betaflight measures how often radio frames arrive. It then tunes RC smoothing and feedforward to that rate. This is part of the Flight Controller, not an Assist, so we replicate it exactly.

*Version scope: this section describes 2025.12 and later. In 4.3 and 4.4 the link rate was "trained" once: Betaflight waited about 6 s after boot, averaged 50 frame gaps, and applied no RC smoothing before training finished. Feedforward was also a different algorithm in those versions, with smoothing default 25, averaging off and a quadratic jitter formula. See [ADR 0008](../adr/0008-copy-betaflight-2026-6-translate-older-tunes.md) and [#21](https://github.com/BartoszSolkaBD/OpenDrone/issues/21).*

The input research ([#3](https://github.com/BartoszSolkaBD/OpenDrone/issues/3)) found that our Input Devices deliver timestamped stick samples at very different rates:

- Radiomaster Pocket over USB: about 1 kHz
- DualSense over USB: 250 Hz
- DualSense over Bluetooth: about 800–1000 Hz, with 8-bit sticks

So how Betaflight behaves at each rate decides how each Input Device feels.

**How Betaflight measures the rate** ([rc.c L286–L327](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L286-L327), [L564–L619](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L564-L619)):

- **Gap between frames.** It uses the receiver's own frame timestamp where the protocol provides one, otherwise the arrival time.
- **Valid range.** The gap is clamped to 0.8–65.5 ms, which is about 15–1250 Hz. A gap outside that range is used clamped, but marked "not valid" for rate tracking. (The 0.8 ms floor is 2025.12 and later; 4.3–4.5 used 0.95 ms.)
- **A slowly smoothed estimate** starts at 100 Hz:
  - Each valid frame within ±20 % of the estimate moves it 10 % of the way.
  - Three outliers in a row in the same direction snap the estimate to the new rate, because the link rate changed.
  - Outliers that alternate direction are ignored as jitter, which is common at 1 kHz.
- **Retuning.** The filters are retuned after every 3 valid frames.
- **Signal loss.** With no frames for 150 ms the link counts as lost: RXLOSS is flagged and arming is blocked ([rx.c L136–L137](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/rx/rx.c#L136-L137)). The sticks keep their last values until about 0.3 s, when the stage-1 values apply: roll, pitch and yaw centred, throttle low, AUX held. Stage 2 (by default DROP, which disarms) starts at `failsafe_delay`, 1.5 s ([failsafe.c L224–L233](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/failsafe.c#L224-L233)). The rate estimate is frozen while the signal is lost. 4.4 flagged the loss at 100 ms.

**What tunes itself to that rate:**

| Part | Behaviour | At 250 Hz | At 1 kHz |
|---|---|---|---|
| **RC smoothing** of setpoint and throttle ([rc.c L346–L425](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L346-L425)) | A 3rd-order (PT3) low-pass that runs every PID loop. It turns the steps between frames into a smooth curve. Cutoff = `max(15 Hz, rate × 1.5 / (1 + factor/10))`. With the default factor 30 that is `rate × 0.375`. | about 94 Hz | about 375 Hz |
| **Feedforward smoothing** ([pid_init.c L509–L524](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L509-L524), [rc.c L386–L395](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L386-L395)) | Two PT1 stages on stick speed and stick acceleration. The time constant is normalised to a 250 Hz link, about 7.4 ms with smooth factor 65. The filters are retuned to the tracked link rate every 3 valid frames, not to each frame's own gap, so the delay stays about the same at any rate. The final feedforward also passes through the setpoint PT3. | 7.4 ms | 7.4 ms |
| **Stick speed** ([rc.c L432–L561](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L432-L561)) | Computed once per *new* frame as the change in setpoint × the measured frame rate. Between frames feedforward is held, and RC smoothing interpolates. | per frame | per frame |
| **Duplicate frames** | Interpolation is on for every receiver type except CRSF ([pid_init.c L524](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid_init.c#L524)). The check reads `serialrx_provider` whatever the receiver is, so a built-in SPI ELRS receiver left at the default provider (CRSF) also runs with interpolation off. If one axis repeats its last value, the first repeat is extrapolated from the previous speed. Later repeats force speed to zero, so feedforward decays instead of stepping. With interpolation off, a repeat simply gives zero speed. | | |
| **Jitter attenuation** | Feedforward is scaled by `min(1, (average abs(Δstick) over the last 2 frames + 1) / (1 + jitter_factor))`, with Δ in µs. At the default jitter factor 7, full feedforward needs about 7 µs of stick change per frame. | | |
| **Averaging and boost** | 2-point moving average. Boost adds stick acceleration × rate × 0.015. | | |

**What this means for each Input Device.** The figures below are our own arithmetic from the formulas above.

- **The same thumb movement gives *less* feedforward on a faster link.** A stick moving 1000 µs per second changes 4 µs per frame at 250 Hz, so jitter attenuation is about 0.63. At 1 kHz it changes only 1 µs per frame, so attenuation is about 0.25. That is real Betaflight behaviour, and it is one reason pilots retune jitter and smoothing per radio link.
- **8-bit sticks move in steps of about 3.9 µs** (1000 µs ÷ 255). At 800–1000 Hz, slow moves arrive as one step followed by several identical samples. The duplicate logic then extrapolates once and drops to zero, so feedforward pulses on each step instead of flowing smoothly.
- **Bursty Bluetooth timing matters too.** Gaps shorter than 0.8 ms are clamped and marked invalid, and alternating outliers are ignored for rate tracking. Feedforward still uses each frame's measured gap, so uneven gaps add feedforward noise.

**Recommendation.** We should reuse the same logic as written: rate measurement from sample timestamps, the auto-cutoffs, duplicate handling, jitter attenuation and the 150 ms signal-loss rule. Then we decide in the input ticket what "frames" the Flight Controller receives. There are two options:

1. **Pass every device sample straight through.** This is faithful to the device, but a DualSense at 1 kHz with 8-bit sticks will feel different from the pilot's real 250 or 500 Hz radio link.
2. **Resample to a set "radio link rate".** For example, emulate an ELRS link at 250 or 500 Hz, chosen to match the pilot's real setup, and then run Betaflight's logic unchanged.

Option 2 is what reproduces the feel of the pilot's real Quad. It isn't an Assist: it models the radio link, and the same Flight Controller code runs either way.

## 4. Mixer, Airmode, motor idle, DShot and throttle

The mixer runs in this order ([mixer.c L676–L845](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L676-L845)):

1. **Throttle** is scaled to 0–1. Throttle limit applies (Off, Scale or Clip, at `throttle_limit_percent`).
2. **Anti-gravity and TPA** both read that throttle.
3. **Throttle boost** adds a high-passed copy of throttle: `boost × (throttle − 15 Hz low-pass of throttle)`, with boost 5 meaning ×0.5.
4. **Thrust linearisation**, if enabled, pre-compensates throttle.
5. **Each motor's mix** is `roll·m_r + pitch·m_p + yaw·m_y`, using the PID sums ÷ 1000.
6. **Airmode logic** (`mixer_type`, Legacy by default; [mixer.c L655–L674](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L655-L674)):
   - If the mix spans more than the full range, all motors are scaled down to fit.
   - Throttle is then shifted up or down so that no motor clips. This is why a Quad can still flip at zero throttle.
   - Without Airmode, authority below half throttle is faded to between 50 % and 100 %. This fade exists only from 4.4 onwards.
   - Linear and Dynamic mixer types also exist ([Mixer doc](https://www.betaflight.com/docs/wiki/guides/current/Mixer)).
7. **Output.** `motor = idle + (1 − idle) × (mix + throttle)` ([mixer.c L455–L506](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L455-L506)).
   - With DShot, idle is `48 + idle% × 1999` on a 48–2047 scale ([dshot.c L62–L70](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/drivers/dshot.c#L62-L70)).
   - The default idle is 5.5 % ([motor.c L83](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/motor.c#L83)).

**Airmode is on by default.** The feature is in the default feature set ([feature.c L34](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/config/feature.c#L34)).

- After arming, it engages once throttle first passes 25 % ([core.c L841–L866](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L841-L866), [rx.c L105](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/rx.c#L105)).
- Until then, I-term is held at zero, but only while the throttle stick is below `min_check` (1050). Between `min_check` and 25 % throttle, I-term already runs. This gives a "soft" feel on the ground before takeoff, which a Scenario can check. The same is true in 4.3 and 4.4.

**The throttle curve changed in 2025.12** ([2025.12.1 rc.c L775–L830](https://github.com/betaflight/betaflight/blob/2025.12.1/src/main/fc/rc.c#L775-L830), [2026.6.2 rc.c L829–L873](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L829-L873)):

- It is now two quadratic Bézier segments through a "hover point" (`thr_mid`, `thr_hover`), bent by `thr_expo`. `thr_mid` now means "the stick position where the curve reaches `thr_hover`".
- The defaults (50/50/0) give a straight line.
- Older versions (4.3–4.5) used a different curve. The defaults are linear in both, so this only matters for imported tunes with non-zero expo.

**DShot itself doesn't need simulating.** What matters for feel is:

1. the idle floor
2. 2000-step command resolution, which is negligible
3. the motor and ESC response time, which belongs to the Quad's physics, not the Flight Controller

Bidirectional DShot only feeds RPM to the RPM filter and dynamic idle. Our physics knows every motor's RPM exactly.

### 4.1 Crash flip, crash detection and yaw spin recovery (2026.6.2)

Added for [#26](https://github.com/BartoszSolkaBD/OpenDrone/issues/26), which put Crash Flip and yaw spin recovery into the alpha and left automatic disarm out ([ADR-0012](../adr/0012-crashes-behave-like-a-real-quad.md)). Checked against the source on 2026-10-04.

**Crash flip** (mode `BOXCRASHFLIP`, permanent id 35, "FLIP OVER AFTER CRASH"):

- **Where the code lives:**
  - Entry and exit: [core.c L294–L313](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L294-L313) and [L597–L610](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L597-L610).
  - Motor output: [mixer.c L278–L401](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L278-L401).
  - Defaults: [mixer_init.c L62–L69](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer_init.c#L62-L69).
- **Entering:**
  - The switch is latched only at the moment of arming, and only with DShot.
  - Turning it on while armed does nothing.
  - Arming with the switch on sends DShot "spin direction reversed" (command 21) to all motors. Every other arm sends "normal" (20). The commands are sent 10 times, 1 ms apart, after a 10 ms wait, once the motors are idle.
- **Arming checks:** with the switch on, the `small_angle` check and runaway takeoff detection are both skipped ([core.c L389–L393](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L389-L393), [L879](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L879)).
- **Motor output:**
  - It's open loop: the PID output is thrown away ([mixer.c L684–L689](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/mixer.c#L684-L689)), and throttle is ignored.
  - Pitch forward drives the front pair, pitch back the rear pair, and roll right the right pair.
  - A stick within 30° of a diagonal drives one motor. Yaw, when it dominates, drives a diagonal pair.
  - There's a 15 % stick deadband, and power is linear in the stick.
  - Motors whose mix would be negative get `crashflip_motor_percent` of it (default 0, so off). Any motor below 2 % is stopped.
- **Leaving:**
  - With `crashflip_auto_rearm` OFF, the default, turning the switch off while armed disarms (reason `CRASHFLIP`). Arming stays blocked ("FLIP_SWITCH") until the pilot disarms with the Arm switch.
  - With it ON, the Quad stays armed and flies normally. Nothing checks that the flip succeeded.
- **`crashflip_rate`** (default 0, which is off): above it, power fades with rotation rate and with how far the Quad has turned since the mode started.
- **What changed since 4.3 and 4.4,** which are identical:
  - Those had `crashflip_expo` (default 35), which 2026.6 removed.
  - Turning the switch off while armed did nothing until disarm.
  - The changes came in PRs #13905, #14410, #14734, #14777 and #14803.

**Crash detection** (`crash_recovery`: OFF, ON, BEEP or DISARM; default OFF):

- **It detects a crash when** all of these hold on one axis ([pid.c L691–L721](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L691-L721)):
  - the mixer is saturated
  - the gyro's rate of change (through the D-term filter) is above `crash_dthreshold`
  - the rate error is above `crash_gthreshold`
  - the setpoint is below `crash_setpoint_threshold`
- **DISARM** disarms at once and blocks arming ("CRASH") until the Arm switch is cycled.
- **Disarm on impact:** `landing_disarm_threshold` (default 0, which is off) disarms on an accelerometer jerk with the sticks and throttle low ([pid.c L881–L908](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L881-L908)).
- **The docs disagree with the source in places.** For example, the docs say DISARM means "recover then disarm", but the source disarms at once.

**Yaw spin recovery** (`yaw_spin_recovery` AUTO by default, `yaw_spin_threshold` 1950):

- **The AUTO threshold** is the max yaw rate + max(25 %, 200 °/s), clamped to 500–1950 °/s ([gyro.c L714–L738](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro.c#L714-L738)).
- **While active:**
  - The yaw setpoint is 0, I is zeroed on all axes, and roll and pitch P, D and F are zeroed.
  - The yaw PID-sum limit rises to 1000.
  - Throttle is forced to 50 %, but only without Airmode.
- **It ends** after yaw stays 100 °/s below the threshold for 20 ms ([gyro.c L355–L392](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro.c#L355-L392)).
- **Gyro range:** Betaflight sets the BMI270, ICM-42688-P and MPU6000 to ±2000 °/s, so readings can't go beyond that.

**An armed Quad on the ground with Airmode active** keeps its PIDs running at zero throttle. I-term winds up to its static clamp of 400 on roll and pitch, and 320 on yaw ([core.c L842–L867](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L842-L867)).

**ESC side (Bluejay):**
- A stopped motor waits 100 ms before it starts.
- A stall waits 100 ms and retries. After 3 failed starts the ESC stops until it sees zero throttle.
- Start-up power is limited.

## 5. Filtering: what matters without simulated noise

The gyro path, in order ([gyro_filter_impl.c L52–L80](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro_filter_impl.c#L52-L80)):

1. RPM notch filters
2. static notches 1 and 2 (off by default)
3. low-pass 1 (dynamic PT1, 250–500 Hz with throttle)
4. low-pass 2 (PT1, 500 Hz)
5. the dynamic notch

The D-term then has its own two low-passes, dynamic PT1 75–150 Hz and PT1 150 Hz ([pid.c L1130–L1132](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L1130-L1132)).

| Filter | Without simulated noise | With simulated noise |
|---|---|---|
| Gyro low-pass 1 and 2 | **Replicate.** Each adds delay. A first-order (PT1) low-pass delays slow signals by about `1/(2π·cutoff)`: about 0.6 ms at 250 Hz and 0.3 ms at 500 Hz. *These figures are our own arithmetic, not from a source.* | Replicate. |
| D-term low-pass 1 and 2 | **Replicate.** About 2.1 ms at 75 Hz plus 1.1 ms at 150 Hz on the D path (our arithmetic). That changes how D damps propwash, so real D gains only transfer with these in place. | Replicate. |
| Yaw P low-pass (100 Hz) | Replicate, as it is cheap. | Replicate. |
| RC smoothing (PT3) and feedforward smoothing | **Replicate.** This isn't sensor noise. It smooths the stick signal and auto-tunes to the radio packet rate ([rc.c L346–L425](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L346-L425)). | Replicate. |
| RPM filter | Skip. With no motor noise it has almost nothing to remove. It also needs bidirectional DShot to be active at all ([rpm_filter.c L75](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/rpm_filter.c#L75)). | Replicate, driven by true motor RPM from physics. |
| Dynamic notch | Skip. It tracks peaks in the 100–600 Hz noise spectrum. | Replicate if motor or frame noise is simulated. |
| Static gyro notches, D-term notch | Skip (off by default). | Only if a Preset uses them. |
| Dynamic idle | Optional (off by default). | Same. |

**The point to remember:** the low-pass filters belong in the alpha because they are *latency*, not noise removal. The notch filters wait until we choose to simulate gyro noise; that is a physics question for [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10). Betaflight's own guide says filtering costs latency, and that gyro low-pass delay is "the biggest drain on phase delay that makes propwash response poor" ([Freestyle Tuning Principles](https://www.betaflight.com/docs/wiki/guides/current/Freestyle-Tuning-Principles)).

## 6. Defaults: Meteor 65 and a typical 5"

### 6.1 Betaflight 2026.6.2 firmware defaults (the 5" reference)

Betaflight's defaults "originally were made to suit 5" freestyle machines" ([4.3 Tuning Notes](https://www.betaflight.com/docs/wiki/tuning/4-3-Tuning-Notes)). Its tuning guide gives suggested 5" values for "6S with 1600 to 1800 KV motors or 4S with 2400-2600 KV motors" ([Freestyle Tuning Principles](https://www.betaflight.com/docs/wiki/guides/current/Freestyle-Tuning-Principles)).

| Setting | Firmware default | Freestyle-guide suggestion for 5" |
|---|---|---|
| Rates | Actual 70 / 670 / 0 | Pilot's choice. The guide says freestyle uses 850–1200 °/s max ([Rate Calculator](https://www.betaflight.com/docs/wiki/guides/current/Rate-Calculator)) |
| P / I / D roll | 45 / 80 / 30 (D max 40) | P 60–70, I 90–100, D 40–50 |
| P / I / D pitch | 47 / 84 / 34 (D max 46) | same as roll |
| P / I / D yaw | 45 / 80 / 0 | P 30–40, I 90–100, D 0 |
| F (roll / pitch / yaw) | 120 / 125 / 120 | 90–100 |
| Feedforward | averaging 2-point, smooth 65, jitter 7, boost 15, max-rate limit 90, transition 0 | transition 0.9–1 |
| I-term relax | RP, setpoint, cutoff 15 | RP increment-only, setpoint, cutoff 7–12 |
| Anti-gravity | 80 (= 8.0) | 3.5–5 |
| TPA | D only, 65 % at 1350 µs | 40–50 % at 1600–1750 µs |
| Dynamic D | on (D max above base) | disabled or conservative |
| Motor idle | 5.5 % | 3–4 % |
| Thrust linear / vbat sag comp. | 0 / 0 | 20–25 / 40–70 |
| Motor protocol | DShot600, bidirectional off, 14 poles | bidirectional on for RPM filter |
| Gyro / D-term filters | as in §5 | lighter, if the build is clean |
| Loop rate | 8 kHz gyro and PID (F4/F7/H7) | 8k/8k with DShot600 ([RPM filtering doc](https://www.betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering)) |

Sources: [pid.c L125–L262](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L125-L262), [pid.h](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.h#L37-L71), [controlrate_profile.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/controlrate_profile.c#L42-L67), [motor.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/motor.c#L58-L122), [gyro.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro.c#L121-L150).

**Suggestion:** the generic 5" Quad's default Preset should use the firmware defaults. That is what a fresh real build flies with. A "tuned freestyle" Preset could then take the guide's suggestions.

### 6.2 Meteor 65 factory settings

BetaFPV publishes a factory CLI `diff all` for each Meteor 65 version. The support site is behind a bot check, so I read the [Wayback Machine snapshot of 2025-10-11](https://web.archive.org/web/20251011135107/https://support.betafpv.com/hc/en-us/articles/900004021966-CLI-for-Meteor65-2022) and its attachments:

- [Meteor65 ELRS](https://web.archive.org/web/20251011135107/https://support.betafpv.com/hc/article_attachments/15323971194393)
- [Meteor65 Pro ELRS](https://web.archive.org/web/20251011141221/https://support.betafpv.com/hc/article_attachments/15324681477785), from the [Meteor65 Pro page](https://web.archive.org/web/20251011141221/https://support.betafpv.com/hc/en-us/articles/4409155091353-CLI-for-Meteor65-Pro-2022)

Both run **Betaflight 4.3.0** on an F411 board (`BETAFPVF411`) with BlueJay ESC firmware. The table translates 4.3's names into 2026 names.

| Setting | Meteor65 (2022) | Meteor65 Pro (2022) | Note |
|---|---|---|---|
| Rates | Not in diff, so 4.3 default: Actual 70 / 670 / 0 | same | Most pilots change rates |
| Roll P / I / D (base → max) / F | 49 / 61 / 39 → 39 / 151 | 40 / 64 / 45 → 48 / 125 | 4.3: `d_min_roll` is base, `d_roll` is max |
| Pitch P / I / D / F | 51 / 64 / 44 → 44 / 158 | 39 / 64 / 49 → 52 / 124 | |
| Yaw P / I / F | 49 / 61 / 151 | 35 / 50 / 100 | |
| TPA rate | 60 | 70 | breakpoint at the default |
| Gyro low-pass | dynamic 287–575 Hz, LPF2 575 Hz (multiplier 115) | same | lighter than the 5" default |
| D-term low-pass | dynamic 82–165 Hz, LPF2 165 Hz | same | |
| Dynamic notch | 1 notch, Q 5.0 | 2 notches, Q 3.5, min 130 Hz | |
| Motors | DShot300, bidirectional **on** (so RPM filter on), 12 poles, idle **6.0 %** | same | |
| PID loop | `pid_process_denom = 1` (PID at gyro rate) | same | |
| Modes (aux switches) | ARM on AUX1 high. **Angle on AUX2 low**, Horizon mid. Crash-flip on AUX3 | ARM on AUX1. Angle on AUX2 high, Horizon mid | Stock whoops ship with self-level on a switch |

**The maintainer's own `diff all` beats this table.** Firmware may have been updated since the factory flash. Running `diff all` in the Betaflight App's CLI tab and pasting it into a ticket takes a minute, and it gives the true rates, tune and Betaflight version for both the Meteor 65 and the Cetus X.

### 6.3 Documentation disagrees with the source

The three sources give three different Dynamic D defaults:

| Source | `d_max_gain` | `d_max_advance` |
|---|---|---|
| [pid.c L186–L189](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/pid.c#L186-L189) (the code at tag 2026.6.2) | 0 | 35 |
| [Dynamic-D guide](https://www.betaflight.com/docs/wiki/guides/current/Dynamic-D) | 37 | 20 |
| [2026.6 release notes](https://www.betaflight.com/docs/wiki/release/Betaflight-2026-6-Release-Notes) | (not stated) | "now defaults to 0" |

Treat the source as the truth for 2026.6.2. Don't copy defaults from the docs.

The guide's 37/20 matches the source of 4.3, 4.4 and 2025.12; the guide is simply older than 2026.6. Also, `d_max_advance` changed meaning in 2025.12. Before that, the stick-driven boost was `d_max_gain × d_max_advance / 100` (7.4 at the defaults). Since then, `d_max_advance` is used on its own.

## 7. SITL Betaflight: builds, interface and timing

Betaflight ships a **SITL** ("software in the loop") target. It compiles the real firmware into a desktop program with virtual sensors ([SITL docs](https://www.betaflight.com/docs/development/SITL), [target README](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/target/SITL/README.md)).

### 7.1 Does it build?

| OS | Status | Evidence |
|---|---|---|
| **Linux** | Yes. This is the main platform, and the SITL target is part of Betaflight's own CI builds. | `SITL_X_PLANE` is in `CI_COMMON_TARGETS` and `SITL` in `PREVIEW_TARGETS` ([Makefile L186–L188](https://github.com/betaflight/betaflight/blob/2026.6.2/Makefile#L186-L188)). CI runs on Ubuntu only ([ci.yml](https://github.com/betaflight/betaflight/blob/2026.6.2/.github/workflows/ci.yml)). |
| **macOS** | **Yes. Verified here**, but not tested in Betaflight's CI. | Apple-silicon fixes were merged in 2025–26: [#14284](https://github.com/betaflight/betaflight/pull/14284) and [#14934](https://github.com/betaflight/betaflight/pull/14934). The makefile has a macOS branch ([SITL.mk L67–L82](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/mk/SITL.mk#L67-L82)). See the experiment below. |
| **Windows** | **No native build.** WSL2 only. | The official docs say "you need Windows 10 or Windows 11 with WSL" ([SITL docs](https://www.betaflight.com/docs/development/SITL)). The SITL code uses POSIX-only APIs (`pthread`, `arpa/inet.h`, `clock_gettime`) with no Windows code path ([sitl.c](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L586-L632), [udplink.h L15](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/udplink.h#L15)). |
| **iOS / Android** | Not practical. | It is a desktop POSIX program, and the licence issues in §8.3 apply. |

**Experiment on the dev machine** (Mac mini M4, Darwin 25.6, Apple clang 17):

- Source: the 2026.6.2 source tarball, plus the `config` and `libcanard` submodules at their pinned commits.
- **Build:** `make TARGET=SITL` built in about 5 s. (Submodule auto-fetch was skipped with `AUTOHYDRATE_STAMPS=` because the tarball isn't a git checkout.) The output is a native arm64 executable.
- **Run:** it started, created `eeprom.bin`, and opened its UDP ports. It used about 4 % of one CPU core while idle.
- **Timing probe:** a small Python script sent fake simulator state packets at a steady rate and counted the motor packets that came back over 2 s:

| State packets sent per second | Motor packets returned (over 2 s) |
|---|---|
| 250 | 490 (≈245/s) |
| 1000 | 1933 (≈967/s) |
| 4000 | 1972 (≈986/s) |

Motor output follows the simulator's rate up to about **1 kHz, then stops rising**. That matches the 1 kHz virtual gyro (§7.3).

### 7.2 Interface

SITL is a separate program. It talks to a simulator over local UDP and to the Betaflight App over TCP ([sitl.c L201–L204](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L201-L204), [target.h L233–L255](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/target/SITL/target.h#L233-L255)):

| Direction | Port | Contents |
|---|---|---|
| Simulator → SITL | UDP 9003 | Flight state: timestamp (s), body angular velocity (rad/s), linear acceleration (m/s²), attitude quaternion, velocity, position, pressure. All are 64-bit floats. |
| Radio → SITL | UDP 9004 | Timestamp plus 16 RC channels in µs (1000–2000) |
| SITL → simulator | UDP 9002 | 4 motor commands, 0.0–1.0 (or −1 to 1 in 3D mode) |
| SITL → RealFlight bridge | UDP 9001 | Raw PWM for up to 16 outputs |
| Betaflight App / MSP / CLI | TCP 5761 (UART1) | Same protocol as a real board. The web App needs a websocket proxy. |

Settings persist in `eeprom.bin`. Since 2026.6, a CLI text file can be loaded with `--config <file>` ([PR #14935](https://github.com/betaflight/betaflight/pull/14935)).

Betaflight now uses this interface in its own end-to-end tests ([sitl_harness.py](https://github.com/betaflight/betaflight/blob/2026.6.2/src/test/sitl/sitl_harness.py)). It is also the documented interface for Gazebo and RealFlight. In other words, it is a generic simulator interface, not internal plumbing, which matters for §8.2.

### 7.3 Timing: why SITL can't be our lock-step Flight Controller

- **It runs on the wall clock.** Betaflight's scheduler reads real time. That time is scaled by a `simRate` estimated from the gap between incoming state-packet timestamps ([sitl.c L470–L475](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L470-L475), [L694–L703](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L694-L703)). Our physics can't step it one tick at a time.
- **A partial sync option exists but is off.** A compile-time option, `ENABLE_SIMULATOR_GYROPID_SYNC`, gates the PID loop on packet arrival, but it is commented out by default ([target.h L60–L65](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/target/SITL/target.h#L60-L65), [core.c L1500](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L1500)). Even with it, time-based code such as RC smoothing and feedforward still reads the scaled wall clock.
- **The virtual gyro is fixed at 1 kHz** ([accgyro_virtual.h L27](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/drivers/accgyro/accgyro_virtual.h#L27), [gyro_init.c L303](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/sensors/gyro_init.c#L303)). The PID loop therefore runs at 1 kHz, not 8 kHz. Filters near or above 500 Hz behave differently from hardware.
- **It answers at most one motor packet per state packet**, and drops extras under load ([sitl.c L876](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L876)). If no state arrives for 500 ms, it resets its timing ([sitl.c L285](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L285)).
- **Missing features compared with a real Quad:**
  - DShot isn't compiled for SITL ([common_pre.h L84](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/target/common_pre.h#L84)).
  - Motor telemetry is a stub ([sitl.c L897](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L897)).
  - The RPM filter, dynamic notch and dynamic idle are only enabled in hardware platform headers, for example [STM32 platform.h L265–L267](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/STM32/include/platform/platform.h#L265-L267), not in the simulator platform.
- **It falls outside our cross-platform determinism guarantee.** The determinism research ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7)) found our physics can be bit-identical on macOS, Windows and Linux, using `libm`, non-SIMD maths and a fixed tick order. A SITL build breaks that in four ways:
  1. **Unsafe float optimisations.** It compiles with `-Ofast` and `-ffast-math` ([Makefile L240–L242](https://github.com/betaflight/betaflight/blob/2026.6.2/Makefile#L240-L242), [SITL.mk L60–L61](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/mk/SITL.mk#L60-L61)). Our own build of it confirms this. That goes beyond fused multiply-adds: it lets the compiler reorder floating-point maths and assume NaNs never occur. A comment in `sitl.c` notes that fast-math "folds `isnan()` to false".
  2. **The platform maths library.** It calls the platform C maths library (`pow`, `sqrt`, `atan2`) in the SITL glue.
  3. **Threads.** It receives packets on separate threads guarded by mutexes ([sitl.c L586–L632](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L586-L632)).
  4. **The wall clock.** It runs on the wall clock (above).

  Bringing it inside the guarantee would take all of these:
  - a rebuild with `-ffp-contract=off` and without fast-math
  - replacing platform maths calls
  - a lock-step patch that drives Betaflight's clock from our tick

  All of that is a maintained GPL fork.

**Verdict on feasibility:** SITL is practical as a **developer and validation tool** on Linux and macOS. For example:

- a "compare our Flight Controller with real Betaflight" check
- a hidden backend for experienced pilots on desktop

It is **not practical as the shipped Flight Controller**:

- It isn't deterministic, so it can't support Scenarios, replays or future multiplayer ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7), [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11)).
- Its loop rate differs from hardware.
- It has no native Windows build and doesn't fit mobile.
- It forces the separate-process licence setup.

Making it lock-step would mean patching Betaflight, which means keeping a GPL fork.

## 8. Licensing (not legal advice)

Betaflight is GPL-3.0. Its file headers say "either version 3 of the License, or (at your option) any later version" ([sitl.c header](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/SIMULATOR/sitl.c#L1-L19), [LICENSE](https://github.com/betaflight/betaflight/blob/2026.6.2/LICENSE)). OpenDrone is MIT or Apache-2.0. The FSF lists Apache-2.0 as "compatible with version 3 of the GNU GPL" (not with GPLv2), and Expat/MIT as compatible with the GPL ([FSF licence list](https://www.gnu.org/licenses/license-list.html#apache2)).

gnu.org timed out from the dev machine, so the FAQ was read from the [Wayback snapshot of 2026-10-02](https://web.archive.org/web/20261002063053/https://www.gnu.org/licenses/gpl-faq.html). The links below are canonical.

### 8.1 Linking Betaflight into our program: the whole becomes GPL

- The FSF says that if modules "are included in the same executable file, they are definitely combined in one program", and that being "linked together in a shared address space ... almost surely means combining them into one program" ([FAQ: MereAggregation](https://www.gnu.org/licenses/gpl-faq.html#MereAggregation)).
- For a GPL library: "the terms of the GPL apply to the entire combination ... the work as a whole must be licensed under the GPL" ([FAQ: IfLibraryIsGPL](https://www.gnu.org/licenses/gpl-faq.html#IfLibraryIsGPL)).
- Linking doesn't force our *own files* to change licence. They must be GPL-compatible, which MIT and Apache-2.0 are, "The combination itself is then available under those GPL versions" ([FAQ: LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.html#LinkingWithGPL)).

So compiling Betaflight's C code into a library and calling it from Rust (static or dynamic) would make **every OpenDrone binary we distribute a GPL-3.0 program**, with full corresponding source. Our repository could keep MIT/Apache headers, but the app people download would be GPL.

### 8.2 Running SITL as a separate program: our code stays MIT/Apache

- "pipes, sockets and command-line arguments are communication mechanisms normally used between two separate programs. So when they are used for communication, the modules normally are separate programs." The FAQ adds a caveat about communication "intimate enough, exchanging complex internal data structures" ([FAQ: MereAggregation](https://www.gnu.org/licenses/gpl-faq.html#MereAggregation); see also [GPLPlugins](https://www.gnu.org/licenses/gpl-faq.html#GPLPlugins)).
- The SITL interface is a short, documented set of sensor values in and motor commands out over UDP. It is the same one Gazebo and RealFlight use. That is the arm's-length case the FAQ describes ([FAQ: GPLInProprietarySystem](https://www.gnu.org/licenses/gpl-faq.html#GPLInProprietarySystem)).
- **Shipping the SITL binary in our download** is an "aggregate", which GPL-3.0 §5 allows. That copy must follow the GPL: licence text, source or a written offer, and no added restrictions.
- **Not shipping it at all**, with a script that builds SITL from Betaflight's own source on the user's or developer's machine, avoids distributing it.

### 8.3 Using SITL only in development or CI: no duties for us

- GPL-3.0 §2: "You may make, run and propagate covered works that you do not convey, without conditions."
- The FAQ adds that modified versions may be used privately "without ever releasing them" ([FAQ: GPLRequireSourcePostedPublic](https://www.gnu.org/licenses/gpl-faq.html#GPLRequireSourcePostedPublic)). Installing two programs side by side doesn't need their licences to be compatible ([FAQ: WhatIsCompatible](https://www.gnu.org/licenses/gpl-faq.html#WhatIsCompatible)).
- **Mobile caveat:**
  - GPL-3.0 §10 forbids imposing "any further restrictions" on recipients.
  - The FSF has stated that Apple's App Store terms conflict with the GPL ([FSF, 2010, about GPLv2](https://www.fsf.org/news/2010-05-app-store-compliance)).
  - Apple requires apps to be "self-contained in their bundles" ([App Review Guideline 2.5.2](https://developer.apple.com/app-store/review/guidelines/#2.5.2)).
  - So a SITL backend would have to stay desktop-only. *That iOS apps also can't launch a helper program is my inference, not verified against an Apple source.*

### 8.4 Reimplementing Betaflight behaviour in Rust

- GPL-3.0 §0: to "modify" a work "means to copy from or adapt all or part of the work in a fashion requiring copyright permission". The EU Software Directive lists "translation, adaptation or transformation of the form of the code" as an infringing act without permission ([Directive 2009/24/EC, recital 15 and Art. 4](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024)). **A line-by-line port of Betaflight's C into Rust would be a modified version and would have to be GPL.**
- The same Directive says "ideas and principles which underlie any element of a computer program, including those which underlie its interfaces, are not protected", and "to the extent that logic, algorithms and programming languages comprise ideas and principles, those ideas and principles are not protected" (Art. 1(2), recital 11). US law is similar: [17 U.S.C. §102(b)](https://www.law.cornell.edu/uscode/text/17/102). *EUR-Lex was read through a Wayback snapshot.*
- **A safe working practice for us:**
  1. Write our Flight Controller from a behaviour spec in our own words, such as §§2–5 above and the Scenarios.
  2. Don't paste or transliterate Betaflight functions.
  3. Reusing Betaflight's *parameter names and units* (`p_roll`, `rates_type`, `tpa_breakpoint`) for settings import is interface compatibility. It is the kind of thing the "interfaces" wording covers.
  4. Credit Betaflight as the reference in our docs.
  5. Formulas and constants such as `0.032029` are facts we need for compatibility, not creative expression. Still, write them in our own structure.
- The [betaflight.com docs repository](https://github.com/betaflight/betaflight.com) has no licence file. Quote it briefly with a link, and don't copy pages into our docs.

## 9. Recommendations

1. **Write our own Rust Flight Controller targeting Betaflight 2026.6 behaviour.** Use the feature order in §1. The alpha needs features 1–10. Features 11–13 can follow, and 14–15 wait for a noise model.
2. **Make the Flight Controller a pure, fixed-rate step function.**
   - Input: gyro and accelerometer reading, attitude, RC channels and time step.
   - Output: four normalised motor commands from 0 to 1, plus a debug record of P, I, D, F and setpoint.
   - Run it at 1–8 kHz, locked to the physics step.
   - It follows the same determinism rules as physics ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7)): `libm`, no fused multiply-adds and a fixed order.
   - This is the same data SITL exchanges, so a desktop-only `SitlFlightController` adapter stays possible later without locking us in. This is input for the crate split, [#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12).
   - RC frames arrive with their own timestamps, separate from the physics tick, so the link-rate logic in §3.1 works.
3. **Use Betaflight parameter names and units in Quad and Preset data.** Write an importer for a pilot's `diff all` that maps old names, such as the 4.3–4.5 `d_min` and `d_roll` split, and takes final PID numbers rather than slider positions. This is input for [#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13) and [#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16).
4. **Write every rate model and mixer rule as table-driven Scenarios** with numbers a pilot can check against the Betaflight App's rate preview.
5. **Validate against public Blackbox logs, without needing SITL.**
   - Betaflight Blackbox logs record setpoint, gyro, `rcCommand`, each PID term (`axisP/I/D/F`) and motor outputs ([blackbox.c L199–L227](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/blackbox/blackbox.c#L199-L227)). The log header also contains the settings.
   - Feeding a log's inputs into our Flight Controller and comparing outputs tests it open-loop.
   - This is input for [#6](https://github.com/BartoszSolkaBD/OpenDrone/issues/6).
6. **Replicate Betaflight's link-rate handling exactly (§3.1)**: RC smoothing auto-cutoff, feedforward smoothing, duplicate-frame handling, jitter attenuation and the 150 ms signal-loss rule. Then let the input ticket decide between passing samples straight through and an emulated radio-link rate.
7. **Optional and later: SITL as a Linux and macOS CI oracle**, built from source and never shipped.
   - Compare it within tolerances, not bit-for-bit, because of fast-math and wall-clock timing.
   - It needs a prototype to see whether the comparisons are stable enough.

## 10. Open questions

- Which Meteor 65 version does the maintainer own (2022, Pro 2022 or Pro O4 2025), on which Betaflight version, and what does his own `diff all` say? The same for the Cetus X.
- Should the Flight Controller see raw Input Device samples, or frames resampled to an emulated radio-link rate such as ELRS 250 or 500 Hz? RC smoothing and feedforward auto-tune to the rate, and 8-bit DualSense sticks at about 1 kHz will feel different from a 500 Hz radio link (§3.1, [#3](https://github.com/BartoszSolkaBD/OpenDrone/issues/3)). If we resample, should the rate default to the pilot's real link?
- What happens when an Input Device stalls or disconnects? Betaflight starts failsafe after 150 ms without frames. Copying that is faithful, but it needs a rule for what the sim does next, such as drop, disarm or pause.
- What Flight Controller loop rate fits the frame budget alongside physics? 8 kHz matches hardware, and 2–4 kHz is probably indistinguishable without noise. This needs measuring.
- Should a "Betaflight version" setting exist (4.3–4.5 vs 2026.x semantics)? Thrust linearisation, the throttle curve and the Dynamic D names all changed between them.

## Sources

**Betaflight firmware** at [tag 2026.6.2](https://github.com/betaflight/betaflight/tree/2026.6.2):

- `src/main/fc/rc.c`, `controlrate_profile.c`, `rc_controls.h`, `core.c`
- `src/main/flight/pid.c`, `pid.h`, `pid_init.c`, `mixer.c`, `rpm_filter.c`
- `src/main/sensors/gyro.c`, `gyro_filter_impl.c`, `gyro_init.c`
- `src/main/pg/motor.c`, `rpm_filter.c`, `dyn_notch.c`, `rx.c`
- `src/main/drivers/dshot.c`, `src/main/config/feature.c`, `src/main/blackbox/blackbox.c`, `src/main/rx/rx.c`
- `src/platform/SIMULATOR/*`, `src/test/sitl/sitl_harness.py`, `Makefile`, `LICENSE`
- GitHub PRs [#14284](https://github.com/betaflight/betaflight/pull/14284), [#14934](https://github.com/betaflight/betaflight/pull/14934) and [#14935](https://github.com/betaflight/betaflight/pull/14935)

**Betaflight docs** ([betaflight.com](https://www.betaflight.com), repo commit `57c42a8`):

- [SITL](https://www.betaflight.com/docs/development/SITL)
- [Rate Calculator](https://www.betaflight.com/docs/wiki/guides/current/Rate-Calculator)
- [Freestyle Tuning Principles](https://www.betaflight.com/docs/wiki/guides/current/Freestyle-Tuning-Principles)
- [Dynamic D](https://www.betaflight.com/docs/wiki/guides/current/Dynamic-D)
- [Mixer](https://www.betaflight.com/docs/wiki/guides/current/Mixer)
- [Modes](https://www.betaflight.com/docs/wiki/guides/current/Modes)
- [DShot RPM Filtering](https://www.betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering)
- [4.3 Tuning Notes](https://www.betaflight.com/docs/wiki/tuning/4-3-Tuning-Notes)
- [2026.6 Release Notes](https://www.betaflight.com/docs/wiki/release/Betaflight-2026-6-Release-Notes)

**BetaFPV:** factory CLI diffs for the Meteor65 and Meteor65 Pro, Betaflight 4.3.0, via the Wayback Machine (links in §6.2).

**Licensing:**

- [GNU GPL v3 text](https://www.gnu.org/licenses/gpl-3.0.html), as shipped in Betaflight's `LICENSE`
- [GPL FAQ](https://www.gnu.org/licenses/gpl-faq.html)
- [FSF licence list](https://www.gnu.org/licenses/license-list.html)
- [FSF on the App Store](https://www.fsf.org/news/2010-05-app-store-compliance)
- [Directive 2009/24/EC](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024)
- [17 U.S.C. §102](https://www.law.cornell.edu/uscode/text/17/102)
- [Apple App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/)

**Experiment:** a SITL build and UDP timing probe on the dev machine, 2026-10-03 (§7.1). The probe script isn't committed. It sent a 144-byte state packet (18 doubles) to UDP 9003 at a fixed rate and counted 16-byte replies on UDP 9002.
