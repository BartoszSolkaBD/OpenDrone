## Where this comes from

Bevy {release} has shipped. This issue was opened by this repo's own monthly Bevy upgrade workflow (`.github/workflows/bevy-upgrade.yml`), from its fixed template (`.github/bevy-upgrade-issue.md`). Its text counts as the maintainer's words ([ADR-0021](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md)): the workflow fills in only the version numbers.

## What to build

Move the game from Bevy {pinned} to Bevy {version}, with the wgpu that Bevy {version} draws with, and change nothing a pilot or a Scenario can see.

Context: [ADR-0021](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md); [ADR-0016](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0016-render-work-counts-from-a-counting-wgpu-on-one-lavapipe-thread.md); the Development deep dive (Bevy upgrades); [docs/research/bevy-0.20.md](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/research/bevy-0.20.md) for how the last port went.

## When to start

- Start once every Bevy add-on crate the game uses has a release that works with Bevy {version}. Until then, leave this issue as it is.
- Aim to merge within 6 weeks of Bevy {release}'s release.
- No upgrade happens between a release's Frame Check and its tag.
- If it isn't merged 6 weeks after the release, because an add-on crate lags or the port is stuck, move this issue to `needs-maintainer` with three choices: keep waiting, replace the crate, or take it from git for a while, with a written reason.

## Acceptance criteria

- [ ] Bevy is pinned exactly to {release} or a later {version}.x patch release, and wgpu to the version that Bevy release uses. `Cargo.lock` is committed.
- [ ] Every usual required check passes.
- [ ] Physics Work Counts and every Scenario fingerprint don't move at all. The core never sees Bevy, so any move is a bug.
- [ ] The counting wgpu patch (ADR-0016) is ported to the new wgpu and proven complete on GitHub's Linux machine: triangles equal the GPU's own count, and two runs match exactly.
- [ ] The PR's "Heavier:" or "Slower:" lines name the upgrade and list every render Work Count that moved.
- [ ] The "Bevy or wgpu version changed" Red Flag waits for the maintainer, who runs the Frame Check (at least 45 fps) and looks at both Maps in both Video Looks before the merge.
