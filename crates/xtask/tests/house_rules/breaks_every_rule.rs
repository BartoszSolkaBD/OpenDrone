//! A crate that breaks every house rule in the core crates' `clippy.toml` at
//! least once. The readable checks in `house_rules.rs` run Clippy on it with
//! and without those settings. Each rule added to `clippy.toml` needs a line
//! here too, or those checks fail.

// `abs_sub` is deprecated, and still banned in the core.
#![allow(deprecated)]

use std::collections::{HashMap, HashSet};
use std::time::{Instant, SystemTime};

pub fn std_maths_f64(x: f64) -> f64 {
    let trigonometry = x.sin() + x.cos() + x.tan() + x.sin_cos().0;
    let inverse = x.asin() + x.acos() + x.atan() + x.atan2(2.0);
    let hyperbolic = x.sinh() + x.cosh() + x.tanh() + x.asinh() + x.acosh() + x.atanh();
    let exponential = x.exp() + x.exp2() + x.exp_m1();
    let logarithm = x.ln() + x.log(3.0) + x.log2() + x.log10() + x.ln_1p();
    let powers = x.powf(1.5) + x.powi(3) + x.cbrt() + x.hypot(2.0) + x.abs_sub(1.0);
    let angles = x.to_degrees() + x.to_radians();
    let algebraic = x.algebraic_add(1.0)
        + x.algebraic_sub(1.0)
        + x.algebraic_mul(2.0)
        + x.algebraic_div(2.0)
        + x.algebraic_rem(2.0);
    trigonometry + inverse + hyperbolic + exponential + logarithm + powers + angles + algebraic
}

pub fn std_maths_f32(x: f32) -> f32 {
    let trigonometry = x.sin() + x.cos() + x.tan() + x.sin_cos().0;
    let inverse = x.asin() + x.acos() + x.atan() + x.atan2(2.0);
    let hyperbolic = x.sinh() + x.cosh() + x.tanh() + x.asinh() + x.acosh() + x.atanh();
    let exponential = x.exp() + x.exp2() + x.exp_m1();
    let logarithm = x.ln() + x.log(3.0) + x.log2() + x.log10() + x.ln_1p();
    let powers = x.powf(1.5) + x.powi(3) + x.cbrt() + x.hypot(2.0) + x.abs_sub(1.0);
    let angles = x.to_degrees() + x.to_radians();
    let algebraic = x.algebraic_add(1.0)
        + x.algebraic_sub(1.0)
        + x.algebraic_mul(2.0)
        + x.algebraic_div(2.0)
        + x.algebraic_rem(2.0);
    trigonometry + inverse + hyperbolic + exponential + logarithm + powers + angles + algebraic
}

pub fn order_changes_from_run_to_run() -> usize {
    let map: HashMap<u8, u8> = HashMap::new();
    let set: HashSet<u8> = HashSet::new();
    map.len() + set.len()
}

pub fn reads_the_clock() -> bool {
    let started = Instant::now();
    let wall_clock = SystemTime::now();
    started.elapsed().is_zero() && wall_clock.elapsed().is_ok()
}

pub fn touches_files_threads_and_the_environment() -> std::io::Result<usize> {
    let file = std::fs::File::open("settings.toml")?;
    drop(file);
    let bytes = std::fs::read("settings.toml")?;
    let text = std::fs::read_to_string("settings.toml")?;
    std::fs::write("results.toml", "")?;
    let worker = std::thread::spawn(|| 1);
    let joined = worker.join().unwrap_or(0);
    let variable = std::env::var("OPENDRONE").map_or(0, |value| value.len());
    Ok(bytes.len() + text.len() + joined + variable)
}
