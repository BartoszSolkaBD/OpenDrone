//! Drawing a Map from its `.glb` (#63): what the Pack checker read out of the
//! file ([`DrawnPart`]) as Bevy meshes, and the material library that gives
//! each part its pictures.
//!
//! - **Meshes.** The `.glb` carries corners, triangles and picture
//!   coordinates, and no normals, so the game works them out: a corner's
//!   normal is the average of the faces that meet there, those of them that
//!   lean within [`CREASE`] of its own face, so a bowl's curve is smooth and
//!   a box's edge is sharp. Tangents come from the picture coordinates, for
//!   the normal maps.
//! - **Materials.** A part names its material (`concrete_smooth`). The
//!   library, `assets-src/materials.toml`, says which CC0 pictures that name
//!   means: `cargo xtask textures` has fetched them into
//!   `assets-src/textures/`, or, for a texture made from scratch, they are
//!   committed in `assets-src/generated/`. Either folder is in the repo or in
//!   a downloaded build, which `cargo xtask package` lays out the same way
//!   (#81). A material with no
//!   picture on disk is drawn in its flat colour from the `.glb`, so the Map
//!   is always flyable.
//!
//! - **No visible repeat.** A material with `break_repeat` or
//!   `macro_strength` in the library is drawn as a [`VariedMaterial`]
//!   ([`crate::repeat_breaking`]): its picture's repeat is broken when it is
//!   drawn and the Map's large-scale picture (`[macro]`) is laid over it
//!   (#304). Every other material is drawn as before.
//! - **Roughness, occlusion, metal and wetness.** A material with
//!   `roughness_map`, `ao_map`, `metallic_map` or `wetness` in the library is
//!   drawn by the same shader ([`crate::surface_look`]), so wet, dull,
//!   polished and rusty parts differ as they do in life (#320). A `metallic`
//!   number alone is the plain material with its metal turned up. A material
//!   that asks for none of it is drawn as before.
//! - **Baked light.** A Map with `[light]` and a light atlas
//!   ([`MapLighting`], [`read_light_atlas`]) draws its parts lit by the atlas,
//!   through Bevy's own lightmaps, on each mesh's second picture coordinates
//!   (#294). Its Quads get the Map's sky as ambient light and its sun as
//!   a direction.
//!
//! Nothing here touches the Simulation: it only draws what the Pack checker
//! already handed over.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
    ImageSamplerDescriptor, ImageType,
};
use bevy::mesh::PrimitiveTopology;
use bevy::pbr::ExtendedMaterial;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use opendrone_pack::{DrawnPart, MapLight};

use crate::light_atlas::read_exr;
use crate::repeat_breaking::{BreakerSettings, RepeatBreaker, VariedMaterial};
use crate::surface_look::{Plane, pack_surface, plane_of, wet};

/// Two faces are one smooth surface when their normals are closer than this
/// (the cosine of 35°); further apart, the edge between them is sharp.
pub const CREASE: f32 = 0.819_152;

/// Where the material library is, from the repo's root.
const LIBRARY: &str = "assets-src/materials.toml";

/// Where the fetched pictures are, from the repo's root.
const TEXTURES: &str = "assets-src/textures";

/// Where the pictures made from scratch are, from the repo's root: committed,
/// not fetched (ADR-0024).
const GENERATED: &str = "assets-src/generated";

/// A point in the Map (east, north, up) in Bevy's axes (east +x, up +y,
/// north -z).
fn bevy_point(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[2], -p[1])
}

