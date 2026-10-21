//! Readable checks for the asset commands (#63): `cargo xtask textures`,
//! `cargo xtask credits` and `cargo xtask maps`. Each runs the real command
//! on a scratch repo. Downloads use `file://` addresses, which `curl` reads
//! like any other, so nothing here touches the network. The Maps check uses
//! a stand-in for Blender, a small shell script, so it runs on Unix only:
//! what it proves (the version is read from one file, the same bytes are
//! required) doesn't depend on the operating system. Basis: Rule (ADR-0014:
//! textures are CC0, fetched by a manifest, checksummed; #63: Blender is
//! pinned in one file, and the Map's `.glb` must be the same bytes).

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

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

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A `file://` address for a path, as `curl` reads it on every system.
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

/// A scratch repo: just `assets-src/`, where the commands find the repo's
/// root.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(case: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("assets")
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

    /// A manifest of one material using one texture whose two pictures are
    /// the files at `diffuse` and `normal`, with these checksums.
    fn manifest(&self, diffuse: (&Path, &str), normal: (&Path, &str)) -> String {
        let text = format!(
            r#"# A comment the commands must keep.
[materials.concrete_plaza]
colour    = [0.40, 0.40, 0.38]
roughness = 0.90
texture   = "worn"

[textures.worn]
source  = "Poly Haven"
page    = "https://polyhaven.com/a/worn"
author  = "A. Photographer"
licence = "CC0-1.0"
size    = "3 m"
diffuse = {{ url = "{}", sha256 = "{}" }}   # the colour picture
normal  = {{ url = "{}", sha256 = "{}" }}
"#,
            file_url(diffuse.0),
            diffuse.1,
            file_url(normal.0),
            normal.1
        );
        self.write("assets-src/materials.toml", text.as_bytes());
        text
    }
}

/// Two pictures in the scratch repo, and their checksums.
fn pictures(repo: &Repo) -> (PathBuf, String, PathBuf, String) {
    let diffuse = repo.write("source/diffuse.jpg", b"the colour picture");
    let normal = repo.write("source/normal.jpg", b"the normal map");
    (
        diffuse,
        sha256(b"the colour picture"),
        normal,
        sha256(b"the normal map"),
    )
}

// textures

#[test]
fn textures_are_downloaded_checked_against_their_checksum_and_kept_apart_from_git() {
    let repo = Repo::new("textures-fetch");
    let (diffuse, d, normal, n) = pictures(&repo);
    repo.manifest((&diffuse, &d), (&normal, &n));
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(ok, "{text}");
    assert!(text.contains("fetched worn/diffuse"), "{text}");
    assert!(text.contains("fetched worn/normal"), "{text}");
    assert_eq!(
        fs::read(repo.root.join("assets-src/textures/worn/diffuse.jpg")).unwrap(),
        b"the colour picture"
    );
    // A second run has them already, and fetches nothing.
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(
        ok && text.contains("have    worn/diffuse") && !text.contains("fetched"),
        "{text}"
    );
}

#[test]
fn a_texture_whose_checksum_differs_is_refused_and_nothing_is_kept() {
    let repo = Repo::new("textures-refused");
    let (diffuse, _, normal, n) = pictures(&repo);
    let wrong = sha256(b"some other picture");
    repo.manifest((&diffuse, &wrong), (&normal, &n));
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok, "{text}");
    assert!(
        text.contains(&format!(
            "has the SHA-256 {}, but assets-src/materials.toml says {wrong}",
            sha256(b"the colour picture")
        )),
        "{text}"
    );
    assert!(
        text.contains("it was refused, and nothing was kept"),
        "{text}"
    );
    assert!(!repo.exists("assets-src/textures/worn/diffuse.jpg"));
}

#[test]
fn a_picture_on_disk_with_the_wrong_checksum_is_fetched_again() {
    let repo = Repo::new("textures-replaced");
    let (diffuse, d, normal, n) = pictures(&repo);
    repo.manifest((&diffuse, &d), (&normal, &n));
    repo.write("assets-src/textures/worn/diffuse.jpg", b"damaged");
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(ok, "{text}");
    assert!(text.contains("fetched worn/diffuse"), "{text}");
    assert!(text.contains("fetched worn/normal"), "{text}");
    assert_eq!(
        fs::read(repo.root.join("assets-src/textures/worn/diffuse.jpg")).unwrap(),
        b"the colour picture"
    );
}

