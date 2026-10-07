//! Every maths function the core may call.
//!
//! They come from the `libm` crate, a pure-Rust port of musl's maths library,
//! so they give the same bits on every computer (ADR-0001). std's versions,
//! such as `f64::sin`, call the operating system's maths library instead, and
//! give different results on Windows, macOS and Linux; Clippy refuses them in
//! the core.
//!
//! The exact operations need nothing from here: `+ - * / %`, `f64::sqrt`,
//! `f64::abs`, `floor`, `ceil`, `round` and `trunc` are the same everywhere.

/// The sine of `x` radians.
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// The cosine of `x` radians.
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// The sine and cosine of `x` radians, worked out together.
pub fn sin_cos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// The angle, in radians from −π to π, whose sine is `y` and cosine is `x`
/// (up to a common scale).
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// The angle, in radians from −π/2 to π/2, whose sine is `x`.
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// What is left of `x` after taking away whole `y`s, with the sign of `x`
/// (C's `fmod`).
pub fn fmod(x: f64, y: f64) -> f64 {
    libm::fmod(x, y)
}
