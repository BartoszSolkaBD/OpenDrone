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
    let either_zero = x.min(0.0) + x.max(0.0);
    let algebraic = x.algebraic_add(1.0)
        + x.algebraic_sub(1.0)
        + x.algebraic_mul(2.0)
        + x.algebraic_div(2.0)
        + x.algebraic_rem(2.0);
    trigonometry
        + inverse
        + hyperbolic
        + exponential
        + logarithm
        + powers
        + angles
        + either_zero
        + algebraic
}

pub fn std_maths_f32(x: f32) -> f32 {
    let trigonometry = x.sin() + x.cos() + x.tan() + x.sin_cos().0;
    let inverse = x.asin() + x.acos() + x.atan() + x.atan2(2.0);
    let hyperbolic = x.sinh() + x.cosh() + x.tanh() + x.asinh() + x.acosh() + x.atanh();
    let exponential = x.exp() + x.exp2() + x.exp_m1();
    let logarithm = x.ln() + x.log(3.0) + x.log2() + x.log10() + x.ln_1p();
    let powers = x.powf(1.5) + x.powi(3) + x.cbrt() + x.hypot(2.0) + x.abs_sub(1.0);
    let angles = x.to_degrees() + x.to_radians();
    let either_zero = x.min(0.0) + x.max(0.0);
    let algebraic = x.algebraic_add(1.0)
        + x.algebraic_sub(1.0)
        + x.algebraic_mul(2.0)
        + x.algebraic_div(2.0)
        + x.algebraic_rem(2.0);
    trigonometry
        + inverse
        + hyperbolic
        + exponential
        + logarithm
        + powers
        + angles
        + either_zero
        + algebraic
}

pub fn runs_parry3ds_operating_system_maths(
    mesh: &mut parry3d_f64::shape::TriMesh,
    tree: &mut parry3d_f64::partitioning::Bvh,
    workspace: &mut parry3d_f64::partitioning::BvhWorkspace,
) {
    use parry3d_f64::shape::{TriMesh, TriMeshFlags};
    let corners = mesh.vertices().to_vec();
    let triangles = mesh.indices().to_vec();
    let _ = mesh.set_flags(TriMeshFlags::ORIENTED);
    let _ = TriMesh::with_flags(corners.clone(), triangles, TriMeshFlags::ORIENTED);
    mesh.update_vertices(|_| {});
    mesh.set_vertices(&corners);
    tree.optimize_incremental(workspace);
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
    let opened = std::fs::OpenOptions::new().read(true).open("settings.toml")?;
    drop(opened);
    let bytes = std::fs::read("settings.toml")?;
    let text = std::fs::read_to_string("settings.toml")?;
    let entries = std::fs::read_dir(".")?.count();
    let size = std::fs::metadata("settings.toml")?.len() as usize;
    let full = std::fs::canonicalize("settings.toml")?.components().count();
    std::fs::write("results.toml", "")?;
    std::fs::create_dir("results")?;
    std::fs::create_dir_all("results")?;
    std::fs::copy("results.toml", "results/copy.toml")?;
    std::fs::rename("results/copy.toml", "results/moved.toml")?;
    std::fs::remove_file("results/moved.toml")?;
    std::fs::remove_dir("results")?;
    std::fs::remove_dir_all("results")?;
    let worker = std::thread::spawn(|| 1);
    let joined = worker.join().unwrap_or(0);
    let built = std::thread::Builder::new().spawn(|| 2)?.join().unwrap_or(0);
    let scoped = std::thread::scope(|scope| scope.spawn(|| 3).join().unwrap_or(0));
    std::thread::sleep(std::time::Duration::from_millis(1));
    let variable = std::env::var("OPENDRONE").map_or(0, |value| value.len());
    let raw = std::env::var_os("OPENDRONE").map_or(0, |value| value.len());
    let all = std::env::vars().count()
        + std::env::vars_os().count()
        + std::env::args().count()
        + std::env::args_os().count();
    let here = std::env::current_dir()?.components().count();
    Ok(bytes.len()
        + text.len()
        + entries
        + size
        + full
        + joined
        + built
        + scoped
        + variable
        + raw
        + all
        + here)
}

pub fn starts_and_stops_programs() -> std::io::Result<u32> {
    let child: std::process::Child = std::process::Command::new("true").spawn()?;
    let me = std::process::id() + child.id();
    if me == 0 {
        std::process::abort();
    }
    std::process::exit(0)
}

pub fn uses_the_network() -> std::io::Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let stream = std::net::TcpStream::connect(listener.local_addr()?)?;
    let socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
    drop((stream, socket));
    Ok(())
}
