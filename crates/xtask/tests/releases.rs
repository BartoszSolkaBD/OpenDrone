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
            "[workspace]\n\n[workspace.dependencies]\nbevy = { version = \"=0.20.0\", default-features = false }\n",
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
        self.write("built/opendrone", "the game's program");
    }

    fn with_the_textures(&self) {
        self.write("assets-src/textures/concrete/diffuse.jpg", "JPEG d");
        self.write("assets-src/textures/concrete/normal.jpg", "JPEG n");
    }
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

#[test]
fn a_download_holds_the_game_with_its_packs_and_scenarios_beside_it() {
    // Basis: Rule (#81): the maintainer unzips a build and double-clicks it,
    // and the game finds packs/ and scenarios/ beside itself
    // (opendrone::set_up::find_repo_folder), the material library and its
    // textures under assets-src/ (map_drawing.rs), as in the repo.
    let repo = Repo::new("layout");
    repo.with_the_games_files();
    repo.with_the_textures();
    let (ok, out, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "macos",
        "--name",
        "0.1.0-alpha.1",
        "--out",
        "dist",
    ]);
    assert!(ok, "{err}");
    assert!(out.contains("1 textures"), "{out}");
    let entries = unzip(&repo.root.join("dist/OpenDrone-0.1.0-alpha.1-macos.zip"));
    let files: Vec<&str> = entries
        .iter()
        .map(|(n, _, _)| n.as_str())
        .filter(|n| !n.ends_with('/'))
        .collect();
    let top = "OpenDrone-0.1.0-alpha.1-macos";
    for want in [
        "opendrone",
        "README.txt",
        "packs/opendrone/pack.toml",
        "packs/opendrone/maps/skate-park/skate-park.glb",
        "scenarios/physics/free-fall.toml",
        "assets-src/materials.toml",
        "assets-src/textures/concrete/diffuse.jpg",
        "assets-src/textures/concrete/normal.jpg",
        "CREDITS.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
    ] {
        assert!(
            files.contains(&format!("{top}/{want}").as_str()),
            "{want} in {files:?}"
        );
    }
    assert_eq!(files.len(), 11, "nothing else: {files:?}");
    let program = entries
        .iter()
        .find(|(n, _, _)| n == &format!("{top}/opendrone"))
        .unwrap();
    assert_eq!(program.2, b"the game's program");
}

#[test]
fn the_game_in_a_download_keeps_its_permission_to_run() {
    // Basis: Rule (#81): on macOS and Linux a program runs only with its
    // execute permission, which a zip keeps only if it's written into it.
    let repo = Repo::new("permissions");
    repo.with_the_games_files();
    repo.with_the_textures();
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "linux",
        "--name",
        "main-abc1234",
        "--out",
        "dist",
    ]);
    assert!(ok, "{err}");
    for (name, mode, _) in unzip(&repo.root.join("dist/OpenDrone-main-abc1234-linux.zip")) {
        let want = if name.ends_with("/opendrone") || name.ends_with('/') {
            0o755
        } else {
            0o644
        };
        assert_eq!(mode, Some(want), "{name}");
    }
}

#[test]
fn the_windows_download_holds_opendrone_exe() {
    // Basis: Rule: Windows runs a program by its .exe name.
    let repo = Repo::new("windows");
    repo.with_the_games_files();
    repo.with_the_textures();
    let (ok, _, err) = repo.run(&[
        "package",
        "--game",
        "built/opendrone",
        "--os",
        "windows",
        "--name",
        "pr81-abc1234",
        "--out",
        "dist",
    ]);
    assert!(ok, "{err}");
    let names: Vec<String> = unzip(&repo.root.join("dist/OpenDrone-pr81-abc1234-windows.zip"))
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();
    assert!(names.contains(&"OpenDrone-pr81-abc1234-windows/opendrone.exe".to_string()));
    assert!(!names.contains(&"OpenDrone-pr81-abc1234-windows/opendrone".to_string()));
}

#[test]
fn the_same_files_always_make_the_same_download() {
    // Basis: Rule: entries in name order with a fixed date, so a rebuilt
    // download differs only where its files do.
    let repo = Repo::new("repeat");
    repo.with_the_games_files();
    repo.with_the_textures();
    let args = |out: &'static str| {
        [
            "package",
            "--game",
            "built/opendrone",
            "--os",
            "macos",
            "--name",
            "x",
            "--out",
            out,
        ]
    };
    assert!(repo.run(&args("one")).0);
    assert!(repo.run(&args("two")).0);
    assert_eq!(
        fs::read(repo.root.join("one/OpenDrone-x-macos.zip")).unwrap(),
        fs::read(repo.root.join("two/OpenDrone-x-macos.zip")).unwrap()
    );
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
