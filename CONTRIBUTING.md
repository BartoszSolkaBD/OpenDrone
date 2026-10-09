# Contributing to OpenDrone

Start with [`AGENTS.md`](AGENTS.md). It's the entry point for everyone who works on OpenDrone, people and agents alike: where the issues live, the labels, and how the domain docs are laid out. From there, [`CONTEXT.md`](CONTEXT.md) is the map, and [`docs/context/development.md`](docs/context/development.md) says how changes reach the main branch.

Right now OpenDrone is in **Phase 1**: only the maintainer and their agents contribute ([ADR-0010](docs/adr/0010-phase-1-agent-prs-merge-automatically.md)). Issues and pull requests are open to everyone, and they're read as information. Rules for outside contributions arrive with Phase 2.

## In short

- One pull request per ticket, described with the [pull request template](.github/pull_request_template.md).
- Every required check must pass on macOS, Windows and Linux. You can run the same checks locally. Three of them need tools that don't come with Rust: install them once with `cargo install --locked cargo-nextest cargo-deny` and `cargo install --locked mdbook --version 0.5.4` (or from their prebuilt downloads).

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace     # needs cargo-nextest
  cargo test --workspace --doc      # the examples in the docs, which nextest skips
  cargo xtask walls
  cargo xtask core-maths --with opendrone --with opendrone-scenario   # the core's compiled code calls no OS maths
  cargo xtask packs                 # every Pack and Test Quad, through the Pack checker
  cargo xtask feel-tests --base origin/main   # the Feel Test log rules, against main
  cargo scenarios check             # every Scenario, run twice, and its Results
  cargo deny check                  # needs cargo-deny
  cargo xtask book                  # needs mdBook: the docs site, rustdoc and every link
  ```

- A change that moves a flight updates the Scenarios' Results files: run `cargo scenarios run` and commit them. [Reading a Scenario and its Results](docs/verification/reading-a-scenario.md) explains both.
- A change that adds a starting-state item, or changes how a Pack file is written, adds a format migration step and runs it over every file with `cargo xtask migrate <step>`: see [Changing a file format](docs/format-migration.md).

- Rust is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` picks it up on its own.
- `opendrone-input` builds SDL 3.4 from source ([ADR-0018](docs/adr/0018-input-through-sdl3-on-its-own-thread.md)), so building needs CMake and a C compiler. On Linux, also install `libudev-dev` and `pkg-config`, so SDL notices devices being plugged in and out. `cargo xtask input-monitor` shows your Input Devices live: [Watching Input Devices](docs/verification/watching-input-devices.md).
- The docs in [`docs/`](docs/README.md) also make the docs site, an mdBook ([`book.toml`](book.toml)). Every page under `docs/` must be listed in [`docs/SUMMARY.md`](docs/SUMMARY.md), its table of contents. `cargo xtask book` builds it into `target/book`, and `mdbook serve` shows it while you edit, without rustdoc.
- Libraries must be permissive or MPL-2.0, from crates.io ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). [`deny.toml`](deny.toml) holds the policy.
- By contributing, you agree your work is dual-licensed under MIT or Apache-2.0, as the [README](README.md#licence) says.

## The required checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs these on every pull request and every push to main:

| Check | What it proves |
|---|---|
| Rust on macOS, Rust on Windows, Rust on Linux | The code is formatted; Clippy finds nothing, with warnings as errors and the house rules in the five core crates; everything builds, with `unsafe` forbidden outside `opendrone-input` and the game; every test and every example in the docs passes; and the walls between crates hold (`cargo xtask walls`, rules in [`crates/xtask/walls.toml`](crates/xtask/walls.toml)), so the core never reaches Bevy or anything that touches the operating system, no core crate has a `.clippy.toml` that would turn its house rules off, and every crate is on Rust edition 2024; the compiled code of every library the core is built with, built together with the game and the Scenario runner, and the core's functions compiled into their code, call none of the operating system's maths library, except where `walls.toml` allows a call with its reason (`cargo xtask core-maths --with opendrone --with opendrone-scenario`); every Pack and Test Quad passes the Pack checker (`cargo xtask packs`); a change to a Quad definition keeps the Feel Test log rules, so a moved Estimate has its log row and stays in its range, and a locked number changes only with a new source (`cargo xtask feel-tests --base HEAD^1`; see [Checking a Pack](docs/verification/checking-a-pack.md)); and every Scenario passes every Expectation, gives the same fingerprint after every step when run twice (the repeat check), and has an up-to-date Results file (`cargo scenarios check`). |
| Scenarios agree on every OS | The fingerprint after every step of every Scenario is the same on macOS, Windows and Linux (ADR-0001). On a mismatch it names the Scenario and the first step where the OSes split. |
| Core builds for iOS, Core builds for Android | The five core crates build for phones, which keeps the mobile door open. |
| Licences and sources | cargo-deny finds only allowed licences and crates.io sources, and, when a PR changes `Cargo.lock` or `deny.toml`, no library with a known security advisory. |
| Docs site and rustdoc | The docs site builds from `docs/` with every page in it; rustdoc builds for every crate, internal items included, with its warnings as errors; and every link in the book leads somewhere real: a page, a heading, a file of the repo or a rustdoc page (`cargo xtask book`). The [Pages workflow](.github/workflows/pages.yml) publishes the same build from main. |

A pull request that changes only Markdown files skips the Rust work inside these checks, except the docs site's, and they still report as passed. A file renamed to Markdown counts as a change to its old path too, so it still runs them, and so does any change under `packs/`, because the Pack checker reads each Quad's Feel Test log.

Two more come from the [review workflow](.github/workflows/review.yml), as commit statuses on every pull request into main, docs-only ones included:

| Check | What it proves |
|---|---|
| Red Flag gate | No Red Flag waits for the maintainer: no Source or Rule Expectation changed or removed, no existing ADR edited, and no move to a new Bevy 0.N or wgpu major version. It is worked out by main's code, from the PR's commits read as data, with the Review Report's Red Flags, What moved and Areas touched. Locally: `cargo xtask review-report --base origin/main --head HEAD`. |
| Review check | The newest Verdict covers the PR's latest commit and says pass, the PR was opened by the maintainer's account or by Dependabot from a branch in this repo, and fewer than 5 review rounds have failed. After 5, a `Triaged to #<issue>` comment naming an open issue counts as a pass Verdict on the commit the fifth Verdict reviewed ([The Reviewer and the Verdict](docs/agents/reviewer.md#what-the-review-check-does-with-it)). |

Any workflow with `statuses: write` (or `permissions: write-all`) can set these two statuses, so for a PR that changes CI's workflows they can't be trusted. The Report says so, and the maintainer merges such a PR by hand. How to read the Review Report, and its limits: [`docs/review-report.md`](docs/review-report.md). What the Reviewer checks and the Verdict format: [`docs/agents/reviewer.md`](docs/agents/reviewer.md).
