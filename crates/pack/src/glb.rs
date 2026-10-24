//! Reading a Map's `.glb`: its solid parts, its Launch Spot and what to draw
//! (#16 §7, #63).
//!
//! A `.glb` is binary glTF, one file: a JSON text that says what is in it and
//! a block of numbers. This reads only what a Map needs, in plain Rust, with
//! no graphics library, so the Simulation and the game see the very same
//! file. What it holds:
//!
//! - **Objects** are the nodes the file starts from, and their children. Each one that has a mesh carries an
//!   `od_collider` tag in its `extras` (what Blender writes custom properties
//!   as): `box`, `convex`, `trimesh` or `none`. `od_collision_only` marks a
//!   solid part that is never drawn. One node, an empty with no mesh, carries
//!   `od_launch_spot`: the compass heading the Quad faces (0 north, 90 east).
//!   Any other tag starting with `od_` is refused, so a misspelt one never
//!   passes for nothing.
//! - **Decals and blends (#321).** `od_decal` marks a drawn-only object as a
//!   decal, a picture lying on a surface, with its layer (0 to 15) as the
//!   value: its picture coordinates run from 0 to 1 across the picture.
//!   `od_blend` names a second material that shows through the object's own
//!   where its vertex colours (`COLOR_0`, grey, 0 to 1) say so.
//! - **Scatter (#338).** `od_scatter` marks a drawn ground as growing a kind
//!   of plant (`grass`), with `od_scatter_density` (clumps a square metre)
//!   and `od_scatter_height` (the shortest and tallest clump, in metres). The
//!   game grows the clumps; nothing here places them.
//! - **Axes:** glTF is Y-up (x east, y up, z south); the Simulation counts
//!   east, north and up. A corner `(x, y, z)` in the file is `(x, -z, y)` in
//!   the Map.
//! - **No compressed meshes.** Draco, meshopt, quantised numbers and GPU
//!   instancing are refused, because Bevy refuses them and nothing here reads
//!   them.
//!
//! Every problem names the `.glb` and the object it is about, and all of them
//! are listed at once.

use std::collections::BTreeMap;

use opendrone_maths::functions::atan2;
use opendrone_maths::{Attitude, Vec3};
use opendrone_physics::{MapCollision, MapShape, MapShapeProblem};
use serde_json::Value;

use crate::document::{Problem, Problems};

/// The tags a Map's objects may carry.
pub const TAGS: [&str; 8] = [
    "od_collider",
    "od_collision_only",
    "od_launch_spot",
    "od_decal",
    "od_blend",
    "od_scatter",
    "od_scatter_density",
    "od_scatter_height",
];

/// The kinds of plant a ground can grow (`od_scatter`), by the word the tag
/// carries.
pub const SCATTER_KINDS: [(&str, ScatterKind); 1] = [("grass", ScatterKind::Grass)];

/// The fewest and most clumps a square metre a ground may grow
/// (`od_scatter_density`).
pub const SCATTER_DENSITY: (f64, f64) = (0.1, 20.0);

/// The shortest and tallest a clump may be, in metres (`od_scatter_height`).
pub const SCATTER_HEIGHT: (f64, f64) = (0.02, 2.0);

/// The highest layer a decal may have: a decal of a higher layer is drawn
/// over one of a lower layer where they overlap.
pub const TOP_DECAL_LAYER: u8 = 15;

/// How far a decal's picture coordinate may stray outside 0 to 1, for the
/// rounding of a 32-bit number.
const DECAL_UV_SLACK: f32 = 1e-3;

/// Extensions that compress or pack a mesh, with what to say about each.
const COMPRESSED: [(&str, &str); 4] = [
    ("KHR_draco_mesh_compression", "Draco compression"),
    ("EXT_meshopt_compression", "meshopt compression"),
    ("KHR_mesh_quantization", "quantised (whole-number) corners"),
    ("EXT_mesh_gpu_instancing", "GPU instancing"),
];

/// Where a Quad starts on a Map, landed, and where the pilot stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchSpot {
    /// The ground under the Quad, in the Map's axes (east, north, up), in
    /// metres.
    pub position: Vec3,
    /// The compass heading the Quad faces, in degrees: 0 north, 90 east.
    pub heading: f64,
}

/// How a part of a Map is solid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collider {
    /// An exact box, found from the eight corners of its mesh.
    Box,
    /// A closed convex solid: its mesh, as it is.
    Convex,
    /// Triangles: a closed mesh is a solid, an open one a surface.
    Trimesh,
    /// Nothing: the part is drawn and nothing touches it.
    None,
}

/// One piece of a Map to draw: a mesh with its material's name, in the Map's
/// axes (east, north and up).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnPart {
    /// The object's name in the `.glb`.
    pub name: String,
    /// The material's name: the game looks it up in its material library.
    pub material: String,
    /// The material's flat colour (red, green, blue, alpha), for when the
    /// library has no picture for it.
    pub colour: [f32; 4],
    pub corners: Vec<[f32; 3]>,
    /// One picture coordinate per corner, if the mesh has them.
    pub uvs: Vec<[f32; 2]>,
    /// One coordinate per corner into the Map's light atlas (the second set,
    /// `TEXCOORD_1`), if the mesh has them: empty when it hasn't.
    pub light_uvs: Vec<[f32; 2]>,
    /// Corner numbers, anticlockwise seen from the front.
    pub triangles: Vec<[u32; 3]>,
    /// If the part is a decal (`od_decal`), its layer: 0 to
    /// [`TOP_DECAL_LAYER`], a higher one drawn over a lower one.
    pub decal: Option<u8>,
    /// If a second material shows through this one (`od_blend`), which, and
    /// how much of it at each corner.
    pub blend: Option<Blend>,
    /// If the part grows plants (`od_scatter`), which and how many.
    pub scatter: Option<Scatter>,
}

