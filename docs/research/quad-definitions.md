# The alpha Quads' numbers: where each comes from

Working notes for [#40](https://github.com/BartoszSolkaBD/OpenDrone/issues/40), which fills in both alpha Quad definitions: `packs/opendrone/quads/whoop-65/quad.toml` and `packs/opendrone/quads/freestyle-5/quad.toml`. Each number in those files names a source; this note holds the working that a source line is too short for. Every value comes from the decisions in [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10) §5, [#14](https://github.com/BartoszSolkaBD/OpenDrone/issues/14), [#26](https://github.com/BartoszSolkaBD/OpenDrone/issues/26) §5, [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28) §8, [#32](https://github.com/BartoszSolkaBD/OpenDrone/issues/32) §6 and [#34](https://github.com/BartoszSolkaBD/OpenDrone/issues/34), and the [flight-dynamics research](flight-dynamics.md) §8–§9.1, unless it says otherwise. Product pages were read on 2026-10-07.

Sea-level air (1.225 kg/m³) and g = 9.81 m/s² throughout. The prop coefficients follow the research's §3.1: thrust T = C_T·ρ·n²·D⁴ and power P = C_P·ρ·n³·D⁵, with n in turns a second and D the prop's diameter.

## Whoop 65 (BetaFPV Meteor65 Pro, LAVA II 320 mAh)

**Checked on the maker's pages:**
- Meteor65 Pro: 23.01 g without battery, 0802SE 19500KV, 35 mm 3-blade props, C03 camera with a 2.1 mm lens for 160°, BMI270 gyro, Bluejay ESC firmware. (The page gives "frame size 65 mm"; the research's §8.1 and #10 use the 66 mm wheelbase, which the file keeps.)
- LAVA II 1S 320 mAh: 64 × 10 × 6 mm, 8.2 g, 95C, BT2.0, charged to 4.35 V. That makes the pack's collision box a Manufacturer number.

**Thrust coefficient: C_T 0.29, Estimate, range 0.2–0.46.** #34's correction: BetaFPV's only RPM data is the 0802 (2026) on 40 mm props, 55 g at 46,481 RPM, which gives C_T ≈ 0.29. On the Meteor65 Pro's 35 mm props the same C_T gives k_f = C_T·ρ·D⁴/4π² ≈ 1.35 × 10⁻⁸ N/(rad/s)², a hover at about 22,700 RPM and full thrust (30.6 g) at about 45,000 RPM. It is an Estimate because it comes from a different prop, and #34 left it to a Feel Test. The range runs up to 0.46, the C_T the research's original §9.1 figure would need on 35 mm props, and down to 0.2, about a free 5″ prop's (0.19). A readable check pins the 22,700 RPM hover.

**Winding resistance: 0.50 Ω, Estimate, range 0.2–0.8 Ω.** At full thrust, 45,000 RPM needs 45,000 ÷ 19,500 = 2.31 V of back-voltage. BetaFPV's 3.4 A at 4 V leaves 1.69 V across the motor and ESC, so the motor model's lumped resistance is 1.69 V ÷ 3.4 A ≈ 0.50 Ω (#34). It rests on C_T, so it is an Estimate.

**Power coefficient: C_P 0.26, Estimate, range 0.18–0.37.** At full thrust the motor turns (3.4 A − 0.3 A no-load) × (1/19,500 KV in SI) ≈ 1.52 × 10⁻³ N·m, so k_m ≈ 6.8 × 10⁻¹¹ and k_m/k_f ≈ 0.005 m, as the research's §9.1 estimates. In coefficient form that is C_P ≈ 0.26. A no-load current anywhere from 0.1 A to 0.6 A moves it only from 0.28 to 0.24; the wider range allows for C_T moving too. The same numbers give a hover current of about 1.0 A, which matches BetaFPV's 8.8 g at 1.0 A (#34).

**Rotor inertia: 0.25 g·cm², Estimate, range ×0.5–×2.** A 35 mm 3-blade prop weighs about 0.25 g; with most of its mass near the hub, I ≈ 0.2·m·R² ≈ 0.15 g·cm². An 0802 motor's bell adds about 0.1 g·cm². As a check, the motor model's spin-up time, τ ≈ J / (1/(R·K_V²) + 2·k_m·ω), comes out at about 30 ms in a hover, inside the 20–50 ms motor lag #10 estimated.

**No-load current: 0.3 A at 4 V, Estimate, range 0.1–0.6 A.** BetaFPV doesn't publish it (research §8.5). It is #16's sample value, so its source says "first guess". The motor ticket (#41) kept it: with it, the motor model meets BetaFPV's thrust table within 7% at every row (see [The motor model against the makers' tables](#the-motor-model-against-the-makers-tables-41) below), and a Feel Test settles it.

**Shapes (Estimates from the layout):** the frame and canopy as a box of 35 × 30 × 20 mm, the props' plane about 8 mm above the centre of mass, the pack's centre about 6 mm below it (the 8.2 g pack hangs under the 23 g frame), and duct rings 37 mm inside (a 1 mm tip gap round a 35 mm prop), 1.5 mm thick and 14 mm tall. Nothing here was measured; photos and the sizes above set them. The camera position (15 mm forward, 12 mm up) is placed the same way; as a camera default it carries no Confidence.

**Battery:**
- **Resistance 29 mΩ, Derived** from BetaFPV's discharge curve for this pack, "Discharge at Current 18A" on the LAVA II product page:
  - At rest the pack sits at about 4.33 V.
  - With 18 A drawn it falls at once to about 3.80 V, where the curve turns, about 2 s in. After that it falls slowly as charge is used.
  - So the resistance is (4.33 V − 3.80 V) ÷ 18 A = 0.53 V ÷ 18 A ≈ 29 mΩ.
  - Those 2 s use about 10 mAh, 3% of the pack, which lowers the open-circuit voltage by a few hundredths of a volt at most, so the figure is good to about ±2 mΩ.
  - The same chart's LAVA 300 mAh falls from about 4.33 V to 3.70 V, about 35 mΩ, which matches the research's §5.3 figure for that pack.
  - The voltages are read off the chart by eye; BetaFPV states no resistance.
- The open-circuit voltage curve is a typical LiHV curve, an Estimate within ×0.97–×1.03. Its ends must equal `full` (4.35 V, Manufacturer) and `empty` (3.30 V, Estimate 3.2–3.5 V), which the checker cross-checks.
- Recovery 3.3 s, Estimate 1–30 s: the one RC pair Bauersfeld & Scaramuzza fitted to 4S–6S packs (research §5.3). No whoop pack has been fitted.
- **Slow sag 0 mV·Ah/W, Estimate, range 0–1.048 mV·Ah/W.** How big that pair's slow sag grows (see [The battery's slow sag](#the-motor-model-against-the-makers-tables-41) below). The 29 mΩ is read about 2 s into BetaFPV's 18 A curve, so it already holds whatever slow sag builds by then; adding Bauersfeld & Scaramuzza's 1.048 mV·Ah/W on top would count about 0.1 V of it twice (18 A at 3.8 V from a 0.32 Ah cell is 214 W/Ah, which settles at 0.22 V, 45% of it by 2 s). After those 2 s the curve falls only as charge is used, so the pack shows no slow sag of its own: none is added, and its recovery time has nothing to act on. If a Feel Test finds the pack recovering after a chop, this may grow up to the 4S–6S fit, and the 29 mΩ would then be the sum of an instant part and a slow one.
- The BT2.0 connector's 10 mΩ is #10's Estimate.

**From the research's §9.1 and #10 §5, unchanged:** inertia (0.7, 0.9, 1.4) × 10⁻⁵ kg·m², motor lag 20–50 ms (35 ms both ways), duct ram drag 1.2 s⁻¹ (range 0.6–2.4) and the ducted rotor's centre of pressure 0.75 × 17.5 mm ≈ 13 mm higher (range 9–18 mm).

**#16's first guesses, since replaced or re-sourced:** the air ticket (#42) worked the body drag areas out from the collision shapes, and re-sourced the rotor drag (see [The air](#the-air-42) below); the motor ticket (#41) kept the no-load current, still marked "first guess".

## Freestyle 5″ (Nazgul Evoque F5 V2 class, Velox V2207 V3 1750KV, T5147, 6S 1400 mAh LiPo)

**Checked on T-Motor's page** (the Velox V2207 V3 1750KV): 12N14P, so 14 poles; 1.2 A no-load at 10 V; and the T5147 table at 20%, 40%, 60%, 80% and 100% throttle, ending at 1,591.1 g, 29,447.1 RPM and 34.6 A at 23.5 V.

**Thrust coefficient: C_T 0.187, Derived.** Each row of the table gives C_T = T ÷ (ρ·n²·D⁴) with D = 5.1″ = 129.5 mm:

| Throttle | RPM | Thrust | C_T |
|---|---|---|---|
| 20% | 11,294 | 210.1 g | 0.169 |
| 40% | 17,179 | 525.7 g | 0.182 |
| 60% | 20,866 | 784.6 g | 0.185 |
| 80% | 25,071 | 1,158.9 g | 0.189 |
| 100% | 29,447 | 1,591.1 g | 0.188 |

A least-squares fit of thrust against n² over all five rows gives 0.187, so k_f ≈ 1.63 × 10⁻⁶ N/(rad/s)², inside the research's 1.5–1.65 × 10⁻⁶. A readable check pins the table at 100% and 60% within 3%.

**Power coefficient: C_P 0.105, Estimate, range 0.07–0.11.** Through the motor model (torque = (current − no-load current) ÷ K_V in SI), the table gives C_P 0.073 at 60%, 0.088 at 80% and 0.106 at 100%: it doesn't hold still, because the model's torque constant and constant no-load current are only first-order. The middle value, 0.09, stood at first, with the spread as its range. The motor ticket (#41) fitted 0.105 to the full-throttle row, since that row's command is the only one whose drive is known (see below); the Feel Test log records the move.

**Winding resistance: 0.19 Ω, Estimate, range 0.05–0.3 Ω.** With the drive at the throttle's share of 23.5 V, the lumped resistance that meets the table is 0.19 Ω at 100%, 0.22 Ω at 80% and 0.19 Ω at 60%. It was 0.20 Ω at first; the motor ticket (#41) fitted 0.19 Ω with C_P to the full-throttle row. #10 fits it to the TII logs.

**Rotor inertia: 55 g·cm², Estimate, range ×0.5–×2.** A T5147 weighs about 4.5 g (0.2·m·R² ≈ 38 g·cm²) and a 2207's bell adds roughly 15–25 g·cm². The spin-up time this gives in a hover, about 28 ms, sits just under the 33 ms measured on NeuroBEM's 5″.

**Motor lag: 33 ms both ways, Measured.** #10 §5 lists the 5″'s motor lag as Measured on similar 5″ quads, 33–39 ms. The file takes 33 ms, NeuroBEM's time constant for its 752 g 5″ racing quad, which the research's §8.2 uses as the generic 5″ reference; Agilicious measured 39 ms on another quad. NeuroBEM fits one time constant both ways, so spin-up and slow-down share it. As a Measured number it is locked, and #10 §6's Thrust Stand Scenario checks it with a Source basis.

**Rotor drag: 0.4 s⁻¹, Estimate, range 0.24–0.54 s⁻¹.** #10 §5 also calls this "Measured on a similar quad", but Faessler et al. measured two numbers on their 610 g quad, 0.49–0.54 s⁻¹ forward and 0.24–0.39 s⁻¹ sideways, and the Quad file holds one. No single measured value exists to lock, and #10 refines it against the TII logs, so it is an Estimate whose range is the measured span.

**Reverse thrust: 48%, Measured.** #16 §4 and #26 §3 lock it: measured on a similar 5.1″ 3-blade prop.

**The pack: a GNB (Gaoneng) 1400 mAh 6S 160C LiPo.** #10 §5 asks for a typical 6S 1400 mAh LiPo and names none, so the file names a common one, which lets the pack's size and mass be the maker's own, as #26 §5 asks for the pack box. The maker's specification, as Rotorvillage lists it: 1400 mAh, 6S1P, nominal 22.2 V, 40 × 38 × 80 mm (height × width × length), 233 g ± 7 g, XT60. (RaceDayQuads lists the same pack at 269 g; the maker's sheet is the one cited.) It is a standard LiPo, not an HV one, so a cell is full at 4.20 V.

**Mass:** 419 g dry (Manufacturer, iFlight) and the 233 g pack (Manufacturer, GNB), so 652 g in all, as #10's "about 650 g" (iFlight's own figure with a 6S 1400 mAh pack is about 644 g).

**Battery:** 6S, LiPo, charged to 4.20 V a cell, 1400 mAh (all from GNB). The pack's resistance is #10's "about 30 mΩ with leads", an Estimate, split here into 28 mΩ for the pack (range 15–45 mΩ) and 2 mΩ for the XT60 (range 1–5 mΩ); GNB publishes none. The curve is a typical LiPo curve ending at `full` and `empty` (3.50 V, Estimate 3.3–3.6 V); recovery is 3.3 s as for the whoop. Its slow sag is Bauersfeld & Scaramuzza's own fit, 1.048 mV·Ah/W, an Estimate within ×0.5–×2: their packs were 4S–6S LiPos like this one, and the 28 mΩ is a plain resistance with no slow part read into it.

**Shapes and drag (Estimates from the layout):** the plates and stack as a box of 80 × 45 × 35 mm; the pack's box (GNB's 80 × 38 × 40 mm, Manufacturer) with its centre 26 mm above the centre of mass (the 233 g pack on top lifts the centre of mass about 14 mm above the frame's); the props' plane about 5 mm below the centre of mass; body drag areas front 45, side 45 and top 100 cm² (each silhouette with a drag coefficient of 1, motors and arms included). The TII logs ticket (#47) fits the drag to those logs.

**Inertia: (14, 15, 25) kg·cm², Estimate, range ×0.7–×1.8:** #10's (1.4, 1.5, 2.5) × 10⁻³ kg·m². The range reaches NeuroBEM's heavier 6″ quad, (2.5, 2.1, 4.3) × 10⁻³ kg·m².

**The gyro:** typical 5″ boards carry an ICM-42688-P or a BMI270, both ±2000 °/s.

## The motor model against the makers' tables (#41)

The motor ticket checks both Quads' motor numbers against their makers' tables with Thrust Stand Scenarios (`scenarios/quads/<quad>/thrust-table.toml`), each on a bench supply like the maker's (`scenarios/test-quads/*-bench-supply.toml`). How the motor model works is in `crates/physics/src/motor.rs`.

**The no-load current grows with speed.** A Quad definition gives the no-load current at one voltage. The model takes it as a loss that grows in step with the motor's speed, from nothing at a standstill to the given current at that voltage's no-load speed. A constant one would act at a standstill too, and on the whoop 0.3 A is more than Bluejay's start-up power limit can push through the windings (1.96% of 4.2 V across 0.5 Ω is 0.16 A), so its motors could never start, which real ones do.

**T-Motor's "throttle" isn't the ESC's drive.** The 20% row turns 11,294 RPM, but a 1750 KV motor on 20% of 24.0 V can turn at most 8,400 RPM. Working each row back through the motor model, the rows sit at about 30%, 50%, 64%, 82% and 100% drive: T-Motor's throttle is a share of its stand's own signal. So the Scenario commands each row's thrust and checks the row's speed and current; only the full-throttle row is commanded as it stands. Fitted to that row, C_P 0.105 and 0.19 Ω give, at full drive from the 24.0 V supply, 1,580 g at 29,500 RPM and 35.0 A (the table: 1,591.1 g, 29,447 RPM, 34.6 A). At the lower rows the model draws 0% (80%), 10% (60%), 13% (40%) and 31% (20%) less current than the table: a small prop needs more power for its thrust at low speed (the table's own C_P rises from 0.106 at full throttle to about 0.12 at 20%), which one fixed C_P can't follow.

**BetaFPV's current is the motor's own.** BetaFPV's 0802SE table gives thrust against current, about 9 g per amp from 0.5 A to 3.4 A. Thrust in step with current is how the current through a motor behaves: its torque, and its prop's thrust, both follow it. The current an ESC draws from the battery at part throttle is smaller, about its drive times the motor's, and grows faster than thrust. So the Scenario checks BetaFPV's line against the motor's own current, as #34 did, and the Whoop 65's numbers meet it within 7% at 25%, 50%, 75% and 100% drive, unchanged. One consequence to settle with the hover-time check (#57): at hover the model's four ESCs draw about 1.6 A from the pack, which would last about 10 minutes, where the maintainer remembers 4–6 minutes (#10 §6). The board, camera and video transmitter draw some too, and a whoop's ESC loses more at part throttle than the model's ideal one (its motor's tiny inductance lets the current ripple), so the gap may be real losses the model lacks, or BetaFPV's current may be the pack's after all.

**Spin-up and slow-down.** A motor approaches the speed its drive holds as a first lag, with the spin-up time while speeding up and the slow-down time while its ESC brakes it. Its rotor inertia sets how much current that takes, and the drive caps it: an ESC gives no more than its command while speeding up, and no less than none (full braking) while slowing. The 5″'s 33 ms both ways fit inside those caps. The whoop's rotor, 0.25 g·cm² through 0.5 Ω windings, can't be braked faster than its inertia × resistance × KV² = 52 ms (less the prop's drag), so its 35 ms slow-down time is the lag it asks for, and the braking it gets is a little slower: 38% of full speed is left after 35 ms, against 37% for a true 35 ms lag.

**The battery's slow sag.** The pack's resistance acts at once. A slower part of the sag builds up under load and dies away with the pack's recovery time: Bauersfeld & Scaramuzza's one resistor–capacitor pair (research §5.3, [S7]). By their eq. 13 it settles at `k` volts per cell for each watt the cell gives per amp-hour of its capacity, and the Quad definition gives `k` as `slow_sag`, in mV·Ah/W (their fit, 0.00104846, is 1.048 mV·Ah/W; dimensionally it is a time, 3.77 s). On the Freestyle 5″ it is their fit, so at hover (about 105 W, 12.5 W/Ah a cell) it settles at about 13 mV a cell, 0.08 V for the pack, and a one-second full-throttle punch after a hover leaves about 0.56 V to recover. On the Whoop 65 it is 0, because its 29 mΩ already holds its slow sag (see its battery above). A bench supply's Test Quad sets it to 0 too.

**Bluejay's start-up power limit.** Read in Bluejay's code (`crates/physics/src/esc.rs` has the file and line for each step): `motor_start` caps every frame's drive at Startup Power Max, 5 of 255, the 1.96% in the Quad definition. The cap holds through the start-up phase, 24 commutations (four electrical turns), and the initial-run phase after it, a countdown of 12 turns that starts on the fourth; only when that reaches nought, on the 15th electrical turn, is it lifted ("lift startup power restrictions", Bluejay.asm L945–951). So a motor starts at 1.96% for 15 electrical turns: 2.1 turns of the 5″'s 14-pole motor (about 0.2 s at its start-up speed of about 800 RPM) and 2.5 of the whoop's 12-pole one (about 0.15 s). Then it runs as commanded. Bluejay also limits power at low speed after the start-up phase (`Pgm_Rpm_Power_Slope`); the model leaves that out, since the measured spin-up times already include whatever a real ESC does. Stalled motors' restarts come with the Prop Strike ticket (#45).

## The air (#42)

The air ticket makes the drag numbers act (crates/physics/src/air.rs explains each effect, and the Physics Scenarios in `scenarios/physics/` prove them). It adds no number to the Quad definition: every effect's strength comes from numbers already there.

| Effect | Numbers it uses |
|---|---|
| Thrust falling in a climb, rising in a descent and at speed (E10, E11) | The props' diameter and thrust coefficient, through momentum theory |
| Rotor drag, and the nose lifting at speed (E16, E18) | `rotor_drag`, acting in the props' plane, `rotor_height` above the centre of mass |
| Body drag (E17) | `drag_area` |
| Duct ram drag and nose-up moment (E25, E26) | `ram_drag`, acting `nose_up_offset` above the props' plane |
| The rotors' own spin: the frame turning against a rotor that speeds up, and their spin pushing at right angles to a flip (E7, E8) | `rotor_inertia` |

**The Whoop 65's body drag areas: front 15, side 17, top 17 cm², Estimate, range ×0.5–×2.** The research (§4.1) gives no drag numbers for any FPV frame and says to estimate them from the frame, then fit them to top speed and terminal speed. #16's first guess was front 9, side 9, top 25 cm². Worked out from the Quad definition's own collision shapes, each silhouette with a drag coefficient of 1, as the Freestyle 5″'s first guess used:

- **Front:** the duct rings, two side by side (each 40 mm across outside and 14 mm tall, their centres 23.3 mm either side of the middle), 11.2 cm²; the frame and canopy box (30 mm wide, 20 mm tall) adds the 3.9 cm² the rings don't cover; the pack hides behind it. About 15 cm².
- **Side:** the rings again, 11.2 cm²; the box (35 mm long) adds 4.4 cm²; the pack, longer than the box (64 mm), adds 1.7 cm² below the rings. About 17 cm².
- **Top:** the four rings' walls, 7.3 cm², plus the box and the pack where they lie outside the rings, 9.3 cm². The prop discs inside the rings are left out: the air through them is the rotors' own, which the inflow handles. About 17 cm².

They moved with a new source, logged in the Whoop 65's Feel Test log; a Feel Test settles them. The terminal speeds they give (17.1 m/s belly first, 18.3 m/s nose first) are in two Physics Scenarios.

**The Whoop 65's rotor drag: 0.3 s⁻¹, Estimate, range 0.1–0.6 s⁻¹, unchanged.** Faessler et al. measured 0.24–0.54 s⁻¹ on a 610 g quad with six-inch props (research §4.2). Nothing has been measured on a whoop, and its ducts add their own ram drag on top, 1.2 s⁻¹, four times as much. #16's 0.3 s⁻¹ sits inside the measured span, so it stays, re-sourced, until a Feel Test.

**The Freestyle 5″'s drag areas stay as they were,** a first guess from the frame's size; the TII logs ticket (#47) fits them, with its rotor drag, to those logs.

**What to watch in Feel Tests and the TII fit:**

- **Translational lift is momentum theory's ideal one.** Hoffmann et al.'s momentum theory at the same power (research §4.3) gives a whoop's rotor turning at its hover speed 26.9% more thrust with 8 m/s of air across its disc, and 7.8% with 4 m/s (at the 19,659 RPM that holds its weight at 8 m/s, 33.7% more than on the thrust stand). Faessler et al.'s fit to their quad (`k_h = 0.009 m⁻¹`) gives 5.9% and 1.5%: momentum theory gives 4.6 to 5.3 times as much, so Faessler's is about a fifth of it. Some of the difference is the pitch their quad flew at, which this model counts as a climb along the rotors' axes. If the whoop balloons too much at speed in a Feel Test, an Estimate for how much of the ideal lift a real rotor gets would be the fix; it would be a new number.
- **The ducts' nose-up moment is strong.** At 8 m/s level, the Whoop 65's ram drag (1.2 s⁻¹) acting 21 mm above its centre of mass lifts the nose with about 40% of the most its motors can pitch it the other way (rear pair at full drive, front pair stopped). Pereira's "much larger nose-up pitching moments" agree in kind; both numbers behind it are Estimates.
- **The Freestyle 5″'s props sit 5 mm below its centre of mass,** so its rotor drag dips its nose slightly at speed instead of lifting it.
- **The thrust guard has no source.** Where air arrives down through a rotor and across it at once, both far faster than the rotor's own still-air flow, momentum theory lets the thrust grow with both speeds multiplied. The air ticket caps it at the same descent's thrust plus ½ρA·(v_c² + μ²), an ideal windmill's most. The cap is a modelling choice the research doesn't give. It acts only in that corner: in units of the still-air flow, it trims about 2% at a descent of 1 across 1.2, 19% at 2 across 2, and 32% at 3 across 3. Fast dives with the motors low reach those.
- **Nearly stopped props still lift.** Descending with air across the disc, momentum theory's thrust stops depending on the rotor's speed as its power goes to nothing. It tends to the smaller of 2ρA·|v_c|·μ and ½ρA·(v_c² + μ²), an ideal autorotation. Only a prop at exactly 0 RPM gives none, and a coasting motor in the motor model never quite reaches 0. The Reviewer probed it: a Whoop 65 pitched 20° nose down and cut from a hover to 0% had its props at 25.8 RPM after 3 s, yet they gave 3.5 gf, 11% of its weight. It fell at 14.3 m/s against 16.2 m/s for a "stopped" start, and levelled itself. That may stand in roughly for windmilling props, which the motor model can't drive. Watch it in Prop Wash (#46), Failsafe and disarm (#52), Prop Strikes (#45) and Feel Tests.

## Prop Wash (#46)

Prop Wash makes the two `[feel]` numbers act (crates/physics/src/prop_wash.rs explains it, and the Physics Scenarios named `prop-wash-…` prove it). It adds no number to the Quad definition.

| Number | What it does |
|---|---|
| `prop_wash_strength` (20%, Estimate, range 10–30%) | At the band's middle, a rotor loses this share of its thrust on average, and its thrust flickers up to this share of what is left either way: from 64% to 96% of momentum theory's thrust. The research's cheap model (§4.4) has two numbers here, a loss and a flicker, both anchored on the same 10–30% thrust fluctuations; the Quad definition gives one, so both use it. |
| `prop_wash_flicker` (15 Hz, Estimate, range 5–40 Hz) | How many times a second each rotor's flicker glides to a new random level. No source; a Feel Test tunes it. |

**Where it acts.** A rotor sinking along its own axis at 0.4 to 1.4 times its hover flow `v_h = √(T / 2ρA)`, with `T` its own thrust (Johnson's band, research §4.4), and only while the air across its disc is under 2.75 times that descent (Talaeizadeh et al.'s rule). For a Whoop 65 rotor carrying a quarter of the weight, `v_h` is 5.70 m/s, so the band is a descent of 2.28–7.98 m/s; for a Freestyle 5″ rotor at its hover share, about 2.8–9.8 m/s (§4.4's Derived numbers). How strong it is inside the band (rising smoothly from either edge to the whole at its middle), and how it fades as the air across the disc nears 2.75 times the descent, are shapes with no source.

**What to watch in Feel Tests:**

- **Open loop, it is violent on a whoop.** With no Flight Controller to catch it, a Whoop 65 sinking straight down through the band's middle is rolled and pitched at about 400 °/s within half a second, and tips 15° in that time. The real test is with the Flight Controller and its delays, as ADR-0005 says.
- **Forward speed and momentum theory.** Sinking with air across its disc, momentum theory lets a rotor carry its share of the weight turning slowly (a drag-free Whoop 65 sinking at 3.5 m/s while flying at 3 times that holds its weight at 3,781 RPM). Prop Wash uses the thrust a rotor gives, not its speed, to place the band, so a rotor carrying the weight there is still inside it, and the 2.75 times rule alone lets it escape.
- **Nearly stopped props** (see the air section above) are not touched by it: their little thrust gives a small `v_h`, so any fall is far past their band.

## Both Quads

- **ESC numbers** (#26 §5, from Bluejay v0.21.0's source): a 100 ms wait before a restart (`wait100ms` between stall restarts), at most 3 restarts (the stall count is checked against 3), and start-up power capped at the default Startup Power Max, 5 on Bluejay's 0–255 scale, which is 1.96% of full drive while the motor starts. They are Manufacturer numbers: Bluejay's own defaults.
- **Prop Wash and ground effect** (research §4.4–§4.5): thrust flicker 20% (range 10–30%), flickering 15 times a second (range 5–40, with no published source), and a ground-effect body term of 2 (range 0–4).
- **Prop grip** 0.5 (range 0.2–0.8) and **reverse torque** 100% of forward (range 50–100%) come from #26.
- **The camera** (#14, #28): lens, FOV, Camera Tilt and VTX power are defaults with no Confidence; Dynamic Range, lines and sharpness are its limits, with a Confidence.
- **The sound block** (#32 §6, #34): the round-2 values from `prototype/quad-sound`'s `tuning/defaults.toml`, with readable names and units. The 5″ has a buzzer at 2.7 kHz; the whoop has none. Both play Bluejay's default start-up melody.

## Still guesses

Every Estimate above may move in a Feel Test (the whoop) or a fit to the TII logs (the 5″), inside its range. These rest on the least:

- **First guesses:** the Freestyle 5″'s body drag areas (the TII logs ticket fits them), the whoop's no-load current (which meets BetaFPV's table as it is), and the whoop's body drag areas and rotor drag, worked out or re-sourced by the air ticket but measured on no whoop.
- **Made from photos and frame sizes, with nothing measured:** every collision shape except the two pack boxes (each from its maker), both props' heights, both packs' heights, the duct rings, and both camera positions.
- **The battery curves:** typical LiPo and LiHV curves, not these packs' own.
- **The whoop's C_T and C_P,** until a Feel Test or the maintainer's recording settles the hover pitch.
- **Prop Wash's flicker speed and the ground effect's body term,** which have no source for these Quads.
- **The whoop's current at hover.** BetaFPV's table is read as the motor's own current, which the model meets; the pack's current at hover then looks low against the maintainer's 4–6 minutes (see the motor model section above). The hover-time check (#57) settles it.
