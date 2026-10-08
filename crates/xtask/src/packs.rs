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
//!   source. A Quad taken out must be renamed, moved or retired in its
//!   Pack's `[retired]` list. Every number that passed on its source alone,
//!   every count or choice the Simulation receives that changed, every new
//!   Quad and every Quad taken out are listed, for the Reviewer, and so is
//!   how many Quads were compared. The rules themselves live in
//!   `opendrone_pack::feel_tests`; this only reads both versions' files.
//! - Both read `packs/` by the Pack checker's rule: a folder whose name starts
//!   with a dot, and a symbolic link, are refused, and nothing in them is
//!   read. In the working tree one blocks; at the base it's named.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use opendrone_pack::feel_tests::{PacksVersion, QuadFiles, RetiredQuad, compare_packs};
use opendrone_pack::{
    Manifest, Packs, Problem, Problems, REFUSED_DOT_FOLDER, REFUSED_LINK, dot_folders_and_links,
    folder_refused, read_manifest,
};

/// The repo around the current folder: the nearest folder holding `packs/`.
pub(crate) fn repo() -> Result<PathBuf, String> {
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
            "Every Pack passes the Pack checker: {} Pack(s) ({}), the Quads {}, the Test Quads {}, and the Input Device profiles {}.",
            packs.manifests().len(),
            packs
                .manifests()
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            ids(packs.quads()),
            ids(packs.test_quads()),
            packs
                .input_devices()
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
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
        let mut report = compare_packs(&before.packs, &after.packs);
        // This change can fix a dot-named folder or link in its own files,
        // so one blocks it. One already at the base can't be fixed by this
        // change, only removed by it, so it's named, not blocked on.
        let mut problems = after.refused;
        problems.extend(report.problems);
        report.problems = problems;
        Ok((report, before.refused))
    });
    let (report, refused_at_base) = match checked {
        Err(message) => {
            eprintln!("The Feel Test log rules can't be checked: {message}");
            return ExitCode::from(2);
        }
        Ok(found) => found,
    };
    if !report.passed_on_a_new_source.is_empty() {
        println!(
            "These passed on their source alone (changed with a new source, re-sourced, taken out, or added); the Reviewer judges whether each source is real:"
        );
        for line in &report.passed_on_a_new_source {
            println!("- {line}");
        }
    }
    if !report.changed_without_a_confidence.is_empty() {
        println!(
            "These changed and carry no Confidence (counts and choices the Simulation receives), so the rules can't judge them; the Reviewer checks them:"
        );
        for line in &report.changed_without_a_confidence {
            println!("- {line}");
        }
    }
    if !report.new_quads.is_empty() {
        println!(
            "New Quads, with no version at {base} to compare: {}",
            report.new_quads.join(", ")
        );
    }
    if !report.previously_retired.is_empty() {
        println!(
            "New, previously retired Quads, with no version at {base} to compare; the Reviewer compares each with its numbers from before it was retired:"
        );
        for line in &report.previously_retired {
            println!("- {line}");
        }
    }
    if !report.taken_out.is_empty() {
        println!(
            "Quads this change takes out, each renamed, moved or retired; the Reviewer checks each one:"
        );
        for line in &report.taken_out {
            println!("- {line}");
        }
    }
    if !refused_at_base.is_empty() {
        println!("At {base}, these broke the Pack rules, so nothing in them was compared:");
        for problem in &refused_at_base.0 {
            println!("- {problem}");
        }
    }
    if report.problems.is_empty() {
        let compared = if report.compared.is_empty() {
            "none".to_string()
        } else {
            report.compared.join(", ")
        };
        println!(
            "The Feel Test log rules hold for the {} Quad(s) compared with their version at {base} ({compared}): every Estimate that moved has its log row and stays inside its range, every locked number that changed has a new source, and every Quad taken out was renamed, moved or retired.",
            report.compared.len(),
        );
        return ExitCode::SUCCESS;
    }
    println!("The Feel Test log rules are broken, compared with {base}:");
    for problem in &report.problems.0 {
        println!("- {problem}");
    }
    ExitCode::FAILURE
}

/// One version of the repo's Packs, as the Feel Test log rules see it.
#[derive(Default)]
struct Version {
    /// Every Quad, and every Quad a Pack's `[retired]` list names.
    packs: PacksVersion,
    /// Every folder whose name starts with a dot and every symbolic link
    /// under `packs/`, or `packs/` itself when it's one, which the Pack
    /// checker refuses; nothing in them is read.
    refused: Problems,
}

/// A Pack's `[retired]` lines, as the Feel Test log rules name them.
fn retired_quads(id: &str, manifest_file: &str, manifest: Option<&Manifest>) -> Vec<RetiredQuad> {
    manifest
        .iter()
        .flat_map(|manifest| &manifest.retired)
        .map(|line| RetiredQuad {
            id: format!("{id}/{}", line.quad),
            manifest_file: manifest_file.to_string(),
            line: line.line,
            why: line.why.clone(),
        })
        .collect()
}

/// A Pack's manifest from its `pack.toml`, if it reads (the Pack checker
/// says why when it doesn't).
fn manifest(text: Option<&str>) -> Option<Manifest> {
    text.and_then(|text| read_manifest("pack.toml", text).ok())
}

/// A Pack's id from its manifest, or its folder's name when the manifest
/// can't be read.
fn pack_id(folder: &str, manifest: Option<&Manifest>) -> String {
    manifest.map_or_else(|| folder.to_string(), |manifest| manifest.id.clone())
}

