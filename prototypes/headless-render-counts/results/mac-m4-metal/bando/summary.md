# Render Work Counts: Bando A+ (bando_a_rooms.glb)

PROTOTYPE output for issue #29. Counts are per frame at 2560x1440, offscreen, MSAA 4x, sun shadows (3 cascades), flat 118 deg camera, Bevy 0.19.1.

- Adapter: Apple M4 | backend Metal | device type IntegratedGpu | driver  
- GPU pipeline statistics: not supported on this backend
- Map: 679 meshes, 10100 triangles in the file
- Flight: 120 frames evenly spaced along "Bando: up the lift shaft, across floor 2, out over the yard" (91 m)
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

- load: 0.20 s
- warmup: 0.14 s
- flight: 1.52 s
- whole run (app start to exit, not counting the build): 1.92 s
- flight frame: mean 12.7 ms, max 116.6 ms (wall clock, includes waiting for the GPU every frame)

## Per flight frame

| Count | Total | Min | Max | Mean |
|---|---:|---:|---:|---:|
| render_passes | 840 | 7 | 7 | 7.0 |
| compute_passes | 840 | 7 | 7 | 7.0 |
| target_pixels | 2394931200 | 19957760 | 19957760 | 19957760.0 |
| draws | 210366 | 1265 | 2238 | 1753.0 |
| draws_direct | 120 | 1 | 1 | 1.0 |
| draws_indirect | 210246 | 1264 | 2237 | 1752.0 |
| indirect_calls | 720 | 6 | 6 | 6.0 |
| indirect_unread | 0 | 0 | 0 | 0.0 |
| draws_nonempty | 210366 | 1265 | 2238 | 1753.0 |
| triangles | 3110056 | 18981 | 33085 | 25917.1 |
| triangles_direct | 120 | 1 | 1 | 1.0 |
| triangles_indirect | 3109936 | 18980 | 33084 | 25916.1 |
| vertices_direct | 360 | 3 | 3 | 3.0 |
| dispatches | 2280 | 19 | 19 | 19.0 |
| render_pipelines_new | 0 | 0 | 0 | 0.0 |
| compute_pipelines_new | 0 | 0 | 0 | 0.0 |
| shader_modules_new | 0 | 0 | 0 | 0.0 |
| bind_groups_new | 3360 | 28 | 28 | 28.0 |
| buffers_new | 701 | 5 | 12 | 5.8 |
| buffer_bytes_new | 66728580 | 509488 | 896680 | 556071.5 |
| textures_new | 0 | 0 | 0 | 0.0 |
| texture_bytes_new | 0 | 0 | 0 | 0.0 |
| upload_bytes | 6933492 | 50004 | 65124 | 57779.1 |
| submits | 240 | 2 | 2 | 2.0 |
| bundles_executed | 0 | 0 | 0 | 0.0 |
| gpu_passes_with_stats | 0 | 0 | 0 | 0.0 |
| gpu_vertex_invocations | 0 | 0 | 0 | 0.0 |
| gpu_clipper_invocations | 0 | 0 | 0 | 0.0 |
| gpu_clipper_primitives_out | 0 | 0 | 0 | 0.0 |
| gpu_fragment_invocations | 0 | 0 | 0 | 0.0 |
| live_buffers | - | 69 | 69 | 69.0 |
| live_buffer_bytes | - | 6493040 | 6557676 | 6553495.0 |
| live_textures | - | 29 | 29 | 29.0 |
| live_texture_bytes | - | 242088412 | 242088412 | 242088412.0 |
| hal_buffer_memory | - | 0 | 0 | 0.0 |
| hal_texture_memory | - | 0 | 0 | 0.0 |
| hal_memory_allocations | - | 0 | 0 | 0.0 |
| hal_buffers | - | 78 | 78 | 78.0 |
| hal_textures | - | 29 | 29 | 29.0 |
| hal_texture_views | - | 71 | 71 | 71.0 |
| hal_bind_groups | - | 56 | 56 | 56.0 |
| hal_render_pipelines | - | 8 | 8 | 8.0 |
| hal_compute_pipelines | - | 36 | 36 | 36.0 |
| hal_shader_modules | - | 43 | 43 | 43.0 |
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
| buffer_bytes_new | 6854504 | 3264588 |
| textures_new | 22 | 7 |
| texture_bytes_new | 19133916 | 222954496 |
| upload_bytes | 21144444 | 498912 |
| submits | 5 | 8 |
| bundles_executed | 0 | 0 |
| gpu_passes_with_stats | 0 | 0 |
| gpu_vertex_invocations | 0 | 0 |
| gpu_clipper_invocations | 0 | 0 |
| gpu_clipper_primitives_out | 0 | 0 |
| gpu_fragment_invocations | 0 | 0 |
| live_buffers | 19 | 69 |
| live_buffer_bytes | 4757012 | 6493040 |
| live_textures | 22 | 29 |
| live_texture_bytes | 19133916 | 242088412 |
| hal_buffer_memory | 0 | 0 |
| hal_texture_memory | 0 | 0 |
| hal_memory_allocations | 0 | 0 |
| hal_buffers | 22 | 78 |
| hal_textures | 22 | 29 |
| hal_texture_views | 45 | 71 |
| hal_bind_groups | 3 | 56 |
| hal_render_pipelines | 1 | 8 |
| hal_compute_pipelines | 33 | 36 |
| hal_shader_modules | 31 | 43 |
| hal_samplers | 30 | 30 |
| cache_pipelines_ok | 32 | 42 |
| cache_pipelines_waiting | 0 | 0 |
| cache_pipelines_err | 0 | 0 |

Shader pipelines compiled: 42 before the flight, 0 during it, 42 in all.

## Passes over the whole flight

Totals over all 120 flight frames.

| Kind | Label | Target | Samples | Passes | Target pixels | Draws | Non-empty draws | Triangles | GPU triangles in | GPU fragments |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| compute | bin unpacking | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering Z slicing pass | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering allocation global pass | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | clustering allocation local pass | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | early_mesh_preprocessing | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | early_prepass_indirect_parameters_building | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| compute | main_indirect_parameters_building | 0x0 | 0x | 120 | 0 | 0 | 0 | 0 | - | - |
| render | clustering count pass | 32x32 | 1x | 120 | 122880 | 120 | 120 | 240 | - | - |
| render | clustering populate pass | 32x32 | 1x | 120 | 122880 | 120 | 120 | 240 | - | - |
| render | main_opaque_pass_3d | 2560x1440 | 4x | 120 | 442368000 | 23730 | 23730 | 357620 | - | - |
| render | shadow_directional_light_0_cascade_0 | 2048x2048 | 1x | 120 | 503316480 | 27725 | 27725 | 408616 | - | - |
| render | shadow_directional_light_0_cascade_1 | 2048x2048 | 1x | 120 | 503316480 | 77071 | 77071 | 1131220 | - | - |
| render | shadow_directional_light_0_cascade_2 | 2048x2048 | 1x | 120 | 503316480 | 81480 | 81480 | 1212000 | - | - |
| render | upscaling | 2560x1440 | 1x | 120 | 442368000 | 120 | 120 | 120 | - | - |
