//! Builds, releases and updates (#81, #15 §8, ADR-0021): the parts of the
//! Build, Release, Bevy upgrade and Security workflows worth a readable
//! check. The workflows themselves only fetch, build and talk to GitHub.
//!
//! - `release-version <tag>`: the version a tag such as `v0.1.0-alpha.1`
//!   releases, or why it isn't a release tag.
//! - `package`: lays out a built game as a download, one per operating
//!   system, each an app a pilot double-clicks (#297): `OpenDrone.app` in a
//!   `.dmg` (or a zip), `OpenDrone.exe` in a zip, and `opendrone` in a
//!   tarball, with the game's files where it looks for them
//!   (`opendrone::set_up::find_repo_folder`). Packs and Scenarios come from
//!   the files git tracks, so data downloaded onto this machine (NeuroBEM's,
//!   which has no licence to redistribute) never goes in. The libraries'
//!   licence texts come from `licences/third-party-<os>.txt`, which
//!   `cargo xtask licences` writes and CI keeps up to date (#270).
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
use zip::{CompressionMethod, System, ZipWriter};

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

/// The operating systems a build is made for, by the name in its download.
const SYSTEMS: [&str; 3] = ["macos", "windows", "linux"];

/// What a download holds besides the game itself, from the repo's root. The
/// game finds `packs/` and `scenarios/` in its own folder, and the material
/// library and its textures under `assets-src/`, as in the repo: beside the
/// program on Windows and Linux, in the app's `Contents/Resources/` on macOS
/// (`opendrone::set_up::find_repo_folder`).
const FOLDERS: [&str; 2] = ["packs", "scenarios"];
const FILES: [&str; 4] = [
    materials::MANIFEST,
    "CREDITS.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
];

/// The name every download gives the libraries' licence texts, which come
/// from `crate::licences::file_for(os)`.
const THIRD_PARTY_LICENCES: &str = "THIRD-PARTY-LICENSES.txt";

/// The licences and credits, which also sit beside the macOS app, where a
/// pilot sees them, as well as inside it, where they travel with it.
const LICENCE_FILES: [&str; 4] = [
    "CREDITS.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    THIRD_PARTY_LICENCES,
];

/// The macOS app, from the download's top.
const APP: &str = "OpenDrone.app/Contents";

/// The macOS app's id. It names the app to macOS, so it never changes.
const BUNDLE_ID: &str = "io.github.bartoszsolkabd.opendrone";

/// The Linux download's script that adds the game to the desktop's app menu.
const MENU_SCRIPT: &str = "add-to-app-menu.sh";

