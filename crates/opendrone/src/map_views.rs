//! A developer command that draws a Map as the game draws it, from any
//! camera, and saves each picture as a PNG (#291), so an agent can see what a
//! Map really looks like: with the game's textures, materials and lighting.
//!
//! A view is a line of `key=value` words (`name=front at=0,-5,1.5
//! toward=0,0,2 fov=100 look=digital`); [`ViewRequest`] reads one and
//! [`ViewRequest::resolve`] fills in what it leaves out from the Quad on the
//! Launch Spot. The window part, [`render_map_views`], draws each view into an
//! image of its own, as `--screenshot` does, so it works with the screen
//! locked too.
//!
//! A view is either **plain** (a flat lens, the Quad hidden, a fixed exposure,
//! the same as the developer's chase camera) or in a **Video Look**: the
//! Quad's own FPV Camera, through its fisheye Lens, Auto-exposure and Look,
//! placed so its lens is where the view says. Nothing here enters the
//! Simulation.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::visibility::NoCpuCulling;
use bevy::camera::{Exposure, RenderTarget};
use bevy::prelude::*;
use opendrone_maths::Vec3 as Vec3d;

use crate::camera::FpvCamera;
use crate::camera::controls::{CameraSettings, FOV_RANGE, QuadCamera};
use crate::camera::look::VideoLook;
use crate::camera::render::FpvDisplay;
use crate::developer::{OutputPicture, draw_into_an_image};
use crate::drawing::{QuadDrawing, WorldDrawing, point};
use crate::set_up::FlightPlan;

/// The size of a picture when `--size` doesn't say: 4:3, the shape of the
/// FPV Camera's own picture, which a Video Look fills.
pub const DEFAULT_SIZE: UVec2 = UVec2::new(1280, 960);

/// A plain view's FOV when it doesn't say, in degrees across the picture's
/// diagonal: the 110 degrees the Blender previews use.
pub const PLAIN_FOV: f64 = 110.0;

/// A plain view's FOV limits, in degrees across the picture's diagonal.
pub const PLAIN_FOV_RANGE: (f64, f64) = (20.0, 170.0);

/// How a view is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A flat lens at a fixed exposure, with the Quad hidden.
    Plain,
    /// The Quad's FPV Camera, through its Lens, in this Video Look.
    Video(VideoLook),
}

/// One view as asked for: anything left out is filled in by
/// [`ViewRequest::resolve`].
#[derive(Clone, Debug, PartialEq)]
pub struct ViewRequest {
    /// Part of the file's name. `view` when not given.
    pub name: String,
    /// The camera's position in the Map, in metres (east, north, up).
    pub at: Option<Vec3d>,
    /// A point the camera looks at, in metres.
    pub toward: Option<Vec3d>,
    /// The compass heading the camera faces, in degrees: 0 north, 90 east.
    pub heading: Option<f64>,
    /// How far the camera looks up, in degrees (down is negative).
    pub pitch: Option<f64>,
    /// The FOV in degrees across the picture's diagonal.
    pub fov: Option<f64>,
    pub kind: Kind,
}

/// A view with everything decided.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedView {
    pub name: String,
    pub kind: Kind,
    /// The camera's position in the Map, in metres (east, north, up).
    pub position: Vec3d,
    /// The way the camera looks, a unit vector in the Map's axes.
    pub direction: Vec3d,
    /// In degrees across the picture's diagonal.
    pub fov: f64,
}

const KEYS: [&str; 7] = ["name", "at", "toward", "heading", "pitch", "fov", "look"];

impl ViewRequest {
    /// A view that asks for nothing: the Quad's camera on the Launch Spot,
    /// plain.
    pub fn launch() -> ViewRequest {
        ViewRequest {
            name: "launch".to_string(),
            at: None,
            toward: None,
            heading: None,
            pitch: None,
            fov: None,
            kind: Kind::Plain,
        }
    }

