//! OpenDrone's developer tools, run with `cargo xtask <command>`: asset
//! generators, texture fetch, Scenario format migration and an input monitor
//! as later tickets add them. Never shipped ([ADR-0003]).
//!
//! Commands so far:
//!
//! - `walls`: checks the walls between crates written in `walls.toml`: who may
//!   use whom, and that the core crates never reach Bevy or anything that
//!   touches the operating system.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::process::ExitCode;

mod walls;

const USAGE: &str = "\
Usage: cargo xtask <command>

Commands:
  walls [--manifest-path <Cargo.toml>]
      Check the walls between crates (ADR-0003): who may use whom, and that
      the core crates never reach Bevy or anything that touches the operating
      system. The rules are in crates/xtask/walls.toml.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((command, rest)) if command == "walls" => walls::run(rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
