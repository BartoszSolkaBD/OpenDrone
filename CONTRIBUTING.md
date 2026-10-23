# Contributing to OpenDrone

Start with [`AGENTS.md`](AGENTS.md). It's the entry point for everyone who works on OpenDrone, people and agents alike: where the issues live, the labels, and how the domain docs are laid out. From there, [`CONTEXT.md`](CONTEXT.md) is the map, and [`docs/context/development.md`](docs/context/development.md) says how changes reach the main branch.

Right now OpenDrone is in **Phase 1**: only the maintainer and their agents contribute ([ADR-0010](docs/adr/0010-phase-1-agent-prs-merge-automatically.md)). Issues and pull requests are open to everyone, and they're read as information. Rules for outside contributions arrive with Phase 2.

## In short

- One pull request per ticket, described with the [pull request template](.github/pull_request_template.md).
- Every required check must pass. A pull request runs every Rust check on Linux, and on macOS and Windows only the Scenarios, when it can change a flight result; each push to main then runs every check on all three ([The required checks](#the-required-checks)). You can run the same checks locally. Three of them need tools that don't come with Rust: install them once with `cargo install --locked cargo-nextest cargo-deny` and `cargo install --locked mdbook --version 0.5.4` (or from their prebuilt downloads).

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace     # needs cargo-nextest
  cargo test --workspace --doc      # the examples in the docs, which nextest skips
  cargo xtask walls
  cargo xtask core-maths --with opendrone --with opendrone-scenario   # the core's compiled code calls no OS maths
  cargo xtask packs                 # every Pack and Test Quad, through the Pack checker
  cargo xtask credits --check       # CREDITS.md is generated from the material library and up to date
  cargo xtask generate-textures --check   # the textures made from scratch are the very same bytes as the committed ones (needs the pinned Blender, like `cargo xtask maps`)
  cargo xtask licences --check      # licences/third-party-*.txt, the libraries' licence texts every download carries, are up to date
  cargo xtask feel-tests --base origin/main   # the Feel Test log rules, against main
  cargo scenarios check             # every Scenario, run twice, and its Results
  cargo run -p opendrone -- --reference scenarios/flight-controller/a-quad-that-hits-a-wall-stays-armed.toml scenarios/physics/whoop-into-a-6-cm-rail-at-30-m-s.toml scenarios/physics/landed-whoop-stays-still-on-the-skate-parks-launch-spot.toml   # the game flies them as the runner does
  cargo run -p opendrone -- --headless --seconds 3   # the game starts and steps with no window, silent if there's no sound device
  cargo deny check                  # needs cargo-deny
  cargo xtask book                  # needs mdBook: the docs site, rustdoc and every link
  ```

- A change that moves a flight updates the Scenarios' Results files: run `cargo scenarios run` and commit them. [Reading a Scenario and its Results](docs/verification/reading-a-scenario.md) explains both.
- A change that adds a starting-state item, or changes how a Pack file is written, adds a format migration step and runs it over every file with `cargo xtask migrate <step>`: see [Changing a file format](docs/format-migration.md).
- A change to a Map's script, the material library or the pinned Blender version (everything under `assets-src/`) rebuilds the Map's `.glb` with `cargo xtask maps` and commits it: CI rebuilds it with Blender on Linux and requires the same bytes. That needs the Blender version in [`assets-src/blender-version.toml`](assets-src/blender-version.toml) installed, and only for this: [Building a Map](docs/building-a-map.md) says how. `cargo xtask textures` downloads the CC0 textures the game draws Maps with; it isn't part of CI's checks. A Map with baked light also has an atlas, which `cargo xtask maps --bake <id>` makes and CI never rebuilds, only checks is not out of date ([Baked light](docs/building-a-map.md#baked-light)).

- To run the game from a fresh checkout or worktree, run `./play` (`play.cmd` on Windows): it checks the toolchain, fetches the textures, rebuilds a Map you edited if Blender is installed, then builds and starts the game. [Starting the game from any checkout](docs/pilot-guide.md#starting-the-game-from-any-checkout) says what each step does.
- Rust is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` picks it up on its own.
- `opendrone-input` builds SDL 3.4 from source ([ADR-0018](docs/adr/0018-input-through-sdl3-on-its-own-thread.md)), so building needs CMake and a C compiler. On Linux, also install `libudev-dev` and `pkg-config`, so SDL notices devices being plugged in and out, `libwayland-dev`, for the game's window (Bevy 0.20, [ADR-0021](docs/adr/0021-alpha-starts-on-bevy-0-20.md)), and `libasound2-dev`, for the Quad's sound ([ADR-0023](docs/adr/0023-quad-sound-made-live-on-firewheel.md)). `cargo xtask input-monitor` shows your Input Devices live: [Watching Input Devices](docs/verification/watching-input-devices.md).
- The docs in [`docs/`](docs/README.md) also make the docs site, an mdBook ([`book.toml`](book.toml)). Every page under `docs/` must be listed in [`docs/SUMMARY.md`](docs/SUMMARY.md), its table of contents. `cargo xtask book` builds it into `target/book`, and `mdbook serve` shows it while you edit, without rustdoc.
- Libraries must be permissive or MPL-2.0, from crates.io ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). [`deny.toml`](deny.toml) holds the policy.
- By contributing, you agree your work is dual-licensed under MIT or Apache-2.0, as the [README](README.md#licence) says.

## The required checks

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs these on every pull request and every push to main:

| Check | What it proves |
|---|---|
| Rust on macOS, Rust on Windows, Rust on Linux | On a push to main, all three check everything below. On a pull request, Rust on Linux checks everything below, and Rust on macOS and Rust on Windows only fly the Scenarios (`cargo scenarios check`) for the agreement check, when the PR can change a flight result; otherwise they pass with nothing to do. Everything: the code is formatted; Clippy finds nothing, with warnings as errors and the house rules in the five core crates; everything builds, with `unsafe` forbidden outside `opendrone-input` and the game; every test and every example in the docs passes; and the walls between crates hold (`cargo xtask walls`, rules in [`crates/xtask/walls.toml`](crates/xtask/walls.toml)), so the core never reaches Bevy or anything that touches the operating system, no core crate has a `.clippy.toml` that would turn its house rules off, and every crate is on Rust edition 2024; the compiled code of every library the core is built with, built together with the game and the Scenario runner, and the core's functions compiled into their code, call none of the operating system's maths library, except where `walls.toml` allows a call with its reason (`cargo xtask core-maths --with opendrone --with opendrone-scenario`); every Pack and Test Quad passes the Pack checker (`cargo xtask packs`), the Maps and their `.glb` files included; `CREDITS.md` is up to date with the material library (`cargo xtask credits --check`); the libraries' licence texts that every download carries are up to date with `Cargo.lock` (`cargo xtask licences --check`, [the libraries' licences](docs/builds-and-releases.md#the-libraries-licences)); a change to a Quad definition keeps the Feel Test log rules, so a moved Estimate has its log row and stays in its range, and a locked number changes only with a new source (`cargo xtask feel-tests --base HEAD^1`; see [Checking a Pack](docs/verification/checking-a-pack.md)); and every Scenario passes every Expectation, gives the same fingerprint after every step when run twice (the repeat check), and has an up-to-date Results file (`cargo scenarios check`); the game, built with Bevy, flies three reference Scenarios through its own stepping to the same fingerprint as the headless runner (`opendrone --reference`); and the game starts and steps for 3 s with no window, silent if there's no sound device (`opendrone --headless`); the sound's own tests make the Quad's sound with no device (`opendrone-sound`). |
| Scenarios agree on every OS | The fingerprint after every step of every Scenario is the same on macOS, Windows and Linux (ADR-0001). On a mismatch it names the Scenario and the first step where the OSes split. It compares on every push to main, and on a pull request when the PR can change a flight result; otherwise it passes with nothing to do. |
| Core builds for iOS, Core builds for Android | The five core crates build for phones, which keeps the mobile door open. |
| Licences and sources | cargo-deny finds only allowed licences and crates.io sources, and, when a PR changes `Cargo.lock` or `deny.toml`, no library with a known security advisory. |
| Maps | When anything under `assets-src/` or a Pack's `maps/` changes: every Map rebuilt with the pinned Blender on Linux (`cargo xtask maps --check`) gives the very same `.glb` as the committed one. It also renders preview pictures of each Map, kept as the `map-previews` download for the Review Report. A Map built on another computer that differs takes the `.glb` CI built (the `maps` download). [Building a Map](docs/building-a-map.md). |
| Docs site and rustdoc | The docs site builds from `docs/` with every page in it; rustdoc builds for every crate, internal items included, with its warnings as errors; and every link in the book leads somewhere real: a page, a heading, a file of the repo or a rustdoc page (`cargo xtask book`). The [Pages workflow](.github/workflows/pages.yml) publishes the same build from main. |

A pull request that changes only Markdown files skips the Rust work inside these checks, except the docs site's, and they still report as passed. A file renamed to Markdown counts as a change to its old path too, so it still runs them, and so does any change under `packs/`, because the Pack checker reads each Quad's Feel Test log, or under `.github/`, because the tests read the Bevy upgrade ticket's template.

A pull request can change a flight result when it changes, other than in Markdown outside `packs/`: the Scenario runner or a crate it is built with (the core crates, the Pack reader and the Blackbox writer); `scenarios/` or `packs/`; the libraries' versions, the build settings or Rust's version; or how CI flies and compares the Scenarios. Only then do macOS and Windows fly the Scenarios on the pull request. The exact list, with its reasons, is `flight_paths` at the top of [`.github/scripts/changes.sh`](.github/scripts/changes.sh), the one place to edit it. A crate the Scenario runner starts being built with has to be added there: a readable check reads the runner's crates from Cargo and fails until it is.

The cost, which the maintainer chose ([#240](https://github.com/BartoszSolkaBD/OpenDrone/issues/240)): a pull request that can't change a flight result, such as one to the game, the Input crate, SDL's build or the docs, is built and tested on Linux only. Even a pull request that changes a flight crate runs only the Scenarios on macOS and Windows: their own core-maths check (`cargo xtask core-maths`, which catches an OS maths call a library or feature only that OS uses), the core's tests, Clippy and the game's checks run there only on main. A break that shows only on macOS or Windows is then found by main's CI after the merge, not on the pull request. A red main is fixed before any other work ([the Delegator](docs/agents/delegator.md#order-of-work)).

Two more come from the [review workflow](.github/workflows/review.yml), as commit statuses on every pull request into main, docs-only ones included:

| Check | What it proves |
|---|---|
| Red Flag gate | No Red Flag waits for the maintainer: no Source or Rule Expectation changed or removed, no existing ADR edited, and no move to a new Bevy 0.N or wgpu major version. It is worked out by main's code, from the PR's commits read as data, with the Review Report's Red Flags, What moved and Areas touched. Locally: `cargo xtask review-report --base origin/main --head HEAD`. |
| Review check | The newest Verdict covers the PR's latest commit and says pass, the PR was opened by the maintainer's account or by Dependabot from a branch in this repo, and fewer than 5 review rounds have failed. After 5, a `Triaged to #<issue>` comment naming an open issue counts as a pass Verdict on the commit the fifth Verdict reviewed ([The Reviewer and the Verdict](docs/agents/reviewer.md#what-the-review-check-does-with-it)). |

Four more workflows never gate a pull request ([Builds and releases](docs/builds-and-releases.md)): [Build](.github/workflows/build.yml) makes the unsigned game for every push to main and for a PR with the `build` label; [Release](.github/workflows/release.yml) publishes a pre-release for a version tag; and, on a schedule, [Bevy upgrade](.github/workflows/bevy-upgrade.yml) opens an "Upgrade to Bevy 0.N" ticket when a new Bevy ships, and [Security](.github/workflows/security.yml) opens an issue for each security advisory found in main's libraries. [Dependabot](.github/dependabot.yml) offers updates monthly.

Any workflow with `statuses: write` (or `permissions: write-all`) can set these two statuses, so for a PR that changes CI's workflows they can't be trusted. The Report says so, and the maintainer merges such a PR by hand. How to read the Review Report, and its limits: [`docs/review-report.md`](docs/review-report.md). What the Reviewer checks and the Verdict format: [`docs/agents/reviewer.md`](docs/agents/reviewer.md).
