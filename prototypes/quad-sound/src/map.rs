//! PROTOTYPE (#34). Loads a blockout Map (.glb from #17, copied from the #28 camera prototype's
//! branch) without Bevy, for two things only: the Launch Spot (where you stand) and a read-only
//! "which surfaces does this line cross" question for wall muffling. In the game that question
//! goes to the Simulation, as the Video Signal's does (#28, ADR-0019).

use glam::{Mat4, Quat, Vec3};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum MapKind {
    #[default]
    SkatePark,
    Bando,
}

impl MapKind {
    pub fn label(self) -> &'static str {
        match self {
            MapKind::SkatePark => "Skate Park",
            MapKind::Bando => "Bando",
        }
    }
    pub fn glb(self) -> &'static str {
        match self {
            MapKind::SkatePark => "maps/skate_park_ba.glb",
            MapKind::Bando => "maps/bando_a_rooms.glb",
        }
    }
    /// The Launch Spot the #28 camera prototype used: Skate Park L1, Bando L3.
    fn launch_tag(self) -> &'static str {
        match self {
            MapKind::SkatePark => "L1",
            MapKind::Bando => "L3",
        }
    }
}

pub struct Obj {
    pub name: String,
    pub min: Vec3,
    pub max: Vec3,
    pub tris: Vec<[Vec3; 3]>,
}

pub struct MapGeom {
    pub kind: MapKind,
    pub objects: Vec<Obj>,
    /// Blender coordinates: x east, y north, z up (metres).
    pub launch: Vec3,
    /// Direction the Launch Spot faces, radians from +x (east), counter-clockwise.
    pub launch_yaw: f32,
    pub min: Vec3,
    pub max: Vec3,
    pub tri_count: usize,
    pub launch_names: Vec<String>,
}

fn to_blender(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

impl MapGeom {
    pub fn load(kind: MapKind) -> Result<Self, String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(kind.glb());
        let (doc, buffers, _) = gltf::import(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut objects = Vec::new();
        let mut launch: Option<(Vec3, f32, String)> = None;
        let mut launch_names = Vec::new();
        let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("no scene")?;
        let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let (t, r, s) = node.transform().decomposed();
            let local = Mat4::from_scale_rotation_translation(Vec3::from(s), Quat::from_array(r), Vec3::from(t));
            let world = parent * local;
            let name = node.name().unwrap_or("").to_string();
            if name.starts_with("launch_spot") {
                let pos = to_blender(world.transform_point3(Vec3::ZERO));
                let f = to_blender(world.transform_vector3(Vec3::new(0.0, 0.0, -1.0)));
                let yaw = f.y.atan2(f.x);
                launch_names.push(name.clone());
                let wanted = name.contains(kind.launch_tag());
                if launch.is_none() || wanted {
                    launch = Some((pos, yaw, name.clone()));
                }
            }
            if let Some(mesh) = node.mesh() {
                let mut tris = Vec::new();
                let mut min = Vec3::splat(f32::MAX);
                let mut max = Vec3::splat(f32::MIN);
                for prim in mesh.primitives() {
                    let reader = prim.reader(|b| Some(&buffers[b.index()]));
                    let Some(pos) = reader.read_positions() else { continue };
                    let pts: Vec<Vec3> = pos.map(|p| to_blender(world.transform_point3(Vec3::from(p)))).collect();
                    for p in &pts {
                        min = min.min(*p);
                        max = max.max(*p);
                    }
                    let idx: Vec<u32> = match reader.read_indices() {
                        Some(i) => i.into_u32().collect(),
                        None => (0..pts.len() as u32).collect(),
                    };
                    for c in idx.chunks_exact(3) {
                        tris.push([pts[c[0] as usize], pts[c[1] as usize], pts[c[2] as usize]]);
                    }
                }
                if !tris.is_empty() {
                    objects.push(Obj { name: name.clone(), min, max, tris });
                }
            }
            for c in node.children() {
                stack.push((c, world));
            }
        }
        let (launch, launch_yaw, _) = launch.unwrap_or((Vec3::ZERO, 0.0, String::new()));
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for o in &objects {
            min = min.min(o.min);
            max = max.max(o.max);
        }
        let tri_count = objects.iter().map(|o| o.tris.len()).sum();
        Ok(MapGeom { kind, objects, launch, launch_yaw, min, max, tri_count, launch_names })
    }

    /// How many separate Map parts the straight line from `a` to `b` passes through (the ground
    /// itself is skipped when the line only grazes it near either end).
    pub fn walls_crossed(&self, a: Vec3, b: Vec3) -> u32 {
        let d = b - a;
        let mut n = 0;
        for o in &self.objects {
            if !seg_aabb(a, d, o.min, o.max) {
                continue;
            }
            let hit = o.tris.iter().any(|t| match seg_tri(a, d, t) {
                Some(s) => s > 0.002 && s < 0.998,
                None => false,
            });
            if hit {
                n += 1;
            }
        }
        n
    }

    /// Walls on the line, averaged over five parallel lines spread `spread` metres sideways and
    /// up/down (as the Video Signal averages over the wave's width, ADR-0019). A column or a rail
    /// then blocks only part of the sound; a building blocks all of it.
    pub fn walls_averaged(&self, a: Vec3, b: Vec3, spread: f32) -> f32 {
        let d = (b - a).normalize_or_zero();
        let side = d.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = side.cross(d).normalize_or(Vec3::Z);
        let offs = [Vec3::ZERO, side * spread, -side * spread, up * spread, -up * spread];
        let total: u32 = offs.iter().map(|o| self.walls_crossed(a + *o, b + *o)).sum();
        total as f32 / offs.len() as f32
    }

    /// The largest part by footprint (the Bando tower, the Skate Park's bowl), for the
    /// "around/through the building" paths.
    pub fn main_feature(&self) -> Option<&Obj> {
        self.objects
            .iter()
            .filter(|o| (o.max.z - o.min.z) > 1.0)
            .max_by(|a, b| {
                let fa = (a.max.x - a.min.x) * (a.max.y - a.min.y) * (a.max.z - a.min.z).min(20.0);
                let fb = (b.max.x - b.min.x) * (b.max.y - b.min.y) * (b.max.z - b.min.z).min(20.0);
                fa.partial_cmp(&fb).unwrap()
            })
    }
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

/// Möller–Trumbore: the segment parameter (0..1) where `a + s*d` hits the triangle.
fn seg_tri(a: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = a - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let tt = e2.dot(q) * inv;
    if (0.0..=1.0).contains(&tt) { Some(tt) } else { None }
}
