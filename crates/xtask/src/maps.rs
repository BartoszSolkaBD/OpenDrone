//! `cargo xtask maps`: builds the built-in Pack's Maps with Blender (#63).
//!
//! Each Map is a Blender script, `assets-src/maps/<id>/build.py`, which
//! writes the Map's `.glb` into the built-in Pack at
//! `packs/opendrone/maps/<id>/<id>.glb`. The `.glb` is a build output, like a
//! compiled program: committed, but never edited by hand.
//!
//! - **The version.** Blender is pinned in `assets-src/blender-version.toml`,
//!   the one file CI and local builds both read. The command runs the
//!   Blender it finds (the `BLENDER` variable, `blender` on the path, or
//!   where each operating system installs it) only if it is that version.
//! - **The same bytes.** Run twice, on any computer, a script writes the very
//!   same `.glb` (the scripts see to it: `assets-src/lib/odgeom.py`).
//!   `maps --check` builds each Map into `target/maps-check/` and says
//!   whether the committed `.glb` is those bytes. CI runs it on Linux
//!   whenever `assets-src/` changes.
//! - **Pictures.** `--previews <folder>` also renders, for each Map, a plan,
//!   a three-quarter view and what each Quad's camera sees from the Launch
//!   Spot and from a few places to fly, into `<folder>/<id>/`. With
//!   `--update-preview` the preview picture becomes the Map's
//!   `preview.png` in the Pack, for the Hub.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::materials;

const USAGE: &str = "Usage: cargo xtask maps [--check] [--previews <folder> [--update-preview]]";

/// The built-in Pack's Maps are in this Pack.
const PACK: &str = "packs/opendrone";

/// Where Blender's version is pinned, from the repo's root.
pub const BLENDER_VERSION_FILE: &str = "assets-src/blender-version.toml";

/// What `assets-src/blender-version.toml` pins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlenderPin {
    /// Such as `5.2.2`.
    pub version: String,
}

pub fn read_pin(text: &str) -> Result<BlenderPin, String> {
    let doc: toml::Table = text
        .parse()
        .map_err(|e| format!("{BLENDER_VERSION_FILE} isn't TOML: {e}"))?;
    let version = doc
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            format!("{BLENDER_VERSION_FILE} needs `version`, such as version = \"5.2.2\"")
        })?;
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(format!(
            "{BLENDER_VERSION_FILE}: version \"{version}\" must be three whole numbers joined by dots, such as \"5.2.2\""
        ));
    }
    Ok(BlenderPin {
        version: version.to_string(),
    })
}

/// Whether the first line `blender --version` prints (such as `Blender 5.2.2
/// LTS`) is the pinned version.
pub fn is_pinned_version(first_line: &str, pin: &BlenderPin) -> bool {
    let Some(rest) = first_line.trim().strip_prefix("Blender ") else {
        return false;
    };
    let number = rest.split_whitespace().next().unwrap_or("");
    number == pin.version
}

struct Options {
    check: bool,
    /// Only these Maps, by id; empty means every Map.
    only: Vec<String>,
    previews: Option<PathBuf>,
    update_preview: bool,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        check: false,
        only: Vec::new(),
        previews: None,
        update_preview: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => options.check = true,
            "--update-preview" => options.update_preview = true,
            "--previews" => match args.next() {
                Some(folder) => options.previews = Some(PathBuf::from(folder)),
                None => return Err("--previews takes the folder to write the pictures to".into()),
            },
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if options.update_preview && options.previews.is_none() {
        return Err("--update-preview needs --previews, which makes the picture".into());
    }
    if options.check && options.update_preview {
        return Err(
            "--check only checks: it never changes the Pack, so not with --update-preview".into(),
        );
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
    match materials::repo().and_then(|root| build(&root, &options)) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

/// The Blender to run: `BLENDER`, then `blender` on the path, then where each
/// operating system installs it.
pub(crate) fn find_blender() -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("BLENDER") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(PathBuf::from("blender"));
    candidates.push(PathBuf::from(
        "/Applications/Blender.app/Contents/MacOS/Blender",
    ));
    candidates.push(PathBuf::from(
        r"C:\Program Files\Blender Foundation\Blender 5.2\blender.exe",
    ));
    for candidate in &candidates {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Ok(candidate.clone());
        }
    }
    Err(format!(
        "Blender isn't installed where this command looks (the BLENDER variable, `blender` on the path, /Applications/Blender.app, Program Files). Install the version {BLENDER_VERSION_FILE} pins, from https://www.blender.org/download/lts/"
    ))
}

