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
    /// The `--with` packages' programs: the package id and the program's
    /// name.
    pub(super) programs: Vec<(String, String)>,
}

/// Builds the core crates and `with` together.
pub(super) fn build(
    packages: &Packages,
    core: &[String],
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
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
                built.programs.push((id.to_owned(), program.to_owned()));
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

/// Builds one of the `--with` packages' programs again, keeping its compiled
/// code as one object file (a program's own is otherwise gone once it is
/// linked), and gives the file's path. Cargo builds it again only when
/// something it's made from changed, so the file at that path is the last
/// build's; if the file is gone, the package is cleaned and built afresh.
pub(super) fn build_program(
    packages: &Packages,
    package: &str,
    program: &str,
    manifest_path: Option<&str>,
    target: &str,
) -> Result<PathBuf, String> {
    let folder = packages.target_directory.join(target).join("core-maths");
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("can't make {}: {error}", folder.display()))?;
    let object = folder.join(format!("{package}-{program}.o"));
    for attempt in 0..2 {
        if attempt > 0 {
            let mut clean = cargo();
            clean.args(["clean", "--target", target, "-p", package]);
            if let Some(path) = manifest_path {
                clean.args(["--manifest-path", path]);
            }
            run_cargo(clean, "cargo clean")?;
        }
        let mut command = cargo();
        command.args(["rustc", "--target", target, "-p", package, "--bin", program]);
        if let Some(path) = manifest_path {
            command.args(["--manifest-path", path]);
        }
        // One codegen unit, so the object file is one file at this path.
        command
            .args(["--", "-C", "codegen-units=1"])
            .arg(format!("--emit=link,obj={}", object.display()));
        run_cargo(command, "cargo rustc")?;
        if object.exists() {
            return Ok(object);
        }
    }
    Err(format!(
        "cargo rustc didn't write {package}'s program {program} to {}",
        object.display()
    ))
}

/// Runs a cargo command whose messages go to the screen.
fn run_cargo(mut command: Command, what: &str) -> Result<(), String> {
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("couldn't run {what}: {error}"))?;
    if !status.success() {
        return Err(format!("{what} failed (its messages are above)"));
    }
    Ok(())
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}
