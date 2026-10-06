# The alpha starts on Bevy 0.20, and each later Bevy version is its own gated ticket

The game crate, `opendrone`, is built on Bevy 0.20 from its first ticket.
- If 0.20.0 isn't out by then, that ticket uses the newest 0.20 release candidate, pinned exactly, and Bevy add-on crates may be release candidates too.
- The move to 0.20.0 follows the patch-release rules below, within a week of the release.
- The core crates come first and never use Bevy, so nothing waits on this.

**We accept the shared glam.** From 0.20 on, Bevy and parry3d use one copy of glam. So parry3d's determinism mode ([ADR-0004](0004-parry3d-geometry-only-f64.md)) also puts Bevy's own maths on glam's scalar path with `libm`.
- Measured on the M4, it made no visible difference at 1440p, and cost at most about 0.07 ms of CPU per frame in a CPU-bound run.
- Every frame measurement runs with the mode on.

**Later Bevy versions are their own tickets** (#15 already said major Bevy versions become tickets):

- **Who starts it.**
  - A monthly workflow in this repo opens an "Upgrade to Bevy 0.N" ticket, labelled `ready-for-agent`, when 0.N.0 ships. It never fires for a release candidate.
  - Agents treat that issue as the maintainer's words. This is a narrow exception to [ADR-0010](0010-phase-1-agent-prs-merge-automatically.md), like the trust already given to Dependabot's PRs. Only code on main can run the workflow, and its issue text is fixed.
  - Dependabot ignores Bevy's 0.N jumps and wgpu's major versions, because those never build without code changes.
- **When.**
  - An agent starts once every Bevy add-on crate we use has a matching release, and aims to merge within 6 weeks of Bevy's release.
  - No upgrade happens between a release's Frame Check and its tag.
  - If it isn't merged after 6 weeks, because an add-on crate lags or the port is stuck, the ticket moves to `needs-maintainer` with three choices: keep waiting, replace the crate, or take it from git for a while, with the written reason #15 requires.
- **What it must pass before it merges:**
  1. Every usual required check.
  2. Physics Work Counts and every Scenario fingerprint don't move at all. The core never sees Bevy, so any move is a bug.
  3. The counting wgpu patch ([ADR-0016](0016-render-work-counts-from-a-counting-wgpu-on-one-lavapipe-thread.md)) is ported to the new wgpu and proven complete on GitHub's Linux machine, as #29 did: triangles equal the GPU's own count, and two runs match exactly.
  4. **A new Red Flag, "Bevy or wgpu version changed", that waits for the maintainer.** The maintainer runs the Frame Check (at least 45 fps) and looks at both Maps in both Video Looks before the merge. Until `cargo xtask frame-check` exists, the visual check is the whole step.
     - It fires only when Bevy moves to a new 0.N version, together with the major wgpu version that comes with it.
     - It never fires for a patch release or for the move from a release candidate to the final release.
- **Work Count baselines.**
  - The upgrade PR's "Heavier:" or "Slower:" lines name the upgrade and list every count that moved.
  - Once the Reviewer accepts and the PR merges, main's numbers are the new baseline. There's no separate reset step.
  - The Review Report shows the upgrade's share of the "since the last release" total on its own line, so later small rises can't hide behind it.
- **Patch releases** (Bevy 0.N.x, wgpu x.y.z) each arrive as their own Dependabot PR, outside the grouped one, and follow the normal rules. A wgpu patch PR must re-apply the counting patch, and the counting job fails if the patch doesn't apply.

Decided in [#33](https://github.com/BartoszSolkaBD/OpenDrone/issues/33). The measurements are in [docs/research/bevy-0.20.md](../research/bevy-0.20.md).

## Considered options

- **Stay on 0.19.1 for the whole alpha.** Rejected:
  - The menus would be built on `Button` and `Interaction`, which 0.20 deprecates.
  - The FPV camera's shaders would be written in the WGSL dialect 0.20 removes.
  - We'd miss em/rem text sizing (the Text size setting) and headless tabs (the Settings drawer).
  - We'd pay for one migration, a re-baseline and a patch port anyway, during or just after the alpha.
- **Start on 0.19.1, then make 0.20 the first upgrade ticket.** Rejected for the same double work, early on.
- **Keep Bevy and parry3d on separate glam versions.** Rejected: it means staying on 0.19.1 or using an older parry3d, to avoid a cost we can't measure.
- **Switch on only the `libm` half of parry3d's determinism mode.** Rejected: it costs the same as the whole mode, and it would be a house-rule exception.
- **Stay one Bevy version behind, or freeze Bevy until 0.1.0.** Rejected: bigger jumps later and longer on retired APIs, while add-on crates catch up within days to weeks anyway.
- **Let Bevy upgrades merge automatically like any PR.** Rejected: Work Counts can't see Bevy changes ([ADR-0013](0013-performance-gates-count-work-not-time.md)), so a person checks the frame rate and the picture.
- **Have the maintainer label each upgrade ticket.** Rejected in favour of trusting our own workflow's fixed-template issue.

## Consequences

- **One exception to "the Frame Check never gates a PR"** (#15, ADR-0013): a Bevy 0.N upgrade waits for the maintainer's Frame Check and visual check.
- **One exception to "agents act only on the maintainer's text"** (ADR-0010): issues opened by the Bevy-upgrade workflow from its fixed template count as the maintainer's.
- **Only the game crate and the counting build touch Bevy.** The core crates, Scenarios and physics Work Counts are shielded, which is what gate 2 checks.
- **Porting to 0.20 was small.**
  - A plain-Bevy benchmark needed 1 edit and the #29 harness 2.
  - The #28 camera prototype needed 6 small edits plus a one-line rewrite of each of its two shaders in WESL, Bevy's new shader language.
  - Custom render steps must now state their order explicitly, because 0.20 orders render sets more loosely.
- **The counting patch on wgpu 30 isn't complete yet.**
  - It applies cleanly and needs 2 one-line fixes.
  - On 0.20 it reads 0 triangles for the shadow passes, which are now culled on the GPU.
  - The ticket that builds the render gate fixes that read-back and proves it on the Linux runner.
- **Render counts move with 0.20.** On the M4, over the same flights:
  - compute passes go from 7 to 14, dispatches rise 63–93%, shader pipelines go from 42 to 45, uploads rise about 17%, and buffer memory grows about 3 MB;
  - render passes, draw calls, pass area and texture memory stay the same.
  0.20 also cost about 0.28 ms more GPU time per frame at 1440p in the #29 scene.
- **A GPU-timer glitch to chase.**
  - With the determinism mode on, the camera prototype's own Metal-timestamp timer stalled or lost its timestamps in 4 of 4 runs, while the picture stayed the same.
  - The ticket that builds the Frame Check or any GPU timing reproduces it with the mode on, then fixes it or reports it upstream.
- **The sharing can come and go.** glam 0.34 is out, but Bevy's main branch and parry3d are still on 0.33. Any later Bevy or parry3d upgrade can switch the sharing on or off. Frame measurements stay with the mode on either way.
