//! Builds, releases and updates (#81, #15 §8, ADR-0021): the parts of the
//! Build, Release, Bevy upgrade and Security workflows worth a readable
//! check. The workflows themselves only fetch, build and talk to GitHub.
//!
//! - `release-version <tag>`: the version a tag such as `v0.1.0-alpha.1`
//!   releases, or why it isn't a release tag.
//! - `package`: lays out a built game as a download, one zip per operating
//!   system, in the layout the game looks for beside itself
//!   (`opendrone::set_up::find_repo_folder`).
//! - `bevy-upgrade <index file>`: the "Upgrade to Bevy 0.N" issues to open,
//!   from crates.io's index entry for `bevy` and the fixed template.
//! - `advisories <cargo-deny output>`: the issues the daily security scan
//!   opens, one per advisory.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::materials;
use crate::review::markdown::{code, plain};

/// The fixed text of every "Upgrade to Bevy 0.N" issue, with `{version}`
/// (such as `0.21`), `{release}` (`0.21.0`) and `{pinned}` (the Bevy in
/// Cargo.toml) filled in.
pub const BEVY_TEMPLATE: &str = ".github/bevy-upgrade-issue.md";

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

fn finish(result: Result<String, String>) -> ExitCode {
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

// ----- release-version -----------------------------------------------------

pub fn run_release_version(args: &[String]) -> ExitCode {
    match args {
        [tag] => finish(release_version(tag)),
        _ => {
            eprintln!("Usage: cargo xtask release-version <tag>");
            ExitCode::from(2)
        }
    }
}

/// A number with no sign and no leading zero.
fn number(text: &str) -> Option<u64> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits || (text.len() > 1 && text.starts_with('0')) {
        return None;
    }
    text.parse().ok()
}

/// The version a tag releases: `v0.1.0-alpha.N` before the 0.1.0 alpha, and
/// `vX.Y.Z` (or `vX.Y.Z-alpha.N` ahead of it) from 0.1.0 on (#15 §8).
pub fn release_version(tag: &str) -> Result<String, String> {
    let refuse = |why: &str| {
        Err(format!(
            "{tag:?} isn't a release tag: {why}. A release tag is v0.1.0-alpha.N for a test build \
             before the 0.1.0 alpha (such as v0.1.0-alpha.1), or vX.Y.Z (or vX.Y.Z-alpha.N) \
             from 0.1.0 on."
        ))
    };
    let Some(version) = tag.strip_prefix('v') else {
        return refuse("it doesn't start with v");
    };
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let parts: Vec<Option<u64>> = core.split('.').map(number).collect();
    let [Some(major), Some(minor), Some(patch)] = parts[..] else {
        return refuse("its version isn't three whole numbers, such as 0.1.0");
    };
    if (major, minor, patch) < (0, 1, 0) {
        return refuse("builds before 0.1.0 are 0.1.0-alpha.N");
    }
    if let Some(pre) = pre {
        match pre.strip_prefix("alpha.").and_then(number) {
            Some(n) if n >= 1 => {}
            _ => return refuse("the only test builds are alpha.N, counting from 1"),
        }
    }
    Ok(version.to_string())
}

// ----- package -------------------------------------------------------------

/// The operating systems a build is made for, by the name in its zip.
const SYSTEMS: [&str; 3] = ["macos", "windows", "linux"];

/// What a download holds besides the game itself, from the repo's root. The
/// game finds `packs/` and `scenarios/` beside itself, and the material
/// library and its textures under `assets-src/`, as in the repo.
const FOLDERS: [&str; 2] = ["packs", "scenarios"];
const FILES: [&str; 4] = [
    materials::MANIFEST,
    "CREDITS.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
];

