//! The licence texts of the libraries compiled into the game (#270, ADR-0014).
//!
//! MIT, Zlib and the rest ask for their texts to travel with a binary copy,
//! and every download is one. `cargo xtask licences` writes, for each
//! operating system, `licences/third-party-<os>.txt`: every library built
//! into the game for that system, with its version, its licence and the full
//! text of its licence files. `cargo xtask package` puts that file into the
//! download as `THIRD-PARTY-LICENSES.txt`; `cargo xtask licences --check`
//! (CI runs it) fails when a file is out of date with `Cargo.lock`.
//!
//! The tool is Cargo's own and nothing else, so no library or development
//! tool is added: `cargo tree` says which libraries the game is built with
//! on a system (normal dependencies only, so no build script's tools and no
//! procedural macro, which run while building and aren't part of the
//! program), and `cargo metadata` says where each one's files are and what
//! licence it declares. `cargo about` would do the same job and add a
//! tool, a configuration file and a list of accepted licences to keep beside
//! `deny.toml`'s.
//!
//! A library's licence texts are the licence files in the top folder of its
//! package (`LICENSE*`, `COPYING*`, `NOTICE*` and the like). A few packages
//! carry none, because their licence files sit in the repository above the
//! crate. For those the file says so and carries the standard text of the
//! licence the library declares: Apache-2.0's, or, for a library offered
//! under MIT or Apache-2.0, Apache-2.0's, which names no copyright holder
//! that would be missing. Any other licence with no file is an error, so a
//! text is never made up.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

/// The operating systems a build is made for, each with the target the
/// Build workflow builds it for (docs/builds-and-releases.md).
pub const SYSTEMS: [(&str, &str); 3] = [
    ("macos", "aarch64-apple-darwin"),
    ("windows", "x86_64-pc-windows-msvc"),
    ("linux", "x86_64-unknown-linux-gnu"),
];

/// The game's crate, whose libraries are listed.
const GAME: &str = "opendrone";

/// The folder, from the repo's root, that holds the generated files.
pub const FOLDER: &str = "licences";

/// The generated file for one operating system, from the repo's root.
pub fn file_for(os: &str) -> String {
    format!("{FOLDER}/third-party-{os}.txt")
}

/// The standard texts for a library that ships no licence file: the game's
/// own, which are the standard ones.
const APACHE_TEXT: &str = include_str!("../../../LICENSE-APACHE");
const MIT_TEXT: &str = include_str!("../../../LICENSE-MIT");

/// A package that Cargo only builds with, whose code ends up in the game all
/// the same, so Cargo's normal dependencies don't show it.
struct NativeSource {
    /// The library in the game's dependencies that builds it.
    used_by: &'static str,
    /// The package that holds the source.
    package: &'static str,
    /// Licence files deeper in the package than its top folder, beside the
    /// ones there, from its folder.
    licence_files: &'static [&'static str],
    why: &'static str,
}

const NATIVE_SOURCES: [NativeSource; 1] = [NativeSource {
    used_by: "sdl3-sys",
    package: "sdl3-src",
    // SDL's own licence, and the BSD licence its bundled HIDAPI is used under.
    licence_files: &["SDL/LICENSE.txt", "SDL/src/hidapi/LICENSE-bsd.txt"],
    why: "SDL itself: its C source, which sdl3-sys builds and links into the game (ADR-0018)",
}];

/// The start of the name of a file that holds a licence or a notice.
const LICENCE_FILE_STARTS: [&str; 6] = [
    "license",
    "licence",
    "copying",
    "unlicense",
    "notice",
    "copyright",
];

pub fn run(args: &[String]) -> ExitCode {
    const USAGE: &str = "Usage: cargo xtask licences [--check] [--os <macos|windows|linux>]";
    let mut check = false;
    let mut only = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--os" => match rest.next() {
                Some(os) if SYSTEMS.iter().any(|(name, _)| name == os) => only = Some(os.clone()),
                _ => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let result = repo().and_then(|root| {
        let mut report = Vec::new();
        let mut stale = Vec::new();
        for (os, target) in SYSTEMS {
            if only.as_deref().is_some_and(|o| o != os) {
                continue;
            }
            let wanted = generate(&root, os, target)?;
            let path = root.join(file_for(os));
            // Git may hand a file out with the other line endings.
            let have = fs::read_to_string(&path)
                .unwrap_or_default()
                .replace("\r\n", "\n");
            if have == wanted {
                report.push(format!("{} is up to date.", file_for(os)));
            } else if check {
                stale.push(file_for(os));
            } else {
                fs::create_dir_all(root.join(FOLDER))
                    .and_then(|()| fs::write(&path, wanted))
                    .map_err(|e| format!("can't write {}: {e}", path.display()))?;
                report.push(format!("Wrote {}.", file_for(os)));
            }
        }
        if stale.is_empty() {
            Ok(report.join("\n"))
        } else {
            Err(format!(
                "{} out of date with Cargo.lock, or missing: run `cargo xtask licences` and commit {}",
                stale.join(" and "),
                if stale.len() == 1 { "it" } else { "them" }
            ))
        }
    });
    match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

/// The repo's root: the nearest folder at or above this one holding both
/// `Cargo.toml` and `.github/`.
fn repo() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| format!("can't tell where this is: {e}"))?;
    here.ancestors()
        .find(|f| f.join("Cargo.toml").is_file() && f.join(".github").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "Run this inside the OpenDrone repo: no folder here or above holds Cargo.toml and .github/."
                .into()
        })
}

