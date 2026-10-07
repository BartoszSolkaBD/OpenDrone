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
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::process::ExitCode;

mod book;
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
      as the book shows it.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match command.as_str() {
        "walls" => walls::run(rest),
        "book" => book::run(rest),
        "book-preprocessor" => book::preprocessor::run(rest),
        "check-links" => book::run_link_check(rest),
        "scenario-catalogue" => scenario_catalogue::run(rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
