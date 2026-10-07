//! `cargo xtask packs` and `cargo xtask feel-tests`: the Pack checker and the
//! Feel Test log rules, as CI runs them on every pull request (#16 §3, #15).
//!
//! - `packs` checks every Pack in `packs/` and every Test Quad in
//!   `scenarios/test-quads/` with the same checker the game uses, and lists
//!   every problem with its file, line and a plain sentence.
//! - `feel-tests --base <revision>` compares every Quad definition with the
//!   same Quad at `<revision>` (in CI, the pull request's base, `HEAD^1`),
//!   paired by id: the Pack's id from its `pack.toml` and the Quad's folder
//!   name, so moving a folder changes nothing. An Estimate that moved needs a
//!   new row in its Quad's `feel-tests.md` and must stay inside its range,
//!   and a Measured, Manufacturer or Derived number that changed needs a new
//!   source. Every number that passed only because its source changed is
//!   listed, for the Reviewer. The rules themselves live in
//!   `opendrone_pack::feel_tests`; this only reads both versions' files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use opendrone_pack::feel_tests::{QuadFiles, compare_packs};
use opendrone_pack::{Packs, read_manifest};

/// The repo around the current folder: the nearest folder holding `packs/`.
fn repo() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| format!("can't tell where this is: {e}"))?;
    here.ancestors()
        .find(|folder| folder.join("packs").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "Run this inside the OpenDrone repo: no folder here or above holds packs/.".into()
        })
}

