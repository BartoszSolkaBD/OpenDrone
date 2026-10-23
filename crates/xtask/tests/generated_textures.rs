//! Readable checks for the textures made from scratch (ADR-0024, #292): how
//! `assets-src/materials.toml` marks one, what `cargo xtask textures` and
//! `cargo xtask credits` do about it (nothing: it has no download and no
//! licence to credit), and `cargo xtask generate-textures`. Each runs the real
//! command on a scratch repo. The command that makes the pictures uses a
//! stand-in for Blender, a small shell script, so those checks run on Unix
//! only: what they prove (the pinned version, the same bytes required) doesn't
//! depend on the operating system. The generators themselves are checked
//! where they can run, in Blender, by `assets-src/generators/test_generators.py`.
//! Basis: Rule (ADR-0024: nothing downloaded for a detailed Map, the pictures
//! committed, a check regenerates them; #292).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs `cargo xtask <args>` in `folder`: whether it passed, and what it
/// printed.
fn xtask(folder: &Path, args: &[&str], envs: &[(&str, &str)]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(folder)
        .envs(envs.iter().copied())
        .output()
        .expect("xtask runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), text)
}

/// A scratch repo: just `assets-src/`, where the commands find the repo's
/// root.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(case: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("generated-textures")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets-src")).unwrap();
        Repo { root }
    }

    fn write(&self, file: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    fn read(&self, file: &str) -> String {
        fs::read_to_string(self.root.join(file)).unwrap()
    }

    fn exists(&self, file: &str) -> bool {
        self.root.join(file).exists()
    }

    /// A manifest with one material using the texture `zinc`, which
    /// `texture_entry` describes.
    fn manifest(&self, texture_entry: &str) {
        let text = format!(
            r#"[materials.zinc_roof]
colour    = [0.25, 0.18, 0.14]
roughness = 0.80
texture   = "zinc"

[textures.zinc]
{texture_entry}
"#
        );
        self.write("assets-src/materials.toml", text.as_bytes());
    }
}

const GENERATED: &str = r#"generator = "zinc_sheet"
seed      = 7
pixels    = 1024
size      = "1.2 m""#;

// the manifest

#[test]
fn a_texture_made_from_scratch_is_not_downloaded_and_nothing_is_credited_for_it() {
    let repo = Repo::new("not-downloaded");
    repo.manifest(GENERATED);
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(ok, "{text}");
    assert!(
        text.contains("0 texture(s) ready"),
        "nothing to fetch: {text}"
    );
    assert!(!repo.exists("assets-src/textures"), "{text}");
    // `--pin` has no checksum to write for it either.
    let (ok, text) = xtask(&repo.root, &["textures", "--pin"], &[]);
    assert!(ok, "{text}");
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(ok, "{text}");
    let credits = repo.read("CREDITS.md");
    assert!(
        !credits.contains("zinc"),
        "CREDITS.md lists nothing for it: {credits}"
    );
    let (ok, text) = xtask(&repo.root, &["credits", "--check"], &[]);
    assert!(ok, "{text}");
}

#[test]
fn a_texture_that_is_neither_a_download_nor_made_by_a_generator_is_refused() {
    let repo = Repo::new("neither");
    repo.manifest(r#"size = "1.2 m""#);
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains("the texture \"zinc\" is neither a download")
            && text.contains("nor made from scratch"),
        "{text}"
    );
}

#[test]
fn a_texture_cannot_be_a_download_and_made_from_scratch_at_once() {
    let repo = Repo::new("both");
    repo.manifest(&format!(
        "{GENERATED}\nsource = \"Poly Haven\"\nlicence = \"CC0-1.0\""
    ));
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains("made from scratch, and a download's `source` doesn't belong with it"),
        "{text}"
    );
}

#[test]
fn a_generated_texture_needs_its_generator_seed_size_in_pixels_and_repeat_size() {
    let repo = Repo::new("incomplete");
    repo.manifest("generator = \"Rust Sheet\"\nseed = -1\npixels = 1000");
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(!ok);
    assert!(
        text.contains("`generator` must be a script's name in quotes, with lowercase letters"),
        "{text}"
    );
    assert!(
        text.contains("needs `seed`, a whole number from 0 to"),
        "{text}"
    );
    assert!(
        text.contains("needs `pixels`, the picture's width and height: one of 512, 1024, 2048"),
        "{text}"
    );
    assert!(text.contains("needs `size`, in quotes"), "{text}");
}

