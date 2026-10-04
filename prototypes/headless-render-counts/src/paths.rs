//! PROTOTYPE (#29). Fixed camera paths, copied from the #28 camera prototype
//! (`prototype/fpv-camera-look`, `src/paths.rs`) so both prototypes fly the same lines.
//! Points are in Blender coordinates from the #17 blockout scripts (x east, y north, z up), with
//! a speed in m/s at each point.

use crate::{b, MapKind};
use bevy::prelude::*;

pub struct PathDef {
    pub name: &'static str,
    #[allow(dead_code)]
    pub map: MapKind,
    pub points: &'static [(f32, f32, f32, f32)],
}

pub const PATHS: &[PathDef] = &[
    PathDef {
        name: "Bando: up the lift shaft, across floor 2, out over the yard",
        map: MapKind::Bando,
        points: &[
            (0.0, -16.0, 0.4, 1.0),
            (0.5, -13.0, 1.4, 3.0),
            (2.0, -10.0, 1.6, 4.0),
            (3.0, -7.4, 1.6, 4.0),
            (3.6, -4.5, 1.6, 3.5),
            (4.2, -1.8, 1.7, 3.0),
            (4.0, 0.0, 1.9, 2.5),
            (1.5, 0.0, 2.4, 2.0),
            (1.5, 0.0, 5.0, 3.0),
            (1.5, 0.0, 8.4, 2.5),
            (-1.5, 0.2, 8.5, 3.0),
            (-4.0, 0.0, 8.5, 4.0),
            (-8.5, -1.0, 8.5, 4.0),
            (-9.0, -5.0, 8.4, 4.5),
            (-9.0, -9.0, 7.6, 5.0),
            (-5.0, -16.0, 5.5, 6.0),
            (2.0, -23.0, 4.5, 6.0),
            (9.0, -18.0, 5.5, 6.0),
            (8.0, -11.0, 7.0, 5.0),
            (2.0, -14.0, 4.0, 4.0),
            (0.0, -16.5, 1.2, 2.0),
        ],
    },
    PathDef {
        name: "Skate Park: pool bowl, cradle, ring, full pipe, snake run",
        map: MapKind::SkatePark,
        points: &[
            (-2.5, 6.0, 0.4, 1.0),
            (-5.0, 6.2, 0.6, 5.0),
            (-8.5, 6.8, -1.6, 7.0),
            (-13.0, 8.0, -1.0, 7.0),
            (-17.0, 6.0, -0.5, 6.5),
            (-13.5, 3.5, -1.0, 7.0),
            (-8.0, 4.0, -1.6, 7.0),
            (-3.5, 4.5, 0.9, 7.0),
            (2.5, 7.0, 0.6, 7.0),
            (7.0, 8.0, -1.0, 7.0),
            (11.5, 6.5, 0.8, 7.0),
            (15.5, 7.0, 2.15, 7.0),
            (18.5, 7.0, 2.15, 7.0),
            (21.5, 6.0, 2.0, 6.0),
            (24.0, 2.0, 1.8, 5.0),
            (23.5, 0.0, 1.8, 5.0),
            (18.5, 0.0, 1.8, 6.0),
            (13.0, 0.0, 1.6, 7.0),
            (8.0, -4.0, 0.8, 7.0),
            (5.0, -9.5, -0.5, 7.0),
            (0.0, -12.5, -0.5, 7.0),
            (-5.0, -15.5, -0.6, 7.0),
            (-10.0, -12.5, -0.6, 7.0),
            (-15.0, -9.5, -0.5, 7.0),
            (-21.0, -13.4, -0.2, 7.0),
            (-25.0, -16.0, 1.5, 6.0),
            (-22.0, -6.0, 4.0, 7.0),
            (-10.0, 1.0, 4.5, 7.0),
            (-3.0, 5.0, 1.5, 5.0),
            (-2.2, 6.0, 0.5, 2.0),
        ],
    },
];

/// A sampled path: positions every few cm with arc length, and the speed at each.
pub struct SampledPath {
    pub pts: Vec<Vec3>,
    pub s: Vec<f32>,
    pub speed: Vec<f32>,
}

impl SampledPath {
    pub fn new(def: &PathDef) -> Self {
        let ctrl: Vec<(Vec3, f32)> = def.points.iter().map(|&(x, y, z, v)| (b(x, y, z), v)).collect();
        let n = ctrl.len();
        let mut pts = Vec::new();
        let mut speed = Vec::new();
        for i in 0..n - 1 {
            let p0 = ctrl[i.saturating_sub(1)].0;
            let p1 = ctrl[i].0;
            let p2 = ctrl[i + 1].0;
            let p3 = ctrl[(i + 2).min(n - 1)].0;
            let steps = ((p2 - p1).length() / 0.05).ceil().max(2.0) as usize;
            for k in 0..steps {
                let t = k as f32 / steps as f32;
                pts.push(centripetal_cr(p0, p1, p2, p3, t));
                speed.push(ctrl[i].1 + (ctrl[i + 1].1 - ctrl[i].1) * t);
            }
        }
        pts.push(ctrl[n - 1].0);
        speed.push(ctrl[n - 1].1);
        let mut s = vec![0.0];
        for i in 1..pts.len() {
            s.push(s[i - 1] + (pts[i] - pts[i - 1]).length());
        }
        Self { pts, s, speed }
    }

    pub fn length(&self) -> f32 {
        *self.s.last().unwrap()
    }

    /// Position and speed at arc length `d`.
    pub fn at(&self, d: f32) -> (Vec3, f32) {
        let d = d.clamp(0.0, self.length());
        let i = self.s.partition_point(|&x| x < d).clamp(1, self.pts.len() - 1);
        let (s0, s1) = (self.s[i - 1], self.s[i]);
        let t = if s1 > s0 { (d - s0) / (s1 - s0) } else { 0.0 };
        (self.pts[i - 1].lerp(self.pts[i], t), self.speed[i - 1] + (self.speed[i] - self.speed[i - 1]) * t)
    }
}

fn centripetal_cr(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let tj = |a: Vec3, bb: Vec3, ti: f32| ti + (bb - a).length().max(1e-4).sqrt();
    let t0 = 0.0;
    let t1 = tj(p0, p1, t0);
    let t2 = tj(p1, p2, t1);
    let t3 = tj(p2, p3, t2);
    let t = t1 + (t2 - t1) * t;
    let a1 = p0 * ((t1 - t) / (t1 - t0)) + p1 * ((t - t0) / (t1 - t0));
    let a2 = p1 * ((t2 - t) / (t2 - t1)) + p2 * ((t - t1) / (t2 - t1));
    let a3 = p2 * ((t3 - t) / (t3 - t2)) + p3 * ((t - t2) / (t3 - t2));
    let b1 = a1 * ((t2 - t) / (t2 - t0)) + a2 * ((t - t0) / (t2 - t0));
    let b2 = a2 * ((t3 - t) / (t3 - t1)) + a3 * ((t - t1) / (t3 - t1));
    b1 * ((t2 - t) / (t2 - t1)) + b2 * ((t - t1) / (t2 - t1))
}