    /// Reads one view from words such as
    /// `name=front at=0,-5,1.5 toward=0,0,2 fov=100 look=digital`.
    pub fn parse(text: &str) -> Result<ViewRequest, String> {
        let mut view = ViewRequest {
            name: "view".to_string(),
            ..ViewRequest::launch()
        };
        let mut seen: Vec<&str> = Vec::new();
        for word in text.split_whitespace() {
            let Some((key, value)) = word.split_once('=') else {
                return Err(format!(
                    "\"{word}\" isn't key=value; the keys are {}",
                    KEYS.join(", ")
                ));
            };
            if !KEYS.contains(&key) {
                return Err(format!(
                    "\"{key}\" isn't a view key; the keys are {}",
                    KEYS.join(", ")
                ));
            }
            if seen.contains(&key) {
                return Err(format!("\"{key}\" is given twice"));
            }
            seen.push(key);
            match key {
                "name" => view.name = file_part(value)?,
                "at" => view.at = Some(triple(key, value)?),
                "toward" => view.toward = Some(triple(key, value)?),
                "heading" => view.heading = Some(number(key, value)?),
                "pitch" => {
                    let pitch = number(key, value)?;
                    if !(-90.0..=90.0).contains(&pitch) {
                        return Err("pitch takes degrees from -90 (down) to 90 (up)".to_string());
                    }
                    view.pitch = Some(pitch);
                }
                "fov" => view.fov = Some(number(key, value)?),
                _ => {
                    view.kind = match value {
                        "plain" => Kind::Plain,
                        "analog" => Kind::Video(VideoLook::Analog),
                        "digital" => Kind::Video(VideoLook::Digital),
                        _ => return Err("look takes plain, analog or digital".to_string()),
                    }
                }
            }
        }
        if view.toward.is_some() && (view.heading.is_some() || view.pitch.is_some()) {
            return Err("toward goes alone: it doesn't go with heading or pitch".to_string());
        }
        Ok(view)
    }

    /// Reads a list of views, one a line. Blank lines and lines starting
    /// with `#` are skipped, and an error names its line.
    pub fn parse_list(text: &str) -> Result<Vec<ViewRequest>, String> {
        let mut views = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            views.push(ViewRequest::parse(line).map_err(|e| format!("line {}: {e}", index + 1))?);
        }
        Ok(views)
    }

    /// Fills in what the view leaves out, from the Quad on the Launch Spot:
    /// the camera sits where the Quad's camera sits there, looks along the
    /// Launch Spot's heading at the Quad's Camera Tilt, and has the Quad's
    /// FOV in a Video Look or [`PLAIN_FOV`] in a plain view.
    pub fn resolve(&self, plan: &FlightPlan) -> Result<ResolvedView, String> {
        let start = plan.launch_spot();
        let camera = &plan.quad.camera;
        let mount = Vec3d::new(camera.position[0], camera.position[1], camera.position[2]);
        let position = self
            .at
            .unwrap_or(start.position + start.attitude.body_to_world(mount));
        let direction = match self.toward {
            Some(target) => {
                let along = target - position;
                if along.length() < 1e-6 {
                    return Err("toward is the same point as at".to_string());
                }
                along * (1.0 / along.length())
            }
            None => {
                let heading = self.heading.unwrap_or_else(|| {
                    plan.map
                        .launch_spot
                        .as_ref()
                        .map_or(0.0, |spot| spot.heading)
                });
                let pitch = self.pitch.unwrap_or(camera.camera_tilt.to_degrees());
                let (heading, pitch) = (heading.to_radians(), pitch.to_radians());
                Vec3d::new(
                    heading.sin() * pitch.cos(),
                    heading.cos() * pitch.cos(),
                    pitch.sin(),
                )
            }
        };
        let (range, default) = match self.kind {
            Kind::Plain => (PLAIN_FOV_RANGE, PLAIN_FOV),
            Kind::Video(_) => (FOV_RANGE, camera.fov.to_degrees()),
        };
        let fov = self.fov.unwrap_or(default);
        if !(range.0..=range.1).contains(&fov) {
            return Err(format!(
                "fov takes degrees from {} to {} in this look",
                range.0, range.1
            ));
        }
        Ok(ResolvedView {
            name: self.name.clone(),
            kind: self.kind,
            position,
            direction,
            fov,
        })
    }
}

/// The file a view is saved to, in the folder: the Map's short id, the view's
/// place in the list (from 1), and its name. The number keeps two views of
/// one name apart and the files in the order they were asked for.
pub fn file_name(map_id: &str, index: usize, name: &str) -> String {
    let short = map_id.rsplit('/').next().unwrap_or(map_id);
    format!("{short}-{index:02}-{name}.png")
}

fn number(key: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{key} takes a number, not \"{value}\""))
}

