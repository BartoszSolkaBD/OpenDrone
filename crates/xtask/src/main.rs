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
//! - `review-report`, `new-libraries`, `review-update` and `merge-check`: the
//!   Review Report, the Red Flag gate and the Review check, run by CI on every
//!   pull request ([`review`]).
//! - `input-monitor`: every connected Input Device live, with its Channels
//!   and their stamps ([`input_monitor`]).
//! - `import-tune`: imports a quad's Betaflight `diff all` as a Quad
//!   definition's `tune.txt` ([`import_tune`]).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::process::ExitCode;

mod book;
mod core_maths;
mod import_tune;
mod input_monitor;
mod migrate;
mod packs;
mod review;
mod scenario_catalogue;
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
  import-tune <export file> (<quad folder> | --print)
      Import a quad's Betaflight diff all (diff bare on 4.3 and 4.4, or a
      dump, from Betaflight 4.3 or newer) as the Quad's tune.txt, in
      Betaflight 2026.6's names with every line marked (ADR-0008, ADR-0015),
      and list what was translated and left out. --print prints the Tune
      instead of writing it.";

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
        "new-libraries" => review::run_new_libraries(rest),
        "merge-check" => review::run_merge_check(rest),
        "input-monitor" => input_monitor::run(rest),
        "import-tune" => import_tune::run(rest),
        _ => {
            eprintln!("{USAGE}\n{}", review::usage());
            ExitCode::from(2)
        }
    }
}