#[test]
fn a_texture_with_no_checksum_needs_pin_and_pin_writes_it_keeping_the_comments() {
    let repo = Repo::new("textures-pin");
    let (diffuse, _, normal, _) = pictures(&repo);
    let before = repo.manifest((&diffuse, ""), (&normal, ""));
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains(
            "has no checksum in assets-src/materials.toml: run `cargo xtask textures --pin`"
        ),
        "{text}"
    );
    let (ok, text) = xtask(&repo.root, &["textures", "--pin"], &[]);
    assert!(ok && text.contains("pinned  2 checksum(s)"), "{text}");
    let after = repo.read("assets-src/materials.toml");
    assert_eq!(
        after,
        before
            .replacen(
                "sha256 = \"\"",
                &format!("sha256 = \"{}\"", sha256(b"the colour picture")),
                1
            )
            .replacen(
                "sha256 = \"\"",
                &format!("sha256 = \"{}\"", sha256(b"the normal map")),
                1
            ),
        "only the checksums changed: comments and layout stay"
    );
    // Pinned, it is checked like any other.
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(ok && text.contains("have    worn/diffuse"), "{text}");
}

#[test]
fn pin_never_changes_a_checksum_that_is_there() {
    let repo = Repo::new("textures-pin-keeps");
    let (diffuse, d, normal, _) = pictures(&repo);
    let wrong = sha256(b"not the normal map");
    repo.manifest((&diffuse, &d), (&normal, &wrong));
    let (ok, text) = xtask(&repo.root, &["textures", "--pin"], &[]);
    assert!(!ok && text.contains("it was refused"), "{text}");
    assert!(repo.read("assets-src/materials.toml").contains(&wrong));
}

#[test]
fn a_file_inside_a_zip_is_taken_out_and_checked() {
    let repo = Repo::new("textures-zip");
    let (diffuse, d, _, _) = pictures(&repo);
    // ambientCG gives a zip: the normal map is a file inside it.
    let zip_path = repo.root.join("source/pack.zip");
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "Worn_1K-JPG_NormalGL.jpg",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"the normal map").unwrap();
        writer.finish().unwrap();
    }
    let mut manifest = repo.manifest((&diffuse, &d), (&zip_path, &sha256(b"the normal map")));
    manifest = manifest.replacen(
        &format!("url = \"{}\", sha256", file_url(&zip_path)),
        &format!(
            "url = \"{}\", in_zip = \"Worn_1K-JPG_NormalGL.jpg\", sha256",
            file_url(&zip_path)
        ),
        1,
    );
    repo.write("assets-src/materials.toml", manifest.as_bytes());
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(ok, "{text}");
    assert_eq!(
        fs::read(repo.root.join("assets-src/textures/worn/normal.jpg")).unwrap(),
        b"the normal map"
    );
    // A name the zip doesn't hold.
    repo.write(
        "assets-src/materials.toml",
        manifest
            .replace("Worn_1K-JPG_NormalGL.jpg", "Missing.jpg")
            .as_bytes(),
    );
    let _ = fs::remove_file(repo.root.join("assets-src/textures/worn/normal.jpg"));
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(
        !ok && text.contains("has no file called Missing.jpg"),
        "{text}"
    );
}

#[test]
fn only_cc0_pictures_from_poly_haven_and_ambientcg_are_listed() {
    let repo = Repo::new("textures-licences");
    let (diffuse, d, normal, n) = pictures(&repo);
    let manifest = repo.manifest((&diffuse, &d), (&normal, &n));
    repo.write(
        "assets-src/materials.toml",
        manifest.replace("CC0-1.0", "CC-BY-4.0").as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains("is under CC-BY-4.0, but only CC0 pictures enter the repo (ADR-0014)"),
        "{text}"
    );
    repo.write(
        "assets-src/materials.toml",
        manifest.replace("Poly Haven", "Some Photo Site").as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok);
    assert!(
        text.contains(
            "comes from \"Some Photo Site\", but textures come only from Poly Haven and ambientCG"
        ),
        "{text}"
    );
}

