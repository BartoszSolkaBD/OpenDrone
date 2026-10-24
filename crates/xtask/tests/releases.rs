//! Readable checks for builds, releases and updates (#81, #15 §8,
//! ADR-0021): `cargo xtask release-version`, `package`, `bevy-upgrade` and
//! `advisories`, each run as the workflows run it, on a scratch repo; and
//! the Dependabot settings, read from the repo itself.
//!
//! The fixtures are real: `bevy-index.json` is the end of crates.io's index
//! entry for `bevy` on 21 October 2026, with each version's dependency list
//! left out, and `deny-advisories.json` is what `cargo deny --format json
//! check advisories` printed for this repo with deny.toml's two accepted
//! advisories not ignored.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/releases")
        .join(name)
}

/// The repo's own root.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs `cargo xtask <args>` in `folder`: whether it passed, what it printed
/// to stdout, and to stderr.
fn xtask(folder: &Path, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(folder)
        .output()
        .expect("xtask runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A scratch repo, emptied first, with `Cargo.toml` and `.github/`, where
/// the commands find the repo's root.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(case: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("releases")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".github")).unwrap();
        let repo = Repo { root };
        repo.write(
            "Cargo.toml",
            "[workspace]\n\n[workspace.package]\nversion = \"0.1.0-alpha.0\"\n\n\
             [workspace.dependencies]\nbevy = { version = \"=0.20.0\", default-features = false }\n",
        );
        repo
    }

    fn write(&self, file: &str, text: &str) -> PathBuf {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    fn run(&self, args: &[&str]) -> (bool, String, String) {
        xtask(&self.root, args)
    }

    /// Lays out what a download carries: two Packs files, a Scenario, the
    /// material library with one texture, the licences and the credits.
    fn with_the_games_files(&self) {
        self.write("packs/opendrone/pack.toml", "id = \"opendrone\"\n");
        self.write("packs/opendrone/maps/skate-park/skate-park.glb", "glTF");
        self.write("packs/opendrone/.DS_Store", "Finder's");
        self.write("scenarios/physics/free-fall.toml", "# a Scenario\n");
        self.write(
            "assets-src/materials.toml",
            "[materials.concrete]\ntexture = \"concrete\"\n\n\
             [textures.concrete]\nsource = \"Poly Haven\"\npage = \"https://polyhaven.com/a/concrete\"\n\
             author = \"Someone\"\nlicence = \"CC0-1.0\"\nsize = \"2 m\"\n\
             diffuse = { url = \"https://example.com/d.jpg\", sha256 = \"\" }\n\
             normal = { url = \"https://example.com/n.jpg\", sha256 = \"\" }\n",
        );
        self.write(
            "assets-src/maps/skate-park/build.py",
            "# Blender's script\n",
        );
        self.write("CREDITS.md", "# Credits\n");
        self.write("LICENSE-MIT", "MIT\n");
        self.write("LICENSE-APACHE", "Apache\n");
        for os in ["macos", "windows", "linux"] {
            self.write(
                &format!("licences/third-party-{os}.txt"),
                &format!("The libraries' licences, for {os}\n"),
            );
        }
        self.write("built/opendrone", "the game's program");
        self.write("crates/opendrone/icon/opendrone.png", "PNG icon");
        self.write("crates/opendrone/icon/opendrone.icns", "icns icon");
        self.track();
    }

    /// Makes the scratch repo a git repo, if it isn't yet, and has git track
    /// every file that isn't ignored: a download takes only tracked files.
    fn track(&self) {
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .output()
                .expect("git runs");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        if !self.root.join(".git").exists() {
            git(&["init", "--quiet"]);
        }
        git(&["add", "--all"]);
    }

    fn with_the_textures(&self) {
        self.write("assets-src/textures/concrete/diffuse.jpg", "JPEG d");
        self.write("assets-src/textures/concrete/normal.jpg", "JPEG n");
    }
}

/// Each entry of a download, a zip or a gzipped tarball: its name, its Unix
/// permissions and its bytes. A folder's name ends in `/`.
fn unpack(path: &Path) -> Vec<(String, Option<u32>, Vec<u8>)> {
    if path.to_string_lossy().ends_with(".tar.gz") {
        untar(path)
    } else {
        unzip(path)
    }
}

/// Each entry of a gzipped tarball, read by the POSIX (ustar) layout.
fn untar(path: &Path) -> Vec<(String, Option<u32>, Vec<u8>)> {
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(path).unwrap())
        .read_to_end(&mut tar)
        .unwrap();
    let text =
        |b: &[u8]| String::from_utf8(b.iter().copied().take_while(|&c| c != 0).collect()).unwrap();
    let octal = |b: &[u8]| u64::from_str_radix(text(b).trim(), 8).unwrap();
    let mut entries = Vec::new();
    let mut at = 0;
    while at + 512 <= tar.len() && tar[at..at + 512].iter().any(|&b| b != 0) {
        let header = &tar[at..at + 512];
        assert_eq!(&header[257..263], b"ustar\0", "a POSIX tar header");
        let stored: u32 = header[..148]
            .iter()
            .chain(&[b' '; 8])
            .chain(&header[156..])
            .map(|&b| u32::from(b))
            .sum();
        assert_eq!(u64::from(stored), octal(&header[148..156]), "its checksum");
        let (prefix, name) = (text(&header[345..500]), text(&header[..100]));
        let name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = octal(&header[124..136]) as usize;
        let mode = octal(&header[100..108]) as u32;
        at += 512;
        entries.push((name, Some(mode & 0o777), tar[at..at + size].to_vec()));
        at += size.div_ceil(512) * 512;
    }
    entries
}

/// Each entry of a zip: its name, its Unix permissions and its bytes.
fn unzip(path: &Path) -> Vec<(String, Option<u32>, Vec<u8>)> {
    let bytes = fs::read(path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (
                entry.name().unwrap().to_string(),
                entry.unix_mode().map(|m| m & 0o777),
                data,
            )
        })
        .collect()
}

