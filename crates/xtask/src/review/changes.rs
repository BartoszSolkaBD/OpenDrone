//! What a pull request changes: every file that differs between the base and
//! the head, with a way to read either version of any file.
//!
//! In CI the two sides are git commits, read as data with `git show`: nothing
//! from the pull request is run. The readable checks use two folders instead.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One side of the comparison: the base or the head.
#[derive(Clone, Debug)]
pub enum Side {
    /// A git commit of the repository in the current folder.
    Commit(String),
    /// A folder holding a whole copy of the repository.
    Folder(PathBuf),
}

impl Side {
    /// The file at `path` (from the repository's root, with `/` between
    /// folders), or `None` if this side has no such file.
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        match self {
            Side::Commit(rev) => {
                let output = Command::new("git")
                    .args(["show", &format!("{rev}:{path}")])
                    .output()
                    .ok()?;
                output.status.success().then_some(output.stdout)
            }
            Side::Folder(folder) => fs::read(folder.join(path)).ok(),
        }
    }

    /// The file at `path` as text, if it exists and is text.
    pub fn text(&self, path: &str) -> Option<String> {
        String::from_utf8(self.read(path)?).ok()
    }
}

/// Every file a pull request changes, and both sides to read them from.
#[derive(Clone, Debug)]
pub struct Changes {
    pub base: Side,
    pub head: Side,
    /// Every path that was added, deleted or changed, in order. A renamed
    /// file counts as its old path deleted and its new path added.
    pub paths: Vec<String>,
}

impl Changes {
    /// The changes between two git commits of the repository in the current
    /// folder.
    pub fn between_commits(base: &str, head: &str) -> Result<Changes, String> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "--no-renames", "-z", base, head])
            .output()
            .map_err(|error| format!("couldn't run git: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git couldn't compare {base} and {head}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let mut paths: Vec<String> = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect();
        paths.sort();
        Ok(Changes {
            base: Side::Commit(base.to_string()),
            head: Side::Commit(head.to_string()),
            paths,
        })
    }

    /// The changes between two folders, each a whole copy of the repository.
    pub fn between_folders(base: &Path, head: &Path) -> Result<Changes, String> {
        let mut every = BTreeSet::new();
        for folder in [base, head] {
            if !folder.is_dir() {
                return Err(format!("{} isn't a folder", folder.display()));
            }
            walk(folder, "", &mut every)
                .map_err(|error| format!("couldn't read {}: {error}", folder.display()))?;
        }
        let paths = every
            .into_iter()
            .filter(|path| fs::read(base.join(path)).ok() != fs::read(head.join(path)).ok())
            .collect();
        Ok(Changes {
            base: Side::Folder(base.to_path_buf()),
            head: Side::Folder(head.to_path_buf()),
            paths,
        })
    }

    /// The changed paths that start with `prefix`.
    pub fn under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.paths
            .iter()
            .map(String::as_str)
            .filter(move |path| path.starts_with(prefix))
    }

    /// Whether `path` changed.
    pub fn touches(&self, path: &str) -> bool {
        self.paths
            .binary_search_by(|p| p.as_str().cmp(path))
            .is_ok()
    }

    /// The lines of `path` on the head side that aren't on the base side: the
    /// lines this pull request adds, counted so a line it adds a second time
    /// counts too.
    pub fn added_lines(&self, path: &str) -> Vec<String> {
        let base = self.base.text(path).unwrap_or_default();
        let head = self.head.text(path).unwrap_or_default();
        let mut base_lines: Vec<&str> = base.lines().collect();
        base_lines.sort_unstable();
        let mut added = Vec::new();
        for line in head.lines() {
            match base_lines.binary_search(&line) {
                Ok(index) => {
                    base_lines.remove(index);
                }
                Err(_) => added.push(line.to_string()),
            }
        }
        added
    }
}

fn walk(folder: &Path, prefix: &str, paths: &mut BTreeSet<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(folder.join(prefix))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type()?.is_dir() {
            walk(folder, &relative, paths)?;
        } else {
            paths.insert(relative);
        }
    }
    Ok(())
}