#[test]
fn a_texture_no_material_uses_and_a_material_with_no_texture_entry_are_refused() {
    let repo = Repo::new("textures-unused");
    let (diffuse, d, normal, n) = pictures(&repo);
    let manifest = repo.manifest((&diffuse, &d), (&normal, &n));
    repo.write(
        "assets-src/materials.toml",
        manifest.replace("texture   = \"worn\"", "").as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(
        !ok && text.contains("the texture \"worn\" is listed but no material uses it"),
        "{text}"
    );
    repo.write(
        "assets-src/materials.toml",
        manifest
            .replace("texture   = \"worn\"", "texture   = \"missing\"")
            .as_bytes(),
    );
    let (ok, text) = xtask(&repo.root, &["textures"], &[]);
    assert!(!ok && text.contains("the material \"concrete_plaza\" uses the texture \"missing\", which [textures] doesn't list"), "{text}");
}

// credits

#[test]
fn credits_are_generated_from_the_manifest_and_checked_for_being_up_to_date() {
    let repo = Repo::new("credits");
    let (diffuse, d, normal, n) = pictures(&repo);
    repo.manifest((&diffuse, &d), (&normal, &n));
    // Not there yet: the check says so, and the command writes it.
    let (ok, text) = xtask(&repo.root, &["credits", "--check"], &[]);
    assert!(!ok && text.contains("CREDITS.md is out of date"), "{text}");
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(ok && text.contains("Wrote CREDITS.md"), "{text}");
    let credits = repo.read("CREDITS.md");
    assert!(credits.contains("# Credits"));
    assert!(credits.contains("**It is generated**"));
    assert!(credits.contains(&format!(
        "| [worn](https://polyhaven.com/a/worn) | Poly Haven | A. Photographer | CC0-1.0 | diffuse | `{d}` |"
    )), "{credits}");
    assert!(
        credits.contains(&format!("|  |  |  |  | normal | `{n}` |")),
        "{credits}"
    );
    let (ok, text) = xtask(&repo.root, &["credits", "--check"], &[]);
    assert!(ok && text.contains("up to date"), "{text}");
    // A second write changes nothing.
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(ok && text.contains("already up to date"), "{text}");
    // A hand edit, or a changed manifest, makes it stale.
    repo.write(
        "CREDITS.md",
        format!("{credits}A hand-written line.\n").as_bytes(),
    );
    let (ok, _) = xtask(&repo.root, &["credits", "--check"], &[]);
    assert!(!ok);
}

#[test]
fn credits_cant_list_a_file_with_no_checksum() {
    let repo = Repo::new("credits-unpinned");
    let (diffuse, _, normal, n) = pictures(&repo);
    repo.manifest((&diffuse, ""), (&normal, &n));
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(!ok);
    assert!(text.contains("worn's diffuse file has no checksum yet, so CREDITS.md can't list it: run `cargo xtask textures --pin`"), "{text}");
    assert!(!repo.exists("CREDITS.md"));
}

#[test]
fn the_built_in_credits_are_up_to_date_with_the_real_material_library() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, text) = xtask(&repo, &["credits", "--check"], &[]);
    assert!(ok, "{text}");
}

// maps (with a stand-in for Blender)