pub fn run_package(args: &[String]) -> ExitCode {
    const USAGE: &str = "Usage: cargo xtask package --game <built program> --os <macos|windows|linux> \
                         --name <version or build name> [--out <folder>] [--no-textures]";
    let mut game = None;
    let mut os = None;
    let mut name = None;
    let mut out = PathBuf::from("target/dist");
    let mut textures = true;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let given = match arg.as_str() {
            "--game" => rest.next().map(|v| game = Some(PathBuf::from(v))),
            "--os" => rest.next().map(|v| os = Some(v.clone())),
            "--name" => rest.next().map(|v| name = Some(v.clone())),
            "--out" => rest.next().map(|v| out = PathBuf::from(v)),
            "--no-textures" => {
                textures = false;
                Some(())
            }
            _ => None,
        };
        if given.is_none() {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    let (Some(game), Some(os), Some(name)) = (game, os, name) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    finish(repo().and_then(|root| package(&root, &game, &os, &name, &out, textures)))
}

/// Every file under `folder`, as paths from the repo's root with `/`
/// between folders, in name order. Hidden files, such as `.DS_Store`, are
/// left out.
fn files_under(root: &Path, folder: &str) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let mut waiting = vec![folder.to_string()];
    while let Some(here) = waiting.pop() {
        let entries = fs::read_dir(root.join(&here)).map_err(|e| format!("{here}: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{here}: {e}"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = format!("{here}/{name}");
            let kind = entry.file_type().map_err(|e| format!("{path}: {e}"))?;
            if kind.is_dir() {
                waiting.push(path);
            } else if kind.is_file() {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The short note at the top of every download.
fn read_me(name: &str, os: &str) -> String {
    let start = match os {
        "macos" => {
            "Double-click `opendrone`. It opens in a Terminal window, which shows what the game \
             says. The build isn't signed, so the first time macOS refuses to open it: open \
             System Settings, then Privacy & Security, and click Open Anyway beside opendrone's \
             name. (Or, in Terminal, in this folder: xattr -dr com.apple.quarantine . )"
        }
        "windows" => {
            "Double-click `opendrone.exe`. The build isn't signed, so the first time Windows \
             SmartScreen may say it protected your PC: click More info, then Run anyway."
        }
        _ => {
            "Run ./opendrone from this folder, or double-click it if your file manager runs \
             programs. It needs the system's sound library (alsa-lib) and a Vulkan driver, \
             which most desktops have."
        }
    };
    format!(
        "OpenDrone {name}, an unsigned test build of the FPV simulator.\r\n\
         \r\n\
         Starting it: {start}\r\n\
         \r\n\
         Keep this folder together: the game reads its Packs (packs/), the Scenarios \
         (scenarios/) and the Maps' textures (assets-src/) from beside itself, and saves \
         flights into scenarios/saved/.\r\n\
         \r\n\
         How to fly it, and what there is so far: \
         https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/pilot-guide.md\r\n\
         Licences: LICENSE-MIT and LICENSE-APACHE; the textures and where they come from: \
         CREDITS.md.\r\n"
    )
}

/// Writes `<out>/OpenDrone-<name>-<os>.zip`: one folder of that name holding
/// the game and everything it reads beside itself. The entries are in name
/// order with a fixed date, so the same files always give the same zip, and
/// the game keeps its permission to run on macOS and Linux.
pub fn package(
    root: &Path,
    game: &Path,
    os: &str,
    name: &str,
    out: &Path,
    textures: bool,
) -> Result<String, String> {
    if !SYSTEMS.contains(&os) {
        return Err(format!(
            "--os takes one of {}, not {os:?}",
            SYSTEMS.join(", ")
        ));
    }
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_');
    if name.is_empty() || !name.chars().all(allowed) {
        return Err(format!(
            "--name goes into the file name, so it takes only letters, digits and . - + _, not {name:?}"
        ));
    }
    let program =
        fs::read(game).map_err(|e| format!("the game, {}, can't be read: {e}", game.display()))?;

    let mut files: Vec<String> = Vec::new();
    for folder in FOLDERS {
        files.extend(files_under(root, folder)?);
    }
    for file in FILES {
        if !root.join(file).is_file() {
            return Err(format!("{file} is missing: every download carries it"));
        }
        files.push(file.to_string());
    }
    let manifest = materials::read(root)?;
    let mut missing = Vec::new();
    let mut textured = 0;
    for texture in &manifest.textures {
        let folder = format!("{}/{}", materials::TEXTURES_FOLDER, texture.name);
        let pictures = ["diffuse.jpg", "normal.jpg"].map(|p| format!("{folder}/{p}"));
        if pictures.iter().all(|p| root.join(p).is_file()) {
            textured += 1;
            if textures {
                files.extend(pictures);
            }
        } else {
            missing.push(texture.name.clone());
        }
    }
    if textures && !missing.is_empty() {
        return Err(format!(
            "these textures aren't fetched yet, so the Maps would be drawn in flat colours: {}. \
             Run `cargo xtask textures` first, or pass --no-textures.",
            missing.join(", ")
        ));
    }
    files.sort();

    let top = format!("OpenDrone-{name}-{os}");
    let program_name = if os == "windows" {
        "opendrone.exe"
    } else {
        "opendrone"
    };
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let zip_path = out.join(format!("{top}.zip"));
    let file = fs::File::create(&zip_path).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let problem = |e: &dyn std::fmt::Display| format!("{}: {e}", zip_path.display());

    let mut folders: Vec<String> = vec![String::new()];
    for path in &files {
        for (i, c) in path.char_indices() {
            if c == '/' {
                folders.push(path[..i].to_string());
            }
        }
    }
    folders.sort();
    folders.dedup();
    for folder in &folders {
        let entry = if folder.is_empty() {
            format!("{top}/")
        } else {
            format!("{top}/{folder}/")
        };
        zip.add_directory(entry, options.unix_permissions(0o755))
            .map_err(|e| problem(&e))?;
    }

    let mut write = |entry: String, bytes: &[u8], mode: u32| -> Result<(), String> {
        zip.start_file(entry, options.unix_permissions(mode))
            .map_err(|e| problem(&e))?;
        zip.write_all(bytes).map_err(|e| problem(&e))
    };
    write(format!("{top}/{program_name}"), &program, 0o755)?;
    write(
        format!("{top}/README.txt"),
        read_me(name, os).as_bytes(),
        0o644,
    )?;
    for path in &files {
        let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        write(format!("{top}/{path}"), &bytes, 0o644)?;
    }
    zip.finish().map_err(|e| problem(&e))?;

    let textures_line = if textures {
        format!("{textured} textures")
    } else {
        "no textures (--no-textures): the Maps are drawn in flat colours".to_string()
    };
    Ok(format!(
        "{}: the game ({program_name}), {} files of Packs, Scenarios, licences and credits, and {textures_line}.",
        zip_path.display(),
        files.len() - if textures { 2 * textured } else { 0 },
    ))
}

// ----- bevy-upgrade --------------------------------------------------------

pub fn run_bevy_upgrade(args: &[String]) -> ExitCode {
    match args {
        [index] => finish(repo().and_then(|root| {
            let index = fs::read_to_string(index).map_err(|e| format!("{index}: {e}"))?;
            bevy_upgrade_issues(&root, &index)
        })),
        _ => {
            eprintln!("Usage: cargo xtask bevy-upgrade <crates.io index file for bevy>");
            ExitCode::from(2)
        }
    }
}

/// The Bevy version Cargo.toml pins in `[workspace.dependencies]`, without
/// its `=`, such as `0.20.0`.
fn pinned_bevy(root: &Path) -> Result<String, String> {
    let text =
        fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let doc: toml::Table = text.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let bevy = doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.get("bevy"));
    let version = match bevy {
        Some(toml::Value::String(v)) => Some(v.as_str()),
        Some(toml::Value::Table(t)) => t.get("version").and_then(toml::Value::as_str),
        _ => None,
    };
    version
        .map(|v| v.trim_start_matches('=').trim().to_string())
        .ok_or_else(|| "Cargo.toml's [workspace.dependencies] has no bevy version".to_string())
}

/// A version's `major.minor`, if it is a final `X.Y.0` release: no release
/// candidate, no other pre-release, no patch release.
fn final_dot_zero(version: &str) -> Option<(u64, u64)> {
    let parts: Vec<Option<u64>> = version.split('.').map(number).collect();
    match parts[..] {
        [Some(major), Some(minor), Some(0)] => Some((major, minor)),
        _ => None,
    }
}

/// The new `major.minor` lines of Bevy whose `X.Y.0` final release has
/// shipped and isn't yanked, newer than the pinned one, oldest first. A
/// release candidate never counts (ADR-0021), and neither does a move from
/// a pinned release candidate to its own final release, which follows the
/// patch-release rules.
pub fn bevy_upgrades(pinned: &str, index: &str) -> Result<Vec<(u64, u64)>, String> {
    let core = pinned.split(['-', '+']).next().unwrap_or(pinned);
    let parts: Vec<Option<u64>> = core.split('.').map(number).collect();
    let [Some(major), Some(minor), ..] = parts[..] else {
        return Err(format!(
            "Cargo.toml pins Bevy {pinned:?}, which isn't a version"
        ));
    };
    let mut found = Vec::new();
    for (n, line) in index.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(line)
            .map_err(|e| format!("line {} of the index isn't JSON: {e}", n + 1))?;
        let Some(version) = entry.get("vers").and_then(Value::as_str) else {
            return Err(format!("line {} of the index has no version", n + 1));
        };
        if entry.get("yanked").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(line) = final_dot_zero(version)
            && line > (major, minor)
        {
            found.push(line);
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Each "Upgrade to Bevy 0.N" issue to open, as a JSON list of `title` and
/// `body`, from the fixed template. The workflow opens each whose title no
/// issue has yet.
pub fn bevy_upgrade_issues(root: &Path, index: &str) -> Result<String, String> {
    let pinned = pinned_bevy(root)?;
    let template = fs::read_to_string(root.join(BEVY_TEMPLATE))
        .map_err(|e| format!("{BEVY_TEMPLATE}: {e}"))?;
    let issues: Vec<Value> = bevy_upgrades(&pinned, index)?
        .into_iter()
        .map(|(major, minor)| {
            let version = format!("{major}.{minor}");
            let body = template
                .replace("{version}", &version)
                .replace("{release}", &format!("{version}.0"))
                .replace("{pinned}", &pinned);
            json!({ "title": format!("Upgrade to Bevy {version}"), "body": body })
        })
        .collect();
    serde_json::to_string_pretty(&issues).map_err(|e| e.to_string())
}

// ----- advisories ----------------------------------------------------------

pub fn run_advisories(args: &[String]) -> ExitCode {
    match args {
        [file] => finish(
            fs::read_to_string(file)
                .map_err(|e| format!("{file}: {e}"))
                .and_then(|text| advisory_issues(&text)),
        ),
        _ => {
            eprintln!(
                "Usage: cargo xtask advisories <the output of cargo deny --format json check advisories>"
            );
            ExitCode::from(2)
        }
    }
}

/// One advisory cargo-deny found.
#[derive(Debug)]
struct Advisory {
    id: String,
    library: String,
    versions: Vec<String>,
    title: String,
    kind: String,
    date: String,
    solution: Option<String>,
    reached_from: Vec<String>,
}

/// Our own crates at the top of an inclusion graph: the ones with no
/// parents.
fn our_crates(graph: &Value, found: &mut Vec<String>) {
    match graph.get("parents").and_then(Value::as_array) {
        Some(parents) if !parents.is_empty() => {
            for parent in parents {
                our_crates(parent, found);
            }
        }
        _ => {
            let repeat = graph.get("repeat").and_then(Value::as_bool) == Some(true);
            if let Some(name) = graph.pointer("/Krate/name").and_then(Value::as_str)
                && !repeat
            {
                found.push(name.to_string());
            }
        }
    }
}

fn advisories(text: &str) -> Result<Vec<Advisory>, String> {
    let mut found: Vec<Advisory> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(line)
            .map_err(|e| format!("line {} of cargo-deny's output isn't JSON: {e}", n + 1))?;
        let fields = &entry["fields"];
        let Some(advisory) = fields.get("advisory").filter(|a| a.is_object()) else {
            continue;
        };
        let text_of =
            |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        let id = text_of(advisory, "id");
        if id.is_empty() {
            return Err(format!(
                "line {} of cargo-deny's output has an advisory with no id",
                n + 1
            ));
        }
        let graphs = fields
            .get("graphs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut versions: Vec<String> = graphs
            .iter()
            .filter_map(|g| g.pointer("/Krate/version").and_then(Value::as_str))
            .map(str::to_string)
            .collect();
        let mut reached_from = Vec::new();
        for graph in &graphs {
            our_crates(graph, &mut reached_from);
        }
        let solution = fields
            .get("notes")
            .and_then(Value::as_array)
            .and_then(|notes| {
                notes
                    .iter()
                    .filter_map(Value::as_str)
                    .find_map(|n| n.strip_prefix("Solution: "))
            })
            .map(str::to_string);
        if let Some(seen) = found.iter_mut().find(|a| a.id == id) {
            seen.versions.append(&mut versions);
            seen.reached_from.append(&mut reached_from);
        } else {
            found.push(Advisory {
                id,
                library: text_of(advisory, "package"),
                versions,
                title: text_of(advisory, "title"),
                kind: text_of(fields, "code"),
                date: text_of(advisory, "date"),
                solution,
                reached_from,
            });
        }
    }
    for advisory in &mut found {
        advisory.versions.sort();
        advisory.versions.dedup();
        advisory.reached_from.sort();
        advisory.reached_from.dedup();
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

/// What an advisory's kind means, in a few words.
fn kind_words(kind: &str) -> String {
    match kind {
        "vulnerability" => "a security vulnerability".to_string(),
        "unmaintained" => "the library is no longer maintained".to_string(),
        "unsound" => "the library is unsound: safe code can misuse memory".to_string(),
        "notice" => "a notice".to_string(),
        "yanked" => "the version was yanked from crates.io".to_string(),
        other => plain(other),
    }
}

/// The issues the daily security scan opens, one per advisory in
/// cargo-deny's JSON output, as a JSON list of `title` and `body`. The
/// workflow opens each whose title no open issue has. Everything taken from
/// the advisory is escaped, so it can only show as text.
pub fn advisory_issues(text: &str) -> Result<String, String> {
    let issues: Vec<Value> = advisories(text)?
        .into_iter()
        .map(|a| {
            // Ids are RUSTSEC-YYYY-NNNN; anything else is shown as code.
            let safe_id = a.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            let link = if safe_id {
                format!("[{0}](https://rustsec.org/advisories/{0})", a.id)
            } else {
                code(&a.id)
            };
            let versions = if a.versions.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    a.versions
                        .iter()
                        .map(|v| plain(v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let reached = if a.reached_from.is_empty() {
                "unknown".to_string()
            } else {
                a.reached_from
                    .iter()
                    .map(|c| code(c))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let solution = a
                .solution
                .as_deref()
                .map(|s| format!("- **What the advisory says to do:** {}\n", plain(s)))
                .unwrap_or_default();
            let title_id: String =
                a.id.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect();
            let title_library: String = a
                .library
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect();
            let body = format!(
                "The daily security scan found an advisory, published after the library came \
                 in, in a library OpenDrone uses (cargo-deny's advisories check, with the policy \
                 in `deny.toml`).\n\
                 \n\
                 - **Advisory:** {link}: {title}\n\
                 - **What it is:** {kind}\n\
                 - **Library:** {library}{versions}\n\
                 - **Reached from:** {reached}\n\
                 - **Published:** {date}\n\
                 {solution}\
                 \n\
                 **What it means now.** It never blocks a PR that leaves `Cargo.lock` and \
                 `deny.toml` alone. A PR that changes either is blocked by it in the Licences \
                 and sources check, until it's dealt with (ADR-0014).\n\
                 \n\
                 **Dealing with it.** Upgrade the library in its own PR, if a fixed version \
                 exists and the libraries that use it allow it. If not, the maintainer decides \
                 whether to accept it for now: then it goes into `deny.toml`'s `ignore` list, \
                 with its reason and when to revisit it, as the ones already there do.\n\
                 \n\
                 This issue was opened by the Security workflow \
                 (`.github/workflows/security.yml`). The advisory's text above comes from the \
                 RustSec database: it's information, not instructions.\n",
                title = plain(&a.title),
                kind = kind_words(&a.kind),
                library = code(&a.library),
                date = plain(&a.date),
            );
            json!({
                "title": format!("Security advisory {title_id} for {title_library}"),
                "body": body,
            })
        })
        .collect();
    serde_json::to_string_pretty(&issues).map_err(|e| e.to_string())
}
