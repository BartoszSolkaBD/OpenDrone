# Render Work Counts come from a counting wgpu, on Mesa's software GPU with one thread

The render Work Counts of [ADR-0013](0013-performance-gates-count-work-not-time.md) are counted inside wgpu, the graphics layer under Bevy, by a small patch applied to the exact wgpu version Bevy uses. They're measured on GitHub's Linux machine with Mesa's software Vulkan driver (lavapipe) running on one thread (`LP_NUM_THREADS=1`).

On that setup, two runs of the same flight give byte-identical counts for every frame:
- render passes and the area each one draws to
- draw calls
- triangles
- pixels shaded
- shader pipelines compiled
- buffer and texture memory

We chose this because Bevy's own render diagnostics miss passes (shadows, full-screen passes) and keep only the latest frame. Counting at wgpu sees every pass Bevy, our code and any plugin records. Lavapipe on several threads changes its pixels-shaded count between identical runs, by up to 15% for one pass on the runner, which would swamp a 2% gate. Every other count repeats either way.

Proven in [#29](https://github.com/BartoszSolkaBD/OpenDrone/issues/29) with a throwaway harness on branch `prototype/headless-render-counts`. It flew 60 frames through Bando A+ and Skate Park B+A at 2560×1440, twice, on `ubuntu-latest` (4 cores, Mesa 25.2.8).

## How each count is made

- **Draw calls, passes, pipelines, memory, uploads.** Counted as Bevy hands them to wgpu.
- **Pass area.** Width × height of what each pass draws to.
- **Triangles.** Bevy decides most draws on the GPU, so their sizes are read back after each frame. Bevy has a debug switch that allows this. The result matched the GPU's own triangle count exactly.
- **Pixels shaded.** A Vulkan statistics query around every pass. Not available on Metal.
- **GPU memory.** Two figures:
  - the sizes Bevy asked for, counted the same everywhere
  - what the Vulkan driver actually allocated
- **The flight.** A fixed list of camera poses, one per frame, with time advancing exactly 1/60 s per frame. Shader pipelines are compiled on the spot instead of on background threads. Loading frames are only totalled, because how many there are depends on the disk.

## Considered options

- **Bevy's render diagnostics.** Rejected. Only some of Bevy's passes report, results for a frame can be overwritten before they're read, and draw calls aren't counted at all.
- **wgpu's built-in counters alone.** Kept for driver memory and object totals. They don't count draws, triangles or passes.
- **A Vulkan layer or a full wgpu trace.** Rejected. Both are heavier and Linux-only, and they're harder to tie to frames.
- **Lavapipe on all cores, dropping pixels shaded or giving it a tolerance.** Rejected. It's about 2.3× faster, but it loses a named Work Count or blurs the 2% line.
- **Telling Bevy to draw from the CPU,** so triangles are known without a read-back. Rejected: it changes the drawing path the game really uses.

## Consequences

- **Upgrades.** The patch fits one wgpu version, 29.0.4 for Bevy 0.19.1. Any Bevy upgrade that moves wgpu needs the patch ported, and main's numbers may move. Bevy 0.20 uses wgpu 30. _Update: tried in [#33](https://github.com/BartoszSolkaBD/OpenDrone/issues/33). The patch applies to wgpu 30.0.1 with two one-line fixes, but on Bevy 0.20 it reads 0 triangles for the shadow passes, which are now culled on the GPU, so that read-back needs work. Each upgrade must prove the port complete on the Linux runner; see [ADR-0021](0021-alpha-starts-on-bevy-0-20.md)._
- **Shipping.** The game ships with plain wgpu. Only the counting build uses the patched copy, made from wgpu's published source and a patch file in the repo.
- **Where counts are compared.** Only on the same kind of machine. The M4 (Metal) matches the runner on:
  - passes and their area
  - draws and triangles
  - pipelines and created objects

  It doesn't match on upload bytes (up to 9% apart) or driver memory, and it can't count pixels shaded.
- **Cost.** One thread is slow:
  - about 3.8 s per 1440p frame in Bando and 2.1 s in Skate Park on the runner
  - so the gate flies a sample of frames along the recorded flight, not every frame
  - the prototype's whole job took about 25 minutes, 8.5 of them a cold build
- **Pipelines.** Counting them predicts the first-launch compile stall, as ADR-0013 says. Plain Bevy with these Maps compiles 42, all before the first frame of flight.