#[cfg(unix)]
mod maps {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in for Blender: a shell script that says it is `version` and,
    /// asked to run a script, writes `bytes` where `--out` says (and a
    /// picture where `--previews` says).
    fn fake_blender(repo: &Repo, version: &str, bytes: &str) -> PathBuf {
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "Blender {version} LTS"
  exit 0
fi
out=""; previews=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --previews) previews="$2"; shift ;;
  esac
  shift
done
printf '%s' '{bytes}' > "$out"
if [ -n "$previews" ]; then printf 'a picture' > "$previews/three-quarter.png"; printf 'a smaller picture' > "$previews/preview.png"; fi
"#
        );
        let path = repo.write("fake-blender", script.as_bytes());
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    /// A scratch repo with one Map script and the Blender pin.
    fn repo_with_a_map(case: &str, committed: &str) -> Repo {
        let repo = Repo::new(case);
        repo.write(
            "assets-src/blender-version.toml",
            b"version = \"5.2.2\"\nlinux_url = \"https://example.com/b.tar.xz\"\nlinux_sha256 = \"00\"\n",
        );
        repo.write("assets-src/maps/yard/build.py", b"# the Map's script\n");
        repo.write("packs/opendrone/maps/yard/yard.glb", committed.as_bytes());
        repo
    }

    fn run(repo: &Repo, blender: &Path, args: &[&str]) -> (bool, String) {
        xtask(&repo.root, args, &[("BLENDER", blender.to_str().unwrap())])
    }

    #[test]
    fn maps_writes_each_maps_glb_into_the_built_in_pack_with_the_pinned_blender() {
        let repo = repo_with_a_map("maps-build", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, text) = run(&repo, &blender, &["maps"]);
        assert!(ok, "{text}");
        assert!(
            text.contains("yard: wrote packs/opendrone/maps/yard/yard.glb (9 bytes)"),
            "{text}"
        );
        assert_eq!(repo.read("packs/opendrone/maps/yard/yard.glb"), "new bytes");
    }

    #[test]
    fn maps_refuses_a_blender_that_isnt_the_pinned_version() {
        let repo = repo_with_a_map("maps-wrong-version", "old bytes");
        let blender = fake_blender(&repo, "5.1.0", "new bytes");
        let (ok, text) = run(&repo, &blender, &["maps"]);
        assert!(!ok);
        assert!(
            text.contains(
                "is \"Blender 5.1.0 LTS\", but assets-src/blender-version.toml pins Blender 5.2.2"
            ),
            "{text}"
        );
        assert_eq!(repo.read("packs/opendrone/maps/yard/yard.glb"), "old bytes");
    }

    #[test]
    fn check_says_the_committed_glb_is_what_blender_builds_and_changes_nothing() {
        let repo = repo_with_a_map("maps-check-same", "the same");
        let blender = fake_blender(&repo, "5.2.2", "the same");
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(ok, "{text}");
        assert!(
            text.contains("yard: the committed .glb is what Blender builds (8 bytes)"),
            "{text}"
        );
        assert!(repo.exists("target/maps-check/yard.glb"));
    }

    #[test]
    fn check_fails_when_the_committed_glb_isnt_the_very_same_bytes() {
        let repo = repo_with_a_map("maps-check-differs", "out of date");
        let blender = fake_blender(&repo, "5.2.2", "rebuilt");
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(!ok);
        assert!(
            text.contains("yard: the committed packs/opendrone/maps/yard/yard.glb (11 bytes) is not what Blender builds from assets-src/maps/yard/ (7 bytes)"),
            "{text}"
        );
        assert!(
            text.contains("download the one CI built (the `maps` artifact"),
            "{text}"
        );
        // Nothing in the Pack changed.
        assert_eq!(
            repo.read("packs/opendrone/maps/yard/yard.glb"),
            "out of date"
        );
    }

    #[test]
    fn previews_are_rendered_into_a_folder_per_map_and_can_become_the_preview_picture() {
        let repo = repo_with_a_map("maps-previews", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, text) = run(
            &repo,
            &blender,
            &["maps", "--previews", "target/pictures", "--update-preview"],
        );
        assert!(ok, "{text}");
        assert_eq!(
            repo.read("target/pictures/yard/three-quarter.png"),
            "a picture"
        );
        assert_eq!(
            repo.read("packs/opendrone/maps/yard/preview.png"),
            "a smaller picture"
        );
        // Without --update-preview the Pack's picture is left alone.
        let repo = repo_with_a_map("maps-previews-only", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        let (ok, _) = run(&repo, &blender, &["maps", "--previews", "target/pictures"]);
        assert!(ok);
        assert!(!repo.exists("packs/opendrone/maps/yard/preview.png"));
    }

    #[test]
    fn a_bad_pin_file_or_bad_options_are_refused_with_a_sentence() {
        let repo = repo_with_a_map("maps-bad-pin", "old bytes");
        let blender = fake_blender(&repo, "5.2.2", "new bytes");
        repo.write("assets-src/blender-version.toml", b"version = \"5.2\"\n");
        let (ok, text) = run(&repo, &blender, &["maps"]);
        assert!(
            !ok && text.contains("version \"5.2\" must be three whole numbers joined by dots"),
            "{text}"
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--update-preview"]);
        assert!(
            !ok && text.contains("--update-preview needs --previews"),
            "{text}"
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--sideways"]);
        assert!(
            !ok && text.contains("unknown argument \"--sideways\""),
            "{text}"
        );
    }

    #[test]
    fn the_pinned_version_in_the_repo_is_the_one_the_docs_and_ci_name() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = fs::read_to_string(repo.join("assets-src/blender-version.toml")).unwrap();
        assert!(pin.contains("version = \"5.2.2\""));
        // CI reads the download and its checksum from this very file.
        assert!(pin.contains("linux_url") && pin.contains("blender-5.2.2-linux-x64.tar.xz"));
        let ci = fs::read_to_string(repo.join(".github/workflows/ci.yml")).unwrap();
        assert!(ci.contains("assets-src/blender-version.toml"));
        assert!(ci.contains("cargo xtask maps --check"));
    }
}