/// Every Quad in the working tree, by id, read by the Pack checker's rule:
/// a folder whose name starts with a dot, and a symbolic link, are refused,
/// and nothing in them is read. When `packs/` itself is a link, no Quad is
/// read, as at the base, where git keeps the link as one small file.
fn quads_now(root: &Path) -> Version {
    let mut version = Version {
        refused: dot_folders_and_links(&root.join("packs"), "packs"),
        ..Version::default()
    };
    if folder_refused(&root.join("packs")).is_some() {
        return version;
    }
    let names = |folder: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    // A folder itself, not a link to one.
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    let read = |path: &str| {
        let path = root.join(path);
        let link = fs::symlink_metadata(&path).is_ok_and(|found| found.file_type().is_symlink());
        if link {
            None
        } else {
            fs::read_to_string(path).ok()
        }
    };
    let PacksVersion { quads, retired } = &mut version.packs;
    for pack in names(&root.join("packs")) {
        let manifest_file = format!("packs/{pack}/pack.toml");
        let manifest = manifest(read(&manifest_file).as_deref());
        let id = pack_id(&pack, manifest.as_ref());
        retired.extend(retired_quads(&id, &manifest_file, manifest.as_ref()));
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
    version
}

/// Git's mode for a symbolic link.
const LINK_MODE: &str = "120000";

/// Every Quad at the base revision, by id, read from git by the same rule as
/// the working tree: a folder whose name starts with a dot, and a symbolic
/// link, are refused, and nothing in them is read. Git keeps a link as one
/// small file holding where it points, so there's nothing behind it to read
/// anyway.
fn quads_at_base(git: &Git<'_>) -> Result<Version, String> {
    let mut version = Version::default();
    let mut refuse = |path: String, sentence: &str| {
        let problem = Problem::of(path, 0, sentence);
        if !version.refused.0.contains(&problem) {
            version.refused.push(problem);
        }
    };
    let mut files = BTreeSet::new();
    for (mode, path) in git.list("packs")? {
        let parts: Vec<&str> = path.split('/').collect();
        let folders = &parts[..parts.len() - 1];
        if let Some(dot) = folders.iter().position(|name| name.starts_with('.')) {
            refuse(parts[..=dot].join("/"), REFUSED_DOT_FOLDER);
        } else if mode == LINK_MODE {
            refuse(path, REFUSED_LINK);
        } else {
            files.insert(path);
        }
    }
    let show = |path: &str| -> Result<Option<String>, String> {
        if files.contains(path) {
            git.show(path)
        } else {
            Ok(None)
        }
    };
    // Each Pack's manifest at the base, by its folder: its id, and the Quads
    // its `[retired]` list names.
    let mut manifests: BTreeMap<String, Option<Manifest>> = BTreeMap::new();
    for path in &files {
        let parts: Vec<&str> = path.split('/').collect();
        if let ["packs", pack, "pack.toml"] = parts.as_slice() {
            let manifest = manifest(show(path)?.as_deref());
            let id = pack_id(pack, manifest.as_ref());
            let retired = retired_quads(&id, path, manifest.as_ref());
            version.packs.retired.extend(retired);
            manifests.insert(pack.to_string(), manifest);
        }
    }
    for path in &files {
        let parts: Vec<&str> = path.split('/').collect();
        let ["packs", pack, "quads", quad, "quad.toml"] = parts.as_slice() else {
            continue;
        };
        let manifest = manifests.get(*pack).and_then(Option::as_ref);
        let id = pack_id(pack, manifest);
        let folder = format!("packs/{pack}/quads/{quad}");
        let Some(text) = show(path)? else {
            continue;
        };
        version.packs.quads.push(QuadFiles {
            id: format!("{id}/{quad}"),
            quad_file: format!("{folder}/quad.toml"),
            log_file: format!("{folder}/feel-tests.md"),
            quad: text,
            feel_tests: show(&format!("{folder}/feel-tests.md"))?,
        });
    }
    Ok(version)
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

    /// Every file under `folder` in the base revision, with its git mode
    /// (such as `100644` for a file, or `120000` for a symbolic link) and its
    /// path, with `/` between folders.
    fn list(&self, folder: &str) -> Result<Vec<(String, String)>, String> {
        // `-z` gives each entry as it is, ended by a NUL byte. Without it, git
        // quotes a path that isn't plain ASCII, such as
        // "packs/opendrone-\305\202\303\263d\305\272/…", which then matches no
        // Quad, so the rules would compare nothing. Each entry is
        // "<mode> <type> <object>", a tab, then the path.
        let listed = self.run(&["ls-tree", "-r", "-z", self.base, "--", folder])?;
        if !listed.status.success() {
            return Err(format!(
                "git can't list {folder} at {}: {}",
                self.base,
                String::from_utf8_lossy(&listed.stderr).trim()
            ));
        }
        listed
            .stdout
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let tab = entry.iter().position(|byte| *byte == b'\t');
                let (about, path) = tab
                    .map(|tab| (&entry[..tab], &entry[tab + 1..]))
                    .ok_or_else(|| {
                        format!(
                            "git listed \"{}\" at {} in a way this doesn't read",
                            String::from_utf8_lossy(entry),
                            self.base
                        )
                    })?;
                let mode = String::from_utf8_lossy(about)
                    .split(' ')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let path = String::from_utf8(path.to_vec()).map_err(|_| {
                    format!(
                        "{} at {} has a name that isn't UTF-8 text, so it can't be compared; rename it",
                        String::from_utf8_lossy(path),
                        self.base
                    )
                })?;
                Ok((mode, path))
            })
            .collect()
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
