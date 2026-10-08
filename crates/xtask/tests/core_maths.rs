//! Readable checks for `cargo xtask core-maths`, the CI check that the code
//! the core runs never calls the operating system's maths library
//! (ADR-0001).
//!
//! Each check writes a small workspace, using the real crate names and the
//! real rules in `walls.toml`, builds it and runs the check on it. Outside
//! libraries are stand-ins, local folders named like the real ones, so
//! nothing is downloaded.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn a_core_crate_calling_the_operating_systems_cos_fails_naming_the_crate_and_the_function() {
    let outcome = Fixture::new("core-calls-cos")
        .member(
            "opendrone-maths",
            &[],
            "pub fn wobble(x: f64) -> f64 { x.cos() }",
        )
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    // The call sits in std's `f64::cos`, copied into the crate, and counts as
    // made by the core's function that calls it.
    outcome.says(
        "- `opendrone-maths` calls the operating system's `cos` (Rust's `f64::cos`) from \
         opendrone_maths::wobble.",
    );
    outcome.says("the core must take every maths function from libm (ADR-0001)");
}

#[test]
fn a_library_the_core_is_built_with_calling_acos_fails_naming_the_library() {
    let outcome = Fixture::new("library-calls-acos")
        .outside("glamx", &[], "pub fn eigen(x: f64) -> f64 { x.acos() }")
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull(x: f64) -> f64 { glamx::eigen(x) }",
        )
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `glamx` calls the operating system's `acos` (Rust's `f64::acos`) from ");
}

#[test]
fn a_library_only_the_game_uses_isnt_part_of_the_core_and_isnt_checked() {
    let outcome = Fixture::new("game-calls-cos")
        .outside("pictures", &[], "pub fn shade(x: f64) -> f64 { x.cos() }")
        .member(
            "opendrone-maths",
            &[],
            "pub fn half(x: f64) -> f64 { x / 2.0 }",
        )
        .program(
            "opendrone",
            &["opendrone-maths", "pictures"],
            "fn main() { println!(\"{}\", pictures::shade(opendrone_maths::half(1.0))); }",
        )
        .check_with(&["opendrone"]);
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says("built together with opendrone");
}

#[test]
fn a_feature_the_game_turns_on_in_a_core_library_counts_when_built_together() {
    // As Bevy might turn on `std` in a library glamx uses: the core alone is
    // clean, but the game's build merges the feature into the core's library.
    let glamx = "#[cfg(feature = \"std\")]\npub fn eigen(x: f64) -> f64 { x.acos() }\n\
                 #[cfg(not(feature = \"std\"))]\npub fn eigen(x: f64) -> f64 { x * 0.5 }";
    let fixture = |case| {
        Fixture::new(case)
            .outside("glamx", &[], glamx)
            .features("glamx", &["std"])
            .member("opendrone-maths", &[], "")
            .member(
                "opendrone-physics",
                &["opendrone-maths", "glamx"],
                "pub fn hull(x: f64) -> f64 { glamx::eigen(x) }",
            )
            .member("opendrone", &["opendrone-physics", "glamx/std"], "")
    };
    let alone = fixture("core-alone").check();
    assert!(alone.passed, "{}", alone.output);
    let with_the_game = fixture("core-with-the-game").check_with(&["opendrone"]);
    assert!(!with_the_game.passed, "{}", with_the_game.output);
    with_the_game.says("- `glamx` calls the operating system's `acos` (Rust's `f64::acos`) from ");
}

#[test]
fn a_core_crate_that_only_adds_multiplies_and_takes_square_roots_passes() {
    let outcome = Fixture::new("core-exact-maths")
        .member(
            "opendrone-maths",
            &[],
            "pub fn length(x: f64, y: f64) -> f64 { (x * x + y * y).sqrt() + x.abs() }",
        )
        .check();
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says("The core calls none of the operating system's maths: 1 libraries checked.");
}

#[test]
fn a_core_crate_using_remainder_and_mul_add_passes_on_every_os() {
    // The house rules allow `%` and `mul_add`: IEEE 754 fixes every bit of
    // their results. They compile to the operating system's `fmod` and (on
    // x86 without FMA instructions) `fma`, which the check lets through too.
    let outcome = Fixture::new("core-remainder-and-mul-add")
        .member(
            "opendrone-maths",
            &[],
            "pub fn wrap(x: f64, y: f64) -> f64 { x % y + x.mul_add(y, 1.0) }\n\
             pub fn wrap_f32(x: f32, y: f32) -> f32 { x % y + x.mul_add(y, 1.0) }",
        )
        .check();
    assert!(outcome.passed, "{}", outcome.output);
}