/// One part of the Map as a mesh: three corners for every triangle, each
/// with a smoothed normal, its picture coordinates and a tangent.
pub fn part_mesh(part: &DrawnPart) -> Mesh {
    let corners: Vec<Vec3> = part.corners.iter().map(|c| bevy_point(*c)).collect();
    // Each triangle's face, as a normal as long as its area is twice big.
    let faces: Vec<Vec3> = part
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| corners[i as usize]);
            (b - a).cross(c - a)
        })
        .collect();
    // The triangles that meet at each place: corners the file repeats (where
    // its picture coordinates change) are one place.
    let mut meeting: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
    for (index, triangle) in part.triangles.iter().enumerate() {
        for corner in triangle {
            let c = corners[*corner as usize];
            let list = meeting
                .entry([c.x.to_bits(), c.y.to_bits(), c.z.to_bits()])
                .or_default();
            if list.last() != Some(&index) {
                list.push(index);
            }
        }
    }
    let mut positions = Vec::with_capacity(part.triangles.len() * 3);
    let mut normals = Vec::with_capacity(part.triangles.len() * 3);
    let mut uvs = Vec::with_capacity(part.triangles.len() * 3);
    let mut tangents = Vec::with_capacity(part.triangles.len() * 3);
    let mut light_uvs = Vec::new();
    for (index, triangle) in part.triangles.iter().enumerate() {
        let own = faces[index].normalize_or_zero();
        let [a, b, c] = triangle.map(|i| corners[i as usize]);
        let uv = triangle.map(|i| part.uvs.get(i as usize).copied().unwrap_or([0.0, 0.0]));
        let tangent = triangle_tangent([a, b, c], uv);
        for (k, corner) in [a, b, c].into_iter().enumerate() {
            let key = [corner.x.to_bits(), corner.y.to_bits(), corner.z.to_bits()];
            let mut sum = Vec3::ZERO;
            for other in &meeting[&key] {
                if faces[*other].normalize_or_zero().dot(own) >= CREASE {
                    sum += faces[*other];
                }
            }
            let normal = sum.normalize_or(own);
            // Tangent at right angles to the smoothed normal, and its side.
            let (t, bitangent) = tangent;
            let t = (t - normal * normal.dot(t)).normalize_or_zero();
            let sign = if normal.cross(t).dot(bitangent) < 0.0 {
                -1.0
            } else {
                1.0
            };
            positions.push(corner.to_array());
            normals.push(normal.to_array());
            uvs.push(uv[k]);
            if !part.light_uvs.is_empty() {
                light_uvs.push(
                    part.light_uvs
                        .get(triangle[k] as usize)
                        .copied()
                        .unwrap_or([0.0, 0.0]),
                );
            }
            tangents.push([t.x, t.y, t.z, sign]);
        }
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    if light_uvs.is_empty() {
        mesh
    } else {
        // Where the part is in the light atlas: the lightmap looks it up here.
        mesh.with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, light_uvs)
    }
}

/// Which way picture coordinates `u` and `v` grow across a triangle, in the
/// picture's own direction (the tangent and bitangent).
fn triangle_tangent(p: [Vec3; 3], uv: [[f32; 2]; 3]) -> (Vec3, Vec3) {
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let (du1, dv1) = (uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]);
    let (du2, dv2) = (uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]);
    let det = du1 * dv2 - du2 * dv1;
    if det.abs() < 1e-12 {
        let t = e1.normalize_or(Vec3::X);
        return (t, e2);
    }
    let r = 1.0 / det;
    ((e1 * dv2 - e2 * dv1) * r, (e2 * du1 - e1 * du2) * r)
}

/// A texture's picture files.
#[derive(Clone, Debug)]
struct Pictures {
    diffuse: PathBuf,
    normal: PathBuf,
    /// The pictures of roughness, occlusion and metal, if the material asks
    /// for them and has them: next to the colour picture.
    roughness: PathBuf,
    ao: PathBuf,
    metallic: PathBuf,
}

/// What a material asks of the game beyond its pictures (#304). A material
/// asks for none of it, and is drawn as before, unless the library says so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variation {
    /// `break_repeat`: draw the picture in hex tiles, so it doesn't repeat.
    pub break_repeat: bool,
    /// `turn_cells`: each tile also turns and mirrors the picture, for a
    /// material with no grain (dirt, grass).
    pub turn_cells: bool,
    /// `snap`: a tile shifts the picture by whole multiples of 1/`u` and 1/`v`
    /// of it (0 for any shift), which keeps the lines of a picture with a
    /// grid (bricks, ridges) in line from one tile to the next.
    pub snap: [u32; 2],
    /// `macro_strength`: how strongly the large-scale picture shows; 0 for
    /// none.
    pub macro_strength: f32,
}

