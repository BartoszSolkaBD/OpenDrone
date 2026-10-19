//! What the pilot changes about the picture in flight: Camera Tilt and FOV,
//! 1° per press and repeating while held, kept for each Quad, with a brief
//! readout. And the pilot's other camera settings, the Video Look and the
//! Aspect ratio.
//!
//! None of it enters the Simulation: the camera is outside the physics, so
//! it can change mid-flight.

use std::collections::BTreeMap;
use std::time::Duration;

use bevy::prelude::Resource;
use opendrone_input::profile::Action;
use opendrone_pack::Camera;

use super::lens::Aspect;
use super::look::VideoLook;

/// Camera Tilt's range on every Quad, in degrees: from level to 80° up.
pub const TILT_RANGE: (f64, f64) = (0.0, 80.0);

/// FOV's range on every Quad, in degrees, corner to corner of the 4:3 picture.
pub const FOV_RANGE: (f64, f64) = (90.0, 170.0);

/// What one press changes, in degrees.
pub const STEP: f64 = 1.0;

/// A held key repeats after this long...
pub const REPEAT_AFTER: Duration = Duration::from_secs(1);

/// ...and then this often.
pub const REPEAT_EVERY: Duration = Duration::from_millis(200);

/// How long the readout shows after a change.
pub const READOUT_FOR: Duration = Duration::from_millis(1500);

/// The Actions that move the camera.
pub const CAMERA_ACTIONS: [Action; 4] = [
    Action::CameraTiltUp,
    Action::CameraTiltDown,
    Action::FovWider,
    Action::FovNarrower,
];

/// One Quad's Camera Tilt and FOV as the pilot has them, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadCamera {
    pub tilt: f64,
    pub fov: f64,
}

impl QuadCamera {
    /// The defaults in a Quad's definition.
    pub fn defaults(camera: &Camera) -> QuadCamera {
        QuadCamera {
            tilt: camera.camera_tilt.to_degrees(),
            fov: camera.fov.to_degrees(),
        }
    }

    /// One press of an Action. It stops at the end of the range. Returns
    /// whether it moved anything, and so whether the readout shows.
    pub fn press(&mut self, action: Action) -> bool {
        let (tilt, fov) = (self.tilt, self.fov);
        match action {
            Action::CameraTiltUp => {
                self.tilt = (self.tilt + STEP).clamp(TILT_RANGE.0, TILT_RANGE.1)
            }
            Action::CameraTiltDown => {
                self.tilt = (self.tilt - STEP).clamp(TILT_RANGE.0, TILT_RANGE.1)
            }
            Action::FovWider => self.fov = (self.fov + STEP).clamp(FOV_RANGE.0, FOV_RANGE.1),
            Action::FovNarrower => self.fov = (self.fov - STEP).clamp(FOV_RANGE.0, FOV_RANGE.1),
            _ => {}
        }
        (tilt, fov) != (self.tilt, self.fov)
    }
}

/// The pilot's camera settings. Camera Tilt and FOV are kept for each Quad,
/// by its id, and start from the Quad's definition. They live here until the
/// settings file arrives with the screens ticket; then they persist there.
#[derive(Resource, Clone, Debug, Default)]
pub struct CameraSettings {
    quads: BTreeMap<String, QuadCamera>,
    pub look: VideoLook,
    pub aspect: Aspect,
}

impl CameraSettings {
    /// This Quad's Camera Tilt and FOV: the pilot's, or its definition's.
    pub fn quad(&self, id: &str, camera: &Camera) -> QuadCamera {
        self.quads
            .get(id)
            .copied()
            .unwrap_or_else(|| QuadCamera::defaults(camera))
    }

    /// Sets this Quad's Camera Tilt and FOV, as the settings file will.
    pub fn set_quad(&mut self, id: &str, camera: QuadCamera) {
        self.quads.insert(id.to_string(), camera);
    }

    /// One press of an Action on this Quad, kept for it. Returns the new
    /// value for the readout when something moved.
    pub fn press(&mut self, id: &str, camera: &Camera, action: Action) -> Option<Readout> {
        let mut quad = self.quad(id, camera);
        let moved = quad.press(action);
        self.quads.insert(id.to_string(), quad);
        moved.then(|| Readout::of(action, quad))
    }
}

/// The brief readout after a change, as the sim's notices say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readout {
    pub text: String,
}

impl Readout {
    fn of(action: Action, quad: QuadCamera) -> Readout {
        let text = match action {
            Action::CameraTiltUp | Action::CameraTiltDown => {
                format!("Camera Tilt {}°", quad.tilt.round())
            }
            _ => format!("FOV {}°", quad.fov.round()),
        };
        Readout { text }
    }
}

/// Holds the Actions that repeat: a press acts at once, and a hold acts again
/// after [`REPEAT_AFTER`] and then every [`REPEAT_EVERY`]. It follows what is
/// held each frame, whichever Input Device holds it.
#[derive(Clone, Debug, Default)]
pub struct Repeater {
    /// For each Action held: when it next acts.
    next: BTreeMap<Action, Duration>,
}

impl Repeater {
    pub fn new() -> Repeater {
        Repeater::default()
    }

    /// Takes what is held at `now` and gives the Actions to act on, in the
    /// order they fall due. An Action not held any more starts afresh.
    pub fn update(&mut self, now: Duration, held: &[Action]) -> Vec<Action> {
        self.next.retain(|action, _| held.contains(action));
        let mut due = Vec::new();
        for &action in held {
            let next = self.next.entry(action).or_insert_with(|| {
                due.push((now, action));
                now + REPEAT_AFTER
            });
            while *next <= now {
                due.push((*next, action));
                *next += REPEAT_EVERY;
            }
        }
        due.sort_by_key(|(at, _)| *at);
        due.into_iter().map(|(_, action)| action).collect()
    }
}
