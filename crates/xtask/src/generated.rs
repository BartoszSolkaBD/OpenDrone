//! `cargo xtask generate-textures`: makes the textures that are made from
//! scratch, and checks the committed pictures are what the generators make
//! (ADR-0024, #292).
//!
//! - **What is generated.** A texture in `assets-src/materials.toml` with a
//!   `generator`, a `seed` and a size in `pixels` has no download: the script
//!   `assets-src/generators/<generator>.py` makes its three pictures,
//!   `diffuse.jpg`, `normal.jpg` and `roughness.jpg`, which are committed in
//!   `assets-src/generated/<texture>/`, with a contact sheet of all of them in
//!   `docs/images/`. A decal's generator makes a fourth, `mask.jpg` (#321),
//!   and its texture is left off the contact sheet.
//! - **The same bytes.** The scripts run in the pinned Blender's Python
//!   (`assets-src/blender-version.toml`, the file `cargo xtask maps` reads
//!   too), whose numpy is the same on every computer, and write their JPG
//!   with a writer of their own that uses nothing a processor may round
//!   differently (`assets-src/lib/odjpeg.py`). `--check` makes the pictures
//!   into `target/textures-check/` and says whether the committed ones are
//!   the very same bytes. CI runs it on Linux.
//! - **Not here.** `cargo xtask textures` and `cargo xtask credits` leave
//!   these textures alone: nothing is downloaded, and nothing outside is
//!   credited.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::maps::{self, BLENDER_VERSION_FILE};
use crate::materials::{self, GENERATED_FOLDER, GENERATORS_FOLDER, Generated, Texture};

const USAGE: &str = "Usage: cargo xtask generate-textures [--check]";

/// The three pictures each generated texture has.
pub const PICTURES: [&str; 3] = ["diffuse.jpg", "normal.jpg", "roughness.jpg"];

/// The fourth picture a decal's texture has: how much of it covers the
/// surface under it (#321).
pub const MASK: &str = "mask.jpg";

/// The contact sheet of every generated texture, from the repo's root.
pub const SHEET: &str = "docs/images/materials-made-from-scratch.jpg";

/// The driver the pinned Blender runs, from the repo's root.
const DRIVER: &str = "assets-src/lib/odtexgen.py";

/// Where `--check` makes its pictures, from the repo's root.
const CHECK_FOLDER: &str = "target/textures-check";

/// A generated texture with its recipe.
struct Recipe<'a> {
    texture: &'a Texture,
    how: &'a Generated,
}

