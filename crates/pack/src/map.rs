//! Maps: their `map.toml` (on-screen text, world values, the files it names),
//! their solid parts and Launch Spot, and the Test Maps built into the code
//! for Scenarios (#16 §7). A Map's solid parts come from its `.glb`
//! ([`crate::glb`]) as the same plain [`MapShape`]s the Test Maps are written
//! as.

use std::path::PathBuf;

use opendrone_maths::{Fingerprint, Fingerprinter};
use opendrone_physics::{MapShape, World};

use crate::document::{Document, FORMAT, Item, Problems};
use crate::glb::{DrawnPart, LaunchSpot};
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
    /// The one-line description for the Hub and the Map picker.
    pub description: String,
    pub world: World,
    /// Its solid parts, in a fixed order: contacts and the line question
    /// name each by its place in this list.
    pub shapes: Vec<MapShape>,
    /// The name of each solid part's object in the `.glb`, in the same order
    /// as `shapes`. Empty on a Test Map, whose parts have no objects.
    pub solid_names: Vec<String>,
    /// Where a Quad starts, landed, and where the pilot stands. A Test Map
    /// has none: its Quads start where the Scenario puts them.
    pub launch_spot: Option<LaunchSpot>,
    /// What the game draws: every part of the `.glb` that isn't collision
    /// only. Empty on a Test Map.
    pub drawing: Vec<DrawnPart>,
    /// The `.glb` and the preview picture `map.toml` names, in the Map's own
    /// folder. Not on a Test Map.
    pub model: Option<String>,
    pub preview: Option<String>,
    /// The Map's Background Sound, if `map.toml` has the slot filled.
    pub background_sound: Option<BackgroundSound>,
    /// The Map's baked light, if `map.toml` has a `[light]` table: where its
    /// sun is and the colours of sun and sky.
    pub light: Option<MapLight>,
    /// Where the Map's light atlas is (`<id>.light.exr` in its folder), if it
    /// has one. Not on a Test Map.
    pub light_atlas: Option<PathBuf>,
}

/// A Map's `[light]`: what its baked light was made from, and what the game
/// lights a Quad with. The atlas itself is a file in the Map's folder.
#[derive(Clone, Debug, PartialEq)]
pub struct MapLight {
    /// How high the sun is above the horizon, in radians: 0 on the horizon,
    /// a quarter turn overhead.
    pub sun_elevation: f64,
    /// The compass direction the sun is in, in radians clockwise from north:
    /// a quarter turn is east.
    pub sun_azimuth: f64,
    /// The sun's colour, sRGB as written in `#rrggbb`.
    pub sun_colour: [u8; 3],
    /// How bright the sun lights a white surface square-on to it; 1 is as
    /// bright as `sky_strength` 1 lights a white surface under open sky.
    pub sun_strength: f64,
    /// The sky's colour, sRGB as written in `#rrggbb`.
    pub sky_colour: [u8; 3],
    /// How bright an open sky lights a white surface facing up.
    pub sky_strength: f64,
}

/// A Map's Background Sound: an Ogg loop in the Map's folder, and how loud it
/// plays.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundSound {
    /// The file's name, in the Map's own folder.
    pub file: String,
    /// How loud it plays, in decibels: 0 as recorded.
    pub level_db: f64,
}