fn issues(json: &str) -> Vec<(String, String)> {
    let list: Value = serde_json::from_str(json).expect("the output is JSON");
    list.as_array()
        .expect("a list")
        .iter()
        .map(|i| {
            (
                i["title"].as_str().unwrap().to_string(),
                i["body"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

// ----- Release versions ----------------------------------------------------

#[test]
fn a_test_build_before_0_1_0_is_tagged_0_1_0_alpha_n() {
    // Basis: Rule (#15 §8): the alpha is 0.1.0, and test builds before it are
    // 0.1.0-alpha.N.
    let here = repo_root();
    for (tag, version) in [
        ("v0.1.0-alpha.1", "0.1.0-alpha.1"),
        ("v0.1.0-alpha.12", "0.1.0-alpha.12"),
        ("v0.1.0", "0.1.0"),
        ("v0.2.0-alpha.3", "0.2.0-alpha.3"),
        ("v1.0.0", "1.0.0"),
    ] {
        let (ok, out, err) = xtask(&here, &["release-version", tag]);
        assert!(ok, "{tag}: {err}");
        assert_eq!(out.trim(), version, "{tag}");
    }
}

#[test]
fn a_tag_that_is_not_a_release_version_releases_nothing() {
    // Basis: Rule (#15 §8): a version below 0.1.0, a release candidate or a
    // beta, alpha.0, and anything not a version are refused, so a mistyped
    // tag never publishes.
    let here = repo_root();
    for tag in [
        "0.1.0-alpha.1",
        "v0.0.9",
        "v0.0.1-alpha.1",
        "v0.1.0-alpha.0",
        "v0.1.0-alpha.01",
        "v0.1.0-rc.1",
        "v0.1.0-beta.1",
        "v0.1",
        "v0.1.0.1",
        "v01.0.0",
        "vnext",
    ] {
        let (ok, out, err) = xtask(&here, &["release-version", tag]);
        assert!(!ok, "{tag} gave {out}");
        assert!(err.contains("isn't a release tag"), "{tag}: {err}");
        assert!(err.contains("v0.1.0-alpha.N"), "{tag}: {err}");
    }
}

// ----- Packaging -----------------------------------------------------------

/// What every download carries, from the repo, where the game reads it:
/// the Packs, a Scenario, the material library and its textures, and the
/// licences and credits.
const THE_GAMES_FILES: [&str; 10] = [
    "packs/opendrone/pack.toml",
    "packs/opendrone/maps/skate-park/skate-park.glb",
    "scenarios/physics/free-fall.toml",
    "assets-src/materials.toml",
    "assets-src/textures/concrete/diffuse.jpg",
    "assets-src/textures/concrete/normal.jpg",
    "CREDITS.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "THIRD-PARTY-LICENSES.txt",
];

/// Packages the scratch repo's game for `os`, named `name`, into `dist/`,
/// and gives the download's entries.
fn package_for(repo: &Repo, os: &str, name: &str) -> Vec<(String, Option<u32>, Vec<u8>)> {
    let (ok, out, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        os,
        "--name",
        name,
        "--out",
        "dist",
    ]);
    assert!(ok, "{os}: {err}");
    assert!(out.contains("1 textures"), "{out}");
    let file = if os == "linux" {
        format!("dist/OpenDrone-{name}-{os}.tar.gz")
    } else {
        format!("dist/OpenDrone-{name}-{os}.zip")
    };
    unpack(&repo.root.join(file))
}

#[test]
fn the_windows_and_linux_downloads_hold_the_game_with_its_packs_and_scenarios_beside_it() {
    // Basis: Rule (#81, #297): the pilot unpacks a build and double-clicks
    // it, and the game finds packs/ and scenarios/ beside itself
    // (opendrone::set_up::find_repo_folder), the material library and its
    // textures under assets-src/ (map_drawing.rs), as in the repo. Linux's
    // also carries its icon and the script that adds it to the app menu.
    let repo = Repo::new("layout");
    repo.with_the_games_files();
    repo.with_the_textures();
    for (os, program, more) in [
        ("windows", "OpenDrone.exe", &[][..]),
        (
            "linux",
            "opendrone",
            &["opendrone.png", "add-to-app-menu.sh"][..],
        ),
    ] {
        let entries = package_for(&repo, os, "0.1.0-alpha.1");
        let top = format!("OpenDrone-0.1.0-alpha.1-{os}");
        let files: Vec<&str> = entries
            .iter()
            .map(|(n, _, _)| n.as_str())
            .filter(|n| !n.ends_with('/'))
            .collect();
        let mut want: Vec<&str> = vec![program, "README.txt"];
        want.extend(THE_GAMES_FILES);
        want.extend(more);
        for file in &want {
            assert!(
                files.contains(&format!("{top}/{file}").as_str()),
                "{os}: {file} in {files:?}"
            );
        }
        assert_eq!(files.len(), want.len(), "{os}: nothing else: {files:?}");
        let (_, _, bytes) = entries
            .iter()
            .find(|(n, _, _)| n == &format!("{top}/{program}"))
            .unwrap();
        assert_eq!(bytes, b"the game's program", "{os}");
    }
}

#[test]
fn the_macos_download_is_an_app_holding_the_game_and_its_files() {
    // Basis: Rule (#297): macOS opens an app bundle with a double-click and
    // no Terminal window: the program in Contents/MacOS/, named by
    // Info.plist's CFBundleExecutable, its icon and files in
    // Contents/Resources/, where the game looks inside its app
    // (opendrone::set_up::find_repo_folder). The README and licences also
    // sit beside the app, where the pilot sees them.
    let repo = Repo::new("macos-app");
    repo.with_the_games_files();
    repo.with_the_textures();
    let entries = package_for(&repo, "macos", "0.1.0-alpha.1");
    let top = "OpenDrone-0.1.0-alpha.1-macos";
    let app = format!("{top}/OpenDrone.app/Contents");
    let files: Vec<&str> = entries
        .iter()
        .map(|(n, _, _)| n.as_str())
        .filter(|n| !n.ends_with('/'))
        .collect();
    let mut want: Vec<String> = vec![
        format!("{app}/Info.plist"),
        format!("{app}/MacOS/OpenDrone"),
        format!("{app}/Resources/OpenDrone.icns"),
        format!("{top}/README.txt"),
    ];
    want.extend(THE_GAMES_FILES.map(|f| format!("{app}/Resources/{f}")));
    want.extend(
        [
            "CREDITS.md",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "THIRD-PARTY-LICENSES.txt",
        ]
        .map(|f| format!("{top}/{f}")),
    );
    for file in &want {
        assert!(files.contains(&file.as_str()), "{file} in {files:?}");
    }
    assert_eq!(files.len(), want.len(), "nothing else: {files:?}");
    let read = |name: &str| {
        let (_, mode, bytes) = entries.iter().find(|(n, _, _)| n == name).unwrap();
        (*mode, String::from_utf8_lossy(bytes).into_owned())
    };
    assert_eq!(
        read(&format!("{app}/MacOS/OpenDrone")),
        (Some(0o755), "the game's program".to_string())
    );
    assert_eq!(
        read(&format!("{app}/Resources/OpenDrone.icns")).1,
        "icns icon"
    );
    let (_, plist) = read(&format!("{app}/Info.plist"));
    for (key, value) in [
        ("CFBundleExecutable", "<string>OpenDrone</string>"),
        ("CFBundleIconFile", "<string>OpenDrone</string>"),
        ("CFBundlePackageType", "<string>APPL</string>"),
        (
            "CFBundleIdentifier",
            "<string>io.github.bartoszsolkabd.opendrone</string>",
        ),
        // A release's version, as three whole numbers.
        ("CFBundleShortVersionString", "<string>0.1.0</string>"),
    ] {
        let after = plist
            .split_once(&format!("<key>{key}</key>"))
            .unwrap_or_else(|| panic!("{key} in {plist}"))
            .1;
        assert!(after.trim_start().starts_with(value), "{key}: {plist}");
    }
}

#[test]
fn a_build_of_main_names_the_app_after_the_repos_version() {
    // Basis: Rule (#297): macOS wants an app's version as whole numbers, and
    // main-abc1234 isn't one, so a build of main or a PR takes the version
    // Cargo.toml gives, without its pre-release.
    let repo = Repo::new("macos-version");
    repo.with_the_games_files();
    repo.with_the_textures();
    let entries = package_for(&repo, "macos", "main-abc1234");
    let (_, _, plist) = entries
        .iter()
        .find(|(n, _, _)| n.ends_with("/Info.plist"))
        .unwrap();
    let plist = String::from_utf8_lossy(plist);
    assert!(
        plist.contains("<key>CFBundleVersion</key>\n\t<string>0.1.0</string>"),
        "{plist}"
    );
}

#[test]
fn each_downloads_readme_says_how_to_open_it_on_its_system() {
    // Basis: Rule (#297): the builds aren't signed, so the README says what
    // the system's first warning looks like and how to get past it, and
    // where saved flights go.
    let repo = Repo::new("readme");
    repo.with_the_games_files();
    repo.with_the_textures();
    for (os, says) in [
        (
            "macos",
            &[
                "Double-click OpenDrone",
                "Applications",
                "Open Anyway",
                "Privacy & Security",
                "~/Library/Application Support/OpenDrone/",
            ][..],
        ),
        (
            "windows",
            &[
                "Extract All",
                "OpenDrone.exe",
                "More info, then Run anyway",
                "%APPDATA%\\OpenDrone\\",
            ][..],
        ),
        (
            "linux",
            &[
                "Double-click opendrone",
                "./add-to-app-menu.sh",
                "Vulkan",
                "~/.config/opendrone/",
            ][..],
        ),
    ] {
        let entries = package_for(&repo, os, "x");
        let (_, _, readme) = entries
            .iter()
            .find(|(n, _, _)| n == &format!("OpenDrone-x-{os}/README.txt"))
            .unwrap_or_else(|| panic!("{os}: a README.txt at the top"));
        let readme = String::from_utf8_lossy(readme);
        for text in says {
            assert!(readme.contains(text), "{os}: {text:?} in {readme}");
        }
    }
}

#[test]
fn the_linux_script_adds_a_desktop_entry_naming_the_folder_it_is_in() {
    // Basis: Rule (#297): a Linux desktop lists an app in its menu from a
    // desktop entry in ~/.local/share/applications/, which must name the
    // program and icon by their full paths, known only once the pilot has
    // unpacked the download: so the download carries a script that writes
    // it, rather than the entry itself.
    let repo = Repo::new("desktop-entry");
    repo.with_the_games_files();
    repo.with_the_textures();
    let entries = package_for(&repo, "linux", "x");
    let (_, mode, script) = entries
        .iter()
        .find(|(n, _, _)| n == "OpenDrone-x-linux/add-to-app-menu.sh")
        .unwrap();
    assert_eq!(*mode, Some(0o755), "it runs");
    let script = String::from_utf8_lossy(script);
    assert!(script.starts_with("#!/bin/sh\n"), "{script}");
    for line in [
        "[Desktop Entry]",
        "Type=Application",
        "Name=OpenDrone",
        "Exec=\"$here/opendrone\"",
        "Icon=$here/opendrone.png",
        "Terminal=false",
        "Categories=Game;Simulation;",
        "--remove",
    ] {
        assert!(script.contains(line), "{line:?} in {script}");
    }
    assert!(
        script.contains("${XDG_DATA_HOME:-$HOME/.local/share}/applications"),
        "{script}"
    );
}

#[test]
fn the_macos_app_is_signed_and_put_in_a_disk_image_with_dmg() {
    // Basis: Rule (#297): Apple Silicon runs only signed programs, so the
    // app is signed (ad hoc, with no certificate, until the maintainer has a
    // Developer ID), and it comes in a .dmg, which macOS opens with a
    // double-click. Only a Mac has codesign and hdiutil, so elsewhere --dmg
    // says so.
    let repo = Repo::new("dmg");
    repo.with_the_games_files();
    repo.with_the_textures();
    // A real program to sign: xtask itself.
    fs::copy(
        env!("CARGO_BIN_EXE_xtask"),
        repo.root.join("built/opendrone"),
    )
    .unwrap();
    let (ok, out, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "macos",
        "--name",
        "x",
        "--out",
        "dist",
        "--dmg",
    ]);
    if cfg!(target_os = "macos") {
        assert!(ok, "{err}");
        assert!(out.contains("OpenDrone-x-macos.dmg"), "{out}");
        let dmg = fs::read(repo.root.join("dist/OpenDrone-x-macos.dmg")).unwrap();
        // A disk image ends with its trailer, which starts "koly".
        assert_eq!(&dmg[dmg.len() - 512..dmg.len() - 508], b"koly");
        assert!(
            !repo.root.join("dist/OpenDrone-x-macos").exists(),
            "the folder it was made from is gone"
        );
    } else {
        assert!(!ok);
        assert!(err.contains("--dmg needs a Mac"), "{err}");
    }
}

#[test]
fn dmg_and_sign_go_only_with_the_macos_app() {
    // Basis: Rule (#297): they are how the macOS app is signed and shipped.
    let repo = Repo::new("dmg-elsewhere");
    repo.with_the_games_files();
    repo.with_the_textures();
    for (os, flags) in [
        ("windows", &["--dmg"][..]),
        ("linux", &["--sign", "Someone"][..]),
        ("macos", &["--sign", "Someone"][..]),
    ] {
        let mut args = vec![
            "package",
            "--game",
            "built/opendrone",
            "--os",
            os,
            "--name",
            "x",
        ];
        args.extend(flags);
        let (ok, _, err) = repo.run(&args);
        assert!(!ok, "{os} {flags:?}");
        assert!(err.contains("--dmg"), "{os}: {err}");
    }
}

/// The names of the files in a download.
fn file_names(path: &Path) -> Vec<String> {
    unpack(path)
        .into_iter()
        .map(|(n, _, _)| n)
        .filter(|n| !n.ends_with('/'))
        .collect()
}

#[test]
fn downloaded_neurobem_data_never_goes_into_a_download() {
    // Basis: Rule (#251, docs/data-policy.md): NeuroBEM states no licence, so
    // its CSVs, which `cargo xtask download neurobem` writes beside the
    // Scenarios that play them and a .gitignore keeps out of git, must not be
    // redistributed, even when they sit on the machine that packages.
    let repo = Repo::new("neurobem");
    repo.write("scenarios/quads/freestyle-5/neurobem/.gitignore", "*.csv\n");
    repo.write(
        "scenarios/quads/freestyle-5/neurobem/rolling-out-of-a-hover.toml",
        "# plays back a NeuroBEM flight\n",
    );
    repo.with_the_games_files();
    repo.with_the_textures();
    // Downloaded after everything was committed, as on a developer's machine.
    repo.write(
        "scenarios/quads/freestyle-5/neurobem/merged_2021-02-18-17-22-36_seg_1.csv",
        "t,x,y,z\n",
    );
    repo.track();
    let (ok, out, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "linux",
        "--name",
        "x",
        "--out",
        "dist",
    ]);
    assert!(ok, "{err}");
    assert!(!out.contains(".csv"), "{out}");
    let files = file_names(&repo.root.join("dist/OpenDrone-x-linux.tar.gz"));
    let top = "OpenDrone-x-linux";
    assert!(
        files.contains(&format!(
            "{top}/scenarios/quads/freestyle-5/neurobem/rolling-out-of-a-hover.toml"
        )),
        "{files:?}"
    );
    assert!(
        files.iter().all(|f| !f.ends_with(".csv")),
        "no downloaded data in {files:?}"
    );
}

#[test]
fn a_file_git_does_not_track_never_goes_into_a_download() {
    // Basis: Rule (#251): a download holds only committed files, so whatever
    // else sits in packs/ or scenarios/ on this machine stays out, whether or
    // not a .gitignore names it.
    let repo = Repo::new("untracked");
    repo.with_the_games_files();
    repo.with_the_textures();
    repo.write("scenarios/physics/not-committed.toml", "# new\n");
    repo.write("packs/opendrone/data.bin", "downloaded");
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "windows",
        "--name",
        "x",
        "--out",
        "dist",
    ]);
    assert!(ok, "{err}");
    let files = file_names(&repo.root.join("dist/OpenDrone-x-windows.zip"));
    assert_eq!(
        files.len(),
        12,
        "the same files as a clean checkout: {files:?}"
    );
    assert!(
        files
            .iter()
            .all(|f| !f.contains("not-committed") && !f.contains("data.bin"))
    );
}

