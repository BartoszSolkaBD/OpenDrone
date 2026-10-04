# Bevy as the app shell: high-rate Quad physics, separate from rendering

Research for [#2](https://github.com/BartoszSolkaBD/OpenDrone/issues/2), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1).
Checked on 2026-10-03 against Bevy **0.19.1** (latest stable, 2026-08-13), with Bevy **0.20** in release candidates (rc.2, 2026-09-28). Bevy 0.19 uses wgpu 29.0.3 and glam 0.32.

## The answer in plain language

**Confirm Bevy.** It can run Quad physics at a fixed high rate that never depends on the frame rate, on macOS, Windows and Linux, and it keeps iOS, Android, multiplayer and scripting open.

**The integration pattern: a fixed timestep with catch-up stepping, plus render interpolation.**

1. Before each frame is drawn, Bevy works out how much real time has passed and runs the physics step as many times as needed to catch up, each step with exactly the same small time slice (for example 1 ms). At 1 kHz physics and 60 fps, that is about 16 or 17 physics steps per frame. Bevy has this built in (`FixedUpdate` driven by `Time<Fixed>`).
2. The physics itself lives in our own engine-free crate. Bevy only calls it. The crate never knows Bevy exists. It can also split one Bevy step into several smaller physics sub-steps, so the physics rate can go higher without touching Bevy.
3. The picture is drawn slightly "between" the last two physics states, so motion looks smooth even though physics and frames don't line up.
4. Stick movements from the Input Device are recorded with timestamps, so each physics step uses the stick position from its own moment, not one value per frame.

Why this and not a separate physics thread: it gives the same physics results, is deterministic (same inputs give the same flight), is what Bevy's multiplayer libraries expect, and avoids fighting each operating system's timer precision. A dedicated thread stays possible later, because the physics crate doesn't care who calls it.

**Physics is never cut to protect frame rate.** Every physics step that is due always runs, with the same time slice. If the machine falls badly behind (a frame taking longer than a quarter of a second), Bevy slows the whole simulation down rather than skipping or stretching steps. Frame rate drops first; physics accuracy doesn't.

**Key facts**

- Bevy ships a breaking release every 3.5 to 5.5 months in practice (0.16 in April 2025, 0.17 in September 2025, 0.18 in January 2026, 0.19 in June 2026, 0.20 due about now). Each comes with a migration guide of 60 to 120 entries. Keeping physics in a crate with **no Bevy dependency** shields it completely.
- No one has published Bevy frame rates for a base M4 at 1440p. A modest Skate Park or Bando should reach 45 fps if we budget the costly effects (shadows, screen-space effects, volumetrics). We must measure early with a prototype scene.
- Bevy's ray-traced lighting (Solari) does not run on Macs today, because wgpu doesn't expose ray tracing on Metal. We should not plan around it.
- iOS and Android builds are supported, with an official example. Gamepads on phones are not covered by Bevy's default gamepad library, so mobile Input Devices will need extra work later.
- Multiplayer libraries for Bevy (lightyear, bevy_ggrs) run game logic in Bevy's fixed timestep, so the chosen pattern lines up with them.
- Bevy's built-in gamepad handling reads sticks once per frame and smooths away small movements (a 5% centre deadzone, 1% change threshold). For faithful flying we should read Input Devices through our own input layer.
- Bevy's defaults add a few frames of delay between stick and screen (vsync queue, rendering one frame behind). These are settings, not blockers, but they matter for feel and should be chosen on purpose.

## 1. Running Quad physics at a fixed high rate

### 1.1 What Bevy gives us

Bevy's fixed timestep is the textbook "Fix Your Timestep" accumulator: "the renderer produces time and the simulation consumes it in discrete dt sized steps" ([Gaffer On Games](https://gafferongames.com/post/fix_your_timestep/)). In Bevy 0.19.1:

- **Rate.** `Time<Fixed>` defaults to 64 Hz ("The default `timestep()` is 64 hertz, or 15625 microseconds"). We set our own with `Time::<Fixed>::from_hz(1000.0)`. The step is stored as a `Duration` with nanosecond resolution, so 1 kHz is exactly 1 ms ([fixed.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/fixed.rs)).
- **Catch-up loop.** Once per frame Bevy adds the elapsed time to an accumulator, then runs the fixed schedules once for every whole step that fits: `while world.resource_mut::<Time<Fixed>>().expend() { ... schedule.run(world); }` ([fixed.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/fixed.rs)). At 1 kHz and 60 fps, that is about 16 or 17 steps back to back each frame. The steps run in a burst, not spaced 1 ms apart on the wall clock. For physics that doesn't matter, because each step only sees simulated time.
- **Where it sits in the frame.** The main schedule order is `First, PreUpdate, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last`. One fixed tick runs five schedules: `FixedFirst, FixedPreUpdate, FixedUpdate, FixedPostUpdate, FixedLast` ([main_schedule.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_app/src/main_schedule.rs)). Input is read in `PreUpdate`, so it is always before the physics burst.
- **What's left over.** After the burst, `overstep_fraction()` says how far we are into the next, not yet simulated step (0 to 1). Rendering uses it to blend between the previous and the current physics state.
- **Overload behaviour.** `Time<Virtual>` caps each frame's time at `max_delta`, 250 ms by default. Its docs say it is "better to lose the extra time and pretend a shorter duration of time passed", and "the game will run slow, and it will run slower than real time, but it will not freeze and it will recover as soon as computation becomes fast again" ([virt.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/virt.rs)). Steps are never skipped or stretched. This matches the Notes' rule that physics is never cut to protect frame rate. Pause and time scaling (`set_relative_speed`) change how many steps run, never the step size.
- **Interpolation.** Bevy's official example [`physics_in_fixed_timestep`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/movement/physics_in_fixed_timestep.rs) shows the pattern: physics in `FixedUpdate` keeps a previous and a current position; a system in `RunFixedMainLoopSystems::AfterFixedMainLoop` blends them with `overstep_fraction()`. It admits the cost: "every visual frame is now slightly lagging behind the actual physical frame". Bevy has no built-in interpolation ([bevy#1259](https://github.com/bevyengine/bevy/issues/1259) is open); the third-party [`bevy_transform_interpolation`](https://github.com/Jondolf/bevy_transform_interpolation) does it for `Transform`s and is what Avian uses.
- **Testing.** Since 0.18, `TimeUpdateStrategy::FixedTimesteps(n)` makes one app update run exactly `n` fixed steps ([bevy_time lib.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/lib.rs)), handy for integration tests of the adapter.
- **Precedent.** Avian, the main Bevy physics engine, runs "at a fixed timestep in `FixedPostUpdate`" and splits each tick into `SubstepCount` sub-steps, 6 by default ([avian3d docs](https://docs.rs/avian3d/0.7.0/avian3d/)). We won't use Avian for the Quad, but it shows the shape: Bevy's tick outside, finer sub-steps inside the physics code.

### 1.2 Recommended pattern

Name: **fixed timestep with catch-up stepping, in-crate sub-steps, render interpolation, and timestamped input.**

1. **The physics crate** has no Bevy dependency. Its core call advances the Quad state by a number of fixed sub-steps, each with its own Input Device sample. The physics rate (1 kHz or higher) is a physics-crate setting, not a Bevy setting.
2. **The adapter** (a thin Bevy plugin) calls it from one system in `FixedUpdate`. Start with Bevy's tick equal to the physics rate (`Time::<Fixed>::from_hz(1000.0)`, one sub-step per tick). This gives the least display lag: the picture trails the physics by one step, 1 ms.
3. **Escape hatch.** Each tick costs five Bevy schedule runs. No one has published numbers for that overhead at 1 kHz, so the tracked physics benchmark should measure it. Two remedies, both cheap: switch the fixed schedules to Bevy's single-threaded executor (Avian does this for its own schedule), or lower Bevy's tick (for example 250 Hz) and run 4 physics sub-steps per tick. The second gives identical physics, as long as each sub-step gets its own input sample, but the picture then trails by up to one tick (4 ms). It is also what multiplayer will want later (section 5).
4. **Rendering** blends the last two physics states with `overstep_fraction()` in `RunFixedMainLoopSystems::AfterFixedMainLoop`, for the Quad's body and the FPV camera alike.
5. **Input.** Bevy reads gamepads once per frame, in `PreUpdate`, and keeps only the latest value. Without extra work, all 16 or so physics steps in a frame see the same stick position. A Bevy pull request says it plainly: "each `FixedUpdate` should only see the events that occurred in its covered timespan, but what happens right now is the first step in the frame reads all pending events. Fixing that will require timestamped events." ([bevy#10077](https://github.com/bevyengine/bevy/pull/10077); see also open [bevy#6183](https://github.com/bevyengine/bevy/issues/6183)). Our input layer should record timestamped stick samples, for example on its own polling thread, or from `gilrs` events, which carry a `time` field that Bevy throws away. Catch-up stepping makes this easy: each burst simulates time that has already passed, so all the samples for that window already exist when the steps run. Scenarios feed the same per-step inputs directly.
6. **Determinism.** The number of steps per frame varies with the frame rate, but the sequence of steps and their inputs doesn't. Bevy runs systems in parallel and only guarantees the order you declare, so the whole physics step stays inside one system.

### 1.3 The alternative: a dedicated physics thread

Bevy can live next to a thread it doesn't manage. Its official example [`external_source_external_thread`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/async_tasks/external_source_external_thread.rs) runs a plain `std::thread` and passes data to Bevy over a channel. A physics thread ticking against the wall clock at 1 kHz would sample input at exactly the physics rate.

We don't recommend it for the alpha:

- **Precise 1 ms pacing is fiddly on every OS.** Rust's `thread::sleep` on Windows now uses a high-resolution waitable timer ("Attempt to use high-precision sleep (Windows 10, version 1803+)", [Rust std source](https://github.com/rust-lang/rust/blob/master/library/std/src/sys/thread/windows.rs)), but even so, sub-millisecond accuracy usually needs a sleep-then-spin hybrid such as [`spin_sleep`](https://docs.rs/spin_sleep/latest/spin_sleep/) ("Only use native sleep as far as it can be trusted, then spin"), which costs CPU. On Apple silicon the thread needs a high quality-of-service class, or the system may move it to an efficiency core ("the system is more likely to run background tasks on lower performance cores", [Apple](https://developer.apple.com/documentation/apple-silicon/tuning-your-code-s-performance-for-apple-silicon)). Phones add thermal throttling.
- **It breaks determinism unless built carefully.** Wall-clock pacing means inputs land on whichever step happens to be running; to replay a flight we would still need to log tick-indexed inputs.
- **It sits outside Bevy's multiplayer libraries**, which expect logic in Bevy's fixed schedule (section 5).
- **It buys little.** With timestamped input, catch-up stepping already gives each step its own stick sample.

Keep it as a later option, for example if real Betaflight in software (SITL, [#5](https://github.com/BartoszSolkaBD/OpenDrone/issues/5)) must run against a real-time clock. Because the physics crate has no idea who calls it, switching costs only the adapter.

### 1.4 Latency: the bigger risk for feel

For an FPV pilot, delay from stick to screen matters as much as physics accuracy. Bevy's defaults favour smoothness over latency:

| Source of delay | Bevy default | Lever |
|---|---|---|
| Vsync queue | `PresentMode::Fifo` ("Vsync On"), a queue "approximately 3 frames long"; `desired_maximum_frame_latency` "will default to 2" ([window.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_window/src/window.rs)) | `Mailbox` or `AutoNoVsync` ("low-latency and ... not capped by the refresh rate"), lower frame latency |
| Pipelined rendering | On for native builds: "the Nth frame's rendering can be run at the same time as the N + 1 frame's simulation" ([pipelined_rendering.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/pipelined_rendering.rs)), so about one extra frame (our inference) | Can be disabled; costs throughput |
| Input read once per frame | `PreUpdate` | Timestamped input (above) |
| Interpolation | One physics step (1 ms at 1 kHz) | Negligible at 1 kHz |
| Frame pacing | Main loop paced by vsync ([winit_config.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_winit/src/winit_config.rs)) | [`bevy_framepace`](https://github.com/aevyrie/bevy_framepace) "to reduce motion-to-photon latency" |

None of this blocks Bevy, but the settings catalogue and FPV camera tickets should pick these deliberately, and a feel test should compare them.

## 2. Release cadence, and keeping the physics crate insulated

**Cadence.** Bevy's README states: "A new version of Bevy containing breaking changes to the API is released approximately once every 3 months" ([README](https://github.com/bevyengine/bevy/blob/v0.19.1/README.md)). The release process page says Bevy "uses three-months-long development cycles, delimited by a weeks-long rolling release process", with weekly release candidates that give "ecosystem crates time to update" ([release process](https://bevy.org/learn/contribute/project-information/release-process/)). In practice the gap has been 3.5 to 5.5 months:

| Release | Date | Migration guide entries |
|---|---|---|
| 0.16.0 | 2025-04-24 | (older format) |
| 0.17.0 | 2025-09-30 | 117 |
| 0.18.0 | 2026-01-13 | 64 |
| 0.19.0 | 2026-06-18 | 103 |
| 0.20.0 | rc.2 on 2026-09-28 | 74 so far (draft) |

Dates from [GitHub releases](https://github.com/bevyengine/bevy/releases); entry counts are `###` headings in the [migration guides](https://github.com/bevyengine/bevy-website/tree/main/content/learn/migration-guides). Patch releases (0.19.1 and so on) land in between. Bevy has no 1.0 and makes no stability promise.

**The changes are broad.** The 0.19 guide's headline items were "resources are now stored as components", "rendering now uses systems too!", a new text engine, and renamed scene crates ([0.18 to 0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/)). The 0.20 draft relaxes the ordering between several built-in schedule sets, and warns that code relying on implicit ordering "through interior mutability on a `Res<T>`, a channel, an atomic" is no longer guaranteed to run in order ([0.19 to 0.20 draft](https://github.com/bevyengine/bevy-website/blob/main/content/learn/migration-guides/0.19-to-0.20.md)).

**The ecosystem keeps up quickly.** After Bevy 0.19.0 (2026-06-18): avian3d 0.7.0 and bevy_transform_interpolation 0.5.0 (2026-06-20), lightyear 0.27.0 (2026-06-22), bevy_mod_scripting 0.20.0 (2026-06-24), bevy_ggrs 0.22.0 (2026-06-26) ([crates.io](https://crates.io)). Lag is days to weeks.

**How to insulate the physics crate.**

- The physics crate depends on **no Bevy crate at all**. Its interface is plain Rust data: a state, a set of inputs per step, a fixed time slice. A thin adapter crate is the only code that touches both.
- **Watch the shared maths library.** Bevy re-exports `glam` (0.32 in Bevy 0.19), and bumps it in releases: the 0.17 guide says "We've upgraded `glam` and the other math crates", and 0.19 has "rand, glam & uuid updated to latest versions" ([0.16 to 0.17](https://bevy.org/learn/migration-guides/0-16-to-0-17/), [0.18 to 0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/)). If the physics crate also uses `glam` and shares the exact version, every Bevy upgrade drags the physics crate along. Either let the physics crate own its maths types (possibly a different `glam` version or 64-bit floats) and convert at the adapter, or accept the lockstep. This belongs to the crate-split ticket ([#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12)).
- Scenarios run the physics crate directly, headless, without Bevy, so Bevy upgrades can't move Scenario results. Only the adapter and the rendering side need migrating.

## 3. Rendering on the Mac mini M4 at 1440p

**The hardware.** The base M4 has "up to four performance cores and ... six efficiency cores" and a 10-core GPU with "Dynamic Caching, hardware-accelerated ray tracing, and hardware-accelerated mesh shading" ([Apple newsroom, M4](https://www.apple.com/newsroom/2024/05/apple-introduces-m4-chip/)).

**No direct benchmark exists.** No Bevy release note, issue or benchmark publishes frame rates for a base M4 at 1440p. The only Apple figure in the 0.19 notes is a relative one: reduced bind-group overhead gave "+15%" on an "Apple M2 Max (Metal)" ([Bevy 0.19](https://bevy.org/news/bevy-0-19/)). The big 0.19 speed-ups were measured on "a laptop with a mobile Nvidia RTX 4090" (`many_cubes` from 21 to 53 fps with 1.6 million cubes), and the `bevy_city` example went "from 19.3ms to 11.8ms in static scenes" ([Bevy 0.19](https://bevy.org/news/bevy-0-19/)). These are stress tests far heavier than a Skate Park or a Bando, but they were not on Apple hardware.

**What works on Metal, what doesn't.** From wgpu 29's feature list ([wgpu-types `Features`](https://docs.rs/wgpu-types/29.0.3/wgpu_types/struct.Features.html)):

| wgpu feature | Metal? | What it means for us |
|---|---|---|
| `INDIRECT_FIRST_INSTANCE`, `IMMEDIATES` | Yes | Bevy's GPU-driven rendering and GPU culling run on the M4. Bevy enables GPU culling when these are present ([source](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/batching/gpu_preprocessing.rs)). |
| `MULTI_DRAW_INDIRECT_COUNT` | No ("DX12, Vulkan 1.2+") | Slightly less efficient batched draws on Mac than on Windows/Linux. |
| `TEXTURE_BINDING_ARRAY` | Yes | Bevy 0.19's "partial bindless": "WGPU's backend for Metal ... currently only permits texture binding arrays but not buffer binding arrays", and standard PBR materials now render efficiently on Mac ([Bevy 0.19](https://bevy.org/news/bevy-0-19/)). |
| `BUFFER_BINDING_ARRAY`, `EXPERIMENTAL_RAY_QUERY` | No ("Vulkan" only) | **Solari, Bevy's ray-traced lighting, can't run on Mac.** It requires both ([bevy_solari source](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_solari/src/lib.rs)), even though the M4 has ray-tracing hardware. |
| `PIPELINE_CACHE` | No ("Unimplemented Platforms: DX12, Metal") | No wgpu-level shader pipeline cache on Mac; first-use shader compiles can hitch. Pre-warm scenes behind a loading screen. |

**Features to budget.** Bevy's own notes give cost models for some: contact shadows cost "scales with the number of pixels on screen lit by lights with contact shadows enabled, multiplied by the number of such lights" ([Bevy 0.19](https://bevy.org/news/bevy-0-19/)). On shadows, the `DirectionalLight` docs say "shadows are rather expensive and become more so with every light that casts them. In general, it's best to aggressively limit the number of lights with shadows enabled to one or two at most", and "soft shadows are significantly more expensive to render than hard shadows" ([directional_light.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_light/src/directional_light.rs)). Point lights cast shadows into a cube map, six views per light ([point_light.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_light/src/point_light.rs)), so a Bando lit by many shadowed lamps is the main risk. SSAO has quality levels, where "Higher slice count means less noise, but worse performance" ([ssao](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/ssao/mod.rs)). Bevy's motion blur is described as "relatively inexpensive" ([motion_blur](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_post_process/src/motion_blur/mod.rs)). Every screen-space effect (SSAO, screen-space reflections, contact shadows, depth of field) scales with pixel count, and 1440p is about 1.8 times the pixels of 1080p. A wide FPV field of view also puts more of the Map on screen at once.

**Known Mac issue right now.** An open report says meshes flicker and materials swap on Apple M3 Pro with 0.19.x "only ... with GPU Preprocessing", not on Windows and not on 0.18.1 ([bevy#25595](https://github.com/bevyengine/bevy/issues/25595), opened 2026-08-28, no response yet). It needs a complex scene to trigger. Watch it; the fallback is turning GPU preprocessing off for the camera.

**Verdict on 45 fps.** Plausible for modest PBR scenes with one shadowed sun light, few shadowed point lights, and post-processing chosen deliberately, but unproven. The prototype ticket for Skate Park and Bando ([#17](https://github.com/BartoszSolkaBD/OpenDrone/issues/17)) should measure on the dev machine before any visual feature is locked.

## 4. iOS and Android: the door stays open

- Bevy ships an official mobile example with an Xcode project for iOS and Gradle projects for Android ([examples/mobile](https://github.com/bevyengine/bevy/tree/v0.19.1/examples/mobile)). Android builds use `cargo-ndk` then `gradlew`; iOS builds via `make run` or Xcode ([examples README](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/README.md#platform-specific-examples)).
- Android: `GameActivity` "only works for Android API level 31 and higher"; `NativeActivity` covers older phones. Since 0.19 neither is a default feature, so the app turns one on explicitly ([examples README](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/README.md#old-phones), [0.18 to 0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/)).
- Rendering uses Metal on iOS and Vulkan (or GLES) on Android through wgpu. Bevy turns off GPU preprocessing on some older Android GPUs (Adreno 730 and earlier, Mali drivers before 48) ([source](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/batching/gpu_preprocessing.rs)), so mobile will run a cheaper rendering path.
- Maturity is lower than desktop: 29 open iOS issues and 43 open Android issues on the Bevy tracker (labels `O-iOS`, `O-Android`, 2026-10-03).
- **Gamepads on phones need work.** Bevy reads gamepads through `gilrs`, whose support table lists Android as unsupported for input, and doesn't list iOS ([gilrs README](https://gitlab.com/gilrs-project/gilrs/-/blob/master/README.md)). A mobile build would need its own Input Device code (Android's game controller APIs, Apple's Game Controller framework). Nothing in the desktop design blocks that, as long as Input Devices sit behind our own input layer.
- The fixed-timestep pattern works the same on phones. A dedicated real-time physics thread would be harder there (thermal throttling, efficiency cores), which is another reason to prefer catch-up stepping.

## 5. Multiplayer and scripting: doors stay open

**Multiplayer.** The main Bevy networking libraries all target Bevy 0.19 today:

- **lightyear** 0.30.1 (2026-09-16): server-client, "Client prediction and rollback", "Snapshot interpolation", "Deterministic replication". Game logic should "run in the FixedMain schedule, but the rendering is done in the PostUpdate schedule" ([docs.rs](https://docs.rs/lightyear/0.30.1/lightyear/)).
- **bevy_ggrs** 0.22.0 (2026-06-26): peer-to-peer rollback. Logic runs in its own `GgrsSchedule`; components and resources are snapshotted with clone or copy strategies; the game must be deterministic ([docs.rs](https://docs.rs/bevy_ggrs/0.22.0/bevy_ggrs/)).
- **bevy_replicon** 0.44.2 and **bevy_renet** 5.0.0: server-authoritative replication and transport.

What this means for us:

- Fixed-timestep stepping is the model these libraries assume. A free-running physics thread would sit outside their tick and rollback machinery, which is a strong reason not to build one.
- An engine-free physics crate with a plain, copyable state and a `step(state, inputs, dt)` shape is the easiest possible thing to snapshot and roll back.
- A network tick at 1 kHz is unrealistic. So the physics crate should be able to run several physics sub-steps per Bevy tick. Then the Bevy tick can later drop to a network-friendly rate (for example 100 to 250 Hz) while physics stays at 1 kHz or more, with identical results.
- Determinism inside Bevy: Bevy runs systems in parallel and only guarantees the order you declare. Keep the whole physics step in one system (or an explicit chain), never split across loosely ordered systems. Cross-platform floating-point determinism is a separate question ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7)).

**Scripting.** `bevy_mod_scripting` 0.21.0 (2026-07-30) supports Lua 5.1 to 5.4, LuaJIT, Luau and Rhai. Scripts reach the game through Bevy's reflection (`bevy_reflect`) and can register "dynamic systems & components". It warns it is "a work in progress" with "significant changes to this API ... anticipated", and doesn't support WASM ([repo](https://github.com/makspll/bevy_mod_scripting)). Implication: game modes and modifiers become scriptable most easily if their state lives in Bevy components that derive `Reflect`. Quad physics itself should not be scriptable; scripts act through the same inputs and Assists a pilot uses. Nothing in the chosen pattern blocks this.

## 6. Surprises for other tickets

- **Input ([#3](https://github.com/BartoszSolkaBD/OpenDrone/issues/3)).** Bevy reads gamepads once per frame (`gilrs_event_system` in `PreUpdate`) and keeps only the latest value; it drops `gilrs`'s per-event timestamps ([bevy_gilrs source](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gilrs/src/lib.rs)). Bevy also filters stick axes by default: a deadzone of ±0.05 around centre and a `threshold: 0.01`, "the minimum value by which input must change before the change is registered" ([gamepad.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input/src/gamepad.rs)). That throws away fine stick resolution a Radio pilot relies on. Faithful 1 kHz physics wants timestamped, unfiltered stick samples, so we likely need our own input layer rather than Bevy's gamepad resources. `gilrs` doesn't cover Android or iOS.
- **Crate split ([#12](https://github.com/BartoszSolkaBD/OpenDrone/issues/12)).** Physics crate: no Bevy dependency, and decide whether it shares Bevy's `glam` version. Its API should accept a run of sub-steps per call with per-sub-step inputs.
- **Determinism ([#7](https://github.com/BartoszSolkaBD/OpenDrone/issues/7), [#11](https://github.com/BartoszSolkaBD/OpenDrone/issues/11)).** Catch-up stepping keeps a deterministic step sequence; only the number of steps per frame varies. Scenarios should drive the physics crate directly, not through Bevy.
- **FPV camera ([#14](https://github.com/BartoszSolkaBD/OpenDrone/issues/14)).** Bevy's defaults (vsync `Fifo`, pipelined rendering) add frames of delay between stick and screen. Choose the present mode and pipelining deliberately, and expose them in Settings ([#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13)). The FPV camera must use the interpolated Quad pose, not the raw physics state, or it will judder.
- **Visuals ([#17](https://github.com/BartoszSolkaBD/OpenDrone/issues/17), [#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13)).** No Solari on Mac. Measure 1440p frame times on the M4 before locking shadows and screen-space effects.

## Sources

Primary sources, all read on 2026-10-03. Bevy source links are pinned to the `v0.19.1` tag.

**Bevy source and examples**
- Fixed timestep: [crates/bevy_time/src/fixed.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/fixed.rs), [virt.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/virt.rs), [lib.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/lib.rs)
- Schedules: [crates/bevy_app/src/main_schedule.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_app/src/main_schedule.rs)
- Example: [examples/movement/physics_in_fixed_timestep.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/movement/physics_in_fixed_timestep.rs)
- Example: [examples/async_tasks/external_source_external_thread.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/async_tasks/external_source_external_thread.rs)
- Input: [crates/bevy_gilrs/src/lib.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gilrs/src/lib.rs), [crates/bevy_input/src/gamepad.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input/src/gamepad.rs)
- Rendering: [pipelined_rendering.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/pipelined_rendering.rs), [gpu_preprocessing.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/batching/gpu_preprocessing.rs), [bevy_solari lib.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_solari/src/lib.rs)
- Lights and effects: [directional_light.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_light/src/directional_light.rs), [point_light.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_light/src/point_light.rs), [ssao](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/ssao/mod.rs), [motion_blur](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_post_process/src/motion_blur/mod.rs)
- Window and loop: [crates/bevy_window/src/window.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_window/src/window.rs), [crates/bevy_winit/src/winit_config.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_winit/src/winit_config.rs)
- Mobile: [examples/mobile](https://github.com/bevyengine/bevy/tree/v0.19.1/examples/mobile), [examples/README.md](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/README.md#platform-specific-examples)
- Dependencies: [crates/bevy_render/Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/Cargo.toml) (wgpu 29.0.3), [crates/bevy_math/Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_math/Cargo.toml) (glam 0.32)

**Bevy project pages, issues and PRs**
- [README](https://github.com/bevyengine/bevy/blob/v0.19.1/README.md), [release process](https://bevy.org/learn/contribute/project-information/release-process/), [releases](https://github.com/bevyengine/bevy/releases)
- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- Migration guides: [0.16 to 0.17](https://bevy.org/learn/migration-guides/0-16-to-0-17/), [0.17 to 0.18](https://bevy.org/learn/migration-guides/0-17-to-0-18/), [0.18 to 0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/), [0.19 to 0.20 (draft)](https://github.com/bevyengine/bevy-website/blob/main/content/learn/migration-guides/0.19-to-0.20.md)
- Issues: [#1259 fixed-timestep interpolation](https://github.com/bevyengine/bevy/issues/1259), [#6183 missed or duplicated inputs with a fixed timestep](https://github.com/bevyengine/bevy/issues/6183), [#25595 macOS rendering corruption in 0.19](https://github.com/bevyengine/bevy/issues/25595); PR [#10077 events in fixed timestep](https://github.com/bevyengine/bevy/pull/10077)

**Graphics and platforms**
- [wgpu-types 29.0.3 `Features`](https://docs.rs/wgpu-types/29.0.3/wgpu_types/struct.Features.html)
- [Apple: M4 announcement](https://www.apple.com/newsroom/2024/05/apple-introduces-m4-chip/), [Apple: Tuning your code's performance for Apple silicon](https://developer.apple.com/documentation/apple-silicon/tuning-your-code-s-performance-for-apple-silicon)
- [Rust std: Windows `sleep`](https://github.com/rust-lang/rust/blob/master/library/std/src/sys/thread/windows.rs), [`spin_sleep`](https://docs.rs/spin_sleep/latest/spin_sleep/)
- [gilrs README](https://gitlab.com/gilrs-project/gilrs/-/blob/master/README.md)
- [Glenn Fiedler, "Fix Your Timestep!"](https://gafferongames.com/post/fix_your_timestep/)

**Ecosystem crates** (versions and dates from crates.io)
- [avian3d 0.7.0](https://docs.rs/avian3d/0.7.0/avian3d/), [bevy_transform_interpolation 0.5.0](https://github.com/Jondolf/bevy_transform_interpolation), [bevy_framepace](https://github.com/aevyrie/bevy_framepace)
- [lightyear 0.30.1](https://docs.rs/lightyear/0.30.1/lightyear/), [bevy_ggrs 0.22.0](https://docs.rs/bevy_ggrs/0.22.0/bevy_ggrs/), [bevy_replicon 0.44.2](https://crates.io/crates/bevy_replicon), [bevy_renet 5.0.0](https://crates.io/crates/bevy_renet)
- [bevy_mod_scripting 0.21.0](https://github.com/makspll/bevy_mod_scripting)

**Not verified.** No published per-tick cost for Bevy's fixed schedules at 1 kHz, no published Bevy frame rates on a base M4, and no official figure for the latency pipelined rendering adds (one frame is our reading of its design).