pub(crate) fn blender_version_line(blender: &Path) -> Result<String, String> {
    let output = Command::new(blender)
        .arg("--version")
        .output()
        .map_err(|e| format!("can't run {}: {e}", blender.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string())
}

/// Builds just the Maps named, into the Pack, for `cargo xtask play`: the
/// report, or why Blender can't.
pub(crate) fn rebuild(root: &Path, ids: &[String]) -> Result<String, String> {
    let options = Options {
        check: false,
        only: ids.to_vec(),
        previews: None,
        update_preview: false,
    };
    build(root, &options)
}

/// The Maps with a script, by id, in order.
pub(crate) fn map_ids(root: &Path) -> Result<Vec<String>, String> {
    let folder = root.join("assets-src/maps");
    let mut ids = Vec::new();
    for entry in fs::read_dir(&folder).map_err(|e| format!("can't read assets-src/maps: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().join("build.py").is_file()
            && let Some(name) = entry.file_name().to_str()
        {
            ids.push(name.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

fn build(root: &Path, options: &Options) -> Result<String, String> {
    let pin = read_pin(
        &fs::read_to_string(root.join(BLENDER_VERSION_FILE))
            .map_err(|e| format!("{BLENDER_VERSION_FILE}: can't be read: {e}"))?,
    )?;
    let blender = find_blender()?;
    let line = blender_version_line(&blender)?;
    if !is_pinned_version(&line, &pin) {
        return Err(format!(
            "{} is \"{line}\", but {BLENDER_VERSION_FILE} pins Blender {}: Maps are built with the pinned version only, so that every computer writes the same bytes",
            blender.display(),
            pin.version
        ));
    }
    let mut ids = map_ids(root)?;
    if ids.is_empty() {
        return Err("assets-src/maps holds no Map with a build.py".to_string());
    }
    if !options.only.is_empty() {
        ids.retain(|id| options.only.contains(id));
    }
    let mut report = Vec::new();
    let mut stale = Vec::new();
    for id in &ids {
        let committed = root
            .join(PACK)
            .join("maps")
            .join(id)
            .join(format!("{id}.glb"));
        let out = if options.check {
            root.join("target/maps-check").join(format!("{id}.glb"))
        } else {
            committed.clone()
        };
        if let Some(folder) = out.parent() {
            fs::create_dir_all(folder)
                .map_err(|e| format!("can't make {}: {e}", folder.display()))?;
        }
        let mut command = Command::new(&blender);
        command
            .args([
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
            ])
            .arg(root.join("assets-src/maps").join(id).join("build.py"))
            .arg("--")
            .arg("--out")
            .arg(&out)
            .env("PYTHONDONTWRITEBYTECODE", "1");
        let pictures = options.previews.as_ref().map(|folder| folder.join(id));
        if let Some(pictures) = &pictures {
            fs::create_dir_all(pictures)
                .map_err(|e| format!("can't make {}: {e}", pictures.display()))?;
            command.arg("--previews").arg(pictures);
        }
        let output = command
            .output()
            .map_err(|e| format!("can't run {}: {e}", blender.display()))?;
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).into_owned()
                + &String::from_utf8_lossy(&output.stderr);
            let tail: Vec<&str> = text.lines().rev().take(25).collect();
            return Err(format!(
                "Blender failed on assets-src/maps/{id}/build.py:\n{}",
                tail.into_iter().rev().collect::<Vec<_>>().join("\n")
            ));
        }
        let built =
            fs::read(&out).map_err(|e| format!("Blender wrote no {}: {e}", out.display()))?;
        if options.check {
            let have = fs::read(&committed).unwrap_or_default();
            if have == built {
                report.push(format!(
                    "{id}: the committed .glb is what Blender builds ({} bytes)",
                    built.len()
                ));
            } else {
                stale.push(id.clone());
                report.push(format!(
                    "{id}: the committed {PACK}/maps/{id}/{id}.glb ({} bytes) is not what Blender builds from assets-src/maps/{id}/ ({} bytes)",
                    have.len(),
                    built.len()
                ));
            }
        } else {
            report.push(format!(
                "{id}: wrote {PACK}/maps/{id}/{id}.glb ({} bytes)",
                built.len()
            ));
        }
        if let Some(pictures) = &pictures {
            if options.update_preview {
                let preview = root.join(PACK).join("maps").join(id).join("preview.png");
                fs::copy(pictures.join("preview.png"), &preview).map_err(|e| {
                    format!(
                        "can't copy the preview picture to {}: {e}",
                        preview.display()
                    )
                })?;
                report.push(format!("{id}: preview.png is the new preview picture"));
            }
            report.push(format!("{id}: pictures in {}", pictures.display()));
        }
    }
    if stale.is_empty() {
        Ok(report.join("\n"))
    } else {
        Err(format!(
            "{}\n\nRun `cargo xtask maps` with Blender {} and commit the new .glb. If a Map built on a Mac or Windows differs from Linux's only here, download the one CI built (the `maps` artifact of the Maps check) and commit that.",
            report.join("\n"),
            pin.version
        ))
    }
}
