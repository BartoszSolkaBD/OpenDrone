//! PROTOTYPE (#28). Loads a blockout Map (.glb from #17), gives it a world-scale detail texture
//! and stand-in sunlight, and keeps a triangle copy of it for the Video Signal's ray checks.
//! (In the game the ray check is a read-only question to the Simulation; here it's our own copy.)

use crate::settings::{MapKind, Tuning};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    light::CascadeShadowConfigBuilder,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    world_serialization::WorldInstanceReady,
};
use std::collections::HashMap;

pub const GLB_DIR: &str = "assets-src/maps/prototype-blockouts/out/glb";

/// Blender (x east, y north, z up) to Bevy (x east, y up, z south).
pub fn b(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, -y)
}

#[derive(Component)]
pub struct MapRoot;

#[derive(Component)]
pub struct Sun;

#[derive(Resource, Default)]
pub struct MapState {
    pub current: Option<MapKind>,
    pub root: Option<Entity>,
    pub ready_frames: Option<u32>,
    pub built: bool,
    pub launch_pos: Vec3,
    pub launch_yaw: f32,
    pub geom: MapGeom,
    pub detail: Option<Handle<Image>>,
    pub load_started: Option<std::time::Instant>,
    pub load_ms: f32,
}

pub struct GeomObject {
    pub min: Vec3,
    pub max: Vec3,
    pub tris: Vec<[Vec3; 3]>,
    pub closed: bool,
}

#[derive(Default)]
pub struct MapGeom {
    pub objects: Vec<GeomObject>,
    pub tri_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Crossing {
    pub t: f32,
    pub obj: usize,
    pub entering: bool,
}

fn seg_aabb(a: Vec3, d: Vec3, min: Vec3, max: Vec3) -> bool {
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i] < min[i] || a[i] > max[i] {
                return false;
            }
        } else {
            let inv = 1.0 / d[i];
            let mut ta = (min[i] - a[i]) * inv;
            let mut tb = (max[i] - a[i]) * inv;
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Moller-Trumbore, both sides. Returns (t along a->a+d, d . geometric normal).
fn seg_tri(a: Vec3, d: Vec3, tri: &[Vec3; 3]) -> Option<(f32, f32)> {
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = a - tri[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some((t, d.dot(e1.cross(e2))))
}

impl MapGeom {
    /// Every place the segment a->b passes through a Map surface: "which Map surfaces does this
    /// line pass through, and where?"
    pub fn crossings(&self, a: Vec3, bb: Vec3) -> Vec<Crossing> {
        let d = bb - a;
        let mut out = Vec::new();
        for (i, o) in self.objects.iter().enumerate() {
            if !seg_aabb(a, d, o.min, o.max) {
                continue;
            }
            for tri in &o.tris {
                if let Some((t, dn)) = seg_tri(a, d, tri) {
                    out.push(Crossing { t, obj: i, entering: dn < 0.0 });
                }
            }
        }
        out.sort_by(|x, y| x.t.total_cmp(&y.t));
        out
    }

    /// First surface hit on a->b, with its normal facing back toward a.
    pub fn first_hit(&self, a: Vec3, bb: Vec3) -> Option<(f32, Vec3)> {
        let d = bb - a;
        let mut best: Option<(f32, Vec3)> = None;
        for o in &self.objects {
            if !seg_aabb(a, d, o.min, o.max) {
                continue;
            }
            for tri in &o.tris {
                if let Some((t, _)) = seg_tri(a, d, tri) {
                    if best.is_none_or(|(bt, _)| t < bt) {
                        let mut n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
                        if n.dot(d) > 0.0 {
                            n = -n;
                        }
                        best = Some((t, n));
                    }
                }
            }
        }
        best
    }
}

pub fn spawn_map(commands: &mut Commands, asset_server: &AssetServer, state: &mut MapState, kind: MapKind) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }
    let path = format!("{GLB_DIR}/{}", kind.glb());
    let root = commands
        .spawn((WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))), MapRoot))
        .id();
    state.root = Some(root);
    state.current = Some(kind);
    state.ready_frames = None;
    state.built = false;
    state.geom = MapGeom::default();
    state.load_started = Some(std::time::Instant::now());
    let (pos, yaw) = default_launch(kind);
    state.launch_pos = pos;
    state.launch_yaw = yaw;
}

