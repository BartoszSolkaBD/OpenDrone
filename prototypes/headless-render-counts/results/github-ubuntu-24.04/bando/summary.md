# Render Work Counts: Bando A+ (bando_a_rooms.glb)

PROTOTYPE output for issue #29. Counts are per frame at 2560x1440, offscreen, MSAA 4x, sun shadows (3 cascades), flat 118 deg camera, Bevy 0.19.1.

- Adapter: llvmpipe (LLVM 20.1.2, 256 bits) | backend Vulkan | device type Cpu | driver llvmpipe Mesa 25.2.8-0ubuntu0.24.04.4 (LLVM 20.1.2)
- GPU pipeline statistics: yes (per render pass)
- Map: 679 meshes, 10100 triangles in the file
- Flight: 60 frames evenly spaced along "Bando: up the lift shaft, across floor 2, out over the yard" (91 m)
- Frames before the flight: 5 loading (varies with disk and threads), 4 warm-up

## Where each count comes from

- `render_passes`, `compute_passes`, `draws*`, `indirect_calls`, `dispatches`, `*_new`, `upload_bytes`, `submits`: counted at wgpu's API by our patch (`wgpu-counting.patch`), every caller included.
- `target_pixels`: width x height of each render pass's first attachment, summed (MSAA samples not multiplied in).
- `triangles_direct`: from each direct draw's arguments. `triangles_indirect` and `draws_nonempty`: Bevy's GPU-driven draws, from their arguments read back after the frame (after GPU culling).
- `gpu_*`: a pipeline-statistics query around every render pass (Vulkan and DX12 only): vertex shader runs, triangles reaching the clipper, triangles leaving it, and fragment shader runs (the pixels actually shaded).
- `live_*`: buffers and textures alive at the end of the frame, sized from their descriptors (textures: every mip, layer and sample).
- `hal_*`: wgpu's own counters (`counters` feature). `hal_buffer_memory` and `hal_texture_memory` are the driver allocations on Vulkan and DX12; they read 0 on Metal.
- `cache_*`: Bevy's pipeline cache: shader pipelines ready, waiting, failed.

## Time (not a Work Count; never compared)

- load: 0.63 s
- warmup: 20.02 s
- flight: 229.52 s
- whole run (app start to exit, not counting the build): 259.69 s
- flight frame: mean 3825.4 ms, max 6302.7 ms (wall clock, includes waiting for the GPU every frame)

## Per flight frame

| Count | Total | Min | Max | Mean |
|---|---:|---:|---:|---:|
| render_passes | 420 | 7 | 7 | 7.0 |
| compute_passes | 420 | 7 | 7 | 7.0 |
| target_pixels | 1197465600 | 19957760 | 19957760 | 19957760.0 |
| draws | 105373 | 1269 | 2238 | 1756.2 |
| draws_direct | 60 | 1 | 1 | 1.0 |
| draws_indirect | 105313 | 1268 | 2237 | 1755.2 |
| indirect_calls | 360 | 6 | 6 | 6.0 |
| indirect_unread | 0 | 0 | 0 | 0.0 |
| draws_nonempty | 105373 | 1269 | 2238 | 1756.2 |
| triangles | 1558032 | 18969 | 33085 | 25967.2 |
| triangles_direct | 60 | 1 | 1 | 1.0 |
| triangles_indirect | 1557972 | 18968 | 33084 | 25966.2 |
| vertices_direct | 180 | 3 | 3 | 3.0 |
| dispatches | 1140 | 19 | 19 | 19.0 |
| render_pipelines_new | 0 | 0 | 0 | 0.0 |
| compute_pipelines_new | 0 | 0 | 0 | 0.0 |
| shader_modules_new | 0 | 0 | 0 | 0.0 |
| bind_groups_new | 1680 | 28 | 28 | 28.0 |
| buffers_new | 354 | 5 | 12 | 5.9 |
| buffer_bytes_new | 33571656 | 509488 | 896680 | 559527.6 |
| textures_new | 0 | 0 | 0 | 0.0 |
| texture_bytes_new | 0 | 0 | 0 | 0.0 |
| upload_bytes | 3321888 | 47604 | 62628 | 55364.8 |
| submits | 120 | 2 | 2 | 2.0 |
| bundles_executed | 0 | 0 | 0 | 0.0 |
| gpu_passes_with_stats | 420 | 7 | 7 | 7.0 |
| gpu_vertex_invocations | 4673856 | 56903 | 99251 | 77897.6 |
| gpu_clipper_invocations | 1558032 | 18969 | 33085 | 25967.2 |
| gpu_clipper_primitives_out | 1563276 | 18885 | 33339 | 26054.6 |
| gpu_fragment_invocations | 716121829 | 8273092 | 16778937 | 11935363.8 |
| live_buffers | - | 69 | 69 | 69.0 |
| live_buffer_bytes | - | 6490576 | 6554772 | 6550296.3 |
| live_textures | - | 29 | 29 | 29.0 |
| live_texture_bytes | - | 242088412 | 242088412 | 242088412.0 |
| hal_buffer_memory | - | 7277236 | 7341432 | 7336956.3 |
| hal_texture_memory | - | 242097728 | 242097728 | 242097728.0 |
| hal_memory_allocations | - | 0 | 0 | 0.0 |
| hal_buffers | - | 73 | 73 | 73.0 |
| hal_textures | - | 0 | 0 | 0.0 |
| hal_texture_views | - | 71 | 71 | 71.0 |
| hal_bind_groups | - | 31 | 31 | 31.0 |
| hal_render_pipelines | - | 8 | 8 | 8.0 |
| hal_compute_pipelines | - | 34 | 34 | 34.0 |
| hal_shader_modules | - | 41 | 41 | 41.0 |
| hal_samplers | - | 30 | 30 | 30.0 |
| cache_pipelines_ok | - | 42 | 42 | 42.0 |
| cache_pipelines_waiting | - | 0 | 0 | 0.0 |
| cache_pipelines_err | - | 0 | 0 | 0.0 |

