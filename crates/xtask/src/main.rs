//! OpenDrone's developer tools, run with `cargo xtask <command>`: asset
//! generators, texture fetch, Scenario format migration and an input monitor
//! as later tickets add them. Never shipped ([ADR-0003]).
//!
//! Commands so far:
//!
//! - `walls`: checks the walls between crates written in `walls.toml`: who may
//!   use whom, and that the core crates never reach Bevy or anything that
//!   touches the operating system.
//! - `core-maths`: checks the compiled code of every library the core is
//!   built with for calls to the operating system's maths library
//!   ([`core_maths`]).
//! - `book`: builds the docs site from `docs/` with mdBook, with rustdoc for
//!   every crate under `api/`, and checks every link in it ([`book`]).
//! - `book-preprocessor`: the step mdBook runs on every page while it builds
//!   the book ([`book::preprocessor`]).
//! - `check-links`: the book's link check alone, on a book that's built.
//! - `scenario-catalogue`: prints the catalogue of every Scenario, the page the
//!   book shows ([`scenario_catalogue`]).
//! - `migrate [<step>]`: the format migration tool ([`migrate`]).
//! - `packs`: the Pack checker, over every Pack in `packs/` and every Test
//!   Quad in `scenarios/test-quads/`.
//! - `feel-tests --base <revision>`: the Feel Test log rules, comparing every
//!   Quad definition with the one at `<revision>`.
//! - `review-report`, `new-libraries`, `review-update`, `triage-issues` and
//!   `merge-check`: the Review Report, the Red Flag gate and the Review check,
//!   run by CI on every pull request ([`review`]).
//! - `input-monitor`: every connected Input Device live, with its Channels
//!   and their stamps ([`input_monitor`]).
//! - `import-tune`: imports a quad's Betaflight `diff all` as a Quad
//!   definition's `tune.txt` ([`import_tune`]).
//! - `maps`: builds the built-in Pack's Maps with the pinned Blender, or
//!   checks the committed `.glb` files are what it builds ([`maps`]).
//! - `play [--map <id>]`: gets the game running from any checkout, with its
//!   toolchain checked, its textures fetched and its Maps built if they need
//!   it ([`play`]).
//! - `textures`: downloads the CC0 textures the material library lists and
//!   checks each against its checksum ([`textures`]).
//! - `credits`: writes `CREDITS.md` from the material library and the list
//!   of datasets, or checks it is up to date ([`textures`], [`datasets`]).
//! - `generate-textures`: makes the textures that are made from scratch with
//!   the pinned Blender's Python, or checks the committed pictures are what
//!   the generators make ([`generated`]).
//! - `licences`: writes the licence texts of the libraries built into the game
//!   for each operating system, which every download carries, or checks they
//!   are up to date ([`licences`]).
//! - `tii-logs` and `download neurobem`: turn real flight logs into the
//!   motor Input Tracks and recorded flights Physics Scenarios play back
//!   ([`flight_logs`]).
//! - `release-version`, `package`, `bevy-upgrade` and `advisories`: the
//!   parts of the Build, Release, Bevy upgrade and Security workflows: a
//!   release tag's version, a build laid out as a download, and the issues
//!   the two scheduled workflows open ([`release`]).
//! - `icon`: draws the app icon every download carries, or checks the
//!   committed files are what it draws ([`icon`]).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::process::ExitCode;

mod book;
mod core_maths;
mod datasets;
mod flight_logs;
mod generated;
mod icon;
mod import_tune;
mod input_monitor;
mod licences;
mod maps;
mod materials;
mod migrate;
mod packs;
mod play;
mod release;
mod review;
mod scenario_catalogue;
mod textures;
mod walls;

const USAGE: &str = "\
Usage: cargo xtask <command>