/// What a material asks of the game for how its surface answers light (#320).
/// A material asks for none of it, and is drawn as before, unless the library
/// says so.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SurfaceAsk {
    /// `roughness_map`: read `roughness.jpg` for each pixel's roughness, in
    /// place of the one number.
    pub roughness_map: bool,
    /// `ao_map`: read `ao.jpg`, how shut in each pixel is from the light.
    pub ao_map: bool,
    /// `metallic_map`: read `metallic.jpg`, which pixels are metal.
    pub metallic_map: bool,
    /// `metallic`: how metal the surface is, 0 to 1. With a picture, it
    /// scales the picture, and is 1 if the library leaves it out.
    pub metallic: f32,
    /// `wetness`: how wet the surface is, 0 to 1.
    pub wetness: f32,
}

impl SurfaceAsk {
    /// Whether the surface needs the shader: a picture to read or wetness. A
    /// `metallic` number alone is a setting of the plain material.
    pub fn needs_shader(&self) -> bool {
        self.roughness_map || self.ao_map || self.metallic_map || self.wetness > 0.0
    }

    /// Whether it asks for anything at all.
    pub fn asks(&self) -> bool {
        self.needs_shader() || self.metallic > 0.0
    }

    fn any_map(&self) -> bool {
        self.roughness_map || self.ao_map || self.metallic_map
    }
}

#[derive(Clone, Debug)]
struct Entry {
    roughness: f32,
    pictures: Option<Pictures>,
    variation: Option<Variation>,
    surface: SurfaceAsk,
}

/// The large-scale picture over the world (`[macro]` in the library): the
/// file, and how many metres one repeat of it covers.
#[derive(Clone, Debug)]
struct WidePicture {
    file: PathBuf,
    metres: f32,
}

/// What a material's entry asks for, if anything: `break_repeat = true` and
/// a `macro_strength` above 0 are the two ways in.
fn variation_of(entry: &toml::Value) -> Option<Variation> {
    let break_repeat = entry
        .get("break_repeat")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let macro_strength = entry
        .get("macro_strength")
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .unwrap_or(0.0)
        .max(0.0) as f32;
    if !break_repeat && macro_strength <= 0.0 {
        return None;
    }
    let snap = entry
        .get("snap")
        .and_then(toml::Value::as_array)
        .map(|a| {
            let at = |i: usize| {
                a.get(i)
                    .and_then(toml::Value::as_integer)
                    .unwrap_or(0)
                    .max(0)
            };
            [at(0) as u32, at(1) as u32]
        })
        .unwrap_or([0, 0]);
    Some(Variation {
        break_repeat,
        turn_cells: entry
            .get("turn_cells")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        snap,
        macro_strength,
    })
}

/// A number between 0 and 1 in an entry, if it has it (an integer is fine).
fn unit_number(entry: &toml::Value, key: &str) -> Option<f32> {
    entry
        .get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .map(|v| v.clamp(0.0, 1.0) as f32)
}

/// What a material's entry asks of its surface (#320).
fn surface_of(entry: &toml::Value) -> SurfaceAsk {
    let flag = |key: &str| {
        entry
            .get(key)
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
    };
    let metallic_map = flag("metallic_map");
    SurfaceAsk {
        roughness_map: flag("roughness_map"),
        ao_map: flag("ao_map"),
        metallic_map,
        metallic: unit_number(entry, "metallic").unwrap_or(if metallic_map { 1.0 } else { 0.0 }),
        wetness: unit_number(entry, "wetness").unwrap_or(0.0),
    }
}

