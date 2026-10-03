# How serious simulators model quad flight dynamics, including whoop-specific effects

Research for [#4](https://github.com/BartoszSolkaBD/OpenDrone/issues/4), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). It feeds [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10) (which physics effects go into the alpha) and [#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12) (crate split). Checked against primary sources on 2026-10-03.

**Question:** what physics model does a faithful FPV simulator need, and which parameters define a Quad?

Vocabulary follows [CONTEXT.md](../../CONTEXT.md), especially [Flying](../context/flying.md) (Quad, Flight Controller, Assist) and [Verification](../context/verification.md) (Scenario).

**Evidence tags.** Untagged claims cite a primary source: a paper, a thesis, a NASA report, source code, official docs or a manufacturer page.
- **Derived**: my own arithmetic from cited numbers.
- **Estimate**: an engineering guess with no direct source.
- **Community**: a forum post, blog or retailer listing.
- **Unverified**: no primary source could be reached.

Sources are keyed `[S#]` and listed at the end.

## Short answer

- **The core is well established and cheap.** Every serious simulator, open or commercial, uses the same backbone:
  - a 6-degree-of-freedom rigid body;
  - per-rotor thrust and drag torque that grow with the square of rotor speed;
  - a motor that takes tens of milliseconds to reach a new speed;
  - a few drag terms.

  Together this costs around a microsecond per step (Derived). At 1 kHz it uses well under 1% of one CPU core. **Physics cost will never be the bottleneck. Collision queries will be.**
- **1 kHz is numerically safe**, provided:
  - attitude is integrated with the exponential map (or a quaternion that is renormalised every step);
  - motor lag is updated with its exact exponential formula.

  Agilicious runs RK4 at 1 kHz [S26]. AirSim's paper targets 1 kHz [S35]. Most research sims run slower (250–400 Hz). Because the model is so cheap, sub-stepping to match Betaflight's 4–8 kHz PID loop is affordable. See [Surprises](#surprises-that-affect-other-tickets).
- **What separates a good FPV sim from a robotics sim is the "feel" layer** that research sims mostly skip:
  - battery sag through a voltage-dependent motor model;
  - motor lag that differs on spin-up and spin-down;
  - rotor drag that makes a quad carve rather than slide;
  - prop wash when you fall into your own air;
  - for the whoop, duct drag and the duct's nose-up moment.

  Velocidrone markets the full chain: battery → ESC → motor → NACA-profile prop, plus a simulated Betaflight 4.2 Flight Controller [S43]. That is the bar.
- **Prop wash is the hardest effect and the one pilots judge by.** No vendor publishes a physical model of it.
  - Liftoff's developer states that its prop wash is "an artificial effect" with a default strength of 42%. Their auto-tuning Flight Controller had cancelled the real effect [S42].
  - The real effect comes from two things together: turbulent air when a quad drops into its own wake, and the imperfect way a real Flight Controller reacts to it (filter delay, motor lag, limited authority at low throttle).
- **The whoop needs its own data. Big-prop numbers can't be scaled down.**
  - Its props run at a blade Reynolds number of about 16,000, against about 100,000+ on a 5" (Estimate). At that scale efficiency drops and coefficient curves change shape [S20].
  - Its ducts add strong drag and a nose-up moment in forward flight and crosswind [S16].
  - BetaFPV publishes thrust tables for its whoop motors [S49]. Nobody publishes whoop inertia, motor time constant, duct coefficients or motor resistance. These must be estimated, then tuned by feel.
- **The ticket's premise about the Meteor 65 was off.** The Meteor65 family uses **0802SE 19500KV** motors, not 0702 (the 0702 motors are on the Air65) [S49].
  - The Meteor65 Pro uses 35 mm props.
  - A **Meteor65 Pro II** launched in 2026 with new 0802 motors and a 70 mm frame [S49].
  - We need to know which version the maintainer flies.
- **Which parameters define a Quad:** a few dozen numbers in seven groups: frame, rotor/prop, motor, ESC, battery, duct and feel parameters. See [section 9](#9-what-defines-a-quad). Each needs a recorded source and confidence level, because many whoop values are estimates.

## 1. Catalogue of candidate physics effects

This is the ticket's deliverable.

**How to read the columns:**
- **Importance to feel** is my judgement of how much a pilot who flies the real Quad would notice the effect's absence.
- **Compute cost** is per physics step for one Quad:
  - *Negligible*: under ~50 floating-point operations.
  - *Low*: 50–500 operations, or one table lookup.
  - *Medium*: needs a query against the world, such as a ray cast.
  - *High*: an iterative or per-blade-element model, or collision solving.
- **Suggested tier** is input for [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10), not a decision:
  - **Core**: needed for a faithful alpha.
  - **Recommended**: cheap and noticeable.
  - **Later**: small payoff, or no data yet.

### 1.1 Rigid body and integration

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 1 | 6-DOF rigid body, including gyroscopic coupling `ω × Jω` [S1, S2] | Everything: momentum, flips, how rotation carries | Critical | Negligible | Mass: manufacturer (measured). Inertia: **not published for any target Quad**. Estimate by adding up components. The 5" can be checked against NeuroBEM's measured inertia [S6] | Core |
| 2 | Attitude integrated by exponential map (or quaternion plus renormalisation) | Correct rates at 1,800–2,000 °/s, no slow drift | Critical (correctness) | Negligible | None | Core |
| 3 | Fixed step ≥ 1 kHz, semi-implicit Euler or RK4 | No visible effect when right; jitter or blow-ups when wrong | Critical (correctness) | Negligible (RK4 ≈ 4×) | None | Core |

### 1.2 Propulsion: props, motors, ESC

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 4 | Per-rotor thrust `k_f·ω²` and drag torque `k_m·ω²` [S21] | Hover throttle, punch, yaw authority | Critical | Negligible | Thrust stand. Whoop: BetaFPV tables. 5": T-Motor and MiniQuadTestBench [S49–S51]. Torque (`k_m`) is published for neither, so estimate it from motor current | Core |
| 5 | Motor lag: first order, separate spin-up and spin-down time constants [S34] | Snappiness, overshoot and bounce-back at the end of flips, how fast the quad recovers | Critical | Negligible (exact exponential update) | 5": **measured 33 ms** (NeuroBEM [S6]) and **39 ms** (Agilicious [S26]); MiniQuadTestBench has spin-up traces. Whoop: **unpublished**. Nearest proxy is the Crazyflie Brushless at 50 ms [S25]. Tune by feel | Core |
| 6 | Motor speed depends on voltage (Kv, winding resistance, no-load current, back-EMF) [S22]. Throttle is a duty command, not a speed command [S28] | Punch fades as the pack drains and under sag. Idle speed changes with voltage | High | Negligible (algebraic) | Manufacturer. 5" motors publish Kv, R and I₀. Whoop motors publish Kv only | Core |
| 7 | Rotor inertia: yaw reaction torque while motors change speed [S2] | Sharp start and stop of yaw. Yaw twitch on punch | Medium | Negligible | Estimate from prop and motor-bell mass | Recommended |
| 8 | Rotor gyroscopic precession | Faint pitch–yaw coupling in fast flips; mostly cancels across counter-rotating pairs | Low | Negligible | Estimate | Later (trivial to add) |
| 9 | ESC behaviour: active braking ("damped light"), idle, duty slew limit [S30] | How quickly motors slow down, which sets control authority at zero throttle and hang time in flips | Medium | Negligible | Firmware docs. The Meteor65 Pro ships with Bluejay 96k [S49]. Model it as the spin-down time constant | Recommended |
| 10 | Thrust falls with axial inflow (momentum theory, or `C_T(J)` from the advance ratio) [S4, S19] | Thrust fades in hard climbs and fast punch-outs, and bites harder when descending | High | Low (~20 operations per rotor, closed form) | Prop diameter and static thrust. UIUC curves give the shape [S21]. No whoop wind-tunnel data exists | Core |
| 11 | Translational lift (edgewise inflow) [S4, S5] | At constant throttle the quad gains lift at speed | Medium | Negligible | Faessler: `k_h = 0.009 m⁻¹` on a 610 g quad [S5], or momentum theory | Recommended |
| 12 | Full blade-element momentum (BEM) per rotor [S6] | Better force accuracy at high speed | Low (extra over #10–11) | High (~10³ operations per rotor, iterative) | Blade geometry, which is not published for our props | Later. Better: compute it offline into tables |

### 1.3 Battery

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 13 | Battery model [S7, S27]: <ul><li>open-circuit voltage against charge used</li><li>internal resistance</li><li>one resistor–capacitor (RC) pair for slow sag and recovery</li><li>charge counting</li></ul> | Punch fades through the pack. Sag on punch, recovery after a chop, flight time | High | Negligible, plus one table lookup | <ul><li>Whoop: BetaFPV discharge curves give **29–60 mΩ** effective (Derived) [S49].</li><li>Curve shape: Bauersfeld & Scaramuzza's fit to 10 packs [S7].</li><li>6S resistance: Community figures only.</li></ul> | Core |
| 14 | Lead and connector resistance | Extra whoop sag: BetaFPV says BT2.0 sags 0.2 V less than PH2.0 at 9 A, about 22 mΩ (Derived) [S49] | Medium (whoop) | Negligible | Manufacturer note | Core (one number) |
| 15 | Temperature and high-rate capacity loss | Cold packs sag more | Low | Negligible | No FPV-specific data | Later |

### 1.4 Aerodynamics

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 16 | Rotor drag (H-force): force ∝ in-plane airspeed × total rotor speed [S5, S8] | The quad slows itself when levelled. It carves turns instead of sliding like a puck | High | Negligible | Measured on a 610 g quad with six-inch props: `d_x = 0.49–0.54 s⁻¹`, `d_y = 0.24–0.39 s⁻¹` (mass-normalised) [S5]. Fit the 5" to the open TII flight logs. Whoop: estimate | Core |
| 17 | Body drag: quadratic, frontal area depends on attitude [S7] | Top speed, coasting after a chop, terminal speed in dives | High | Negligible | No published drag coefficients or areas. Estimate from frame dimensions, then fit to top speed and terminal velocity | Core |
| 18 | Blade-flapping moment [S4] | Nose-up tendency in fast forward flight | Medium | Negligible | Fold it into rotor drag. Literature only | Recommended |
| 19 | Prop wash: vortex ring state (VRS) and turbulent wake when descending into your own air [S4, S10, S11] | Shaking when dropping into your own wash, wobble after chop-and-dive moves. **Pilots judge sims on this** | High | Low (a noise generator per rotor) | Partial: <ul><li>VRS boundaries from Johnson and Hoffmann</li><li>≥ 10% thrust fluctuation measured in a wind tunnel [S11]</li><li>Noise bandwidth and strength have **no source**: tune by feel</li></ul> | Core. Feel also depends on the Flight Controller (§4.4) |
| 20 | Ground effect, multirotor version [S12] | Floaty landings, a cushion when skimming the floor, a pitch kick when flying off an edge | Medium | Medium (one ray down per rotor) | Geometry only: rotor radius and spacing, plus an empirical body term `K_b ≈ 2` | Recommended |
| 21 | Ceiling effect [S13, S14] | The whoop gets sucked onto ceilings indoors | Medium (whoop in Bando), Low (5") | Medium (one ray up) | Closed-form formula from geometry | Recommended for the whoop |
| 22 | Wall effect [S15] | Pulled towards walls | Low | Medium | **No verified closed-form model** | Later |
| 23 | Rotor–rotor and rotor–frame interaction [S6] | Small force and torque errors | Low | High (needs a data-driven model) | NeuroBEM's learned residual (no licence) | Later |
| 24 | Wind and gusts (Dryden model) [S54] | Drift and bumps outdoors | Low for the alpha | Low | MIL-F-8785C (secondary summary) | Later, or a Map setting |

### 1.5 Whoop-specific

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 25 | Duct "ram" drag: drag ∝ air mass flow × side airspeed [S16] | The whoop stops quickly, has a low top speed and feels draggy | High (whoop) | Negligible | Momentum-theory estimate: about **1.2 s⁻¹** mass-normalised for a Meteor65 Pro, against about 0.9 s⁻¹ for the same props without ducts (Derived). Tune by feel | Core (whoop) |
| 26 | Duct nose-up moment in forward flight and crosswind [S16] | The nose lifts when you push forward, the whoop balloons, and gusts shove it | High (whoop) | Negligible | Pereira: a ducted rotor's centre of pressure sits about **0.75 R** above an open rotor's, giving "much larger nose-up pitching moments" [S16]. Size of the effect: Estimate | Core (whoop) |
| 27 | Duct thrust gain in hover [S16, S18] | Hover efficiency | Low | None | Folded into the thrust table. **Unknown whether BetaFPV's stand tests use the duct** | Fold into #4 |
| 28 | Low Reynolds number [S19, S20] | Lower efficiency and different curve shapes | Indirect: affects parameter choice, not a runtime term | None | Rule: use measured whoop data and never scale 5" coefficients | Rule |

### 1.6 Contact and sensors

| # | Effect | What the pilot feels | Importance | Compute | Parameter source | Tier |
|---|---|---|---|---|---|---|
| 29 | Collisions with the world: contact, friction, restitution, continuous collision detection (CCD) [S38] | Crashes, bounces, skids. Without CCD, passing through thin rails at speed | Critical | **Highest of all**, though still small against the frame | Restitution and friction tuned by feel. AirSim defaults are 0.55 and 0.5 [S35] | Core |
| 30 | Prop strikes and damage | Losing a motor after clipping something | Medium | Low | The map lists "Crashes" as not yet specified | Later |
| 31 | Frame vibration and gyro noise fed to the Flight Controller | Shapes the Flight Controller's filter delay, which is part of how prop wash feels | Medium | Negligible | 5": noise spectrum from public logs (TII, CC BY 4.0). Whoop: estimate | Recommended (ties to [#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5)) |
| 32 | Air density (altitude, temperature) | Less thrust at altitude | Low | Negligible | Standard atmosphere | A constant in the alpha |

**Total cost (Derived, not benchmarked):**
- Effects 1–21 and 25–26 together come to roughly 1–3 µs per step on a modern core.
- At 1 kHz that is under 0.3% of one core. At 8 kHz it is under 3%.
- The ray casts for ground and ceiling effect and the collision solve will cost more than all the flight physics combined.
- Check with a `criterion` benchmark once code exists.

## 2. Rigid body and integrators

### 2.1 Equations

The standard model [S1] is:
- position: `ẋ = v`
- translation: `m·v̇ = m·g·e₃ − f·R·e₃` (plus aerodynamic forces)
- attitude: `Ṙ = R·Ω̂`
- rotation: `J·Ω̇ + Ω × J·Ω = M`

Tal & Karaman [S2] add the rotor terms:
- Motor thrust and torque become `[μ; T] = G₁·ω² + G₂·ω̇`.
- `G₂` carries rotor inertia. This is the yaw "kick" when motors change speed.
- They drop rotor gyroscopic precession as small.

Flightmare drops the rotor terms too (`flightlib/src/dynamics/quadrotor_dynamics.cpp`) [S31].

### 2.2 What other simulators use

| Simulator | Integrator | Physics rate | Attitude update | Source |
|---|---|---|---|---|
| Agilicious (UZH) | RK4 (Euler optional) | **1 kHz** | — | [S26] |
| Flightmare | RK4, sub-steps ≤ 2.5 ms inside the control step | 400 Hz minimum | Quaternion, renormalised every step | `flightlib/src/objects/quadrotor.cpp`, `integrator_rk4.cpp` [S31] |
| AirSim | Velocity Verlet: trapezoid rule on velocity, averaged angular rate applied as an angle-axis rotation | Paper targets 1 kHz. Shipped default is 3 ms (333 Hz). Comments: "300Hz seems to be minimum for non-aggressive flights… 500Hz is recommended for more aggressive flights" | Exponential map, then normalise | `FastPhysicsEngine.hpp`, `SimModeWorldBase.h` [S35] |
| gym-pybullet-drones | PyBullet, or semi-implicit Euler in `DYN` mode | 240 Hz default | Closed-form quaternion exponential (`_integrateQ`) | `BaseAviary.py` [S32] |
| PX4 + Gazebo | ODE solver | 250 Hz (`max_step_size 0.004`) | Engine-managed | [S34] |
| RotorS | ODE solver | 100 Hz in the example world. Rotor joints spin 10× slower than real to avoid aliasing | Engine-managed | `basic.world`, `common.h` [S33] |
| Peng (Rust) | Semi-implicit Euler (default) or RK4 | 1 kHz | `UnitQuaternion::from_scaled_axis(ω·dt)` (exponential map) | `config/quad.yaml`, `src/lib.rs` [S37] |
| Betaflight (the reference Flight Controller) | — | Gyro 3.2–8 kHz; PID loop 2, 4 or 8 kHz | — | [S28] |

### 2.3 Numerical stability at 1 kHz

**Motor lag is the stiffest part of the model.** A first-order lag `τ·ẋ = u − x` behaves as follows (standard stability result, Derived):
- explicit Euler is stable only while `dt < 2τ`;
- RK4 is stable while `dt ≲ 2.8τ`;
- the exact update `x += (u − x)·(1 − e^(−dt/τ))` is always stable. Flightmare and PX4 both use it [S31, S34].

Measured motor time constants are 30–72 ms [S6, S24, S25, S26, S56], so `dt/τ ≤ 0.034` at 1 kHz. There is no stiffness problem. A full electrical motor model, with winding inductance, would bring a sub-millisecond time constant and force much smaller steps. That is not worth it (§3.2).

**Attitude at high spin rates (Derived):**
- At 2,000 °/s and 1 kHz the quad turns 2° per step.
- A naive quaternion update grows the quaternion's length by about 0.015% per step, which is about 16% per second if never renormalised.
- Renormalising leaves an angle error of about `θ³/12` per step, roughly 0.2 °/s.
- **The exponential-map update is exact for a constant rotation rate.** It costs one `sin`, one `cos` and one `sqrt`. Use it.
- Bullet takes the same approach. It uses the exponential map and caps rotation at 45° per step, a cap that never bites at 1 kHz (`btTransformUtil.h`) [S41].

**Choice of integrator:**
- Semi-implicit Euler "tends to preserve energy on average" and is the game-physics default [S40].
- RK4 is more accurate per step but slowly loses energy [S40], and costs 4× as much.
- At 1 kHz both are fine. Semi-implicit Euler with the exponential map and exact motor lag is the simplest stable choice. RK4 is an affordable upgrade if a Scenario shows drift.

**Sub-stepping is normal practice.** Flightmare, PyBullet and gym-pybullet-drones all run physics faster than control. The [#2 resolution](https://github.com/BartoszSolkaBD/OpenDrone/issues/2) already plans in-crate sub-steps inside Bevy's fixed 1 kHz tick.

### 2.4 Determinism

- Rust's `sin`, `cos` and `exp` are documented as platform-dependent in precision [S39].
- The model above uses `exp` for motor lag and `sin`/`cos` for attitude. These must go through `libm`, as the [determinism research](https://github.com/BartoszSolkaBD/OpenDrone/blob/research/determinism/docs/research/determinism.md) for #7 already recommends.
- Rapier offers cross-platform determinism with its `enhanced-determinism` feature, IEEE-compliant targets and identical insertion order [S38].

## 3. Motors, props and ESC

### 3.1 Props

**The standard model** [S21]:
- `T = C_T·ρ·n²·D⁴`
- `Q = C_P/(2π)·ρ·n²·D⁵`
- with ω in rad/s these become `T = k_f·ω²` and `Q = k_m·ω²`.

**`C_T` and `C_P` fall as forward or climbing speed rises.** Both fall monotonically with the advance ratio `J = V/(nD)`, reaching zero thrust at the windmill state [S19]. UIUC only measures axial inflow. Edgewise flow needs momentum theory or BEM [S6].

**How simulators implement it:**

| Simulator | Thrust model | Notes |
|---|---|---|
| PX4 / RotorS | `motor_constant·ω·\|ω\|` | Linear fade to zero thrust at 25 m/s airspeed (`max_relative_airspeed`) |
| Flightmare | `a·ω² + b·ω + c` polynomial | |
| gym-pybullet-drones | `KF·rpm²` | No motor lag at all |
| AirSim | UIUC `C_T`, `C_P` | |

Sources: [S31–S35]. **None of the four models how `C_T` changes with `J`** beyond PX4's fade.

**Measured whoop and 5" coefficients (Derived from manufacturer tables):**

| Setup | `C_T` | `k_f` (N/(rad/s)²) | Source |
|---|---|---|---|
| BetaFPV 0802 (2026) + GF1614 40 mm | ≈ 0.29 | ≈ 2.3 × 10⁻⁸ | [S49] |
| T-Motor Velox 2207 + T5147 5" | ≈ 0.19 | ≈ 1.5–1.65 × 10⁻⁶ | [S50] |

- The whoop value is flat from 50% to 100% throttle. The 5" value stays within about 10% from 20% to 100% throttle. So the quadratic model fits both.
- The whoop `C_T` is high for a free prop. That fits either a high-solidity 3-blade prop or a stand test done inside a duct. BetaFPV doesn't say which.

### 3.2 Motors

**Drela's first-order model** [S22]:
- torque `Q_m = (i − i₀)/K_Q`
- back-EMF `v_m = Ω/K_V`
- supply `v = v_m + i·R`

**Drive voltage is set by duty, not speed.** With BLHeli-, Bluejay- or AM32-style drive, `v = duty × V_battery`, so full-throttle speed follows battery voltage. Betaflight's DShot values (48–2047) are drive commands [S28]. AM32's closed-loop speed control is optional and off by default [S30].

**Spin-up and spin-down differ (Derived from Drela's model).**
- Linearised, the time constant is `τ ≈ J_r / (1/(R·K_V²) + 2·k_q·ω₀)`.
- With active braking ("damped light"), back-EMF also slows the motor, so spin-down is quick.
- When freewheeling, only prop drag slows it, so spin-down is much slower. The worked example for a 2306 motor: about 17 ms up against about 84 ms freewheeling down.
- PX4 and RotorS use separate up and down constants: 12.5 ms and 25 ms by default [S34].

**Measured response times:**
- 5" quads: 33 ms [S6] and 39 ms [S26].
- 0.75 kg quad: 30 ms [S56].
- Crazyflie 2.x (brushed): 72 ms [S24].
- Crazyflie Brushless (08028 motors, 44 g): 50 ms [S25].
- Gauthier et al. found real step responses are a "double lag", with the ESC adding a lag "substantially longer than the time constant of the electrical circuit alone" [S23].
- **No whoop (0702/0802 on Bluejay) measurement exists.**

**A full electrical model is not worth it.** The winding inductance's time constant is likely under a millisecond (Estimate), and commutation and PWM switching run at 24–96 kHz [S30]. Either would need much smaller steps for no feel gain. The quasi-static form, setting inductance to zero to get `i = (d·V − ω/K_V)/R`, is algebraic and cheap.

### 3.3 ESC and the Betaflight mixer

**Betaflight settings that interact with the motor model** [S28]:
- `dshot_idle_value`: idle drive, default 5.5%. Resulting idle speed depends on voltage.
- `motor_output_limit`.
- `thrust_linear`: `pid.c`, `pidApplyThrustLinearization`.
- `vbat_sag_compensation`: about 16% motor-range reduction on a full pack.
- Dynamic idle: `dyn_idle_min_rpm`.
- Airmode: mix authority halves at zero throttle without it.

**The thrust curve formula changed.** Per the [#6 research](https://github.com/BartoszSolkaBD/OpenDrone/issues/6), Betaflight moved to ArduPilot's thrust-curve formula in May 2026. Our throttle-to-thrust model should take the same parameter, so pilots' real settings carry over.

**Bidirectional DShot.** The ESC reports eRPM, and `erpm / (poles/2)` gives RPM. Betaflight's RPM filter uses it. The motor model should expose RPM and pole count (default 14; often 12 on small motors) [S28].

These are Flight Controller concerns ([#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5)). They matter here only because the physics must expose the right signals.

## 4. Aerodynamics

**Reference numbers (Derived, ρ = 1.225 kg/m³).** The hover induced velocity is `v_h = √(T/(2ρA))` per rotor [S4]:

| Quad | Mass | Prop | `v_h` |
|---|---|---|---|
| Meteor65 Pro | 31.3 g | 35 mm | ≈ 5.7 m/s |
| Original Meteor65 | 30.5 g | 31 mm | ≈ 6.4 m/s |
| 5" | 650 g | 5.1" | ≈ 7.0 m/s |

**Both Quads hit prop wash at similar descent speeds in m/s.**

### 4.1 Body drag

- **Model:** `f = −½·c·ρ·|A·v|·v` in body axes, where `A` holds the front, side and top areas. This makes drag attitude-dependent at no extra cost [S7].
- No paper gives numbers for an FPV frame. Svacha et al. fold lumped drag into their model, but only the abstract was reachable [S55].
- **Fitting:** fit to terminal velocity (a falling-Quad Scenario, already proposed in #6) and to top speed from the TII logs.

### 4.2 Rotor drag and blade flapping

- **Model:** `F = −λ·(Σωᵢ)·v_⊥`: linear in in-plane airspeed and proportional to total rotor speed [S8]. Mahony et al.'s survey uses the same linear form [S3].
- gym-pybullet-drones implements exactly this in `_drag` [S32]. Faessler et al. lump flapping and induced drag into `f = −R·D·Rᵀ·v` [S5].
- **Measured:** `d_x = 0.491–0.544 s⁻¹`, `d_y = 0.236–0.386 s⁻¹`, `d_z ≈ 0`, on a 610 g quad with stiff six-inch props [S5].
- Hoffmann et al. show that flapping also "tips the thrust vector", giving a nose-up moment in forward flight [S4].
- **Feel:** this term is why a real quad slows itself and carves turns. Without it, a sim feels "floaty" or "on ice".

### 4.3 Induced inflow, translational lift and BEM

**Climb and descent.** In a climb, `v_i = −v_c/2 + √((v_c/2)² + v_h²)` [S4]. Thrust per unit power falls as climb rate rises. A cheap closed-form version:
- `T = k_f·ω² − k_v·ω·(V_axial + Δv_i)`, from linearised blade-element theory (Unverified textbook form);
- or interpolate a `C_T(J)` table.

**Translational lift.** Forward speed raises thrust at constant power. "At high speeds, the flight dynamics resemble those of an airplane" [S4]. Faessler fits `T = c + k_h·v_h²` with `k_h = 0.009 m⁻¹` [S5].

**NeuroBEM** [S6] used 1 h 15 min of flight from a 752 g 5" racing quad at up to 18 m/s:
- **BEM alone misses rotor-frame interactions.** Its torque error was *worse* than predicting zero.
- **BEM plus a neural network** cut force and torque errors by about 50% against prior models.

| Model | Horizontal force error (RMSE) |
|---|---|
| Linear drag + quadratic thrust | 1.54 N |
| BEM | 0.80 N |
| BEM + neural network | 0.20 N |

**Recommendation (Estimate):**
- Use the closed-form inflow terms at runtime. Bangura et al. derive them from momentum and blade-element theory [S9].
- If BEM is wanted later, compute it offline into tables over (advance ratio, disk angle, RPM).
- Per-step BEM is affordable in Rust but buys little.

### 4.4 Prop wash and descending into your own air

**Flight regimes.** These follow Johnson's *Helicopter Theory*, as reproduced in Hoffmann et al. [S4]:
- normal working state for climb ratio `v_c/v_h ≥ 0`;
- **vortex ring state** for `−2 ≤ v_c/v_h < 0`;
- windmill brake below −2.
- Inside VRS, induced velocity "varies greatly, particularly over the domain −1.4 ≤ v_c/v_h ≤ −0.4".
- **For our Quads that is a vertical descent of about 2.3–8 m/s for the whoop and 2.8–9.8 m/s for the 5" (Derived).** That matches where pilots feel prop wash.

**Measurements and models:**
- Johnson's NASA report gives a VRS model as a "parametric extension of momentum theory… suitable for simple calculations and real-time simulations" [S10].
- Talaeizadeh et al. measured thrust fluctuations of **≥ 10%** in VRS and windmill-brake states in a wind tunnel [S11].
- Their safe-descent rule: horizontal speed at least ~2.75× the descent rate. That is why pilots escape prop wash by adding forward speed.

**"Prop wash" as FPV pilots use the word** (wobble after a chop-and-dive or a split-S) **has no peer-reviewed source.** It is best explained as:
1. the wake disturbance above,
2. hitting a quad whose rotors are slow (low authority),
3. while the Flight Controller's filters and the motor lag delay the correction.

**Liftoff's experience confirms point 3 matters** [S42]. Once their auto-tuning Flight Controller "compensates perfectly for forces", the natural prop wash vanished, and they added an artificial on/off effect at 42% strength.

**Cheap model (Estimate).** Per rotor:
- `s` = descent along the rotor axis ÷ `v_h`;
- `μ` = in-plane speed ÷ `v_h`;
- intensity `I = bump(s; 0.4…1.4) · exp(−(μ/0.7)²)`;
- thrust `Tᵢ = T_nom·(1 − k_loss·I)·(1 + σ·I·nᵢ(t))`;
- `nᵢ(t)` is independent band-limited noise for each rotor. Because each rotor's noise differs, the quad also gets roll and pitch torque noise.

Anchors:
- `k_loss ≈ 0.1–0.3` and `σ ≈ 0.1–0.3` come from the ≥ 10% fluctuations [S11] and Washizu's ΔT/T = 0.15–0.30 contours, cited in [S10].
- **The noise bandwidth has no source and must be tuned by feel.**
- It is a Quad parameter, not a Setting, so it stays physics, not an Assist.

### 4.5 Ground, ceiling and wall effect

**Single rotor (Cheeseman–Bennett):** `T_IGE/T_OGE = 1/(1 − (R/4z)²)`:
- +6.7% at one rotor radius above the ground;
- +1.6% at two radii.

**Multirotor version.** Sanchez-Cuevas, Heredia and Ollero [S12] add terms for neighbouring and diagonal rotors plus a body-lift term with `K_b ≈ 2`:
- The effect stays significant up to **about 5 rotor radii**, against about 2 for helicopters.
- That is ~9 cm for the whoop and ~32 cm for the 5" (Derived).
- Only some rotors over a surface (partial ground effect) produce a disturbing moment.
- gym-pybullet-drones' `_groundEffect` uses a coefficient about 11× the single-rotor term, consistent with this [S32].

**Ceiling:** `T/T_free = ½ + ½·√(1 + 1/(8·(z/R)²))`, roughly +11% at half a radius [S14].
- In measurements a 23 mm-radius prop went from 0.18 N to 0.45 N at 1 mm from the ceiling [S13].
- This is positive feedback, which is why whoops stick to ceilings.

**Wall:** the craft tends to "pitch towards the wall and be drawn into it" [S15]. **No verified closed-form model**, so this is left for later.

**Cost.** One short ray cast per rotor, or one per Quad with an analytic plane. It is the only aerodynamic effect that needs a world query.

### 4.6 Wind and turbulence

- The Dryden model (MIL-F-8785C) gives gust spectra for outdoor flight [S54].
- Indoor whoops don't need it.
- For the alpha, wind is a candidate Map setting rather than a core effect.
- It matters most for the whoop, whose ducts turn gusts into nose-up moments (§5.1).

## 5. Whoop-specific effects

### 5.1 Ducts

**Hover.** Pereira's University of Maryland thesis [S16] is the most thorough source:
- In ideal momentum theory, a straight duct gives **+26% thrust at the same power** (or −29% power for the same thrust).
- A well-designed micro-air-vehicle-scale shroud gave up to +94% thrust at the same power, with a 0.1% tip gap and a lip radius of 13% of the duct diameter.
- Raising the tip gap from 0.1% to 1.6% cut the duct's thrust. Historical data puts the loss at about 20% of static thrust between 0.3% and 2% gaps. NASA's ducted-fan tests also varied the gap, from 1% to 4.5% of radius [S17].

**Whoop ducts have thin lips and loose gaps (Unverified), so expect little hover gain.** A 2026 study of a shrouded Crazyflie supports this: thrust out of ground effect was only "comparable", and flight time fell by 30% [S18].

**Forward flight and crosswind** [S16]:
- The ducted rotor's centre of pressure sits about **0.75 R higher** than an open rotor's.
- Its side force grows much faster with airspeed, so the duct produces "much larger nose-up pitching moments".
- Drag in edgewise flow is mostly momentum ("ram") drag: 80–95% of the total at low speed in the studies cited. Ram drag grows linearly with speed.
- In gusts, the duct tends to "tilt the thrust vector towards the downwind direction".

**Cheap model (Estimate):**
- Per duct, ram drag `D = ṁ·V_⊥`, with air mass flow `ṁ ≈ √(ρ·A·T)` for an ideal straight duct.
- It produces a nose-up moment `D·(h_lip + 0.75R)` about the centre of gravity.
- For a Meteor65 Pro at hover this gives about **1.2 s⁻¹** of linear drag, mass-normalised. The same props without ducts would give about 0.9 s⁻¹ (Derived).
- Both are higher than the 0.24–0.54 s⁻¹ measured on a 610 g quad [S5]. That matches the "stops fast, low top speed" whoop feel.
- **These are starting values to tune by feel**, since no duct coefficients are published for any whoop.

### 5.2 Low Reynolds number

**Estimated blade Reynolds numbers** (Estimate, assuming chords of 5 mm and 12 mm):
- Whoop: about 16,000 at 40,000 RPM.
- 5": about 100,000–120,000 at 25,000–30,000 RPM.

**What the data says:**
- Deters, Ananda and Selig tested props from 2.25" to 9": "As the Reynolds number increases, the thrust coefficient increases, the power coefficient decreases, or both". Props of different sizes match when run at the same Reynolds number [S20].
- Brandt and Selig measured peak efficiencies of 0.28–0.65 across 79 small props [S19].
- UIUC's smallest prop is the Crazyflie's 1.85" (static `C_T` ≈ 0.16) [S21]. **No 31–40 mm whoop prop is in any public database.**

**Consequence:** the whoop must be built from whoop measurements (BetaFPV's tables) and never from scaled 5" coefficients. Low Reynolds number is a rule about parameters, not a runtime term.

### 5.3 Battery: 1S whoop compared with 6S

**Proportional sag is similar on both (Derived):**
- 1S whoop: a 10 A punch through 25 + 10 mΩ (cell plus connector) sags about 0.35 V, roughly 9%.
- 6S: a 120 A punch through about 21 mΩ sags about 2.5 V, roughly 10%.
- **So "6S sags proportionally less" is not automatically true.** The difference comes from parameters, not cell count.

**What makes the whoop feel worse:**
- Connector and lead resistance is comparable to the cell's own resistance. BetaFPV's note on BT2.0 against PH2.0 shows this [S49].
- The LiHV window is wider: 4.35 V down to about 3.3 V is about 24%, against about 17% for LiPo from 4.2 V to 3.5 V.
- Thrust scales roughly with voltage squared, so a 20% voltage drop costs about 36% of thrust.
- Full-throttle draw (4 × 3.4 A ≈ 13.6 A on the Meteor65 Pro table) is close to the BT2.0 connector's 15 A burst rating [S49].

**Rule:** no special-case code for 1S. The same battery model, with the whoop's measured resistance, OCV curve and lead resistance, produces the whoop's fading punch.

**Battery model** (Bauersfeld & Scaramuzza [S7]):
- OCV is a cubic in energy drawn.
- `R₀` depends on power and C-rate, with a floor of 4.5 mΩ.
- One RC pair with τ = 3.3 s.
- Terminal voltage comes from solving a quadratic in power.
- Validated on 10 packs (4S–6S) to 43 mV RMSE.

Chen and Rincon-Mora's two-RC model is the general reference [S27]. **No whoop-pack fit exists.** BetaFPV's discharge curves give effective resistance (Derived) [S49]:

| Pack | Effective resistance |
|---|---|
| LAVA 300 mAh | ≈ 35–39 mΩ |
| BT2.0 300 mAh 30C | ≈ 60 mΩ |
| LAVA II 320 mAh | ≈ 29 mΩ |

## 6. Collisions with the world

- **Tunnelling risk.** At 30 m/s a 5" travels 3 cm per 1 kHz step (Derived). Thin rails, cables and gate tubes need continuous collision detection (CCD).
- **Rapier's CCD.**
  - It automatically sweeps fast dynamic bodies against fixed colliders.
  - `ccd_enabled(true)` adds sweeps against moving bodies.
  - `soft_ccd_prediction` is a cheaper predictive option [S38].
  - Rapier is engine-free, so it fits the physics crate's no-Bevy rule from #2.
- **Contact models in research sims are crude.**
  - AirSim uses a full-inertia impulse with Coulomb friction (restitution 0.55, friction 0.5) and calls parts of its contact code "a hack" [S35].
  - Flightmare only clamps to a world box [S31].
  - gym-pybullet-drones' `DYN` mode has no collision response at all [S32].
  - QuadSwarm uses hand-made heuristics: velocity decay plus noise [S57].
- **Prop strikes.** No open-source simulator models them. Velocidrone added prop damage on collision in January 2026 [S43]. Uncrashed has a propeller-damage toggle [S44].
- **Crash behaviour is a gap.** It is already listed under "Not yet specified" on the map.

## 7. How existing simulators approach it

### 7.1 Commercial FPV simulators

No vendor publishes its equations, tick rate or validation data. Everything below is what they claim on first-party pages.

| Simulator | Engine | Physics approach | Effects claimed | Quad parameters | Flight Controller | Source |
|---|---|---|---|---|---|---|
| **Liftoff** | Unity | Own flight model; the Physics 5.0 update's new Flight Controller "relies a lot less on Unity's physics system". Thrust from part data; drag from projected area sampled at 42 angles | Area drag; battery with amp draw, C-rating overheating, fast and slow sag (Apr 2024). **Prop wash is artificial**, default 42% | Part-based builder | Own Flight Controller with "A.I. PID tune"; manual tuning possible | [S42] |
| **Liftoff: Micro Drones** | Unity | Whoop-focused spin-off, full release Aug 2025 | Nothing published on ducts | Drone editor | Unverified | [S42] |
| **Velocidrone** | Unverified | "Proprietary physics and aerodynamics system"; NACA prop profiles; full drivetrain. "Very high and ultra high" physics settings, multithreaded prop and drag physics (Dec 2024) | Battery voltage and current; ESC "real ESC code for throttle ramping, braking"; motor Kv, coil resistance, torque; prop flex under load; induced and parasitic drag; prop damage (Jan 2026) | Editor with weight, drag per axis, prop size and power, **prop wash strength** (Community) | Legacy in-house, or **Betaflight 4.2 simulation** with rates, PIDs, throttle curve, TPA | [S43] |
| **Uncrashed** | Unverified (likely Unreal) | Parametric, "settings based on real values"; users can tune gravity, air friction, throttle-dependent prop efficiency | Propeller damage toggle | Prop size and pitch, Kv, cells, weight, air friction, "air grip" (Community) | Not documented | [S44] |
| **DRL Simulator** | Unity | "Advanced aerodynamic modeling"; partner claims "unsteady drag model (from Georgia Tech)", ground effect, battery dynamics, 5,000+ bench tests | Ground effect, battery sag | Thousands of real parts | Unverified | [S45] |
| **Tiny Whoop GO** | Unverified | Launched ~Jan 2021; **no physics information published** | — | — | — | [S46] |
| **TRYP FPV** | Unverified (likely Unreal 5) | "Digital twin"; wind, air resistance | Wind, drag | Frames, props, batteries | Adjustable rates | [S47] |

**Community view on feel** (Community, Oscar Liang, updated Aug 2026 [S48]):
- Velocidrone has "excellent realistic physics".
- Liftoff is realistic.
- Uncrashed's physics are "average", but it is good for freestyle.
- TRYP quads feel "slightly heavier than in real life".
- Liftoff: Micro Drones whoops carry "slightly more momentum".
- Tiny Whoop GO is "slightly easier to fly than in real life".

### 7.2 Open-source simulators

| Simulator | Motor model | Aero effects | Battery | Licence, status |
|---|---|---|---|---|
| Flightmare [S31] | First-order lag; polynomial thrust | None in the dynamics | No | MIT; last push 2024 |
| gym-pybullet-drones [S32] | None (instant `KF·rpm²`) | Ground effect, rotor drag, inter-drone downwash | No | MIT; active, now at `learnsyslab/gym-pybullet-drones` |
| RotorS [S33] | First-order, separate up and down | Rotor drag, rolling moment, wind | No | Apache-2.0; ROS1, dormant |
| PX4 + gz-sim [S34] | RotorS port | Rotor drag, rolling moment, wind | Linear drain with no sag | Apache-2.0 / BSD |
| AirSim [S35] | 5 ms low-pass; UIUC prop data | Per-face drag, wind, collisions | No | MIT; maintenance only. Project AirSim continues under IAMAI |
| Agilicious [S26] | First-order, stand-identified | Quadratic or BEM, optional neural-network residual | No | GPL-3.0 |
| RotorPy [S36] | First-order plus noise | Parasitic drag, rotor drag, induced inflow, translational lift, wind | No | MIT |
| Aerial Gym, OmniDrones [S36] | First-order | Linear or quadratic damping; simplified downwash | No | BSD-3 / MIT |

- **None of the research simulators models battery sag reducing thrust.**
- **None models ducts.**
- **No maintained, open-source, physics-first FPV simulator for pilots exists**, in Rust or otherwise. KestrelFPV (Unity, MIT) was last updated in April 2024.
- **Rust prior art is small:**
  - Peng: MIT/Apache, quadrotor dynamics, no motor model [S37].
  - rotor-rs: a RotorPy port, golden-tested against the Python original [S37].
  - Betaflight SITL exchanges sensor and motor packets over UDP, which is the route for "real Betaflight later" [S29].

## 8. Published parameter data

### 8.1 Meteor65 family (BetaFPV [S49])

| Variant | Dry mass | Wheelbase | Motor | Props | Pack | Max thrust per motor | Thrust-to-weight |
|---|---|---|---|---|---|---|---|
| Meteor65 (2022) | 22.83 g | 65 mm | 0802SE 19500KV | 31 mm 3-blade | BT2.0 300 mAh | 24.6 g @ 3.1 A, 4 V | ≈ 3.2 (Derived, 30.5 g all-up) |
| Meteor65 Pro (2022) | 23.01 g | 66 mm | 0802SE 19500KV | Gemfan 35 mm 3-blade | BT2.0 300 mAh 75C | 30.6 g @ 3.4 A, 4 V | ≈ 3.9 (Derived); BetaFPV states 3.97 |
| Meteor65 Pro O4 | 28.53 g | 66 mm | 0802SE 19500KV | 35 mm | LAVA II | — | — |
| Meteor65 Pro II (2026) | 22.60 g | 70 mm | 0802 Racing 25000KV | GF1409 35 mm | LAVA II 480 | ≈ 45 g (55 g on 40 mm props @ 46,481 RPM) | 5.17 (manufacturer) |

**Thrust against current** for 0802SE 19500KV with 35 mm props at 4 V (manufacturer) [S49]:

| Current | 0.5 A | 1.0 A | 1.5 A | 2.0 A | 2.5 A | 3.0 A | 3.4 A |
|---|---|---|---|---|---|---|---|
| Thrust | 4.1 g | 8.8 g | 14.0 g | 18.5 g | 23.0 g | 26.3 g | 30.6 g |

- **The table has no RPM column.** BetaFPV's 0802 (2026) table does have RPM, which is how `k_f` was derived above.
- **Hover check (Derived).** A 31.3 g Meteor65 Pro needs about 7.8 g per motor, roughly 25% of maximum thrust. That puts hover near half throttle, matching the #6 finding.

### 8.2 Generic 5"

| Item | Value | Source |
|---|---|---|
| Reference airframe | iFlight Nazgul Evoque F5 V2: 419 g dry, about 644 g with a 6S 1400 mAh pack, 225 mm diagonal | [S52] |
| Motor and prop | T-Motor Velox V2207 V3 1750KV with T5147 | [S50] |
| Thrust at 100% | 1,591 g, 34.6 A, 29,447 RPM at 23.5 V | [S50] |
| Thrust at 20% | 210 g | [S50] |
| Independent check | F60 ProII on MiniQuadTestBench: 1,971 g max (Community) | [S51] |
| Thrust-to-weight | ≈ 9.8 (bench supply, Derived) | — |
| Hover throttle | ≈ 18% on a full pack (Derived) | — |
| Measured inertia (NeuroBEM, 0.77 kg, 6" frame) | diag(2.5, 2.1, 4.3) × 10⁻³ kg·m² | [S6] |
| Motor time constant | 33 ms | [S6] |

### 8.3 Cetus X (reference only)

The maintainer also flies one [S49]:
- 55 g, 95 mm wheelbase;
- 1103 11000KV motors on 2S (two 1S packs in series);
- Gemfan 2020 4-blade 50 mm props;
- **no thrust table published**.

### 8.4 Sanity reference: Crazyflie 2.x

The best-characterised micro quad [S53]:
- 27 g, arm 39.7 mm;
- inertia (1.40, 1.44, 2.17) × 10⁻⁵ kg·m²;
- `k_f = 2.88 × 10⁻⁸ N/(rad/s)²`, close to the whoop's derived 2.3 × 10⁻⁸.

### 8.5 Not published anywhere we found

- Inertia for any whoop, the Cetus X or a 5" freestyle quad (only research racers).
- Torque or `k_m` data for any FPV motor.
- Motor time constants for brushless whoop motors.
- RPM for 0802SE or 0702 motors.
- Winding resistance and no-load current for whoop motors.
- Duct coefficients, and whether BetaFPV tests in-duct.
- Measured 6S internal resistance from a manufacturer.
- Thrust data for 1103 11000KV.

The map says the maintainer can't record flight logs. So whoop values in this list are estimated, checked by physics-rule Scenarios (hover throttle, flight time), and finally tuned by feel.

## 9. What defines a Quad

The parameters a Quad definition needs, with how each is normally sourced:

| Group | Parameter | Source type |
|---|---|---|
| **Frame** | Mass, centre of gravity | Manufacturer, measured |
| | Inertia (3 values, or a 3×3 matrix) | Estimate from components; identify from flight data |
| | Motor positions (arm geometry), rotor-plane height relative to the centre of gravity | Manufacturer dimensions |
| | Body drag areas (front, side, top) and drag coefficient | Estimate; fit to terminal velocity and top speed |
| | Collision shape, restitution, friction | Estimate; tune by feel |
| **Rotor and prop** | Diameter, blade count, spin direction | Manufacturer |
| | `k_f`, or a `C_T` table against RPM (and advance ratio) | Thrust stand |
| | `k_m`, or a `C_Q` table | Thrust stand (torque cell); otherwise estimate from current |
| | Rotor drag coefficient | Literature; fit to flight data |
| | Rotor inertia | Estimate from prop and bell mass; spin-down test |
| **Motor** | Kv, winding resistance R, no-load current I₀, pole count | Manufacturer (5"); whoop gives Kv only |
| | Spin-up and spin-down time constants | Step-response test; MiniQuadTestBench (5"); estimate (whoop) |
| **ESC** | Braking mode, idle drive, slew limit | Firmware defaults |
| | Thrust-curve parameter (Betaflight's ArduPilot-style formula) | Betaflight settings |
| **Battery** | Cell count, chemistry (full and empty voltage), capacity, mass | Manufacturer |
| | OCV curve, internal resistance R₀, one RC pair | Discharge curves; pulse tests |
| | Lead and connector resistance | Measured or estimated |
| **Duct** (whoop only) | Present, ram-drag factor, centre-of-pressure offset | Momentum-theory estimate; tune by feel |
| **Feel parameters** | Prop-wash intensity, noise bandwidth, ground-effect body term `K_b` | Literature anchors; tune by feel |

**Every value should carry its source and a confidence level:** measured, manufacturer, derived or estimate. Many whoop values are estimates and will be revisited.

### 9.1 Draft values

**Meteor65 Pro.** The variant still needs confirming.

| Parameter | Value | Confidence |
|---|---|---|
| All-up mass | 31.3 g (23.0 g dry + 8.3 g LAVA 300) | Manufacturer |
| Motor axis distance | 33 mm from centre (66 mm diagonal) | Manufacturer |
| Max thrust per motor | 30.6 g at 4 V | Manufacturer |
| `k_f` | ≈ 2.0–2.3 × 10⁻⁸ N/(rad/s)² | Derived |
| `k_m / k_f` | ≈ 0.005 m | Estimate |
| Inertia | ≈ (0.7, 0.9, 1.4) × 10⁻⁵ kg·m² | Estimate |
| Motor time constant | 20–50 ms | Estimate |
| Pack effective resistance | ≈ 35–39 mΩ (LAVA 300) | Derived |
| Duct ram drag | ≈ 1.2 s⁻¹ mass-normalised | Derived/Estimate |

**Generic 5".**

| Parameter | Value | Confidence |
|---|---|---|
| All-up mass | 620–650 g | Manufacturer |
| Arm length | 112.5 mm | Manufacturer |
| Max thrust per motor | 1.59 kg | Manufacturer |
| `k_f` | ≈ 1.5–1.65 × 10⁻⁶ N/(rad/s)² | Derived |
| `k_m / k_f` | ≈ 0.01 m | Estimate |
| Inertia | ≈ (1.4, 1.5, 2.5) × 10⁻³ kg·m², checked against NeuroBEM | Estimate |
| Motor time constant | 33–39 ms | Measured on similar quads |
| Pack plus leads resistance | ≈ 30 mΩ | Estimate |
| Rotor drag | 0.24–0.54 s⁻¹ | Measured on a similar quad |

## Surprises that affect other tickets

- **[#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10), [#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16): the Meteor 65 isn't what the ticket assumed.**
  - It uses 0802SE 19500KV motors, not 0702.
  - There are four variants: original (31 mm props), Pro (35 mm), Pro O4, and the 2026 Pro II (70 mm frame, 25000KV, 5.17:1 thrust-to-weight).
  - Their thrust-to-weight differs by 60%. **Confirm which one the maintainer flies** before freezing the whoop Quad.
- **[#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5), [#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10): prop wash feel needs an imperfect Flight Controller.**
  - Liftoff's "perfect" auto-tuned Flight Controller removed prop wash, so they faked it back in.
  - For OpenDrone's prop wash to emerge physically, our Flight Controller must keep the real delays: gyro filtering, D-term filtering, motor lag. The physics must also supply gyro noise and vibration.
  - An idealised Flight Controller will make the sim feel wrong even when the aerodynamics are right.
- **[#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5), [#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12): the physics is cheap enough to run at Betaflight's loop rate.**
  - Betaflight's PID loop runs at 4–8 kHz, and its filters assume 3.2–8 kHz gyro data.
  - The flight model costs about a microsecond per step, so sub-stepping the physics (and our Flight Controller) at 4–8 kHz inside each 1 kHz Bevy tick is affordable.
  - This keeps a door open for running real Betaflight in software later. Make the physics and Flight Controller rates parameters, not constants.
- **[#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13): no "prop wash strength" slider.**
  - Velocidrone and Liftoff both expose one.
  - Under our "Assists never change physics" rule, prop wash intensity is a Quad parameter. A pilot-facing slider would be an arcade knob.
- **[#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16): every Quad value needs a source and confidence field.** About half the whoop parameters are estimates (inertia, motor lag, `k_m`, duct terms). The Pack format should record where each number came from, so later measurements can replace estimates traceably.
- **[#10](https://github.com/BartoszSolkaBD/OpenDrone/issues/10): don't special-case 1S against 6S.** The whoop's fading punch falls out of the same battery model with its measured resistance, connector resistance and LiHV voltage window. Proportional sag on 6S is similar, not smaller.
- **[#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12): collisions dominate cost, and Rapier fits the engine-free crate.**
  - Its CCD sweeps fast bodies against static geometry automatically.
  - Ground and ceiling effect need ray casts, so the flight model needs a world-query interface even if contact solving lives elsewhere.
- **[#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7), [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11): use `libm` for `exp`, `sin` and `cos`.** The motor-lag and attitude updates both use them, so they must follow the `libm` rule from the determinism research.

## Open questions

- Which Meteor 65 variant does the maintainer fly? This decides props (31 or 35 mm), motor Kv and thrust-to-weight.
- Does BetaFPV measure its thrust tables with the duct fitted? It decides whether duct hover gain is already "in" the table.
- No whoop motor time constant, inertia or duct coefficient exists publicly. The plan is to estimate, check with hover and flight-time Scenarios, then tune by feel. A single donated Meteor 65 Blackbox log with RPM telemetry would let us identify them properly (the "donate a log" idea from #6).
- Whether the closed-form prop wash model feels right can only be judged by flying it. It is the strongest candidate for a prototype ticket.

## Sources

Code paths were read from the default branch on 2026-10-03.

**Rigid body, integration, determinism**
- [S1] Lee, Leok, McClamroch, "Geometric tracking control of a quadrotor UAV on SE(3)", arXiv:1003.2005 (eqs. 2–5). https://arxiv.org/abs/1003.2005
- [S2] Tal & Karaman, "Accurate tracking of aggressive quadrotor trajectories using incremental nonlinear dynamic inversion", arXiv:1809.04048 (eqs. 4–5). https://arxiv.org/abs/1809.04048
- [S38] Rapier docs: determinism and rigid bodies (CCD). https://rapier.rs/docs/user_guides/rust/determinism, https://rapier.rs/docs/user_guides/rust/rigid_bodies
- [S39] Rust `f32` docs (non-deterministic precision of `sin`, `cos`, `exp`). https://doc.rust-lang.org/std/primitive.f32.html
- [S40] Glenn Fiedler, "Integration Basics". https://gafferongames.com/post/integration_basics/
- [S41] Bullet Physics: `src/LinearMath/btTransformUtil.h` (`integrateTransform`), `src/BulletDynamics/Dynamics/btRigidBody.cpp`. https://github.com/bulletphysics/bullet3

**Aerodynamics**
- [S3] Mahony, Kumar, Corke, "Multirotor Aerial Vehicles", IEEE RAM 19(3), 2012 (paywalled; cited via [S5], [S8]). https://doi.org/10.1109/MRA.2012.2206474
- [S4] Hoffmann, Huang, Waslander, Tomlin, "Quadrotor helicopter flight dynamics and control: theory and experiment", AIAA GNC 2007. https://ai.stanford.edu/~gabeh/papers/Quadrotor_Dynamics_GNC07.pdf
- [S5] Faessler, Franchi, Scaramuzza, "Differential flatness of quadrotor dynamics subject to rotor drag", RA-L 2018, arXiv:1712.02402. https://arxiv.org/abs/1712.02402
- [S6] Bauersfeld, Kaufmann, Loianno, Scaramuzza, "NeuroBEM: Hybrid Aerodynamic Quadrotor Model", RSS 2021, arXiv:2106.08015; dataset readme. https://arxiv.org/abs/2106.08015, https://rpg.ifi.uzh.ch/neuro_bem/Readme.html
- [S7] Bauersfeld & Scaramuzza, "Range, endurance, and optimal speed estimates for multicopters", RA-L 2022, arXiv:2109.04741. https://arxiv.org/abs/2109.04741
- [S8] Abeywardena et al., arXiv:1509.03388. https://arxiv.org/abs/1509.03388
- [S9] Bangura, Melega, Naldi, Mahony, arXiv:1601.00733. https://arxiv.org/abs/1601.00733
- [S10] Johnson, "Model for Vortex Ring State Influence on Rotorcraft Flight Dynamics", NASA TP-2005-213477. https://ntrs.nasa.gov/citations/20060024029
- [S11] Talaeizadeh et al., arXiv:1909.09069. https://arxiv.org/abs/1909.09069
- [S12] Sanchez-Cuevas, Heredia, Ollero, "Characterization of the aerodynamic ground effect and its influence in multirotor control", Int. J. Aerospace Eng. 2017. https://doi.org/10.1155/2017/1823056
- [S13] Hsiao & Chirarattananon, "Ceiling effects for hybrid aerial-surface locomotion of small rotorcraft", arXiv:1905.04632. https://arxiv.org/abs/1905.04632
- [S14] Elliott-Roe et al., IMAV 2024. https://www.imavs.org/papers/2024/37.pdf
- [S15] Conyers, PhD dissertation, University of Denver, 2019. https://digitalcommons.du.edu/etd/1570
- [S54] Dryden turbulence model (MIL-F-8785C), MathWorks summary (secondary). https://www.mathworks.com/help/aeroblks/drydenwindturbulencemodelcontinuous.html
- [S55] Svacha, Mohta, Kumar, ICUAS 2017 (abstract only). https://www.semanticscholar.org/paper/aa234ceaff9d3a3640d7c2dc0dc41da730a6e6e4
- [S56] Sun et al., T-RO 2022, arXiv:2109.01365. https://arxiv.org/abs/2109.01365

**Ducts and low Reynolds number**
- [S16] Pereira, "Hover and wind-tunnel testing of shrouded rotors for improved micro air vehicle design", PhD thesis, University of Maryland, 2008. https://drum.lib.umd.edu/handle/1903/8752
- [S17] Martin & Tung, NASA ducted-fan hover tests (scanned; numbers Unverified). https://ntrs.nasa.gov/citations/20050009943
- [S18] Modak & Drew, arXiv:2608.06707 (2026). https://arxiv.org/abs/2608.06707
- [S19] Brandt & Selig, "Propeller performance data at low Reynolds numbers", AIAA 2011-1255. https://m-selig.web.engr.illinois.edu/pubs/BrandtSelig-2011-AIAA-2011-1255-LRN-Propellers.pdf
- [S20] Deters, Ananda, Selig, "Reynolds number effects on the performance of small-scale propellers", AIAA 2014-2151. https://m-selig.ae.illinois.edu/pubs/DetersAnandaSelig-2014-AIAA-2014-2151.pdf
- [S21] UIUC Propeller Database, volumes 1–2. https://m-selig.ae.illinois.edu/props/propDB.html

**Motors, ESC, battery**
- [S22] Drela, "First-Order DC Electric Motor Model", MIT, 2007. https://web.mit.edu/drela/Public/web/qprop/motor1_theory.pdf
- [S23] Gauthier et al., arXiv:2312.09981. https://arxiv.org/abs/2312.09981
- [S24] Eschmann et al., arXiv:2404.07837 (2024). https://arxiv.org/abs/2404.07837
- [S25] Gräfe et al., arXiv:2603.05944 (2026). https://arxiv.org/abs/2603.05944
- [S27] Chen & Rincon-Mora, "Accurate electrical battery model capable of predicting runtime and I–V performance", IEEE TEC 21(2), 2006. https://rincon-mora.gatech.edu/publicat/jrnls/tec05_batt_mdl.pdf
- [S28] Betaflight docs and source:
  - configuration tab (loop rates): https://betaflight.com/docs/wiki/app/configuration-tab
  - Dynamic Idle: https://betaflight.com/docs/wiki/guides/current/Dynamic-Idle
  - RPM filtering: https://betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering
  - 4.2 tuning notes: https://betaflight.com/docs/wiki/tuning/4-2-Tuning-Notes
  - source files: `src/main/flight/mixer.c`, `pid.c`, `drivers/dshot.c`; https://github.com/betaflight/betaflight
- [S29] Betaflight SITL. https://betaflight.com/docs/development/SITL
- [S30] ESC firmware:
  - Bluejay: https://github.com/mathiasvr/bluejay
  - AM32 `Src/main.c`: https://github.com/am32-firmware/AM32
  - BLHeli: https://github.com/bitdump/BLHeli

**Simulators**
- [S26] Foehn et al., "Agilicious", arXiv:2307.06100. https://arxiv.org/abs/2307.06100
- [S31] Flightmare: `flightlib/src/dynamics/quadrotor_dynamics.cpp`, `flightlib/src/objects/quadrotor.cpp`, `flightlib/src/common/integrator_rk4.cpp`; paper arXiv:2009.00563. https://github.com/uzh-rpg/flightmare
- [S32] gym-pybullet-drones: `gym_pybullet_drones/envs/BaseAviary.py`, `assets/cf2x.urdf`; paper arXiv:2103.02142. https://github.com/learnsyslab/gym-pybullet-drones
- [S33] RotorS: `rotors_gazebo/worlds/basic.world`, `rotors_gazebo_plugins/src/gazebo_motor_model.cpp`. https://github.com/ethz-asl/rotors_simulator
- [S34] PX4 SITL and gz-sim:
  - motor model `src/gazebo_motor_model.cpp`: https://github.com/PX4/PX4-SITL_gazebo-classic
  - gz port `src/systems/multicopter_motor_model/MulticopterMotorModel.cc`: https://github.com/gazebosim/gz-sim
  - `BatterySimulator.cpp`: https://github.com/PX4/PX4-Autopilot
- [S35] AirSim:
  - `AirLib/include/physics/FastPhysicsEngine.hpp`
  - `AirLib/include/vehicles/multirotor/RotorParams.hpp`
  - `Unreal/Plugins/AirSim/Source/SimMode/SimModeWorldBase.h`
  - paper arXiv:1705.05065
  - repos: https://github.com/microsoft/AirSim, https://github.com/iamaisim/ProjectAirSim
- [S36] RotorPy: https://github.com/spencerfolk/rotorpy. Aerial Gym: https://github.com/ntnu-arl/aerial_gym_simulator. OmniDrones: https://github.com/btx0424/OmniDrones
- [S37] Rust:
  - Peng: https://github.com/makeecat/Peng
  - rotor-rs: https://github.com/Papayasalade/rotor-rs
- [S57] QuadSwarm, arXiv:2306.09537. https://arxiv.org/abs/2306.09537
- [S42] Liftoff:
  - Physics 5.0: https://www.liftoff-game.com/news/test-our-biggest-update-yet
  - drag: https://www.liftoff-game.com/node/80
  - battery (milestone 1.6.0): https://www.liftoff-game.com/news/milestone-160-released
  - developer post on prop wash, 27 Apr 2024: https://steamcommunity.com/app/410340/discussions/0/4361249282374071189
  - Micro Drones: https://store.steampowered.com/app/1432320/
- [S43] Velocidrone:
  - features: https://velocidrone.com/features
  - news: https://www.velocidrone.com/news
  - manual: https://velocidrone.com/downloads/VelociDroneManual.pdf
- [S44] Uncrashed: https://store.steampowered.com/app/1682970/; developer post on prop damage: https://steamcommunity.com/app/1682970/discussions/0/599650671167331642
- [S45] DRL Simulator: https://store.steampowered.com/app/641780; MultiGP announcement: https://dev.multigp.com/the-drl-sim-is-now-the-official-fpv-sim-of-multigp/
- [S46] Tiny Whoop GO: https://www.tinywhoopgo.com/
- [S47] TRYP FPV: https://store.steampowered.com/app/1881200/
- [S48] Oscar Liang, FPV simulator review (Community). https://oscarliang.com/fpv-simulator/

**Hardware data**
- [S49] BetaFPV product pages:
  - Meteor65: https://betafpv.com/products/meteor65-brushless-whoop-quadcopter-1s
  - Meteor65 Pro: https://betafpv.com/products/meteor65-pro-brushless-whoop-quadcopter-1s
  - Meteor65 Pro O4: https://betafpv.com/products/meteor65-pro-o4-brushless-whoop-quadcopter
  - Meteor65 Pro II: https://betafpv.com/products/meteor65-pro-ii-brushless-whoop-quadcopter
  - 0802SE motors: https://betafpv.com/products/0802se-22000kv-brushless-motors
  - 0802 (2026) motors: https://betafpv.com/products/0802-brushless-motors-2026
  - LAVA 300: https://betafpv.com/products/lava-1s-300mah-75c-battery-5pcs
  - LAVA II: https://betafpv.com/products/lava-ii-1s-battery
  - Cetus X: https://betafpv.com/products/cetus-x-brushless-quadcopter
- [S50] T-Motor Velox V2207 V3 1750KV (T-Motor official store). https://www.ligpower.com/product/v2207-v3-kv1750-fpv-motor.html
- [S51] MiniQuadTestBench, T-Motor F60 ProII (Community; all rights reserved). https://www.miniquadtestbench.com/assets/components/motordata/motorinfo.php?uid=242
- [S52] iFlight Nazgul Evoque F5 V2. https://shop.iflight.com/Nazgul-Evoque-F5-V2-6S-Pro1954
- [S53] Luis & Le Ny, Crazyflie 2.0 parameters (citing Förster 2015), arXiv:1608.05786. https://arxiv.org/abs/1608.05786