#[test]
fn a_folder_that_is_not_a_git_checkout_is_refused_with_a_plain_message() {
    // Basis: Rule (#251): with no git to say which files are committed, a
    // download can't promise it holds no downloaded data, so it isn't made.
    let repo = Repo::new("no-git");
    repo.with_the_games_files();
    repo.with_the_textures();
    fs::remove_dir_all(repo.root.join(".git")).unwrap();
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "linux",
        "--name",
        "x",
        "--out",
        "dist",
    ]);
    assert!(!ok);
    assert!(err.contains("isn't a git checkout"), "{err}");
    assert!(!repo.root.join("dist").exists());
}

#[test]
fn every_download_carries_the_licence_texts_of_the_libraries_built_into_its_game() {
    // Basis: Rule (#270, ADR-0014): MIT and Zlib ask for their texts to
    // travel with a binary copy, so each system's zip holds that system's
    // list of libraries with their licence texts, as THIRD-PARTY-LICENSES.txt
    // beside the game's own licences.
    let repo = Repo::new("third-party");
    repo.with_the_games_files();
    repo.with_the_textures();
    for os in ["macos", "windows", "linux"] {
        let (ok, _, err) = repo.run(&[
            "package",
            "--game",
            "built/opendrone",
            "--os",
            os,
            "--name",
            "x",
            "--out",
            "dist",
        ]);
        assert!(ok, "{os}: {err}");
        let file = if os == "linux" { "tar.gz" } else { "zip" };
        let entries = unpack(&repo.root.join(format!("dist/OpenDrone-x-{os}.{file}")));
        let mut places = vec![format!("OpenDrone-x-{os}/THIRD-PARTY-LICENSES.txt")];
        if os == "macos" {
            // Inside the app too, so it travels with the app when the pilot
            // drags it to Applications.
            places.push(format!(
                "OpenDrone-x-{os}/OpenDrone.app/Contents/Resources/THIRD-PARTY-LICENSES.txt"
            ));
        }
        for place in places {
            let (_, _, text) = entries
                .iter()
                .find(|(n, _, _)| n == &place)
                .unwrap_or_else(|| panic!("{os}: no {place}"));
            assert_eq!(
                String::from_utf8_lossy(text),
                format!("The libraries' licences, for {os}\n")
            );
        }
        let readme = entries
            .iter()
            .find(|(n, _, _)| n.ends_with("/README.txt"))
            .unwrap();
        assert!(
            String::from_utf8_lossy(&readme.2).contains("THIRD-PARTY-LICENSES.txt"),
            "the README points to it"
        );
    }
}

