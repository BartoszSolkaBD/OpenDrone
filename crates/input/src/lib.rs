//! Input Devices on their own thread, on SDL 3.4 ([ADR-0018]).
//!
//! An edge crate ([ADR-0003]): it talks to the operating system, so it is one
//! of the two crates that may use `unsafe`. It knows only devices and Channels;
//! the game turns keys, buttons and switches into Actions.
//!
//! Empty for now: the tickets of the 0.1.0 alpha spec (#37) fill it in.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0018]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0018-input-through-sdl3-on-its-own-thread.md