#[test]
fn a_library_calling_abs_sub_fails_naming_the_rust_operation() {
    // `fdim` is exact too, but its Rust operation, the old `abs_sub`, is
    // banned in the core crates, so the check bans it as well.
    let outcome = Fixture::new("library-calls-abs-sub")
        .outside(
            "glamx",
            &[],
            "#[allow(deprecated)]\npub fn gap(x: f64) -> f64 { x.abs_sub(1.0) }",
        )
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull(x: f64) -> f64 { glamx::gap(x) }",
        )
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `glamx` calls the operating system's `fdim` (Rust's `f64::abs_sub`) from ");
}

#[test]
fn powf_with_a_base_of_10_fails_on_every_os_and_as_apples_exp10_on_macos() {
    // Optimised, the compiler turns `10.powf(x)` into Apple's own `__exp10`
    // on macOS, and keeps `pow` elsewhere. `inline(never)` keeps the function
    // in glamx's own optimised code: a small function of an optimised library
    // may otherwise be compiled into its caller's crate.
    let outcome = Fixture::new("library-powers-of-ten")
        .outside(
            "glamx",
            &[],
            "#[inline(never)]\npub fn decibels(x: f64) -> f64 { 10f64.powf(x) }",
        )
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn loudness(x: f64) -> f64 { glamx::decibels(x) }",
        )
        .optimise("glamx")
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `glamx` calls the operating system's `");
    if cfg!(target_vendor = "apple") {
        outcome.says(
            "`__exp10` (Rust's `f64::powf` with a base of 10, as the compiler writes it for \
             Apple's systems)",
        );
    }
}

#[test]
fn a_call_to_any_of_apples_own_maths_functions_fails() {
    // A library can call them directly. The compiled code isn't linked, so
    // this works on every OS.
    let glamx = "#[repr(C)] pub struct Both { sin: f64, cos: f64 }\n\
                 #[repr(C)] pub struct BothF32 { sin: f32, cos: f32 }\n\
                 unsafe extern \"C\" {\n\
                     safe fn __exp10(x: f64) -> f64;\n\
                     safe fn __exp10f(x: f32) -> f32;\n\
                     safe fn __sinpi(x: f64) -> f64;\n\
                     safe fn __sinpif(x: f32) -> f32;\n\
                     safe fn __cospi(x: f64) -> f64;\n\
                     safe fn __cospif(x: f32) -> f32;\n\
                     safe fn __tanpi(x: f64) -> f64;\n\
                     safe fn __tanpif(x: f32) -> f32;\n\
                     safe fn __sincospi_stret(x: f64) -> Both;\n\
                     safe fn __sincospif_stret(x: f32) -> BothF32;\n\
                 }\n\
                 pub fn every_one(x: f64) -> f64 {\n\
                     let y = x as f32;\n\
                     let both = __sincospi_stret(x);\n\
                     let both_f32 = __sincospif_stret(y);\n\
                     __exp10(x) + __sinpi(x) + __cospi(x) + __tanpi(x) + both.sin + both.cos\n\
                         + f64::from(__exp10f(y) + __sinpif(y) + __cospif(y) + __tanpif(y))\n\
                         + f64::from(both_f32.sin + both_f32.cos)\n\
                 }";
    let outcome = Fixture::new("library-calls-apples-maths")
        .outside("glamx", &[], glamx)
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull(x: f64) -> f64 { glamx::every_one(x) }",
        )
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    for name in [
        "__exp10",
        "__exp10f",
        "__sinpi",
        "__sinpif",
        "__cospi",
        "__cospif",
        "__tanpi",
        "__tanpif",
        "__sincospi_stret",
        "__sincospif_stret",
    ] {
        outcome.says(&format!(
            "- `glamx` calls the operating system's `{name}` ("
        ));
    }
}

