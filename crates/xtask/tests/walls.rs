//! Readable checks for `cargo xtask walls`, the CI check that keeps the walls
//! between crates from ADR-0003.
//!
//! Each check but the first writes a small workspace with deliberately broken
//! manifests, using the real crate names and the real rules in `walls.toml`,
//! and runs the check on it. Outside libraries are stand-ins, local folders
//! named like the real ones, so nothing is downloaded.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn the_real_workspace_passes_the_walls_check() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let outcome = walls(&manifest, false);
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says("The walls hold: all 12 crates");
}

#[test]
fn physics_using_the_flight_controller_breaks_the_walls() {
    let outcome = Fixture::new("physics-uses-the-flight-controller")
        .member("opendrone-maths", &[])
        .member("opendrone-flight-controller", &["opendrone-maths"])
        .member(
            "opendrone-physics",
            &["opendrone-maths", "opendrone-flight-controller"],
        )
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "`opendrone-physics` uses `opendrone-flight-controller`, but it may use only \
         `opendrone-maths`: physics and the Flight Controller each use only maths, and never \
         each other (ADR-0003).",
    );
}

#[test]
fn the_flight_controller_using_physics_breaks_the_walls() {
    let outcome = Fixture::new("flight-controller-uses-physics")
        .member("opendrone-maths", &[])
        .member("opendrone-physics", &["opendrone-maths"])
        .member(
            "opendrone-flight-controller",
            &["opendrone-maths", "opendrone-physics"],
        )
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("`opendrone-flight-controller` uses `opendrone-physics`");
}

#[test]
fn physics_using_the_flight_controller_only_in_its_tests_still_breaks_the_walls() {
    let outcome = Fixture::new("physics-tests-use-the-flight-controller")
        .member("opendrone-maths", &[])
        .member("opendrone-flight-controller", &["opendrone-maths"])
        .member("opendrone-physics", &["opendrone-maths"])
        .test_only("opendrone-physics", "opendrone-flight-controller")
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("`opendrone-physics` uses `opendrone-flight-controller`");
}

#[test]
fn a_core_crate_reaching_bevy_through_another_library_breaks_the_walls_and_names_the_chain() {
    let outcome = Fixture::new("core-reaches-bevy")
        .outside("bevy_math", &[])
        .outside("shapes", &["bevy_math"])
        .member("opendrone-maths", &[])
        .member("opendrone-physics", &["opendrone-maths", "shapes"])
        .member("opendrone-sim", &["opendrone-maths", "opendrone-physics"])
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "The core must never reach `bevy_math`: it is part of the Bevy game engine, and only \
         the game uses Bevy (ADR-0003). It gets there through \
         opendrone-physics → shapes → bevy_math.",
    );
}

#[test]
fn a_core_crate_reaching_the_operating_system_breaks_the_walls() {
    let outcome = Fixture::new("core-reaches-the-os")
        .outside("libc", &[])
        .member("opendrone-maths", &["libc"])
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "The core must never reach `libc`: it calls the operating system's C library. It gets \
         there through opendrone-maths → libc.",
    );
}

#[test]
fn a_core_crate_using_an_outside_library_nobody_has_checked_breaks_the_walls() {
    let outcome = Fixture::new("core-uses-an-unchecked-library")
        .outside("left-pad", &[])
        .member("opendrone-maths", &[])
        .member(
            "opendrone-flight-controller",
            &["opendrone-maths", "left-pad"],
        )
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "The core reaches `left-pad`, which isn't on its list of allowed outside libraries. It \
         gets there through opendrone-flight-controller → left-pad.",
    );
    outcome.says("add it under [core-libraries] in crates/xtask/walls.toml");
}

#[test]
fn the_core_may_use_libm() {
    let outcome = Fixture::new("core-uses-libm")
        .outside("libm", &[])
        .member("opendrone-maths", &["libm"])
        .check_walls();
    assert!(outcome.passed, "{}", outcome.output);
}

#[test]
fn a_core_crate_using_an_edge_crate_breaks_the_walls() {
    let outcome = Fixture::new("core-uses-an-edge-crate")
        .member("opendrone-maths", &[])
        .member("opendrone-pack", &["opendrone-maths"])
        .member("opendrone-sim", &["opendrone-maths", "opendrone-pack"])
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "`opendrone-sim` uses `opendrone-pack`, but it may use only `opendrone-maths`, \
         `opendrone-physics` and `opendrone-flight-controller`",
    );
}

