//! Maps: so far their world values, and the Test Maps built into the code
//! for Scenarios (#16 §7). A Map's `.glb` shapes and Launch Spot arrive with
//! the Map tickets (#43, #63).

use opendrone_maths::{Fingerprint, Fingerprinter};
use opendrone_physics::World;

use crate::document::{Document, Problems};
use crate::units::{self, Dimension};

/// A checked Map: what the Simulation receives from it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapDefinition {
    /// Such as `test/empty-air`.
    pub id: String,
    /// The on-screen name.
    pub name: String,
    pub world: World,
}

impl MapDefinition {
    /// The fingerprint of what the Simulation receives from this Map: so far
    /// its world values (#16 §9).
    pub fn fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        self.world.write_fingerprint(&mut f);
        f.finish()
    }
}

/// The Test Maps, built into the code for Scenarios only, each written as the
/// text of a `map.toml`. They use the `test/` prefix (#16 §2).
const TEST_MAPS: &[(&str, &str)] = &[(
    "empty-air",
    r#"
format      = 1
name        = "Empty air"
description = "Nothing but air: no floor, no walls and no wind. For Physics Scenarios."

[world]
gravity     = "9.81 m/s²"     # standard gravity, as the alpha Maps write it
air_density = "1.225 kg/m³"   # sea level
"#,
)];

/// A Test Map by the part of its id after `test/`, such as `empty-air`.
pub fn test_map(name: &str) -> Option<Result<MapDefinition, Problems>> {
    TEST_MAPS
        .iter()
        .find(|(map, _)| *map == name)
        .map(|(map, text)| {
            read_map_file(
                &format!("test/{map}"),
                &format!("the built-in Test Map test/{map}"),
                text,
            )
        })
}

/// The ids of every Test Map.
pub fn test_map_ids() -> Vec<String> {
    TEST_MAPS
        .iter()
        .map(|(map, _)| format!("test/{map}"))
        .collect()
}

/// Reads a `map.toml`: its name and its world values.
pub fn read_map_file(id: &str, file: &str, text: &str) -> Result<MapDefinition, Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
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
    })
}