pub fn run_packs(args: &[String]) -> ExitCode {
    if !args.is_empty() {
        eprintln!("Usage: cargo xtask packs");
        return ExitCode::from(2);
    }
    let root = match repo() {
        Ok(root) => root,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let packs = match Packs::open(&root.join("packs"), "packs") {
        Ok(packs) => {
            packs.with_test_quads(&root.join("scenarios/test-quads"), "scenarios/test-quads")
        }
        Err(problems) => {
            println!("The Packs can't be read:\n{problems}");
            return ExitCode::FAILURE;
        }
    };
    let problems = packs.problems();
    if problems.is_empty() {
        let ids = |quads: Vec<&opendrone_pack::QuadDefinition>| {
            quads
                .iter()
                .map(|q| q.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "Every Pack passes the Pack checker: {} Pack(s) ({}), the Quads {}, and the Test Quads {}.",
            packs.manifests().len(),
            packs
                .manifests()
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            ids(packs.quads()),
            ids(packs.test_quads()),
        );
        return ExitCode::SUCCESS;
    }
    println!("The Pack checker found {} problem(s):", problems.0.len());
    for problem in &problems.0 {
        println!("- {problem}");
    }
    ExitCode::FAILURE
}

pub fn run_feel_tests(args: &[String]) -> ExitCode {
    let [flag, base] = args else {
        eprintln!("Usage: cargo xtask feel-tests --base <git revision>");
        return ExitCode::from(2);
    };
    if flag != "--base" {
        eprintln!("Usage: cargo xtask feel-tests --base <git revision>");
        return ExitCode::from(2);
    }
    let checked = repo().and_then(|root| {
        let git = Git { root: &root, base };
        git.check_base()?;
        let before = quads_at_base(&git)?;
        let after = quads_now(&root);
        Ok((after.len(), compare_packs(&before, &after)))
    });
    let (quads, report) = match checked {
        Err(message) => {
            eprintln!("The Feel Test log rules can't be checked: {message}");
            return ExitCode::from(2);
        }
        Ok(found) => found,
    };
    if !report.passed_on_a_new_source.is_empty() {
        println!(
            "These changed and passed only because their source changed too; the Reviewer judges whether each new source is real:"
        );
        for line in &report.passed_on_a_new_source {
            println!("- {line}");
        }
    }
    if !report.changed_without_a_confidence.is_empty() {
        println!(
            "These changed and carry no Confidence (counts and choices), so the rules can't judge them; the Reviewer checks them:"
        );
        for line in &report.changed_without_a_confidence {
            println!("- {line}");
        }
    }
    if report.problems.is_empty() {
        println!(
            "The Feel Test log rules hold for all {quads} Quad(s), compared with {base}: every Estimate that moved has its log row and stays inside its range, and every locked number that changed has a new source."
        );
        return ExitCode::SUCCESS;
    }
    println!("The Feel Test log rules are broken, compared with {base}:");
    for problem in &report.problems.0 {
        println!("- {problem}");
    }
    ExitCode::FAILURE
}

/// A Pack's id from its `pack.toml`, or its folder's name when the manifest
/// can't be read (the Pack checker says why).
fn pack_id(folder: &str, manifest: Option<&str>) -> String {
    manifest
        .and_then(|text| read_manifest("pack.toml", text).ok())
        .map_or_else(|| folder.to_string(), |manifest| manifest.id)
}

/// Every Quad in the working tree, by id.
fn quads_now(root: &Path) -> Vec<QuadFiles> {
    let names = |folder: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    let read = |path: &str| fs::read_to_string(root.join(path)).ok();
    let mut quads = Vec::new();
    for pack in names(&root.join("packs")) {
        let id = pack_id(&pack, read(&format!("packs/{pack}/pack.toml")).as_deref());
        for quad in names(&root.join("packs").join(&pack).join("quads")) {
            let folder = format!("packs/{pack}/quads/{quad}");
            let Some(text) = read(&format!("{folder}/quad.toml")) else {
                continue;
            };
            quads.push(QuadFiles {
                id: format!("{id}/{quad}"),
                quad_file: format!("{folder}/quad.toml"),
                log_file: format!("{folder}/feel-tests.md"),
                quad: text,
                feel_tests: read(&format!("{folder}/feel-tests.md")),
            });
        }
    }
    quads
}

/// Every Quad at the base revision, by id, read from git.
fn quads_at_base(git: &Git<'_>) -> Result<Vec<QuadFiles>, String> {
    let files = git.list("packs")?;
    let mut quads = Vec::new();
    for path in &files {
        let parts: Vec<&str> = path.split('/').collect();
        let ["packs", pack, "quads", quad, "quad.toml"] = parts.as_slice() else {
            continue;
        };
        let manifest = git.show(&format!("packs/{pack}/pack.toml"))?;
        let id = pack_id(pack, manifest.as_deref());
        let folder = format!("packs/{pack}/quads/{quad}");
        let Some(text) = git.show(path)? else {
            continue;
        };
        quads.push(QuadFiles {
            id: format!("{id}/{quad}"),
            quad_file: format!("{folder}/quad.toml"),
            log_file: format!("{folder}/feel-tests.md"),
            quad: text,
            feel_tests: git.show(&format!("{folder}/feel-tests.md"))?,
        });
    }
    Ok(quads)
}

/// The files of one revision, read with git.
struct Git<'a> {
    root: &'a Path,
    base: &'a str,
}

impl Git<'_> {
    fn run(&self, args: &[&str]) -> Result<std::process::Output, String> {
        Command::new("git")
            .args(args)
            .current_dir(self.root)
            .output()
            .map_err(|e| format!("git doesn't run: {e}"))
    }

    fn check_base(&self) -> Result<(), String> {
        let found = self.run(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{}^{{commit}}", self.base),
        ])?;
        if found.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git has no revision \"{}\" here; in CI the base is HEAD^1, which needs a checkout with fetch-depth 2",
                self.base
            ))
        }
    }

    /// Every file under `folder` in the base revision, with `/` between
    /// folders.
    fn list(&self, folder: &str) -> Result<Vec<String>, String> {
        let listed = self.run(&["ls-tree", "-r", "--name-only", self.base, "--", folder])?;
        if !listed.status.success() {
            return Err(format!(
                "git can't list {folder} at {}: {}",
                self.base,
                String::from_utf8_lossy(&listed.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&listed.stdout)
            .lines()
            .map(str::to_string)
            .collect())
    }

    /// The file at `path` in the base revision, or `None` when it wasn't
    /// there.
    fn show(&self, path: &str) -> Result<Option<String>, String> {
        let exists = self.run(&["cat-file", "-e", &format!("{}:{path}", self.base)])?;
        if !exists.status.success() {
            return Ok(None);
        }
        let shown = self.run(&["show", &format!("{}:{path}", self.base)])?;
        if !shown.status.success() {
            return Err(format!(
                "git can't show {path} at {}: {}",
                self.base,
                String::from_utf8_lossy(&shown.stderr).trim()
            ));
        }
        String::from_utf8(shown.stdout)
            .map(Some)
            .map_err(|_| format!("{path} at {} isn't text", self.base))
    }
}