fn triple(key: &str, value: &str) -> Result<Vec3d, String> {
    let parts: Vec<&str> = value.split(',').collect();
    match parts.as_slice() {
        [e, n, u] => Ok(Vec3d::new(
            number(key, e)?,
            number(key, n)?,
            number(key, u)?,
        )),
        _ => Err(format!(
            "{key} takes east,north,up in metres, such as 0,-5,1.5, not \"{value}\""
        )),
    }
}

/// A view's name as part of a file's name: letters, digits, `-` and `_`.
fn file_part(name: &str) -> Result<String, String> {
    if !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(name.to_string())
    } else {
        Err("name takes letters, digits, - and _".to_string())
    }
}

/// How far a plain view draws, in metres: the FPV Camera's own reach.
const PLAIN_FAR: f32 = 3000.0;

/// How long the first view waits for the shaders to compile, in the
/// computer's time.
const FIRST_WARM_UP: Duration = Duration::from_secs(3);

/// How long a view waits after it is placed before it is drawn: a plain
/// view only a few frames, a Video Look long enough for Auto-exposure to
/// adapt to the new picture (it brightens at 3 to 4 stops a second).
fn settle_time(kind: Kind) -> Duration {
    match kind {
        Kind::Plain => Duration::from_millis(300),
        Kind::Video(_) => Duration::from_millis(3000),
    }
}

/// How long nothing may happen before the run gives up.
const STALLED_AFTER: Duration = Duration::from_secs(60);

/// The camera of the plain views.
#[derive(Component)]
struct PlainCamera;

#[derive(Resource)]
struct Rendering {
    views: Vec<ResolvedView>,
    folder: PathBuf,
    map_id: String,
    /// The view being drawn.
    next: usize,
    /// When the view was first placed, in the computer's time.
    placed: Option<Duration>,
    frames: u32,
    capturing: bool,
    /// The last time a view was placed or saved.
    progress: Duration,
}

/// A developer command: draws each view into an image of `size` and saves it
/// as a PNG in `folder`, named by [`file_name`], then quits.
pub fn render_map_views(
    app: &mut App,
    views: Vec<ResolvedView>,
    folder: PathBuf,
    map_id: &str,
    size: UVec2,
) {
    draw_into_an_image(app, size);
    app.insert_resource(Rendering {
        views,
        folder,
        map_id: map_id.to_string(),
        next: 0,
        placed: None,
        frames: 0,
        capturing: false,
        progress: Duration::ZERO,
    })
    .add_systems(
        PostUpdate,
        place_the_view.before(TransformSystems::Propagate),
    )
    .add_systems(Update, take_the_view);
}

/// The vertical angle, in radians, of a rectilinear lens whose diagonal sees
/// `fov` degrees, for a picture `width` by `height` pixels.
fn vertical_fov(fov: f64, width: f64, height: f64) -> f32 {
    let across = (fov.to_radians() / 2.0).tan();
    let half = across / (1.0 + (width / height).powi(2)).sqrt();
    (2.0 * half.atan()) as f32
}

/// Every camera the views switch: the FPV Camera, its display and the plain
/// views' camera.
type Cameras = (
    &'static mut Camera,
    Option<&'static FpvCamera>,
    Option<&'static FpvDisplay>,
    Option<&'static PlainCamera>,
);
type PlainParts = (&'static mut Transform, &'static mut Projection);
type PlainOnly = (With<PlainCamera>, Without<QuadDrawing>, Without<FpvCamera>);
type QuadParts = (&'static mut Transform, &'static mut Visibility);
type QuadOnly = (With<QuadDrawing>, Without<PlainCamera>, Without<FpvCamera>);
type MountOnly = (With<FpvCamera>, Without<QuadDrawing>, Without<PlainCamera>);

