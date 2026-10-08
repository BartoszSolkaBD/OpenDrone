//! `cargo xtask core-maths`: checks that the code the core runs never calls
//! the operating system's maths library ([ADR-0001]).
//!
//! The operating systems' maths libraries (`cos`, `acos`, `log2` and the
//! rest) give different last bits on different computers, so the core takes
//! every maths function from `libm` instead. Clippy keeps std's float methods
//! out of the core crates' own code, but not out of the libraries they use,
//! and a library can switch to std's maths without a line of it changing:
//! once std is anywhere in a crate's dependencies, Rust resolves `x.acos()` on
//! an `f64` to std's own method, ahead of a maths trait such as simba's.
//!
//! So this check looks at the machine code itself:
//!
//! 1. It works out every library the core crates (the `core` list in
//!    `walls.toml`) are built with, leaving out code-writing macros, which
//!    only run while building.
//! 2. It builds the core crates, plus any packages named with `--with`, in one
//!    `cargo build`, so the features every package turns on are merged as they
//!    are in the real build. The game's ticket runs it `--with opendrone`, so
//!    Bevy's features count.
//! 3. It reads every one of those libraries' compiled code (each `.rlib`) and
//!    lists every call to a maths function the code leaves for the operating
//!    system to supply, with the functions that make it.
//!
//! Any call not allowed in `walls.toml`'s `[core-platform-maths]`, with its
//! reason, fails the check with a plain sentence naming the library, the
//! maths function and where it is called from.
//!
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use object::read::archive::ArchiveFile;
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolKind};
use serde_json::Value;

use crate::walls::{RULES_FILE, Rules};

/// The C maths library's functions, double and single precision: every one
/// whose result can differ from one operating system's library to another.
/// Exact operations (`sqrt`, `fabs`, `floor`, `ceil`, `trunc`, `round`,
/// `copysign`) aren't listed: every library gives the same bits for them.
const PLATFORM_MATHS: &[&str] = &[
    "acos",
    "acosf",
    "acosh",
    "acoshf",
    "asin",
    "asinf",
    "asinh",
    "asinhf",
    "atan",
    "atanf",
    "atan2",
    "atan2f",
    "atanh",
    "atanhf",
    "cbrt",
    "cbrtf",
    "cos",
    "cosf",
    "cosh",
    "coshf",
    "erf",
    "erff",
    "erfc",
    "erfcf",
    "exp",
    "expf",
    "exp2",
    "exp2f",
    "exp10",
    "exp10f",
    "expm1",
    "expm1f",
    "fdim",
    "fdimf",
    "fma",
    "fmaf",
    "fmod",
    "fmodf",
    "hypot",
    "hypotf",
    "lgamma",
    "lgammaf",
    "lgamma_r",
    "lgammaf_r",
    "log",
    "logf",
    "log10",
    "log10f",
    "log1p",
    "log1pf",
    "log2",
    "log2f",
    "pow",
    "powf",
    "remainder",
    "remainderf",
    "sin",
    "sinf",
    "sincos",
    "sincosf",
    "__sincos_stret",
    "__sincosf_stret",
    "sinh",
    "sinhf",
    "tan",
    "tanf",
    "tanh",
    "tanhf",
    "tgamma",
    "tgammaf",
];

const USAGE: &str = "Usage: cargo xtask core-maths [--with <package>]... [--target <triple>] \
                     [--manifest-path <Cargo.toml>]";