## Before the flight (load + warm-up)

| Count | Load | Warm-up |
|---|---:|---:|
| render_passes | 0 | 28 |
| compute_passes | 0 | 28 |
| target_pixels | 0 | 79831040 |
| draws | 0 | 8952 |
| draws_direct | 0 | 4 |
| draws_indirect | 0 | 8948 |
| indirect_calls | 0 | 24 |
| indirect_unread | 0 | 0 |
| draws_nonempty | 0 | 8952 |
| triangles | 0 | 132340 |
| triangles_direct | 0 | 4 |
| triangles_indirect | 0 | 132336 |
| vertices_direct | 0 | 12 |
| dispatches | 0 | 76 |
| render_pipelines_new | 1 | 7 |
| compute_pipelines_new | 31 | 3 |
| shader_modules_new | 58 | 24 |
| bind_groups_new | 4 | 113 |
| buffers_new | 23 | 66 |
| buffer_bytes_new | 6854504 | 3262124 |
| textures_new | 22 | 7 |
| texture_bytes_new | 19133916 | 222954496 |
| upload_bytes | 21144444 | 489056 |
| submits | 5 | 8 |
| bundles_executed | 0 | 0 |
| gpu_passes_with_stats | 0 | 28 |
| gpu_vertex_invocations | 0 | 397004 |
| gpu_clipper_invocations | 0 | 132340 |
| gpu_clipper_primitives_out | 0 | 133356 |
| gpu_fragment_invocations | 0 | 50202068 |
| live_buffers | 19 | 69 |
| live_buffer_bytes | 4757012 | 6490576 |
| live_textures | 22 | 29 |
| live_texture_bytes | 19133916 | 242088412 |
| hal_buffer_memory | 5543672 | 7277236 |
| hal_texture_memory | 19142208 | 242097728 |
| hal_memory_allocations | 0 | 0 |
| hal_buffers | 23 | 73 |
| hal_textures | 0 | 0 |
| hal_texture_views | 45 | 71 |
| hal_bind_groups | 2 | 31 |
| hal_render_pipelines | 1 | 8 |
| hal_compute_pipelines | 31 | 34 |
| hal_shader_modules | 29 | 41 |
| hal_samplers | 30 | 30 |
| cache_pipelines_ok | 32 | 42 |
| cache_pipelines_waiting | 0 | 0 |
| cache_pipelines_err | 0 | 0 |

Shader pipelines compiled: 42 before the flight, 0 during it, 42 in all.

## Passes over the whole flight

Totals over all 60 flight frames.

| Kind | Label | Target | Samples | Passes | Target pixels | Draws | Non-empty draws | Triangles | GPU triangles in | GPU fragments |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| compute | bin unpacking | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering Z slicing pass | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering allocation global pass | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering allocation local pass | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | early_mesh_preprocessing | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | early_prepass_indirect_parameters_building | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| compute | main_indirect_parameters_building | 0x0 | 0x | 60 | 0 | 0 | 0 | 0 | - | - |
| render | clustering count pass | 32x32 | 1x | 60 | 61440 | 60 | 60 | 120 | 120 | 60 |
| render | clustering populate pass | 32x32 | 1x | 60 | 61440 | 60 | 60 | 120 | 120 | 60 |
| render | main_opaque_pass_3d | 2560x1440 | 4x | 60 | 221184000 | 12031 | 12031 | 181392 | 181392 | 494937709 |
| render | shadow_directional_light_0_cascade_0 | 2048x2048 | 1x | 60 | 251658240 | 13878 | 13878 | 204660 | 204660 | 0 |
| render | shadow_directional_light_0_cascade_1 | 2048x2048 | 1x | 60 | 251658240 | 38544 | 38544 | 565680 | 565680 | 0 |
| render | shadow_directional_light_0_cascade_2 | 2048x2048 | 1x | 60 | 251658240 | 40740 | 40740 | 606000 | 606000 | 0 |
| render | upscaling | 2560x1440 | 1x | 60 | 221184000 | 60 | 60 | 60 | 60 | 221184000 |
