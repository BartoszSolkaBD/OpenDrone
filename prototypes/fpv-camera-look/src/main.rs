#![allow(dead_code)]
//! PROTOTYPE for issue #28: "Does the FPV camera look right and fit 2 ms on the M4?"
//! Throwaway. Not production code. Run it with ./run.sh (usage at the top of run.sh).
//!
//! Bevy 0.19.1 (the current stable release; 0.20 is still a release candidate).

mod bench;
mod input_sdl;
mod map;
mod paths;
mod quad_model;
mod render;
mod rig;
mod settings;
mod signal;
mod ui;

use bevy::{
    camera::{Exposure, RenderTarget},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    diagnostic::FrameTimeDiagnosticsPlugin,
    image::ImageSampler,
    light::GlobalAmbientLight,
    post_process::auto_exposure::{AutoExposure, AutoExposurePlugin},
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        renderer::RenderDevice,
        texture::ManualTextureViews,
        view::Msaa,
    },
    window::{PresentMode, PrimaryWindow, WindowMode, WindowResolution},
};
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use render::{FpvDisplay, FpvFrame, FpvParams, SourceTexture};
use rig::{CameraRig, FpvCam3d, PoseHistory, Rig};
use settings::{Aspect, Look, MapKind, Tuning};
use signal::VideoSignal;
use std::collections::VecDeque;

#[derive(Resource, Clone, Debug)]
pub struct LaunchArgs {
    pub map: MapKind,
    pub pipelined: bool,
    pub windowed: bool,
    pub bench: bool,
    pub latency_test: bool,
    pub screenshots: bool,
    pub debug_shots: bool,
    /// Render the final picture into an offscreen image instead of the window (works with the screen locked).
    pub offscreen: bool,
    /// Frames allowed on the GPU at once (1-3; Bevy's default is 2). Fewer = less latency.
    pub frames_in_flight: usize,
    pub load: Option<String>,
}

/// Benchmark-only: draw the Map straight to the window with a flat lens, no camera effects.
#[derive(Resource, Default)]
pub struct Bypass(pub bool);

/// Where the final picture goes, and its size in pixels.
#[derive(Resource, Default)]
pub struct Output {
    pub size: UVec2,
    pub image: Option<Handle<Image>>,
}

pub fn screenshot_of(out: &Output) -> bevy::render::view::screenshot::Screenshot {
    match &out.image {
        Some(h) => bevy::render::view::screenshot::Screenshot::image(h.clone()),
        None => bevy::render::view::screenshot::Screenshot::primary_window(),
    }
}

/// 0 = normal, 1 = show the warp's source coordinates, 2 = show the flat render unwarped.
#[derive(Resource, Default)]
pub struct DebugView(pub f32);

/// Benchmark cost probe: draw the merged pass this many times per frame.
#[derive(Resource)]
pub struct CostProbe(pub u32);

impl Default for CostProbe {
    fn default() -> Self {
        Self(1)
    }
}

#[derive(Resource, Default)]
pub struct MapRequest(pub Option<MapKind>);

#[derive(Resource, Default)]
pub struct Measurements {
    pub gpu: VecDeque<render::FrameGpu>,
    pub cpu_ms: VecDeque<f32>,
    pub latency: Vec<render::LatencySample>,
    pub ts_supported: Option<bool>,
    /// Shaders still compiling, and whether they've been quiet for a second.
    pub pipelines_waiting: usize,
    pub pipelines_settled: bool,
    pub frame: u64,
    pub source_size: UVec2,
    pub mip_levels: u32,
}

#[derive(Resource, Default)]
pub struct MeterMask {
    pub key: (i32, i32, i32, bool),
    pub handle: Option<Handle<Image>>,
}

#[derive(Resource, Default)]
pub struct Flash(pub Option<(String, f64)>);

pub fn now_stamp() -> String {
    let out = std::process::Command::new("date").arg("+%Y%m%d-%H%M%S").output();
    out.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|| "now".into())
}

