//! PROTOTYPE (#34). Preset flights for Where you stand: the Quad flies a fixed line at a fixed
//! speed, so distance, delay, Doppler and wall muffling can be compared on exactly the same pass.
//! Points are relative to the Launch Spot (x forward, y left, z up), except the ones built from
//! the Map's main feature.

use crate::map::MapGeom;
use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathKind {
    FlyBy,
    OutAndBack,
    Orbit,
    /// Bando: round the tower, so it passes behind it.
    AroundTower,
    /// Bando: through the tower's ground floor.
    ThroughTower,
    /// Skate Park: down into the pool bowl and out again.
    IntoBowl,
}

pub const ALL_PATHS: [PathKind; 6] =
    [PathKind::FlyBy, PathKind::OutAndBack, PathKind::Orbit, PathKind::AroundTower, PathKind::ThroughTower, PathKind::IntoBowl];

impl PathKind {
    pub fn label(self) -> &'static str {
        match self {
            PathKind::FlyBy => "Fly-by, 8 m in front of you",
            PathKind::OutAndBack => "Out to 60 m and back",
            PathKind::Orbit => "Orbit around you at 12 m",
            PathKind::AroundTower => "Bando: round the tower (behind it)",
            PathKind::ThroughTower => "Bando: through the tower's ground floor",
            PathKind::IntoBowl => "Skate Park: down into the pool bowl",
        }
    }
    pub fn fits(self, map: crate::map::MapKind) -> bool {
        use crate::map::MapKind::*;
        match self {
            PathKind::AroundTower | PathKind::ThroughTower => map == Bando,
            PathKind::IntoBowl => map == SkatePark,
            _ => true,
        }
    }
}

/// The Bando tower's concrete frame (24 x 15 m, roof at 16.9 m; #17 blockout ground_slab and
/// floors) and the Skate Park's pool bowl (#17 blockout `pool_bowl`), Blender coordinates.
const TOWER: (Vec3, Vec3) = (Vec3::new(-12.5, -8.0, 0.0), Vec3::new(12.5, 8.0, 16.9));
const BOWL: (Vec3, Vec3) = (Vec3::new(-20.6, 0.4, -3.2), Vec3::new(-3.4, 11.6, 0.0));

pub struct Path {
    pub pts: Vec<Vec3>,
    pub cum: Vec<f32>,
    pub speed: Vec<f32>,
    pub length: f32,
}

