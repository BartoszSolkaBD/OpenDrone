//! Reads and checks Pack files, and pulls colliders and the Launch Spot out of
//! each Map's `.glb`.
//!
//! An edge crate ([ADR-0003]): it reads files, and hands the Simulation checked
//! plain data.
//!
//! So far it reads:
//!
//! - [`units`]: the shared unit list, which reads every number in Scenario and
//!   Pack files. The Scenario runner uses it too.
//! - Each Pack's manifest, `pack.toml` ([`read_manifest`]).
//! - The part of a Quad definition the Simulation uses so far: mass, inertia
//!   and drag, each with its Confidence and source ([`QuadDefinition`]).
//! - Test Quads, which build on a real Quad with `based_on` and list only what
//!   they change. Only the Scenario runner reads them.
//! - The Test Maps built into the code, such as `test/empty-air`
//!   ([`MapDefinition`]).
//!
//! Every problem names its file, its line and a plain sentence
//! ([`Problems`]). The full Pack checker, with every rule of #16, arrives with
//! #40.
//!
//! The numbers it hands the Simulation must be the same on every computer
//! (ADR-0001), so it converts units with multiplication and division only, and
//! never with std's maths functions.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

pub mod document;
mod manifest;
mod map;
mod quad;
pub mod units;

use std::fs;
use std::path::{Path, PathBuf};

pub use document::{Problem, Problems};
pub use manifest::{Manifest, is_an_id, read_manifest};
pub use map::{MapDefinition, read_map_file, test_map, test_map_ids};
pub use quad::{
    Confidence, QuadDefinition, QuadFile, Setting, quad_definition, read_quad_file, read_test_quad,
};

/// The Packs in a folder, and where the Test Quads are.
#[derive(Clone, Debug)]
pub struct Packs {
    packs: Vec<OpenPack>,
    test_quads: Option<Folder>,
}

#[derive(Clone, Debug)]
struct OpenPack {
    folder: Folder,
    manifest: Manifest,
}

/// A folder on disk, and how problems name it.
#[derive(Clone, Debug)]
struct Folder {
    path: PathBuf,
    label: String,
}

impl Folder {
    fn child(&self, name: &str) -> Folder {
        Folder {
            path: self.path.join(name),
            label: format!("{}/{name}", self.label),
        }
    }

    fn read(&self) -> Result<String, Problems> {
        fs::read_to_string(&self.path)
            .map_err(|error| Problems::of_file(&self.label, format!("can't be read: {error}")))
    }
}

impl Packs {
    /// Opens every Pack in `folder`: each folder in it that holds a
    /// `pack.toml`. Problems name files starting with `label`, such as
    /// `packs`. A broken manifest skips the whole Pack, and its problems come
    /// back with the rest.
    pub fn open(folder: &Path, label: &str) -> Result<Packs, Problems> {
        let root = Folder {
            path: folder.to_path_buf(),
            label: label.to_string(),
        };
        let mut names: Vec<String> = fs::read_dir(folder)
            .map_err(|error| Problems::of_file(label, format!("can't be read: {error}")))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("pack.toml").is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        let mut problems = Problems::new();
        let mut packs = Vec::new();
        for name in names {
            let folder = root.child(&name);
            let manifest_file = folder.child("pack.toml");
            match manifest_file
                .read()
                .and_then(|text| read_manifest(&manifest_file.label, &text))
            {
                Ok(manifest) => packs.push(OpenPack { folder, manifest }),
                Err(found) => problems.extend(found),
            }
        }
        problems.or(Packs {
            packs,
            test_quads: None,
        })
    }

    /// Where the Test Quads are, such as `scenarios/test-quads`.
    pub fn with_test_quads(mut self, folder: &Path, label: &str) -> Packs {
        self.test_quads = Some(Folder {
            path: folder.to_path_buf(),
            label: label.to_string(),
        });
        self
    }

    /// The manifests of every open Pack, in folder order.
    pub fn manifests(&self) -> Vec<&Manifest> {
        self.packs.iter().map(|pack| &pack.manifest).collect()
    }

    /// The checked Quad definition with this id, such as `opendrone/whoop-65`,
    /// or a Test Quad such as `test/whoop-65-no-drag`.
    pub fn quad(&self, id: &str) -> Result<QuadDefinition, Problems> {
        if let Some(name) = id.strip_prefix("test/") {
            let folder = self.test_quads.as_ref().ok_or_else(|| {
                Problems::of_file(id, "Test Quads are read only by the Scenario runner")
            })?;
            if !is_an_id(name) {
                return Err(Problems::of_file(id, not_an_id(id)));
            }
            let file = folder.child(&format!("{name}.toml"));
            let text = file.read()?;
            let (based_on, quad) =
                read_test_quad(&file.label, &text, |based_on| self.quad_file(based_on))?;
            return quad_definition(id, Some(based_on), &quad);
        }
        let quad = self.quad_file(id)?;
        quad_definition(id, None, &quad)
    }

    /// The Map with this id. So far only the built-in Test Maps exist.
    pub fn map(&self, id: &str) -> Result<MapDefinition, Problems> {
        if let Some(name) = id.strip_prefix("test/")
            && let Some(map) = test_map(name)
        {
            return map;
        }
        Err(Problems::of_file(
            id,
            format!(
                "there's no Map with this id; so far only the built-in Test Maps exist: {}",
                test_map_ids().join(", ")
            ),
        ))
    }

    fn quad_file(&self, id: &str) -> Result<QuadFile, Problems> {
        let Some((pack, name)) = id.split_once('/') else {
            return Err(Problems::of_file(id, not_an_id(id)));
        };
        if !is_an_id(pack) || !is_an_id(name) {
            return Err(Problems::of_file(id, not_an_id(id)));
        }
        let pack = self
            .packs
            .iter()
            .find(|open| open.manifest.id == pack)
            .ok_or_else(|| {
                Problems::of_file(id, format!("there's no Pack with the id \"{pack}\""))
            })?;
        let file = pack.folder.child("quads").child(name).child("quad.toml");
        if !file.path.is_file() {
            return Err(Problems::of_file(
                id,
                format!("there's no Quad here: {} doesn't exist", file.label),
            ));
        }
        let text = file.read()?;
        read_quad_file(&file.label, &text)
    }
}

fn not_an_id(id: &str) -> String {
    format!(
        "\"{id}\" isn't an id: write the Pack's id, a slash and the item's folder name, such as \"opendrone/whoop-65\""
    )
}