#[test]
fn a_download_without_the_libraries_licence_texts_is_refused() {
    // Basis: Rule (#270): no download goes out without them.
    let repo = Repo::new("no-third-party");
    repo.with_the_games_files();
    repo.with_the_textures();
    fs::remove_file(repo.root.join("licences/third-party-linux.txt")).unwrap();
    repo.track();
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "linux",
        "--name",
        "x",
        "--out",
        "dist",
    ]);
    assert!(!ok);
    assert!(err.contains("licences/third-party-linux.txt"), "{err}");
    assert!(err.contains("cargo xtask licences"), "{err}");
    assert!(!repo.root.join("dist").exists());
}

#[test]
fn the_game_in_a_download_keeps_its_permission_to_run() {
    // Basis: Rule (#81): on macOS and Linux a program runs only with its
    // execute permission, which a zip or tarball keeps only if it's written
    // into it. On Linux the app-menu script runs too.
    let repo = Repo::new("permissions");
    repo.with_the_games_files();
    repo.with_the_textures();
    for (os, programs) in [
        ("linux", &["/opendrone", "/add-to-app-menu.sh"][..]),
        ("macos", &["/OpenDrone.app/Contents/MacOS/OpenDrone"][..]),
    ] {
        for (name, mode, _) in package_for(&repo, os, "main-abc1234") {
            let want = if programs.iter().any(|p| name.ends_with(p)) || name.ends_with('/') {
                0o755
            } else {
                0o644
            };
            assert_eq!(mode, Some(want), "{name}");
        }
    }
}