/// Runs `cargo <args>` in `root` and returns what it printed.
fn cargo(root: &Path, args: &[&str]) -> Result<String, String> {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("can't run cargo: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(15).collect();
        return Err(format!(
            "`cargo {}` failed:\n{}",
            args.join(" "),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// One outside library of the build.
struct Library {
    name: String,
    version: String,
    licence: String,
    repository: Option<String>,
    /// Numbers of its licence texts, in the order of its files.
    texts: Vec<usize>,
    /// Set when the package had no licence file.
    note: Option<String>,
}

/// The text of the file for one operating system, as `cargo xtask licences`
/// writes it.
pub fn generate(root: &Path, os: &str, target: &str) -> Result<String, String> {
    // The libraries the game is built with on this system: normal
    // dependencies, with no procedural macro, as `cargo tree` resolves them
    // for the game alone.
    let tree = cargo(
        root,
        &[
            "tree",
            "--locked",
            "--package",
            GAME,
            "--target",
            target,
            "--edges",
            "normal,no-proc-macro",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ],
    )?;
    let mut built_with: Vec<(String, String)> = Vec::new();
    for line in tree.lines() {
        let mut words = line.split_whitespace();
        if let (Some(name), Some(version)) = (words.next(), words.next())
            && let Some(version) = version.strip_prefix('v')
        {
            built_with.push((name.to_string(), version.to_string()));
        }
    }
    built_with.sort();
    built_with.dedup();
    if !built_with.iter().any(|(name, _)| name == GAME) {
        return Err(format!(
            "`cargo tree` doesn't list the game, {GAME}, so there is nothing to list the libraries of"
        ));
    }

    // Where each one's files are, and what it declares.
    let metadata = cargo(
        root,
        &[
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ],
    )?;
    let metadata: Value =
        serde_json::from_str(&metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let own: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();

    // Native libraries whose source Cargo only builds with.
    for native in NATIVE_SOURCES {
        if built_with.iter().any(|(name, _)| name == native.used_by) {
            for package in packages.iter().filter(|p| p["name"] == native.package) {
                if let Some(version) = package["version"].as_str() {
                    built_with.push((native.package.to_string(), version.to_string()));
                }
            }
        }
    }
    built_with.sort();
    built_with.dedup();

    let mut libraries: Vec<Library> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut number_of = |text: String| -> usize {
        texts.iter().position(|t| *t == text).unwrap_or_else(|| {
            texts.push(text);
            texts.len() - 1
        })
    };
    for (name, version) in &built_with {
        let package = packages
            .iter()
            .find(|p| p["name"] == name.as_str() && p["version"] == version.as_str())
            .ok_or_else(|| format!("cargo metadata doesn't list {name} {version}"))?;
        if package["id"].as_str().is_some_and(|id| own.contains(&id)) {
            continue;
        }
        let licence = package["license"].as_str().unwrap_or("").to_string();
        let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap_or(""));
        let folder = manifest.parent().unwrap_or(Path::new("."));
        let native = NATIVE_SOURCES.iter().find(|n| n.package == name);
        let mut files = licence_files(
            folder,
            package["license_file"].as_str(),
            native.map_or(&[], |n| n.licence_files),
        )?;
        let mut note = native.map(|n| n.why.to_string());
        if files.is_empty() {
            let (which, text) = standard_text(&licence).ok_or_else(|| {
                format!(
                    "{name} {version} (licence {licence:?}) has no licence file in its package, and \
                     the only standard texts this tool carries are Apache-2.0's and MIT's: add \
                     the library's own licence text by extending `standard_text` in \
                     crates/xtask/src/licences.rs, once you have read it in the library's repository"
                )
            })?;
            note = Some(format!(
                "its package has no licence file, so the standard text of {which} follows, \
                 without its copyright holders' names"
            ));
            files.push(("standard text".to_string(), text));
        }
        libraries.push(Library {
            name: name.clone(),
            version: version.clone(),
            licence,
            repository: package["repository"].as_str().map(str::to_string),
            texts: files.into_iter().map(|(_, text)| number_of(text)).collect(),
            note,
        });
    }

    let mut out = String::new();
    out.push_str(&format!(
        "THIRD-PARTY LICENCES: the libraries built into OpenDrone for {os}\n\
         \n\
         OpenDrone's own code is MIT OR Apache-2.0 (LICENSE-MIT and LICENSE-APACHE). The game is \
         built with the {} libraries below, for {target}, each under its own licence. Its full \
         text, as the library's package carries it, follows the list, once however many libraries \
         share it.\n\
         \n\
         This file is generated from Cargo.lock by `cargo xtask licences`: don't edit it.\n\
         \n\
         \n\
         LIBRARIES\n",
        libraries.len()
    ));
    for library in &libraries {
        out.push_str(&format!(
            "\n{} {}\n    licence: {}\n",
            library.name,
            library.version,
            if library.licence.is_empty() {
                "(not declared)"
            } else {
                &library.licence
            }
        ));
        if let Some(repository) = &library.repository {
            out.push_str(&format!("    source: {repository}\n"));
        }
        let numbers: Vec<String> = library
            .texts
            .iter()
            .map(|n| format!("[{}]", n + 1))
            .collect();
        out.push_str(&format!("    licence text: {}\n", numbers.join(" ")));
        if let Some(note) = &library.note {
            out.push_str(&format!("    note: {note}\n"));
        }
    }
    out.push_str("\n\nLICENCE TEXTS\n");
    for (i, text) in texts.iter().enumerate() {
        let mut names: Vec<&str> = libraries
            .iter()
            .filter(|l| l.texts.contains(&i))
            .map(|l| l.name.as_str())
            .collect();
        names.dedup();
        out.push_str(&format!(
            "\n================================================================================\n\
             [{}] used by: {}\n\
             ================================================================================\n\n{text}",
            i + 1,
            names.join(", ")
        ));
    }
    Ok(out)
}

/// The licence files in a package's top folder, as (file name, text), in
/// name order. `declared` is the manifest's `license-file`, which counts
/// whatever it is called.
fn licence_files(
    folder: &Path,
    declared: Option<&str>,
    more: &[&str],
) -> Result<Vec<(String, String)>, String> {
    let mut names: Vec<String> = Vec::new();
    let entries = fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if LICENCE_FILE_STARTS.iter().any(|s| lower.starts_with(s)) && entry.path().is_file() {
            names.push(name);
        }
    }
    if let Some(declared) = declared
        && folder.join(declared).is_file()
        && !names.iter().any(|n| n == declared)
    {
        names.push(declared.to_string());
    }
    names.sort();
    names.extend(more.iter().map(|m| m.to_string()));
    names
        .into_iter()
        .map(|name| {
            let bytes = fs::read(folder.join(&name))
                .map_err(|e| format!("{}: {e}", folder.join(&name).display()))?;
            Ok((name, tidy(&String::from_utf8_lossy(&bytes))))
        })
        .collect()
}

/// A text with the same line endings on every system, no byte-order mark
/// and no blank space at its end, ending in one newline.
fn tidy(text: &str) -> String {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    format!("{}\n", text.trim_end())
}

/// The standard text for a licence expression, and its name: Apache-2.0's
/// when the library may be used under it, MIT's if not. `None` for any other
/// licence, and for an expression that needs both of two licences (AND),
/// which no single standard text covers.
fn standard_text(expression: &str) -> Option<(&'static str, String)> {
    if expression.split_whitespace().any(|w| w == "AND") {
        return None;
    }
    let options: Vec<&str> = expression
        .split(|c: char| c.is_whitespace() || matches!(c, '/' | '(' | ')'))
        .filter(|w| !w.is_empty() && *w != "OR")
        .collect();
    if options.contains(&"Apache-2.0") {
        Some(("Apache-2.0", tidy(APACHE_TEXT)))
    } else if options.contains(&"MIT") {
        // The game's own MIT text, which is the standard one, but for the
        // line that names its copyright holders.
        let standard: Vec<&str> = MIT_TEXT
            .lines()
            .filter(|l| !l.starts_with("Copyright"))
            .collect();
        Some(("MIT", tidy(&standard.join("\n"))))
    } else {
        None
    }
}