#[test]
fn an_allowed_call_made_from_a_function_the_allowance_doesnt_name_fails() {
    // walls.toml lets parry3d-f64 call `acos` only from TriMesh's
    // pseudo-normals. A stand-in parry3d-f64, built optimised as the real one
    // is, that calls it from somewhere else too must fail.
    let parry = "pub struct TriMesh;\n\
                 impl TriMesh {\n\
                     #[inline(never)] pub fn compute_pseudo_normals(&self, x: f64) -> f64 { x.acos() }\n\
                     #[inline(never)] pub fn something_new(&self, x: f64) -> f64 { x.acos() * 2.0 }\n\
                 }";
    let outcome = Fixture::new("allowed-call-from-elsewhere")
        .outside_module("parry3d-f64", "shape::trimesh", parry)
        .member("opendrone-maths", &[], "")
        .member("opendrone-physics", &["opendrone-maths", "parry3d-f64"], "")
        .optimise("parry3d-f64")
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `parry3d-f64` calls the operating system's `acos` (Rust's `f64::acos`) from ");
    outcome.says("something_new");
}

#[test]
fn an_allowed_call_made_only_from_the_function_the_allowance_names_passes() {
    let parry = "pub struct TriMesh;\n\
                 impl TriMesh {\n\
                     #[inline(never)] pub fn compute_pseudo_normals(&self, x: f64) -> f64 { x.acos() }\n\
                 }";
    let outcome = Fixture::new("allowed-call")
        .outside_module("parry3d-f64", "shape::trimesh", parry)
        .member("opendrone-maths", &[], "")
        .member("opendrone-physics", &["opendrone-maths", "parry3d-f64"], "")
        .optimise("parry3d-f64")
        .check();
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says(
        "- Allowed by crates/xtask/walls.toml: `parry3d-f64` calls `acos` (from \
         <parry3d_f64::shape::trimesh::TriMesh>::compute_pseudo_normals)",
    );
}

#[test]
fn a_table_pointing_at_acos_outside_any_function_fails_even_beside_an_allowed_call() {
    // The same library's allowed call must not cover a reference from data:
    // nothing says what calls through a table of function pointers.
    let parry = "unsafe extern \"C\" { fn acos(x: f64) -> f64; }\n\
                 pub static ANGLES: [unsafe extern \"C\" fn(f64) -> f64; 1] = [acos];\n\
                 pub struct TriMesh;\n\
                 impl TriMesh {\n\
                     #[inline(never)] pub fn compute_pseudo_normals(&self, x: f64) -> f64 { x.acos() }\n\
                 }";
    let outcome = Fixture::new("table-of-acos")
        .outside_module("parry3d-f64", "shape::trimesh", parry)
        .member("opendrone-maths", &[], "")
        .member("opendrone-physics", &["opendrone-maths", "parry3d-f64"], "")
        .optimise("parry3d-f64")
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `parry3d-f64` calls the operating system's `acos` (Rust's `f64::acos`) from ");
    outcome.says("`parry3d_f64::shape::trimesh::ANGLES`, data outside any function (in section `");
    outcome.says(
        "No allowance can cover a reference from outside any function, such as a table of \
         function pointers",
    );
}

#[test]
fn a_generic_core_function_calling_acos_compiled_into_the_games_program_fails() {
    // A generic function is compiled into the crate that uses it: here the
    // physics' and glamx's, into the game's program. The core's own compiled
    // code holds no call, so only reading the game's finds it.
    let fixture = |case| {
        Fixture::new(case)
            .outside(
                "glamx",
                &[],
                "pub fn eigen<T: Into<f64>>(x: T) -> f64 { x.into().acos() }",
            )
            .member("opendrone-maths", &[], "")
            .member(
                "opendrone-physics",
                &["opendrone-maths", "glamx"],
                "pub fn hull<T: Into<f64>>(x: T) -> f64 { glamx::eigen(x) }",
            )
            .optimise("opendrone-physics")
            .program(
                "opendrone",
                &["opendrone-physics"],
                "fn main() { println!(\"{}\", opendrone_physics::hull(0.5_f32)); }",
            )
    };
    let alone = fixture("generic-core-alone").check();
    assert!(alone.passed, "{}", alone.output);
    let with_the_game = fixture("generic-core-in-the-game").check_with(&["opendrone"]);
    assert!(!with_the_game.passed, "{}", with_the_game.output);
    // Named from glamx's `eigen::<f32>`, or from the physics' `hull::<f32>`
    // when the optimised physics has taken `eigen` into it.
    with_the_game.says("calls the operating system's `acos` (Rust's `f64::acos`) from ");
    with_the_game.says("::<f32> (compiled into the program `opendrone`).");
}

