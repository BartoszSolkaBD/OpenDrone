//! Every maths function the core may call.
//!
//! They come from the `libm` crate, a pure-Rust port of musl's maths library,
//! so they give the same bits on every computer (ADR-0001). std's versions,
//! such as `f64::sin`, call the operating system's maths library instead, and
//! give different results on Windows, macOS and Linux; Clippy refuses them in
//! the core.
//!
//! The exact operations need nothing from here: `+ - * / %`, `f64::sqrt`,
//! `f64::abs`, `floor`, `ceil`, `round`, `trunc` and `clamp` are the same
//! everywhere. std's `f64::min` and `f64::max` are not: given +0 and -0 they
//! may return either, and ARM and x86 pick differently, so use [`min`] and
//! [`max`] here instead.

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

/// The smaller of `a` and `b`, always picking the same one: when they are
/// equal, such as +0 and -0, it is `a`, and when either is "not a number", so
/// is the answer.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if b < a {
        b
    } else {
        a
    }
}

/// The larger of `a` and `b`, always picking the same one: when they are
/// equal, such as +0 and -0, it is `a`, and when either is "not a number", so
/// is the answer.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if b > a {
        b
    } else {
        a
    }
}