/// A kind of plant a ground grows (#338).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScatterKind {
    /// Short grass: clumps of thin blades that lean and sway.
    Grass,
}

impl ScatterKind {
    /// The word `od_scatter` carries for this kind.
    pub fn word(self) -> &'static str {
        match self {
            ScatterKind::Grass => "grass",
        }
    }
}

/// What a ground grows (`od_scatter`, `od_scatter_density` and
/// `od_scatter_height`): the game places the clumps on its upward-facing
/// triangles, from a fixed seed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scatter {
    pub kind: ScatterKind,
    /// Clumps a square metre of ground.
    pub density: f32,
    /// The shortest and the tallest clump, in metres.
    pub height: [f32; 2],
}

/// A second material that shows through a part's own (`od_blend` and the
/// mesh's vertex colours, `COLOR_0`).
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    /// The second material's name, from the material library.
    pub material: String,
    /// How much of it shows at each corner, from 0 (none: the part's own
    /// material) to 1 (all of it), one per corner.
    pub weights: Vec<f32>,
}

/// What a Map's `.glb` holds, checked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapModel {
    /// Each solid part in the order of the file, with its object's name.
    pub solids: Vec<(String, MapShape)>,
    pub launch_spot: Option<LaunchSpot>,
    pub drawing: Vec<DrawnPart>,
}

/// Reads and checks a `.glb`. `file` is how problems name it.
pub fn read_model(file: &str, bytes: &[u8]) -> Result<MapModel, Problems> {
    let mut problems = Problems::new();
    let model = Reader {
        file,
        problems: &mut problems,
    }
    .read(bytes);
    problems.or(model.unwrap_or_default())
}

struct Reader<'p> {
    file: &'p str,
    problems: &'p mut Problems,
}

