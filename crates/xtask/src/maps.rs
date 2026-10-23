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
//! - **Baked light** (#293). A script that calls `lightmap(texel_cm)` gets a
//!   second UV set in its `.glb`, and `cargo xtask maps --bake <id>` bakes the
//!   Map's `[light]` into an atlas with Cycles and writes `<id>.bake.toml`
//!   beside it, which says which `.glb` and which `[light]` the atlas was
//!   baked from. The bake isn't rebuilt in CI (Cycles may differ from run to
//!   run), so `maps --check` instead says the atlas is out of date when the
//!   `.glb` or the `[light]` it was baked from is not the committed one.
//!   `assets-src/lightmap-uv.toml` holds the SHA-256 of every Map's `.glb`
//!   built with a second UV set at 25 cm per texel, which `--check` demands
//!   on every computer, so the unwrap is proven to give the same bytes too.
//! - **Pictures.** `--previews <folder>` also renders, for each Map, a plan,
//!   a three-quarter view and what each Quad's camera sees from the Launch
//!   Spot and from a few places to fly, into `<folder>/<id>/`. With
//!   `--update-preview` the preview picture becomes the Map's
//!   `preview.png` in the Pack, for the Hub.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::materials;

const USAGE: &str = "Usage: cargo xtask maps [--check] [--previews <folder> [--update-preview]]\n       cargo xtask maps --bake <map id> [--samples <n>]";

/// The SHA-256 of each Map's `.glb` built with a second UV set at
/// [`PROOF_TEXEL_CM`] centimetres per texel, from the repo's root.
pub const LIGHT_UV_FILE: &str = "assets-src/lightmap-uv.toml";