#[test]
fn the_windows_download_holds_opendrone_exe() {
    // Basis: Rule (#297): Windows runs a program by its .exe name, and the
    // pilot looks for the app's own name.
    let repo = Repo::new("windows");
    repo.with_the_games_files();
    repo.with_the_textures();
    let names: Vec<String> = package_for(&repo, "windows", "pr81-abc1234")
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();
    assert!(names.contains(&"OpenDrone-pr81-abc1234-windows/OpenDrone.exe".to_string()));
    assert!(!names.contains(&"OpenDrone-pr81-abc1234-windows/opendrone".to_string()));
}

#[test]
fn the_same_files_always_make_the_same_download() {
    // Basis: Rule: entries in name order with a fixed date, so a rebuilt
    // download differs only where its files do.
    let repo = Repo::new("repeat");
    repo.with_the_games_files();
    repo.with_the_textures();
    for (os, file) in [("macos", "zip"), ("linux", "tar.gz")] {
        let args = |out: &'static str| {
            [
                "package",
                "--game",
                "built/opendrone",
                "--os",
                os,
                "--name",
                "x",
                "--out",
                out,
            ]
        };
        assert!(repo.run(&args("one")).0);
        assert!(repo.run(&args("two")).0);
        assert_eq!(
            fs::read(repo.root.join(format!("one/OpenDrone-x-{os}.{file}"))).unwrap(),
            fs::read(repo.root.join(format!("two/OpenDrone-x-{os}.{file}"))).unwrap(),
            "{os}"
        );
    }
}

#[test]
fn a_download_without_its_textures_is_refused_unless_asked_for() {
    // Basis: Rule (#63): without its textures a Map is drawn in flat
    // colours, so a build missing them says so instead of shipping quietly.
    let repo = Repo::new("no-textures");
    repo.with_the_games_files();
    let args = [
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "linux",
        "--name",
        "x",
        "--out",
        "dist",
    ];
    let (ok, _, err) = repo.run(&args);
    assert!(!ok);
    assert!(err.contains("concrete"), "{err}");
    assert!(err.contains("cargo xtask textures"), "{err}");
    let mut flat = args.to_vec();
    flat.push("--no-textures");
    let (ok, out, err) = repo.run(&flat);
    assert!(ok, "{err}");
    assert!(out.contains("flat colours"), "{out}");
}

// ----- Every picture of a material ---------------------------------------------

/// The pictures a texture's folder may hold, which the game reads by these
/// names (`crates/opendrone/src/map_drawing.rs`).
const THE_PICTURES: [&str; 6] = [
    "diffuse.jpg",
    "normal.jpg",
    "roughness.jpg",
    "ao.jpg",
    "metallic.jpg",
    "mask.jpg",
];

/// A material library of two textures made from scratch: `rust`, drawn with
/// its roughness, occlusion and metal pictures, and `moss`, a decal with a
/// mask and a roughness picture.
const THE_PICTURE_LIBRARY: &str = "\
[materials.rust]
texture = \"rust\"
roughness_map = true
ao_map = true
metallic_map = true

[materials.moss]
texture = \"moss\"
roughness_map = true

[textures.rust]
generator = \"rust\"
seed = 1
pixels = 512
size = \"2 m\"

[textures.moss]
generator = \"moss\"
seed = 1
pixels = 512
size = \"1 m\"
";

const PACKAGE_LINUX: [&str; 9] = [
    "package",
    "--game",
    "built/opendrone",
    "--os",
    "linux",
    "--name",
    "x",
    "--out",
    "dist",
];

fn with_the_pictures_of(repo: &Repo, texture: &str, pictures: &[&str]) {
    for picture in pictures {
        repo.write(
            &format!("assets-src/generated/{texture}/{picture}"),
            &format!("{texture} {picture}"),
        );
    }
}

#[test]
fn a_download_carries_every_picture_a_material_has_not_only_its_colour_and_normal_map() {
    // Basis: Rule (#332, #320, #321): the game reads a material's roughness,
    // occlusion and metal pictures and a decal's mask from its texture's
    // folder, so a download without them would draw no decals and no
    // roughness.
    let repo = Repo::new("every-picture");
    repo.with_the_games_files();
    repo.write("assets-src/materials.toml", THE_PICTURE_LIBRARY);
    with_the_pictures_of(&repo, "rust", &THE_PICTURES[..5]);
    with_the_pictures_of(
        &repo,
        "moss",
        &["diffuse.jpg", "normal.jpg", "roughness.jpg", "mask.jpg"],
    );
    repo.track();
    let (ok, out, err) = repo.run(&PACKAGE_LINUX);
    assert!(ok, "{err}");
    assert!(out.contains("2 textures (9 pictures)"), "{out}");
    let entries = unpack(&repo.root.join("dist/OpenDrone-x-linux.tar.gz"));
    let carried = |name: &str| entries.iter().any(|(n, _, _)| n == name);
    for picture in &THE_PICTURES[..5] {
        let name = format!("OpenDrone-x-linux/assets-src/generated/rust/{picture}");
        assert!(carried(&name), "{name}");
    }
    for picture in ["diffuse.jpg", "normal.jpg", "roughness.jpg", "mask.jpg"] {
        let name = format!("OpenDrone-x-linux/assets-src/generated/moss/{picture}");
        assert!(carried(&name), "{name}");
    }
    assert!(
        !carried("OpenDrone-x-linux/assets-src/generated/moss/ao.jpg"),
        "a picture a texture doesn't have isn't made up"
    );
}