impl Reader<'_> {
    fn problem(&mut self, sentence: impl Into<String>) {
        self.problems.push(Problem::of(self.file, 0, sentence));
    }

    fn read(mut self, bytes: &[u8]) -> Option<MapModel> {
        let (json, bin) = self.chunks(bytes)?;
        let doc: Value = match serde_json::from_slice(json) {
            Ok(doc) => doc,
            Err(error) => {
                self.problem(format!("its glTF text can't be read: {error}"));
                return None;
            }
        };
        self.refuse_compression(&doc);
        let gltf = Gltf { doc: &doc, bin };
        let mut model = MapModel::default();
        let mut launch_spots = Vec::new();
        let scene = doc.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
        let roots: Vec<usize> = doc
            .get("scenes")
            .and_then(|scenes| scenes.get(scene))
            .and_then(|scene| scene.get("nodes"))
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|n| n as usize)
                    .collect()
            })
            .unwrap_or_default();
        let mut seen = vec![false; gltf.nodes().len()];
        for root in roots {
            self.walk(
                &gltf,
                root,
                IDENTITY,
                &mut seen,
                &mut model,
                &mut launch_spots,
            );
        }
        match launch_spots.as_slice() {
            [] => self.problem(
                "the Map has no Launch Spot: exactly one object must carry the od_launch_spot tag, an empty at the spot with the compass heading the Quad faces as its value",
            ),
            [(_, spot)] => model.launch_spot = Some(*spot),
            many => {
                let names: Vec<String> = many.iter().map(|(name, _)| format!("\"{name}\"")).collect();
                self.problem(format!(
                    "the Map has {} Launch Spots ({}), and exactly one is allowed",
                    many.len(),
                    names.join(", ")
                ));
            }
        }
        Some(model)
    }

    /// The JSON text and the block of numbers.
    fn chunks<'b>(&mut self, bytes: &'b [u8]) -> Option<(&'b [u8], &'b [u8])> {
        const NOT_GLB: &str = "isn't a binary glTF file (.glb): export the Map as GLB";
        if bytes.len() < 20 || &bytes[..4] != b"glTF" {
            self.problem(NOT_GLB);
            return None;
        }
        let number = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        if number(4) != 2 {
            self.problem(format!(
                "is glTF version {}, and only version 2 is read",
                number(4)
            ));
            return None;
        }
        if number(8) as usize != bytes.len() {
            self.problem(
                "its length doesn't match its header: the file is cut short or has extra bytes",
            );
            return None;
        }
        let (mut json, mut bin) = (None, None);
        let mut at = 12;
        while at + 8 <= bytes.len() {
            let length = number(at) as usize;
            let kind = &bytes[at + 4..at + 8];
            let Some(body) = bytes.get(at + 8..at + 8 + length) else {
                self.problem("has a block that runs past the end of the file");
                return None;
            };
            match kind {
                b"JSON" if json.is_none() => json = Some(body),
                b"BIN\0" if bin.is_none() => bin = Some(body),
                _ => {}
            }
            at += 8 + length.div_ceil(4) * 4;
        }
        match json {
            Some(json) => Some((json, bin.unwrap_or(&[]))),
            None => {
                self.problem(NOT_GLB);
                None
            }
        }
    }

    fn refuse_compression(&mut self, doc: &Value) {
        for key in ["extensionsRequired", "extensionsUsed"] {
            let Some(list) = doc.get(key).and_then(Value::as_array) else {
                continue;
            };
            for name in list.iter().filter_map(Value::as_str) {
                if let Some((_, what)) = COMPRESSED.iter().find(|(extension, _)| *extension == name)
                {
                    self.problem(format!(
                        "uses {name} ({what}): no compressed meshes, because every program must read the very same corners; export the Map with mesh compression off"
                    ));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        gltf: &Gltf<'_>,
        index: usize,
        parent: Matrix,
        seen: &mut [bool],
        model: &mut MapModel,
        launch_spots: &mut Vec<(String, LaunchSpot)>,
    ) {
        let Some(node) = gltf.nodes().get(index) else {
            self.problem(format!(
                "the file starts from node {index}, which doesn't exist"
            ));
            return;
        };
        if std::mem::replace(&mut seen[index], true) {
            self.problem(format!(
                "node {index} is reached twice, so the nodes don't make a tree"
            ));
            return;
        }
        let name = node
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("node {index}"), str::to_string);
        let world = multiply(&parent, &local_matrix(node));
        let tags = self.tags(&name, node.get("extras"));
        let has_mesh = node.get("mesh").is_some();
        if let Some(heading) = tags.launch_heading {
            if has_mesh {
                self.problem(format!(
                    "object \"{name}\" is the Launch Spot but has a mesh: the Launch Spot is an empty, a point with no shape"
                ));
            }
            if tags.collider.is_some() {
                self.problem(format!(
                    "object \"{name}\" is the Launch Spot but also carries od_collider: the Launch Spot is nothing solid"
                ));
            }
            let at = to_map(Vec3::new(world[0][3], world[1][3], world[2][3]));
            launch_spots.push((
                name.clone(),
                LaunchSpot {
                    position: at,
                    heading,
                },
            ));
        } else if has_mesh {
            self.mesh_object(gltf, &name, node, &world, &tags, model);
        } else if tags.collider.is_some() || tags.collision_only {
            self.problem(format!(
                "object \"{name}\" carries a collider tag but has no mesh, so there is nothing to be solid"
            ));
        }
        let children: Vec<usize> = node
            .get("children")
            .and_then(Value::as_array)
            .map(|c| {
                c.iter()
                    .filter_map(Value::as_u64)
                    .map(|n| n as usize)
                    .collect()
            })
            .unwrap_or_default();
        for child in children {
            self.walk(gltf, child, world, seen, model, launch_spots);
        }
    }

    /// The `od_` tags in a node's extras.
    fn tags(&mut self, name: &str, extras: Option<&Value>) -> Tags {
        let mut tags = Tags::default();
        let Some(extras) = extras.and_then(Value::as_object) else {
            return tags;
        };
        for (key, value) in extras {
            if !key.starts_with("od_") {
                continue;
            }
            match key.as_str() {
                "od_collider" => match value.as_str() {
                    Some("box") => tags.collider = Some(Collider::Box),
                    Some("convex") => tags.collider = Some(Collider::Convex),
                    Some("trimesh") => tags.collider = Some(Collider::Trimesh),
                    Some("none") => tags.collider = Some(Collider::None),
                    _ => {
                        tags.collider_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_collider {value}, which isn't one of \"box\", \"convex\", \"trimesh\" or \"none\""
                        ));
                    }
                },
                "od_collision_only" => match value {
                    Value::Bool(on) => tags.collision_only = *on,
                    Value::Number(n) if n.as_f64().is_some_and(|n| n == 0.0 || n == 1.0) => {
                        tags.collision_only = n.as_f64() == Some(1.0);
                    }
                    _ => self.problem(format!(
                        "object \"{name}\" has od_collision_only {value}, which must be 1 (solid but never drawn) or 0"
                    )),
                },
                "od_decal" => match value.as_f64() {
                    Some(layer)
                        if layer.fract() == 0.0
                            && (0.0..=f64::from(TOP_DECAL_LAYER)).contains(&layer) =>
                    {
                        tags.decal = Some(layer as u8);
                    }
                    _ => {
                        tags.decal_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_decal {value}, which must be a whole number from 0 to {TOP_DECAL_LAYER}: the decal's layer, a higher one drawn over a lower one where they overlap"
                        ));
                    }
                },
                "od_blend" => match value.as_str() {
                    Some(material) if !material.trim().is_empty() => {
                        tags.blend = Some(material.to_string());
                    }
                    _ => {
                        tags.blend_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_blend {value}, which must be the name of a material: the one that shows through the object's own where its vertex colours say so"
                        ));
                    }
                },
                "od_scatter" => match value
                    .as_str()
                    .and_then(|word| SCATTER_KINDS.iter().find(|(w, _)| *w == word))
                {
                    Some((_, kind)) => tags.scatter = Some(*kind),
                    None => {
                        tags.scatter_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_scatter {value}, which isn't a kind of plant OpenDrone grows: the kinds are {}",
                            SCATTER_KINDS
                                .iter()
                                .map(|(w, _)| format!("\"{w}\""))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                },
                "od_scatter_density" => match value.as_f64() {
                    Some(d) if (SCATTER_DENSITY.0..=SCATTER_DENSITY.1).contains(&d) => {
                        tags.scatter_density = Some(d as f32);
                    }
                    _ => {
                        tags.scatter_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_scatter_density {value}, which must be a number from {} to {}: how many clumps grow on a square metre",
                            SCATTER_DENSITY.0, SCATTER_DENSITY.1
                        ));
                    }
                },
                "od_scatter_height" => match scatter_height(value) {
                    Some(height) => tags.scatter_height = Some(height),
                    None => {
                        tags.scatter_refused = true;
                        self.problem(format!(
                            "object \"{name}\" has od_scatter_height {value}, which must be two numbers in metres, the shortest clump then the tallest, each from {} to {}, the first no taller than the second",
                            SCATTER_HEIGHT.0, SCATTER_HEIGHT.1
                        ));
                    }
                },
                "od_launch_spot" => match value.as_f64() {
                    Some(heading) if (0.0..360.0).contains(&heading) => {
                        tags.launch_heading = Some(heading);
                    }
                    _ => self.problem(format!(
                        "object \"{name}\" has od_launch_spot {value}, which must be a number: the compass heading the Quad faces, from 0 up to 360 degrees (0 north, 90 east)"
                    )),
                },
                other => self.problem(format!(
                    "object \"{name}\" has the tag \"{other}\", which OpenDrone doesn't know: the tags that start with od_ are {}",
                    TAGS.join(", ")
                )),
            }
        }
        tags
    }

    fn mesh_object(
        &mut self,
        gltf: &Gltf<'_>,
        name: &str,
        node: &Value,
        world: &Matrix,
        tags: &Tags,
        model: &mut MapModel,
    ) {
        let Some(collider) = tags.collider else {
            if tags.collider_refused {
                // Already said so, with the value it has.
                return;
            }
            self.problem(format!(
                "object \"{name}\" has no od_collider tag: every object says whether it is solid, with \"box\", \"convex\", \"trimesh\" or \"none\" (none for a part nothing can touch)"
            ));
            return;
        };
        if tags.collision_only && collider == Collider::None {
            self.problem(format!(
                "object \"{name}\" is od_collision_only but its od_collider is \"none\": it would be neither drawn nor solid"
            ));
        }
        if tags.decal.is_some() {
            if collider != Collider::None {
                self.problem(format!(
                    "object \"{name}\" is a decal (od_decal) but its od_collider is \"{}\": a decal is a picture lying on a surface and is never solid, so its od_collider is \"none\"",
                    collider_word(collider)
                ));
            }
            if tags.collision_only {
                self.problem(format!(
                    "object \"{name}\" is a decal (od_decal) and od_collision_only: a decal is only drawn, never solid"
                ));
            }
            if tags.blend.is_some() {
                self.problem(format!(
                    "object \"{name}\" is a decal (od_decal) and has od_blend: a decal is one picture, with nothing to blend"
                ));
            }
        }
        let scatter = self.scatter(name, tags);
        let mesh = node.get("mesh").and_then(Value::as_u64).unwrap_or(0) as usize;
        let Some(primitives) = gltf.primitives(mesh) else {
            self.problem(format!(
                "object \"{name}\" names a mesh the file doesn't have"
            ));
            return;
        };
        let mut corners = Vec::new();
        let mut triangles = Vec::new();
        for primitive in primitives {
            match gltf.read_primitive(primitive, world) {
                Ok(read) => {
                    let offset = corners.len() as u32;
                    corners.extend(read.corners.iter().map(|c| to_map(*c)));
                    triangles.extend(
                        read.triangles
                            .iter()
                            .map(|t| [t[0] + offset, t[1] + offset, t[2] + offset]),
                    );
                    if let Err(why) = self.check_decal_and_blend(tags, &read) {
                        self.problem(format!("object \"{name}\" {why}"));
                        continue;
                    }
                    if !tags.collision_only {
                        let blend = tags.blend.as_ref().and_then(|material| {
                            Some(Blend {
                                material: material.clone(),
                                weights: read.weights.clone()?,
                            })
                        });
                        model.drawing.push(DrawnPart {
                            name: name.to_string(),
                            material: read.material,
                            colour: read.colour,
                            corners: read
                                .corners
                                .iter()
                                .map(|c| {
                                    let at = to_map(*c);
                                    [at.x as f32, at.y as f32, at.z as f32]
                                })
                                .collect(),
                            uvs: read.uvs,
                            light_uvs: read.light_uvs,
                            triangles: read.triangles,
                            decal: tags.decal,
                            blend,
                            scatter,
                        });
                    }
                }
                Err(why) => self.problem(format!("object \"{name}\" {why}")),
            }
        }
        if triangles.is_empty() {
            return;
        }
        let shape = match collider {
            Collider::None => return,
            Collider::Trimesh => {
                let (corners, triangles) = weld(&corners, &triangles);
                MapShape::TriangleMesh { corners, triangles }
            }
            Collider::Convex => {
                let (corners, triangles) = weld(&corners, &triangles);
                MapShape::Convex { corners, triangles }
            }
            Collider::Box => match fit_box(&corners) {
                Ok(shape) => shape,
                Err(why) => {
                    self.problem(format!(
                        "object \"{name}\" is tagged box, but its mesh isn't a box: {why}"
                    ));
                    return;
                }
            },
        };
        match MapCollision::new(std::slice::from_ref(&shape)) {
            Ok(_) => model.solids.push((name.to_string(), shape)),
            Err(error) => self.problem(format!(
                "object \"{name}\" is tagged {}, but {}",
                collider_word(collider),
                describe(&error.problem)
            )),
        }
    }
}

