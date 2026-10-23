//! `cargo xtask play [--map <id>] [--no-launch]`: from a fresh checkout or
//! worktree, gets the game running with everything it needs (#298), and says
//! plainly what it did and what it skipped.
//!
//! In order:
//!
//! 1. **The toolchain.** The Rust the repo pins (`rust-toolchain.toml`) and
//!    CMake, which builds SDL.
//! 2. **The textures.** `cargo xtask textures`: pictures already there with
//!    the right checksum aren't fetched again. With no network the game
//!    starts all the same, drawing flat colours.
//! 3. **The Maps.** A Map's `.glb` is rebuilt only when a script it's built
//!    from has been edited in this checkout and is newer than the `.glb`,
//!    and only with the pinned Blender. Otherwise the committed `.glb` is
//!    used, which CI has checked is what the scripts build. So a fresh
//!    checkout never changes a tracked file: only a script you edited makes
//!    its own `.glb` change, the file you would commit with it.
//! 4. **The game,** built and started in release mode, on the Map named if
//!    `--map` names one.
//!
//! `--no-launch` stops before step 4, for a check that everything is ready.
//! Each step that can't go ahead says why in one sentence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::SystemTime;

use crate::{maps, materials, textures};

const USAGE: &str = "Usage: cargo xtask play [--map <id>] [--no-launch]";

/// The Test Map that flies at once, which has no folder in a Pack.
const TEST_MAPS: [&str; 1] = ["flat-floor"];

struct Options {
    map: Option<String>,
    no_launch: bool,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        map: None,
        no_launch: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-launch" => options.no_launch = true,
            "--map" => match args.next() {
                Some(id) => options.map = Some(id.clone()),
                None => return Err("--map takes the id of a Map, such as skate-park".into()),
            },
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(options)
}

pub fn run(args: &[String]) -> ExitCode {
    let options = match options(args) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("{problem}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = match materials::repo() {
        Ok(root) => root,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    };
    match play(&root, &options) {
        Ok(code) => code,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

fn play(root: &Path, options: &Options) -> Result<ExitCode, String> {
    println!("Getting OpenDrone ready in {}", root.display());
    check_map_name(root, options)?;

    println!("1/4 Toolchain");
    toolchain(root)?;

    println!("2/4 Textures");
    println!("    {}", textures_step(root));

    println!("3/4 Maps");
    for line in maps_step(root) {
        println!("    {line}");
    }

    if options.no_launch {
        println!(
            "4/4 Game: not started, because of --no-launch. `cargo xtask play` builds and starts it."
        );
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "4/4 Game: building in release mode (the first build in a checkout takes several minutes; the next start reuses it), then starting{}.",
        options
            .map
            .as_ref()
            .map(|m| format!(" on {m}"))
            .unwrap_or_default()
    );
    let mut game = Command::new("cargo");
    game.args(["run", "--release", "-p", "opendrone", "--"])
        .current_dir(root);
    if let Some(map) = &options.map {
        game.args(["--map", map]);
    }
    let status = game
        .status()
        .map_err(|e| format!("Cargo can't be run to build the game: {e}. Install Rust from https://rustup.rs and start again."))?;
    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        // The build or the game said why, just above.
        Ok(ExitCode::from(
            status.code().unwrap_or(1).clamp(1, 255) as u8
        ))
    }
}

/// The Maps `--map` can name: the built-in Pack's, and the developer's Test Map.
fn known_maps(root: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("packs/opendrone/maps")) {
        for entry in entries.flatten() {
            if entry.path().join("map.toml").is_file()
                && let Some(name) = entry.file_name().to_str()
            {
                ids.push(name.to_string());
            }
        }
    }
    ids.extend(TEST_MAPS.iter().map(|m| (*m).to_string()));
    ids.sort();
    ids
}

fn check_map_name(root: &Path, options: &Options) -> Result<(), String> {
    let Some(map) = &options.map else {
        return Ok(());
    };
    let known = known_maps(root);
    if known.contains(map) {
        Ok(())
    } else {
        Err(format!(
            "There is no Map called {map}: the Maps are {}.",
            known.join(", ")
        ))
    }
}

// ---- 1. The toolchain ----

/// The channel `rust-toolchain.toml` pins, such as `1.99.1`.
fn pinned_rust(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    doc.get("toolchain")?
        .get("channel")?
        .as_str()
        .map(str::to_string)
}

/// Why the Rust on this computer isn't the pinned one, as one sentence, or
/// nothing if it is. `rustc_line` is the first line of `rustc --version`
/// (such as `rustc 1.99.1 (abc 2026-01-01)`), or why it couldn't be run.
fn rust_problem(pinned: &str, rustc_line: &Result<String, String>) -> Option<String> {
    match rustc_line {
        Err(_) => Some(format!(
            "Rust isn't installed here: get rustup from https://rustup.rs, and run this again, which installs the Rust {pinned} that rust-toolchain.toml pins."
        )),
        Ok(line) => {
            let have = line.split_whitespace().nth(1).unwrap_or("");
            if have == pinned {
                None
            } else {
                Some(format!(
                    "This repo pins Rust {pinned} (rust-toolchain.toml), but the Rust here is {have:?}: run `rustup show` in this folder to install the pinned one."
                ))
            }
        }
    }
}