pub fn run(args: &[String]) -> ExitCode {
    let mut with = Vec::new();
    let mut manifest_path = None;
    let mut target = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match (arg.as_str(), rest.next()) {
            ("--with", Some(package)) => with.push(package.clone()),
            ("--manifest-path", Some(path)) => manifest_path = Some(path.clone()),
            ("--target", Some(triple)) => target = Some(triple.clone()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let checked = Rules::load().and_then(|rules| {
        let target = match target.clone() {
            Some(target) => target,
            None => host_triple()?,
        };
        let calls = platform_maths_calls(&rules, &with, manifest_path.as_deref(), &target)?;
        Ok((rules, calls))
    });
    let (rules, (libraries, calls)) = match checked {
        Ok(found) => found,
        Err(error) => {
            eprintln!("Could not check the core's maths: {error}");
            return ExitCode::from(2);
        }
    };
    let (allowed, refused): (Vec<&Call>, Vec<&Call>) = calls
        .iter()
        .partition(|call| rules.platform_maths_reason(call).is_some());
    if refused.is_empty() {
        println!(
            "The core calls none of the operating system's maths: {libraries} libraries checked{}.",
            if with.is_empty() {
                String::new()
            } else {
                format!(", built together with {}", with.join(", "))
            }
        );
        for call in allowed {
            println!(
                "- Allowed by {RULES_FILE}: `{}` calls `{}` (from {}): {}.",
                call.library,
                call.function,
                call.callers(),
                rules.platform_maths_reason(call).unwrap_or_default()
            );
        }
        return ExitCode::SUCCESS;
    }
    println!(
        "The core calls the operating system's maths, whose results differ from one operating \
         system to another; the core must take every maths function from libm (ADR-0001):"
    );
    for call in refused {
        println!(
            "- `{}` calls the operating system's `{}` (from {}). If it can never run in the \
             Simulation, allow it under [core-platform-maths] in {RULES_FILE} with the reason; \
             otherwise stop it being called.",
            call.library,
            call.function,
            call.callers()
        );
    }
    ExitCode::FAILURE
}

/// One library's calls to one of the operating system's maths functions.
#[derive(Debug)]
pub(crate) struct Call {
    pub library: String,
    pub function: String,
    /// The functions in the library that make the call, demangled.
    pub callers: BTreeSet<String>,
}

impl Call {
    fn callers(&self) -> String {
        const SHOWN: usize = 3;
        let mut names: Vec<&str> = self
            .callers
            .iter()
            .map(String::as_str)
            .take(SHOWN)
            .collect();
        if names.is_empty() {
            names.push("code this check couldn't name");
        }
        let more = self.callers.len().saturating_sub(SHOWN);
        let mut text = names.join(", ");
        if more > 0 {
            text.push_str(&format!(" and {more} more"));
        }
        text
    }
}

/// How many libraries the core is built with, and every call any of them
/// makes to the operating system's maths, in order of library and function.
fn platform_maths_calls(
    rules: &Rules,
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
) -> Result<(usize, Vec<Call>), String> {
    let metadata = cargo_metadata(manifest_path)?;
    let core_libraries = core_closure(&metadata, &rules.core)?;
    let rlibs = build(&rules.core, with, manifest_path, target)?;
    let mut calls = Vec::new();
    for (id, name) in &core_libraries {
        let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for rlib in rlibs.get(id).into_iter().flatten() {
            let bytes = std::fs::read(rlib)
                .map_err(|error| format!("can't read {}: {error}", rlib.display()))?;
            read_rlib(&bytes, &mut found)
                .map_err(|error| format!("can't read {}: {error}", rlib.display()))?;
        }
        for (function, callers) in found {
            calls.push(Call {
                library: name.clone(),
                function,
                callers,
            });
        }
    }
    Ok((core_libraries.len(), calls))
}

fn cargo_metadata(manifest_path: Option<&str>) -> Result<Value, String> {
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

/// Every package the core crates are built with (their normal dependencies,
/// all the way down, and themselves), by id, with its name. Code-writing
/// macros and build scripts' libraries only run while building, so they are
/// left out.
fn core_closure(metadata: &Value, core: &[String]) -> Result<BTreeMap<String, String>, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages")?;
    let mut names = BTreeMap::new();
    let mut macros = BTreeSet::new();
    for package in packages {
        let id = package["id"].as_str().unwrap_or_default().to_owned();
        let is_macro = package["targets"].as_array().is_some_and(|targets| {
            targets.iter().any(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "proc-macro"))
            })
        });
        if is_macro {
            macros.insert(id.clone());
        }
        names.insert(id, package["name"].as_str().unwrap_or_default().to_owned());
    }
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("cargo metadata has no resolve graph")?;
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
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
        .ok_or("cargo metadata has no workspace members")?;
    let mut queue: VecDeque<String> = members
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| names.get(*id).is_some_and(|name| core.contains(name)))
        .map(str::to_owned)
        .collect();
    let mut reached: BTreeMap<String, String> = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        if macros.contains(&id) || reached.contains_key(&id) {
            continue;
        }
        reached.insert(id.clone(), names.get(&id).cloned().unwrap_or_default());
        for next in uses.get(&id).into_iter().flatten() {
            queue.push_back(next.clone());
        }
    }
    let mut by_name: Vec<(String, String)> = reached.into_iter().collect();
    by_name.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));
    Ok(by_name.into_iter().collect())
}