Commands:
  walls [--manifest-path <Cargo.toml>]
      Check the walls between crates (ADR-0003): who may use whom, and that
      the core crates never reach Bevy or anything that touches the operating
      system. The rules are in crates/xtask/walls.toml.
  core-maths [--with <package>]... [--target <triple>] [--manifest-path <Cargo.toml>]
      Build the core crates (with the packages named, so their features are
      merged as in the real build; for this computer, or the target named)
      and check the compiled code of every library the core is built with,
      and of the core's functions compiled into other crates and programs:
      none may call the operating system's maths library (ADR-0001), except
      where crates/xtask/walls.toml allows it with a reason.
  book
      Build the docs site from docs/ with mdBook, with rustdoc for every crate
      under api/, and check every link in it. Needs mdBook (the version is in
      book.toml). The book ends up in target/book.
  book-preprocessor [supports <renderer>]
      The step mdBook runs on every page of the book (book.toml names it).
  check-links <folder>
      Check every link in a book that's already built.
  scenario-catalogue [<folder>]
      Print the catalogue of every Scenario in a folder (default scenarios),
      as the book shows it.
  migrate [<step>]
      Run a named format migration step over every Scenario, or every Pack
      file, Test Quad and Test Map, keeping comments and layout. Alone, list
      the steps.
  packs
      Check every Pack in packs/ and every Test Quad in scenarios/test-quads/
      with the Pack checker, listing every problem with its file and line.
  feel-tests --base <git revision>
      Compare every Quad definition with the one at <git revision> (in CI,
      HEAD^1, the pull request's base): an Estimate that moved needs a new
      row in its feel-tests.md and must stay inside its range, and a
      Measured, Manufacturer or Derived number that changed needs a new
      source.
  input-monitor [--seconds <how long>]
      Show every connected Input Device live, read by SDL on its own thread as
      the game reads it: what was found and its profile, its Channels in µs,
      its raw values, and how often they change. Runs until Ctrl+C, or for
      the seconds given.
  maps [--check] [--previews <folder> [--update-preview]]
      Build the built-in Pack's Maps with Blender, the exact version pinned in
      assets-src/blender-version.toml: each assets-src/maps/<id>/build.py
      writes packs/opendrone/maps/<id>/<id>.glb. --check builds into
      target/maps-check/ instead and says whether the committed .glb files are
      those bytes (CI does this on Linux). --previews renders pictures of each
      Map into <folder>, and --update-preview copies its preview.png render
      into the Pack as the Map's picture.
  play [--map <id>] [--no-launch]
      Get the game running from any checkout or worktree, saying what it does
      and what it skips: check the Rust the repo pins and CMake, fetch the
      textures (only what is missing), rebuild a Map's .glb only if you
      edited its script and Blender is installed (else use the committed
      one), then build the game in release mode and start it, on the Map
      named. --no-launch stops before the build. ./play (play.cmd on
      Windows) calls it.
  textures [--pin]
      Download the CC0 textures assets-src/materials.toml lists into
      assets-src/textures/ (git ignores it) and refuse any file whose SHA-256
      differs from the manifest's. --pin writes the checksum of a new file.
      A texture made from scratch (it names a generator) is left alone.
  generate-textures [--check]
      Make the textures assets-src/materials.toml marks as made from scratch
      (a generator, a seed and a size): the script assets-src/generators/
      <generator>.py runs in the pinned Blender's Python and writes the
      texture's diffuse.jpg, normal.jpg and roughness.jpg into
      assets-src/generated/<texture>/, and a contact sheet of them all into
      docs/images/. --check makes them into target/textures-check/ instead and
      says whether the committed pictures are the very same bytes (CI does
      this on Linux). It prints the size each texture costs.
  credits [--check]
      Write CREDITS.md from assets-src/materials.toml and
      assets-src/datasets.toml, with the checksum of every committed data
      file, or with --check say whether it is up to date.
  licences [--check] [--os <macos|windows|linux>]
      Write licences/third-party-<os>.txt for each system (or the one --os
      names): every library built into the game there, with its version,
      its licence and the full text of its licence files, which every
      download carries. With --check, say whether they are up to date with
      Cargo.lock.
  tii-logs <folder>
      Trim TII's Race Against the Machine flights (CC BY 4.0), as their
      downloader unpacks them into <folder>, into the motor Input Tracks and
      recorded flights the Freestyle 5″'s Scenarios in
      scenarios/quads/freestyle-5/tii-logs/ play back, and print each
      Scenario's starting state.
  download neurobem [--zip <processed_data.zip>]
      Download NeuroBEM's processed flights (no licence, so never committed)
      into target/downloads/neurobem/, or read the copy --zip names, and write
      the files its Scenarios in scenarios/quads/freestyle-5/neurobem/ play
      back, beside them, where git ignores them.
  import-tune <export file> (<quad folder> | --print)
      Import a quad's Betaflight diff all (diff bare on 4.3 and 4.4, or a
      dump, from Betaflight 4.3 or newer) as the Quad's tune.txt, in
      Betaflight 2026.6's names with every line marked (ADR-0008, ADR-0015),
      and list what was translated and left out. --print prints the Tune
      instead of writing it.
  release-version <tag>
      Print the version a release tag releases (v0.1.0-alpha.N before the
      0.1.0 alpha, vX.Y.Z from 0.1.0 on), or say why it isn't one.
  package --game <built program> --os <macos|windows|linux> --name <name> [--out <folder>] [--no-textures] [--dmg [--sign <identity>]]
      Lay a built game out as a download in <folder> (default target/dist),
      an app a pilot double-clicks: on macOS OpenDrone.app, in a zip, or with
      --dmg (on a Mac) signed and in OpenDrone-<name>-macos.dmg; on Windows
      OpenDrone.exe in OpenDrone-<name>-windows.zip; on Linux opendrone in
      OpenDrone-<name>-linux.tar.gz, with its icon and add-to-app-menu.sh.
      Each carries the Packs, the Scenarios, the material library and its
      textures where the game looks for them, the licences (the game's own,
      and the libraries' in THIRD-PARTY-LICENSES.txt), the credits and a
      README.txt. --sign takes the Developer ID to sign the app with; it is
      signed ad hoc (-) without one.
  icon [--check]
      Draw the app icon into crates/opendrone/icon/ (opendrone.png for Linux,
      .ico for Windows, .icns for macOS), from shapes, with no outside
      picture. --check fails when a committed file isn't what it draws.
  bevy-upgrade <index file>
      From crates.io's index entry for bevy, print the \"Upgrade to Bevy 0.N\"
      issues to open, as JSON, from .github/bevy-upgrade-issue.md: one for
      each final 0.N.0 newer than Cargo.toml's Bevy, never a release
      candidate (ADR-0021).
  advisories <file>
      From cargo-deny's JSON output (cargo deny --format json check
      advisories), print the issues the daily security scan opens, as JSON:
      one per advisory.";

