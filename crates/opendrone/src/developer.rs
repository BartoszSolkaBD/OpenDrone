//! Developer commands that draw the picture into an image of their own
//! instead of the window, so they work on a machine nobody is watching, with
//! the screen locked too: one saves a picture, one measures what the FPV
//! Camera costs a frame.

use std::time::Duration;

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::camera::Output;
use crate::camera::controls::CameraSettings;
use crate::camera::look::VideoLook;
use crate::camera::render::{CameraCost, CostLink, CostProbe, FpvDisplay};
use crate::drawing::ChaseCamera;

/// The cameras whose picture is the pilot's: the display camera, or in the
/// chase view the chase camera.
type PictureCameras = Or<(With<FpvDisplay>, With<ChaseCamera>)>;

/// The image the picture is drawn into, in place of the window.
#[derive(Resource)]
struct OutputPicture {
    size: UVec2,
    image: Option<Handle<Image>>,
}

/// Sends the picture to an image of `size` instead of the window.
fn draw_into_an_image(app: &mut App, size: UVec2) {
    app.insert_resource(OutputPicture { size, image: None })
        .add_systems(PostStartup, draw_into_a_picture);
}

fn draw_into_a_picture(
    mut picture: ResMut<OutputPicture>,
    mut images: ResMut<Assets<Image>>,
    mut output: ResMut<Output>,
    cameras: Query<Entity, PictureCameras>,
    mut commands: Commands,
) {
    let image = images.add(Image::new_target_texture(
        picture.size.x,
        picture.size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(RenderTarget::Image(image.clone().into()));
    }
    output.size = picture.size;
    output.fixed = true;
    picture.image = Some(image);
}

/// A developer command: saves the camera's picture to a BMP file once the
/// game has run for `seconds`, then quits. Agents use it to look at the
/// picture on a machine they can't watch. The camera draws into a picture
/// of its own, of `size`, instead of the window.
pub fn screenshot_after(app: &mut App, seconds: f64, path: std::path::PathBuf, size: UVec2) {
    draw_into_an_image(app, size);
    app.insert_resource(ScreenshotAt {
        after: Duration::from_secs_f64(seconds),
        path,
        taken: false,
    })
    .add_systems(Update, take_the_screenshot);
}

#[derive(Resource)]
struct ScreenshotAt {
    after: Duration,
    path: std::path::PathBuf,
    taken: bool,
}

fn take_the_screenshot(
    mut at: ResMut<ScreenshotAt>,
    picture: Res<OutputPicture>,
    real: Res<Time<Real>>,
    mut commands: Commands,
) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    if at.taken || real.elapsed() < at.after {
        return;
    }
    let Some(image) = picture.image.clone() else {
        return;
    };
    at.taken = true;
    let path = at.path.clone();
    commands.spawn(Screenshot::image(image)).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            let saved = captured
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|e| e.to_string())
                .and_then(|picture| {
                    let rgb = picture.to_rgb8();
                    write_bmp(&path, rgb.width(), rgb.height(), rgb.as_raw())
                        .map_err(|e| e.to_string())
                });
            match saved {
                Ok(()) => {
                    info!("saved the picture to {}", path.display());
                    exit.write(AppExit::Success);
                }
                Err(problem) => {
                    error!("the picture can't be saved: {problem}");
                    exit.write(AppExit::error());
                }
            }
        },
    );
}