pub fn run_package(args: &[String]) -> ExitCode {
    const USAGE: &str = "Usage: cargo xtask package --game <built program> --os <macos|windows|linux> \
                         --name <version or build name> [--out <folder>] [--no-textures] \
                         [--dmg [--sign <identity>]]";
    let mut game = None;
    let mut os = None;
    let mut name = None;
    let mut out = PathBuf::from("target/dist");
    let mut options = Options {
        textures: true,
        dmg: false,
        sign: None,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let given = match arg.as_str() {
            "--game" => rest.next().map(|v| game = Some(PathBuf::from(v))),
            "--os" => rest.next().map(|v| os = Some(v.clone())),
            "--name" => rest.next().map(|v| name = Some(v.clone())),
            "--out" => rest.next().map(|v| out = PathBuf::from(v)),
            "--sign" => rest.next().map(|v| options.sign = Some(v.clone())),
            "--no-textures" => {
                options.textures = false;
                Some(())
            }
            "--dmg" => {
                options.dmg = true;
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
    finish(repo().and_then(|root| package(&root, &game, &os, &name, &out, &options)))
}

/// How `package` makes a download, beyond what it holds.
pub struct Options {
    /// Whether the Maps' textures go in. Without them the Maps are drawn in
    /// flat colours.
    pub textures: bool,
    /// macOS only, on a Mac: the app is signed and put in a `.dmg`, instead
    /// of a zip.
    pub dmg: bool,
    /// The identity `codesign` signs the app with, such as "Developer ID
    /// Application: …". Without one, it's signed ad hoc (`-`), which needs
    /// no certificate: Apple Silicon runs only signed programs.
    pub sign: Option<String>,
}

/// Every file git tracks under `folder`, as paths from the repo's root with
/// `/` between folders, in name order. Only committed files go into a
/// download, never whatever else sits in the folder on this machine:
/// NeuroBEM's data, which `cargo xtask download neurobem` writes beside its
/// Scenarios, has no licence to be redistributed and is kept out of git by a
/// `.gitignore` (#251). Hidden files and folders, such as `.gitignore`, are
/// left out, and so is anything that isn't a plain file.
fn files_under(root: &Path, folder: &str) -> Result<Vec<String>, String> {
    if !root.join(".git").exists() {
        return Err(format!(
            "{} isn't a git checkout, and a download takes only the files git tracks, so \
             downloaded data can't slip in. Run this inside a clone of the OpenDrone repo.",
            root.display()
        ));
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--", folder])
        .output()
        .map_err(|e| format!("can't run git to list {folder}/: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git couldn't list {folder}/: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let listed = String::from_utf8(output.stdout)
        .map_err(|_| format!("git listed a file under {folder}/ whose name isn't text"))?;
    let mut found = Vec::new();
    for path in listed.split('\0').filter(|p| !p.is_empty()) {
        if path.split('/').any(|part| part.starts_with('.')) {
            continue;
        }
        let kind = fs::symlink_metadata(root.join(path))
            .map_err(|e| format!("{path}: git tracks it, but {e}"))?;
        if kind.is_file() {
            found.push(path.to_string());
        }
    }
    if found.is_empty() {
        return Err(format!("git tracks no files under {folder}/"));
    }
    found.sort();
    Ok(found)
}

/// The short note at the top of every download, with Windows' line ends,
/// which every system's text editor reads.
fn read_me(name: &str, os: &str) -> String {
    let (start, keep) = match os {
        "macos" => (
            "Double-click OpenDrone. To keep it, drag OpenDrone onto Applications first, \
             then open it from there.\r\n\
             \r\n\
             The build isn't signed by a registered developer yet, so the first time, macOS \
             says it could not verify that OpenDrone is free of malware, and offers only Done \
             or Move to Trash: click Done. Then open System Settings, then Privacy & Security, \
             scroll down to Security, and click Open Anyway beside \"OpenDrone was blocked\". \
             Confirm with Open Anyway and your password. From then on it opens with a \
             double-click. (On macOS 14 or older: right-click OpenDrone, choose Open, then \
             Open.)",
            "Everything the game reads is inside OpenDrone.app. It keeps your settings, and \
             the flights you save, in ~/Library/Application Support/OpenDrone/ (saved flights \
             in its saved-flights/ folder), never inside the app.",
        ),
        "windows" => (
            "First take the files out of the zip: right-click it, choose Extract All, then \
             Extract. Then, in the folder it makes, double-click OpenDrone.exe. (Opened \
             straight from inside the zip, Windows copies out only the program, and it can't \
             find its Maps.)\r\n\
             \r\n\
             The build isn't signed yet, so the first time, Windows SmartScreen may say \
             \"Windows protected your PC\": click More info, then Run anyway.",
            "Keep this folder together: the game reads its Packs (packs\\), the Scenarios \
             (scenarios\\) and the Maps' textures (assets-src\\) from beside OpenDrone.exe. It \
             keeps your settings, and the flights you save, in %APPDATA%\\OpenDrone\\ (saved \
             flights in its saved-flights\\ folder).",
        ),
        _ => (
            "Double-click opendrone, or run ./opendrone in this folder. It needs a Vulkan \
             driver and the system's sound library (alsa-lib), which most desktops have.\r\n\
             \r\n\
             To add OpenDrone to your desktop's app menu, with its icon, run \
             ./add-to-app-menu.sh in this folder once (./add-to-app-menu.sh --remove takes it \
             out again). It points the menu at this folder, so run it again if you move the \
             folder.",
            "Keep this folder together: the game reads its Packs (packs/), the Scenarios \
             (scenarios/) and the Maps' textures (assets-src/) from beside itself. It keeps \
             your settings, and the flights you save, in ~/.config/opendrone/ (saved flights \
             in its saved-flights/ folder).",
        ),
    };
    format!(
        "OpenDrone {name}, a test build of the FPV simulator.\r\n\
         \r\n\
         Starting it: {start}\r\n\
         \r\n\
         {keep}\r\n\
         \r\n\
         How to fly it, and what there is so far: \
         https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/pilot-guide.md\r\n\
         How to open a build on each system: \
         https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/builds-and-releases.md\r\n\
         Licences: LICENSE-MIT and LICENSE-APACHE for OpenDrone; THIRD-PARTY-LICENSES.txt for \
         the libraries built into the game; the textures and where they come from: \
         CREDITS.md.\r\n"
    )
}

/// The macOS app's description of itself, which macOS reads to start it:
/// its name, program, icon and version. `version` is three whole numbers.
fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>OpenDrone</string>
	<key>CFBundleExecutable</key>
	<string>OpenDrone</string>
	<key>CFBundleIconFile</key>
	<string>OpenDrone</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>OpenDrone</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.simulation-games</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// The version macOS shows for the app: three whole numbers, as it wants
/// them. A release's own version (`0.1.0-alpha.1` gives `0.1.0`), or else the
/// version in the repo's `Cargo.toml`, for a build of main or a PR.
fn app_version(root: &Path, name: &str) -> Result<String, String> {
    let version = match release_version(&format!("v{name}")) {
        Ok(version) => version,
        Err(_) => {
            let text = fs::read_to_string(root.join("Cargo.toml"))
                .map_err(|e| format!("Cargo.toml: {e}"))?;
            let doc: toml::Table = text.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
            doc.get("workspace")
                .and_then(|w| w.get("package"))
                .and_then(|p| p.get("version"))
                .and_then(toml::Value::as_str)
                .ok_or("Cargo.toml's [workspace.package] has no version")?
                .to_string()
        }
    };
    let core = version.split(['-', '+']).next().unwrap_or(&version);
    match core.split('.').map(number).collect::<Vec<_>>()[..] {
        [Some(_), Some(_), Some(_)] => Ok(core.to_string()),
        _ => Err(format!(
            "the version {version:?} doesn't start with three whole numbers, as macOS wants"
        )),
    }
}

/// The script that adds the Linux build to the desktop's app menu: a desktop
/// entry, the file every Linux desktop reads its menu from, written for this
/// user only, pointing at the folder the script is in. A desktop entry needs
/// the program's full path, which only the pilot's computer knows, so the
/// download carries this script rather than the entry itself.
fn menu_script() -> String {
    format!(
        r#"#!/bin/sh
# Adds OpenDrone to your desktop's app menu, with its icon, for you only:
# it writes a desktop entry pointing at this folder. Run it again if you move
# the folder. ./{MENU_SCRIPT} --remove takes the entry out again.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
menu="${{XDG_DATA_HOME:-$HOME/.local/share}}/applications"
entry="$menu/opendrone.desktop"
if [ "${{1:-}}" = "--remove" ]; then
  rm -f "$entry"
  echo "OpenDrone is out of the app menu."
  exit 0
fi
case "$here" in
  *'"'*|*'\'*|*'`'*|*'$'*|*'%'*)
    echo "This folder's path holds a character a desktop entry can't name: move the folder, then run this again." >&2
    exit 1
    ;;
esac
mkdir -p "$menu"
cat > "$entry" <<ENTRY
[Desktop Entry]
Type=Application
Name=OpenDrone
Comment=An open-source FPV flight simulator
Exec="$here/opendrone"
Path=$here
Icon=$here/opendrone.png
Terminal=false
Categories=Game;Simulation;
ENTRY
echo "OpenDrone is in the app menu: $entry"
"#
    )
}

/// One file of a download: where it goes, from the download's top folder,
/// with `/` between folders, its bytes, and its Unix permissions.
struct Item {
    path: String,
    bytes: Vec<u8>,
    mode: u32,
}

impl Item {
    fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>, mode: u32) -> Item {
        Item {
            path: path.into(),
            bytes: bytes.into(),
            mode,
        }
    }
}

/// Every folder the items sit in, the top one (`""`) first, then in name
/// order.
fn folders_of(items: &[Item]) -> Vec<String> {
    let mut folders: Vec<String> = vec![String::new()];
    for item in items {
        for (i, c) in item.path.char_indices() {
            if c == '/' {
                folders.push(item.path[..i].to_string());
            }
        }
    }
    folders.sort();
    folders.dedup();
    folders
}

/// Lays a built game out as a download, in `out`, and says what it wrote:
///
/// - **macOS:** `OpenDrone.app`, with the game's files in its
///   `Contents/Resources/` and the licences and a README beside it, in
///   `OpenDrone-<name>-macos.zip`; or, with `--dmg` on a Mac, signed and in
///   `OpenDrone-<name>-macos.dmg`, with a link to Applications to drag it
///   onto;
/// - **Windows:** `OpenDrone.exe` with the game's files beside it, in one
///   folder in `OpenDrone-<name>-windows.zip`;
/// - **Linux:** `opendrone` with the game's files, its icon and the script
///   that adds it to the app menu beside it, in one folder in
///   `OpenDrone-<name>-linux.tar.gz`.
///
/// A zip or tarball has its entries in name order with a fixed date, so the
/// same files always give the same download, and the programs keep their
/// permission to run on macOS and Linux.
pub fn package(
    root: &Path,
    game: &Path,
    os: &str,
    name: &str,
    out: &Path,
    options: &Options,
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
    if (options.dmg || options.sign.is_some()) && os != "macos" {
        return Err("--dmg and --sign are for the macOS app: they go with --os macos".into());
    }
    if options.sign.is_some() && !options.dmg {
        return Err("--sign signs the app on its way into the .dmg: it goes with --dmg".into());
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
    let third_party = crate::licences::file_for(os);
    let third_party_text = fs::read(root.join(&third_party)).map_err(|e| {
        format!(
            "{third_party} can't be read ({e}): every download carries the libraries' licence \
             texts. Run `cargo xtask licences` and commit it."
        )
    })?;
    let manifest = materials::read(root)?;
    let mut missing = Vec::new();
    let mut textured = 0;
    for texture in &manifest.textures {
        let folder = format!("{}/{}", materials::TEXTURES_FOLDER, texture.name);
        let pictures = ["diffuse.jpg", "normal.jpg"].map(|p| format!("{folder}/{p}"));
        if pictures.iter().all(|p| root.join(p).is_file()) {
            textured += 1;
            if options.textures {
                files.extend(pictures);
            }
        } else {
            missing.push(texture.name.clone());
        }
    }
    if options.textures && !missing.is_empty() {
        return Err(format!(
            "these textures aren't fetched yet, so the Maps would be drawn in flat colours: {}. \
             Run `cargo xtask textures` first, or pass --no-textures.",
            missing.join(", ")
        ));
    }
    let icon = |path: &str| {
        fs::read(root.join(path))
            .map_err(|e| format!("{path} can't be read ({e}): run `cargo xtask icon`"))
    };

    // The game's own files, where it reads them from: beside the program, or
    // in the macOS app's Resources.
    let data = if os == "macos" {
        format!("{APP}/Resources/")
    } else {
        String::new()
    };
    let mut items = Vec::new();
    for path in &files {
        let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        items.push(Item::new(format!("{data}{path}"), bytes, 0o644));
    }
    items.push(Item::new(
        format!("{data}{THIRD_PARTY_LICENCES}"),
        third_party_text.clone(),
        0o644,
    ));
    items.push(Item::new("README.txt", read_me(name, os), 0o644));
    let program_name = match os {
        "macos" => {
            items.push(Item::new(
                format!("{APP}/Info.plist"),
                info_plist(&app_version(root, name)?),
                0o644,
            ));
            items.push(Item::new(
                format!("{APP}/Resources/OpenDrone.icns"),
                icon(crate::icon::ICNS)?,
                0o644,
            ));
            for file in LICENCE_FILES {
                let bytes = if file == THIRD_PARTY_LICENCES {
                    third_party_text.clone()
                } else {
                    fs::read(root.join(file)).map_err(|e| format!("{file}: {e}"))?
                };
                items.push(Item::new(file, bytes, 0o644));
            }
            format!("{APP}/MacOS/OpenDrone")
        }
        "windows" => "OpenDrone.exe".to_string(),
        _ => {
            items.push(Item::new("opendrone.png", icon(crate::icon::PNG)?, 0o644));
            items.push(Item::new(MENU_SCRIPT, menu_script(), 0o755));
            "opendrone".to_string()
        }
    };
    items.push(Item::new(program_name.clone(), program, 0o755));
    items.sort_by(|a, b| a.path.cmp(&b.path));

    let top = format!("OpenDrone-{name}-{os}");
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let written = match os {
        "macos" if options.dmg => {
            write_dmg(out, &top, &items, options.sign.as_deref().unwrap_or("-"))?
        }
        "linux" => write_tarball(&out.join(format!("{top}.tar.gz")), &top, &items)?,
        _ => write_zip(&out.join(format!("{top}.zip")), &top, &items)?,
    };

    let textures_line = if options.textures {
        format!("{textured} textures")
    } else {
        "no textures (--no-textures): the Maps are drawn in flat colours".to_string()
    };
    Ok(format!(
        "{}: the game ({program_name}), {} files of Packs, Scenarios, licences and credits, and {textures_line}.",
        written.display(),
        files.len() + 1 - if options.textures { 2 * textured } else { 0 },
    ))
}

/// Writes the items into one folder, `top`, in a zip.
fn write_zip(path: &Path, top: &str, items: &[Item]) -> Result<PathBuf, String> {
    let file = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    // Unix permissions whichever system makes the zip, so a Windows-made zip
    // still lets the game run on macOS and Linux.
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .system(System::Unix);
    let problem = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    for folder in folders_of(items) {
        let entry = if folder.is_empty() {
            format!("{top}/")
        } else {
            format!("{top}/{folder}/")
        };
        zip.add_directory(entry, options.unix_permissions(0o755))
            .map_err(|e| problem(&e))?;
    }
    for item in items {
        zip.start_file(
            format!("{top}/{}", item.path),
            options.unix_permissions(item.mode),
        )
        .map_err(|e| problem(&e))?;
        zip.write_all(&item.bytes).map_err(|e| problem(&e))?;
    }
    zip.finish().map_err(|e| problem(&e))?;
    Ok(path.to_path_buf())
}

/// The fixed date every tarball entry carries: 1 January 1980, as a zip's
/// entries do, in seconds since 1970.
const TAR_DATE: u64 = 315_532_800;

/// One 512-byte tar header, in the POSIX (ustar) layout every `tar` reads:
/// a path of up to 100 bytes, or up to 255 split at a `/` into a prefix and
/// a name. `kind` is `b'0'` for a file and `b'5'` for a folder.
fn tar_header(path: &str, size: u64, mode: u32, kind: u8) -> Result<[u8; 512], String> {
    let mut header = [0_u8; 512];
    let (prefix, name) = if path.len() <= 100 {
        ("", path)
    } else {
        let split = path
            .char_indices()
            .filter(|&(i, c)| c == '/' && i <= 155 && path.len() - i - 1 <= 100)
            .map(|(i, _)| i)
            .next()
            .ok_or_else(|| format!("{path} is too long a path for a tarball"))?;
        (&path[..split], &path[split + 1..])
    };
    let mut put = |at: usize, text: &[u8]| header[at..at + text.len()].copy_from_slice(text);
    put(0, name.as_bytes());
    put(100, format!("{mode:07o}\0").as_bytes());
    put(108, b"0000000\0");
    put(116, b"0000000\0");
    put(124, format!("{size:011o}\0").as_bytes());
    put(136, format!("{TAR_DATE:011o}\0").as_bytes());
    put(148, b"        ");
    put(156, &[kind]);
    put(257, b"ustar\0");
    put(263, b"00");
    put(345, prefix.as_bytes());
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    Ok(header)
}

/// Writes the items into one folder, `top`, in a gzipped tarball, the way a
/// Linux download usually comes.
fn write_tarball(path: &Path, top: &str, items: &[Item]) -> Result<PathBuf, String> {
    let mut tar = Vec::new();
    for folder in folders_of(items) {
        let entry = if folder.is_empty() {
            format!("{top}/")
        } else {
            format!("{top}/{folder}/")
        };
        tar.extend(tar_header(&entry, 0, 0o755, b'5')?);
    }
    for item in items {
        let entry = format!("{top}/{}", item.path);
        tar.extend(tar_header(
            &entry,
            item.bytes.len() as u64,
            item.mode,
            b'0',
        )?);
        tar.extend(&item.bytes);
        tar.resize(tar.len().div_ceil(512) * 512, 0);
    }
    // Two empty blocks end a tarball.
    tar.resize(tar.len() + 1024, 0);
    let file = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut gzip = flate2::GzBuilder::new()
        .mtime(0)
        .write(std::io::BufWriter::new(file), flate2::Compression::best());
    let problem = |e: std::io::Error| format!("{}: {e}", path.display());
    gzip.write_all(&tar).map_err(problem)?;
    gzip.finish().map_err(problem)?.flush().map_err(problem)?;
    Ok(path.to_path_buf())
}

/// Runs a macOS tool, and says what it said if it fails.
#[cfg(target_os = "macos")]
fn run_tool(tool: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = std::process::Command::new(tool)
        .args(args)
        .output()
        .map_err(|e| format!("can't run {tool}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{tool} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Lays the items out in `out/<top>/`, signs the app with `identity` (`-`
/// for ad hoc), checks the signature, and puts the folder, with a link to
/// Applications, into `out/<top>.dmg`, a disk image macOS opens with a
/// double-click. Both tools, `codesign` and `hdiutil`, come with macOS.
#[cfg(target_os = "macos")]
fn write_dmg(out: &Path, top: &str, items: &[Item], identity: &str) -> Result<PathBuf, String> {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;

    let stage = out.join(top);
    let problem = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| problem(&stage, e))?;
    }
    for item in items {
        let path = stage.join(&item.path);
        let folder = path.parent().expect("inside the stage");
        fs::create_dir_all(folder).map_err(|e| problem(folder, e))?;
        fs::write(&path, &item.bytes).map_err(|e| problem(&path, e))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(item.mode))
            .map_err(|e| problem(&path, e))?;
    }
    let applications = stage.join("Applications");
    std::os::unix::fs::symlink("/Applications", &applications)
        .map_err(|e| problem(&applications, e))?;

    let app = stage.join("OpenDrone.app");
    let mut sign: Vec<&OsStr> = ["--force", "--sign", identity].map(OsStr::new).to_vec();
    if identity == "-" {
        sign.push(OsStr::new("--timestamp=none"));
    } else {
        // What notarisation asks for: the hardened runtime and Apple's
        // timestamp.
        sign.extend(["--options", "runtime", "--timestamp"].map(OsStr::new));
    }
    sign.push(app.as_os_str());
    run_tool("codesign", &sign)?;
    run_tool(
        "codesign",
        &[
            OsStr::new("--verify"),
            OsStr::new("--strict"),
            app.as_os_str(),
        ],
    )?;

    let dmg = out.join(format!("{top}.dmg"));
    let args: Vec<&OsStr> = vec![
        OsStr::new("create"),
        OsStr::new("-volname"),
        OsStr::new("OpenDrone"),
        OsStr::new("-srcfolder"),
        stage.as_os_str(),
        OsStr::new("-fs"),
        OsStr::new("HFS+"),
        OsStr::new("-format"),
        OsStr::new("UDZO"),
        OsStr::new("-ov"),
        dmg.as_os_str(),
    ];
    // hdiutil sometimes finds the disk busy on a shared machine, such as
    // GitHub's: a second or third try works.
    let mut made = run_tool("hdiutil", &args);
    for _ in 0..2 {
        if made.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
        made = run_tool("hdiutil", &args);
    }
    made?;
    fs::remove_dir_all(&stage).map_err(|e| problem(&stage, e))?;
    Ok(dmg)
}

#[cfg(not(target_os = "macos"))]
fn write_dmg(_: &Path, _: &str, _: &[Item], _: &str) -> Result<PathBuf, String> {
    Err(
        "--dmg needs a Mac: it signs the app with codesign and makes the disk image with \
         hdiutil, which come only with macOS. Without --dmg, the app goes in a zip."
            .into(),
    )
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