#[test]
fn an_edge_crate_may_use_libraries_that_touch_the_operating_system() {
    let outcome = Fixture::new("edge-crate-touches-the-os")
        .outside("libc", &[])
        .outside("sdl3-sys", &["libc"])
        .member("opendrone-input", &["sdl3-sys"])
        .check_walls();
    assert!(outcome.passed, "{}", outcome.output);
}

#[test]
fn sound_using_another_opendrone_crate_breaks_the_walls() {
    let outcome = Fixture::new("sound-uses-maths")
        .member("opendrone-maths", &[])
        .member("opendrone-sound", &["opendrone-maths"])
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "`opendrone-sound` uses `opendrone-maths`, but it may use no other OpenDrone crate: \
         sound uses no other OpenDrone crate; the game hands it each frame's state (ADR-0003).",
    );
}

#[test]
fn a_crate_that_adr_0003_doesnt_name_breaks_the_walls() {
    let outcome = Fixture::new("unknown-crate")
        .member("opendrone-collisions", &[])
        .check_walls();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("`opendrone-collisions`");
    outcome.says("isn't one of the crates in crates/xtask/walls.toml");
}

/// What the walls check printed, and whether it passed.
struct Outcome {
    passed: bool,
    output: String,
}

impl Outcome {
    fn says(&self, sentence: &str) {
        assert!(
            self.output.contains(sentence),
            "expected the walls check to say:\n  {sentence}\nbut it said:\n{}",
            self.output
        );
    }
}

/// Runs `cargo xtask walls` on a workspace.
fn walls(manifest: &Path, offline: bool) -> Outcome {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.arg("walls").arg("--manifest-path").arg(manifest);
    if offline {
        command.env("CARGO_NET_OFFLINE", "true");
    }
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
        _ => panic!("the walls check couldn't run:\n{text}"),
    }
}

/// A small workspace written to a scratch folder: OpenDrone crates as members,
/// and stand-in outside libraries in a folder beside it.
struct Fixture {
    root: PathBuf,
    members: Vec<(String, Vec<String>)>,
    test_only: Vec<(String, String)>,
    outside: Vec<(String, Vec<String>)>,
}

impl Fixture {
    fn new(case: &str) -> Fixture {
        Fixture {
            root: Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("walls")
                .join(case),
            members: Vec::new(),
            test_only: Vec::new(),
            outside: Vec::new(),
        }
    }

    /// An OpenDrone crate in the workspace, and what it depends on.
    fn member(mut self, name: &str, uses: &[&str]) -> Fixture {
        self.members.push((name.to_owned(), owned(uses)));
        self
    }

    /// A dependency that only the crate's tests use.
    fn test_only(mut self, name: &str, uses: &str) -> Fixture {
        self.test_only.push((name.to_owned(), uses.to_owned()));
        self
    }

    /// A stand-in for an outside library, and what it depends on.
    fn outside(mut self, name: &str, uses: &[&str]) -> Fixture {
        self.outside.push((name.to_owned(), owned(uses)));
        self
    }

    fn check_walls(&self) -> Outcome {
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
            .map(|(name, _)| format!("{name:?}"))
            .collect();
        write(
            &workspace.join("Cargo.toml"),
            &format!(
                "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
                members.join(", ")
            ),
        );
        for (name, uses) in &self.members {
            let dev: Vec<String> = self
                .test_only
                .iter()
                .filter(|(crate_name, _)| crate_name == name)
                .map(|(_, dev)| dev.clone())
                .collect();
            self.write_package(&workspace.join(name), name, uses, &dev);
        }
        for (name, uses) in &self.outside {
            self.write_package(&self.root.join("outside").join(name), name, uses, &[]);
        }
        walls(&workspace.join("Cargo.toml"), true)
    }

    fn write_package(&self, folder: &Path, name: &str, uses: &[String], dev: &[String]) {
        let mut manifest = format!(
            "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             publish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n"
        );
        for dependency in uses {
            manifest.push_str(&format!(
                "{dependency} = {{ path = {:?} }}\n",
                self.path_to(dependency)
            ));
        }
        manifest.push_str("\n[dev-dependencies]\n");
        for dependency in dev {
            manifest.push_str(&format!(
                "{dependency} = {{ path = {:?} }}\n",
                self.path_to(dependency)
            ));
        }
        write(&folder.join("Cargo.toml"), &manifest);
        write(&folder.join("lib.rs"), "");
    }

    /// Where a dependency lives, seen from any package folder.
    fn path_to(&self, dependency: &str) -> String {
        if self.outside.iter().any(|(name, _)| name == dependency) {
            format!("../../outside/{dependency}")
        } else {
            format!("../../workspace/{dependency}")
        }
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a file has a folder")).expect("can make the folder");
    fs::write(path, contents).expect("can write the fixture file");
}
