# Contributing to OpenDrone

Start with [`AGENTS.md`](AGENTS.md). It's the entry point for everyone who works on OpenDrone, people and agents alike: where the issues live, the labels, and how the domain docs are laid out. From there, [`CONTEXT.md`](CONTEXT.md) is the map, and [`docs/context/development.md`](docs/context/development.md) says how changes reach the main branch.

Right now OpenDrone is in **Phase 1**: only the maintainer and their agents contribute ([ADR-0010](docs/adr/0010-phase-1-agent-prs-merge-automatically.md)). Issues and pull requests are open to everyone, and they're read as information. Rules for outside contributions arrive with Phase 2.

## In short

- One pull request per ticket, described with the [pull request template](.github/pull_request_template.md).
- Every required check must pass on macOS, Windows and Linux. You can run the same checks locally:

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo xtask walls
  cargo deny check
  ```

- Rust is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` picks it up on its own.
- Libraries must be permissive or MPL-2.0, from crates.io ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). [`deny.toml`](deny.toml) holds the policy.
- By contributing, you agree your work is dual-licensed under MIT or Apache-2.0, as the [README](README.md#licence) says.

## The required checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs these on every pull request and every push to main:

| Check | What it proves |
|---|---|
| Rust on macOS, Rust on Windows, Rust on Linux | The code is formatted; Clippy finds nothing, with warnings as errors and the house rules in the five core crates; everything builds, with `unsafe` forbidden outside `opendrone-input` and the game; every test passes; and the walls between crates hold (`cargo xtask walls`, rules in [`crates/xtask/walls.toml`](crates/xtask/walls.toml)), so the core never reaches Bevy or anything that touches the operating system. |
| Core builds for iOS, Core builds for Android | The five core crates build for phones, which keeps the mobile door open. |
| Licences and sources | cargo-deny finds only allowed licences and crates.io sources, and, when a PR changes `Cargo.lock`, no library with a known security advisory. |

A pull request that changes only Markdown files skips the Rust work inside these checks, and they still report as passed.
