# Public reference data for validating Quad physics

Research for [#6](https://github.com/BartoszSolkaBD/OpenDrone/issues/6), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). Checked against primary sources on 2026-10-03.

**Question:** we can't record our own flights. What public data can we validate Quad physics against?

The two alpha Quads are a whoop modelled on the BetaFPV Meteor 65 (1S) and a generic 5" freestyle quad. Vocabulary follows [CONTEXT.md](../../CONTEXT.md), especially [Flying](../context/flying.md) and [Verification](../context/verification.md).

## Short answer

- **5" Quad: enough public data to validate properly.** Two datasets record real 5" flights with documented hardware:
  - TII's *Race Against the Machine* is openly licensed (data CC BY 4.0). It has per-motor commands, gyro and battery voltage, but no confirmed motor RPM.
  - UZH's NeuroBEM has measured motor speeds, mass and inertia. It states no licence, so we can use it in tests that download it, but we can't copy it into the repo.
  - MiniQuadTestBench has static thrust curves and motor spin-up traces for about 200 motors in the 5" class. It is all rights reserved: we may cite its numbers but not copy its data.
- **Whoop Quad: no public flight logs exist.** I found no licensed Betaflight log from a Meteor 65 or any tiny whoop. The whoop is validated through:
  - BetaFPV's own thrust sheets for its 0702 and 0802 motors
  - its published weight and flight time
  - physics-rule checks
  - feel tests
  - a nano-quad research dataset (IDSIA, Crazyflie 2.1 Brushless, 45 g, no licence) as a rough cross-check only
- **Reading Betaflight logs is solved, with one catch.** The Rust crate `blackbox-log` is MIT/Apache, but its current release decodes gyro and motor values wrongly and rejects logs from Betaflight 2025.12 onwards. The fixes are unmerged.
  - **Plan:** convert logs to CSV once with Betaflight's official `blackbox_decode`, run as a separate program (it is GPL-3.0, so never linked). Tests then read plain CSV.
- **Most validation needs no logs at all.** Four physics-rule checks can be written as Scenarios from published specs alone:
  - hover throttle against weight
  - battery energy accounting
  - terminal velocity
  - step response

  Section 4 gives the rules and starting tolerance bands.
- **Licences are the main constraint, not data volume.** Most useful data is all rights reserved or GPL. Only CC BY 4.0 data (TII, Swift) may be committed to the repo, with attribution.

## 1. Public flight logs

"No licence" means none is stated anywhere we could find, which legally defaults to all rights reserved. We can download and test against such data privately, but not copy it into the repo.

### 1.1 Research datasets with real flights

| Source | What's in it | Hardware documented? | Licence | Fit |
|---|---|---|---|---|
| **TII *Race Against the Machine*** [S1] | <ul><li>Releases v1 to v3 (latest Oct 2025): 36 flights, piloted and autonomous</li><li>CSV at 500 Hz, plus ROS2 bags</li><li>Gyro, accelerometer, per-motor `thrust[0-3]` (normalised 0 to 1), RC channels, battery voltage</li><li>Motion-capture pose and velocity</li></ul> | Yes: full bill of materials and Betaflight CLI dump. <ul><li>T-Motor F60PROV 2020KV, T5147 props, 6S 1400 mAh</li><li>Kakute H7, bidirectional DShot600</li><li>~870 g, because it carries a Jetson Orin NX</li></ul> | **Data CC BY 4.0, code MIT** | 5": high. Whoop: none. |
| **NeuroBEM** (UZH RPG) [S2, S3] | <ul><li>1 h 15 min of agile flight at 400 Hz</li><li>Per-motor speed (rad/s) and its derivative, battery voltage</li><li>Motion-capture state, body rates and accelerations</li><li>The raw folder also holds the original Betaflight `.bfl` logs</li></ul> | Yes. <ul><li>Chameleon 6" frame, Hobbywing XRotor 2306, 5" tri-blade props, Betaflight</li><li>Paper: 752 g, max static thrust ≈33 N, motor time constant 33 ms</li><li>Readme: 0.772 kg, inertia diagonal [0.0025, 0.0021, 0.0043] kg·m²</li></ul> | **None stated** ("© 2021 Robotics and Perception Group", please cite) | 5": high, the best physics data. Whoop: none. |
| **IDSIA nano-drone system-identification benchmark** [S4] | ~75k samples at 100 Hz. Inputs: four motor speeds from bidirectional DShot. Outputs: motion-capture state. | Yes: Crazyflie 2.1 Brushless, 45 g, 1S 350 mAh, 8 mm 10000KV motors | **None stated** | Whoop: medium (right scale, has RPM, but not Betaflight and heavier than a Meteor 65). |
| **Blackbird** (MIT) [S5] | 10 h, 168 flights, motor RPM from optical encoders, PWM commands | Yes (915 g, DJI Snail propulsion) | Code MIT; no data licence | Low: the data host doesn't resolve today, so the data is effectively unavailable. |
| **UZH-FPV Drone Racing** [S6] | Camera, IMU and ground truth | Partial | CC BY-NC-SA 3.0 | Low: no motor or battery data. |
| **Swift "Champion-level drone racing" supplement** [S7] | Race positions and pseudocode | Paper: 870 g, ≈35 N max thrust, T-Motor Velox 2306, 5" tri-blade | **CC BY 4.0** (paper and Zenodo record) | Low as logs; useful for whole-Quad numbers. |

### 1.2 Betaflight log files in public repositories

| Source | What's in it | Hardware documented? | Licence | Fit |
|---|---|---|---|---|
| **pichim/bf_controller_tuning** [S8] | <ul><li>Real flight logs with **chirp excitation**: an automatic frequency sweep on each axis, built for system identification (see §2.4)</li><li>Craft "apex5" (5") plus two smaller quads</li><li>2025 Betaflight builds, high-resolution logging</li></ul> | Craft names and log headers only | GPL-3.0 (whole repo) | 5": high for closed-loop frequency response, if we accept the GPL terms on the data files. |
| **gimbal-ghost** test logs [S9] | Two Diatone Roma F5 logs (stock 5" ready-to-fly quad), Betaflight 4.2.9, current logged | Craft name only | GPL-3.0 | 5": low to medium. |
| **c0debreaker/5inch-apex-rcinpower-bblogs** [S10] | 14 logs plus CLI dumps, Betaflight 4.3, 2 kHz | Yes: Apex 5", RCinPower GTS v2 2207 1860KV, HQ 5x4.3x3, 6S. No weight given. | **None stated** | 5": medium, but can't be redistributed. |
| **fc-blackbox** test data [S11] | 8 logs, Betaflight 4.2 | No | MIT OR Apache-2.0 (the logs' original source is unverified) | Low: good as parser test files. |
| **Plasmatree PID-Analyzer** samples [S12] | 2 logs from a 2.5" quad, Betaflight 3.1.5 | No | Beerware for the code; unclear for the logs | Low. |

Checked with no logs found: the official `blackbox-log-viewer`, `blackbox-tools` and `betaflight` repositories; PIDtoolbox (its original repo now returns 404); UAV Tech; Zenodo, Hugging Face and Kaggle searches. Whoop logs exist only as scattered forum posts with no licence.

**No public log carries the dedicated `eRPM` fields** (per-motor RPM, logged since Betaflight 4.5 with bidirectional DShot). The public logs that have bidirectional DShot on are from older firmware. Per-motor RPM is only available from NeuroBEM, the IDSIA dataset, and possibly TII.

## 2. The Blackbox log format and decoders

### 2.1 The format in plain language

A Blackbox log has two parts:

- **A text header.** It lists every logged field, plus two things per field:
  - its *predictor*: how to guess the value from earlier frames
  - its *encoding*: how the difference from the guess is packed
- **A stream of binary frames:**
  - **I frames:** full snapshots, every 32 ms
  - **P frames:** small differences from the prediction, most of the data
  - **S frames:** slow state such as flight modes
  - **G and H frames:** GPS
  - **E frames:** events such as disarm or end of log

One file may hold several flights. Sources: [S13], [S14].

There is **no formal specification.** The "Blackbox Logging Internals" page [S14] is the closest, but it is out of date: it omits predictor 11 and encoding 10 and describes the old `P interval` header. The real definition is the firmware code: `blackbox.c`, `blackbox_encoding.c` and `blackbox_fielddefs.h` [S13], plus the viewer's `src/flightlog_parser.js` [S15].

### 2.2 The fields that matter for physics validation

| Field | Meaning | Units, from Betaflight source |
|---|---|---|
| `time` | Timestamp | µs |
| `gyroADC[0-2]`, `gyroUnfilt[0-2]` | Measured rotation rate, filtered and unfiltered | deg/s (×10 when `blackbox_high_resolution` is on) |
| `setpoint[0-3]` | Requested rotation rate per axis; `[3]` is throttle | deg/s; throttle ×1000 |
| `motor[0-7]` | Command sent to each motor | DShot units 48 to 2047; the `motorOutput` header gives the range |
| `eRPM[0-7]` | Measured motor speed (Betaflight 4.5+, bidirectional DShot) | eRPM/100. Mechanical RPM = value × 100 ÷ (`motor_poles` ÷ 2) |
| `vbatLatest`, `amperageLatest` | Battery voltage and current | 0.01 V and 0.01 A |
| `debug[0-7]` | Mode-dependent extras, for example chirp data | Per `debug_mode` |

- **Logging rate:** the PID loop rate divided by `2^blackbox_sample_rate`. The default is 1/4, so an 8 kHz loop logs at 2 kHz.
- **Versions:** Betaflight now uses calendar version numbers: 4.5 was followed by 2025.12, then 2026.6. The latest release is 2026.6.2, published 2026-09-16 [S16]. The header's `Data version` is still 2. New releases mainly add fields such as `imuQuaternion` (2025.12) and GPS time (2026.6). Sources: [S13], [S16].

### 2.3 Decoders

| Decoder | Language | Licence | State on 2026-10-03 | Usable by OpenDrone? |
|---|---|---|---|---|
| **blackbox-log** [S17] | Rust | MIT OR Apache-2.0 since 0.4.0; versions up to 0.3.2 were GPL-3.0 | <ul><li>0.4.3, April 2025</li><li>Supports Betaflight 4.2 to 4.5 only</li><li>Open bug [S18]: P frames after an I frame use the wrong history, so gyro and motor values diverge from `blackbox_decode`</li><li>Fixes for the bug and for 2025.12 support are open, unmerged PRs (#167, #168)</li></ul> | Can be linked, but **not trustworthy yet**. |
| **fc-blackbox** [S11] | Rust | MIT OR Apache-2.0 | crates.io 0.2.0 is from 2022 | Can be linked; coverage of new firmware unverified. |
| **bbl_parser** [S19] | Rust | AGPL-3.0-or-later, or a paid licence | 1.1.1, September 2026 | **Don't link.** Usable as a separate CLI. |
| **blackbox_decode** (blackbox-tools) [S20] | C | GPL-3.0 | Maintained (commits through 2026) | **Reference decoder.** Run as a separate CLI only. |
| **Blackbox Explorer** [S15] | JS | GPL-3.0 | Active | Manual cross-check and CSV export. |
| **orangebox** [S21] | Python | GPL-3.0 | 0.5.0, May 2026 | Separate tool only. |

**Licence rule:** OpenDrone is MIT/Apache-2.0.

- Linking a GPL or AGPL decoder into our code, even only for tests, makes those binaries GPL works. Avoid it.
- Running the GPL tool as a separate program and reading its CSV output is fine: the GPL treats that as two programs communicating, not one combined work [S22].

### 2.4 Two Betaflight features that change what logs can tell us

1. **Chirp mode for system identification** [S23], [S24].
   - Betaflight's chirp mode adds an automatic frequency sweep to the requested rotation rate. It runs one axis at a time, by default from 0.2 Hz to 600 Hz over 20 s.
   - It was added in January 2025 as a build-time option.
   - In April 2026 Betaflight started logging the sweep's frequency and axis. The stated purpose is that analysis tools can compute the Quad's frequency response, and recover the Quad's own dynamics, from a log.
   - A chirp log is the best real-world reference for how a quad responds to commands. pichim's repository [S8] holds such logs, including a 5".
2. **Thrust model.** Since May 2026 [S25], Betaflight's `thrust_linear` setting uses ArduPilot's motor thrust model [S26]: thrust = (1 − e)·m + e·m².
   - *m* is the motor command (0 to 1) and *e* is the "thrust expo".
   - ArduPilot's default is e = 0.65. The PR author suggests about 0.55 for 5" props.
   - Using the same model in our motor definitions lets a pilot's real `thrust_linear` setting carry straight into OpenDrone.

## 3. Thrust-stand and motor data

| Source | What's in it | Whoop | 5" | Raw data? | Licence |
|---|---|---|---|---|---|
| **MiniQuadTestBench** (Ryan Harrell) [S27] | <ul><li>Custom stand logging at 4 ms: thrust, RPM, current, voltage</li><li>Steady holds at idle, 25, 50, 75 and 100% throttle, plus a 0 to 100% ramp</li><li>**Step transitions** (idle to 50/75/100% and back), so motor spin-up and spin-down time series</li><li>~219 motors, sizes 2204 to 2408, on 5" tri-blades</li></ul> | None (no 0702, 0802 or 1102 motors) | **High** | No download offered | "Copyright 2015 © Ryan Harrell". No terms page, so **all rights reserved**. |
| **Tyto Robotics database** [S28] | User-uploaded static tests: ~1,500 systems, including several 2207 motors and a few 1103/1104 micro motors | None | Medium | Plots only without login | © Tyto Robotics; no data licence |
| **UIUC Propeller Database** [S29] | Wind-tunnel and static thrust and power coefficients. Includes a 1.85" Crazyflie prop, 2.5" tri-blades and 5" research props, but no modern FPV props. | Medium (small-prop behaviour at low Reynolds number) | Low to medium | Yes, full zip | "© UIUC Applied Aerodynamics Group", cite on use; no licence |
| **BetaFPV motor sheets** [S30] | <ul><li>Thrust against current at 4.0 V</li><li>0802SE 19500KV with Gemfan 1219 31 mm tri-blade: max 24.6 g at 3.1 A</li><li>0802SE 23000KV: 27.4 g at 3.3 A</li><li>0702SE II 23000KV with 1219S: 32.0 g at 4.19 A</li><li>The newer 0702 (2026) sheet also gives RPM</li></ul> | **High: the only whoop source** | None | Images | © BETAFPV, all rights reserved |
| **fpv-db-data** [S31] | <ul><li>JSON/CSV thrust tables (KV, prop, V, A, RPM, g) for ~200 motors</li><li>Includes BetaFPV 0702SE II and EMAX ECO II 2207</li><li>The 0802SE table is empty</li></ul> | Medium | Medium | Yes | Its README says CC BY 4.0; GitHub detects "Other". The rows are transcribed from manufacturer sheets, which an aggregator cannot license. Created 2026-09-10. Treat as unverified. |
| **Swift paper** [S7] | Whole-Quad numbers: 870 g, ≈35 N max static thrust, thrust-to-weight 4.1 | None | High | n/a | **CC BY 4.0** |
| **NeuroBEM paper** [S3] | 752 g, ≈33 N, thrust-to-weight 4.5, **motor time constant 33 ms** | None | High | n/a | arXiv non-exclusive licence (cite) |
| **Agilicious paper** [S32] | XRotor 2306 2400KV on 4S, 5.1" props: 4 × 9.5 N continuous static thrust, **motor time constant 39.1 ms** | None | High | n/a | arXiv non-exclusive licence (cite) |
| **gym-pybullet-drones** `cf2x.urdf` [S33] | Crazyflie 2.x: 27 g, thrust and torque coefficients, inertia ≈1.4e-5 kg·m², drag coefficients | Medium (whoop-scale proxy) | None | Yes | MIT |

Numeric facts aren't usually copyrightable, but a compiled table can be, and the EU also has a separate database right. **Rule of thumb:** cite the source for each number we put in a Quad definition. Don't copy whole tables or scrape all-rights-reserved sites. This is not legal advice.

### 3.1 Where each Quad parameter can come from

| Parameter | Whoop (Meteor 65) | 5" freestyle |
|---|---|---|
| Mass | <ul><li>22.83 g dry [S34]</li><li>plus a 1S 300 mAh BT2.0 pack, ≈7.6 to 8.3 g [S35]</li><li>≈30.5 g ready to fly (our sum)</li></ul> | <ul><li>Racing references are heavier because they carry computers: 752 to 870 g [S1, S3, S7]</li><li>A ~650 g freestyle build needs its own bill-of-materials estimate</li></ul> |
| Max thrust per motor | 24.6 g on the stock 0802SE 19500KV at 4.0 V [S30] | ~1.8 to 2.0 kg on 6S (MiniQuadTestBench average ~1.96 kg at 100%) [S27]; 8.2 to 8.8 N from the racing papers |
| Thrust against RPM | Only the newer BetaFPV 0702 sheet gives RPM; 0802SE RPM is unpublished | MiniQuadTestBench: thrust, RPM and current at each throttle step |
| Motor time constant | **Unpublished** | 33 ms [S3], 39.1 ms [S32] |
| Inertia | Unpublished; Crazyflie proxy ≈1.4e-5 kg·m² [S33] | [0.0025, 0.0021, 0.0043] kg·m² for the 772 g NeuroBEM quad [S2] |
| Aerodynamic drag | Unpublished | Must be fitted from NeuroBEM flight data (no licence) |
| Flight time (energy check) | "4 mins" on 300 mAh [S34] | Not published for a generic build |

## 4. Physics-rule checks that need no logs

Each rule below can become a Scenario: a start state, a scripted input and expected values with tolerances. The bands come from our own arithmetic on the sourced figures in §3. They are starting tolerances to refine, not measurements.

### 4.1 Hover throttle against weight

**Rule:** in a steady hover, total thrust equals weight. Using the thrust model from §2.4, the hover motor command *m* solves (1 − e)·m + e·m² = weight ÷ max thrust.

**Illustrative bands:**

- **Meteor 65:**
  - 30.5 g against 4 × 24.6 g gives a thrust-to-weight ratio of ≈3.2.
  - Hover command ≈ 0.45 to 0.56 for e = 0.55 to 1.
  - On a sagging pack (3.7 V instead of 4.0 V), max thrust drops by roughly (3.7 ÷ 4.0)², so the hover command rises to ≈ 0.50 to 0.60.
- **5":**
  - 650 g against 4 × ~1.9 kg gives a thrust-to-weight ratio of ≈11 to 12.
  - Hover command ≈ 0.15 to 0.30 above idle.

**Scenario:** hold altitude for 10 s; expect mean motor command ≈ the computed value, within a tolerance set by the uncertainty in *e*.

### 4.2 Battery energy accounting

**Rules:**

1. **Charge.** Charge used equals the time-integral of current. Energy drawn equals the time-integral of voltage × current.
2. **Motor energy.** Electrical power in = shaft power + resistive loss (i²·R) + friction loss. Energy conservation requires the motor's torque constant to equal its speed constant (KQ = KV in SI units) [S36]. No-load RPM ≈ KV × V, so at a fixed command thrust scales roughly with voltage squared.
3. **Voltage sag.** Terminal voltage = open-circuit voltage at the current state of charge − current × internal resistance. This is the standard equivalent-circuit model [S37]. Betaflight's sag compensation assumes about a 16% voltage drop from full (4.2 V) to warning level (3.5 V) [S38].
4. **Hover power floor.** Ideal hover power (momentum theory, from a helicopter-aerodynamics textbook [S39]) is thrust^1.5 ÷ √(2·ρ·A), where ρ is air density and A the total rotor disc area. Simulated electrical power must always exceed it. The ratio between them (overall efficiency) must fall in a plausible band.

**Illustrative bands:**

- **Meteor 65:**
  - Ideal hover power is ≈1.9 W.
  - BetaFPV's figures imply ≈2 to 3 g/W, so ≈10 to 15 W electrical at hover: about 3 to 4 A.
  - That gives ≈3.5 to 5 minutes on 80% of 300 mAh, which matches BetaFPV's "4 mins".
  - Overall efficiency ≈0.12 to 0.19: tiny props and motors are inefficient.
- **5" (650 g, 6S):**
  - Ideal ≈46 W.
  - Bench data in the 5–8 g/W range gives ≈80 to 130 W at hover: about 4 to 6 A.
  - Overall efficiency ≈0.35 to 0.55.

**Scenarios:**

- Hover until the pack reaches warning voltage; expect elapsed time within the band.
- Expect counted mAh to equal the integral of current to within rounding.
- Expect the efficiency ratio inside its band.

### 4.3 Terminal velocity and top speed

**Rules:**

- In a power-off fall, speed approaches √(2·m·g ÷ (ρ·Cd·A)), using the Quad's own drag area.
- At full throttle and a fixed tilt, top speed is reached when the horizontal thrust component equals drag.
- These are **consistency checks** for the drag model. We have no public drag figures for either Quad. NeuroBEM's flight data is the only source to fit 5" drag from, so these bands start wide and tighten once a drag model is fitted.

**Scenarios:**

- Disarm at 100 m; expect speed at 10 s ≈ the formula's value ± 5%.
- Expect free fall with no drag to give exactly g.

### 4.4 Step and frequency response

**Rules:**

1. **Motor lag.** A motor approaches a new commanded speed as a first-order lag. Published time constants for 5" racing quads are 33 to 39 ms [S3, S32]. A whoop's is unpublished; we expect it to be shorter (smaller rotors) and should treat it as a tunable assumption.
2. **Response shape.** With the same rates and PID values, the simulated response of `gyro` to a `setpoint` step should have a rise time and overshoot similar to a real quad of that class.
   - Compute it the same way the community tools do: deconvolution of setpoint against gyro, as in PID-Analyzer [S12].
   - Better still, compute the full frequency response from a chirp log (§2.4).
3. **Saturation.** Peak rotation acceleration is bounded by maximum differential thrust × arm length ÷ inertia. No simulated manoeuvre may exceed it.

**Scenario:** inject a 500 deg/s roll step; expect rise time and overshoot inside bands taken from a public 5" log of the same class.

### 4.5 Invariants (no data at all)

These are cheap Scenarios that catch integration bugs:

- **Free fall:** motors off, no drag, gives exactly g.
- **Conservation:** angular momentum is conserved in a torque-free tumble; there is no energy gain without power input.
- **Symmetry:** mirrored inputs give mirrored outputs.
- **Yaw direction:** reaction torque turns the Quad opposite to the motors' net spin.
- **Physics rate:** results stay the same, within tolerance, when the fixed physics rate changes.

## 5. Proposed validation approach

Five layers, cheapest first. Every layer produces plain-language Scenario expectations a non-programmer can review.

1. **Invariant Scenarios (§4.5).** No data needed. They run in CI on every change.
2. **Component checks against bench data (§3).** Each Quad's motor-and-prop definition must reproduce its sourced static thrust and current at 25/50/75/100% throttle:
   - Whoop: BetaFPV sheets.
   - 5": MiniQuadTestBench numbers for one named motor, cited, not copied wholesale.
   - The 5" motor step response must match the 33 to 39 ms time constant.
3. **Whole-Quad rule Scenarios (§4.1 to 4.4).** Hover command, hover current, flight time, efficiency ratio and terminal speed, each with a tolerance band and a cited source.
   - For the whoop, this is the main evidence, because no flight logs exist.
4. **Log replay against real flights (5" only).**
   - **a. Open loop.** Feed the logged motor speeds (NeuroBEM) or motor commands (TII) into the simulated Quad, without our Flight Controller. Compare predicted rotation rates and accelerations over short windows (0.5 to 2 s, before drift dominates). Report force and torque errors the same way NeuroBEM reports its own model's errors.
   - **b. Closed loop.** Run our Flight Controller with the logged settings and the logged `setpoint`. Compare step responses (TII and other logs) and frequency responses (chirp logs).
   - **Data handling:** a script downloads the source data and converts it with `blackbox_decode` run as an external tool.
     - Only CC BY 4.0 data (TII) is committed, as trimmed CSVs with an attribution notice.
     - Data with no licence (NeuroBEM) or GPL data (chirp logs) stays outside the repo: tests that need it download it into a local cache and skip when it's absent. They don't gate CI until the licences are cleared.
5. **Feel tests.** A human flies against their memory of the real Quad (map Notes). Most important for the whoop, which has the least data.

### Actions that would close the gaps

- **Ask for licences:**
  - the NeuroBEM authors (UZH RPG) and the IDSIA authors, asking them to put their datasets under CC BY 4.0
  - Ryan Harrell, for permission to use MiniQuadTestBench numbers and step traces
  - pichim, for a CC BY licence on the chirp logs
- **Call for donated logs.** A short guide asking whoop and 5" pilots to donate Betaflight 4.5+ logs under CC BY 4.0, with:
  - bidirectional DShot on, so `eRPM` is logged
  - high-resolution logging
  - a CLI dump
  - weighed all-up mass
  - ideally a chirp run

  One Meteor 65 log with RPM would turn the whoop from "rules only" into "validated".

## 6. Surprises that may affect other tickets

- **[#10 physics effects](https://github.com/BartoszSolkaBD/OpenDrone/issues/10), [#16 Pack format](https://github.com/BartoszSolkaBD/OpenDrone/issues/16):**
  - Betaflight adopted ArduPilot's thrust model in May 2026 (§2.4). Using it in Quad definitions keeps pilots' real settings meaningful.
  - The motor time constant should be a first-class Quad parameter: 33 to 39 ms for 5", unknown for the whoop.
- **[#5 Betaflight internals](https://github.com/BartoszSolkaBD/OpenDrone/issues/5):**
  - Betaflight is now calendar-versioned (2026.6.2).
  - It ships a chirp system-identification mode and logs per-motor RPM.
  - Its logging code is the only specification of the log format.
- **[#12 crates](https://github.com/BartoszSolkaBD/OpenDrone/issues/12), [#15 repo and CI](https://github.com/BartoszSolkaBD/OpenDrone/issues/15):**
  - No trustworthy MIT/Apache log decoder exists today. CI should consume pre-converted CSVs, and the repo needs a data-provenance policy:
    - what may be committed
    - where downloaded data lives
    - how attribution is kept
- **[#11 Scenario format](https://github.com/BartoszSolkaBD/OpenDrone/issues/11):** Scenario tolerances will often come from external sources. The format needs a place to cite where each expected value came from.
- **Whoop weight:** the real Meteor 65 is ≈30.5 g ready to fly (22.83 g dry plus ≈8 g pack), heavier than "20 g whoop" folklore.
  - It hovers near half throttle with a thrust-to-weight of only ≈3.2.
  - Battery sag noticeably changes how it flies through a pack.
- **5" reference quads are racing builds.** The openly documented 5" quads weigh 750 to 870 g because they carry compute. A "generic 5" freestyle" Quad around 650 g will be parameterised by scaling, not matched one-to-one.

## Sources

Licence shown for each source. "No licence" means all rights reserved by default.

- [S1] TII Racing, *Race Against the Machine* dataset. Data CC BY 4.0, code MIT. https://github.com/tii-racing/drone-racing-dataset. Hardware: [`quadrotor/bom.md`](https://github.com/tii-racing/drone-racing-dataset/blob/main/quadrotor/bom.md) and `quadrotor/BTFL_cli_backup.txt`.
- [S2] NeuroBEM dataset page and Readme. No licence. https://rpg.ifi.uzh.ch/NeuroBEM.html, https://download.ifi.uzh.ch/rpg/NeuroBEM/Readme.md
- [S3] Bauersfeld, Kaufmann, Foehn, Scaramuzza, "NeuroBEM: Hybrid Aerodynamic Quadrotor Model", RSS 2021. arXiv non-exclusive licence. https://arxiv.org/abs/2106.08015
- [S4] IDSIA nano-drone system-identification benchmark. No licence. https://github.com/idsia-robotics/nanodrone-sysid-benchmark (paper: arXiv 2512.14450)
- [S5] MIT Blackbird dataset. Code MIT, no data licence; data host unreachable 2026-10-03. https://github.com/mit-aera/Blackbird-Dataset
- [S6] UZH-FPV Drone Racing dataset. CC BY-NC-SA 3.0. https://fpv.ifi.uzh.ch/
- [S7] Kaufmann et al., "Champion-level drone racing using deep reinforcement learning", *Nature* 620 (2023). CC BY 4.0. https://doi.org/10.1038/s41586-023-06419-4. Supplementary data, CC BY 4.0: https://zenodo.org/records/7955278
- [S8] pichim, `bf_controller_tuning`, including chirp `.bbl` logs. GPL-3.0. https://github.com/pichim/bf_controller_tuning
- [S9] gimbal-ghost test logs. GPL-3.0. https://github.com/gimbal-ghost/gimbal-ghost
- [S10] c0debreaker, 5" Apex logs. No licence. https://github.com/c0debreaker/5inch-apex-rcinpower-bblogs
- [S11] ilya-epifanov, `fc-blackbox` (Rust decoder plus test logs). MIT OR Apache-2.0. https://github.com/ilya-epifanov/fc-blackbox
- [S12] Plasmatree, PID-Analyzer. Beerware licence. https://github.com/Plasmatree/PID-Analyzer
- [S13] Betaflight firmware, blackbox sources (`blackbox.c`, `blackbox_encoding.c`, `blackbox_fielddefs.h`, `dshot.c`). GPL-3.0. https://github.com/betaflight/betaflight/tree/master/src/main/blackbox
- [S14] Betaflight docs, "Blackbox Logging Internals". https://betaflight.com/docs/development/Blackbox-Internals
- [S15] Betaflight Blackbox Explorer. GPL-3.0. https://github.com/betaflight/blackbox-log-viewer
- [S16] Betaflight releases (2026.6.2, 2026-09-16). https://github.com/betaflight/betaflight/releases
- [S17] `blackbox-log` crate. MIT OR Apache-2.0 from 0.4.0. https://crates.io/crates/blackbox-log, https://github.com/blackbox-log/blackbox-log
- [S18] blackbox-log issue #164, "motor/gyro values diverge from blackbox_decode after I-frame boundaries" (open). https://github.com/blackbox-log/blackbox-log/issues/164. Fix PRs #167 and #168 (open).
- [S19] `bbl_parser` crate. AGPL-3.0-or-later. https://crates.io/crates/bbl_parser
- [S20] Betaflight `blackbox-tools` (`blackbox_decode`). GPL-3.0. https://github.com/betaflight/blackbox-tools
- [S21] `orangebox`. GPL-3.0. https://github.com/atomgomba/orangebox
- [S22] GNU GPL FAQ, "mere aggregation" and "output of a GPL program". https://www.gnu.org/licenses/gpl-faq.html#MereAggregation
- [S23] Betaflight PR #13105, "Chirp signal generator as flight mode" (merged 2025-01-21). https://github.com/betaflight/betaflight/pull/13105
- [S24] Betaflight PR #15113, "Expand chirp debug channels for offline system identification" (merged 2026-04-17). https://github.com/betaflight/betaflight/pull/15113
- [S25] Betaflight PR #15226, "align thrust_linearization closer to thrust stand test data shapes" (merged 2026-05-24); see `pidApplyThrustLinearization` in `src/main/flight/pid.c`. https://github.com/betaflight/betaflight/pull/15226
- [S26] ArduPilot, motor thrust scaling (`MOT_THST_EXPO`, default 0.65). https://ardupilot.org/copter/docs/motor-thrust-scaling.html. Source: `libraries/AP_Motors/AP_Motors_Thrust_Linearization.cpp` (GPL-3.0).
- [S27] MiniQuadTestBench, Ryan Harrell. All rights reserved. https://www.miniquadtestbench.com/
- [S28] Tyto Robotics motor and propeller database. All rights reserved. https://database.tytorobotics.com
- [S29] UIUC Propeller Data Site. No licence; cite on use. https://m-selig.ae.illinois.edu/props/propDB.html
- [S30] BetaFPV 0802SE and 0702SE II motor product pages (thrust sheets). © BETAFPV. https://betafpv.com. These figures were read from the sheets by a research helper and not re-checked line by line.
- [S31] fpv-db-data. README states CC BY 4.0; provenance caveats in §3. https://github.com/fpvdb/fpv-db-data
- [S32] Foehn et al., "Agilicious: Open-Source and Open-Hardware Agile Quadrotor for Vision-Based Flight", *Science Robotics* (2022). arXiv non-exclusive licence. https://arxiv.org/abs/2307.06100
- [S33] `gym-pybullet-drones`, `cf2x.urdf`. MIT. https://github.com/utiasDSL/gym-pybullet-drones
- [S34] BetaFPV, Meteor65 Brushless Whoop Quadcopter (2022) product page: 22.83 g, 0802SE 19500KV, 31 mm tri-blade, BT2.0 300 mAh, "4 mins". https://betafpv.com/products/meteor65-brushless-whoop-quadcopter-1s
- [S35] BetaFPV BT2.0 300 mAh 1S battery listings: 7.59 g to 8.3 g depending on variant. Retailer copies of BetaFPV specs, e.g. https://www.getfpv.com/betafpv-bt2-0-300mah-1s-30c-hv-battery-8pcs.html
- [S36] M. Drela, "First-Order DC Electric Motor Model", MIT, Feb 2007. https://web.mit.edu/drela/Public/web/qprop/motor1_theory.pdf
- [S37] M. Chen and G. A. Rincón-Mora, "Accurate electrical battery model capable of predicting runtime and I–V performance", *IEEE Trans. Energy Conversion* 21(2), 2006. doi:10.1109/TEC.2006.874229
- [S38] Betaflight 4.2 Tuning Notes, battery sag compensation. https://betaflight.com/docs/wiki/tuning/4-2-Tuning-Notes
- [S39] J. G. Leishman, *Principles of Helicopter Aerodynamics*, 2nd ed., Cambridge University Press, 2006, ch. 2 (momentum theory). Textbook, cited for the standard formula only.
