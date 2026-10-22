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
    /// Each package's version, and the folder its `Cargo.toml` sits in.
    versions: BTreeMap<String, (String, String)>,
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
        let mut versions = BTreeMap::new();
        for package in list {
            let id = package["id"].as_str().unwrap_or_default().to_owned();
            let name = package["name"].as_str().unwrap_or_default().to_owned();
            let folder = package["manifest_path"]
                .as_str()
                .map(Path::new)
                .and_then(Path::parent)
                .map(|folder| folder.display().to_string())
                .unwrap_or_default();
            let version = package["version"].as_str().unwrap_or_default().to_owned();
            versions.insert(id.clone(), (version, folder));
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
            versions,
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

    /// The packages [`build`] asks cargo for: the core crates the workspace has
    /// (a test's small workspace may hold a few), then `with`.
    fn selected<'a>(&self, core: &'a [String], with: &'a [String]) -> Vec<&'a str> {
        let present: BTreeSet<&str> = self.names.values().map(String::as_str).collect();
        core.iter()
            .filter(|name| present.contains(name.as_str()))
            .chain(with)
            .map(String::as_str)
            .collect()
    }

    /// The packages a line of `cargo tree` names, by id: its name and
    /// version, as `glamx v0.1.0 (/path/to/glamx) (*)`. Two packages can share
    /// a name and version only from different places, so then the place
    /// printed after them picks; failing that, both count.
    fn ids_of(&self, line: &str) -> Result<Vec<&str>, String> {
        let mut words = line.split_whitespace();
        let name = words.next().unwrap_or_default();
        let version = words
            .next()
            .and_then(|word| word.strip_prefix('v'))
            .unwrap_or_default();
        let named: Vec<&str> = self
            .versions
            .iter()
            .filter(|(id, (package_version, _))| {
                package_version == version && self.name(id) == name
            })
            .map(|(id, _)| id.as_str())
            .collect();
        if named.is_empty() {
            return Err(format!(
                "cargo tree names {line:?}, a package cargo metadata doesn't describe"
            ));
        }
        let placed: Vec<&str> = named
            .iter()
            .copied()
            .filter(|id| {
                self.versions
                    .get(*id)
                    .is_some_and(|(_, folder)| line.contains(&format!("({folder})")))
            })
            .collect();
        Ok(if placed.len() == 1 { placed } else { named })
    }
}

/// Every library the core crates are built with in the build [`build`]
/// makes (their normal dependencies, all the way down, and themselves), by
/// id, with its name. A dependency counts only if that build turns it on:
/// one only another operating system uses, or one turned on only by a
/// feature of a package outside the build, isn't built, so it isn't the
/// core's. `cargo tree`, asked for the same packages and target as the build,
/// works that out as the build does. Code-writing macros only run while
/// building, so they are left out.
pub(super) fn core_closure(
    packages: &Packages,
    core: &[String],
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
) -> Result<BTreeMap<String, String>, String> {
    let mut command = cargo();
    command.args([
        "tree",
        "--target",
        target,
        "--edges",
        "normal,no-proc-macro",
        "--prefix",
        "depth",
        "--format",
        "{p}",
        "--color",
        "never",
    ]);
    if let Some(path) = manifest_path {
        command.args(["--manifest-path", path]);
    }
    for package in packages.selected(core, with) {
        command.args(["-p", package]);
    }
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("couldn't run cargo tree: {error}"))?;
    if !output.status.success() {
        return Err("cargo tree failed (its messages are above)".to_owned());
    }
    // Each line is a package, after its depth in the tree it's printed in, as
    // `3glamx v0.1.0`; it uses the packages printed one deeper right below
    // it. A package printed in full once is marked `(*)` after that, with
    // nothing below it, so every package's dependencies are printed once.
    let mut uses: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut above: Vec<Vec<&str>> = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let digits = line
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(line.len());
        let Ok(depth) = line[..digits].parse::<usize>() else {
            continue;
        };
        if depth > above.len() {
            return Err(format!("cargo tree printed {line:?} below nothing"));
        }
        let ids = packages.ids_of(&line[digits..])?;
        above.truncate(depth);
        for parent in above.last().into_iter().flatten() {
            uses.entry(parent).or_default().extend(&ids);
        }
        above.push(ids);
    }
    let mut waiting: VecDeque<&str> = packages
        .members
        .iter()
        .map(String::as_str)
        .filter(|id| core.iter().any(|name| name == packages.name(id)))
        .collect();
    let mut reached: BTreeMap<String, String> = BTreeMap::new();
    while let Some(id) = waiting.pop_front() {
        if packages.macros.contains(id) || reached.contains_key(id) {
            continue;
        }
        reached.insert(id.to_owned(), packages.name(id).to_owned());
        waiting.extend(uses.get(id).into_iter().flatten());
    }
    Ok(reached)
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
    /// The programs' package ids that cargo didn't build this time, because
    /// an earlier build was still fresh. Their kept object file is the last
    /// build's, which may have had other features, so it can't be trusted.
    pub(super) fresh: BTreeSet<String>,
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
    // changed, so a program's object file is its last build's, and a build
    // with other features (another `--with`) may have overwritten it since.
    // If one is gone, or cargo reused an earlier build of its program, its
    // package is cleaned and built again, once.
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
            .filter(|(id, _, object)| !object.exists() || built.fresh.contains(id))
            .map(|(id, _, _)| packages.name(id))
            .collect();
        if missing.is_empty() {
            return Ok(built);
        }
        if attempt > 0 {
            return Err(format!(
                "cargo built {} without keeping its fresh compiled code in {}",
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
    for package in packages.selected(core, with) {
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
        fresh: BTreeSet::new(),
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
                if message["fresh"].as_bool() == Some(true) {
                    built.fresh.insert(id.to_owned());
                }
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