impl Reader<'_> {
    /// What a ground grows (#338), if its three scatter tags are there and
    /// agree with the rest of its tags; a problem for each that isn't.
    fn scatter(&mut self, name: &str, tags: &Tags) -> Option<Scatter> {
        let any = tags.scatter.is_some()
            || tags.scatter_density.is_some()
            || tags.scatter_height.is_some();
        if !any || tags.scatter_refused {
            return None;
        }
        let Some(kind) = tags.scatter else {
            self.problem(format!(
                "object \"{name}\" has od_scatter_density or od_scatter_height but no od_scatter: say which plant grows on it, such as od_scatter \"grass\""
            ));
            return None;
        };
        let (Some(density), Some(height)) = (tags.scatter_density, tags.scatter_height) else {
            self.problem(format!(
                "object \"{name}\" grows {} (od_scatter) but doesn't say how much and how tall: it needs od_scatter_density (clumps a square metre) and od_scatter_height (the shortest and tallest clump, in metres)",
                kind.word()
            ));
            return None;
        };
        if tags.collision_only {
            self.problem(format!(
                "object \"{name}\" grows {} (od_scatter) but is od_collision_only: plants grow on a ground that is drawn",
                kind.word()
            ));
            return None;
        }
        if tags.decal.is_some() {
            self.problem(format!(
                "object \"{name}\" is a decal (od_decal) and grows {} (od_scatter): plants grow on a ground, not on a picture lying on it",
                kind.word()
            ));
            return None;
        }
        Some(Scatter {
            kind,
            density,
            height,
        })
    }

    /// What a decal's picture coordinates and a blend's vertex colours must
    /// be (#321), or why a primitive's aren't.
    fn check_decal_and_blend(&self, tags: &Tags, read: &Primitive) -> Result<(), String> {
        if tags.decal.is_some() || tags.decal_refused {
            if read.uvs.len() != read.corners.len() {
                return Err(
                    "is a decal (od_decal), so its mesh needs picture coordinates (TEXCOORD_0), one for every corner, to say where its picture lies"
                        .to_string(),
                );
            }
            let range = -DECAL_UV_SLACK..=1.0 + DECAL_UV_SLACK;
            if let Some(uv) = read
                .uvs
                .iter()
                .find(|uv| !range.contains(&uv[0]) || !range.contains(&uv[1]))
            {
                return Err(format!(
                    "is a decal (od_decal), so its picture coordinates (TEXCOORD_0) run from 0 to 1 across its one picture, but one is ({}, {}): outside 0 to 1 the picture would repeat, and its edge show as a rectangle",
                    uv[0], uv[1]
                ));
            }
        }
        match (&read.weights, tags.blend.is_some() || tags.blend_refused) {
            (Some(_), false) => Err(
                "has vertex colours (COLOR_0) but no od_blend: on a Map a vertex colour says how much of a second material shows through the object's own, so name that material with od_blend, or export the mesh without vertex colours"
                    .to_string(),
            ),
            (None, true) => Err(
                "names a material to blend with (od_blend), but its mesh has no vertex colours (COLOR_0) to say where it shows: give it one grey vertex colour per corner, black for its own material and white for the other"
                    .to_string(),
            ),
            _ => Ok(()),
        }
    }
}

