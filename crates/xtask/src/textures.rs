//! `cargo xtask textures` and `cargo xtask credits` (ADR-0014, #63).
//!
//! - `textures` fetches every picture the manifest ([`crate::materials`])
//!   lists into `assets-src/textures/<texture>/diffuse.jpg` and `normal.jpg`,
//!   and refuses any file whose SHA-256 differs from the manifest's: it never
//!   keeps one. A picture already there with the right checksum isn't fetched
//!   again. `--pin` fills in the checksum of every file that has none, for a
//!   texture just added; it never changes one that's there.
//! - `credits` writes `CREDITS.md` from the manifest, and `credits --check`
//!   says whether the committed one is up to date (CI runs it).
//!
//! Downloads go through `curl`, which every machine OpenDrone builds on has.
//! A `file://` address works too, which the readable checks use.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use sha2::{Digest, Sha256};

use crate::materials::{self, MANIFEST, Manifest, TEXTURES_FOLDER, TextureFile};

const USER_AGENT: &str = "OpenDrone-asset-fetch (https://github.com/BartoszSolkaBD/OpenDrone)";

/// The SHA-256 of some bytes, as 64 lowercase hex digits.
pub fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn run_textures(args: &[String]) -> ExitCode {
    let pin = match args {
        [] => false,
        [flag] if flag == "--pin" => true,
        _ => {
            eprintln!("Usage: cargo xtask textures [--pin]");
            return ExitCode::from(2);
        }
    };
    let result = materials::repo().and_then(|root| {
        let manifest = materials::read(&root)?;
        fetch(&root, &manifest, pin)
    });
    match result {
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

/// Downloads `url` into `to`. The file is written only if the whole download
/// worked.
pub(crate) fn download(url: &str, to: &Path) -> Result<(), String> {
    if let Some(folder) = to.parent() {
        fs::create_dir_all(folder).map_err(|e| format!("can't make {}: {e}", folder.display()))?;
    }
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "600",
            "--user-agent",
            USER_AGENT,
            "--output",
        ])
        .arg(to)
        .arg(url)
        .output()
        .map_err(|e| format!("can't run curl to download {url}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let _ = fs::remove_file(to);
        Err(format!(
            "{url} can't be downloaded: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The bytes of one file of one texture, downloaded: the file itself, or the
/// named file inside the zip its address gives.
fn fetch_bytes(file: &TextureFile, scratch: &Path) -> Result<Vec<u8>, String> {
    let name = file
        .url
        .rsplit(['/', '='])
        .next()
        .unwrap_or("download")
        .to_string();
    let to = scratch.join(&name);
    download(&file.url, &to)?;
    let bytes = fs::read(&to).map_err(|e| format!("can't read {}: {e}", to.display()))?;
    let _ = fs::remove_file(&to);
    match &file.in_zip {
        None => Ok(bytes),
        Some(inside) => {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
                .map_err(|e| format!("{} isn't a zip file: {e}", file.url))?;
            let mut entry = archive
                .by_name(inside)
                .map_err(|_| format!("{} has no file called {inside}", file.url))?;
            let mut out = Vec::new();
            entry
                .read_to_end(&mut out)
                .map_err(|e| format!("can't unpack {inside} from {}: {e}", file.url))?;
            Ok(out)
        }
    }
}

pub(crate) fn fetch(root: &Path, manifest: &Manifest, pin: bool) -> Result<String, String> {
    let cache = root.join(TEXTURES_FOLDER);
    let scratch = root.join("target/textures-download");
    let mut lines = Vec::new();
    let mut pinned: Vec<(String, String, String)> = Vec::new();
    for texture in &manifest.textures {
        for file in &texture.files {
            let at = cache.join(file.cached_as(&texture.name));
            let label = format!("{}/{}", texture.name, file.kind);
            if !file.sha256.is_empty()
                && let Ok(have) = fs::read(&at)
                && sha256(&have) == file.sha256
            {
                lines.push(format!("have    {label}"));
                continue;
            }
            let bytes = fetch_bytes(file, &scratch)?;
            let found = sha256(&bytes);
            if file.sha256.is_empty() {
                if !pin {
                    return Err(format!(
                        "{label} has no checksum in {MANIFEST}: run `cargo xtask textures --pin` to write the checksum of what is downloaded, and review that it is the picture the source's page shows"
                    ));
                }
                pinned.push((texture.name.clone(), file.kind.clone(), found.clone()));
            } else if found != file.sha256 {
                return Err(format!(
                    "{label} from {} has the SHA-256 {found}, but {MANIFEST} says {}: it was refused, and nothing was kept",
                    file.url, file.sha256
                ));
            }
            if let Some(folder) = at.parent() {
                fs::create_dir_all(folder)
                    .map_err(|e| format!("can't make {}: {e}", folder.display()))?;
            }
            fs::write(&at, &bytes).map_err(|e| format!("can't write {}: {e}", at.display()))?;
            lines.push(format!("fetched {label} ({} bytes)", bytes.len()));
        }
    }
    if !pinned.is_empty() {
        write_pins(&root.join(MANIFEST), &pinned)?;
        lines.push(format!(
            "pinned  {} checksum(s) in {MANIFEST}",
            pinned.len()
        ));
    }
    lines.push(format!(
        "{} texture(s) ready in {TEXTURES_FOLDER}/, which git ignores.",
        manifest.textures.len()
    ));
    Ok(lines.join("\n"))
}

/// Writes checksums into the manifest, keeping every comment and its layout.
fn write_pins(path: &PathBuf, pins: &[(String, String, String)]) -> Result<(), String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("{MANIFEST} isn't TOML: {e}"))?;
    for (texture, kind, sha) in pins {
        let file = doc["textures"][texture.as_str()][kind.as_str()]
            .as_inline_table_mut()
            .ok_or_else(|| format!("{texture}'s {kind} isn't an inline table"))?;
        file.insert("sha256", sha.as_str().into());
    }
    fs::write(path, doc.to_string()).map_err(|e| format!("can't write {}: {e}", path.display()))
}

pub fn run_credits(args: &[String]) -> ExitCode {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            eprintln!("Usage: cargo xtask credits [--check]");
            return ExitCode::from(2);
        }
    };
    let result = materials::repo().and_then(|root| {
        let manifest = materials::read(&root)?;
        if let Some((name, file)) = manifest.textures.iter().find_map(|t| {
            t.files
                .iter()
                .find(|f| f.sha256.is_empty())
                .map(|f| (&t.name, f))
        }) {
            return Err(format!(
                "{name}'s {} file has no checksum yet, so CREDITS.md can't list it: run `cargo xtask textures --pin`",
                file.kind
            ));
        }
        let datasets = crate::datasets::read(&root)?;
        let wanted = materials::credits(&manifest, &datasets);
        let path = root.join("CREDITS.md");
        let have = fs::read_to_string(&path).unwrap_or_default();
        if check {
            if have == wanted {
                Ok("CREDITS.md is up to date.".to_string())
            } else {
                Err("CREDITS.md is out of date with assets-src/materials.toml or assets-src/datasets.toml and the data files it lists: run `cargo xtask credits` and commit it".to_string())
            }
        } else if have == wanted {
            Ok("CREDITS.md was already up to date.".to_string())
        } else {
            fs::write(&path, wanted).map_err(|e| format!("can't write CREDITS.md: {e}"))?;
            Ok("Wrote CREDITS.md.".to_string())
        }
    });
    match result {
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
