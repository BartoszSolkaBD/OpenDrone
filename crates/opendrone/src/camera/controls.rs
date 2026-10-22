//! What the pilot changes about the picture in flight: Camera Tilt and FOV,
//! 1° per press and repeating while held, kept for each Quad, with a brief
//! readout. And the pilot's other camera settings, the Video Look and the
//! Aspect ratio.
//!
//! None of it enters the Simulation: the camera is outside the physics, so
//! it can change mid-flight.

use std::collections::BTreeMap;
use std::time::Duration;

use bevy::prelude::{ButtonInput, KeyCode, Resource};
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

/// The keys that act as the camera's Actions.
pub const CAMERA_KEYS: [(KeyCode, Action); 4] = [
    (KeyCode::ArrowUp, Action::CameraTiltUp),
    (KeyCode::ArrowDown, Action::CameraTiltDown),
    (KeyCode::ArrowRight, Action::FovWider),
    (KeyCode::ArrowLeft, Action::FovNarrower),
];

/// What the keyboard did to the camera's Actions in one frame: the Actions
/// whose key is down now, and the Actions whose key went down during the
/// frame. A key pressed and let go between two frames isn't down any more
/// when the frame looks, so it shows only in the second list: it still counts
/// as a press.
pub fn camera_keys(keys: &ButtonInput<KeyCode>) -> (Vec<Action>, Vec<Action>) {
    let mut held = Vec::new();
    let mut tapped = Vec::new();
    for (key, action) in CAMERA_KEYS {
        if keys.pressed(key) {
            held.push(action);
        }
        if keys.just_pressed(key) {
            tapped.push(action);
        }
    }
    (held, tapped)
}

/// What the keyboard and the Input Devices' buttons did to the camera's
/// Actions in one frame: the Actions held now, and the Actions pressed during
/// the frame (see [`camera_keys`]), each once. `held` and `tapped` are what
/// the Input Devices report ([`crate::pilot::Pilot::actions_held`] and
/// [`crate::pilot::Pilot::actions_tapped`]), so a button counts as a key does.
pub fn camera_inputs(
    keys: &ButtonInput<KeyCode>,
    mut held: Vec<Action>,
    mut tapped: Vec<Action>,
) -> (Vec<Action>, Vec<Action>) {
    let (keys_held, keys_tapped) = camera_keys(keys);
    for (list, more) in [(&mut held, keys_held), (&mut tapped, keys_tapped)] {
        for action in more {
            if !list.contains(&action) {
                list.push(action);
            }
        }
        list.retain(|action| CAMERA_ACTIONS.contains(action));
    }
    (held, tapped)
}

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
/// by its id, and start from the Quad's definition. The settings file keeps
/// them ([`crate::settings_file`]).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
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

    /// Every Quad the pilot has changed the camera of, by id, in id order.
    pub fn quads(&self) -> impl Iterator<Item = (&String, &QuadCamera)> {
        self.quads.iter()
    }

    /// Sets this Quad's Camera Tilt and FOV, as the settings file does.
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
        self.update_with_taps(now, held, &[])
    }

    /// Like [`Repeater::update`], and also takes the Actions pressed since
    /// the last call (`tapped`). A tapped Action that isn't held any more was
    /// pressed and let go within the frame: it acts once. A tapped Action
    /// that is still held was pressed afresh, so it acts at once and waits
    /// a second again, even if it was already held before the frame.
    pub fn update_with_taps(
        &mut self,
        now: Duration,
        held: &[Action],
        tapped: &[Action],
    ) -> Vec<Action> {
        self.next.retain(|action, _| held.contains(action));
        let mut due = Vec::new();
        for &action in tapped {
            if held.contains(&action) {
                self.next.remove(&action);
            } else if !due.iter().any(|&(_, due_action)| due_action == action) {
                due.push((now, action));
            }
        }
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