fn collider_word(collider: Collider) -> &'static str {
    match collider {
        Collider::Box => "box",
        Collider::Convex => "convex",
        Collider::Trimesh => "trimesh",
        Collider::None => "none",
    }
}

/// A plain sentence for why a mesh can't be a solid part.
fn describe(problem: &MapShapeProblem) -> &'static str {
    match problem {
        MapShapeProblem::NotARealNumber => "a number in it isn't a real number",
        MapShapeProblem::BoxHasNoSize => {
            "a box needs a size above zero along all three of its sides"
        }
        MapShapeProblem::ConvexHasNoVolume => {
            "a convex solid needs at least four triangles that enclose some volume"
        }
        MapShapeProblem::ConvexIsNotClosed => {
            "a convex solid must be closed: every edge shared by exactly two triangles, with every corner used"
        }
        MapShapeProblem::ConvexIsNotConvex => {
            "its mesh isn't convex, or its triangles don't face outward: tag it trimesh, or split it into convex parts"
        }
        MapShapeProblem::MeshHasNoTriangles => "it has no triangles",
        MapShapeProblem::MeshCornerMissing => "a triangle names a corner it doesn't have",
    }
}

#[derive(Default)]
struct Tags {
    collider: Option<Collider>,
    /// The `od_collider` tag is there, but its value is wrong, and a problem
    /// says so.
    collider_refused: bool,
    collision_only: bool,
    launch_heading: Option<f64>,
    /// `od_decal`'s layer, if the object is a decal.
    decal: Option<u8>,
    /// `od_decal` is there, but its value is wrong, and a problem says so.
    decal_refused: bool,
    /// `od_blend`'s material, if a second one shows through.
    blend: Option<String>,
    /// `od_blend` is there, but its value is wrong, and a problem says so.
    blend_refused: bool,
    /// `od_scatter`'s kind of plant, if the ground grows one.
    scatter: Option<ScatterKind>,
    /// `od_scatter_density`, clumps a square metre.
    scatter_density: Option<f32>,
    /// `od_scatter_height`, the shortest and tallest clump in metres.
    scatter_height: Option<[f32; 2]>,
    /// A scatter tag is there, but its value is wrong, and a problem says so.
    scatter_refused: bool,
}

/// `od_scatter_height`'s two heights, if it is two numbers in range with the
/// first no taller than the second.
fn scatter_height(value: &Value) -> Option<[f32; 2]> {
    let list = value.as_array()?;
    let [low, high] = list.as_slice() else {
        return None;
    };
    let (low, high) = (low.as_f64()?, high.as_f64()?);
    let range = SCATTER_HEIGHT.0..=SCATTER_HEIGHT.1;
    (range.contains(&low) && range.contains(&high) && low <= high)
        .then_some([low as f32, high as f32])
}

// ---------- glTF numbers ----------

/// A 4×4 matrix, row by row, acting on columns: `M * (x, y, z, 1)`.
type Matrix = [[f64; 4]; 4];

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    out
}