/// From the #17 blockouts: Skate Park L1 (-1.8, 6) facing west; Bando L3 (0, -17) facing north.
pub fn default_launch(kind: MapKind) -> (Vec3, f32) {
    match kind {
        MapKind::SkatePark => (b(-1.8, 6.0, 0.0), std::f32::consts::FRAC_PI_2),
        MapKind::Bando => (b(0.0, -17.0, -0.03), 0.0),
    }
}

pub fn on_map_ready(ev: On<WorldInstanceReady>, roots: Query<(), With<MapRoot>>, mut state: ResMut<MapState>) {
    if roots.get(ev.entity).is_ok() && state.root == Some(ev.entity) {
        state.ready_frames = Some(0);
    }
}

/// Two frames after the scene spawned (so world transforms are valid): detail UVs, the shared
/// detail texture on every material, the Launch Spot, and the ray-check triangles.
#[allow(clippy::too_many_arguments)]
pub fn build_map(
    mut state: ResMut<MapState>,
    children: Query<&Children>,
    mesh_q: Query<(&Mesh3d, &GlobalTransform, Option<&MeshMaterial3d<StandardMaterial>>)>,
    names: Query<(&Name, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    tuning: Res<Tuning>,
) {
    let Some(n) = state.ready_frames else { return };
    if state.built {
        return;
    }
    if n < 2 {
        state.ready_frames = Some(n + 1);
        return;
    }
    let Some(root) = state.root else { return };
    if state.detail.is_none() {
        state.detail = Some(images.add(detail_texture()));
    }
    let detail = state.detail.clone().unwrap();
    let mut geom = MapGeom::default();
    let mut done_materials = std::collections::HashSet::new();
    for e in children.iter_descendants(root) {
        if let Ok((name, gt)) = names.get(e) {
            if name.as_str().starts_with("launch_spot_") {
                state.launch_pos = gt.translation();
                let f = gt.forward();
                state.launch_yaw = (-f.x).atan2(-f.z);
            }
        }
        let Ok((mesh3d, gt, mat)) = mesh_q.get(e) else { continue };
        let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else { continue };
        let affine = gt.affine();
        // Ray-check triangles (world space) before de-indexing.
        let positions: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| affine.transform_point3(Vec3::from(*p))).collect(),
            _ => continue,
        };
        let idx: Vec<usize> = match mesh.indices() {
            Some(Indices::U16(v)) => v.iter().map(|&i| i as usize).collect(),
            Some(Indices::U32(v)) => v.iter().map(|&i| i as usize).collect(),
            None => (0..positions.len()).collect(),
        };
        let mut tris = Vec::with_capacity(idx.len() / 3);
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let key = |p: Vec3| ((p.x * 2000.0).round() as i64, (p.y * 2000.0).round() as i64, (p.z * 2000.0).round() as i64);
        let mut edges: HashMap<((i64, i64, i64), (i64, i64, i64)), u32> = HashMap::new();
        for t in idx.chunks_exact(3) {
            let tri = [positions[t[0]], positions[t[1]], positions[t[2]]];
            for p in tri {
                min = min.min(p);
                max = max.max(p);
            }
            for (a, c) in [(0, 1), (1, 2), (2, 0)] {
                let (ka, kc) = (key(tri[a]), key(tri[c]));
                let k = if ka < kc { (ka, kc) } else { (kc, ka) };
                *edges.entry(k).or_insert(0) += 1;
            }
            tris.push(tri);
        }
        let closed = !edges.is_empty() && edges.values().all(|&c| c == 2);
        geom.tri_count += tris.len();
        geom.objects.push(GeomObject { min: min - Vec3::splat(0.01), max: max + Vec3::splat(0.01), tris, closed });

        // Detail UVs: one flat projection per triangle, 2 m per texture tile.
        if tuning.scene.detail_texture {
            mesh.duplicate_vertices();
            let wpos: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| affine.transform_point3(Vec3::from(*p))).collect(),
                _ => continue,
            };
            let mut uvs = vec![[0.0f32; 2]; wpos.len()];
            for t in (0..wpos.len()).step_by(3) {
                if t + 2 >= wpos.len() {
                    break;
                }
                let n = (wpos[t + 1] - wpos[t]).cross(wpos[t + 2] - wpos[t]).abs();
                for k in 0..3 {
                    let p = wpos[t + k] * 0.5;
                    uvs[t + k] = if n.y >= n.x && n.y >= n.z {
                        [p.x, p.z]
                    } else if n.x >= n.z {
                        [p.z, -p.y]
                    } else {
                        [p.x, -p.y]
                    };
                }
            }
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            if let Some(mat) = mat {
                if done_materials.insert(mat.0.id()) {
                    if let Some(mut m) = materials.get_mut(&mat.0) {
                        m.base_color_texture = Some(detail.clone());
                        m.perceptual_roughness = 0.9;
                    }
                }
            }
        }
    }
    info!(
        "map built: {} objects, {} triangles, {} closed",
        geom.objects.len(),
        geom.tri_count,
        geom.objects.iter().filter(|o| o.closed).count()
    );
    state.geom = geom;
    state.built = true;
    if let Some(t0) = state.load_started {
        state.load_ms = t0.elapsed().as_secs_f32() * 1000.0;
    }
}

