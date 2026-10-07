//! The game, and the only crate that uses Bevy ([ADR-0021]: Bevy 0.20).
//!
//! It uses every other OpenDrone crate except `xtask`. It turns the computer's
//! clock into Simulation Time and hands the Simulation Flight Inputs. It is one
//! of the two crates that may use `unsafe` ([ADR-0003]).
//!
//! Empty for now: the tickets of the 0.1.0 alpha spec (#37) fill it in. The
//! first game ticket adds Bevy.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0021]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0021-alpha-starts-on-bevy-0-20.md

fn main() {
    println!(
        "OpenDrone {}: nothing to fly yet.",
        env!("CARGO_PKG_VERSION")
    );
}
