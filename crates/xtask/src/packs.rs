//! `cargo xtask packs` and `cargo xtask feel-tests`: the Pack checker and the
//! Feel Test log rules, as CI runs them on every pull request (#16 §3, #15).
//!
//! - `packs` checks every Pack in `packs/` and every Test Quad in
//!   `scenarios/test-quads/` with the same checker the game uses, and lists
//!   every problem with its file, line and a plain sentence.
//! - `feel-tests --base <revision>` compares every Quad definition with the
//!   one at `<revision>` (in CI, the pull request's base, `HEAD^1`): an
//!   Estimate that moved needs a new row in its Quad's `feel-tests.md` and must
//!   stay inside its range, and a Measured, Manufacturer or Derived number
//!   that changed needs a new source. The rules themselves live in
//!   `opendrone_pack::feel_tests`; this only fetches the old files from git.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use opendrone_pack::Packs;
use opendrone_pack::feel_tests::{QuadVersion, check_feel_test_rules};

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
        let mut problems = Vec::new();
        let quads = quad_folders(&root);
        for folder in &quads {
            let quad = format!("{folder}/quad.toml");
            let log = format!("{folder}/feel-tests.md");
            let now = |file: &str| fs::read_to_string(root.join(file)).ok();
            let (quad_now, log_now) = (now(&quad), now(&log));
            let (quad_before, log_before) = (git.show(&quad)?, git.show(&log)?);
            let found = check_feel_test_rules(
                &quad,
                &log,
                QuadVersion {
                    quad: quad_before.as_deref(),
                    feel_tests: log_before.as_deref(),
                },
                QuadVersion {
                    quad: quad_now.as_deref(),
                    feel_tests: log_now.as_deref(),
                },
            );
            problems.extend(found.0.iter().map(ToString::to_string));
        }
        Ok((quads.len(), problems))
    });
    match checked {
        Err(message) => {
            eprintln!("The Feel Test log rules can't be checked: {message}");
            ExitCode::from(2)
        }
        Ok((quads, problems)) if problems.is_empty() => {
            println!(
                "The Feel Test log rules hold for all {quads} Quad(s), compared with {base}: every Estimate that moved has its log row and stays inside its range, and every locked number that changed has a new source."
            );
            ExitCode::SUCCESS
        }
        Ok((_, problems)) => {
            println!("The Feel Test log rules are broken, compared with {base}:");
            for problem in &problems {
                println!("- {problem}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Every Quad's folder in the working tree, as `packs/<pack>/quads/<quad>`.
fn quad_folders(root: &Path) -> Vec<String> {
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
    let mut folders = Vec::new();
    for pack in names(&root.join("packs")) {
        for quad in names(&root.join("packs").join(&pack).join("quads")) {
            folders.push(format!("packs/{pack}/quads/{quad}"));
        }
    }
    folders
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
