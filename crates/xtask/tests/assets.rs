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
fn credits_list_each_cc_by_data_file_with_its_checksum_and_go_stale_when_one_changes() {
    let repo = Repo::new("credits-datasets");
    let (diffuse, d, normal, n) = pictures(&repo);
    repo.manifest((&diffuse, &d), (&normal, &n));
    repo.write(
        "assets-src/datasets.toml",
        br#"[[dataset]]
name    = "Some Flights"
page    = "https://example.org/flights"
by      = "A. Pilot"
licence = "CC-BY-4.0"
folder  = "data/flights"
"#,
    );
    repo.write("data/flights/NOTICE.md", b"# Credit\n");
    repo.write("data/flights/b.csv", b"time (s)\n0\n");
    repo.write("data/flights/a.csv", b"time (s)\n1\n");
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(ok, "{text}");
    let credits = repo.read("CREDITS.md");
    let a = sha256(b"time (s)\n1\n");
    let b = sha256(b"time (s)\n0\n");
    assert!(credits.contains(&format!(
        "| [Some Flights](https://example.org/flights) | A. Pilot | CC-BY-4.0 | [`a.csv`](data/flights/a.csv) | `{a}` |\n|  |  |  | [`b.csv`](data/flights/b.csv) | `{b}` |"
    )), "{credits}");
    assert!(!credits.contains("- Reference datasets under CC BY."));
    // A changed data file makes CREDITS.md out of date.
    repo.write("data/flights/a.csv", b"time (s)\n2\n");
    let (ok, text) = xtask(&repo.root, &["credits", "--check"], &[]);
    assert!(!ok && text.contains("CREDITS.md is out of date"), "{text}");
    // Only CC BY data enters the repo, and only with its notice beside it.
    repo.write(
        "assets-src/datasets.toml",
        br#"[[dataset]]
name    = "Some Flights"
page    = "https://example.org/flights"
by      = "A. Pilot"
licence = "CC-BY-NC-SA-3.0"
folder  = "data/flights"
"#,
    );
    fs::remove_file(repo.root.join("data/flights/NOTICE.md")).unwrap();
    let (ok, text) = xtask(&repo.root, &["credits"], &[]);
    assert!(!ok);
    assert!(
        text.contains(
            "\"Some Flights\" is under CC-BY-NC-SA-3.0, but only CC BY data enters the repo"
        ),
        "{text}"
    );
    assert!(
        text.contains("\"Some Flights\"'s folder data/flights has no NOTICE.md"),
        "{text}"
    );
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
        fake_blender_with_light(repo, version, bytes, "", false)
    }

    /// The same stand-in, for a Map whose script asks for baked light when
    /// `asks_for_light`: it writes the info file `--light-info` names, and
    /// when asked to `--bake` it writes an atlas where `--bake` says and
    /// `report` where `--bake-report` says.
    fn fake_blender_with_light(
        repo: &Repo,
        version: &str,
        bytes: &str,
        report: &str,
        asks_for_light: bool,
    ) -> PathBuf {
        let asks = if asks_for_light { "yes" } else { "" };
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "Blender {version} LTS"
  exit 0
fi
out=""; previews=""; info=""; bake=""; bake_report=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --previews) previews="$2"; shift ;;
    --light-info) info="$2"; shift ;;
    --bake) bake="$2"; shift ;;
    --bake-report) bake_report="$2"; shift ;;
  esac
  shift