/// What `materials.toml` says about a material's drawing. A material that
/// asks for nothing is a [`StandardMaterial`], and one that does a
/// [`VariedMaterial`].
pub enum Surface {
    Plain(StandardMaterial),
    Varied(VariedMaterial),
}

impl Surface {
    /// The standard material underneath, for what both kinds share.
    /// Whether the picture's repeat is broken (#304).
    pub fn repeat_broken(&self) -> bool {
        matches!(self, Surface::Varied(m) if m.extension.settings.snap_turn.w > 0.5)
    }

    /// Whether it reads a picture of roughness, occlusion or metal (#320).
    pub fn has_surface_picture(&self) -> bool {
        matches!(self, Surface::Varied(m) if m.extension.surface.is_some())
    }

    pub fn standard_mut(&mut self) -> &mut StandardMaterial {
        match self {
            Surface::Plain(m) => m,
            Surface::Varied(m) => &mut m.base,
        }
    }
}

/// The material library: each name a Map's parts use, and the fetched
/// pictures behind it, if any.
#[derive(Clone, Debug, Default)]
pub struct MaterialLibrary {
    entries: BTreeMap<String, Entry>,
    wide: Option<WidePicture>,
}

impl MaterialLibrary {
    /// Reads `assets-src/materials.toml` in the repo, and notes which pictures
    /// `cargo xtask textures` has fetched. A repo with no library, or no
    /// pictures, gives an empty one: every material is then its flat colour.
    pub fn read(repo: &Path) -> MaterialLibrary {
        let Ok(text) = fs::read_to_string(repo.join(LIBRARY)) else {
            return MaterialLibrary::default();
        };
        let Ok(doc) = text.parse::<toml::Table>() else {
            return MaterialLibrary::default();
        };
        let mut entries = BTreeMap::new();
        if let Some(toml::Value::Table(materials)) = doc.get("materials") {
            for (name, entry) in materials {
                let roughness = entry
                    .get("roughness")
                    .and_then(toml::Value::as_float)
                    .unwrap_or(0.9) as f32;
                // A downloaded texture is in the fetched folder, one made from
                // scratch in the committed one; the first with both pictures.
                let pictures =
                    entry
                        .get("texture")
                        .and_then(toml::Value::as_str)
                        .and_then(|texture| {
                            [TEXTURES, GENERATED].into_iter().find_map(|folder| {
                                let folder = repo.join(folder).join(texture);
                                let pictures = Pictures {
                                    diffuse: folder.join("diffuse.jpg"),
                                    normal: folder.join("normal.jpg"),
                                    roughness: folder.join("roughness.jpg"),
                                    ao: folder.join("ao.jpg"),
                                    metallic: folder.join("metallic.jpg"),
                                };
                                (pictures.diffuse.is_file() && pictures.normal.is_file())
                                    .then_some(pictures)
                            })
                        });
                entries.insert(
                    name.clone(),
                    Entry {
                        roughness,
                        pictures,
                        variation: variation_of(entry),
                        surface: surface_of(entry),
                    },
                );
            }
        }
        MaterialLibrary {
            entries,
            wide: wide_picture(&doc, repo),
        }
    }