/// The density the second UV set is proven at, whatever a Map asks for.
const PROOF_TEXEL_CM: &str = "25";

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
    previews: Option<PathBuf>,
    update_preview: bool,
    /// The Map to bake light for, by id.
    bake: Option<String>,
    samples: Option<u32>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        check: false,
        previews: None,
        update_preview: false,
        bake: None,
        samples: None,
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
            "--bake" => match args.next() {
                Some(id) => options.bake = Some(id.clone()),
                None => return Err("--bake takes the id of the Map to bake, such as bando".into()),
            },
            "--samples" => match args.next().and_then(|n| n.parse::<u32>().ok()) {
                Some(n) if n > 0 => options.samples = Some(n),
                _ => {
                    return Err(
                        "--samples takes a whole number of samples per texel, such as 256".into(),
                    );
                }
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
    if options.samples.is_some() && options.bake.is_none() {
        return Err("--samples is for --bake, which says how many samples each texel gets".into());
    }
    if options.bake.is_some() && (options.check || options.previews.is_some()) {
        return Err(
            "--bake bakes one Map's light and nothing else: not with --check or --previews".into(),
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
fn find_blender() -> Result<PathBuf, String> {
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

fn blender_version_line(blender: &Path) -> Result<String, String> {
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

/// The Maps with a script, by id, in order.
fn map_ids(root: &Path) -> Result<Vec<String>, String> {
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

/// The SHA-256 of some bytes, as 64 lowercase hex digits.
fn sha256(bytes: &[u8]) -> String {
    crate::textures::sha256(bytes)
}

/// What a bake was made from, as one fingerprint: the keys of a `map.toml`'s
/// `[light]` table, in key order, each as `key = value` on a line, with the
/// values as written. `assets-src/lib/odlight.py` makes the same one.
pub fn light_key(map_toml: &str) -> Result<String, String> {
    let doc: toml::Table = map_toml
        .parse()
        .map_err(|e| format!("map.toml isn't TOML: {e}"))?;
    let light = doc
        .get("light")
        .and_then(toml::Value::as_table)
        .ok_or("map.toml has no [light] table, which the bake needs")?;
    let mut keys: Vec<&String> = light.keys().collect();
    keys.sort();
    let mut text = String::new();
    for key in keys {
        let value = light[key]
            .as_str()
            .ok_or_else(|| format!("[light] `{key}` must be text in quotes"))?;
        text.push_str(&format!("{key} = {value}\n"));
    }
    Ok(sha256(text.as_bytes()))
}

/// Whether a Map that asks for baked light has an atlas baked from the very
/// `.glb` and `[light]` it has now: a sentence on what is out of date, or the
/// one that says it is current. `sidecar` is the text of `<id>.bake.toml`, if
/// the Map has one.
pub fn bake_state(
    id: &str,
    sidecar: Option<&str>,
    atlas_exists: impl Fn(&str) -> bool,
    glb: &[u8],
    map_toml: &str,
) -> Result<String, String> {
    let rebake =
        format!("run `cargo xtask maps --bake {id}` and commit the atlas and {id}.bake.toml");
    let Some(sidecar) = sidecar else {
        return Err(format!(
            "{id}: its script asks for baked light, but there is no {id}.bake.toml, so no atlas has been baked: {rebake}"
        ));
    };
    let table: toml::Table = sidecar
        .parse()
        .map_err(|e| format!("{id}: {id}.bake.toml isn't TOML ({e}): {rebake}"))?;
    let text = |key: &str| table.get(key).and_then(toml::Value::as_str);
    let atlas =
        text("atlas").ok_or_else(|| format!("{id}: {id}.bake.toml names no `atlas`: {rebake}"))?;
    if !atlas_exists(atlas) {
        return Err(format!(
            "{id}: {id}.bake.toml names the atlas {atlas}, but the file isn't in the Map's folder: {rebake}"
        ));
    }
    if text("glb_sha256") != Some(sha256(glb).as_str()) {
        return Err(format!(
            "{id}: the atlas {atlas} was baked from another {id}.glb than the one Blender builds now (the shapes, or the second UV set, changed): {rebake}"
        ));
    }
    if text("light_sha256") != Some(light_key(map_toml)?.as_str()) {
        return Err(format!(
            "{id}: the atlas {atlas} was baked with other [light] values than map.toml has now: {rebake}"
        ));
    }
    Ok(format!(
        "{id}: the atlas {atlas} was baked from this .glb and these [light] values"
    ))
}

/// The `[maps]` table of `assets-src/lightmap-uv.toml`: Map id to SHA-256.
fn read_light_uv_hashes(text: &str) -> Result<Vec<(String, String)>, String> {
    let doc: toml::Table = text
        .parse()
        .map_err(|e| format!("{LIGHT_UV_FILE} isn't TOML: {e}"))?;
    let maps = doc
        .get("maps")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{LIGHT_UV_FILE} needs a [maps] table"))?;
    let mut out = Vec::new();
    for (id, hash) in maps {
        let hash = hash
            .as_str()
            .ok_or_else(|| format!("{LIGHT_UV_FILE}: {id} must be text"))?;
        out.push((id.clone(), hash.to_string()));
    }
    Ok(out)
}

fn write_light_uv_hashes(hashes: &[(String, String)]) -> String {
    let mut text = format!(
        "# The SHA-256 of each Map's .glb built with a second UV set at {PROOF_TEXEL_CM} cm per texel:\n# `cargo xtask maps` writes this file and `maps --check` (CI, on Linux) demands it, so the\n# unwrap in assets-src/lib/odlight.py is proven to give the same bytes on every computer (#293).\n# Never edit it by hand.\n\n[maps]\n"
    );
    for (id, hash) in hashes {
        text.push_str(&format!("{id} = \"{hash}\"\n"));
    }
    text
}

/// Runs Blender on a Map's script with these arguments after `--`.
fn run_script(
    blender: &Path,
    root: &Path,
    id: &str,
    out: &Path,
    extra: &[std::ffi::OsString],
) -> Result<(), String> {
    let mut command = Command::new(blender);
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
        .arg(out)
        .args(extra)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let output = command
        .output()
        .map_err(|e| format!("can't run {}: {e}", blender.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = text.lines().rev().take(25).collect();
    Err(format!(
        "Blender failed on assets-src/maps/{id}/build.py:\n{}",
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    ))
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
    if let Some(bake) = &options.bake {
        if !ids.contains(bake) {
            return Err(format!(
                "--bake {bake}: there's no Map with that id; the Maps with a script are {}",
                ids.join(", ")
            ));
        }
        ids = vec![bake.clone()];
    }
    let scratch = root.join("target/maps-check");
    fs::create_dir_all(&scratch).map_err(|e| format!("can't make {}: {e}", scratch.display()))?;
    let golden_file = root.join(LIGHT_UV_FILE);
    let golden_text = fs::read_to_string(&golden_file).ok();
    let golden = golden_text
        .as_deref()
        .map(read_light_uv_hashes)
        .transpose()?;
    let mut proven: Vec<(String, String)> = Vec::new();
    let mut report = Vec::new();
    let mut stale = Vec::new();
    for id in &ids {
        let folder = root.join(PACK).join("maps").join(id);
        let committed = folder.join(format!("{id}.glb"));
        let out = if options.check {
            scratch.join(format!("{id}.glb"))
        } else {
            committed.clone()
        };
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("can't make {}: {e}", parent.display()))?;
        }
        let info = scratch.join(format!("{id}.light-info.toml"));
        let _ = fs::remove_file(&info);
        let mut extra: Vec<std::ffi::OsString> = vec!["--light-info".into(), info.clone().into()];
        let atlas = folder.join(format!("{id}.light.exr"));
        let bake_report = scratch.join(format!("{id}.bake-report.toml"));
        if options.bake.is_some() {
            extra.extend([
                "--bake".into(),
                atlas.clone().into(),
                "--map-toml".into(),
                folder.join("map.toml").into(),
                "--bake-report".into(),
                bake_report.clone().into(),
            ]);
            if let Some(samples) = options.samples {
                extra.extend(["--samples".into(), samples.to_string().into()]);
            }
        }
        let pictures = options.previews.as_ref().map(|folder| folder.join(id));
        if let Some(pictures) = &pictures {
            fs::create_dir_all(pictures)
                .map_err(|e| format!("can't make {}: {e}", pictures.display()))?;
            extra.extend(["--previews".into(), pictures.clone().into()]);
        }
        run_script(&blender, root, id, &out, &extra)?;
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
        if options.bake.is_some() {
            let measured = fs::read_to_string(&bake_report).map_err(|e| {
                format!("Blender left no bake report {}: {e}", bake_report.display())
            })?;
            let sidecar = format!(
                "# Written by `cargo xtask maps --bake {id}`; never edit it by hand. It says what {id}'s\n# light atlas was baked from, so `maps --check` can tell when the atlas is out of date.\n{measured}"
            );
            fs::write(folder.join(format!("{id}.bake.toml")), sidecar)
                .map_err(|e| format!("can't write {id}.bake.toml: {e}"))?;
            let size = fs::metadata(&atlas).map(|m| m.len()).unwrap_or(0);
            report.push(format!(
                "{id}: baked {PACK}/maps/{id}/{id}.light.exr ({size} bytes) and wrote {id}.bake.toml:\n{measured}"
            ));
        } else if info.is_file() {
            let sidecar = fs::read_to_string(folder.join(format!("{id}.bake.toml"))).ok();
            let map_toml = fs::read_to_string(folder.join("map.toml")).unwrap_or_default();
            match bake_state(
                id,
                sidecar.as_deref(),
                |name| folder.join(name).is_file(),
                &built,
                &map_toml,
            ) {
                Ok(line) => report.push(line),
                Err(problem) => {
                    if options.check {
                        stale.push(id.clone());
                    }
                    report.push(problem);
                }
            }
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
        // The second UV set, at one density for every Map, must be the same
        // bytes everywhere: only the hash is committed.
        if golden.is_some() && options.bake.is_none() {
            let proof = scratch.join(format!("{id}.light-uv.glb"));
            let args: Vec<std::ffi::OsString> =
                vec!["--light-uv-proof".into(), PROOF_TEXEL_CM.into()];
            run_script(&blender, root, id, &proof, &args)?;
            let bytes = fs::read(&proof)
                .map_err(|e| format!("Blender wrote no {}: {e}", proof.display()))?;
            proven.push((id.clone(), sha256(&bytes)));
        }
    }
    if let Some(golden) = &golden {
        if options.check {
            for (id, hash) in &proven {
                if golden.iter().any(|(g, h)| g == id && h == hash) {
                    report.push(format!(
                        "{id}: the second UV set is the very same bytes as the committed hash in {LIGHT_UV_FILE}"
                    ));
                } else {
                    stale.push(id.clone());
                    report.push(format!(
                        "{id}: the .glb built with a second UV set at {PROOF_TEXEL_CM} cm per texel hashes to {hash}, which isn't what {LIGHT_UV_FILE} says: the unwrap changed, or it isn't deterministic on this computer"
                    ));
                }
            }
        } else if options.bake.is_none() {
            let text = write_light_uv_hashes(&proven);
            if golden_text.as_deref() != Some(text.as_str()) {
                fs::write(&golden_file, text)
                    .map_err(|e| format!("can't write {LIGHT_UV_FILE}: {e}"))?;
                report.push(format!(
                    "{LIGHT_UV_FILE} has the new hashes of the second UV sets"
                ));
            }
        }
    }
    if stale.is_empty() {
        Ok(report.join("\n"))
    } else {
        Err(format!(
            "{}\n\nRun `cargo xtask maps` with Blender {} and commit the new .glb (and, for a Map with baked light, `cargo xtask maps --bake <id>` for its atlas). If a Map built on a Mac or Windows differs from Linux's only here, download the one CI built (the `maps` artifact of the Maps check) and commit that.",
            report.join("\n"),
            pin.version
        ))
    }
}