#[test]
fn a_generated_texture_no_material_uses_is_refused_like_any_other() {
    let repo = Repo::new("unused");
    repo.manifest(GENERATED);
    let text = fs::read_to_string(repo.root.join("assets-src/materials.toml")).unwrap();
    repo.write(
        "assets-src/materials.toml",
        text.replace("texture   = \"zinc\"", "").as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(!ok);
    assert!(
        text.contains("the texture \"zinc\" is listed but no material uses it"),
        "{text}"
    );
}

#[test]
fn the_texture_the_macro_table_names_needs_no_material_of_its_own() {
    // The large-scale picture (#304) is laid over the whole world, so no one
    // material uses it.
    let repo = Repo::new("macro-texture");
    repo.manifest(GENERATED);
    let text = repo.read("assets-src/materials.toml");
    repo.write(
        "assets-src/materials.toml",
        format!(
            "{}\n[macro]\ntexture = \"wide\"\n\n[textures.wide]\ngenerator = \"zinc_sheet\"\nseed = 1\npixels = 512\nsize = \"80 m\"\n",
            text
        )
        .as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(ok, "{text}");
}

#[test]
fn a_generator_with_no_script_is_refused_before_blender_is_asked() {
    let repo = Repo::new("no-script");
    repo.manifest(GENERATED);
    let (ok, text) = xtask(&repo.root, &["generate-textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains("names the generator \"zinc_sheet\", but assets-src/generators/zinc_sheet.py doesn't exist"),
        "{text}"
    );
}

#[test]
fn a_manifest_with_no_generated_texture_has_nothing_to_make() {
    let repo = Repo::new("nothing");
    repo.write(
        "assets-src/materials.toml",
        b"[materials.flat]\ncolour = [0.1, 0.1, 0.1]\nroughness = 0.9\n\n[textures]\n",
    );
    let (ok, text) = xtask(&repo.root, &["generate-textures", "--check"], &[]);
    assert!(ok, "{text}");
    assert!(text.contains("No texture is made from scratch."), "{text}");
}

// the real repo

/// The width and height a JPEG says it has, from its frame header.
fn jpeg_size(bytes: &[u8]) -> Option<(usize, usize)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 9 < bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        let length = usize::from(bytes[at + 2]) << 8 | usize::from(bytes[at + 3]);
        if marker == 0xC0 {
            let height = usize::from(bytes[at + 5]) << 8 | usize::from(bytes[at + 6]);
            let width = usize::from(bytes[at + 7]) << 8 | usize::from(bytes[at + 8]);
            return Some((width, height));
        }
        at += 2 + length;
    }
    None
}

#[test]
fn every_texture_the_repo_makes_from_scratch_is_committed_as_three_pictures_of_the_size_it_says() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = fs::read_to_string(repo.join("assets-src/materials.toml")).unwrap();
    let doc: toml::Table = manifest.parse().unwrap();
    let textures = doc["textures"].as_table().unwrap();
    let mut made = 0;
    for (name, entry) in textures {
        let Some(pixels) = entry.get("pixels").and_then(toml::Value::as_integer) else {
            continue;
        };
        made += 1;
        for picture in ["diffuse.jpg", "normal.jpg", "roughness.jpg"] {
            let path = repo.join("assets-src/generated").join(name).join(picture);
            let bytes =
                fs::read(&path).unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
            assert_eq!(
                jpeg_size(&bytes),
                Some((pixels as usize, pixels as usize)),
                "{name}/{picture} is a JPEG of {pixels} x {pixels} pixels"
            );
        }
    }
    assert!(made >= 8, "the first slice's eight materials: {made}");
}

#[test]
fn the_pictures_made_from_scratch_stay_within_the_size_budget() {
    // ADR-0024: a generated picture is 2K at most for a hero surface and 1K
    // otherwise, and a Map has a size budget for them. The first slice's
    // budget is 10 MB for all eight; the PR says what each costs.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut total = 0;
    for entry in fs::read_dir(repo.join("assets-src/generated")).unwrap() {
        let folder = entry.unwrap().path();
        for file in fs::read_dir(&folder).unwrap() {
            total += file.unwrap().metadata().unwrap().len();
        }
    }
    assert!(
        total <= 10_000_000,
        "the committed pictures take {total} bytes, over the 10 MB budget"
    );
}

// generate-textures, with a stand-in for Blender

#[cfg(unix)]
mod generate {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in for Blender: a shell script that says it is `version` and,
    /// asked to run the driver, writes `bytes` as each picture of every
    /// `--texture` into `--out`, and as the contact sheet where `--sheet` says.
    fn fake_blender(repo: &Repo, version: &str, bytes: &str) -> PathBuf {
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "Blender {version} LTS"
  exit 0
fi
out=""; sheet=""; names=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --sheet) sheet="$2"; shift ;;
    --texture) names="$names $2"; shift 4 ;;
  esac
  shift
done
for name in $names; do
  mkdir -p "$out/$name"
  for picture in diffuse normal roughness; do printf '%s' '{bytes}' > "$out/$name/$picture.jpg"; done
