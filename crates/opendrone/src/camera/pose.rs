//! Digital's late picture: a history of where the Quad was, to draw it where
//! it was 15 ms ago.
//!
//! The delay is in the picture only. The history is filled from the Quad's
//! states after each tick and read only by the drawing, so it is never in the
//! sticks, the Flight Controller or the Simulation, and a Scenario never sees
//! it.

use std::collections::VecDeque;

use bevy::math::{Quat, Vec3};

use super::look::VideoLook;

/// Digital always shows the picture this much later than Analog, in seconds
/// (#14 §4): measured gaps between analog and DJI or Walksnail are 10 to
/// 20 ms.
pub const DIGITAL_DELAY: f64 = 0.015;

/// How much more delay the edge of the Video Signal's range may add to
/// Digital's, in seconds (#28 §3).
pub const MAX_EDGE_DELAY: f64 = 0.020;

/// How long the picture is behind the Quad, in seconds, for a Video Look,
/// with the extra delay the Video Signal asks for near the edge of range
/// (Digital only, and never more than [`MAX_EDGE_DELAY`]). Analog has none:
/// its own delay is the display's, which nothing here adds to.
pub fn picture_delay(look: VideoLook, edge_delay: f64) -> f64 {
    match look {
        VideoLook::Analog => 0.0,
        VideoLook::Digital => DIGITAL_DELAY + edge_delay.clamp(0.0, MAX_EDGE_DELAY),
    }
}

/// Where the Quad is and how it is turned, drawn in Bevy's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

/// The Quad's poses after each tick, with the Simulation Time of each in
/// seconds, newest last.
#[derive(Clone, Debug, Default)]
pub struct PoseHistory {
    poses: VecDeque<(f64, Pose)>,
}

/// How long the history keeps poses, in seconds: more than the longest
/// delay, [`DIGITAL_DELAY`] plus [`MAX_EDGE_DELAY`], with room to spare.
const KEPT: f64 = 0.1;

impl PoseHistory {
    pub fn new() -> PoseHistory {
        PoseHistory::default()
    }

    /// Records the Quad's pose at a moment, and forgets what is too old to
    /// draw.
    pub fn push(&mut self, at: f64, pose: Pose) {
        self.poses.push_back((at, pose));
        while self.poses.len() > 2 && at - self.poses[1].0 > KEPT {
            self.poses.pop_front();
        }
    }

    /// Forgets everything: the Quad was put somewhere else at once (Reset), so
    /// there is no motion to draw across it and nothing of the old place to
    /// show late.
    pub fn restart(&mut self, at: f64, pose: Pose) {
        self.poses.clear();
        self.push(at, pose);
    }

    /// The pose at a moment, between the two poses either side of it. Before
    /// the oldest it is the oldest, and after the newest it is the newest.
    pub fn at(&self, moment: f64) -> Option<Pose> {
        let first = self.poses.front()?;
        let last = self.poses.back()?;
        if moment <= first.0 {
            return Some(first.1);
        }
        if moment >= last.0 {
            return Some(last.1);
        }
        let next = self.poses.partition_point(|(t, _)| *t < moment);
        let (t0, p0) = self.poses[next - 1];
        let (t1, p1) = self.poses[next];
        let along = if t1 > t0 {
            (moment - t0) / (t1 - t0)
        } else {
            0.0
        };
        Some(Pose {
            position: p0.position.lerp(p1.position, along as f32),
            rotation: p0.rotation.slerp(p1.rotation, along as f32),
        })
    }

    /// The pose the picture shows at a moment, for a Video Look: where the
    /// Quad was [`picture_delay`] earlier.
    pub fn shown(&self, moment: f64, look: VideoLook, edge_delay: f64) -> Option<Pose> {
        self.at(moment - picture_delay(look, edge_delay))
    }

    pub fn len(&self) -> usize {
        self.poses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.poses.is_empty()
    }
}
