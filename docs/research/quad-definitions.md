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

**No-load current: 0.3 A at 4 V, Estimate, range 0.1–0.6 A.** BetaFPV doesn't publish it (research §8.5). It is #16's sample value, so its source says "first guess"; the motor ticket (#41) works it out.

**Shapes (Estimates from the layout):** the frame and canopy as a box of 35 × 30 × 20 mm, the props' plane about 8 mm above the centre of mass, the pack's centre about 6 mm below it (the 8.2 g pack hangs under the 23 g frame), and duct rings 37 mm inside (a 1 mm tip gap round a 35 mm prop), 1.5 mm thick and 14 mm tall. Nothing here was measured; photos and the sizes above set them. The camera position (15 mm forward, 12 mm up) is placed the same way; as a camera default it carries no Confidence.

**Battery:**
- Resistance 29 mΩ, Derived: the research's §5.3 worked it out from BetaFPV's LAVA II discharge curves.
- The open-circuit voltage curve is a typical LiHV curve, an Estimate within ×0.97–×1.03. Its ends must equal `full` (4.35 V, Manufacturer) and `empty` (3.30 V, Estimate 3.2–3.5 V), which the checker cross-checks.
- Recovery 3.3 s, Estimate 1–30 s: the one RC pair Bauersfeld & Scaramuzza fitted to 4S–6S packs (research §5.3). No whoop pack has been fitted.
- The BT2.0 connector's 10 mΩ is #10's Estimate.

**From the research's §9.1 and #10 §5, unchanged:** inertia (0.7, 0.9, 1.4) × 10⁻⁵ kg·m², motor lag 20–50 ms (35 ms both ways), duct ram drag 1.2 s⁻¹ (range 0.6–2.4) and the ducted rotor's centre of pressure 0.75 × 17.5 mm ≈ 13 mm higher (range 9–18 mm).

**Still placeholders from #16's sample,** marked "first guess": body drag areas (front 9, side 9, top 25 cm²), rotor drag 0.3 s⁻¹ and the no-load current. The air ticket (#42) and the motor ticket (#41) set them.

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

**Power coefficient: C_P 0.09, Estimate, range 0.07–0.11.** Through the motor model (torque = (current − no-load current) ÷ K_V in SI), the table gives C_P 0.073 at 60%, 0.088 at 80% and 0.106 at 100%: it doesn't hold still, because the model's torque constant and constant no-load current are only first-order. The middle value stands, with the spread as its range; the motor ticket fits it.

**Winding resistance: 0.20 Ω, Estimate, range 0.05–0.3 Ω.** With the drive at the throttle's share of 23.5 V, the lumped resistance that meets the table is 0.19 Ω at 100%, 0.22 Ω at 80% and 0.19 Ω at 60%. #10 fits it to the TII logs.

**Rotor inertia: 55 g·cm², Estimate, range ×0.5–×2.** A T5147 weighs about 4.5 g (0.2·m·R² ≈ 38 g·cm²) and a 2207's bell adds roughly 15–25 g·cm². The spin-up time this gives in a hover, about 28 ms, sits just under the 33 ms measured on NeuroBEM's 5″.

**Motor lag: 33 ms both ways, Measured.** #10 §5 lists the 5″'s motor lag as Measured on similar 5″ quads, 33–39 ms. The file takes 33 ms, NeuroBEM's time constant for its 752 g 5″ racing quad, which the research's §8.2 uses as the generic 5″ reference; Agilicious measured 39 ms on another quad. NeuroBEM fits one time constant both ways, so spin-up and slow-down share it. As a Measured number it is locked, and #10 §6's Thrust Stand Scenario checks it with a Source basis.

**Rotor drag: 0.4 s⁻¹, Estimate, range 0.24–0.54 s⁻¹.** #10 §5 also calls this "Measured on a similar quad", but Faessler et al. measured two numbers on their 610 g quad, 0.49–0.54 s⁻¹ forward and 0.24–0.39 s⁻¹ sideways, and the Quad file holds one. No single measured value exists to lock, and #10 refines it against the TII logs, so it is an Estimate whose range is the measured span.

**Reverse thrust: 48%, Measured.** #16 §4 and #26 §3 lock it: measured on a similar 5.1″ 3-blade prop.

**Mass:** 419 g dry (Manufacturer, iFlight) and a 225 g pack (Derived: iFlight's about 644 g with a 6S 1400 mAh pack, less 419 g), so 644 g in all, near #10's "about 650 g".

**Battery:** 6S, LiPo, charged to 4.20 V a cell, 1400 mAh. The pack's resistance is #10's "about 30 mΩ with leads", an Estimate, split here into 28 mΩ for the pack (range 15–45 mΩ) and 2 mΩ for the XT60 (range 1–5 mΩ). The curve is a typical LiPo curve ending at `full` and `empty` (3.50 V, Estimate 3.3–3.6 V); recovery is 3.3 s as for the whoop.

**Shapes and drag (Estimates from the layout):** the plates and stack as a box of 80 × 45 × 35 mm; the pack as a typical 6S 1400 mAh box of 75 × 35 × 40 mm, its centre 26 mm above the centre of mass (the 225 g pack on top lifts the centre of mass about 14 mm above the frame's); the props' plane about 5 mm below the centre of mass; body drag areas front 45, side 45 and top 100 cm² (each silhouette with a drag coefficient of 1). The air ticket fits the drag to the TII logs.

**Inertia: (14, 15, 25) kg·cm², Estimate, range ×0.7–×1.8:** #10's (1.4, 1.5, 2.5) × 10⁻³ kg·m². The range reaches NeuroBEM's heavier 6″ quad, (2.5, 2.1, 4.3) × 10⁻³ kg·m².

**The gyro:** typical 5″ boards carry an ICM-42688-P or a BMI270, both ±2000 °/s.

## Both Quads

- **ESC numbers** (#26 §5, from Bluejay v0.21.0's source): a 100 ms wait before a restart (`wait100ms` between stall restarts), at most 3 restarts (the stall count is checked against 3), and start-up power capped at the default Startup Power Max, 5 on Bluejay's 0–255 scale, which is 1.96% of full drive while the motor starts. They are Manufacturer numbers: Bluejay's own defaults.
- **Prop Wash and ground effect** (research §4.4–§4.5): thrust flicker 20% (range 10–30%), flickering 15 times a second (range 5–40, with no published source), and a ground-effect body term of 2 (range 0–4).
- **Prop grip** 0.5 (range 0.2–0.8) and **reverse torque** 100% of forward (range 50–100%) come from #26.
- **The camera** (#14, #28): lens, FOV, Camera Tilt and VTX power are defaults with no Confidence; Dynamic Range, lines and sharpness are its limits, with a Confidence.
- **The sound block** (#32 §6, #34): the round-2 values from `prototype/quad-sound`'s `tuning/defaults.toml`, with readable names and units. The 5″ has a buzzer at 2.7 kHz; the whoop has none. Both play Bluejay's default start-up melody.

## Still guesses

Every Estimate above may move in a Feel Test (the whoop) or a fit to the TII logs (the 5″), inside its range. These rest on the least:

- **Placeholders from #16's sample:** both Quads' body drag areas, the whoop's rotor drag and the whoop's no-load current.
- **Made from photos and frame sizes, with nothing measured:** every collision shape except the whoop's pack box, both props' heights, both packs' heights, the duct rings, and both camera positions.
- **The battery curves:** typical LiPo and LiHV curves, not these packs' own.
- **The whoop's C_T and C_P,** until a Feel Test or the maintainer's recording settles the hover pitch.
- **Prop Wash's flicker speed and the ground effect's body term,** which have no source for these Quads.
- **What Bluejay's start-up power cap does.** The number is locked, because it is Bluejay's own default (Startup Power Max, 5 of 255), stored as the share of full drive it caps a starting motor at. How that cap acts on a stalled motor is the ESC ticket's to check against Bluejay's code.
- **The 5″'s pack box.** #26 §5 takes a pack's size from its product page, but #10 names no particular 6S 1400 mAh pack, so the box is a typical pack's size, as an Estimate.