fn parse_args() -> LaunchArgs {
    let mut a = LaunchArgs {
        map: MapKind::Bando,
        pipelined: false,
        windowed: false,
        bench: false,
        latency_test: false,
        screenshots: false,
        debug_shots: false,
        offscreen: false,
        frames_in_flight: 2,
        load: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--skate" | "--skate-park" => a.map = MapKind::SkatePark,
            "--bando" => a.map = MapKind::Bando,
            "--pipelined" => a.pipelined = true,
            "--windowed" => a.windowed = true,
            "--bench" => a.bench = true,
            "--latency" => a.latency_test = true,
            "--screenshots" => a.screenshots = true,
            "--debug-shots" => a.debug_shots = true,
            "--offscreen" => a.offscreen = true,
            "--frames-in-flight" => {
                i += 1;
                a.frames_in_flight = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(2).clamp(1, 3);
            }
            "--load" => {
                i += 1;
                a.load = args.get(i).cloned();
            }
            other => eprintln!("unknown argument {other}"),
        }
        i += 1;
    }
    if a.screenshots || a.debug_shots {
        a.offscreen = true;
    }
    a
}

fn main() {
    let args = parse_args();
    render::MAX_IN_FLIGHT.store(args.frames_in_flight, std::sync::atomic::Ordering::Relaxed);
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let mut tuning = Tuning::default();
    if let Some(l) = &args.load {
        match settings::load_tuning(std::path::Path::new(l)) {
            Ok(t) => tuning = t,
            Err(e) => eprintln!("could not load tuning {l}: {e}"),
        }
    }

    let window = Window {
        title: "OpenDrone PROTOTYPE #28: FPV camera look".into(),
        resolution: WindowResolution::new(2560, 1440).with_scale_factor_override(1.0),
        present_mode: PresentMode::AutoNoVsync,
        desired_maximum_frame_latency: std::num::NonZero::new(args.frames_in_flight as u32),
        mode: if args.windowed { WindowMode::Windowed } else { WindowMode::BorderlessFullscreen(MonitorSelection::Primary) },
        ..default()
    };
    let mut plugins = DefaultPlugins
        .set(if args.offscreen {
            // No window at all: nothing waits for the screen (works with the screen locked).
            WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() }
        } else {
            WindowPlugin { primary_window: Some(window), ..default() }
        })
        .set(AssetPlugin { file_path: repo_root.to_string_lossy().into_owned(), ..default() });
    if !args.pipelined {
        plugins = plugins.disable::<PipelinedRenderingPlugin>();
    }

    let mut app = App::new();
    // Render every frame as fast as possible, even when the window isn't focused (or doesn't exist).
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::Continuous,
    });
    app.add_plugins(plugins)
        .add_plugins((
            AutoExposurePlugin,
            FrameTimeDiagnosticsPlugin::default(),
            EguiPlugin::default(),
            render::FpvRenderPlugin,
        ))
        .insert_resource(ClearColor(Color::LinearRgba(LinearRgba::rgb(0.14, 0.22, 0.42))))
        .insert_resource(tuning)
        .insert_resource(args.clone())
        .insert_resource(input_sdl::start())
        .insert_resource(SourceTexture { texture: None, views: None, size: UVec2::ZERO, mips_enabled: true })
        .init_resource::<Rig>()
        .init_resource::<PoseHistory>()
        .init_resource::<VideoSignal>()
        .init_resource::<map::MapState>()
        .init_resource::<Measurements>()
        .init_resource::<MeterMask>()
        .init_resource::<Bypass>()
        .init_resource::<MapRequest>()
        .init_resource::<DebugView>()
        .init_resource::<CostProbe>()
        .init_resource::<Output>()
        .init_resource::<Flash>()
        .init_resource::<ui::EguiBusy>()
        .init_resource::<ui::UiState>()
        .init_resource::<bench::Script>()
        .init_resource::<bench::SyntheticSteps>()
        .add_observer(map::on_map_ready)
        .add_systems(Startup, (setup, quad_model::spawn_quads, bench::start_script).chain())
        .add_systems(
            Update,
            (
                update_output,
                ui::shortcuts,
                handle_map_request,
                map::build_map,
                bench::run_script,
                bench::synthetic_steps,
                rig::step_rig,
                signal::update_signal,
                rig::apply_pose,
                apply_tuning,
                quad_model::show_quad,
                update_frame,
                collect_measurements,
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, ui::panel);
    app.run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<map::MapState>,
    mut egui_settings: ResMut<EguiGlobalSettings>,
    args: Res<LaunchArgs>,
    tuning: Res<Tuning>,
    mut rig: ResMut<Rig>,
    device: Res<RenderDevice>,
    mut src: ResMut<SourceTexture>,
    mut manual: ResMut<ManualTextureViews>,
    mut images: ResMut<Assets<Image>>,
    mut output: ResMut<Output>,
) {
    egui_settings.auto_create_primary_context = false;
    // The 3D camera's target must exist before its first frame.
    let (pic, _) = picture_rect(UVec2::new(2560, 1440), tuning.aspect);
    render::ensure_source(&device, &mut src, &mut manual, UVec2::new(pic.z as u32, pic.w as u32), true);

    // The pilot's view: a body (delayed for Digital) carrying the FPV Camera and the Quad model.
    commands.spawn((CameraRig, Transform::default(), Visibility::default())).with_children(|c| {
        c.spawn((
            Camera3d::default(),
            Camera { order: 0, ..default() },
            RenderTarget::TextureView(render::SOURCE_VIEW),
            Projection::Perspective(PerspectiveProjection { fov: 1.5, near: 0.004, far: 3000.0, ..default() }),
            Tonemapping::TonyMcMapface,
            Exposure { ev100: 15.0 },
            Msaa::Sample4,
            FpvCam3d,
            Transform::default(),
        ));
    });

    // The display camera: runs the merged pass into the window, and carries the tuning panel.
    let target = if args.offscreen {
        let img = Image::new_target_texture(2560, 1440, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
        let h = images.add(img);
        output.image = Some(h.clone());
        output.size = UVec2::new(2560, 1440);
        RenderTarget::Image(h.into())
    } else {
        RenderTarget::Window(bevy::window::WindowRef::Primary)
    };
    commands.spawn((
        target,
        Camera2d,
        Camera { order: 1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        FpvDisplay,
        PrimaryEguiContext,
    ));

    map::spawn_sun(&mut commands, &tuning);
    map::spawn_map(&mut commands, &asset_server, &mut state, args.map);
    rig.control = rig::Control::Path;
    rig.path = None;
}

fn update_output(mut output: ResMut<Output>, windows: Query<&Window, With<PrimaryWindow>>) {
    if output.image.is_some() {
        return;
    }
    if let Ok(win) = windows.single() {
        let size = UVec2::new(win.physical_width(), win.physical_height());
        if output.size != size {
            output.size = size;
        }
    }
}

fn handle_map_request(
    mut req: ResMut<MapRequest>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<map::MapState>,
    mut rig: ResMut<Rig>,
) {
    let Some(kind) = req.0.take() else { return };
    if state.current == Some(kind) {
        return;
    }
    map::spawn_map(&mut commands, &asset_server, &mut state, kind);
    rig.path = None;
    if rig.control != rig::Control::Path {
        rig::reset_to_launch(&mut rig, &state);
    }
}

/// Picture rectangle in the window (x0, y0, w, h) and the 4:3 frame's half-diagonal R.
pub fn picture_rect(win: UVec2, aspect: Aspect) -> (Vec4, f32) {
    let (w, h) = (win.x as f32, win.y as f32);
    let ar = match aspect {
        Aspect::FourThree => 4.0 / 3.0,
        Aspect::SixteenNine => 16.0 / 9.0,
    };
    let pw = w.min(h * ar).floor();
    let ph = (pw / ar).floor();
    let r = 0.625 * pw;
    (Vec4::new(((w - pw) * 0.5).floor(), ((h - ph) * 0.5).floor(), pw, ph), r)
}

/// The Lens: theta_max, k, g(theta_max) for a FOV (corner to corner of 4:3) and curve setting.
pub fn lens(fov_deg: f32, curve: f32) -> (f32, f32, f32) {
    let theta_max = (fov_deg.clamp(60.0, 175.0) * 0.5).to_radians();
    let k = curve.clamp(0.0, (std::f32::consts::FRAC_PI_2 - 0.05) / theta_max);
    let g = if k > 1e-4 { (k * theta_max).tan() / k } else { theta_max };
    (theta_max, k, g)
}

fn theta_at(r: f32, theta_max: f32, k: f32, g: f32) -> f32 {
    if k > 1e-4 { (r * g * k).atan() / k } else { r * theta_max }
}

/// Source render: tan half-extents needed to cover the picture's corners.
pub fn source_extent(pic: Vec4, r_frame: f32, theta_max: f32, k: f32, g: f32) -> Vec2 {
    let c = Vec2::new(pic.z * 0.5, pic.w * 0.5) / r_frame;
    let rc = c.length();
    let th = theta_at(rc, theta_max, k, g).min(1.53);
    let t = th.tan();
    c / rc * t
}

#[allow(clippy::too_many_arguments)]
fn apply_tuning(
    tuning: Res<Tuning>,
    output: Res<Output>,
    device: Option<Res<RenderDevice>>,
    mut src: ResMut<SourceTexture>,
    mut manual: ResMut<ManualTextureViews>,
    mut cams: Query<(Entity, &mut Projection, &mut Msaa, Option<&mut AutoExposure>, &mut RenderTarget), With<FpvCam3d>>,
    mut display: Query<&mut Camera, (With<FpvDisplay>, Without<FpvCam3d>)>,
    mut suns: Query<&mut DirectionalLight, With<map::Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut mask: ResMut<MeterMask>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    bypass: Res<Bypass>,
    mut meas: ResMut<Measurements>,
) {
    if output.size.x == 0 {
        return;
    }
    let (pic, r_frame) = picture_rect(output.size, tuning.aspect);
    let (theta_max, k, g) = lens(tuning.fov_deg, tuning.lens_curve());
    let ext = source_extent(pic, r_frame, theta_max, k, g);
    let scale = tuning.source_scale().clamp(0.25, 3.0);
    let size = UVec2::new(((pic.z * scale).round() as u32).max(16), ((pic.w * scale).round() as u32).max(16));
    if let Some(device) = device {
        render::ensure_source(&device, &mut src, &mut manual, size, std::env::var("FPV_NO_MIPS").is_err());
    }
    meas.source_size = src.size;
    meas.mip_levels = src.views.as_ref().map(|v| v.mips.len() as u32).unwrap_or(0);

    // Metering mask: weigh the flat render by how much of the picture each part becomes.
    let key = ((tuning.fov_deg * 2.0) as i32, (k * 1000.0) as i32, tuning.aspect as i32, true);
    if mask.key != key || mask.handle.is_none() {
        mask.key = key;
        let img = metering_mask(pic, r_frame, ext, theta_max, k, g);
        mask.handle = Some(images.add(img));
    }

    for (e, mut proj, mut msaa, ae, mut target) in &mut cams {
        if let Projection::Perspective(p) = proj.as_mut() {
            let fov = if bypass.0 { 2.0 * ((110f32.to_radians() * 0.5).tan() * 9.0 / 16.0).atan() } else { 2.0 * ext.y.atan() };
            if (p.fov - fov).abs() > 1e-5 {
                p.fov = fov;
            }
            if p.near != 0.004 {
                p.near = 0.004;
            }
        }
        let want_target = match (bypass.0, &output.image) {
            (true, Some(img)) => RenderTarget::Image(img.clone().into()),
            (true, None) => RenderTarget::Window(bevy::window::WindowRef::Primary),
            (false, _) => RenderTarget::TextureView(render::SOURCE_VIEW),
        };
        if !matches!(
            (&*target, &want_target),
            (RenderTarget::Window(_), RenderTarget::Window(_))
                | (RenderTarget::TextureView(_), RenderTarget::TextureView(_))
                | (RenderTarget::Image(_), RenderTarget::Image(_))
        ) {
            *target = want_target;
        }
        let want_msaa = if tuning.scene.msaa { Msaa::Sample4 } else { Msaa::Off };
        if *msaa != want_msaa {
            *msaa = want_msaa;
        }
        let ex = tuning.exposure();
        if ex.enabled {
            let want = AutoExposure {
                range: -ex.max_darken_ev..=ex.max_brighten_ev,
                filter: 0.10..=0.90,
                speed_brighten: ex.speed_brighten,
                speed_darken: ex.speed_darken,
                exponential_transition_distance: 1.5,
                metering_mask: mask.handle.clone().unwrap_or_default(),
                ..default()
            };
            match ae {
                Some(mut cur) => {
                    if cur.range != want.range
                        || cur.speed_brighten != want.speed_brighten
                        || cur.speed_darken != want.speed_darken
                        || cur.metering_mask != want.metering_mask
                    {
                        *cur = want;
                    }
                }
                None => {
                    commands.entity(e).insert(want);
                }
            }
        } else if ae.is_some() {
            commands.entity(e).remove::<AutoExposure>();
        }
    }
    for mut cam in &mut display {
        let want = !bypass.0;
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
    for mut sun in &mut suns {
        if sun.illuminance != tuning.scene.sun_lux {
            sun.illuminance = tuning.scene.sun_lux;
        }
        if sun.shadow_maps_enabled != tuning.scene.shadows {
            sun.shadow_maps_enabled = tuning.scene.shadows;
        }
    }
    if ambient.brightness != tuning.scene.ambient_nits {
        ambient.brightness = tuning.scene.ambient_nits;
    }
}

fn metering_mask(pic: Vec4, r_frame: f32, ext: Vec2, _theta_max: f32, k: f32, g: f32) -> Image {
    let w = 128u32;
    let h = ((w as f32 * ext.y / ext.x).round() as u32).max(8);
    let c = Vec2::new(pic.z * 0.5, pic.w * 0.5) / r_frame;
    let gfun = |th: f32| if k > 1e-4 { (k * th).tan() / k } else { th };
    let mut vals = vec![0.0f32; (w * h) as usize];
    let mut maxv = 0.0f32;
    for y in 0..h {
        for x in 0..w {
            let s = Vec2::new(((x as f32 + 0.5) / w as f32 - 0.5) * 2.0 * ext.x, ((y as f32 + 0.5) / h as f32 - 0.5) * 2.0 * ext.y);
            let sl = s.length().max(1e-5);
            let th = sl.atan();
            let r = gfun(th) / g;
            let q = s / sl * r;
            if q.x.abs() > c.x || q.y.abs() > c.y {
                continue;
            }
            // Picture area per flat-render area: radial (dr/dtheta * dtheta/ds) x tangential (r / s).
            let dg = if k > 1e-4 { 1.0 / (k * th).cos().powi(2) } else { 1.0 };
            let radial = dg / g * th.cos().powi(2);
            let tangential = r / sl;
            let centre = 1.0 - 0.4 * r.min(1.0).powi(2);
            let v = radial * tangential * centre;
            vals[(y * w + x) as usize] = v;
            maxv = maxv.max(v);
        }
    }
    let data: Vec<u8> = vals.iter().map(|v| ((v / maxv.max(1e-9)).clamp(0.0, 1.0) * 255.0) as u8).collect();
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::linear();
    img
}

fn update_frame(
    tuning: Res<Tuning>,
    output: Res<Output>,
    src: Res<SourceTexture>,
    sig: Res<VideoSignal>,
    time: Res<Time<Real>>,
    mut frame: ResMut<FpvFrame>,
    mut meas: ResMut<Measurements>,
    rig: Res<Rig>,
    mut steps: ResMut<bench::SyntheticSteps>,
    debug: Res<DebugView>,
    probe: Res<CostProbe>,
) {
    if output.size.x == 0 {
        return;
    }
    let (pic, r_frame) = picture_rect(output.size, tuning.aspect);
    let (theta_max, k, g) = lens(tuning.fov_deg, tuning.lens_curve());
    let ext = source_extent(pic, r_frame, theta_max, k, g);
    meas.frame += 1;
    let reduce = tuning.reduce_motion;
    let a = &tuning.analog;
    let d = &tuning.digital;
    let apx = pic.z / (a.lines.max(100.0) * 4.0 / 3.0);
    let ab = sig.analog_shown;
    let db = sig.digital_shown;
    let smooth = |e0: f32, e1: f32, x: f32| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    frame.params = FpvParams {
        picture: pic,
        lens: Vec4::new(r_frame, theta_max, k, g),
        source: Vec4::new(ext.x, ext.y, src.size.x as f32, src.size.y as f32),
        mode: Vec4::new(
            if tuning.look == Look::Digital { 1.0 } else { 0.0 },
            (time.elapsed_secs_f64() % 1000.0) as f32,
            (meas.frame % 100_000) as f32,
            if reduce { 1.0 } else { 0.0 },
        ),
        analog: Vec4::new(apx, a.softness, a.grain, a.colour_bleed),
        analog2: Vec4::new(a.contrast, a.saturation, a.brightness, if reduce { 0.0 } else { 1.0 }),
        breakup_a: Vec4::new(ab, smooth(0.35, 0.8, ab), smooth(0.55, 0.95, ab), if reduce { 0.0 } else { smooth(0.9, 1.0, ab) }),
        breakup_d: Vec4::new((db / 0.5).min(1.0) * 0.6, smooth(0.15, 0.7, db) * 0.7, 0.0, 0.0),
        digital: Vec4::new(d.sharpen, d.contrast, d.saturation, d.brightness),
        misc: Vec4::new(debug.0, 0.0, 0.0, 0.0),
    };
    frame.source = src.views.clone();
    frame.mips_enabled = std::env::var("FPV_NO_MIPS").is_err();
    frame.repeat_look = probe.0.max(1);
    frame.frame_id = meas.frame;
    // Latency probe: a synthetic step (bench) or a sharp stick move consumed this frame.
    frame.latency_t0_ns = steps.take_pending().or(rig.stick_step_ns);
}

fn collect_measurements(
    link: Res<render::TimingLink>,
    mut meas: ResMut<Measurements>,
    time: Res<Time<Real>>,
    mut last_log: Local<f64>,
    sig: Res<VideoSignal>,
) {
    let dt = time.delta_secs() * 1000.0;
    meas.cpu_ms.push_back(dt);
    while meas.cpu_ms.len() > 5000 {
        meas.cpu_ms.pop_front();
    }
    let mut shared = link.0.lock().unwrap();
    meas.ts_supported = shared.timestamps_supported;
    meas.pipelines_waiting = shared.pipelines_waiting;
    meas.pipelines_settled =
        shared.pipelines_waiting == 0 && shared.pipelines_busy_since.is_none_or(|t| t.elapsed().as_secs_f32() > 1.0);
    for f in shared.frames.drain(..) {
        meas.gpu.push_back(f);
    }
    let lat: Vec<_> = shared.latency.drain(..).collect();
    drop(shared);
    meas.latency.extend(lat);
    while meas.gpu.len() > 5000 {
        meas.gpu.pop_front();
    }
    // A line in the terminal every 2 s, so runs can be checked without looking at the screen.
    let now = time.elapsed_secs_f64();
    if now - *last_log > 2.0 {
        *last_log = now;
        let n = 120;
        let g = |f: &dyn Fn(&render::FrameGpu) -> f32| stats(meas.gpu.iter().rev().take(n).map(f).collect()).0;
        let cpu = stats(meas.cpu_ms.iter().rev().take(n).copied().collect()).0;
        info!(
            "[meas] frame {:.2} ms ({:.0} fps) | gpu frame {:.2} | mips {:.3} | merged {:.3} | source {}x{} | signal {:.1} dBm, {} walls {:.2} m, analog {:.0}%, digital {} {:.0}%",
            cpu,
            1000.0 / cpu,
            g(&|f| f.total),
            g(&|f| f.mips),
            g(&|f| f.look),
            meas.source_size.x,
            meas.source_size.y,
            sig.rx_dbm,
            sig.walls + sig.open_surfaces,
            sig.concrete_m,
            sig.analog_shown * 100.0,
            sig.digital_state.label(),
            sig.digital_shown * 100.0
        );
    }
}

pub fn stats(mut v: Vec<f32>) -> (f32, f32, f32) {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return (f32::NAN, f32::NAN, f32::NAN);
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let p = |q: f32| v[((v.len() - 1) as f32 * q).round() as usize];
    (p(0.5), p(0.95), p(0.99))
}