    /// How many materials have their pictures on disk.
    pub fn textured(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.pictures.is_some())
            .count()
    }

    /// Whether the library asks for the material `name` to be drawn with its
    /// repeat broken or the large-scale picture over it (#304), pictures on
    /// disk or not.
    pub fn asks_for_variation(&self, name: &str) -> bool {
        self.entries
            .get(name)
            .is_some_and(|e| e.variation.is_some())
    }

    /// Whether the library asks for the material `name` to read pictures of
    /// roughness, occlusion or metal, to be wet, or to be metal (#320),
    /// pictures on disk or not.
    pub fn asks_for_surface(&self, name: &str) -> bool {
        self.entries.get(name).is_some_and(|e| e.surface.asks())
    }

    /// How many materials are drawn with their repeat broken, of those that
    /// have their pictures on disk.
    pub fn varied(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.pictures.is_some() && e.variation.is_some())
            .count()
    }

    /// The material called `name`: its pictures if they're fetched, else the
    /// flat colour the `.glb` gives it (linear red, green, blue, alpha).
    pub fn material(&self, name: &str, flat: [f32; 4], images: &mut Assets<Image>) -> Surface {
        let entry = self.entries.get(name);
        let roughness = entry.map_or(0.9, |e| e.roughness);
        let ask = entry.map(|e| e.surface).unwrap_or_default();
        let pictures = entry.and_then(|e| e.pictures.as_ref()).and_then(|p| {
            Some((
                load(&p.diffuse, true, images)?,
                load(&p.normal, false, images)?,
            ))
        });
        let variation = entry.and_then(|e| e.variation);
        match pictures {
            Some((diffuse, normal)) if variation.is_some() || ask.needs_shader() => {
                // Asking for a surface and not for the repeat to be broken is
                // the same shader with the repeat left alone.
                let variation = variation.unwrap_or(Variation {
                    break_repeat: false,
                    turn_cells: false,
                    snap: [0, 0],
                    macro_strength: 0.0,
                });
                let wide = self
                    .wide
                    .as_ref()
                    .filter(|_| variation.macro_strength > 0.0)
                    .and_then(|w| Some((load(&w.file, false, images)?, w.metres)));
                let (wide_picture, per_metre) = match wide {
                    Some((picture, metres)) => (Some(picture), 1.0 / metres),
                    None => (None, 0.0),
                };
                let surface = entry
                    .and_then(|e| e.pictures.as_ref())
                    .and_then(|p| surface_picture(name, p, &ask, roughness, images));
                Surface::Varied(ExtendedMaterial {
                    base: StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: roughness,
                        metallic: ask.metallic,
                        ..default()
                    },
                    extension: RepeatBreaker {
                        settings: BreakerSettings {
                            snap_turn: Vec4::new(
                                variation.snap[0] as f32,
                                variation.snap[1] as f32,
                                f32::from(u8::from(variation.turn_cells)),
                                f32::from(u8::from(variation.break_repeat)),
                            ),
                            wide: Vec4::new(
                                if wide_picture.is_some() {
                                    variation.macro_strength
                                } else {
                                    0.0
                                },
                                per_metre,
                                0.0,
                                0.0,
                            ),
                            surface: Vec4::new(
                                f32::from(u8::from(surface.is_some())),
                                ask.wetness,
                                0.0,
                                0.0,
                            ),
                        },
                        diffuse,
                        normal,
                        wide: wide_picture,
                        surface,
                    },
                })
            }
            Some((diffuse, normal)) => Surface::Plain(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(diffuse),
                normal_map_texture: Some(normal),
                perceptual_roughness: roughness,
                metallic: ask.metallic,
                ..default()
            }),
            None => {
                // No pictures, so wetness is worked out here: the colour is
                // darker, the roughness lower and the reflectance higher.
                let wet = wet(roughness, ask.wetness);
                Surface::Plain(StandardMaterial {
                    base_color: Color::linear_rgba(
                        flat[0] * wet.colour,
                        flat[1] * wet.colour,
                        flat[2] * wet.colour,
                        flat[3],
                    ),
                    perceptual_roughness: wet.roughness,
                    reflectance: wet.reflectance,
                    metallic: ask.metallic,
                    ..default()
                })
            }
        }
    }
}

