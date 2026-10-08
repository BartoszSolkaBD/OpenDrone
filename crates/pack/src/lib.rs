//! The Pack checker: reads Pack folders, refuses anything that breaks the
//! Pack rules, and hands the Simulation checked content with its fingerprints
//! (#16, ADR-0011). It also pulls colliders and the Launch Spot out of each
//! Map's `.glb`, once Maps arrive (#63).
//!
//! An edge crate ([ADR-0003]): it reads files, and hands the Simulation
//! checked plain data.
//!
//! - [`Packs::open`] checks every Pack in a folder: each one's manifest
//!   ([`read_manifest`]), and every item, found by its folder and named
//!   `<pack>/<item>`. So far the items are Quads: `quads/<id>/` holds
//!   `quad.toml` ([`read_quad_file`], [`check_quad`]), `tune.txt`
//!   ([`read_tune`]), `picture.png` and the Feel Test log, `feel-tests.md`
//!   ([`feel_tests`]); `input-devices/<id>.toml` holds an Input Device
//!   profile ([`input_device`]).
//! - Every problem names its file, its line and a plain sentence
//!   ([`Problems`]), and all of them are listed at once. A broken item is
//!   skipped and the rest of its Pack loads; a broken manifest skips the
//!   whole Pack.
//! - [`Packs::with_test_quads`] adds the Test Quads, which build on a real
//!   Quad with `based_on` and list only what they change. Only the Scenario
//!   runner reads them.
//! - [`units`] is the shared unit list, which reads every number in Scenario
//!   and Pack files. The Scenario runner uses it too.
//! - The Test Maps built into the code, such as `test/empty-air`
//!   ([`MapDefinition`]).
//!
//! Every file starts with `format = N`: a newer format is refused ("needs a
//! newer OpenDrone"), and an older one is upgraded in memory
//! ([`document::upgraded`]).
//!
//! The numbers it hands the Simulation must be the same on every computer
//! (ADR-0001), so it converts units with multiplication and division only, and
//! never with std's maths functions.
//!
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

pub mod document;
pub mod feel_tests;
pub mod input_device;
mod manifest;
mod map;
mod quad;
pub mod schema;
mod tune;
pub mod units;
mod values;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub use document::{Problem, Problems};
pub use input_device::{InputDeviceProfile, read_input_device_file};
pub use manifest::{LicenceOverride, Manifest, is_an_id, read_manifest};
pub use map::{MapDefinition, read_map_file, test_map, test_map_ids};
pub use quad::{
    Battery, Board, Camera, Chemistry, Collision, Confidence, Ducts, Feel, Frame, Motors,
    PropDirection, Props, QuadDefinition, QuadFile, Setting, Sound, Value, check_quad, label,
    read_quad_file, read_test_quad,
};
pub use tune::{Tune, TuneSetting, read_tune};

use quad::{ReadInFull, check_quad_as_read, read_quad_file_as_far_as_it_goes};
use tune::read_tune_as_far_as_it_goes;

/// The kinds of item a Pack may hold, each in its own folder. Maps are known,
/// but not read yet (#63).
const KINDS: &[&str] = &["quads", "maps", "input-devices"];

/// The first bytes of every PNG file.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The checked Packs in a folder, every problem found in them, and the Test
/// Quads.
#[derive(Clone, Debug)]
pub struct Packs {
    packs: Vec<OpenPack>,
    /// Every Quad by id.
    quads: BTreeMap<String, QuadItem>,
    /// Every Input Device profile by id, checked or with its problems.
    input_devices: BTreeMap<String, Result<InputDeviceProfile, Problems>>,
    /// Packs skipped because their manifest is broken.
    skipped: Vec<String>,
    problems: Problems,
    test_quads: Option<TestQuads>,
}

/// One Quad of a Pack: the checked definition, or every problem with it, and
/// its files, for Test Quads to build on.
#[derive(Clone, Debug)]
struct QuadItem {
    checked: Result<QuadDefinition, Problems>,
    files: Option<(QuadFile, Tune)>,
}