fn numbers(value: Option<&Value>, count: usize) -> Option<Vec<f64>> {
    let list = value?.as_array()?;
    (list.len() == count)
        .then(|| list.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .filter(|numbers| numbers.len() == count)
}

/// A node's own matrix: its `matrix`, or its translation, rotation and scale.
fn local_matrix(node: &Value) -> Matrix {
    if let Some(m) = numbers(node.get("matrix"), 16) {
        // glTF writes a matrix column by column.
        let mut out = IDENTITY;
        for (column, chunk) in m.chunks(4).enumerate() {
            for (row, value) in chunk.iter().enumerate() {
                out[row][column] = *value;
            }
        }
        return out;
    }
    let t = numbers(node.get("translation"), 3).unwrap_or_else(|| vec![0.0; 3]);
    let s = numbers(node.get("scale"), 3).unwrap_or_else(|| vec![1.0; 3]);
    let r = numbers(node.get("rotation"), 4).unwrap_or_else(|| vec![0.0, 0.0, 0.0, 1.0]);
    let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
    let length = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = if length > 0.0 {
        (x / length, y / length, z / length, w / length)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };
    let rotation = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let mut out = IDENTITY;
    for row in 0..3 {
        for column in 0..3 {
            out[row][column] = rotation[row][column] * s[column];
        }
        out[row][3] = t[row];
    }
    out
}

fn transform(m: &Matrix, p: [f32; 3]) -> Vec3 {
    let (x, y, z) = (f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
    Vec3::new(
        m[0][0] * x + m[0][1] * y + m[0][2] * z + m[0][3],
        m[1][0] * x + m[1][1] * y + m[1][2] * z + m[1][3],
        m[2][0] * x + m[2][1] * y + m[2][2] * z + m[2][3],
    )
}

/// glTF's axes (x east, y up, z south) as the Map's (east, north, up). The
/// `+ 0.0` turns a minus zero into a plain one, so two corners in one place
/// are one place to the bit.
fn to_map(v: Vec3) -> Vec3 {
    Vec3::new(v.x + 0.0, -v.z + 0.0, v.y + 0.0)
}

struct Gltf<'a> {
    doc: &'a Value,
    bin: &'a [u8],
}

struct Primitive {
    /// In glTF's own axes, already turned and moved by the node.
    corners: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    light_uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
    material: String,
    colour: [f32; 4],
    /// The vertex colours (`COLOR_0`) as blend weights, one per corner, if
    /// the mesh has them.
    weights: Option<Vec<f32>>,
}

impl Gltf<'_> {
    fn nodes(&self) -> &[Value] {
        self.doc
            .get("nodes")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    fn primitives(&self, mesh: usize) -> Option<&Vec<Value>> {
        self.doc
            .get("meshes")?
            .get(mesh)?
            .get("primitives")?
            .as_array()
    }

    /// The bytes of an accessor's elements, `count` of them, each `width`
    /// bytes, with the buffer view's stride, or why they can't be read.
    fn accessor_bytes(&self, index: usize, width: usize) -> Result<(&[u8], usize, usize), String> {
        let accessor = self
            .doc
            .get("accessors")
            .and_then(|a| a.get(index))
            .ok_or_else(|| format!("names accessor {index}, which the file doesn't have"))?;
        if accessor.get("sparse").is_some() {
            return Err("uses a sparse accessor, which isn't read".to_string());
        }
        let count = accessor.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let view_index = accessor
            .get("bufferView")
            .and_then(Value::as_u64)
            .ok_or_else(|| "has an accessor with no buffer view".to_string())?
            as usize;
        let view = self
            .doc
            .get("bufferViews")
            .and_then(|v| v.get(view_index))
            .ok_or_else(|| {
                format!("names buffer view {view_index}, which the file doesn't have")
            })?;
        if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
            return Err("reads from a second buffer, and only the file's own is read".to_string());
        }
        let stride = view
            .get("byteStride")
            .and_then(Value::as_u64)
            .map_or(width, |s| s as usize);
        let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + accessor
                .get("byteOffset")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
        let end = if count == 0 {
            start
        } else {
            start + (count - 1) * stride + width
        };
        let bytes = self.bin.get(start..end).ok_or_else(|| {
            "has an accessor that runs past the end of the file's numbers".to_string()
        })?;
        Ok((bytes, count, stride))
    }

    fn component(&self, index: usize) -> (u64, bool, String) {
        let accessor = &self.doc["accessors"][index];
        (
            accessor
                .get("componentType")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            accessor
                .get("normalized")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            accessor
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )
    }

    fn floats(&self, index: usize, parts: usize, what: &str) -> Result<Vec<Vec<f32>>, String> {
        let (component, normalized, kind) = self.component(index);
        let expected = if parts == 3 { "VEC3" } else { "VEC2" };
        if component != 5126 || normalized || kind != expected {
            return Err(format!(
                "has {what} that aren't plain 32-bit decimal numbers in groups of {parts}: whole-number (quantised) corners are a compressed mesh, which isn't read"
            ));
        }
        let (bytes, count, stride) = self.accessor_bytes(index, parts * 4)?;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let at = i * stride;
            out.push(
                (0..parts)
                    .map(|k| {
                        let o = at + k * 4;
                        f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
                    })
                    .collect(),
            );
        }
        Ok(out)
    }

    fn indices(&self, index: usize) -> Result<Vec<u32>, String> {
        let (component, _, kind) = self.component(index);
        let width = match component {
            5121 => 1,
            5123 => 2,
            5125 => 4,
            _ => return Err("has triangle numbers that aren't whole numbers".to_string()),
        };
        if kind != "SCALAR" {
            return Err("has triangle numbers that aren't a list of single numbers".to_string());
        }
        let (bytes, count, stride) = self.accessor_bytes(index, width)?;
        Ok((0..count)
            .map(|i| {
                let at = i * stride;
                match width {
                    1 => u32::from(bytes[at]),
                    2 => u32::from(u16::from_le_bytes([bytes[at], bytes[at + 1]])),
                    _ => {
                        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
                    }
                }
            })
            .collect())
    }

    fn read_primitive(&self, primitive: &Value, world: &Matrix) -> Result<Primitive, String> {
        if let Some(extensions) = primitive.get("extensions").and_then(Value::as_object) {
            for (name, what) in COMPRESSED {
                if extensions.contains_key(name) {
                    return Err(format!("uses {name} ({what}), which isn't read"));
                }
            }
        }
        let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
        if mode != 4 {
            return Err("has a mesh made of something other than triangles".to_string());
        }
        let attributes = primitive
            .get("attributes")
            .ok_or_else(|| "has a mesh with no corners".to_string())?;
        let position = attributes
            .get("POSITION")
            .and_then(Value::as_u64)
            .ok_or_else(|| "has a mesh with no corners".to_string())?
            as usize;
        let corners: Vec<[f32; 3]> = self
            .floats(position, 3, "corners")?
            .into_iter()
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        if corners.iter().flatten().any(|n| !n.is_finite()) {
            return Err("has a corner that isn't a real number".to_string());
        }
        let uvs = match attributes.get("TEXCOORD_0").and_then(Value::as_u64) {
            Some(index) => self
                .floats(index as usize, 2, "picture coordinates")?
                .into_iter()
                .map(|c| [c[0], c[1]])
                .collect(),
            None => Vec::new(),
        };
        let light_uvs: Vec<[f32; 2]> = match attributes.get("TEXCOORD_1").and_then(Value::as_u64) {
            Some(index) => self
                .floats(index as usize, 2, "light atlas coordinates")?
                .into_iter()
                .map(|c| [c[0], c[1]])
                .collect(),
            None => Vec::new(),
        };
        if light_uvs.iter().flatten().any(|n| !n.is_finite()) {
            return Err("has a light atlas coordinate that isn't a real number".to_string());
        }
        if !light_uvs.is_empty() && light_uvs.len() != corners.len() {
            return Err(
                "has light atlas coordinates (TEXCOORD_1) that aren't one for every corner"
                    .to_string(),
            );
        }
        let weights = match attributes.get("COLOR_0").and_then(Value::as_u64) {
            Some(index) => Some(self.blend_weights(index as usize, corners.len())?),
            None => None,
        };
        let listed = match primitive.get("indices").and_then(Value::as_u64) {
            Some(index) => self.indices(index as usize)?,
            None => (0..corners.len() as u32).collect(),
        };
        if listed.len() % 3 != 0 {
            return Err(
                "has a number of triangle corners that isn't a multiple of three".to_string(),
            );
        }
        let triangles: Vec<[u32; 3]> = listed.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
        if triangles
            .iter()
            .flatten()
            .any(|i| *i as usize >= corners.len())
        {
            return Err("has a triangle that names a corner it doesn't have".to_string());
        }
        let (material, colour) = primitive
            .get("material")
            .and_then(Value::as_u64)
            .and_then(|m| self.doc.get("materials")?.get(m as usize))
            .map_or_else(
                || (String::new(), [0.8, 0.8, 0.8, 1.0]),
                |m| {
                    let factor = numbers(
                        m.get("pbrMetallicRoughness")
                            .and_then(|p| p.get("baseColorFactor")),
                        4,
                    )
                    .map_or([1.0, 1.0, 1.0, 1.0], |f| {
                        [f[0] as f32, f[1] as f32, f[2] as f32, f[3] as f32]
                    });
                    (
                        m.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        factor,
                    )
                },
            );
        Ok(Primitive {
            corners: corners.iter().map(|c| transform(world, *c)).collect(),
            uvs,
            light_uvs,
            triangles,
            material,
            colour,
            weights,
        })
    }

    /// The vertex colours (`COLOR_0`) of a mesh as blend weights: the red of
    /// each, which must be grey (the three channels alike, within a 255th)
    /// and from 0 to 1, one for every corner. glTF lets them be 32-bit
    /// decimal numbers or whole numbers that stand for 0 to 1, in groups of
    /// three or four.
    fn blend_weights(&self, index: usize, corners: usize) -> Result<Vec<f32>, String> {
        let (component, normalized, kind) = self.component(index);
        let parts = match kind.as_str() {
            "VEC3" => 3,
            "VEC4" => 4,
            _ => {
                return Err(
                    "has vertex colours (COLOR_0) that aren't groups of three or four numbers"
                        .to_string(),
                );
            }
        };
        let (width, scale) = match (component, normalized) {
            (5126, false) => (4, 1.0),
            (5121, true) => (1, 255.0),
            (5123, true) => (2, 65535.0),
            _ => {
                return Err(
                    "has vertex colours (COLOR_0) that are neither 32-bit decimal numbers nor whole numbers standing for 0 to 1"
                        .to_string(),
                );
            }
        };
        let (bytes, count, stride) = self.accessor_bytes(index, parts * width)?;
        if count != corners {
            return Err(
                "has vertex colours (COLOR_0) that aren't one for every corner".to_string(),
            );
        }
        let mut weights = Vec::with_capacity(count);
        for i in 0..count {
            let channel = |k: usize| {
                let o = i * stride + k * width;
                match width {
                    4 => f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]),
                    2 => f32::from(u16::from_le_bytes([bytes[o], bytes[o + 1]])) / scale,
                    _ => f32::from(bytes[o]) / scale,
                }
            };
            let [r, g, b] = [channel(0), channel(1), channel(2)];
            if ![r, g, b]
                .iter()
                .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
            {
                return Err(format!(
                    "has a vertex colour (COLOR_0) of ({r}, {g}, {b}), and a blend weight is from 0 to 1"
                ));
            }
            if (g - r).abs() > 1.0 / 255.0 || (b - r).abs() > 1.0 / 255.0 {
                return Err(format!(
                    "has a vertex colour (COLOR_0) of ({r}, {g}, {b}), which isn't grey: a vertex colour on a Map is a blend weight, from black (the object's own material) to white (the od_blend one), not a tint"
                ));
            }
            weights.push(r);
        }
        Ok(weights)
    }
}

