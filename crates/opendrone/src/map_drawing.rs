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
//!   means, and `cargo xtask textures` has fetched them into
//!   `assets-src/textures/`, in the repo or in a downloaded build, which
//!   `cargo xtask package` lays out the same way (#81). A material with no
//!   picture on disk is drawn in its flat colour from the `.glb`, so the Map
//!   is always flyable.
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
use bevy::prelude::*;
use opendrone_pack::DrawnPart;

/// Two faces are one smooth surface when their normals are closer than this
/// (the cosine of 35°); further apart, the edge between them is sharp.
pub const CREASE: f32 = 0.819_152;

/// Where the material library is, from the repo's root.
const LIBRARY: &str = "assets-src/materials.toml";

/// Where the fetched pictures are, from the repo's root.
const TEXTURES: &str = "assets-src/textures";

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
            tangents.push([t.x, t.y, t.z, sign]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tangents)
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
}

#[derive(Clone, Debug)]
struct Entry {
    roughness: f32,
    pictures: Option<Pictures>,
}

/// The material library: each name a Map's parts use, and the fetched
/// pictures behind it, if any.
#[derive(Clone, Debug, Default)]
pub struct MaterialLibrary {
    entries: BTreeMap<String, Entry>,
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
                let pictures = entry
                    .get("texture")
                    .and_then(toml::Value::as_str)
                    .map(|texture| Pictures {
                        diffuse: repo.join(TEXTURES).join(texture).join("diffuse.jpg"),
                        normal: repo.join(TEXTURES).join(texture).join("normal.jpg"),
                    })
                    .filter(|p| p.diffuse.is_file() && p.normal.is_file());
                entries.insert(
                    name.clone(),
                    Entry {
                        roughness,
                        pictures,
                    },
                );
            }
        }
        MaterialLibrary { entries }
    }

    /// How many materials have their pictures on disk.
    pub fn textured(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.pictures.is_some())
            .count()
    }

    /// The material called `name`: its pictures if they're fetched, else the
    /// flat colour the `.glb` gives it (linear red, green, blue, alpha).
    pub fn material(
        &self,
        name: &str,
        flat: [f32; 4],
        images: &mut Assets<Image>,
    ) -> StandardMaterial {
        let entry = self.entries.get(name);
        let roughness = entry.map_or(0.9, |e| e.roughness);
        let pictures = entry.and_then(|e| e.pictures.as_ref()).and_then(|p| {
            Some((
                load(&p.diffuse, true, images)?,
                load(&p.normal, false, images)?,
            ))
        });
        match pictures {
            Some((diffuse, normal)) => StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(diffuse),
                normal_map_texture: Some(normal),
                perceptual_roughness: roughness,
                ..default()
            },
            None => StandardMaterial {
                base_color: Color::linear_rgba(flat[0], flat[1], flat[2], flat[3]),
                perceptual_roughness: roughness,
                ..default()
            },
        }
    }
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