/// The picture of occlusion, roughness and metal a material asks for, packed
/// into one and added to the images. A picture it asks for and can't have (no
/// file, or one that isn't a picture) is said in the log and left out, with
/// what it would have said replaced by its default; with none to read, there
/// is no picture and the library's roughness number stands.
fn surface_picture(
    name: &str,
    pictures: &Pictures,
    ask: &SurfaceAsk,
    roughness: f32,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if !ask.any_map() {
        return None;
    }
    let read = |wanted: bool, what: &str, path: &Path| {
        if !wanted {
            return None;
        }
        let plane = read_plane(path);
        if plane.is_none() {
            warn!(
                "material {name} asks for its {what} picture, and {} can't be read: using the default",
                path.display()
            );
        }
        plane
    };
    let image = pack_surface(
        read(ask.roughness_map, "roughness", &pictures.roughness),
        read(ask.ao_map, "occlusion", &pictures.ao),
        read(ask.metallic_map, "metal", &pictures.metallic),
        roughness,
    )?;
    Some(images.add(with_mipmaps(image)))
}

/// A grey picture's pixels, from a JPG on disk.
fn read_plane(path: &Path) -> Option<Plane> {
    let bytes = fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("jpg"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::default(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()?;
    plane_of(&image)
}

/// The large-scale picture the library's `[macro]` table names, if its
/// pictures are on disk, with the metres one repeat of it covers.
fn wide_picture(doc: &toml::Table, repo: &Path) -> Option<WidePicture> {
    let name = doc.get("macro")?.get("texture")?.as_str()?;
    let size = doc.get("textures")?.get(name)?.get("size")?.as_str()?;
    let metres: f32 = size.trim().strip_suffix('m')?.trim().parse().ok()?;
    let file = [TEXTURES, GENERATED]
        .into_iter()
        .map(|folder| repo.join(folder).join(name).join("diffuse.jpg"))
        .find(|file| file.is_file())?;
    (metres > 0.0).then_some(WidePicture { file, metres })
}

/// Reads a picture from disk, with its mipmaps worked out and its edges
/// repeating, and adds it to the images.
fn load(path: &Path, is_srgb: bool, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let bytes = fs::read(path).ok()?;
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("jpg"),
        CompressedImageFormats::NONE,
        is_srgb,
        sampler,
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()?;
    Some(images.add(with_mipmaps(image)))
}

/// The picture with every smaller size down to one pixel, each the average of
/// the four above it, so far away it doesn't shimmer. The picture must be
/// four bytes a pixel.
pub fn with_mipmaps(mut image: Image) -> Image {
    let (mut width, mut height) = (image.width() as usize, image.height() as usize);
    let Some(base) = image.data.take() else {
        return image;
    };
    if base.len() != width * height * 4 {
        image.data = Some(base);
        return image;
    }
    let mut all = base.clone();
    let mut level = base;
    let mut count = 1;
    while width > 1 || height > 1 {
        let (next_width, next_height) = ((width / 2).max(1), (height / 2).max(1));
        let mut next = Vec::with_capacity(next_width * next_height * 4);
        for y in 0..next_height {
            for x in 0..next_width {
                for channel in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(width - 1);
                        let sy = (y * 2 + dy).min(height - 1);
                        sum += u32::from(level[(sy * width + sx) * 4 + channel]);
                    }
                    next.push(((sum + 2) / 4) as u8);
                }
            }
        }
        all.extend_from_slice(&next);
        level = next;
        (width, height) = (next_width, next_height);
        count += 1;
    }
    image.data = Some(all);
    image.texture_descriptor.mip_level_count = count;
    image
}

/// How bright a strength of 1 is, in the game's own light: what a direct sun
/// of this many lux lights a white surface square-on to it with, which
/// `Exposure::SUNLIGHT` is set for. The real-time sun the Skate Park and
/// Bando have is 1 in these units, so a baked Map with `sun_strength` 1 is as
/// bright as they are.
pub const ONE_STRENGTH_LUX: f32 = 100_000.0;

