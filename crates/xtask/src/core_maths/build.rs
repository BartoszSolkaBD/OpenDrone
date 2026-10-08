//! Building the core for `cargo xtask core-maths`, and what `cargo metadata`
//! says about the packages.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

pub(super) fn cargo_metadata(manifest_path: Option<&str>) -> Result<Value, String> {
    let mut command = cargo();
    command.args(["metadata", "--format-version", "1"]);
    if let Some(path) = manifest_path {
        command.args(["--manifest-path", path]);
    }
    let output = command
        .output()
        .map_err(|error| format!("couldn't run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata gave unreadable output: {error}"))
}

/// The packages `cargo metadata` describes, by id.
pub(super) struct Packages {
    names: BTreeMap<String, String>,
    /// Each library's name in Rust code, such as `parry3d_f64`.
    crate_names: BTreeMap<String, String>,
    /// Code-writing macros, which only run while building.
    macros: BTreeSet<String>,
    /// Each package's normal dependencies (not build or test ones).
    uses: BTreeMap<String, Vec<String>>,
    members: Vec<String>,
    target_directory: PathBuf,
}

impl Packages {
    pub(super) fn read(metadata: &Value) -> Result<Packages, String> {
        let list = metadata["packages"]
            .as_array()
            .ok_or("cargo metadata has no packages")?;
        let mut names = BTreeMap::new();
        let mut crate_names = BTreeMap::new();
        let mut macros = BTreeSet::new();
        for package in list {
            let id = package["id"].as_str().unwrap_or_default().to_owned();
            let name = package["name"].as_str().unwrap_or_default().to_owned();
            let targets = package["targets"].as_array().into_iter().flatten();
            for target in targets {
                let kinds: Vec<&str> = target["kind"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if kinds.contains(&"proc-macro") {
                    macros.insert(id.clone());
                }
                if kinds
                    .iter()
                    .any(|kind| matches!(*kind, "lib" | "rlib" | "dylib" | "staticlib"))
                    && let Some(crate_name) = target["name"].as_str()
                {
                    crate_names.insert(id.clone(), crate_name.replace('-', "_"));
                }
            }
            names.insert(id, name);
        }
        let nodes = metadata["resolve"]["nodes"]
            .as_array()
            .ok_or("cargo metadata has no resolve graph")?;
        let mut uses = BTreeMap::new();
        for node in nodes {
            let id = node["id"].as_str().unwrap_or_default().to_owned();
            let normal = node["deps"].as_array().into_iter().flatten().filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()))
            });
            uses.insert(
                id,
                normal
                    .filter_map(|dep| dep["pkg"].as_str().map(str::to_owned))
                    .collect(),
            );
        }
        let members = metadata["workspace_members"]
            .as_array()
            .ok_or("cargo metadata has no workspace members")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let target_directory = metadata["target_directory"]
            .as_str()
            .map(PathBuf::from)
            .ok_or("cargo metadata names no target directory")?;
        Ok(Packages {
            names,
            crate_names,
            macros,
            uses,
            members,
            target_directory,
        })
    }

    pub(super) fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map(String::as_str).unwrap_or(id)
    }

    pub(super) fn crate_name(&self, id: &str) -> &str {
        self.crate_names
            .get(id)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Every package the core crates are built with (their normal
    /// dependencies, all the way down, and themselves), by id, with its name,
    /// in order of name. Code-writing macros only run while building, so they
    /// are left out.
    pub(super) fn core_closure(&self, core: &[String]) -> BTreeMap<String, String> {
        let mut queue: VecDeque<&String> = self
            .members
            .iter()
            .filter(|id| core.iter().any(|name| name == self.name(id)))
            .collect();
        let mut reached: BTreeMap<String, String> = BTreeMap::new();
        while let Some(id) = queue.pop_front() {
            if self.macros.contains(id) || reached.contains_key(id) {
                continue;
            }
            reached.insert(id.clone(), self.name(id).to_owned());
            queue.extend(self.uses.get(id).into_iter().flatten());
        }
        let mut by_name: Vec<(String, String)> = reached.into_iter().collect();
        by_name.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));
        by_name.into_iter().collect()
    }

    /// Whether the package uses any of `libraries`, directly or through
    /// others.
    pub(super) fn uses_any(&self, id: &str, libraries: &BTreeMap<String, String>) -> bool {
        let mut seen = BTreeSet::new();
        let mut waiting = vec![id];
        while let Some(id) = waiting.pop() {
            if !seen.insert(id) {
                continue;
            }
            for next in self.uses.get(id).into_iter().flatten() {
                if libraries.contains_key(next) {
                    return true;
                }
                waiting.push(next);
            }
        }
        false
    }
}

/// The computer's own target, such as `aarch64-apple-darwin`. The core is
/// built for it by name, so the libraries built for the program land in
/// their own folder, apart from those built for build scripts and macros.
pub(super) fn host_triple() -> Result<String, String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(|error| format!("couldn't run rustc: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV names no host".to_owned())
}

/// What one `cargo build` made for the program.
pub(super) struct Built {
    /// Every library's compiled `.rlib` files, by package id.
    pub(super) rlibs: BTreeMap<String, Vec<PathBuf>>,
    /// The `--with` packages' programs: the package id, the program's name
    /// and the object file holding its compiled code.
    pub(super) programs: Vec<(String, String, PathBuf)>,
}

/// While this is set, xtask runs as the compiler wrapper `build` gives
/// cargo, and keeps each program's compiled code in the folder it names.
const OBJECTS: &str = "OPENDRONE_CORE_MATHS_OBJECTS";

