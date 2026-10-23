//! Readable checks for `cargo xtask play` (#298). Each runs the real command
//! on a scratch repo with `--no-launch`, so nothing builds the game, and with
//! stand-ins for `rustc` and `cmake` (small shell scripts first on the path),
//! so the checks don't depend on what is installed. Downloads use `file://`
//! addresses, which `curl` reads like any other, so nothing here touches the
//! network. Unix only: what they prove (one plain sentence for each thing
//! that's missing, nothing refetched, no tracked file changed) doesn't depend
//! on the operating system. Basis: Rule (#298: a missing toolchain, Blender
//! or network each give one plain sentence; a second run fetches nothing;
//! the command never changes tracked files).

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

const PIN: &str = "1.99.1";

struct Repo {
    root: PathBuf,
    /// The folder of stand-in tools, first on the path.
    tools: PathBuf,
}

impl Repo {
    /// A scratch repo with a Rust pin, one texture, one Map and the Blender
    /// pin, all committed to git, and stand-ins for `rustc` and `cmake`.
    fn new(case: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("play")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let repo = Repo {
            tools: root.join("tools"),
            root,
        };
        fs::create_dir_all(&repo.tools).unwrap();
        repo.tool("rustc", &format!("echo 'rustc {PIN} (0000 2026-01-01)'"));
        repo.tool("cmake", "echo 'cmake version 3.30.0'");
        repo.write(
            "rust-toolchain.toml",
            format!("[toolchain]\nchannel = \"{PIN}\"\n").as_bytes(),
        );
        let picture = repo.write("source/diffuse.jpg", b"the colour picture");
        let normal = repo.write("source/normal.jpg", b"the normal map");
        repo.write(
            "assets-src/materials.toml",
            repo.manifest(&picture, &normal).as_bytes(),
        );
        repo.write(
            "assets-src/blender-version.toml",
            b"version = \"5.2.2\"\nlinux_url = \"https://example.com/b.tar.xz\"\nlinux_sha256 = \"00\"\n",
        );
        repo.write("assets-src/maps/yard/build.py", b"# the Map's script\n");
        repo.write("packs/opendrone/maps/yard/map.toml", b"id = \"yard\"\n");
        repo.write("packs/opendrone/maps/yard/yard.glb", b"committed bytes");
        // The stand-in tools and the scratch pictures aren't part of the repo.
        repo.write(".gitignore", b"tools/\nsource/\nassets-src/textures/\n");
        repo.git(&["init", "-q"]);
        repo.git(&["add", "."]);
        repo.git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "-m",
            "start",
        ]);
        repo
    }

    fn tool(&self, name: &str, body: &str) -> PathBuf {
        let path = self.tools.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn write(&self, file: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    fn read(&self, file: &str) -> Vec<u8> {
        fs::read(self.root.join(file)).unwrap()
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn manifest(&self, diffuse: &Path, normal: &Path) -> String {
        format!(
            r#"[materials.concrete_plaza]
colour    = [0.40, 0.40, 0.38]
roughness = 0.90
texture   = "worn"

[textures.worn]
source  = "Poly Haven"
page    = "https://polyhaven.com/a/worn"
author  = "A. Photographer"
licence = "CC0-1.0"
size    = "3 m"
diffuse = {{ url = "{}", sha256 = "{}" }}
normal  = {{ url = "{}", sha256 = "{}" }}
"#,
            file_url(diffuse),
            sha256(b"the colour picture"),
            file_url(normal),
            sha256(b"the normal map"),
        )
    }

    /// Runs `cargo xtask play --no-launch <args>` with only the stand-ins and
    /// the system's own tools on the path: whether it passed, and what it
    /// printed.
    fn play(&self, args: &[&str]) -> (bool, String) {
        self.play_with(
            args,
            &format!("{}:/usr/bin:/bin", self.tools.display()),
            None,
        )
    }

    fn play_with(&self, args: &[&str], path: &str, blender: Option<&Path>) -> (bool, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .arg("play")
            .arg("--no-launch")
            .args(args)
            .current_dir(&self.root)
            .env("PATH", path)
            .env_remove("BLENDER");
        if let Some(blender) = blender {
            command.env("BLENDER", blender);
        }
        let output = command.output().expect("xtask runs");
        let text = String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr);
        (output.status.success(), text)
    }

    /// Like [`Repo::play`], with this Blender found first.
    fn play_with_blender(&self, blender: &Path) -> (bool, String) {
        self.play_with(
            &[],
            &format!("{}:/usr/bin:/bin", self.tools.display()),
            Some(blender),
        )
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

/// A stand-in for Blender that says it is `version` and writes `bytes`.
fn fake_blender(repo: &Repo, version: &str, bytes: &str) -> PathBuf {
    repo.tool(
        "fake-blender",
        &format!(
            r#"if [ "$1" = "--version" ]; then echo "Blender {version} LTS"; exit 0; fi
out=""
while [ $# -gt 0 ]; do
  case "$1" in --out) out="$2"; shift ;; esac
  shift
done
printf '%s' '{bytes}' > "$out""#
        ),
    )
}

/// An edit made after the `.glb` was written, as a pilot's own edit is.
fn edit_the_maps_script(repo: &Repo) {
    sleep(Duration::from_millis(50));
    repo.write(
        "assets-src/maps/yard/build.py",
        b"# the Map's script, edited\n",
    );
}

fn plain(text: &str) {
    assert!(
        !text.contains("panicked") && !text.contains("RUST_BACKTRACE"),
        "no stack trace: {text}"
    );
}

#[test]
fn a_fresh_checkout_gets_its_textures_and_changes_no_tracked_file() {
    let repo = Repo::new("fresh");
    let (ok, text) = repo.play(&[]);
    assert!(ok, "{text}");
    assert!(text.contains("Fetched 2 pictures"), "{text}");
    assert_eq!(
        repo.read("assets-src/textures/worn/diffuse.jpg"),
        b"the colour picture"
    );
    assert!(
        text.contains("committed .glb files of yard are what the scripts build"),
        "{text}"
    );
    // Nothing tracked changed, and nothing new but what git ignores.
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo.root)
        .output()
        .unwrap();
    assert!(status.stdout.is_empty(), "{status:?}");
}

#[test]
fn a_second_run_fetches_nothing() {
    let repo = Repo::new("second-run");
    let (ok, _) = repo.play(&[]);
    assert!(ok);
    let (ok, text) = repo.play(&[]);
    assert!(ok, "{text}");
    assert!(
        text.contains("All 2 pictures were already there: nothing fetched."),
        "{text}"
    );
}

#[test]
fn no_network_starts_the_game_in_flat_colours_and_says_so_in_one_sentence() {
    let repo = Repo::new("no-network");
    let gone = repo.root.join("source/gone.jpg");
    let text = repo.manifest(&gone, &repo.root.join("source/normal.jpg"));
    repo.write("assets-src/materials.toml", text.as_bytes());
    let (ok, text) = repo.play(&[]);
    assert!(ok, "the game still starts: {text}");
    plain(&text);
    assert!(
        text.contains("Skipped: the textures couldn't be fetched")
            && text.contains("flat colours this time"),
        "{text}"
    );
    assert!(text.contains("4/4 Game"), "{text}");
}

#[test]
fn a_missing_toolchain_is_one_sentence_that_says_how_to_get_it() {
    let repo = Repo::new("no-rust");
    let empty = repo.root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let (ok, text) = repo.play_with(&[], empty.to_str().unwrap(), None);
    assert!(!ok);
    plain(&text);
    assert!(
        text.contains("Rust isn't installed here: get rustup from https://rustup.rs")
            && text.contains(PIN),
        "{text}"
    );
    assert!(!text.contains("2/4"), "it stops before fetching: {text}");
}

#[test]
fn a_rust_that_isnt_the_pinned_one_says_which_and_how_to_fix_it() {
    let repo = Repo::new("wrong-rust");
    repo.tool("rustc", "echo 'rustc 1.80.0 (0000 2024-01-01)'");
    let (ok, text) = repo.play(&[]);
    assert!(!ok);
    plain(&text);
    assert!(
        text.contains(&format!("pins Rust {PIN}")) && text.contains("run `rustup show`"),
        "{text}"
    );
}

#[test]
fn a_missing_cmake_is_one_sentence_that_says_how_to_get_it() {
    let repo = Repo::new("no-cmake");
    fs::remove_file(repo.tools.join("cmake")).unwrap();
    let (ok, text) = repo.play(&[]);
    // A cmake in /usr/bin would be found, and the check right to pass.
    if Path::new("/usr/bin/cmake").exists() {
        assert!(ok, "{text}");
        return;
    }
    assert!(!ok);
    plain(&text);
    assert!(
        text.contains("CMake isn't installed") && text.contains("install it with"),
        "{text}"
    );
}

#[test]
fn a_script_edited_here_is_rebuilt_with_the_pinned_blender() {
    let repo = Repo::new("rebuild");
    let blender = fake_blender(&repo, "5.2.2", "rebuilt bytes");
    edit_the_maps_script(&repo);
    let (ok, text) = repo.play_with_blender(&blender);
    assert!(ok, "{text}");
    assert!(
        text.contains("yard: wrote packs/opendrone/maps/yard/yard.glb"),
        "{text}"
    );
    assert_eq!(
        repo.read("packs/opendrone/maps/yard/yard.glb"),
        b"rebuilt bytes"
    );
}

#[test]
fn a_blender_that_isnt_the_pinned_version_leaves_the_committed_glb_and_says_so() {
    let repo = Repo::new("wrong-blender");
    let blender = fake_blender(&repo, "4.0.0", "wrong bytes");
    edit_the_maps_script(&repo);
    let (ok, text) = repo.play_with_blender(&blender);
    assert!(ok, "the game still starts: {text}");
    plain(&text);
    assert!(
        text.contains("Skipped: yard has a script edited here, but Blender can't rebuild it"),
        "{text}"
    );
    assert_eq!(
        repo.read("packs/opendrone/maps/yard/yard.glb"),
        b"committed bytes"
    );
}

#[test]
fn no_blender_leaves_the_committed_glb_and_says_so() {
    // A real Blender where each system installs it would be found, and the
    // Map rightly rebuilt, so this check can't stand in for a missing one.
    if Path::new("/Applications/Blender.app").exists() {
        return;
    }
    let repo = Repo::new("no-blender");
    edit_the_maps_script(&repo);
    let (ok, text) = repo.play(&[]);
    assert!(ok, "the game still starts: {text}");
    plain(&text);
    if text.contains("wrote packs/") {
        return; // a Blender of the right version is on this computer's path
    }
    assert!(
        text.contains("Skipped: yard has a script edited here, but Blender can't rebuild it"),
        "{text}"
    );
    assert_eq!(
        repo.read("packs/opendrone/maps/yard/yard.glb"),
        b"committed bytes"
    );
}

#[test]
fn a_script_that_is_the_committed_one_is_never_rebuilt_even_when_newer_than_the_glb() {
    // What a fresh checkout looks like: file times are the checkout's, in no
    // order that means anything.
    let repo = Repo::new("untouched-script");
    sleep(Duration::from_millis(50));
    let script = repo.root.join("assets-src/maps/yard/build.py");
    let touched = fs::OpenOptions::new().write(true).open(&script).unwrap();
    touched.set_modified(SystemTime::now()).unwrap();
    let (ok, text) = repo.play(&[]);
    assert!(ok, "{text}");
    assert!(text.contains("used as they are"), "{text}");
}

#[test]
fn a_map_that_doesnt_exist_is_named_with_the_ones_that_do() {
    let repo = Repo::new("bad-map");
    let (ok, text) = repo.play(&["--map", "nowhere"]);
    assert!(!ok);
    plain(&text);
    assert!(
        text.contains("There is no Map called nowhere: the Maps are flat-floor, yard."),
        "{text}"
    );
    let (ok, _) = repo.play(&["--map", "yard"]);
    assert!(ok);
}
