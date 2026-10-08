//! Maps: so far their world values and solid parts, and the Test Maps built
//! into the code for Scenarios (#16 §7). Reading a Map's `.glb` (its collider
//! tags and Launch Spot) arrives with the Map ticket (#63); its colliders
//! become the same plain [`MapShape`]s the Test Maps are written as.

use opendrone_maths::{Fingerprint, Fingerprinter};
use opendrone_physics::{MapShape, World};

use crate::document::{Document, FORMAT, Problems};
use crate::migration::{self, FileKind, PACK_STEPS, Step};
use crate::test_maps::TEST_MAPS;
use crate::units::{self, Dimension};

/// A checked Map: what the Simulation receives from it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapDefinition {
    /// Such as `test/empty-air`.
    pub id: String,
    /// The on-screen name.
    pub name: String,
    pub world: World,
    /// Its solid parts, in a fixed order: contacts and the line question
    /// name each by its place in this list.
    pub shapes: Vec<MapShape>,
}

impl MapDefinition {
    /// The fingerprint of what the Simulation receives from this Map: its
    /// world values, then its solid parts in order (#16 §9). Each shape
    /// starts with its kind and spells out its own lengths, so a run of them
    /// reads back one way only, and a Map with none, such as
    /// `test/empty-air`, keeps the fingerprint its world values alone give.
    pub fn fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        self.world.write_fingerprint(&mut f);
        for shape in &self.shapes {
            shape.write_fingerprint(&mut f);
        }
        f.finish()
    }
}

/// Where the Test Maps' `map.toml` files live, from the repo's root: one
/// `<name>.toml` each. They are built into the code, and `cargo xtask
/// migrate` rewrites them like any Pack file, so their `format` keeps up.
pub const TEST_MAPS_FOLDER: &str = "crates/pack/test-maps";

/// A Test Map by the part of its id after `test/`, such as `empty-air`: its
/// `map.toml` (in [`TEST_MAPS_FOLDER`]) read like any Map's, with its solid
/// parts, which are written in the code (`test_maps.rs`) because a Test Map
/// has no `.glb`.
pub fn test_map(name: &str) -> Option<Result<MapDefinition, Problems>> {
    TEST_MAPS.iter().find(|map| map.name == name).map(|map| {
        read_map_file(
            &format!("test/{}", map.name),
            &format!("the built-in Test Map test/{}", map.name),
            map.text,
        )
        .map(|definition| MapDefinition {
            shapes: (map.shapes)(),
            ..definition
        })
    })
}

/// The ids of every Test Map.
pub fn test_map_ids() -> Vec<String> {
    TEST_MAPS
        .iter()
        .map(|map| format!("test/{}", map.name))
        .collect()
}

/// Reads a `map.toml`: its name and its world values. An older file is
/// upgraded in memory first, like every Pack file. Its solid parts come from
/// elsewhere (a Test Map's code, later a Map's `.glb`), so they are empty
/// here.
pub fn read_map_file(id: &str, file: &str, text: &str) -> Result<MapDefinition, Problems> {
    read_map_file_with_steps(id, file, text, PACK_STEPS)
}

/// [`read_map_file`], upgrading an older file with `steps` instead of
/// [`PACK_STEPS`]: the steps must lead to the newest format, [`FORMAT`].
/// The readable checks give it a synthetic step.
pub fn read_map_file_with_steps(
    id: &str,
    file: &str,
    text: &str,
    steps: &[Step],
) -> Result<MapDefinition, Problems> {
    let text = migration::upgraded_with(file, text, FileKind::Map, FORMAT, steps)?;
    let doc = Document::parse(file, &text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    if doc.is_newer() {
        return Err(problems);
    }
    let root = doc.root();
    let name = root
        .text("name", &mut problems)
        .map(|(name, _)| name.to_string());
    root.text("description", &mut problems);
    let world = root.table("world", &mut problems);
    let mut value = |key: &str, dimension: Dimension| -> Option<f64> {
        let world = world.as_ref()?;
        let (text, item) = world.text(key, &mut problems)?;
        match units::parse_quantity(text).and_then(|q| q.as_a(dimension)) {
            Ok(value) => Some(value),
            Err(p) => {
                problems.push(item.problem(p.0));
                None
            }
        }
    };
    let gravity = value("gravity", Dimension::ACCELERATION);
    let air_density = value("air_density", Dimension::DENSITY);
    if let Some(world) = &world {
        world.refuse_unknown(&["gravity", "air_density"], &mut problems);
    }
    let (Some(name), Some(gravity), Some(air_density)) = (name, gravity, air_density) else {
        return Err(problems);
    };
    problems.or(MapDefinition {
        id: id.to_string(),
        name,
        world: World {
            gravity,
            air_density,
        },
        shapes: Vec::new(),
    })
}