/// What the game lights a baked Map with, worked out from its `[light]`
/// ([`MapLight`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapLighting {
    /// How bright the atlas is drawn: the material's `lightmap_exposure`. An
    /// atlas value of 1 (a white surface that a strength of 1 lights) becomes
    /// what a sun of [`ONE_STRENGTH_LUX`] gives such a surface, which is
    /// `ONE_STRENGTH_LUX / pi`, as Bevy's own diffuse light divides by pi.
    pub lightmap_exposure: f32,
    /// The sun's illuminance, in lux, for what the atlas doesn't light
    /// (Quads).
    pub sun_illuminance: f32,
    pub sun_colour: Color,
    /// The way the sun's light travels, in Bevy's axes: away from the sun.
    pub light_travels: Vec3,
    /// The sky's colour and brightness (cd/m2) as ambient light, for what
    /// the atlas doesn't light. An open sky of strength 1 lights a white
    /// surface facing up as much as a sun of strength 1 lights one square-on.
    pub sky_colour: Color,
    pub sky_brightness: f32,
}

impl MapLighting {
    pub fn of(light: &MapLight) -> MapLighting {
        let unit = ONE_STRENGTH_LUX / std::f32::consts::PI;
        let (sin_az, cos_az) = (
            light.sun_azimuth.sin() as f32,
            light.sun_azimuth.cos() as f32,
        );
        let (sin_el, cos_el) = (
            light.sun_elevation.sin() as f32,
            light.sun_elevation.cos() as f32,
        );
        // To the sun in the Map's axes (east, north, up), then in Bevy's.
        let to_sun = Vec3::new(sin_az * cos_el, sin_el, -cos_az * cos_el);
        let [sr, sg, sb] = light.sun_colour;
        let [kr, kg, kb] = light.sky_colour;
        MapLighting {
            lightmap_exposure: unit,
            sun_illuminance: light.sun_strength as f32 * ONE_STRENGTH_LUX,
            sun_colour: Color::srgb_u8(sr, sg, sb),
            light_travels: -to_sun,
            sky_colour: Color::srgb_u8(kr, kg, kb),
            sky_brightness: light.sky_strength as f32 * unit,
        }
    }
}

/// Reads a Map's light atlas (an OpenEXR picture, [`crate::light_atlas`])
/// into a picture of half floats, which every graphics card can filter and
/// which takes half the memory of 32-bit ones.
pub fn read_light_atlas(path: &Path, images: &mut Assets<Image>) -> Result<Handle<Image>, String> {
    let bytes = fs::read(path).map_err(|e| format!("can't be read: {e}"))?;
    let picture = read_exr(&bytes)?;
    let data: Vec<u8> = picture
        .texels
        .iter()
        .flatten()
        .flat_map(|bits| bits.to_le_bytes())
        .collect();
    let mut atlas = Image::new(
        Extent3d {
            width: picture.width,
            height: picture.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    atlas.sampler = ImageSampler::linear();
    Ok(images.add(atlas))
}

/// A number as the bits of a 16-bit decimal number, rounded to the nearest
/// (ties to even). One too big is infinity, a tiny one a denormal or zero.
pub fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let fraction = bits & 0x007f_ffff;
    if exponent == 0xff {
        // Infinity, or a NaN that stays one.
        return sign | 0x7c00 | if fraction != 0 { 0x0200 } else { 0 };
    }
    let unbiased = exponent - 127;
    if unbiased > 15 {
        return sign | 0x7c00;
    }
    if unbiased >= -14 {
        let mut half = (((unbiased + 15) as u32) << 10) | (fraction >> 13);
        let rest = fraction & 0x1fff;
        if rest > 0x1000 || (rest == 0x1000 && half & 1 == 1) {
            // A carry out of the fraction raises the exponent, as it should.
            half += 1;
        }
        return sign | half as u16;
    }
    if unbiased < -25 {
        return sign;
    }
    // A denormal: the leading one is written out.
    let mantissa = fraction | 0x0080_0000;
    let shift = (-unbiased - 14 + 13) as u32;
    let mut half = mantissa >> shift;
    let rest = mantissa & ((1 << shift) - 1);
    let middle = 1 << (shift - 1);
    if rest > middle || (rest == middle && half & 1 == 1) {
        half += 1;
    }
    sign | half as u16
}