done
if [ -n "$sheet" ]; then printf '%s' '{bytes}' > "$sheet"; fi
"#
        );
        let path = repo.write("fake-blender", script.as_bytes());
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    /// A scratch repo with one generated texture, its script, the Blender
    /// pin, and the committed pictures made of `committed`.
    fn repo_with_a_texture(case: &str, committed: &str) -> Repo {
        let repo = Repo::new(case);
        repo.manifest(GENERATED);
        repo.write(
            "assets-src/blender-version.toml",
            b"version = \"5.2.2\"\nlinux_url = \"https://example.com/b.tar.xz\"\nlinux_sha256 = \"00\"\n",
        );
        repo.write("assets-src/generators/zinc_sheet.py", b"# the generator\n");
        for picture in ["diffuse", "normal", "roughness"] {
            repo.write(
                &format!("assets-src/generated/zinc/{picture}.jpg"),
                committed.as_bytes(),
            );
        }
        repo.write(
            "docs/images/materials-made-from-scratch.jpg",
            committed.as_bytes(),
        );
        repo
    }

    fn run(repo: &Repo, blender: &Path, args: &[&str]) -> (bool, String) {
        xtask(&repo.root, args, &[("BLENDER", blender.to_str().unwrap())])
    }

    #[test]
    fn generate_writes_each_textures_three_pictures_and_the_sheet_with_the_pinned_blender() {
        let repo = repo_with_a_texture("generate", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, text) = run(&repo, &blender, &["generate-textures"]);
        assert!(ok, "{text}");
        assert!(
            text.contains("zinc: 1024px, 0 KB (diffuse 0 KB, normal 0 KB, roughness 0 KB)"),
            "it says what each texture costs: {text}"
        );
        for picture in ["diffuse", "normal", "roughness"] {
            assert_eq!(
                repo.read(&format!("assets-src/generated/zinc/{picture}.jpg")),
                "new bytes"
            );
        }
        assert_eq!(
            repo.read("docs/images/materials-made-from-scratch.jpg"),
            "new bytes"
        );
    }

    #[test]
    fn generate_refuses_a_blender_that_isnt_the_pinned_version() {
        let repo = repo_with_a_texture("wrong-version", "old bytes");
        let blender = fake_blender(&repo, "5.1.0", "new bytes");
        let (ok, text) = run(&repo, &blender, &["generate-textures"]);
        assert!(!ok);
        assert!(
            text.contains(
                "is \"Blender 5.1.0 LTS\", but assets-src/blender-version.toml pins Blender 5.2.2"
            ),
            "{text}"
        );
        assert_eq!(
            repo.read("assets-src/generated/zinc/diffuse.jpg"),
            "old bytes"
        );
    }

    #[test]
    fn check_says_the_committed_pictures_are_what_the_generators_make_and_changes_nothing() {
        let repo = repo_with_a_texture("check-same", "the same");
        let blender = fake_blender(&repo, "5.2.2", "the same");
        let (ok, text) = run(&repo, &blender, &["generate-textures", "--check"]);
        assert!(ok, "{text}");
        assert!(
            text.contains("the committed pictures are what the generators make"),
            "{text}"
        );
        assert!(repo.exists("target/textures-check/textures/zinc/diffuse.jpg"));
    }

    #[test]
    fn check_fails_when_a_committed_picture_isnt_the_very_same_bytes() {
        let repo = repo_with_a_texture("check-differs", "out of date");
        let blender = fake_blender(&repo, "5.2.2", "rebuilt");
        let (ok, text) = run(&repo, &blender, &["generate-textures", "--check"]);
        assert!(!ok);
        assert!(
            text.contains(
                "assets-src/generated/zinc/normal.jpg (11 bytes) is not what zinc_sheet.py makes (7 bytes)"
            ),
            "{text}"
        );
        assert!(
            text.contains("download the ones CI made (the `textures` download"),
            "{text}"
        );
        assert_eq!(
            repo.read("assets-src/generated/zinc/normal.jpg"),
            "out of date"
        );
    }

    #[test]
    fn check_fails_for_a_missing_picture_and_for_a_folder_no_texture_makes() {
        let repo = repo_with_a_texture("check-missing", "the same");
        fs::remove_file(repo.root.join("assets-src/generated/zinc/roughness.jpg")).unwrap();
        repo.write("assets-src/generated/gone/diffuse.jpg", b"left over");
        let blender = fake_blender(&repo, "5.2.2", "the same");
        let (ok, text) = run(&repo, &blender, &["generate-textures", "--check"]);
        assert!(!ok);
        assert!(
            text.contains("assets-src/generated/zinc/roughness.jpg is missing"),
            "{text}"
        );
        assert!(
            text.contains("assets-src/generated/gone/ belongs to no texture"),
            "{text}"
        );
    }

    #[test]
    fn generate_removes_a_folder_no_texture_makes_any_more() {
        let repo = repo_with_a_texture("remove-left-overs", "old bytes");
        repo.write("assets-src/generated/gone/diffuse.jpg", b"left over");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, text) = run(&repo, &blender, &["generate-textures"]);
        assert!(ok, "{text}");
        assert!(!repo.exists("assets-src/generated/gone"), "{text}");
    }

    #[test]
    fn bad_options_are_refused_with_the_usage() {
        let repo = repo_with_a_texture("bad-options", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, text) = run(&repo, &blender, &["generate-textures", "--sideways"]);
        assert!(
            !ok && text.contains("Usage: cargo xtask generate-textures [--check]"),
            "{text}"
        );
    }
}