#[test]
fn an_inline_core_function_calling_acos_compiled_into_the_scenario_runner_fails() {
    // An `#[inline]` function is compiled into each crate that uses it, as
    // here into the Scenario runner's library.
    let outcome = Fixture::new("inline-core-in-the-runner")
        .outside(
            "glamx",
            &[],
            "#[inline]\npub fn eigen(x: f64) -> f64 { x.acos() }",
        )
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "#[inline]\npub fn hull(x: f64) -> f64 { glamx::eigen(x) }",
        )
        .optimise("opendrone-physics")
        .member(
            "opendrone-scenario",
            &["opendrone-physics"],
            "pub fn run(x: f64) -> f64 { opendrone_physics::hull(x) }",
        )
        .check_with(&["opendrone-scenario"]);
    assert!(!outcome.passed, "{}", outcome.output);
    // Named from glamx's `eigen`, or from the physics' `hull` when the
    // optimised physics has taken `eigen` into it.
    outcome.says("calls the operating system's `acos` (Rust's `f64::acos`) from ");
    outcome.says("(compiled into `opendrone-scenario`)");
}

#[test]
fn a_generic_copy_shared_from_a_crate_that_uses_only_a_core_library_is_read_too() {
    // Crates built without optimisation share their generic copies: here
    // `pictures` builds `glamx::eigen::<f32>`, and the game's copy of the
    // physics' `hull::<f32>` calls that copy instead of building its own. The
    // call to `acos` sits only in `pictures`, which uses glamx but no core
    // crate.
    let outcome = Fixture::new("shared-generic-copy")
        .outside(
            "glamx",
            &[],
            "pub fn eigen<T: Into<f64>>(x: T) -> f64 { x.into().acos() }",
        )
        .outside(
            "pictures",
            &["glamx"],
            "pub fn shade(x: f32) -> f64 { glamx::eigen(x) }",
        )
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull<T: Into<f64>>(x: T) -> f64 { glamx::eigen(x) }",
        )
        .optimise("opendrone-physics")
        .program(
            "opendrone",
            &["opendrone-physics", "pictures"],
            "fn main() { println!(\"{} {}\", opendrone_physics::hull(0.5_f32), \
             pictures::shade(0.25)); }",
        )
        .check_with(&["opendrone"]);
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("- `glamx` calls the operating system's `acos` (Rust's `f64::acos`) from ");
}

#[test]
fn a_program_is_checked_with_the_features_every_package_turns_on_together() {
    // The game turns on glamx's `std`, which makes its generic `eigen` call
    // `acos`. The Scenario runner's program, built alone, would get glamx
    // without it; built together with the game, as the check builds it, its
    // copy of `eigen` calls `acos`.
    let glamx = "#[cfg(feature = \"std\")]\n\
                 pub fn eigen<T: Into<f64>>(x: T) -> f64 { x.into().acos() }\n\
                 #[cfg(not(feature = \"std\"))]\n\
                 pub fn eigen<T: Into<f64>>(x: T) -> f64 { x.into() * 0.5 }";
    let outcome = Fixture::new("program-with-merged-features")
        .outside("glamx", &[], glamx)
        .features("glamx", &["std"])
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull<T: Into<f64>>(x: T) -> f64 { glamx::eigen(x) }",
        )
        .member("opendrone", &["opendrone-physics", "glamx/std"], "")
        .program(
            "opendrone-scenario",
            &["opendrone-physics"],
            "fn main() { println!(\"{}\", opendrone_physics::hull(0.5_f32)); }",
        )
        .check_with(&["opendrone", "opendrone-scenario"]);
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("calls the operating system's `acos` (Rust's `f64::acos`) from ");
    outcome.says("(compiled into the program `opendrone-scenario`)");
}

#[test]
fn the_summary_counts_the_crates_read_and_those_holding_core_functions_apart() {
    // The game calls a generic function of the physics, so its program holds
    // a core function; the Scenario runner's library calls none.
    let outcome = Fixture::new("summary-counts")
        .member(
            "opendrone-maths",
            &[],
            "pub fn twice<T: Copy + std::ops::Add<Output = T>>(x: T) -> T { x + x }",
        )
        .member(
            "opendrone-scenario",
            &["opendrone-maths"],
            "pub fn name() -> &'static str { \"runner\" }",
        )
        .program(
            "opendrone",
            &["opendrone-maths", "opendrone-scenario"],
            "fn main() { println!(\"{} {}\", opendrone_maths::twice(2.0_f64), \
             opendrone_scenario::name()); }",
        )
        .check_with(&["opendrone"]);
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says(
        "1 libraries checked, and 2 other crates' compiled code read for the core's functions \
         (found in 1)",
    );
}

