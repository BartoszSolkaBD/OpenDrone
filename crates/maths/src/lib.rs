//! Shared 64-bit number types and the house-rule maths ([ADR-0004]).
//!
//! Every other OpenDrone crate except `opendrone-sound` may use it. It uses no
//! other OpenDrone crate.
//!
//! - [`Vec3`]: a direction, position, speed or rotation in three dimensions.
//! - [`Mat3`]: a 3 × 3 matrix, such as a Quad's inertia.
//! - [`Attitude`]: which way a Quad points, kept as a unit quaternion and
//!   turned with the exact exponential map.
//! - [`PilotAngles`] and [`PilotRates`]: the same attitude and rotation in the
//!   words pilots and Scenarios use (roll, pitch, heading; roll, pitch and yaw
//!   rates).
//! - [`functions`]: every maths function the core may call, from `libm`.
//! - [`Fingerprinter`]: a fingerprint of numbers that is the same on every
//!   computer, for checking that two runs or two computers agree.
//! - [`Random`]: a seeded random number generator that gives the same numbers
//!   on every computer, for the Simulation's randomness.
//!
//! # Directions
//!
//! Every OpenDrone crate uses the same axes:
//!
//! - **The world** (a Map): x points east, y north and z up, in metres, as in
//!   Blender, where the Maps are made.
//! - **The Quad's body**: x points forward (out of the nose), y to its left and
//!   z up out of its top, so a level Quad's body z is the world's up.
//! - An [`Attitude`] turns body directions into world directions.
//! - Angles inside the code are radians. Files never use radians: Scenarios and
//!   Packs write degrees and RPM, and `opendrone-pack` converts them.
//!
//! # House rules
//!
//! This is a core crate: part of the Simulation, which gives bit-identical
//! results on every computer ([ADR-0001], [ADR-0003]). So it follows the house
//! rules:
//!
//! - Every maths function comes from `libm`, never from std's float methods
//!   such as `f64::sin` or `f64::powf`, which give different results on
//!   different platforms.
//! - No `HashMap` or `HashSet`: their order changes from run to run. Use
//!   `BTreeMap`, `BTreeSet` or a `Vec`.
//! - No clock, no files, no Bevy and nothing else that touches the operating
//!   system.
//!
//! Clippy enforces the first two (and catches the usual clock and file calls)
//! with this crate's `clippy.toml`, which only the five core crates have.
//! `cargo xtask walls` checks the dependencies.
//!
//! The types here are plain structs of `f64` with no SIMD, so every operation
//! runs the same instructions on ARM and x86. Only `+ - * /` and `sqrt` are
//! used directly, because IEEE 754 makes those exact everywhere; everything
//! else goes through [`functions`].
//!
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md
//! [ADR-0004]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0004-parry3d-geometry-only-f64.md

mod attitude;
mod fingerprint;
pub mod functions;
mod matrix;
mod random;
mod vector;

pub use attitude::{Attitude, PilotAngles, PilotRates};
pub use fingerprint::{Fingerprint, Fingerprinter};
pub use matrix::Mat3;
pub use random::Random;
pub use vector::Vec3;

/// One degree, in radians.
pub const DEGREE: f64 = core::f64::consts::PI / 180.0;
