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
    outcome.says("- `opendrone-maths` calls the operating system's `cos` (from ");
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
    outcome.says("- `glamx` calls the operating system's `acos` (from ");
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
        .member(
            "opendrone",
            &["opendrone-maths", "pictures"],
            "pub fn draw(x: f64) -> f64 { pictures::shade(opendrone_maths::half(x)) }",
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
    with_the_game.says("- `glamx` calls the operating system's `acos` (from ");
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
    outcome.says("- `parry3d-f64` calls the operating system's `acos` (from ");
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
}

struct Package {
    name: String,
    uses: Vec<String>,
    /// The module path the code sits in, such as `shape::trimesh`.
    module: Option<String>,
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
        }
    }

    fn member(mut self, name: &str, uses: &[&str], code: &str) -> Fixture {
        self.members.push(package(name, uses, None, code));
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
    /// workspace's Cargo.toml does for parry3d-f64.
    fn optimise(mut self, name: &str) -> Fixture {
        self.optimised.push(name.to_owned());
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
        let mut manifest = format!(
            "[package]\nname = {:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n",
            package.name
        );
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
        write(&folder.join("lib.rs"), &code);
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
        code: code.to_owned(),
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("can make the folder");
    fs::write(path, contents).expect("can write the fixture file");
}
