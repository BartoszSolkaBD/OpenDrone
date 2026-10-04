# Render Work Counts: Skate Park B+A (skate_park_ba.glb)

PROTOTYPE output for issue #29. Counts are per frame at 2560x1440, offscreen, MSAA 4x, sun shadows (3 cascades), flat 118 deg camera, Bevy 0.19.1.

- Adapter: Apple M4 | backend Metal | device type IntegratedGpu | driver  
- GPU pipeline statistics: not supported on this backend
- Map: 193 meshes, 44956 triangles in the file
- Flight: 120 frames evenly spaced along "Skate Park: pool bowl, cradle, ring, full pipe, snake run" (161 m)
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

- load: 0.15 s
- warmup: 0.13 s
- flight: 1.74 s
- whole run (app start to exit, not counting the build): 2.09 s
- flight frame: mean 14.5 ms, max 93.6 ms (wall clock, includes waiting for the GPU every frame)

## Per flight frame

| Count | Total | Min | Max | Mean |
|---|---:|---:|---:|---:|
| render_passes | 840 | 7 | 7 | 7.0 |
| compute_passes | 840 | 7 | 7 | 7.0 |
| target_pixels | 2394931200 | 19957760 | 19957760 | 19957760.0 |
| draws | 59459 | 330 | 577 | 495.5 |
| draws_direct | 120 | 1 | 1 | 1.0 |
| draws_indirect | 59339 | 329 | 576 | 494.5 |
| indirect_calls | 720 | 6 | 6 | 6.0 |
| indirect_unread | 0 | 0 | 0 | 0.0 |
| draws_nonempty | 59459 | 330 | 577 | 495.5 |
| triangles | 19319364 | 119353 | 175241 | 160994.7 |
| triangles_direct | 120 | 1 | 1 | 1.0 |
| triangles_indirect | 19319244 | 119352 | 175240 | 160993.7 |
| vertices_direct | 360 | 3 | 3 | 3.0 |
| dispatches | 2280 | 19 | 19 | 19.0 |
| render_pipelines_new | 0 | 0 | 0 | 0.0 |
| compute_pipelines_new | 0 | 0 | 0 | 0.0 |
| shader_modules_new | 0 | 0 | 0 | 0.0 |
| bind_groups_new | 3360 | 28 | 28 | 28.0 |
| buffers_new | 807 | 5 | 19 | 6.7 |
| buffer_bytes_new | 63420124 | 509488 | 633144 | 528501.0 |
| textures_new | 0 | 0 | 0 | 0.0 |
| texture_bytes_new | 0 | 0 | 0 | 0.0 |
| upload_bytes | 3692564 | 28004 | 54964 | 30771.4 |
| submits | 240 | 2 | 2 | 2.0 |
| bundles_executed | 0 | 0 | 0 | 0.0 |
| gpu_passes_with_stats | 0 | 0 | 0 | 0.0 |
| gpu_vertex_invocations | 0 | 0 | 0 | 0.0 |
| gpu_clipper_invocations | 0 | 0 | 0 | 0.0 |
| gpu_clipper_primitives_out | 0 | 0 | 0 | 0.0 |
| gpu_fragment_invocations | 0 | 0 | 0 | 0.0 |
| live_buffers | - | 69 | 69 | 69.0 |
| live_buffer_bytes | - | 11934184 | 12002076 | 11988586.0 |
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
| draws | 0 | 1848 |
| draws_direct | 0 | 4 |
| draws_indirect | 0 | 1844 |
| indirect_calls | 0 | 24 |
| indirect_unread | 0 | 0 |
| draws_nonempty | 0 | 1848 |
| triangles | 0 | 654004 |
| triangles_direct | 0 | 4 |
| triangles_indirect | 0 | 654000 |
| vertices_direct | 0 | 12 |
| dispatches | 0 | 76 |
| render_pipelines_new | 1 | 7 |
| compute_pipelines_new | 31 | 3 |
| shader_modules_new | 58 | 24 |
| bind_groups_new | 4 | 113 |
| buffers_new | 23 | 66 |
| buffer_bytes_new | 13767680 | 2316876 |
| textures_new | 22 | 7 |
| texture_bytes_new | 19133916 | 222954496 |
| upload_bytes | 25988292 | 185760 |
| submits | 5 | 8 |
| bundles_executed | 0 | 0 |
| gpu_passes_with_stats | 0 | 0 |
| gpu_vertex_invocations | 0 | 0 |
| gpu_clipper_invocations | 0 | 0 |
| gpu_clipper_primitives_out | 0 | 0 |
| gpu_fragment_invocations | 0 | 0 |
| live_buffers | 19 | 69 |
| live_buffer_bytes | 11145868 | 11934184 |
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
| render | main_opaque_pass_3d | 2560x1440 | 4x | 120 | 442368000 | 9708 | 9708 | 4104112 | - | - |
| render | shadow_directional_light_0_cascade_0 | 2048x2048 | 1x | 120 | 503316480 | 5924 | 5924 | 4506796 | - | - |
| render | shadow_directional_light_0_cascade_1 | 2048x2048 | 1x | 120 | 503316480 | 20325 | 20325 | 5313476 | - | - |
| render | shadow_directional_light_0_cascade_2 | 2048x2048 | 1x | 120 | 503316480 | 23142 | 23142 | 5394380 | - | - |
| render | upscaling | 2560x1440 | 1x | 120 | 442368000 | 120 | 120 | 120 | - | - |