#[test]
fn link_time_optimisation_fails_the_check_instead_of_hiding_the_calls() {
    // With link-time optimisation, the libraries hold LLVM bitcode instead of
    // machine code, and their calls can't be read.
    let outcome = Fixture::new("link-time-optimisation")
        .outside("glamx", &[], "pub fn eigen(x: f64) -> f64 { x.acos() }")
        .member("opendrone-maths", &[], "")
        .member(
            "opendrone-physics",
            &["opendrone-maths", "glamx"],
            "pub fn hull(x: f64) -> f64 { glamx::eigen(x) }",
        )
        .profile("[profile.dev]\nlto = \"fat\"\n")
        .check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "- `glamx` was compiled to LLVM bitcode, not machine code. Link-time optimisation does \
         that",
    );
}

#[test]
fn ci_runs_the_check_on_macos_windows_and_linux_with_the_game_and_the_runner() {
    // Each OS checks its own build: a library or feature only Windows uses
    // shows only in the Windows build.
    let ci = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml"),
    )
    .expect("can read ci.yml")
    .replace("\r\n", "\n");
    let rust_job = ci
        .split_once("\n  rust:\n")
        .and_then(|(_, rest)| rest.split("\n  agreement:\n").next())
        .expect("ci.yml has the Rust job");
    for os in ["macos-latest", "windows-latest", "ubuntu-latest"] {
        assert!(
            rust_job.contains(&format!("os: {os}")),
            "the Rust job doesn't run on {os}"
        );
    }
    let step = rust_job
        .split("\n      - name: ")
        .find(|step| {
            step.starts_with("The core's compiled code calls none of the operating system's maths")
        })
        .expect("the Rust job has the core-maths step");
    let condition = step
        .lines()
        .find_map(|line| line.trim().strip_prefix("if: "))
        .unwrap_or_default();
    assert!(
        !condition.contains("runner.os"),
        "the core-maths step must run on every OS, not only where `{condition}`"
    );
    assert!(
        step.contains("run: cargo xtask core-maths --with opendrone --with opendrone-scenario"),
        "the core-maths step must build the core with the game and the Scenario runner:\n{step}"
    );
}

struct Outcome {
    passed: bool,
    output: String,
}

impl Outcome {
    fn says(&self, text: &str) {
        assert!(
            self.output.contains(text),
            "expected {text:?} in:\n{}",
            self.output
        );
    }
}

/// A small workspace written to a scratch folder: OpenDrone crates as members,
/// and stand-in outside libraries in a folder beside it, each with its code.
struct Fixture {
    root: PathBuf,
    members: Vec<Package>,
    outside: Vec<Package>,
    optimised: Vec<String>,
    /// Features an outside library declares.
    features: Vec<(String, Vec<String>)>,
    /// More of the workspace's Cargo.toml, such as a `[profile.dev]` table.
    profile: String,
}

struct Package {
    name: String,
    uses: Vec<String>,
    /// The module path the code sits in, such as `shape::trimesh`.
    module: Option<String>,
    /// A program (`main.rs`) rather than a library.
    program: bool,
    code: String,
}

impl Fixture {
    fn new(case: &str) -> Fixture {
        Fixture {
            root: Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("core-maths")
                .join(case),
            members: Vec::new(),
            outside: Vec::new(),
            optimised: Vec::new(),
            features: Vec::new(),
            profile: String::new(),
        }
    }

    fn member(mut self, name: &str, uses: &[&str], code: &str) -> Fixture {
        self.members.push(package(name, uses, None, code));
        self
    }

    /// A member that is a program, as the game and the Scenario runner are.
    fn program(mut self, name: &str, uses: &[&str], code: &str) -> Fixture {
        let mut program = package(name, uses, None, code);
        program.program = true;
        self.members.push(program);
        self
    }

    fn outside(mut self, name: &str, uses: &[&str], code: &str) -> Fixture {
        self.outside.push(package(name, uses, None, code));
        self
    }

    /// An outside library whose code sits in nested modules, so its
    /// functions have the real library's paths.
    fn outside_module(mut self, name: &str, module: &str, code: &str) -> Fixture {
        self.outside.push(package(name, &[], Some(module), code));
        self
    }