#[test]
fn a_download_is_refused_when_a_material_asks_for_a_picture_its_texture_lacks() {
    // Basis: Rule (#332): every material of the library finds the pictures it
    // asks for in the download, or the build says which one is missing
    // instead of drawing it without.
    let repo = Repo::new("lost-picture");
    repo.with_the_games_files();
    repo.write("assets-src/materials.toml", THE_PICTURE_LIBRARY);
    with_the_pictures_of(&repo, "rust", &THE_PICTURES[..4]);
    with_the_pictures_of(&repo, "moss", &["diffuse.jpg", "normal.jpg", "mask.jpg"]);
    repo.track();
    let (ok, _, err) = repo.run(&PACKAGE_LINUX);
    assert!(!ok);
    assert!(
        err.contains("rust asks for metallic.jpg, which the texture rust doesn't have"),
        "{err}"
    );
    assert!(
        err.contains("moss asks for roughness.jpg, which the texture moss doesn't have"),
        "{err}"
    );
    assert!(!err.contains("ao.jpg"), "{err}");
}

/// Copies a folder and everything in it.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The files below a folder, by path from the folder, in name order.
fn files_below(folder: &Path, from: &str) -> Vec<String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let entry = entry.unwrap();
        let name = format!("{from}{}", entry.file_name().to_string_lossy());
        if entry.file_type().unwrap().is_dir() {
            files.extend(files_below(&entry.path(), &format!("{name}/")));
        } else {
            files.push(name);
        }
    }
    files.sort();
    files
}

#[test]
fn the_download_of_the_real_material_library_carries_every_picture_its_materials_read() {
    // Basis: Rule (#332): the committed library, as it stands, packages: each
    // material finds the pictures it asks for, and every picture committed
    // for a texture made from scratch (decal masks and roughness pictures
    // among them) is in the download. Fetched textures, which a checkout may
    // not have, are stood in for by two files each.
    let real = repo_root();
    let repo = Repo::new("real-library");
    repo.with_the_games_files();
    let library = fs::read_to_string(real.join("assets-src/materials.toml")).unwrap();
    repo.write("assets-src/materials.toml", &library);
    copy_folder(
        &real.join("assets-src/generated"),
        &repo.root.join("assets-src/generated"),
    );
    let doc: toml::Table = library.parse().unwrap();
    for (name, texture) in doc["textures"].as_table().unwrap() {
        if texture.get("generator").is_none() {
            for picture in ["diffuse.jpg", "normal.jpg"] {
                repo.write(&format!("assets-src/textures/{name}/{picture}"), "fetched");
            }
        }
    }
    repo.track();
    let (ok, _, err) = repo.run(&PACKAGE_LINUX);
    assert!(ok, "{err}");
    let entries = unpack(&repo.root.join("dist/OpenDrone-x-linux.tar.gz"));
    let committed = files_below(&real.join("assets-src/generated"), "");
    assert!(
        committed.iter().any(|f| f.ends_with("mask.jpg")),
        "the decals' masks are committed: {committed:?}"
    );
    for file in &committed {
        let name = format!("OpenDrone-x-linux/assets-src/generated/{file}");
        assert!(
            entries.iter().any(|(n, _, _)| n == &name),
            "{name} is in the download"
        );
    }
}

#[test]
fn the_game_reads_the_same_picture_names_a_download_carries() {
    // Basis: Rule (#332): the list of pictures a download carries
    // (xtask's materials::PICTURES) and the names the game reads
    // (map_drawing.rs) are two lists, and this keeps them one.
    let source = fs::read_to_string(repo_root().join("crates/opendrone/src/map_drawing.rs"))
        .expect("the game's drawing code");
    let mut read: Vec<&str> = source
        .split("folder.join(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    read.sort_unstable();
    let mut carried = THE_PICTURES.to_vec();
    carried.sort_unstable();
    assert_eq!(read, carried);
}

#[test]
fn a_build_name_that_cannot_be_a_file_name_is_refused() {
    // Basis: Rule: the name goes into the zip's file name.
    let repo = Repo::new("bad-name");
    repo.with_the_games_files();
    for name in ["", "a/b", "a b", "..\\x"] {
        let (ok, _, err) = repo.run(&[
            "package",
            "--game",
            "built/opendrone",
            "--os",
            "linux",
            "--name",
            name,
            "--no-textures",
        ]);
        assert!(!ok, "{name:?}");
        assert!(err.contains("--name"), "{name:?}: {err}");
    }
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "amiga",
        "--name",
        "x",
    ]);
    assert!(!ok);
    assert!(err.contains("macos, windows, linux"), "{err}");
}

// ----- Bevy upgrades -------------------------------------------------------

fn bevy_repo(case: &str, pinned: &str) -> Repo {
    let repo = Repo::new(case);
    repo.write(
        "Cargo.toml",
        &format!(
            "[workspace]\n\n[workspace.dependencies]\nbevy = {{ version = \"={pinned}\", default-features = false }}\n"
        ),
    );
    repo.write(
        ".github/bevy-upgrade-issue.md",
        "Bevy {release} has shipped; move from {pinned} to {version}.\n",
    );
    repo
}

/// The real index, with more lines after it.
fn index_with(repo: &Repo, more: &[(&str, bool)]) -> String {
    let mut text = fs::read_to_string(fixture("bevy-index.json")).unwrap();
    for (version, yanked) in more {
        text.push_str(&format!(
            "{{\"name\":\"bevy\",\"vers\":\"{version}\",\"deps\":[],\"yanked\":{yanked}}}\n"
        ));
    }
    repo.write("bevy-index.json", &text);
    "bevy-index.json".to_string()
}

#[test]
fn no_upgrade_issue_while_bevy_0_20_is_the_newest_release() {
    // Basis: Observed: crates.io's index on 21 October 2026 ends at 0.20.0.
    let repo = bevy_repo("newest", "0.20.0");
    let index = index_with(&repo, &[]);
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    assert!(issues(&out).is_empty(), "{out}");
}