fn first_line_of(program: &str, arg: &str, folder: &Path) -> Result<String, String> {
    let output = Command::new(program)
        .arg(arg)
        .current_dir(folder)
        .output()
        .map_err(|e| format!("can't run {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!("{program} {arg} failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string())
}

fn install_cmake_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "`brew install cmake`"
    } else if cfg!(windows) {
        "`winget install Kitware.CMake`"
    } else {
        "`sudo apt install cmake` (or your system's package for it)"
    }
}

fn toolchain(root: &Path) -> Result<(), String> {
    if let Some(pinned) = pinned_rust(root) {
        let line = first_line_of("rustc", "--version", root);
        if let Some(problem) = rust_problem(&pinned, &line) {
            return Err(problem);
        }
        println!("    Rust {pinned}, the version the repo pins.");
    } else {
        println!("    The repo pins no Rust version, so any will do.");
    }
    if first_line_of("cmake", "--version", root).is_err() {
        return Err(format!(
            "CMake isn't installed, and the game's build needs it to build SDL: install it with {}, and run this again.",
            install_cmake_hint()
        ));
    }
    println!("    CMake found.");
    Ok(())
}

// ---- 2. The textures ----

/// What the textures step did, as one line. It never stops the game: without
/// the pictures every material is its flat colour.
fn textures_step(root: &Path) -> String {
    let manifest = match materials::read(root) {
        Ok(manifest) => manifest,
        Err(problem) => return format!("Skipped: {problem}. The Maps are drawn in flat colours."),
    };
    match textures::fetch(root, &manifest, false) {
        Ok(report) => {
            let have = report.lines().filter(|l| l.starts_with("have")).count();
            let fetched = report.lines().filter(|l| l.starts_with("fetched")).count();
            if fetched == 0 {
                format!("All {have} pictures were already there: nothing fetched.")
            } else {
                format!("Fetched {fetched} pictures; {have} were already there.")
            }
        }
        Err(problem) => format!(
            "Skipped: the textures couldn't be fetched ({}), so the Maps are drawn in flat colours this time. Run it again with a network to get them.",
            problem.lines().next().unwrap_or("").trim_end_matches('.')
        ),
    }
}

// ---- 3. The Maps ----

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The files a Map is built from: its script, the shared helpers, and the
/// material library and Blender pin the scripts read.
fn map_inputs(root: &Path, id: &str) -> Vec<PathBuf> {
    let mut inputs = vec![
        root.join("assets-src/maps").join(id).join("build.py"),
        root.join(materials::MANIFEST),
        root.join(maps::BLENDER_VERSION_FILE),
    ];
    if let Ok(entries) = fs::read_dir(root.join("assets-src/lib")) {
        let mut helpers: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "py"))
            .collect();
        helpers.sort();
        inputs.extend(helpers);
    }
    inputs
}

/// Whether git says any of these files differs from what is committed. With
/// no git, or no repo, nothing is known to be edited.
fn edited(root: &Path, files: &[PathBuf]) -> bool {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--"])
        .args(files)
        .current_dir(root)
        .output();
    output.is_ok_and(|o| o.status.success() && !o.stdout.is_empty())
}

/// The Maps to rebuild: with a script edited here and newer than the `.glb`,
/// or with no `.glb` at all.
fn stale_maps(root: &Path) -> Vec<String> {
    let mut stale = Vec::new();
    for id in maps::map_ids(root).unwrap_or_default() {
        let glb = root
            .join("packs/opendrone/maps")
            .join(&id)
            .join(format!("{id}.glb"));
        let inputs = map_inputs(root, &id);
        let built_at = modified(&glb);
        let newest = inputs.iter().filter_map(|p| modified(p)).max();
        let missing = built_at.is_none();
        let older = matches!((built_at, newest), (Some(b), Some(n)) if n > b);
        if missing || (older && edited(root, &inputs)) {
            stale.push(id);
        }
    }
    stale
}

fn maps_step(root: &Path) -> Vec<String> {
    let ids = maps::map_ids(root).unwrap_or_default();
    let stale = stale_maps(root);
    if stale.is_empty() {
        return vec![format!(
            "The committed .glb files of {} are what the scripts build: used as they are.",
            if ids.is_empty() {
                "every Map".to_string()
            } else {
                ids.join(" and ")
            }
        )];
    }
    match maps::rebuild(root, &stale) {
        Ok(report) => report.lines().map(str::to_string).collect(),
        Err(problem) => vec![format!(
            "Skipped: {} has a script edited here, but Blender can't rebuild it ({}). The game uses the committed .glb as it is, without your change.",
            stale.join(" and "),
            problem.lines().next().unwrap_or("").trim_end_matches('.')
        )],
    }
}