/// The computer's own target, such as `aarch64-apple-darwin`. The core is
/// built for it by name, so the libraries built for the program land in
/// their own folder, apart from those built for build scripts and macros.
fn host_triple() -> Result<String, String> {
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

/// Builds the core crates and `with` together, and gives every library's
/// compiled `.rlib` files built for the program, by package id.
fn build(
    core: &[String],
    with: &[String],
    manifest_path: Option<&str>,
    host: &str,
) -> Result<BTreeMap<String, Vec<PathBuf>>, String> {
    let mut command = cargo();
    command.args(["build", "--message-format=json", "--target", host]);
    if let Some(path) = manifest_path {
        command.args(["--manifest-path", path]);
    }
    // Only the core crates the workspace has (a test's small workspace may
    // hold a few).
    let metadata = cargo_metadata(manifest_path)?;
    let present: BTreeSet<&str> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| package["name"].as_str())
        .collect();
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
    let mut rlibs: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
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
        for file in message["filenames"].as_array().into_iter().flatten() {
            let Some(file) = file.as_str().map(Path::new) else {
                continue;
            };
            let for_the_program = file
                .components()
                .any(|part| part.as_os_str() == std::ffi::OsStr::new(host));
            if for_the_program && file.extension().is_some_and(|ext| ext == "rlib") {
                rlibs
                    .entry(id.to_owned())
                    .or_default()
                    .push(file.to_path_buf());
            }
        }
    }
    Ok(rlibs)
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// Adds every call in one `.rlib` (an archive of object files) to one of
/// the operating system's maths functions, with the functions that make it.
pub(crate) fn read_rlib(
    bytes: &[u8],
    found: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    let archive = ArchiveFile::parse(bytes).map_err(|error| error.to_string())?;
    for member in archive.members() {
        let member = member.map_err(|error| error.to_string())?;
        let data = member.data(bytes).map_err(|error| error.to_string())?;
        // The archive also holds Rust's own metadata, which isn't code.
        let Ok(file) = object::File::parse(data) else {
            continue;
        };
        read_object(&file, found);
    }
    Ok(())
}

/// A symbol's plain name: Mach-O puts an underscore before every C name.
fn plain_name<'a>(file: &object::File<'_>, name: &'a str) -> &'a str {
    if file.format() == object::BinaryFormat::MachO {
        name.strip_prefix('_').unwrap_or(name)
    } else {
        name
    }
}

fn read_object(file: &object::File<'_>, found: &mut BTreeMap<String, BTreeSet<String>>) {
    let maths: BTreeMap<usize, String> = file
        .symbols()
        .filter(|symbol| symbol.is_undefined())
        .filter_map(|symbol| {
            let name = plain_name(file, symbol.name().ok()?);
            PLATFORM_MATHS
                .contains(&name)
                .then(|| (symbol.index().0, name.to_owned()))
        })
        .collect();
    if maths.is_empty() {
        return;
    }
    for name in maths.values() {
        found.entry(name.clone()).or_default();
    }
    // Which function each call is in: the last function symbol at or before
    // the call, in the same section.
    for section in file.sections() {
        let mut functions: Vec<(u64, String)> = file
            .symbols()
            .filter(|symbol| {
                symbol.section_index() == Some(section.index())
                    && matches!(symbol.kind(), SymbolKind::Text)
            })
            .filter_map(|symbol| Some((symbol.address(), symbol.name().ok()?.to_owned())))
            .collect();
        functions.sort();
        for (offset, relocation) in section.relocations() {
            let RelocationTarget::Symbol(index) = relocation.target() else {
                continue;
            };
            let Some(name) = maths.get(&index.0) else {
                continue;
            };
            let at = section.address() + offset;
            // Every name at that start: identical functions may be merged into
            // one, which then answers to each of their names.
            let Some((start, _)) = functions.iter().rev().find(|(start, _)| *start <= at) else {
                continue;
            };
            for (_, caller) in functions.iter().filter(|(address, _)| address == start) {
                let caller = format!("{:#}", rustc_demangle::demangle(plain_name(file, caller)));
                found.entry(name.clone()).or_default().insert(caller);
            }
        }
    }
}