impl Path {
    pub fn build(kind: PathKind, map: &MapGeom, cruise: f32, listener: Vec3) -> Path {
        let (cy, sy) = (map.launch_yaw.cos(), map.launch_yaw.sin());
        let l = map.launch;
        let local = |x: f32, y: f32, z: f32| l + Vec3::new(x * cy - y * sy, x * sy + y * cy, z);
        let (ctrl, speeds): (Vec<Vec3>, Vec<f32>) = match kind {
            PathKind::FlyBy => {
                (vec![local(8.0, 45.0, 3.0), local(8.0, 20.0, 3.0), local(8.0, 0.0, 3.0), local(8.0, -20.0, 3.0), local(8.0, -45.0, 3.0)], vec![1.2; 5])
            }
            PathKind::OutAndBack => (
                vec![
                    local(1.5, 0.0, 0.3),
                    local(6.0, 0.0, 3.0),
                    local(30.0, 0.0, 6.0),
                    local(60.0, 0.0, 8.0),
                    local(66.0, 6.0, 8.0),
                    local(60.0, 12.0, 8.0),
                    local(30.0, 6.0, 6.0),
                    local(6.0, 2.0, 3.0),
                    local(1.5, 0.0, 0.6),
                ],
                vec![0.2, 0.7, 1.0, 1.0, 0.7, 1.0, 1.0, 0.6, 0.15],
            ),
            PathKind::Orbit => {
                let mut v = Vec::new();
                for i in 0..=24 {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    v.push(listener + Vec3::new(12.0 * a.cos(), 12.0 * a.sin(), 3.0 - (listener.z - l.z)));
                }
                let n = v.len();
                (v, vec![0.8; n])
            }
            PathKind::AroundTower => {
                // Start in front, go round at 4 m from the walls, 2 m up, come back.
                let (lo, hi) = (TOWER.0 - Vec3::new(4.0, 4.0, 0.0), TOWER.1 + Vec3::new(4.0, 4.0, 0.0));
                let z = 2.0;
                let v = vec![
                    local(3.0, 0.0, 1.5),
                    Vec3::new(0.0, lo.y, z),
                    Vec3::new(hi.x, lo.y, z),
                    Vec3::new(hi.x, hi.y, z),
                    Vec3::new(0.0, hi.y, z),
                    Vec3::new(lo.x, hi.y, z),
                    Vec3::new(lo.x, lo.y, z),
                    Vec3::new(0.0, lo.y, z),
                    local(3.0, 0.0, 1.5),
                ];
                let n = v.len();
                (v, vec![0.6; n])
            }
            PathKind::ThroughTower => {
                let z = 1.6;
                let v = vec![local(3.0, 0.0, 1.5), Vec3::new(1.0, TOWER.0.y - 3.0, z), Vec3::new(1.0, 0.0, z), Vec3::new(1.0, TOWER.1.y + 3.0, z), Vec3::new(1.0, TOWER.1.y + 14.0, 3.0)];
                (v, vec![0.4; 5])
            }
            PathKind::IntoBowl => {
                let c = (BOWL.0 + BOWL.1) * 0.5;
                let v = vec![
                    local(2.0, 0.0, 1.5),
                    Vec3::new(BOWL.1.x - 2.0, c.y, 0.5),
                    Vec3::new(c.x + 3.0, c.y, BOWL.0.z + 1.0),
                    Vec3::new(c.x - 3.0, c.y - 2.0, BOWL.0.z + 1.0),
                    Vec3::new(BOWL.0.x + 2.0, c.y, BOWL.0.z + 1.6),
                    Vec3::new(BOWL.0.x - 3.0, c.y, 2.5),
                ];
                (v, vec![0.4, 0.4, 0.35, 0.35, 0.4, 0.5])
            }
        };
        // Catmull-Rom through the control points, sampled every ~0.1 m.
        let mut pts = Vec::new();
        let mut spd = Vec::new();
        let n = ctrl.len();
        for i in 0..n - 1 {
            let p0 = ctrl[i.saturating_sub(1)];
            let p1 = ctrl[i];
            let p2 = ctrl[i + 1];
            let p3 = ctrl[(i + 2).min(n - 1)];
            let seg = p1.distance(p2).max(0.1);
            let steps = (seg / 0.1).ceil() as usize;
            for s in 0..steps {
                let t = s as f32 / steps as f32;
                let t2 = t * t;
                let t3 = t2 * t;
                let p = 0.5
                    * ((2.0 * p1)
                        + (-p0 + p2) * t
                        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
                pts.push(p);
                spd.push((speeds[i] + (speeds[i + 1] - speeds[i]) * t) * cruise);
            }
        }
        pts.push(ctrl[n - 1]);
        spd.push(speeds[n - 1] * cruise);
        let mut cum = vec![0.0];
        for i in 1..pts.len() {
            cum.push(cum[i - 1] + pts[i].distance(pts[i - 1]));
        }
        let length = *cum.last().unwrap();
        Path { pts, cum, speed: spd, length }
    }

    /// Position and target speed at arc length `s`.
    pub fn at(&self, s: f32) -> (Vec3, f32) {
        let s = s.clamp(0.0, self.length);
        let i = match self.cum.binary_search_by(|c| c.partial_cmp(&s).unwrap()) {
            Ok(i) => i,
            Err(i) => i.max(1) - 1,
        };
        let j = (i + 1).min(self.pts.len() - 1);
        let span = (self.cum[j] - self.cum[i]).max(1e-6);
        let t = ((s - self.cum[i]) / span).clamp(0.0, 1.0);
        (self.pts[i].lerp(self.pts[j], t), self.speed[i] + (self.speed[j] - self.speed[i]) * t)
    }
}