pub fn run(args: &[String]) -> ExitCode {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match materials::repo().and_then(|root| generate(&root, check)) {
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

fn kilobytes(bytes: usize) -> String {
    format!("{:.0} KB", bytes as f64 / 1000.0)
}

fn generate(root: &Path, check: bool) -> Result<String, String> {
    let manifest = materials::read(root)?;
    let recipes: Vec<Recipe> = manifest
        .textures
        .iter()
        .filter_map(|texture| {
            texture
                .generated
                .as_ref()
                .map(|how| Recipe { texture, how })
        })
        .collect();
    let committed = root.join(GENERATED_FOLDER);
    let orphans = orphan_folders(&committed, &recipes);
    if recipes.is_empty() {
        if !check && !orphans.is_empty() {
            remove_orphans(&committed, &orphans)?;
        }
        return Ok("No texture is made from scratch.".to_string());
    }
    for recipe in &recipes {
        let script = root
            .join(GENERATORS_FOLDER)
            .join(format!("{}.py", recipe.how.generator));
        if !script.is_file() {
            return Err(format!(
                "the texture \"{}\" names the generator \"{}\", but {GENERATORS_FOLDER}/{}.py doesn't exist",
                recipe.texture.name, recipe.how.generator, recipe.how.generator
            ));
        }
    }

    let pin = maps::read_pin(
        &fs::read_to_string(root.join(BLENDER_VERSION_FILE))
            .map_err(|e| format!("{BLENDER_VERSION_FILE}: can't be read: {e}"))?,
    )?;
    let blender = maps::find_blender()?;
    let line = maps::blender_version_line(&blender)?;
    if !maps::is_pinned_version(&line, &pin) {
        return Err(format!(
            "{} is \"{line}\", but {BLENDER_VERSION_FILE} pins Blender {}: textures are made with the pinned version only, so that every computer writes the same bytes",
            blender.display(),
            pin.version
        ));
    }

    let (out, sheet) = if check {
        let folder = root.join(CHECK_FOLDER);
        let _ = fs::remove_dir_all(&folder);
        (folder.join("textures"), folder.join("sheet.jpg"))
    } else {
        (committed.clone(), root.join(SHEET))
    };
    for folder in [Some(out.as_path()), sheet.parent()].into_iter().flatten() {
        fs::create_dir_all(folder).map_err(|e| format!("can't make {}: {e}", folder.display()))?;
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
        .arg(root.join(DRIVER))
        .arg("--")
        .arg("--out")
        .arg(&out)
        .arg("--sheet")
        .arg(&sheet);
    for recipe in &recipes {
        command.arg("--texture").args([
            recipe.texture.name.clone(),
            recipe.how.generator.clone(),
            recipe.how.seed.to_string(),
            recipe.how.pixels.to_string(),
        ]);
    }
    let output = command
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .map_err(|e| format!("can't run {}: {e}", blender.display()))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = text.lines().rev().take(25).collect();
        return Err(format!(
            "Blender failed on {DRIVER}:\n{}",
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }

    let mut report = Vec::new();
    let mut problems = Vec::new();
    let mut total = 0;
    for recipe in &recipes {
        let name = &recipe.texture.name;
        let mut sizes = Vec::new();
        let mut texture_total = 0;
        // A decal's mask, if the generator made one or one is committed.
        let decal = [&out, &committed]
            .iter()
            .any(|folder| folder.join(name).join(MASK).is_file());
        for picture in PICTURES.into_iter().chain(decal.then_some(MASK)) {
            let made = fs::read(out.join(name).join(picture))
                .map_err(|e| format!("Blender wrote no {name}/{picture}: {e}"))?;
            texture_total += made.len();
            sizes.push(format!(
                "{} {}",
                picture.trim_end_matches(".jpg"),
                kilobytes(made.len())
            ));
            if check {
                let at = format!("{GENERATED_FOLDER}/{name}/{picture}");
                match fs::read(committed.join(name).join(picture)) {
                    Ok(have) if have == made => {}
                    Ok(have) => problems.push(format!(
                        "{at} ({} bytes) is not what {}.py makes ({} bytes)",
                        have.len(),
                        recipe.how.generator,
                        made.len()
                    )),
                    Err(_) => problems.push(format!("{at} is missing")),
                }
            }
        }
        total += texture_total;
        report.push(format!(
            "{name}: {}px, {} ({})",
            recipe.how.pixels,
            kilobytes(texture_total),
            sizes.join(", ")
        ));
    }
    let made_sheet =
        fs::read(&sheet).map_err(|e| format!("Blender wrote no contact sheet: {e}"))?;
    if check {
        match fs::read(root.join(SHEET)) {
            Ok(have) if have == made_sheet => {}
            Ok(_) => problems.push(format!("{SHEET} is not what the contact sheet is made as")),
            Err(_) => problems.push(format!("{SHEET} is missing")),
        }
        for orphan in &orphans {
            problems.push(format!(
                "{GENERATED_FOLDER}/{orphan}/ belongs to no texture in assets-src/materials.toml"
            ));
        }
    } else if !orphans.is_empty() {
        remove_orphans(&committed, &orphans)?;
        report.push(format!(
            "removed {GENERATED_FOLDER}/{}: no texture in the manifest makes them",
            orphans.join(", ")
        ));
    }
    report.push(format!(
        "{} texture(s) made from scratch, {} in all, and a contact sheet of {}",
        recipes.len(),
        kilobytes(total),
        kilobytes(made_sheet.len())
    ));
    if problems.is_empty() {
        if check {
            report.push("the committed pictures are what the generators make".to_string());
        }
        Ok(report.join("\n"))
    } else {
        Err(format!(
            "{}\n\n{}\n\nRun `cargo xtask generate-textures` with Blender {} and commit the new pictures. If they differ only on one operating system, download the ones CI made (the `textures` download of the Maps check) and commit those.",
            report.join("\n"),
            problems.join("\n"),
            pin.version
        ))
    }
}

/// The folders in `assets-src/generated/` that no texture of the manifest
/// makes, by name.
fn orphan_folders(committed: &Path, recipes: &[Recipe]) -> Vec<String> {
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(committed) {
        for entry in entries.flatten() {
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
                && !recipes.iter().any(|r| r.texture.name == name)
            {
                found.push(name.to_string());
            }
        }
    }
    found.sort();
    found
}

fn remove_orphans(committed: &Path, orphans: &[String]) -> Result<(), String> {
    for orphan in orphans {
        let folder: PathBuf = committed.join(orphan);
        fs::remove_dir_all(&folder)
            .map_err(|e| format!("can't remove {}: {e}", folder.display()))?;
    }
    Ok(())
}
