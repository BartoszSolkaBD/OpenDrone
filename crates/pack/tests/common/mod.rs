//! The fixture Packs the readable checks use, committed in
//! `crates/pack/tests/fixtures/`:
//!
//! - `good/`: a Pack called `fixture` holding one Quad, `fixture/ducted`, with
//!   every section, ducts and a buzzer, and a Test Quad,
//!   `test/ducted-no-drag`.
//! - `broken/`: a Pack with a fine Quad, a Quad with a problem on each line
//!   marked BROKEN, and a Quad in a newer format, plus a Pack whose manifest
//!   is broken.
//!
//! Most checks copy the good fixture into a scratch folder and change one
//! line ([`Fixture::change`]), so each shows exactly what it broke.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use opendrone_pack::Packs;

/// The quad file of the good fixture, as problems name it.
pub const QUAD: &str = "packs/fixture/quads/ducted/quad.toml";
pub const TUNE: &str = "packs/fixture/quads/ducted/tune.txt";
pub const LOG: &str = "packs/fixture/quads/ducted/feel-tests.md";
pub const MANIFEST: &str = "packs/fixture/pack.toml";
pub const TEST_QUAD: &str = "test-quads/ducted-no-drag.toml";

pub fn good_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/good")
}

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A copy of the good fixture in its own scratch folder.
pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    pub fn new(case: &str) -> Fixture {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("pack-fixtures")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        copy(&good_fixture(), &root);
        Fixture { root }
    }

    /// Changes `old` to `new` in one file. `old` must be there exactly once,
    /// so a check can't quietly break nothing.
    pub fn change(self, file: &str, old: &str, new: &str) -> Fixture {
        let text = self.read(file);
        assert_eq!(
            text.matches(old).count(),
            1,
            "{file} should hold {old:?} exactly once"
        );
        self.write(file, &text.replacen(old, new, 1));
        self
    }

    pub fn read(&self, file: &str) -> String {
        fs::read_to_string(self.root.join(file)).unwrap()
    }

    pub fn write(&self, file: &str, text: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn remove(&self, file: &str) {
        fs::remove_file(self.root.join(file)).unwrap();
    }

    /// The fixture's Packs and Test Quads, read the way the Scenario runner
    /// reads the repo's.
    pub fn packs(&self) -> Packs {
        Packs::open(&self.root.join("packs"), "packs")
            .unwrap()
            .with_test_quads(&self.root.join("test-quads"), "test-quads")
    }

    /// Every problem the checker finds, one sentence each.
    pub fn problems(&self) -> Vec<String> {
        self.packs()
            .problems()
            .0
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The problems that kept the fixture's Quad, `fixture/ducted`, out: the
    /// problems of that one item. (Its Test Quad is then refused too, saying
    /// it can't build on it.)
    pub fn quad_problems(&self) -> Vec<String> {
        match self.packs().quad("fixture/ducted") {
            Ok(_) => Vec::new(),
            Err(problems) => problems.0.iter().map(ToString::to_string).collect(),
        }
    }

    /// The line of `file` that holds `text`.
    pub fn line_of(&self, file: &str, text: &str) -> usize {
        line_of(&self.read(file), text)
    }
}

/// Makes `link` a symbolic link to `target`, a path relative to the link's
/// folder, as `ln -s` does. `None` means this computer can't make one: Windows
/// lets only an administrator, or Developer Mode, make symbolic links, so
/// there a check that needs one says why and skips. macOS and Linux CI always
/// run it.
pub fn symbolic_link(target: &str, link: &Path) -> Option<()> {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = if link.parent().unwrap().join(target).is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    match made {
        Ok(()) => Some(()),
        Err(error) if cfg!(windows) => {
            eprintln!(
                "Skipped: Windows didn't make the symbolic link {} ({error}); it needs Developer Mode or an administrator. macOS and Linux CI run this check.",
                link.display()
            );
            None
        }
        Err(error) => panic!("can't make the symbolic link {}: {error}", link.display()),
    }
}

/// The line of `text` that holds `needle`.
pub fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle:?}"))
        + 1
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
