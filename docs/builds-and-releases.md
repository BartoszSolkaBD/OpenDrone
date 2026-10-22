# Builds and releases

How to get a build of OpenDrone you can double-click, how a release is made, and how updates to Rust, Bevy and the libraries arrive. The rules behind them are in the [Development deep dive](context/development.md) and [ADR-0021](adr/0021-alpha-starts-on-bevy-0-20.md).

## Getting a build

Every build is a zip: `OpenDrone-<name>-macos.zip`, `-windows.zip` or `-linux.zip`. Unzip it anywhere and keep its folder together.

- **The newest main.** Every push to main builds the game for all three operating systems. On GitHub, open **Actions**, then **Build**, then the newest run on main: the three zips are under **Artifacts**, named `main-<commit>`. They're kept for 30 days.
- **A pull request, to Feel Test it before it merges.** Add the `build` label to the PR. The Build workflow then builds it, and builds it again on every push while the label stays. The zips are under that run's **Artifacts**, from the PR's **Checks** tab, named `pr<number>-<commit>`. The Review Report's Downloads section says the same.
- **A release.** Each release on the repo's **Releases** page carries the three zips, named after its version.

The builds are made on GitHub's machines: macOS on Apple Silicon, Windows and Linux on 64-bit Intel or AMD. They're optimised, as `cargo run --release` builds the game.

## Opening a build

The builds aren't signed, so each system warns about them the first time. Signing and notarisation come later.

- **macOS.** Double-click `opendrone`. It opens in a Terminal window, which shows what the game says while it runs. The first time, macOS refuses it, because it isn't signed: open **System Settings**, then **Privacy & Security**, and click **Open Anyway** beside opendrone's name. Then double-click it again.
- **Windows.** Double-click `opendrone.exe`. The first time, SmartScreen may say it protected your PC: click **More info**, then **Run anyway**.
- **Linux.** Run `./opendrone` in the folder, or double-click it if your file manager runs programs. It needs the system's sound library (alsa-lib) and a Vulkan driver, which most desktops have.

What's inside, beside the game: the built-in Pack (`packs/`), the Scenarios (`scenarios/`), the Maps' textures and the material library (`assets-src/`), the licences, `CREDITS.md` and a short `README.txt`. The game looks for its Packs in the folder it's in first, so it finds them wherever it's started from. A flight saved with Save Flight goes into the build's own `scenarios/saved/`. [Flying OpenDrone](pilot-guide.md) says how to fly what there is.

## Making a release

1. **Run the Frame Check first.** No release is tagged below the 45 fps promise. Until `cargo xtask frame-check` exists, fly the heaviest Map and look.
2. **Tag main with the version, and push the tag.** Before the 0.1.0 alpha, each test release is `0.1.0-alpha.N`, counting from 1: the first is `v0.1.0-alpha.1`. The alpha itself is `v0.1.0`.

   ```sh
   git fetch origin
   git tag v0.1.0-alpha.1 origin/main
   git push origin v0.1.0-alpha.1
   ```

   Or ask an agent to do it.
3. **The Release workflow does the rest,** in about the time a build of main takes. It checks the tag, builds the three zips, and publishes a pre-release named "OpenDrone 0.1.0-alpha.1" with them. Its notes list every PR merged since the last release, by its title. Every release is a pre-release for now.

The Release workflow refuses a tag that isn't on main, and one that isn't a release version: `v0.0.5`, `v0.1.0-rc.1`, `v0.1.0-beta.1` and `v0.1.0-alpha.0` publish nothing (`cargo xtask release-version <tag>` says why). If you make the release on GitHub's Releases page instead, with a new tag, the workflow adds the three zips to it and marks it a pre-release, and keeps the notes you wrote there.

## Updates

**Dependabot** offers updates once a month, each as a PR that goes through the same checks and review as any other ([`.github/dependabot.yml`](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/.github/dependabot.yml)):

- **CI's actions** come in one grouped PR.
- **The libraries outside the core,** such as SDL's bindings, Firewheel and the tools' libraries, come in one grouped PR.
- **Rust itself, `libm`, parry3d** and any other library the five core crates use each come in a PR of their own, so a change to how a flight comes out always has one cause. Rust's PR also moves `ra-ap-rustc_lexer`, the copy of Rust's lexer the Review Report reads code with, which Dependabot leaves alone for that reason.
- **Bevy's and wgpu's patch releases** each come in a PR of their own. Bevy's jumps to a new 0.N, and wgpu's major versions, never come from Dependabot: they never build without code changes.

The ticket asked for CI's actions and the outside libraries in one PR. Dependabot can't do that and still give the core libraries their own PRs: an update that spans two kinds of package (its multi-ecosystem groups) updates nothing outside its group. So they come as two grouped PRs.

A readable check keeps the grouped list honest: every outside library the workspace uses is either in the grouped PR's list, or a core library, Bevy, wgpu or the lexer. A new library added without updating `.github/dependabot.yml` fails the tests.

Some libraries are shared: a grouped PR can also move a library the core uses indirectly, such as `serde`. The Scenarios' fingerprints catch any change that makes to a flight.

**Bevy upgrades.** On the 1st of each month, the Bevy upgrade workflow asks crates.io for Bevy's releases. When a new 0.N.0 has shipped, newer than the Bevy the game is built on, it opens an "Upgrade to Bevy 0.N" ticket, labelled `ready-for-agent`, from a fixed template ([`.github/bevy-upgrade-issue.md`](https://github.com/BartoszSolkaBD/OpenDrone/blob/main/.github/bevy-upgrade-issue.md)). A release candidate never opens one, and neither does a patch release. Agents treat that ticket as your words: the workflow fills in only the version numbers. The ticket says when to start and what must pass, as [ADR-0021](adr/0021-alpha-starts-on-bevy-0-20.md) sets out; its PR waits for your Frame Check and your look at both Maps in both Video Looks.

**Security advisories.** Every day, the Security workflow scans main's libraries against the RustSec advisory database, with the policy in `deny.toml`. For each advisory it finds, it opens one issue, labelled `needs-triage`, unless one is already open: the library, the advisory, what the advisory says to do, and which of our crates reach it. An advisory never blocks unrelated PRs; it blocks only a PR that changes `Cargo.lock` or `deny.toml`. Dealing with one means upgrading the library, or, if that can't be done yet, your decision to accept it for now, written into `deny.toml` with its reason, as the two accepted today are.

You can start either scheduled workflow by hand from GitHub's **Actions** tab, on main (**Run workflow**). Started from any other branch, they do nothing.

## Limits

- The builds aren't signed or notarised, so each system warns the first time, as above.
- The macOS build runs on Apple Silicon only.
- The Linux build needs a system at least as new as the Ubuntu GitHub builds it on.
- The Windows build needs Microsoft's Visual C++ runtime, which most Windows PCs already have.
- A build carries OpenDrone's own licences and `CREDITS.md`, but not yet the licence texts of the libraries built into the game.