#[derive(Clone, Debug)]
struct OpenPack {
    folder: Folder,
    manifest: Manifest,
}

#[derive(Clone, Debug)]
struct TestQuads {
    folder: Folder,
    quads: BTreeMap<String, Result<QuadDefinition, Problems>>,
    problems: Problems,
}

/// A folder or file on disk, and how problems name it.
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

    /// The names in this folder, in order, leaving out hidden ones such as
    /// `.DS_Store`.
    fn names(&self) -> Result<Vec<String>, Problems> {
        let mut names: Vec<String> = fs::read_dir(&self.path)
            .map_err(|error| Problems::of_file(&self.label, format!("can't be read: {error}")))?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort();
        Ok(names)
    }
}

impl Packs {
    /// Opens and checks every Pack in `folder`. Problems name files starting
    /// with `label`, such as `packs`. Only a folder that can't be read at all
    /// is an error; every other problem is kept in [`Packs::problems`], with
    /// the broken item, or the Pack whose manifest is broken, left out.
    pub fn open(folder: &Path, label: &str) -> Result<Packs, Problems> {
        let root = Folder {
            path: folder.to_path_buf(),
            label: label.to_string(),
        };
        let mut packs = Packs {
            packs: Vec::new(),
            quads: BTreeMap::new(),
            input_devices: BTreeMap::new(),
            skipped: Vec::new(),
            problems: Problems::new(),
            test_quads: None,
        };
        for name in root.names()? {
            let folder = root.child(&name);
            if !folder.path.is_dir() {
                continue;
            }
            let manifest_file = folder.child("pack.toml");
            if !manifest_file.path.is_file() {
                packs.problems.extend(Problems::of_file(
                    &folder.label,
                    "has no pack.toml, so it isn't a Pack",
                ));
                continue;
            }
            let manifest = match manifest_file
                .read()
                .and_then(|text| read_manifest(&manifest_file.label, &text))
            {
                Ok(manifest) => manifest,
                Err(found) => {
                    packs.problems.extend(found);
                    packs.problems.extend(Problems::of_file(
                        &folder.label,
                        "its manifest (pack.toml) is broken, so the whole Pack is skipped",
                    ));
                    packs.skipped.push(folder.label);
                    continue;
                }
            };
            if let Some(first) = packs.packs.iter().find(|p| p.manifest.id == manifest.id) {
                packs.problems.extend(Problems::of_file(
                    &manifest_file.label,
                    format!(
                        "{} already has the id \"{}\", and every Pack's id is its own, so this Pack is skipped",
                        first.folder.label, manifest.id
                    ),
                ));
                packs.skipped.push(folder.label);
                continue;
            }
            packs.open_items(&folder, &manifest.id);
            packs.packs.push(OpenPack { folder, manifest });
        }
        Ok(packs)
    }