/// One corner for every place a mesh has one, whatever it is split into: the
/// file repeats a corner wherever its picture coordinates change. Corners
/// that sit at exactly the same place are one corner, in order of first
/// appearance.
fn weld(corners: &[Vec3], triangles: &[[u32; 3]]) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let key = |v: &Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
    let mut places: BTreeMap<[u64; 3], u32> = BTreeMap::new();
    let mut welded = Vec::new();
    let mut new_number = Vec::with_capacity(corners.len());
    for corner in corners {
        let next = welded.len() as u32;
        let number = *places.entry(key(corner)).or_insert_with(|| {
            welded.push(*corner);
            next
        });
        new_number.push(number);
    }
    let triangles = triangles
        .iter()
        .map(|t| t.map(|i| new_number[i as usize]))
        // A triangle whose corners were one place is nothing.
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    (welded, triangles)
}

/// How far a corner of a box may be from where a perfect box puts it, in
/// metres: the 0.1 mm the Blender scripts round to, with room to spare.
const BOX_TOLERANCE: f64 = 1e-3;

/// A box, from the corners of its mesh: its centre, its size along its own
/// axes and which way they point.
fn fit_box(corners: &[Vec3]) -> Result<MapShape, String> {
    let (welded, _) = weld(corners, &[]);
    if welded.len() != 8 {
        return Err(format!("it has {} corners, and a box has 8", welded.len()));
    }
    let origin = welded[0];
    // From the first corner to each of the others: the three edges, and the
    // sums of two or three of them. The shortest is an edge; the shortest at
    // right angles to it is another (a face's diagonal is never at right
    // angles to an edge it starts with); the one at right angles to both is
    // the third.
    let mut to_others: Vec<Vec3> = (1..8).map(|i| welded[i] - origin).collect();
    to_others.sort_by(|a, b| a.length().total_cmp(&b.length()));
    let at_right_angles = |a: Vec3, b: Vec3| a.dot(b).abs() <= 0.01 * a.length() * b.length();
    let mut edges: Vec<Vec3> = Vec::new();
    for candidate in to_others {
        if candidate.length() <= BOX_TOLERANCE {
            return Err("two of its corners are in the same place".to_string());
        }
        if edges.iter().all(|e| at_right_angles(*e, candidate)) {
            edges.push(candidate);
        }
        if edges.len() == 3 {
            break;
        }
    }
    if edges.len() != 3 {
        return Err("its sides don't meet at right angles".to_string());
    }
    // Every corner must be where origin + a·e1 + b·e2 + c·e3 puts it, for a,
    // b and c each 0 or 1.
    let mut expected = Vec::new();
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                expected.push(
                    origin
                        + edges[0] * f64::from(a)
                        + edges[1] * f64::from(b)
                        + edges[2] * f64::from(c),
                );
            }
        }
    }
    for corner in &welded {
        let off = expected
            .iter()
            .map(|e| (*e - *corner).length())
            .fold(f64::INFINITY, f64::min);
        if off > BOX_TOLERANCE {
            return Err("its corners aren't those of a box".to_string());
        }
    }
    let centre = origin + (edges[0] + edges[1] + edges[2]) / 2.0;
    // Each side goes with the world axis it lies nearest, if that gives every
    // axis one: a level box then has no turn at all, and its size reads
    // east, north and up.
    let nearest: Vec<usize> = edges
        .iter()
        .map(|e| {
            let parts = [e.x.abs(), e.y.abs(), e.z.abs()];
            (0..3).fold(0, |best, k| if parts[k] > parts[best] { k } else { best })
        })
        .collect();
    let mut slots = [None; 3];
    for (edge, axis) in nearest.iter().enumerate() {
        slots[*axis] = Some(edge);
    }
    let order: [usize; 3] = match slots {
        [Some(a), Some(b), Some(c)] => [a, b, c],
        _ => [0, 1, 2],
    };
    let size = Vec3::new(
        edges[order[0]].length(),
        edges[order[1]].length(),
        edges[order[2]].length(),
    );
    let x = unit(edges[order[0]]);
    let mut y = edges[order[1]] - x * edges[order[1]].dot(x);
    y = unit(y);
    // Signs: the first two axes point along their world axis when they can.
    let x = if dominant(x) < 0.0 { -x } else { x };
    let y = if dominant(y) < 0.0 { -y } else { y };
    let z = x.cross(y);
    Ok(MapShape::Box {
        centre,
        size,
        attitude: attitude_of(x, y, z),
    })
}