/// Builds the core crates and `with` together, in one `cargo build`, so every
/// library and program is built with the features merged across them all.
/// A program's own compiled code is otherwise gone once it is linked, so
/// cargo runs the workspace's crates through this program, which also keeps
/// each program's compiled code as one object file
/// ([`compile_keeping_programs`]).
pub(super) fn build(
    packages: &Packages,
    core: &[String],
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
) -> Result<Built, String> {
    let objects = packages.target_directory.join(target).join("core-maths");
    std::fs::create_dir_all(&objects)
        .map_err(|error| format!("can't make {}: {error}", objects.display()))?;
    let wrapper = std::env::current_exe()
        .map_err(|error| format!("can't find this program's own file: {error}"))?;
    // Cargo builds a program again only when something it's made from
    // changed, so a program's object file is its last build's. If one is
    // gone, its package is cleaned and built again, once.
    for attempt in 0..2 {
        let built = build_once(
            packages,
            core,
            with,
            manifest_path,
            target,
            &wrapper,
            &objects,
        )?;
        let missing: BTreeSet<&str> = built
            .programs
            .iter()
            .filter(|(_, _, object)| !object.exists())
            .map(|(id, _, _)| packages.name(id))
            .collect();
        if missing.is_empty() {
            return Ok(built);
        }
        if attempt > 0 {
            return Err(format!(
                "cargo built {} without keeping its compiled code in {}",
                missing.into_iter().collect::<Vec<_>>().join(", "),
                objects.display()
            ));
        }
        let mut clean = cargo();
        clean.args(["clean", "--target", target]);
        for package in missing {
            clean.args(["-p", package]);
        }
        if let Some(path) = manifest_path {
            clean.args(["--manifest-path", path]);
        }
        let status = clean
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| format!("couldn't run cargo clean: {error}"))?;
        if !status.success() {
            return Err("cargo clean failed (its messages are above)".to_owned());
        }
    }
    unreachable!("the loop returns on its second pass")
}

fn build_once(
    packages: &Packages,
    core: &[String],
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
    wrapper: &Path,
    objects: &Path,
) -> Result<Built, String> {
    let mut command = cargo();
    command.args(["build", "--message-format=json", "--target", target]);
    if let Some(path) = manifest_path {
        command.args(["--manifest-path", path]);
    }
    // Only the core crates the workspace has (a test's small workspace may
    // hold a few).
    let present: BTreeSet<&str> = packages.names.values().map(String::as_str).collect();
    for package in core.iter().filter(|name| present.contains(name.as_str())) {
        command.args(["-p", package]);
    }
    for package in with {
        command.args(["-p", package]);
    }
    let output = command
        .env("RUSTC_WORKSPACE_WRAPPER", wrapper)
        .env(OBJECTS, objects)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("couldn't run cargo build: {error}"))?;
    if !output.status.success() {
        return Err("cargo build failed (its messages are above)".to_owned());
    }
    let mut built = Built {
        rlibs: BTreeMap::new(),
        programs: Vec::new(),
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        let Some(id) = message["package_id"].as_str() else {
            continue;
        };
        let is_program = message["target"]["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"));
        if is_program {
            if with.iter().any(|name| name == packages.name(id))
                && let Some(program) = message["target"]["name"].as_str()
            {
                let object = objects.join(object_name(&program.replace('-', "_")));
                built
                    .programs
                    .push((id.to_owned(), program.to_owned(), object));
            }
            continue;
        }
        for file in message["filenames"].as_array().into_iter().flatten() {
            let Some(file) = file.as_str().map(Path::new) else {
                continue;
            };
            let for_the_program = file
                .components()
                .any(|part| part.as_os_str() == std::ffi::OsStr::new(target));
            if for_the_program && file.extension().is_some_and(|ext| ext == "rlib") {
                built
                    .rlibs
                    .entry(id.to_owned())
                    .or_default()
                    .push(file.to_path_buf());
            }
        }
    }
    Ok(built)
}

/// The object file a program's compiled code is kept in, by its crate name.
fn object_name(crate_name: &str) -> String {
    format!("{crate_name}.o")
}

/// When cargo runs xtask as the compiler wrapper [`build`] gives it, compiles
/// the crate and gives the compiler's exit code. A program built for the
/// target (not a build script) is compiled as one codegen unit, keeping its
/// compiled code as an object file as well. Returns `None` when xtask runs
/// as itself.
pub(crate) fn compile_keeping_programs() -> Option<std::process::ExitCode> {
    let objects = PathBuf::from(std::env::var_os(OBJECTS)?);
    let mut args = std::env::args_os().skip(1);
    let rustc = args.next()?;
    let args: Vec<std::ffi::OsString> = args.collect();
    let value_after = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|at| args.get(at + 1))
            .and_then(|value| value.to_str())
    };
    let is_program = value_after("--crate-type") == Some("bin");
    let for_the_target = args.iter().any(|arg| arg == "--target");
    let mut command = Command::new(rustc);
    command.args(&args);
    if is_program
        && for_the_target
        && let Some(crate_name) = value_after("--crate-name")
        && !crate_name.starts_with("build_script_")
    {
        command.args(["-C", "codegen-units=1"]).arg(format!(
            "--emit=obj={}",
            objects.join(object_name(crate_name)).display()
        ));
    }
    let code = match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("xtask couldn't run the compiler: {error}");
            1
        }
    };
    Some(std::process::ExitCode::from(
        u8::try_from(code).unwrap_or(1),
    ))
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}
