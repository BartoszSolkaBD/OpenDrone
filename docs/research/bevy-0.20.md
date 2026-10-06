# Bevy 0.20: release status, what changes for us, and what the shared glam costs

Research for [#33](https://github.com/BartoszSolkaBD/OpenDrone/issues/33), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). Checked on 2026-10-05 against Bevy **0.19.1** (latest stable, 2026-08-13) and **0.20.0-rc.2** (2026-09-28). The decision is [ADR-0021](../adr/0021-alpha-starts-on-bevy-0-20.md).

## The answer in plain language

**Start the alpha on Bevy 0.20.** It isn't final yet, but it should be out in mid to late October. Porting our prototypes to it took a handful of small edits. It brings UI parts our screens need and retires the button and shader styles we'd otherwise write on 0.19.1.

**The shared maths library costs nothing we can see.** From 0.20 on, Bevy and parry3d use the same copy of glam, so parry3d's determinism mode also puts Bevy's own maths on the slower one-number-at-a-time path. On the M4 that made no visible difference at 1440p and cost at most about 0.07 ms of CPU per frame in a CPU-bound run.

**Bevy releases cost us one migration every 3.5 to 5.5 months.** Only the game crate and the render-count build touch Bevy. The core crates, Scenarios and physics Work Counts are shielded.

## 1. Release status

| Release | First release candidate | Final | Days in release candidates |
|---|---|---|---|
| 0.15 | 2024-10-22 | 2024-11-29 | 38 |
| 0.16 | 2025-03-19 | 2025-04-24 | 36 |
| 0.17 | 2025-09-12 | 2025-09-30 | 18 |
| 0.18 | 2025-12-17 | 2026-01-13 | 27 |
| 0.19 | 2026-05-13 | 2026-06-19 | 37 |
| 0.20 | 2026-09-15 | not yet (rc.2 on 2026-09-28) | 20 so far |

- The 0.20 milestone still had 12 open items, 173 closed. They include three slowdowns found by Bevy's own benchmarks (transform propagation, wide `for_each` queries, `UnsafeWorldCell`) and a bug in the deprecated `Interaction` component.
- The release notes and the migration guide are hidden drafts on the Bevy website. The guide has 74 entries so far, and its summary list still reads "TODO".
- From past releases, 0.20.0 is likely in mid to late October 2026.

## 2. What 0.20 changes that matters to OpenDrone

**Libraries** (from each crate's `Cargo.toml` at the two tags):

| | 0.19.1 | 0.20.0-rc.2 |
|---|---|---|
| wgpu | 29.0.3 | 30 (30.0.1 today) |
| glam | 0.32 | 0.33.2 or newer (0.33.12 today) |
| rodio (audio), gilrs (gamepads), winit (windows), accesskit | 0.22, 0.11, 0.30, 0.24 | unchanged |
| Minimum Rust | 1.95 | 1.96 (we pin 1.99) |

**Changes we'll meet:**
- **Shaders are written in WESL.** Bevy's own WGSL dialect is gone. Custom shaders change `#import a::b` to `import a::b;` and are renamed from `.wgsl` to `.wesl`. The camera's two shaders needed one line each.
- **Render steps are ordered more loosely.** The `Render`, `RenderGraph`, `Core2d`/`Core3d` and UI system sets now keep an order only between systems that touch the same data. A custom render step that relied on hidden ordering, through an atomic or a channel for example, must now state its order.
- **Extraction is generic.** Data sent to the renderer needs `#[extract_app(RenderApp)]`.
- **Shadows are culled on the GPU.** Each shadow view gets its own GPU-built draw lists, which adds compute passes.
- **wgpu 30:**
  - `get_mapped_range` returns a `Result`.
  - Bindless storage buffers and mesh shaders now work on Metal.
  - Surfaces can pick an HDR colour space. Bevy's own HDR output is listed as future work, not in 0.20.
- **UI:**
  - `em` and `rem` sizes, which suit the Text size setting.
  - Headless tab widgets, which suit the tabbed Settings drawer.
  - `FixedNode` and `InlineImage`.
  - `ui::widgets::Button` and `ui::Interaction` are deprecated, so menus written on 0.19.1 would need rewriting.
- **ECS:**
  - A panic in a system becomes an error that can be handled.
  - Schedule order can be randomised to find hidden order bugs.
  - Change detection can skip whole columns.

**Nothing found for:** input (we read Input Devices through SDL, [ADR-0018](../adr/0018-input-through-sdl3-on-its-own-thread.md)), audio, windowing or mobile. Solari, Bevy's path-traced lighting, now runs on macOS but has no denoiser there; we bake our light anyway.

## 3. Porting effort, measured

| What | Size | Edits for 0.20 |
|---|---|---|
| A plain-Bevy benchmark (the #29 scene) | about 500 lines | 1 import (`Tonemapping` moved) |
| The #29 render-count harness | about 1,000 lines | 2: the `Tonemapping` import and `#[extract_app(RenderApp)]` |
| The #28 FPV camera prototype | about 5,000 lines | 6 small edits (extraction attributes, a moved spline import, `push_encoder` now takes a name, `get_mapped_range` returns a `Result`) plus both shaders rewritten in WESL. It runs, and its debug screenshots match 0.19.1 by eye. |
| The wgpu counting patch ([ADR-0016](../adr/0016-render-work-counts-from-a-counting-wgpu-on-one-lavapipe-thread.md)) on wgpu 30.0.1 | about 1,000 lines | It applies with no conflicts and its hooks land in the renamed `dispatch_workgroups` functions. Two one-line fixes (`get_mapped_range`), then it builds and runs on Metal. |

**The counting patch has a gap on 0.20.** Its triangle read-back reports **0 triangles in the three shadow passes**, although the shadows do draw. 0.20's GPU-culled shadow draws need harness work before triangles can be counted again. The Linux and lavapipe run of the ported patch hasn't been done.

## 4. Render Work Counts, 0.19.1 to 0.20

Same harness, flights and Maps, on the M4 (Metal), 120 frames per Map, per-frame means. The 0.19.1 numbers are #29's committed M4 run.

| Count | Bando A+ 0.19.1 → 0.20 | Skate Park B+A 0.19.1 → 0.20 |
|---|---|---|
| Render passes | 7 → 7 | 7 → 7 |
| Area drawn to | unchanged | unchanged |
| Draw calls | 1,753 → 1,753 | 496 → 496 |
| Compute passes | 7 → 14 | 7 → 14 |
| Dispatches | 19 → 36.6 | 19 → 31 |
| Shader pipelines | 42 → 45 | 42 → 45 |
| Upload bytes | 57.8 → 68.3 kB (+18%) | 30.8 → 36.1 kB (+17%) |
| Buffer memory | 6.6 → 9.7 MB | 12.0 → 15.1 MB |
| Texture memory | 242 MB → 242 MB | 242 MB → 242 MB |
| Triangles | not comparable: shadow passes read 0 (section 3) | not comparable |

The new compute passes are the shadow views' early and late draw-list building (three cascades each), plus "uniform allocation" and, on Skate Park, "sparse buffer updates".

## 5. The shared glam, measured

**The sharing is real.** `cargo tree` shows it:
- On 0.19.1, Bevy's glam 0.32 keeps its fast parallel (SIMD) maths. parry3d's determinism mode (`enhanced-determinism`) switches only parry's own glam 0.33 to `scalar-math` and `libm`.
- On 0.20 there is one glam 0.33, and the mode switches it for Bevy too. At run time a `Vec4` is aligned to 4 bytes instead of 16.
- Bevy 0.19.1 can't even be compiled with glam's `scalar-math` (`bevy_reflect` fails), but 0.20 can.

**The cost** comes from the plain-Bevy benchmark: the #29 scene (2560×1440, MSAA 4×, three shadow cascades, the detail texture) flown along the #28 paths. Pipelined rendering is off, as in the game, and at most 2 frames are in flight. The figures are mean ms per frame over 3 runs × 500 frames.

| Build | Bando 1440p | Skate Park 1440p | Bando, CPU-bound, 1 thread | Skate Park, CPU-bound, 1 thread |
|---|---|---|---|---|
| 0.19.1 | 2.069 | 2.062 | 1.239 | 1.163 |
| 0.19.1 + determinism mode | 2.060 | 2.052 | 1.188 | 1.155 |
| 0.20 | 2.364 | 2.334 | 1.269 | 1.280 |
| **0.20 + determinism mode** | **2.368** | **2.340** | **1.340** | **1.320** |
| 0.20 + `libm` only | 2.342 | 2.341 | 1.330 | 1.317 |

- **At 1440p the GPU sets the pace,** and the mode changes nothing (+0.2%, within noise).
- **CPU-bound** means a 320×180 target with no MSAA, so only Bevy's CPU work counts:
  - With one worker thread, the mode costs +0.07 ms (Bando) and +0.04 ms (Skate Park).
  - With all threads it is lost in the noise (+0.05 / −0.04 ms, spread ±0.03–0.06).
- **The `libm` half alone costs about the same as the whole mode,** so splitting the mode would gain nothing.
- **0.20 itself is about 0.28 ms per frame slower on the GPU in this scene,** probably from the new shadow compute passes. That is 1.3% of the 22 ms a 45 fps frame allows.

**The FPV camera prototype on 0.20.** Its quick bench: Bando, windowless at 1440p, Metal timestamps, mean frame ms.

| Build | Plain camera | Analog | Analog, merged pass ×5 | Digital | Digital, merged pass ×5 |
|---|---|---|---|---|---|
| 0.19.1 (#28, 2026-10-04) | 4.45 | 6.65 | 12.07 | 6.60 | 7.38 |
| 0.19.1 (2026-10-05) | 4.41 | 6.68 | 11.32 | 5.90 | 7.49 |
| 0.20 (3 runs) | 4.27–4.29 | 6.22 | 11.05 | 5.48–5.50 | 7.00 |

- The camera's own steps are unchanged: Digital 0.49 ms on both, and one Analog merged pass about 1.2 ms on both (by the ×5 probe).
- **The camera's GPU timer misbehaved with the determinism mode on.** It broke in 4 of 4 runs, either stalling about 10 s per measured frame or recording no timestamps:
  - glam's `scalar-math` alone broke it in 1 of 2 runs;
  - `libm` alone in 0 of 2;
  - the mode off in 0 of 3.
- The picture was the same with the mode on and off. The cause wasn't found; it may be a race in the prototype's own timer code.

**The sharing can come and go.** glam 0.34.0 came out on 2026-10-03, but Bevy's main branch (glam 0.33.9) and parry3d's glamx 0.3.1 (glam 0.33) haven't moved. Any later Bevy or parry3d upgrade can switch the sharing on or off.

## 6. What a Bevy release costs a project

- **Cadence:** a breaking release every 3.5 to 5.5 months, with 2 to 5 release candidates over 2.5 to 5 weeks. Patch releases (0.N.1) followed 1 day to 8 weeks later.
- **Size:** migration guides of 117 (0.17), 64 (0.18) and 103 (0.19) entries; 74 for 0.20 so far ([bevy-app-shell.md](bevy-app-shell.md) section 2).
- **Other projects:**
  - Bevy's official 2D template (`bevy_new_2d`) opened its 0.19 upgrade 8 days after the release and merged it 40 days later (13 files).
  - The 3D template `foxtrot` upgraded to 0.16 within 5 days and to 0.18 within 18 days (36 files). It moved to 0.17 during the release candidates and is still on 0.18. It depends on at least nine Bevy add-on crates.
- **Add-on crates follow fast:** within days to weeks of a release. For 0.20, `bevy_egui` 0.43.0-rc.1 and `bevy_kira_audio` 0.27.0-rc.1 already exist.
- **What an upgrade touches for us:** the `opendrone` game crate (render steps, UI, shaders), the wgpu counting patch, the render Work Count baselines, and any Bevy add-on crates.

## Sources

- Bevy versions and dates: [crates.io](https://crates.io/crates/bevy/versions), [GitHub releases](https://github.com/bevyengine/bevy/releases), [the 0.20 milestone](https://github.com/bevyengine/bevy/milestone/43), and issues [#25839](https://github.com/bevyengine/bevy/issues/25839), [#25840](https://github.com/bevyengine/bevy/issues/25840), [#25841](https://github.com/bevyengine/bevy/issues/25841)
- [Bevy 0.20 release notes (draft)](https://github.com/bevyengine/bevy-website/blob/main/content/news/2026-09-17-bevy-0.20/index.md) and [the 0.19 to 0.20 migration guide (draft)](https://github.com/bevyengine/bevy-website/blob/main/content/learn/migration-guides/0.19-to-0.20.md)
- Dependency versions: `crates/bevy_render`, `bevy_math`, `bevy_audio`, `bevy_gilrs`, `bevy_winit` and the root `Cargo.toml` at [v0.19.1](https://github.com/bevyengine/bevy/tree/v0.19.1) and [v0.20.0-rc.2](https://github.com/bevyengine/bevy/tree/v0.20.0-rc.2)
- [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md), v30.0.0 and v30.0.1
- glam's CHANGELOG in the published 0.33.2 and 0.34.0 crates. parry3d-f64 0.31.1's `Cargo.toml`: `enhanced-determinism = ["simba/libm_force", "indexmap", "glamx/libm", "glamx/scalar-math"]`
- [bevy_new_2d #489](https://github.com/TheBevyFlock/bevy_new_2d/pull/489); foxtrot [#352](https://github.com/janhohenheim/foxtrot/pull/352), [#428](https://github.com/janhohenheim/foxtrot/pull/428) and [#448](https://github.com/janhohenheim/foxtrot/pull/448)
- Measurements: on the dev M4 (macOS 26.6.2) on 2026-10-05. The trial code and results are kept on the local branch `prototype/bevy-020-trial`, which isn't pushed.
