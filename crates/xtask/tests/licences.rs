//! Readable checks for the libraries' licence texts that every download
//! carries (#270, ADR-0014): `cargo xtask licences`, run as CI runs it, on a
//! scratch workspace whose game and libraries are small crates on disk, so
//! nothing here needs the network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch workspace: a game crate named `opendrone` with its libraries
/// beside it, outside the workspace, as crates.io's libraries are.
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(case: &str) -> Workspace {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("licences")
            .join(case);
        let _ = fs::remove_dir_all(&base);
        let workspace = Workspace {
            root: base.join("repo"),
        };
        workspace.write(".github/placeholder", "");
        workspace.write("Cargo.toml", "[workspace]\nmembers = [\"opendrone\"]\n");
        workspace.write("opendrone/src/lib.rs", "");
        workspace.set_game_dependencies("");
        workspace
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The game's `[dependencies]` and friends, and then Cargo.lock.
    fn set_game_dependencies(&self, text: &str) {
        self.write(
            "opendrone/Cargo.toml",
            &format!(
                "[package]\nname = \"opendrone\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{text}"
            ),
        );
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["generate-lockfile", "--offline"])
            .current_dir(&self.root)
            .output()
            .expect("cargo runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A library outside the workspace, from the folder `libs/<name>`.
    fn library(&self, name: &str, version: &str, licence: &str, files: &[(&str, &str)]) {
        self.write(
            &format!("../libs/{name}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2024\"\n\
                 license = \"{licence}\"\nrepository = \"https://example.com/{name}\"\n"
            ),
        );
        self.write(&format!("../libs/{name}/src/lib.rs"), "");
        for (file, text) in files {
            self.write(&format!("../libs/{name}/{file}"), text);
        }
    }

    fn run(&self, args: &[&str]) -> (bool, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("xtask runs");
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn file(&self, os: &str) -> String {
        fs::read_to_string(self.root.join(format!("licences/third-party-{os}.txt")))
            .expect("the file was written")
    }
}

const PRETTY_MIT: &str = "MIT License\n\nCopyright (c) 2026 The Pretty Authors\n\nPermission is hereby granted, free of charge, to any person.\n";

#[test]
fn the_file_lists_a_library_with_its_version_licence_and_licence_text() {
    // Basis: Rule (#270, ADR-0014): MIT and Zlib ask for their texts to
    // travel with a binary copy, so each library compiled into the game is
    // listed with its version, its licence and the text of its licence file.
    let w = Workspace::new("lists-a-library");
    w.library("pretty-lib", "1.2.3", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies("[dependencies]\npretty-lib = { path = \"../../libs/pretty-lib\" }\n");
    let (ok, out, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    assert!(
        out.contains("Wrote licences/third-party-linux.txt"),
        "{out}"
    );
    let file = w.file("linux");
    assert!(
        file.contains("pretty-lib 1.2.3\n    licence: MIT\n"),
        "{file}"
    );
    assert!(
        file.contains("source: https://example.com/pretty-lib"),
        "{file}"
    );
    assert!(
        file.contains("Copyright (c) 2026 The Pretty Authors"),
        "the licence file's own text, with its copyright holders: {file}"
    );
    assert!(
        !file.contains("opendrone 0.1.0"),
        "the game's own crates aren't third-party libraries: {file}"
    );
}

#[test]
fn a_text_shared_by_several_libraries_is_written_once() {
    // Basis: Rule (#270): the file stays readable and small, so a text that
    // two libraries share word for word appears once, and both point to it.
    let w = Workspace::new("shared-text");
    w.library("one", "1.0.0", "MIT", &[("LICENSE-MIT", PRETTY_MIT)]);
    w.library("two", "2.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies(
        "[dependencies]\none = { path = \"../../libs/one\" }\ntwo = { path = \"../../libs/two\" }\n",
    );
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let file = w.file("linux");
    assert_eq!(file.matches("The Pretty Authors").count(), 1, "{file}");
    assert!(file.contains("[1] used by: one, two"), "{file}");
    assert_eq!(file.matches("licence text: [1]").count(), 2, "{file}");
}

#[test]
fn a_library_only_a_build_script_or_a_test_uses_is_not_listed() {
    // Basis: Rule (#270): the file lists the libraries "actually compiled
    // into the game". A build dependency or a dev dependency runs or tests
    // while building, and its code isn't in the program.
    let w = Workspace::new("not-compiled-in");
    w.library("in-the-game", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.library("build-tool", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.library("test-helper", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies(
        "[dependencies]\nin-the-game = { path = \"../../libs/in-the-game\" }\n\
         [build-dependencies]\nbuild-tool = { path = \"../../libs/build-tool\" }\n\
         [dev-dependencies]\ntest-helper = { path = \"../../libs/test-helper\" }\n",
    );
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let file = w.file("linux");
    assert!(file.contains("in-the-game 1.0.0"), "{file}");
    assert!(!file.contains("build-tool"), "{file}");
    assert!(!file.contains("test-helper"), "{file}");
}

#[test]
fn a_library_only_one_operating_system_builds_is_listed_for_that_system_only() {
    // Basis: Rule (#270): the libraries "actually compiled into the game for
    // that platform": a Windows-only library isn't in the Linux download.
    let w = Workspace::new("per-system");
    w.library("everywhere", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.library("windows-only", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies(
        "[dependencies]\neverywhere = { path = \"../../libs/everywhere\" }\n\
         [target.'cfg(windows)'.dependencies]\nwindows-only = { path = \"../../libs/windows-only\" }\n",
    );
    let (ok, _, err) = w.run(&["licences"]);
    assert!(ok, "{err}");
    for os in ["macos", "windows", "linux"] {
        assert!(w.file(os).contains("everywhere 1.0.0"), "{os}");
    }
    assert!(w.file("windows").contains("windows-only 1.0.0"));
    assert!(!w.file("linux").contains("windows-only"));
    assert!(!w.file("macos").contains("windows-only"));
}

#[test]
fn a_library_with_no_licence_file_gets_the_standard_text_and_a_note() {
    // Basis: Observed (cargo metadata on this repo's Cargo.lock, 23 October
    // 2026): 17 of the game's libraries, such as taffy and parry3d-f64, keep
    // their licence files in the repository above the crate, so the package
    // has none. They are offered under MIT or Apache-2.0, so the file gives
    // the standard text of Apache-2.0, which names no missing copyright
    // holder, and says so.
    let w = Workspace::new("no-licence-file");
    w.library("bare", "0.4.0", "MIT OR Apache-2.0", &[]);
    w.set_game_dependencies("[dependencies]\nbare = { path = \"../../libs/bare\" }\n");
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let file = w.file("linux");
    assert!(
        file.contains("bare 0.4.0\n    licence: MIT OR Apache-2.0\n"),
        "{file}"
    );
    assert!(
        file.contains("note: its package has no licence file"),
        "{file}"
    );
    assert!(file.contains("Apache License"), "{file}");
    assert!(file.contains("Version 2.0, January 2004"), "{file}");
}

#[test]
fn a_library_with_no_licence_file_under_a_licence_with_no_standard_text_is_an_error() {
    // Basis: Rule (#270): a licence text is never made up. A library with no
    // licence file under a licence the tool has no standard text for stops
    // the tool, naming the library.
    let w = Workspace::new("no-standard-text");
    w.library("odd", "1.0.0", "Zlib", &[]);
    w.set_game_dependencies("[dependencies]\nodd = { path = \"../../libs/odd\" }\n");
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(!ok);
    assert!(err.contains("odd 1.0.0"), "{err}");
    assert!(err.contains("no licence file"), "{err}");
}

#[test]
fn a_stale_file_fails_the_check_until_it_is_written_again() {
    // Basis: Rule (#270): the file is generated and kept up to date, so a
    // library added or upgraded after it was written fails `--check`, which
    // CI runs, and passes again when the file is regenerated.
    let w = Workspace::new("stale");
    w.library("first", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies("[dependencies]\nfirst = { path = \"../../libs/first\" }\n");

    let (ok, _, err) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(!ok, "no file yet");
    assert!(err.contains("licences/third-party-linux.txt"), "{err}");
    assert!(err.contains("cargo xtask licences"), "{err}");

    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let (ok, out, err) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(ok, "{err}");
    assert!(out.contains("up to date"), "{out}");

    // The library moves to a new version.
    w.library("first", "1.1.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies("[dependencies]\nfirst = { path = \"../../libs/first\" }\n");
    let (ok, _, err) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(!ok, "the file still says 1.0.0");
    assert!(err.contains("out of date"), "{err}");

    // Someone edits a licence text by hand.
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let path = w.root.join("licences/third-party-linux.txt");
    let edited = fs::read_to_string(&path)
        .unwrap()
        .replace("The Pretty Authors", "Nobody");
    fs::write(&path, edited).unwrap();
    let (ok, _, _) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(!ok, "a hand edit is stale too");

    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let (ok, _, err) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(ok, "{err}");
}

#[test]
fn a_file_checked_out_with_windows_line_endings_is_still_up_to_date() {
    // Basis: Rule (#270): .gitattributes checks text files out with LF, but a
    // file that a Windows editor saved with CRLF is the same file.
    let w = Workspace::new("crlf");
    w.library("first", "1.0.0", "MIT", &[("LICENSE", PRETTY_MIT)]);
    w.set_game_dependencies("[dependencies]\nfirst = { path = \"../../libs/first\" }\n");
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let path = w.root.join("licences/third-party-linux.txt");
    let text = fs::read_to_string(&path).unwrap().replace('\n', "\r\n");
    fs::write(&path, text).unwrap();
    let (ok, _, err) = w.run(&["licences", "--check", "--os", "linux"]);
    assert!(ok, "{err}");
}

#[test]
fn sdl_is_listed_with_its_licence_though_cargo_only_builds_with_its_source() {
    // Basis: Rule (#270, ADR-0018): SDL is built from source and linked into
    // the game, so its Zlib licence has to travel with it, although Cargo
    // knows its source, `sdl3-src`, only as a build dependency of `sdl3-sys`.
    let w = Workspace::new("sdl");
    w.library(
        "sdl3-sys",
        "0.7.2",
        "Zlib",
        &[("LICENSE", "bindings licence\n")],
    );
    w.library(
        "sdl3-src",
        "3.4.18",
        "Zlib",
        &[
            ("LICENSE.md", "the packager's licence\n"),
            ("SDL/LICENSE.txt", "Simple DirectMedia Layer licence\n"),
            ("SDL/src/hidapi/LICENSE-bsd.txt", "HIDAPI BSD licence\n"),
        ],
    );
    let manifest = w.root.join("../libs/sdl3-sys/Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        format!("{text}\n[build-dependencies]\nsdl3-src = {{ path = \"../sdl3-src\" }}\n"),
    )
    .unwrap();
    w.set_game_dependencies("[dependencies]\nsdl3-sys = { path = \"../../libs/sdl3-sys\" }\n");
    let (ok, _, err) = w.run(&["licences", "--os", "linux"]);
    assert!(ok, "{err}");
    let file = w.file("linux");
    assert!(
        file.contains("sdl3-src 3.4.18\n    licence: Zlib\n"),
        "{file}"
    );
    assert!(file.contains("note: SDL itself"), "{file}");
    assert!(file.contains("Simple DirectMedia Layer licence"), "{file}");
    assert!(file.contains("HIDAPI BSD licence"), "{file}");
}
