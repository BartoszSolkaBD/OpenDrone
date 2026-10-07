# Summary

<!--
The book's table of contents (book.toml, at the repo's root, builds it).
Every Markdown page under docs/ must be listed here once; the book won't build
otherwise. Pages in docs/book/ exist only to arrange the book: part intros, and
pages that show a file from outside docs/ (such as CONTEXT.md) or are made when
the book is built (the Scenario catalogue, the Feel Test logs, the crate list).
-->

[OpenDrone](README.md)

# Pilot guide

- [Flying OpenDrone](pilot-guide.md)

# How OpenDrone works

- [The map](book/map.md)
  - [Flying](context/flying.md)
  - [Input](context/input.md)
  - [World and content](context/world.md)
  - [Camera and video](context/camera.md)
  - [Simulation](context/simulation.md)
  - [Verification](context/verification.md)
  - [Screens and navigation](context/screens.md)
  - [Sound](context/sound.md)
  - [Development](context/development.md)
- [The unit list](units.md)

# Decisions

- [Decisions](book/decisions.md)
  - [0001 Bit-exact determinism](adr/0001-bit-exact-determinism-with-ordinary-floats.md)
  - [0002 Starting states spell out everything](adr/0002-scenario-starting-state-spells-out-everything.md)
  - [0003 Crate split and Flight Inputs](adr/0003-crate-split-and-flight-inputs.md)
  - [0004 parry3d for geometry only, in 64-bit](adr/0004-parry3d-geometry-only-f64.md)
  - [0005 Prop Wash from physics](adr/0005-prop-wash-from-physics.md)
  - [0006 Thrust from a motor model](adr/0006-thrust-from-motor-model.md)
  - [0007 An emulated Radio Link](adr/0007-emulated-radio-link.md)
  - [0008 Copy Betaflight 2026.6](adr/0008-copy-betaflight-2026-6-translate-older-tunes.md)
  - [0009 Fisheye from one warped render](adr/0009-fisheye-from-one-warped-render.md)
  - [0010 Agent PRs merge automatically](adr/0010-phase-1-agent-prs-merge-automatically.md)
  - [0011 Packs are data only](adr/0011-packs-are-data-only-toml-named-pack-item.md)
  - [0012 Crashes behave like a real quad](adr/0012-crashes-behave-like-a-real-quad.md)
  - [0013 Performance gates count work](adr/0013-performance-gates-count-work-not-time.md)
  - [0014 Licences for libraries and assets](adr/0014-licences-for-libraries-and-assets.md)
  - [0015 A Tune is Betaflight CLI text](adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)
  - [0016 Render Work Counts](adr/0016-render-work-counts-from-a-counting-wgpu-on-one-lavapipe-thread.md)
  - [0017 Switches with fixed meanings](adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md)
  - [0018 Input through SDL 3.4](adr/0018-input-through-sdl3-on-its-own-thread.md)
  - [0019 The Video Signal through walls](adr/0019-video-signal-averages-walls-over-the-wave-width.md)
  - [0020 The Radio Link's report beat](adr/0020-radio-link-locks-to-the-device-report-beat.md)
  - [0021 Bevy 0.20](adr/0021-alpha-starts-on-bevy-0-20.md)
  - [0022 The OSD beside the Simulation](adr/0022-osd-worked-out-beside-the-simulation.md)
  - [0023 Quad sound on Firewheel](adr/0023-quad-sound-made-live-on-firewheel.md)

# Proving the physics

- [Proving the physics](book/proving-the-physics.md)
- [Reading a Scenario and its Results](verification/reading-a-scenario.md)
- [Checking a Pack](verification/checking-a-pack.md)
- [The Scenario catalogue](book/scenario-catalogue.md)
- [The Feel Test checklist](feel-test-checklist.md)
- [Feel Test logs](book/feel-test-logs.md)

# Research

- [Research](research/README.md)
  - [Bevy as the app shell](research/bevy-app-shell.md)
  - [Input Devices](research/input-devices.md)
  - [Flight dynamics](research/flight-dynamics.md)
  - [Betaflight and our Flight Controller](research/betaflight-flight-controller.md)
  - [Reference flight data](research/reference-flight-data.md)
  - [Determinism](research/determinism.md)
  - [The asset pipeline](research/asset-pipeline.md)
  - [The maintainer's quad settings](research/quad-settings/README.md)
  - [FPV camera dynamic range](research/fpv-camera-dynamic-range-and-resolution.md)
  - [The goggles OSD](research/goggles-osd.md)
  - [Bevy 0.20](research/bevy-0.20.md)
  - [Quad sound](research/quad-sound.md)
  - [The alpha Quads' numbers](research/quad-definitions.md)

# Working on OpenDrone

- [Working on OpenDrone](book/working-on-opendrone.md)
- [Setup and CI](book/contributing.md)
- [Changing a file format](format-migration.md)
- [The agent workflow](book/agents.md)
  - [Issue tracker](agents/issue-tracker.md)
  - [Triage labels](agents/triage-labels.md)
  - [Domain docs](agents/domain.md)
- [The data policy](data-policy.md)

# The code

- [The code (rustdoc)](book/the-code.md)