fn unit(v: Vec3) -> Vec3 {
    v / v.length()
}

/// The sign of a direction's biggest part.
fn dominant(v: Vec3) -> f64 {
    let parts = [v.x, v.y, v.z];
    let big = (0..3).fold(0, |best, k| {
        if parts[k].abs() > parts[best].abs() {
            k
        } else {
            best
        }
    });
    parts[big]
}

/// The attitude whose forward, left and up directions (the box's own x, y and
/// z axes) are these three, which must be at right angles and right-handed.
fn attitude_of(x: Vec3, y: Vec3, z: Vec3) -> Attitude {
    // The three directions are the rotation matrix's columns.
    let (r00, r01, r02) = (x.x, y.x, z.x);
    let (r10, r11, r12) = (x.y, y.y, z.y);
    let (r20, r21, r22) = (x.z, y.z, z.z);
    // A box that is level within 0.06° has no turn at all, so the numbers of
    // a box a Map script rounded to 0.1 mm stay exactly level.
    let off_axis = [r01, r02, r10, r12, r20, r21]
        .iter()
        .fold(0.0f64, |m, n| m.max(n.abs()));
    if off_axis < 1e-3 && r00 > 0.0 && r11 > 0.0 && r22 > 0.0 {
        return Attitude::BODY_IS_WORLD;
    }
    // Shepperd's method: the quaternion of a rotation matrix.
    let trace = r00 + r11 + r22;
    let (w, qx, qy, qz) = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        (s / 4.0, (r21 - r12) / s, (r02 - r20) / s, (r10 - r01) / s)
    } else if r00 > r11 && r00 > r22 {
        let s = (1.0 + r00 - r11 - r22).sqrt() * 2.0;
        ((r21 - r12) / s, s / 4.0, (r01 + r10) / s, (r02 + r20) / s)
    } else if r11 > r22 {
        let s = (1.0 + r11 - r00 - r22).sqrt() * 2.0;
        ((r02 - r20) / s, (r01 + r10) / s, s / 4.0, (r12 + r21) / s)
    } else {
        let s = (1.0 + r22 - r00 - r11).sqrt() * 2.0;
        ((r10 - r01) / s, (r02 + r20) / s, (r12 + r21) / s, s / 4.0)
    };
    let (w, qx, qy, qz) = if w < 0.0 {
        (-w, -qx, -qy, -qz)
    } else {
        (w, qx, qy, qz)
    };
    let vector = Vec3::new(qx, qy, qz);
    let sine = vector.length();
    if sine == 0.0 {
        return Attitude::BODY_IS_WORLD;
    }
    // From the level attitude, body directions are world directions, so the
    // turn about this axis is the same in both.
    let angle = 2.0 * atan2(sine, w);
    Attitude::BODY_IS_WORLD.turned_by(vector / sine * angle)
}