#[test]
fn a_release_candidate_never_opens_an_upgrade_issue() {
    // Basis: Rule (ADR-0021): the issue opens when 0.N.0 ships, never for a
    // release candidate.
    let repo = bevy_repo("candidate", "0.20.0");
    let index = index_with(&repo, &[("0.21.0-rc.1", false), ("0.21.0-rc.2", false)]);
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    assert!(issues(&out).is_empty(), "{out}");
}

#[test]
fn bevy_0_21_0_opens_upgrade_to_bevy_0_21_from_the_fixed_template() {
    // Basis: Rule (ADR-0021): one "Upgrade to Bevy 0.N" issue, its text the
    // fixed template with only the versions filled in.
    let repo = bevy_repo("final", "0.20.0");
    let index = index_with(
        &repo,
        &[("0.20.1", false), ("0.21.0-rc.1", false), ("0.21.0", false)],
    );
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    assert_eq!(
        issues(&out),
        vec![(
            "Upgrade to Bevy 0.21".to_string(),
            "Bevy 0.21.0 has shipped; move from 0.20.0 to 0.21.\n".to_string()
        )]
    );
}

#[test]
fn a_patch_release_or_a_yanked_release_opens_no_upgrade_issue() {
    // Basis: Rule (ADR-0021): patch releases arrive as Dependabot PRs; a
    // yanked 0.N.0 hasn't shipped.
    let repo = bevy_repo("patch", "0.20.0");
    let index = index_with(
        &repo,
        &[("0.20.1", false), ("0.20.2", false), ("0.21.0", true)],
    );
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    assert!(issues(&out).is_empty(), "{out}");
}

#[test]
fn a_pinned_release_candidate_moving_to_its_own_final_is_no_upgrade() {
    // Basis: Rule (ADR-0021): the move from a release candidate to its final
    // release follows the patch-release rules.
    let repo = bevy_repo("own-final", "0.21.0-rc.2");
    let index = index_with(
        &repo,
        &[
            ("0.21.0-rc.1", false),
            ("0.21.0-rc.2", false),
            ("0.21.0", false),
        ],
    );
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    assert!(issues(&out).is_empty(), "{out}");
}

#[test]
fn two_new_bevy_versions_open_one_issue_each_oldest_first() {
    // Basis: Rule (ADR-0021): each later Bevy version is its own ticket.
    let repo = bevy_repo("two", "0.20.0");
    let index = index_with(
        &repo,
        &[("0.22.0", false), ("0.21.0", false), ("0.21.1", false)],
    );
    let (ok, out, err) = repo.run(&["bevy-upgrade", &index]);
    assert!(ok, "{err}");
    let titles: Vec<String> = issues(&out).into_iter().map(|(t, _)| t).collect();
    assert_eq!(titles, ["Upgrade to Bevy 0.21", "Upgrade to Bevy 0.22"]);
}

#[test]
fn the_repos_upgrade_template_spells_out_adr_0021s_gates() {
    // Basis: Rule (ADR-0021): when an agent starts, the 6 weeks, and the four
    // things it must pass before it merges.
    let text = fs::read_to_string(repo_root().join(".github/bevy-upgrade-issue.md")).unwrap();
    for want in [
        "{version}",
        "{release}",
        "{pinned}",
        "add-on crate",
        "6 weeks",
        "needs-maintainer",
        "Every usual required check passes",
        "Scenario fingerprint don't move",
        "counting wgpu patch",
        "Frame Check",
        "Video Looks",
    ] {
        assert!(text.contains(want), "{want}");
    }
}

// ----- Security advisories -------------------------------------------------

#[test]
fn each_advisory_cargo_deny_finds_becomes_one_issue() {
    // Basis: Observed: deny-advisories.json, cargo-deny 0.20.2's output for
    // this repo, holds RUSTSEC-2026-0192 (ttf-parser, unmaintained) and
    // RUSTSEC-2026-0344 (ruzstd, a vulnerability), both reached from the
    // game.
    let here = repo_root();
    let file = fixture("deny-advisories.json");
    let (ok, out, err) = xtask(&here, &["advisories", file.to_str().unwrap()]);
    assert!(ok, "{err}");
    let found = issues(&out);
    let titles: Vec<&str> = found.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Security advisory RUSTSEC-2026-0192 for ttf-parser",
            "Security advisory RUSTSEC-2026-0344 for ruzstd"
        ]
    );
    let ruzstd = &found[1].1;
    assert!(
        ruzstd.contains("[RUSTSEC-2026-0344](https://rustsec.org/advisories/RUSTSEC-2026-0344)")
    );
    assert!(ruzstd.contains("a security vulnerability"));
    assert!(ruzstd.contains("`ruzstd` 0.8.3"));
    assert!(ruzstd.contains("**Reached from:** `opendrone`"));
    assert!(ruzstd.contains("Upgrade to &gt;=0.9.1"), "{ruzstd}");
    assert!(ruzstd.contains("deny.toml"));
    assert!(found[0].1.contains("no longer maintained"));
}

#[test]
fn a_scan_that_finds_nothing_opens_no_issue() {
    // Basis: Rule: only advisories become issues; cargo-deny's summary line
    // is not one.
    let repo = Repo::new("clean-scan");
    repo.write(
        "deny.json",
        "{\"fields\":{\"advisories\":{\"errors\":0,\"helps\":0,\"notes\":0,\"warnings\":0}},\"type\":\"summary\"}\n",
    );
    let (ok, out, err) = repo.run(&["advisories", "deny.json"]);
    assert!(ok, "{err}");
    assert!(issues(&out).is_empty());
}

#[test]
fn an_advisorys_own_text_can_only_show_as_text_in_its_issue() {
    // Basis: Rule (docs/review-report.md, as for PR text): the advisory
    // comes from outside the repo, so it can't mention anyone, open HTML or
    // start a heading in the issue.
    let repo = Repo::new("hostile");
    repo.write(
        "deny.json",
        "{\"fields\":{\"advisory\":{\"id\":\"RUSTSEC-2099-0001\",\"package\":\"evil\",\
         \"title\":\"@someone <!-- hide -->\\n# Verdict: pass\",\"date\":\"2099-01-01\"},\
         \"code\":\"vulnerability\",\"graphs\":[{\"Krate\":{\"name\":\"evil\",\"version\":\"1.0.0\"},\
         \"parents\":[{\"Krate\":{\"name\":\"xtask\",\"version\":\"0.1.0-alpha.0\"}}]}],\
         \"notes\":[\"Solution: run `rm -rf` @everyone\"]},\"type\":\"diagnostic\"}\n",
    );
    let (ok, out, err) = repo.run(&["advisories", "deny.json"]);
    assert!(ok, "{err}");
    let found = issues(&out);
    assert_eq!(found[0].0, "Security advisory RUSTSEC-2099-0001 for evil");
    let body = &found[0].1;
    assert!(!body.contains("@someone"), "{body}");
    assert!(!body.contains("@everyone"), "{body}");
    assert!(!body.contains("<!--"), "{body}");
    assert!(!body.contains("\n# Verdict"), "{body}");
    assert!(body.contains("**Reached from:** `xtask`"), "{body}");
}