fn main() -> ExitCode {
    // `core-maths` has cargo run xtask as its compiler wrapper, to keep each
    // program's compiled code.
    if let Some(code) = core_maths::compile_keeping_programs() {
        return code;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}\n{}", review::usage());
        return ExitCode::from(2);
    };
    match command.as_str() {
        "walls" => walls::run(rest),
        "core-maths" => core_maths::run(rest),
        "book" => book::run(rest),
        "book-preprocessor" => book::preprocessor::run(rest),
        "check-links" => book::run_link_check(rest),
        "scenario-catalogue" => scenario_catalogue::run(rest),
        "migrate" => migrate::run(rest),
        "packs" => packs::run_packs(rest),
        "feel-tests" => packs::run_feel_tests(rest),
        "review-report" => review::run_report(rest),
        "review-update" => review::run_update(rest),
        "triage-issues" => review::run_triage_issues(rest),
        "new-libraries" => review::run_new_libraries(rest),
        "merge-check" => review::run_merge_check(rest),
        "input-monitor" => input_monitor::run(rest),
        "import-tune" => import_tune::run(rest),
        "maps" => maps::run(rest),
        "play" => play::run(rest),
        "textures" => textures::run_textures(rest),
        "credits" => textures::run_credits(rest),
        "generate-textures" => generated::run(rest),
        "licences" => licences::run(rest),
        "tii-logs" => flight_logs::run_tii(rest),
        "download" => flight_logs::run_download(rest),
        "icon" => icon::run(rest),
        "release-version" => release::run_release_version(rest),
        "package" => release::run_package(rest),
        "bevy-upgrade" => release::run_bevy_upgrade(rest),
        "advisories" => release::run_advisories(rest),
        _ => {
            eprintln!("{USAGE}\n{}", review::usage());
            ExitCode::from(2)
        }
    }
}