/// Writes 8-bit RGB rows, top first, as an uncompressed 24-bit BMP file.
fn write_bmp(path: &std::path::Path, width: u32, height: u32, rgb: &[u8]) -> std::io::Result<()> {
    let row = (width as usize * 3).div_ceil(4) * 4;
    let size = 54 + row * height as usize;
    let mut file = Vec::with_capacity(size);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&(size as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&(width as i32).to_le_bytes());
    file.extend_from_slice(&(height as i32).to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&24u16.to_le_bytes());
    file.extend_from_slice(&[0; 24]);
    for y in (0..height as usize).rev() {
        let start = y * width as usize * 3;
        for pixel in rgb[start..start + width as usize * 3].chunks(3) {
            file.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        file.resize(file.len() + row - width as usize * 3, 0);
    }
    std::fs::write(path, file)
}

/// How long each Look runs before its frames count, for the shaders to
/// compile and the GPU to settle.
const WARM_UP: Duration = Duration::from_secs(2);

/// A developer command: draws the picture into an image of `size`, once for
/// each Video Look, for `seconds` each, and reports what the camera's own
/// steps cost a frame on the GPU, where the GPU has a timer. Then quits.
///
/// Only the camera's own steps count: the mip chain of the flat render and
/// the merged pass. Drawing the Map wider than the picture counts as drawing
/// the Map.
pub fn measure_camera_cost(app: &mut App, seconds: f64, size: UVec2) {
    draw_into_an_image(app, size);
    // Render as fast as possible, window focused or not: a GPU with little to
    // do runs slowly to save power, and the passes would look slower than
    // they are when the game is busy.
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::Continuous,
    });
    app.insert_resource(CostProbe(true))
        .insert_resource(CostRun {
            seconds: Duration::from_secs_f64(seconds),
            size,
            looks: vec![VideoLook::Analog, VideoLook::Digital],
            phase_began: Duration::ZERO,
            counted_from: None,
            results: Vec::new(),
        })
        .add_systems(Update, run_the_cost_measurement);
}

#[derive(Resource)]
struct CostRun {
    seconds: Duration,
    size: UVec2,
    /// The Looks still to measure, the one being measured first.
    looks: Vec<VideoLook>,
    /// When this Look began, in the computer's time.
    phase_began: Duration,
    /// How many frames had been timed when this Look's warm-up ended.
    counted_from: Option<usize>,
    results: Vec<(VideoLook, Vec<CameraCost>)>,
}

fn run_the_cost_measurement(
    mut run: ResMut<CostRun>,
    link: Res<CostLink>,
    real: Res<Time<Real>>,
    mut settings: ResMut<CameraSettings>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(&look) = run.looks.first() else {
        return;
    };
    settings.look = look;
    let samples = link.0.lock().expect("the cost link");
    if samples.supported == Some(false) {
        eprintln!("this GPU can't take timestamps, so the camera's cost can't be measured here");
        exit.write(AppExit::error());
        return;
    }
    let into = real.elapsed().saturating_sub(run.phase_began);
    if run.counted_from.is_none() && into >= WARM_UP {
        run.counted_from = Some(samples.frames.len());
    }
    if into < WARM_UP + run.seconds {
        return;
    }
    let from = run.counted_from.unwrap_or(0).min(samples.frames.len());
    let frames = samples.frames[from..].to_vec();
    drop(samples);
    run.results.push((look, frames));
    run.looks.remove(0);
    run.phase_began = real.elapsed();
    run.counted_from = None;
    if run.looks.is_empty() {
        report_the_cost(&run);
        exit.write(AppExit::Success);
    }
}

fn report_the_cost(run: &CostRun) {
    println!(
        "The FPV Camera's own steps (the flat render's mip chain and the merged pass), \
         on the GPU, with the picture drawn into {} × {} pixels, {} s of frames for each Look:",
        run.size.x,
        run.size.y,
        run.seconds.as_secs_f64()
    );
    for (look, frames) in &run.results {
        if frames.is_empty() {
            println!("  {look:?}: no frames were timed");
            continue;
        }
        let spread = |value: fn(&CameraCost) -> f32| {
            let mut all: Vec<f32> = frames.iter().map(value).collect();
            all.sort_by(f32::total_cmp);
            (
                all[all.len() / 2],
                all[(all.len() * 95 / 100).min(all.len() - 1)],
            )
        };
        let (steps, steps_p95) = spread(CameraCost::camera_steps);
        let (mips, _) = spread(|c| c.mips);
        let (look_pass, _) = spread(|c| c.look);
        println!(
            "  {look:?}: median {steps:.2} ms a frame (95th percentile {steps_p95:.2} ms): \
             mip chain {mips:.2} ms, merged pass {look_pass:.2} ms, over {} frames",
            frames.len()
        );
    }
    println!("The cap is 2 ms a frame at native 1440p on the Mac mini M4.");
}