/// Every frame, until the view is saved: puts the cameras where the view
/// says, over whatever the game's own systems did this frame.
///
/// A plain view uses a camera of its own and hides the Quad, with the FPV
/// Camera off. A Video Look view runs the FPV Camera as the game does, with
/// the Quad moved so that the camera's lens lands where the view says.
#[allow(clippy::too_many_arguments)]
fn place_the_view(
    run: Res<Rendering>,
    picture: Res<OutputPicture>,
    plan: Res<FlightPlan>,
    mut settings: ResMut<CameraSettings>,
    mut cameras: Query<Cameras>,
    mut plain: Query<PlainParts, PlainOnly>,
    mut quads: Query<QuadParts, QuadOnly>,
    mounted: Query<&Transform, MountOnly>,
    mut commands: Commands,
) {
    let Some(view) = run.views.get(run.next) else {
        return;
    };
    let Some(image) = picture.image.clone() else {
        return;
    };
    if plain.is_empty() {
        commands.spawn((
            PlainCamera,
            Camera3d::default(),
            Camera {
                order: 2,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(image.into()),
            NoCpuCulling,
            Exposure::SUNLIGHT,
            Transform::IDENTITY,
        ));
        return;
    }
    let looking = point(view.direction);
    let desired = Transform::from_translation(point(view.position)).looking_to(looking, Vec3::Y);
    let is_plain = view.kind == Kind::Plain;
    for (mut camera, fpv, display, plain_camera) in &mut cameras {
        if plain_camera.is_some() {
            camera.is_active = is_plain;
        } else if display.is_some() {
            camera.is_active = !is_plain;
        } else if fpv.is_some() && is_plain {
            // The game turns this one back on itself, each frame.
            camera.is_active = false;
        }
    }
    for (mut transform, mut visibility) in &mut quads {
        *visibility = if is_plain {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if let (Kind::Video(_), Ok(mount)) = (view.kind, mounted.single()) {
            let quad =
                Mat4::from(desired.compute_affine()) * Mat4::from(mount.compute_affine()).inverse();
            *transform = Transform::from_matrix(quad);
        }
    }
    match view.kind {
        Kind::Plain => {
            if let Ok((mut transform, mut projection)) = plain.single_mut() {
                *transform = desired;
                *projection = Projection::Perspective(PerspectiveProjection {
                    fov: vertical_fov(
                        view.fov,
                        f64::from(picture.size.x),
                        f64::from(picture.size.y),
                    ),
                    near: 0.01,
                    far: PLAIN_FAR,
                    ..default()
                });
            }
        }
        Kind::Video(look) => {
            let own = settings.quad(&plan.quad.id, &plan.quad.camera);
            let wanted = QuadCamera {
                tilt: own.tilt,
                fov: view.fov,
            };
            if settings.look != look {
                settings.look = look;
            }
            if own != wanted {
                settings.set_quad(&plan.quad.id, wanted);
            }
        }
    }
}

/// Counts frames while the view settles, then saves the picture as a PNG and
/// moves on to the next view, and quits after the last.
fn take_the_view(
    mut run: ResMut<Rendering>,
    picture: Res<OutputPicture>,
    world: Res<WorldDrawing>,
    real: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    let now = real.elapsed();
    if now.saturating_sub(run.progress) > STALLED_AFTER {
        eprintln!("the view wasn't drawn in a minute, so the run stops");
        exit.write(AppExit::error());
        return;
    }
    let Some(view) = run.views.get(run.next).cloned() else {
        return;
    };
    let Some(image) = picture.image.clone() else {
        return;
    };
    if !world.is_drawn() || run.capturing {
        return;
    }
    if run.placed.is_none() {
        run.placed = Some(now);
        run.progress = now;
    }
    let placed = run.placed.unwrap_or(now);
    run.frames += 1;
    let wait = settle_time(view.kind)
        + if run.next == 0 {
            FIRST_WARM_UP
        } else {
            Duration::ZERO
        };
    if now.saturating_sub(placed) < wait || run.frames < 5 {
        return;
    }
    run.capturing = true;
    let path = run
        .folder
        .join(file_name(&run.map_id, run.next + 1, &view.name));
    commands.spawn(Screenshot::image(image)).observe(
        move |captured: On<ScreenshotCaptured>,
              mut run: ResMut<Rendering>,
              real: Res<Time<Real>>,
              mut exit: MessageWriter<AppExit>| {
            match save_png(&captured.image, &path) {
                Ok(()) => {
                    println!("{}", path.display());
                    run.next += 1;
                    run.placed = None;
                    run.frames = 0;
                    run.capturing = false;
                    run.progress = real.elapsed();
                    if run.next >= run.views.len() {
                        exit.write(AppExit::Success);
                    }
                }
                Err(problem) => {
                    eprintln!(
                        "the picture can't be saved to {}: {problem}",
                        path.display()
                    );
                    exit.write(AppExit::error());
                }
            }
        },
    );
}

fn save_png(image: &Image, path: &Path) -> Result<(), String> {
    image
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .to_rgb8()
        .save(path)
        .map_err(|e| e.to_string())
}