/// A tileable concrete-ish texture (2 m per tile) with a faint 1 m grid, so softness, grain and
/// aliasing have detail to show against. Stand-in for the CC0 textures.
pub fn detail_texture() -> Image {
    const N: usize = 512;
    let lattice = |f: usize, seed: u32, x: usize, y: usize| -> f32 {
        let h = (x % f) as u32 * 73856093 ^ (y % f) as u32 * 19349663 ^ seed.wrapping_mul(83492791);
        let h = h.wrapping_mul(0x9E3779B1) ^ (h >> 15);
        let h = h.wrapping_mul(0x85EBCA77) ^ (h >> 13);
        (h & 0xFFFF) as f32 / 65535.0
    };
    let noise = |f: usize, seed: u32, u: f32, v: f32| -> f32 {
        let x = u * f as f32;
        let y = v * f as f32;
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (s(fx), s(fy));
        let a = lattice(f, seed, x0, y0);
        let bq = lattice(f, seed, x0 + 1, y0);
        let c = lattice(f, seed, x0, y0 + 1);
        let d = lattice(f, seed, x0 + 1, y0 + 1);
        (a + (bq - a) * sx) + ((c + (d - c) * sx) - (a + (bq - a) * sx)) * sy
    };
    let mut levels: Vec<Vec<f32>> = Vec::new();
    let mut base = vec![0.0f32; N * N];
    for y in 0..N {
        for x in 0..N {
            let (u, v) = (x as f32 / N as f32, y as f32 / N as f32);
            let mut n = 0.0;
            let mut amp = 0.5;
            let mut tot = 0.0;
            for (i, f) in [4usize, 8, 16, 32, 64, 128].iter().enumerate() {
                n += noise(*f, i as u32 + 1, u, v) * amp;
                tot += amp;
                amp *= 0.62;
            }
            let n = n / tot;
            let mut val = 0.78 + (n - 0.5) * 0.55;
            // 1 m grid: lines 1.6 cm wide at x/y = 0 and N/2.
            let near = |c: usize| {
                let m = c % (N / 2);
                m.min(N / 2 - m)
            };
            if near(x) < 2 || near(y) < 2 {
                val *= 0.72;
            }
            base[y * N + x] = val.clamp(0.0, 1.2);
        }
    }
    levels.push(base);
    while levels.last().unwrap().len() > 1 {
        let prev = levels.last().unwrap();
        let w = (prev.len() as f32).sqrt() as usize;
        let h = w / 2;
        let mut next = vec![0.0f32; h * h];
        for y in 0..h {
            for x in 0..h {
                next[y * h + x] = (prev[(2 * y) * w + 2 * x]
                    + prev[(2 * y) * w + 2 * x + 1]
                    + prev[(2 * y + 1) * w + 2 * x]
                    + prev[(2 * y + 1) * w + 2 * x + 1])
                    * 0.25;
            }
        }
        levels.push(next);
    }
    let mut data = Vec::new();
    for l in &levels {
        for &v in l {
            let s = (v.min(1.0).powf(1.0 / 2.2) * 255.0) as u8;
            data.extend_from_slice(&[s, s, s, 255]);
        }
    }
    let mut image = Image::new_uninit(
        Extent3d { width: N as u32, height: N as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels.len() as u32;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });
    image
}

/// Sun from the south-south-east, 50 degrees up, like the blockout renders.
pub fn spawn_sun(commands: &mut Commands, tuning: &Tuning) {
    let el = 50f32.to_radians();
    let az = 157.5f32.to_radians();
    let to_sun = b(az.sin() * el.cos(), az.cos() * el.cos(), el.sin());
    commands.spawn((
        DirectionalLight { illuminance: tuning.scene.sun_lux, shadow_maps_enabled: tuning.scene.shadows, ..default() },
        Transform::from_translation(to_sun * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 3, first_cascade_far_bound: 8.0, maximum_distance: 120.0, ..default() }
            .build(),
        Sun,
    ));
}
