//! OpenDrone's developer tools, run with `cargo xtask <command>`: asset
//! generators, texture fetch, Scenario format migration and an input monitor
//! as later tickets add them. Never shipped ([ADR-0003]).
//!
//! Commands so far:
//!
//! - `walls`: checks the walls between crates written in `walls.toml`: who may
//!   use whom, and that the core crates never reach Bevy or anything that
//!   touches the operating system.
//! - `book`: builds the docs site from `docs/` with mdBook, with rustdoc for
//!   every crate under `api/`, and checks every link in it ([`book`]).
//! - `book-preprocessor`: the step mdBook runs on every page while it builds
//!   the book ([`book::preprocessor`]).
//! - `check-links`: the book's link check alone, on a book that's built.
//! - `scenario-catalogue`: prints the catalogue of every Scenario, the page the
//!   book shows ([`scenario_catalogue`]).
//! - `packs`: the Pack checker, over every Pack in `packs/` and every Test
//!   Quad in `scenarios/test-quads/`.
//! - `feel-tests --base <revision>`: the Feel Test log rules, comparing every
//!   Quad definition with the one at `<revision>`.
//! - `review-report`, `new-libraries`, `review-update` and `merge-check`: the
//!   Review Report, the Red Flag gate and the Review check, run by CI on every
//!   pull request ([`review`]).
//! - `input-monitor`: every connected Input Device live, with its Channels
//!   and their stamps ([`input_monitor`]).
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::process::ExitCode;

mod book;
mod input_monitor;
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
      the seconds given.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}\n{}", review::usage());
        return ExitCode::from(2);
    };
    match command.as_str() {
        "walls" => walls::run(rest),
        "book" => book::run(rest),
        "book-preprocessor" => book::preprocessor::run(rest),
        "check-links" => book::run_link_check(rest),
        "scenario-catalogue" => scenario_catalogue::run(rest),
        "packs" => packs::run_packs(rest),
        "feel-tests" => packs::run_feel_tests(rest),
        "review-report" => review::run_report(rest),
        "review-update" => review::run_update(rest),
        "new-libraries" => review::run_new_libraries(rest),
        "merge-check" => review::run_merge_check(rest),
        "input-monitor" => input_monitor::run(rest),
        _ => {
            eprintln!("{USAGE}\n{}", review::usage());
            ExitCode::from(2)
        }
    }
}