    /// Features an outside library declares, each turning on nothing else.
    /// A dependency written `name/feature` turns one on.
    fn features(mut self, name: &str, features: &[&str]) -> Fixture {
        self.features.push((
            name.to_owned(),
            features.iter().map(|name| (*name).to_owned()).collect(),
        ));
        self
    }

    /// Builds this package optimised in development builds, as the
    /// workspace's Cargo.toml does for parry3d-f64 and the physics.
    fn optimise(mut self, name: &str) -> Fixture {
        self.optimised.push(name.to_owned());
        self
    }

    /// Adds to the workspace's Cargo.toml.
    fn profile(mut self, text: &str) -> Fixture {
        self.profile.push_str(text);
        self
    }

    fn check(&self) -> Outcome {
        self.check_with(&[])
    }

    fn check_with(&self, with: &[&str]) -> Outcome {
        match fs::remove_dir_all(&self.root) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                panic!("can't clear {}: {error}", self.root.display())
            }
            _ => {}
        }
        let workspace = self.root.join("workspace");
        let members: Vec<String> = self
            .members
            .iter()
            .map(|member| format!("{:?}", member.name))
            .collect();
        let mut manifest = format!(
            "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
            members.join(", ")
        );
        for name in &self.optimised {
            manifest.push_str(&format!("\n[profile.dev.package.{name}]\nopt-level = 3\n"));
        }
        manifest.push('\n');
        manifest.push_str(&self.profile);
        write(&workspace.join("Cargo.toml"), &manifest);
        for member in &self.members {
            self.write_package(&workspace.join(&member.name), member);
        }
        for library in &self.outside {
            self.write_package(&self.root.join("outside").join(&library.name), library);
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command.arg("core-maths");
        for package in with {
            command.args(["--with", package]);
        }
        command
            .arg("--manifest-path")
            .arg(workspace.join("Cargo.toml"))
            .env("CARGO_NET_OFFLINE", "true");
        let output = command.output().expect("the xtask program runs");
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        match output.status.code() {
            Some(0) => Outcome {
                passed: true,
                output: text,
            },
            Some(1) => Outcome {
                passed: false,
                output: text,
            },
            _ => panic!("the core-maths check couldn't run:\n{text}"),
        }
    }

    fn write_package(&self, folder: &Path, package: &Package) {
        let (kind, file) = if package.program {
            ("[[bin]]", "main.rs")
        } else {
            ("[lib]", "lib.rs")
        };
        let mut manifest = format!(
            "[package]\nname = {:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             publish = false\n\n{kind}\n",
            package.name
        );
        if package.program {
            manifest.push_str(&format!("name = {:?}\n", package.name));
        }
        manifest.push_str(&format!("path = {file:?}\n\n[dependencies]\n"));
        for dependency in &package.uses {
            let (dependency, features) = match dependency.split_once('/') {
                Some((name, feature)) => (name, format!(", features = [{feature:?}]")),
                None => (dependency.as_str(), String::new()),
            };
            manifest.push_str(&format!(
                "{dependency} = {{ path = {:?}{features} }}\n",
                self.path_to(dependency)
            ));
        }
        manifest.push_str("\n[features]\n");
        for (_, features) in self
            .features
            .iter()
            .filter(|(owner, _)| *owner == package.name)
        {
            for feature in features {
                manifest.push_str(&format!("{feature} = []\n"));
            }
        }
        write(&folder.join("Cargo.toml"), &manifest);
        let code = match &package.module {
            None => package.code.clone(),
            Some(module) => {
                let parts: Vec<&str> = module.split("::").collect();
                let mut code = package.code.clone();
                for part in parts.iter().rev() {
                    code = format!("pub mod {part} {{\n{code}\n}}");
                }
                code
            }
        };
        write(&folder.join(file), &code);
    }

    fn path_to(&self, dependency: &str) -> String {
        if self
            .outside
            .iter()
            .any(|library| library.name == dependency)
        {
            format!("../../outside/{dependency}")
        } else {
            format!("../../workspace/{dependency}")
        }
    }
}

fn package(name: &str, uses: &[&str], module: Option<&str>, code: &str) -> Package {
    Package {
        name: name.to_owned(),
        uses: uses.iter().map(|name| (*name).to_owned()).collect(),
        module: module.map(str::to_owned),
        program: false,
        code: code.to_owned(),
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("can make the folder");
    fs::write(path, contents).expect("can write the fixture file");
}