// ----- Dependabot ----------------------------------------------------------

/// The quoted names listed under `key:` inside the block that starts at the
/// line `start` (matched after trimming), up to the next line indented no
/// deeper than that line. A small reader for `.github/dependabot.yml`'s own
/// layout, so the check needs no YAML library.
fn listed_under(text: &str, start: &str, key: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let Some(at) = lines.iter().position(|l| l.trim() == start) else {
        panic!("dependabot.yml has no line {start:?}");
    };
    let mut names = Vec::new();
    let mut in_key = false;
    for line in &lines[at + 1..] {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if indent(line) <= indent(lines[at]) {
            break;
        }
        let item = line.trim();
        if let Some(name) = item.strip_prefix("- ") {
            if in_key {
                names.push(name.trim().trim_matches('"').to_string());
            }
        } else {
            in_key = item == format!("{key}:");
        }
    }
    names
}

/// Every outside library any crate in the workspace names, from the root
/// `Cargo.toml`'s `[workspace.dependencies]`, and those the core crates (in
/// walls.toml) name themselves.
fn libraries() -> (Vec<String>, Vec<String>) {
    let root = repo_root();
    let manifest: toml::Table = fs::read_to_string(root.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let workspace = manifest["workspace"]["dependencies"].as_table().unwrap();
    let outside: Vec<String> = workspace
        .iter()
        .filter(|(_, spec)| spec.get("path").is_none())
        .map(|(name, _)| name.clone())
        .collect();
    let walls: toml::Table = fs::read_to_string(root.join("crates/xtask/walls.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let mut core = Vec::new();
    for krate in walls["core"].as_array().unwrap() {
        let folder = krate.as_str().unwrap().trim_start_matches("opendrone-");
        let own: toml::Table =
            fs::read_to_string(root.join("crates").join(folder).join("Cargo.toml"))
                .unwrap()
                .parse()
                .unwrap();
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = own.get(kind).and_then(toml::Value::as_table) {
                core.extend(table.keys().filter(|k| outside.contains(k)).cloned());
            }
        }
    }
    core.sort();
    core.dedup();
    (outside, core)
}

#[test]
fn every_crate_takes_its_outside_libraries_from_the_workspace_list() {
    // Basis: Rule: Dependabot's settings below are checked against
    // [workspace.dependencies], so a library named only in one crate's
    // Cargo.toml would slip past them.
    let root = repo_root();
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let path = entry.unwrap().path().join("Cargo.toml");
        if !path.is_file() {
            continue;
        }
        let own: toml::Table = fs::read_to_string(&path).unwrap().parse().unwrap();
        let mut tables: Vec<&toml::Table> = Vec::new();
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(t) = own.get(kind).and_then(toml::Value::as_table) {
                tables.push(t);
            }
        }
        if let Some(targets) = own.get("target").and_then(toml::Value::as_table) {
            for target in targets.values() {
                for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(t) = target.get(kind).and_then(toml::Value::as_table) {
                        tables.push(t);
                    }
                }
            }
        }
        for table in tables {
            for (name, spec) in table {
                assert_eq!(
                    spec.get("workspace").and_then(toml::Value::as_bool),
                    Some(true),
                    "{}: {name} isn't taken from [workspace.dependencies]",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn dependabot_gives_rust_and_every_core_library_its_own_pr_and_groups_the_rest() {
    // Basis: Rule (#15 §8, #81): Rust itself, libm, parry3d and every other
    // library the core crates use each arrive in a separate PR, and every
    // other library comes grouped. Bevy and wgpu, whose patch releases each
    // arrive on their own (ADR-0021), and ra-ap-rustc_lexer, which moves only
    // with Rust, stay out of the group too.
    let text = fs::read_to_string(repo_root().join(".github/dependabot.yml")).unwrap();
    let (outside, core) = libraries();
    assert!(core.contains(&"libm".to_string()) && core.contains(&"parry3d-f64".to_string()));
    let grouped = listed_under(&text, "edge-libraries:", "patterns");
    let on_their_own = ["bevy", "wgpu", "ra-ap-rustc_lexer"];
    for library in &outside {
        let alone = core.contains(library) || on_their_own.contains(&library.as_str());
        assert_eq!(
            grouped.contains(library),
            !alone,
            "{library}: {}",
            if alone {
                "a core library, Bevy, wgpu or the lexer is never in the grouped PR"
            } else {
                "a library outside the core belongs in edge-libraries' patterns"
            }
        );
    }
    for name in &grouped {
        assert!(
            outside.contains(name),
            "edge-libraries names {name}, which no crate uses"
        );
    }
    assert!(text.contains("package-ecosystem: rust-toolchain"));
    assert!(text.contains("package-ecosystem: github-actions"));
    assert_eq!(
        text.matches("interval: monthly").count(),
        3,
        "every update runs monthly"
    );
}

#[test]
fn dependabot_never_offers_a_bevy_0_n_jump_or_a_wgpu_major_version() {
    // Basis: Rule (ADR-0021): those never build without code changes, so the
    // monthly workflow's issue starts them instead.
    let text = fs::read_to_string(repo_root().join(".github/dependabot.yml")).unwrap();
    let block = |name: &str| -> String {
        let at = text
            .find(&format!("- dependency-name: {name}\n"))
            .unwrap_or_else(|| panic!("no ignore entry for {name}"));
        text[at..]
            .lines()
            .skip(1)
            .take_while(|l| !l.trim_start().starts_with("- dependency-name"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let bevy = block("bevy");
    assert!(
        bevy.contains("version-update:semver-minor")
            && bevy.contains("version-update:semver-major")
    );
    assert!(!bevy.contains("semver-patch"));
    let wgpu = block("wgpu");
    assert!(wgpu.contains("version-update:semver-major"));
    assert!(!wgpu.contains("semver-minor") && !wgpu.contains("semver-patch"));
}