    /// Finds and checks every item in a Pack's folder.
    fn open_items(&mut self, folder: &Folder, pack: &str) {
        let names = match folder.names() {
            Ok(names) => names,
            Err(found) => return self.problems.extend(found),
        };
        for name in names {
            let kind = folder.child(&name);
            if !kind.path.is_dir() {
                continue;
            }
            match name.as_str() {
                "quads" => self.open_quads(&kind, pack),
                "input-devices" => self.open_input_devices(&kind, pack),
                known if KINDS.contains(&known) => {}
                other => self.problems.extend(Problems::of_file(
                    &kind.label,
                    format!(
                        "`{other}` isn't a kind of item this OpenDrone knows, so it's skipped; a Pack holds {}",
                        KINDS
                            .iter()
                            .map(|k| format!("{k}/"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )),
            }
        }
    }

    /// Reads every Input Device profile in a Pack's `input-devices/`, each
    /// written as `<id>.toml`.
    fn open_input_devices(&mut self, folder: &Folder, pack: &str) {
        let names = match folder.names() {
            Ok(names) => names,
            Err(found) => return self.problems.extend(found),
        };
        for name in names {
            let file = folder.child(&name);
            let stem = name.strip_suffix(".toml").filter(|_| file.path.is_file());
            let Some(stem) = stem else {
                self.problems.extend(Problems::of_file(
                    &file.label,
                    "only Input Device profiles, each written as <id>.toml, live in input-devices/",
                ));
                continue;
            };
            if !is_an_id(stem) {
                self.problems.extend(Problems::of_file(
                    &file.label,
                    format!(
                        "the file's name is the profile's id, so \"{stem}\" must be lowercase words joined by dashes, such as \"radiomaster-pocket\""
                    ),
                ));
                continue;
            }
            let id = format!("{pack}/{stem}");
            let checked = file
                .read()
                .and_then(|text| read_input_device_file(&id, &file.label, &text));
            if let Err(found) = &checked {
                self.problems.extend(found.clone());
            }
            self.input_devices.insert(id, checked);
        }
    }

    fn open_quads(&mut self, quads: &Folder, pack: &str) {
        let names = match quads.names() {
            Ok(names) => names,
            Err(found) => return self.problems.extend(found),
        };
        for name in names {
            let folder = quads.child(&name);
            if !folder.path.is_dir() {
                self.problems.extend(Problems::of_file(
                    &folder.label,
                    "every Quad is a folder in quads/, holding its quad.toml and tune.txt",
                ));
                continue;
            }
            if !is_an_id(&name) {
                self.problems.extend(Problems::of_file(
                    &folder.label,
                    format!(
                        "the folder's name is the Quad's id, so \"{name}\" must be lowercase words joined by dashes, such as \"whoop-65\""
                    ),
                ));
                continue;
            }
            let id = format!("{pack}/{name}");
            let item = read_quad_folder(&folder, &id);
            if let Err(found) = &item.checked {
                self.problems.extend(found.clone());
            }
            self.quads.insert(id, item);
        }
    }

    /// Reads and checks every Test Quad in `folder`, such as
    /// `scenarios/test-quads`. Its problems join [`Packs::problems`].
    pub fn with_test_quads(mut self, folder: &Path, label: &str) -> Packs {
        let folder = Folder {
            path: folder.to_path_buf(),
            label: label.to_string(),
        };
        let mut test_quads = TestQuads {
            folder: folder.clone(),
            quads: BTreeMap::new(),
            problems: Problems::new(),
        };
        let names = if folder.path.is_dir() {
            folder.names().unwrap_or_else(|found| {
                test_quads.problems.extend(found);
                Vec::new()
            })
        } else {
            Vec::new()
        };
        for name in names {
            let file = folder.child(&name);
            let Some(stem) = name.strip_suffix(".toml") else {
                test_quads.problems.extend(Problems::of_file(
                    &file.label,
                    format!(
                        "only Test Quads, each written as <id>.toml, live in {}",
                        folder.label
                    ),
                ));
                continue;
            };
            let id = format!("test/{stem}");
            let checked = if is_an_id(stem) {
                self.read_test_quad(&file, &id)
            } else {
                Err(Problems::of_file(
                    &file.label,
                    format!(
                        "the file's name is the Test Quad's id, so \"{stem}\" must be lowercase words joined by dashes, such as \"whoop-65-no-drag\""
                    ),
                ))
            };
            if let Err(found) = &checked {
                test_quads.problems.extend(found.clone());
            }
            test_quads.quads.insert(id, checked);
        }
        self.test_quads = Some(test_quads);
        self
    }

    fn read_test_quad(&self, file: &Folder, id: &str) -> Result<QuadDefinition, Problems> {
        let text = file.read()?;
        let mut base_tune = None;
        let (based_on, quad) = read_test_quad(&file.label, &text, |based_on| {
            let (quad, tune) = self.quad_files(based_on)?;
            base_tune = Some(tune.clone());
            Ok(quad.clone())
        })?;
        let tune = base_tune.expect("a Test Quad that read has its real Quad's Tune");
        check_quad(id, Some(based_on), &quad, &tune)
    }

    /// Every problem in every Pack and Test Quad, in folder order, each once:
    /// a Test Quad built on a refused Quad repeats that Quad's problems, which
    /// are listed with it already.
    pub fn problems(&self) -> Problems {
        let mut all = Problems::new();
        let test_quad_problems = self.test_quads.iter().flat_map(|t| t.problems.0.iter());
        for problem in self.problems.0.iter().chain(test_quad_problems) {
            if !all.0.contains(problem) {
                all.push(problem.clone());
            }
        }
        all
    }

    /// The manifests of every Pack that opened, in folder order.
    pub fn manifests(&self) -> Vec<&Manifest> {
        self.packs.iter().map(|pack| &pack.manifest).collect()
    }

    /// Every Quad that passed the checker, in id order.
    pub fn quads(&self) -> Vec<&QuadDefinition> {
        self.quads
            .values()
            .filter_map(|item| item.checked.as_ref().ok())
            .collect()
    }

    /// Every Input Device profile that passed the checker, in id order.
    pub fn input_devices(&self) -> Vec<&InputDeviceProfile> {
        self.input_devices
            .values()
            .filter_map(|checked| checked.as_ref().ok())
            .collect()
    }

    /// The checked Input Device profile with this id, such as
    /// `opendrone/dualsense`, or the problems that kept it out.
    pub fn input_device(&self, id: &str) -> Result<InputDeviceProfile, Problems> {
        let (pack, name) = self.find(id)?;
        match self.input_devices.get(id) {
            Some(checked) => checked.clone(),
            None => Err(Problems::of_file(
                id,
                format!(
                    "there's no Input Device profile here: {}/input-devices/{name}.toml doesn't exist",
                    pack.folder.label
                ),
            )),
        }
    }

    /// Every Test Quad that passed the checker, in id order.
    pub fn test_quads(&self) -> Vec<&QuadDefinition> {
        self.test_quads
            .iter()
            .flat_map(|t| t.quads.values())
            .filter_map(|q| q.as_ref().ok())
            .collect()
    }

    /// The checked Quad definition with this id, such as `opendrone/whoop-65`,
    /// or a Test Quad such as `test/whoop-65-no-drag`; or the problems that
    /// kept it out.
    pub fn quad(&self, id: &str) -> Result<QuadDefinition, Problems> {
        if let Some(name) = id.strip_prefix("test/") {
            let test_quads = self.test_quads.as_ref().ok_or_else(|| {
                Problems::of_file(id, "Test Quads are read only by the Scenario runner")
            })?;
            if !is_an_id(name) {
                return Err(Problems::of_file(id, not_an_id(id)));
            }
            return match test_quads.quads.get(id) {
                Some(checked) => checked.clone(),
                None => Err(Problems::of_file(
                    id,
                    format!(
                        "there's no Test Quad here: {}/{name}.toml doesn't exist",
                        test_quads.folder.label
                    ),
                )),
            };
        }
        self.quad_item(id).and_then(|item| item.checked.clone())
    }

    /// The files of the real Quad with this id, for a Test Quad to build on,
    /// or the problems that kept it out.
    fn quad_files(&self, id: &str) -> Result<&(QuadFile, Tune), Problems> {
        let item = self.quad_item(id)?;
        match (&item.checked, &item.files) {
            (Ok(_), Some(files)) => Ok(files),
            (Err(found), _) => Err(found.clone()),
            (Ok(_), None) => Err(Problems::of_file(id, "its files weren't kept")),
        }
    }

    fn quad_item(&self, id: &str) -> Result<&QuadItem, Problems> {
        let (pack, name) = self.find(id)?;
        self.quads.get(id).ok_or_else(|| {
            Problems::of_file(
                id,
                format!(
                    "there's no Quad here: {}/quads/{name}/quad.toml doesn't exist",
                    pack.folder.label
                ),
            )
        })
    }

    /// The open Pack an item id names, and the item's name.
    fn find<'a>(&self, id: &'a str) -> Result<(&OpenPack, &'a str), Problems> {
        let Some((pack, name)) = id.split_once('/') else {
            return Err(Problems::of_file(id, not_an_id(id)));
        };
        if !is_an_id(pack) || !is_an_id(name) {
            return Err(Problems::of_file(id, not_an_id(id)));
        }
        match self.packs.iter().find(|open| open.manifest.id == pack) {
            Some(open) => Ok((open, name)),
            None => {
                let mut sentence = format!("there's no Pack with the id \"{pack}\"");
                if !self.skipped.is_empty() {
                    sentence.push_str(&format!(
                        "; these Packs were skipped because of their problems: {}",
                        self.skipped.join(", ")
                    ));
                }
                Err(Problems::of_file(id, sentence))
            }
        }
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
}

/// Reads one Quad's folder. Every file is read as far as it goes, so all of
/// the Quad's problems are listed at once: those with each file's shape, and
/// those with every value that did read.
fn read_quad_folder(folder: &Folder, id: &str) -> QuadItem {
    let mut problems = Problems::new();
    let required = |name: &str, problems: &mut Problems| {
        let file = folder.child(name);
        if file.path.is_file() {
            Some(file)
        } else {
            problems.extend(Problems::of_file(
                &folder.label,
                format!("every Quad's folder holds a {name}, but this one has none"),
            ));
            None
        }
    };
    let mut read_in_full = ReadInFull {
        quad: true,
        tune: true,
    };
    let quad = required("quad.toml", &mut problems).and_then(|file| {
        let read = file
            .read()
            .and_then(|text| read_quad_file_as_far_as_it_goes(&file.label, &text));
        match read {
            Ok((quad, found)) => {
                read_in_full.quad = found.is_empty();
                problems.extend(found);
                Some(quad)
            }
            Err(found) => {
                problems.extend(found);
                None
            }
        }
    });
    let tune = required("tune.txt", &mut problems).and_then(|file| {
        let text = file.read().map_err(|found| problems.extend(found)).ok()?;
        let (tune, found) = read_tune_as_far_as_it_goes(&file.label, &text);
        read_in_full.tune = found.is_empty();
        problems.extend(found);
        Some(tune)
    });
    let log = folder.child("feel-tests.md");
    if log.path.is_file()
        && let Err(found) = log
            .read()
            .and_then(|text| feel_tests::read_feel_tests(&log.label, &text))
    {
        problems.extend(found);
    }
    if let Some(quad) = &quad {
        problems.extend(check_picture(folder, quad));
    }
    let files = quad.zip(tune);
    let checked = match &files {
        Some((quad, tune)) => match check_quad_as_read(id, None, quad, tune, read_in_full) {
            Ok(definition) if problems.is_empty() => Ok(definition),
            Ok(_) => Err(problems),
            Err(found) => {
                problems.extend(found);
                Err(problems)
            }
        },
        None => Err(problems),
    };
    QuadItem { checked, files }
}

/// The Quad picker's picture must be a PNG file in the Quad's own folder.
fn check_picture(folder: &Folder, quad: &QuadFile) -> Problems {
    let name = &quad.picture;
    if name.is_empty() {
        return Problems::new();
    }
    if name.contains(['/', '\\']) || name.starts_with('.') || !name.ends_with(".png") {
        return Problems::of_file(
            &quad.file,
            format!(
                "`picture` names a PNG file in the Quad's own folder, such as \"picture.png\", not \"{name}\""
            ),
        );
    }
    let file = folder.child(name);
    match fs::read(&file.path) {
        Ok(bytes) if bytes.starts_with(PNG_SIGNATURE) => Problems::new(),
        Ok(_) => Problems::of_file(&file.label, "isn't a PNG picture"),
        Err(_) => Problems::of_file(
            &file.label,
            "doesn't exist, but the Quad definition's `picture` names it for the Quad picker",
        ),
    }
}

fn not_an_id(id: &str) -> String {
    format!(
        "\"{id}\" isn't an id: write the Pack's id, a slash and the item's folder name, such as \"opendrone/whoop-65\""
    )
}
