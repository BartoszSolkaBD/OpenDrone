//! Makes and plays the Quad's sound, Background Sound and menu sounds on
//! Firewheel, on the sound's own thread ([ADR-0023]).
//!
//! An edge crate ([ADR-0003]): it uses no other OpenDrone crate. The game hands
//! it each frame's state, and nothing flows back into the Simulation.
//!
//! Empty for now: the tickets of the 0.1.0 alpha spec (#37) fill it in.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0023]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0023-quad-sound-made-live-on-firewheel.md