impl MapDefinition {
    /// The fingerprint of what the Simulation receives from this Map: its
    /// world values, then its solid parts in order, then its Launch Spot, if
    /// it has one (#16 §9). Each shape starts with its kind and spells out
    /// its own lengths, so a run of them reads back one way only, and a Map
    /// with none, such as `test/empty-air`, keeps the fingerprint its world
    /// values alone give; so does a Map with no Launch Spot, so the Test Maps
    /// keep theirs. What is only drawn, and the names of the parts, don't
    /// count.
    pub fn fingerprint(&self) -> Fingerprint {
        let mut f = Fingerprinter::new();
        self.world.write_fingerprint(&mut f);
        for shape in &self.shapes {
            shape.write_fingerprint(&mut f);
        }
        if let Some(spot) = &self.launch_spot {
            let at = spot.position;
            f.write_f64s(&[at.x, at.y, at.z, spot.heading]);
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
    root.refuse_unknown(
        &[
            "format",
            "name",
            "description",
            "world",
            "model",
            "preview",
            "background_sound",
            "light",
        ],
        &mut problems,
    );
    let name = root
        .text("name", &mut problems)
        .map(|(name, _)| name.to_string());
    let description = root
        .text("description", &mut problems)
        .map(|(description, _)| description.to_string());
    let model = root
        .get("model")
        .and_then(|item| item.text(&mut problems).map(str::to_string));
    let preview = root
        .get("preview")
        .and_then(|item| item.text(&mut problems).map(str::to_string));
    let background_sound = root.get("background_sound").and_then(|item| {
        let table = item.table(&mut problems)?;
        table.refuse_unknown(&["file", "level"], &mut problems);
        let file = table
            .text("file", &mut problems)
            .map(|(file, _)| file.to_string());
        let level = table.text("level", &mut problems).and_then(|(text, item)| {
            match units::parse_quantity(text).and_then(|q| q.as_a(Dimension::DECIBELS)) {
                Ok(db) if (-60.0..=12.0).contains(&db) => Some(db),
                Ok(_) => {
                    problems.push(item.problem(
                        "`background_sound.level` must be from -60 dB to 12 dB: 0 dB plays the file as recorded",
                    ));
                    None
                }
                Err(p) => {
                    problems.push(item.problem(p.0));
                    None
                }
            }
        });
        Some(BackgroundSound {
            file: file?,
            level_db: level?,
        })
    });
    let light = root
        .get("light")
        .and_then(|item| read_light(&item, &mut problems));
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
    let (Some(name), Some(description), Some(gravity), Some(air_density)) =
        (name, description, gravity, air_density)
    else {
        return Err(problems);
    };
    problems.or(MapDefinition {
        id: id.to_string(),
        name,
        description,
        world: World {
            gravity,
            air_density,
        },
        shapes: Vec::new(),
        solid_names: Vec::new(),
        launch_spot: None,
        drawing: Vec::new(),
        model,
        preview,
        background_sound,
        light,
        light_atlas: None,
    })
}

/// Reads `[light]`: every value is needed, and each is checked in plain words.
fn read_light(item: &Item<'_, '_>, problems: &mut Problems) -> Option<MapLight> {
    let table = item.table(problems)?;
    table.refuse_unknown(
        &[
            "sun_elevation",
            "sun_azimuth",
            "sun_colour",
            "sun_strength",
            "sky_colour",
            "sky_strength",
        ],
        problems,
    );
    let mut angle = |key: &str, to: f64, range: &str| -> Option<f64> {
        let (text, item) = table.text(key, problems)?;
        match units::parse_quantity(text).and_then(|q| q.as_a(Dimension::ANGLE)) {
            Ok(radians) if (0.0..=to).contains(&radians) => Some(radians),
            Ok(_) => {
                problems.push(item.problem(format!("`light.{key}` must be {range}")));
                None
            }
            Err(p) => {
                problems.push(item.problem(p.0));
                None
            }
        }
    };
    let sun_elevation = angle(
        "sun_elevation",
        std::f64::consts::FRAC_PI_2,
        "from 0 ° (on the horizon) to 90 ° (overhead)",
    );
    let sun_azimuth = angle(
        "sun_azimuth",
        std::f64::consts::TAU,
        "a compass direction from 0 ° (north) to 360 °, such as \"225 °\" for south-west",
    );
    let mut colour = |key: &str| -> Option<[u8; 3]> {
        let (text, item) = table.text(key, problems)?;
        let digits = text
            .strip_prefix('#')
            .filter(|d| d.len() == 6 && d.is_ascii());
        let byte = |i: usize| digits.and_then(|d| u8::from_str_radix(&d[i..i + 2], 16).ok());
        match (byte(0), byte(2), byte(4)) {
            (Some(r), Some(g), Some(b)) => Some([r, g, b]),
            _ => {
                problems.push(item.problem(format!(
                    "`light.{key}` is a colour written like \"#fff2d6\" (sRGB, as in a paint program), not \"{text}\""
                )));
                None
            }
        }
    };
    let sun_colour = colour("sun_colour");
    let sky_colour = colour("sky_colour");
    let mut strength = |key: &str| -> Option<f64> {
        let (text, item) = table.text(key, problems)?;
        match units::parse_quantity(text).and_then(|q| q.as_a(Dimension::NONE)) {
            Ok(value) if (0.0..=1000.0).contains(&value) => Some(value),
            Ok(_) => {
                problems.push(item.problem(format!(
                    "`light.{key}` must be 0 or more, and at most 1000: 1 lights a white surface as an open sky of strength 1 does"
                )));
                None
            }
            Err(p) => {
                problems.push(item.problem(p.0));
                None
            }
        }
    };
    let sun_strength = strength("sun_strength");
    let sky_strength = strength("sky_strength");
    Some(MapLight {
        sun_elevation: sun_elevation?,
        sun_azimuth: sun_azimuth?,
        sun_colour: sun_colour?,
        sun_strength: sun_strength?,
        sky_colour: sky_colour?,
        sky_strength: sky_strength?,
    })
}