done
printf '%s' '{bytes}' > "$out"
if [ -n "$previews" ]; then printf 'a picture' > "$previews/three-quarter.png"; printf 'a smaller picture' > "$previews/preview.png"; fi
if [ -n "{asks}" ] && [ -n "$info" ]; then printf 'texel_cm = 25.0\n' > "$info"; fi
if [ -n "$bake" ]; then printf 'an atlas' > "$bake"; printf '%s' '{report}' > "$bake_report"; fi
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

    const LIGHT_TOML: &str =
        "name = \"Yard\"\n\n[light]\nsun_elevation = \"37 °\"\nsky_colour = \"#a5c4f0\"\n";

    /// What `maps --bake` is told the bake was made from, as the Python
    /// script writes it: the `.glb`'s hash, and the `[light]` values' (the
    /// keys in order, each as `key = value` on a line).
    fn bake_report(glb: &str, light: &str) -> String {
        format!(
            "atlas = \"yard.light.exr\"\nglb_sha256 = \"{}\"\nlight_sha256 = \"{}\"\nwidth = 2048\n",
            sha256(glb.as_bytes()),
            sha256(light.as_bytes())
        )
    }

    const LIGHT_LINES: &str = "sky_colour = #a5c4f0\nsun_elevation = 37 °\n";

    fn repo_with_baked_light(case: &str) -> Repo {
        let repo = repo_with_a_map(case, "the glb");
        repo.write("packs/opendrone/maps/yard/map.toml", LIGHT_TOML.as_bytes());
        repo
    }

    #[test]
    fn check_says_a_map_that_asks_for_baked_light_has_no_atlas_until_it_is_baked() {
        let repo = repo_with_baked_light("maps-light-unbaked");
        let blender = fake_blender_with_light(&repo, "5.2.2", "the glb", "", true);
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(!ok, "{text}");
        assert!(
            text.contains("yard: its script asks for baked light, but there is no yard.bake.toml")
                && text.contains("run `cargo xtask maps --bake yard`"),
            "{text}"
        );
    }

    #[test]
    fn bake_writes_the_atlas_and_what_it_was_baked_from_and_then_check_passes() {
        let repo = repo_with_baked_light("maps-light-baked");
        let report = bake_report("the glb", LIGHT_LINES);
        let blender = fake_blender_with_light(&repo, "5.2.2", "the glb", &report, true);
        let (ok, text) = run(
            &repo,
            &blender,
            &["maps", "--bake", "yard", "--samples", "64"],
        );
        assert!(ok, "{text}");
        assert_eq!(
            repo.read("packs/opendrone/maps/yard/yard.light.exr"),
            "an atlas"
        );
        let sidecar = repo.read("packs/opendrone/maps/yard/yard.bake.toml");
        assert!(
            sidecar
                .starts_with("# Written by `cargo xtask maps --bake yard`; never edit it by hand.")
        );
        assert!(sidecar.contains(&format!("glb_sha256 = \"{}\"", sha256(b"the glb"))));
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(ok, "{text}");
        assert!(
            text.contains(
                "yard: the atlas yard.light.exr was baked from this .glb and these [light] values"
            ),
            "{text}"
        );
    }

    #[test]
    fn check_says_the_atlas_is_out_of_date_when_the_glb_or_the_light_changes() {
        let repo = repo_with_baked_light("maps-light-stale");
        let report = bake_report("the glb", LIGHT_LINES);
        let blender = fake_blender_with_light(&repo, "5.2.2", "the glb", &report, true);
        let (ok, text) = run(&repo, &blender, &["maps", "--bake", "yard"]);
        assert!(ok, "{text}");
        // The shapes changed: Blender now builds other bytes.
        let rebuilt = fake_blender_with_light(&repo, "5.2.2", "another glb", &report, true);
        let (ok, text) = run(&repo, &rebuilt, &["maps", "--check"]);
        assert!(!ok, "{text}");
        assert!(
            text.contains("the atlas yard.light.exr was baked from another yard.glb than the one Blender builds now"),
            "{text}"
        );
        // The sun moved in map.toml, with Blender building the same bytes
        // again (the stand-in is one file, so it is written once more).
        let blender = fake_blender_with_light(&repo, "5.2.2", "the glb", &report, true);
        repo.write(
            "packs/opendrone/maps/yard/map.toml",
            LIGHT_TOML.replace("37 °", "40 °").as_bytes(),
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(!ok, "{text}");
        assert!(
            text.contains("was baked with other [light] values than map.toml has now"),
            "{text}"
        );
        // The atlas file went missing.
        repo.write("packs/opendrone/maps/yard/map.toml", LIGHT_TOML.as_bytes());
        fs::remove_file(repo.root.join("packs/opendrone/maps/yard/yard.light.exr")).unwrap();
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(
            !ok && text
                .contains("names the atlas yard.light.exr, but the file isn't in the Map's folder"),
            "{text}"
        );
    }

    #[test]
    fn the_second_uv_set_is_proven_by_a_committed_hash_which_a_plain_run_rewrites() {
        let repo = repo_with_a_map("maps-uv-proof", "the glb");
        repo.write("assets-src/lightmap-uv.toml", b"[maps]\nyard = \"00\"\n");
        let blender = fake_blender(&repo, "5.2.2", "the glb");
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(!ok, "{text}");
        assert!(
            text.contains("yard: the .glb built with a second UV set at 25 cm per texel hashes to"),
            "{text}"
        );
        let (ok, text) = run(&repo, &blender, &["maps"]);
        assert!(ok, "{text}");
        assert!(
            repo.read("assets-src/lightmap-uv.toml")
                .contains(&sha256(b"the glb"))
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--check"]);
        assert!(ok, "{text}");
        assert!(
            text.contains("yard: the second UV set is the very same bytes"),
            "{text}"
        );
    }

    #[test]
    fn bake_refuses_a_map_that_isnt_there_and_options_that_dont_go_with_it() {
        let repo = repo_with_baked_light("maps-bake-options");
        let blender = fake_blender(&repo, "5.2.2", "the glb");
        let (ok, text) = run(&repo, &blender, &["maps", "--bake", "nowhere"]);
        assert!(
            !ok && text.contains("there's no Map with that id; the Maps with a script are yard"),
            "{text}"
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--bake", "yard", "--check"]);
        assert!(
            !ok && text.contains("not with --check or --previews"),
            "{text}"
        );
        let (ok, text) = run(&repo, &blender, &["maps", "--samples", "8"]);
        assert!(!ok && text.contains("--samples is for --bake"), "{text}");
    }

    #[test]
    fn the_repo_proves_the_second_uv_set_of_every_map_with_a_script() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let proof = fs::read_to_string(repo.join("assets-src/lightmap-uv.toml")).unwrap();
        for entry in fs::read_dir(repo.join("assets-src/maps")).unwrap() {
            let id = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                proof.contains(&format!("{id} = \"")),
                "assets-src/lightmap-uv.toml has no hash for {id}"
            );
        }
    }
}
