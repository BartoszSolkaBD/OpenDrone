//! PROTOTYPE for issue #29: "Can Bevy render headless on GitHub's Linux runner and report
//! per-frame render counts?" Throwaway. Not production code. Run it with ./run.sh.
//!
//! No window: the Map is drawn into a 2560x1440 offscreen image. A fixed camera path is flown
//! for N frames, and every frame's render Work Counts are written to CSV files:
//!
//! - wgpu API counts (draw calls, passes, target pixels, pipelines, buffers, textures, uploads)
//!   come from our patched wgpu 29.0.4 (`wgpu-counting.patch`, module `wgpu::od_counters`).
//! - Triangles: direct draws from their arguments; Bevy's indirect (GPU-driven) draws from their
//!   arguments read back after each frame. Works on every backend.
//! - GPU pipeline statistics per render pass (triangles reaching the clipper, fragment shader
//!   invocations) come from a pipeline-statistics query the patch wraps around every render
//!   pass. Vulkan/DX12 only (so on GitHub's Linux runner, not on Metal).
//! - GPU memory as the driver allocated it comes from wgpu's own internal counters
//!   (`counters` feature). Vulkan/DX12 only.
//!
//! Bevy 0.19.1, as in the #28 camera prototype.

mod paths;

use bevy::{
    app::ScheduleRunnerPlugin,
    asset::RenderAssetUsages,
    camera::{Exposure, RenderTarget},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    light::{CascadeShadowConfigBuilder, GlobalAmbientLight},
    log::LogPlugin,
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{CachedPipelineState, Extent3d, PipelineCache, TextureDimension, TextureFormat},
        renderer::{RenderAdapterInfo, RenderDevice, RenderQueue},
        view::{
            screenshot::{save_to_disk, Screenshot},
            Msaa,
        },
        Render, RenderApp, RenderDebugFlags, RenderPlugin, RenderSystems,
    },
    time::TimeUpdateStrategy,
    window::ExitCondition,
    world_serialization::WorldInstanceReady,
};
use paths::{SampledPath, PATHS};
use std::{
    fmt::Write as _,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use wgpu::od_counters::{FrameCounts, PassRecord};

const WIDTH: u32 = 2560;
const HEIGHT: u32 = 1440;
const GLB_DIR: &str = "assets-src/maps/prototype-blockouts/out/glb";

/// Blender (x east, y north, z up) to Bevy (x east, y up, z south).
pub fn b(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, -y)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    SkatePark,
    Bando,
}

impl MapKind {
    fn glb(self) -> &'static str {
        match self {
            MapKind::SkatePark => "skate_park_ba.glb",
            MapKind::Bando => "bando_a_rooms.glb",
        }
    }
    fn id(self) -> &'static str {
        match self {
            MapKind::SkatePark => "skate-park",
            MapKind::Bando => "bando",
        }
    }
    fn label(self) -> &'static str {
        match self {
            MapKind::SkatePark => "Skate Park B+A (skate_park_ba.glb)",
            MapKind::Bando => "Bando A+ (bando_a_rooms.glb)",
        }
    }
}

#[derive(Resource, Clone, Debug)]
struct Args {
    map: MapKind,
    frames: u32,
    out: PathBuf,
    /// Skip the screenshot taken after the flight.
    no_screenshot: bool,
    /// Bevy's worker threads (default: one per core). Counts must not depend on it.
    threads: Option<usize>,
}

fn parse_args() -> Args {
    let mut a = Args { map: MapKind::Bando, frames: 120, out: PathBuf::new(), no_screenshot: false, threads: None };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" => {
                i += 1;
                a.map = match args.get(i).map(|s| s.as_str()) {
                    Some("bando") => MapKind::Bando,
                    Some("skate-park") | Some("skate") => MapKind::SkatePark,
                    other => panic!("--map bando|skate-park, got {other:?}"),
                };
            }
            "--frames" => {
                i += 1;
                a.frames = args.get(i).and_then(|v| v.parse::<u32>().ok()).expect("--frames N").max(2);
            }
            "--out" => {
                i += 1;
                a.out = PathBuf::from(args.get(i).expect("--out DIR"));
            }
            "--no-screenshot" => a.no_screenshot = true,
            "--threads" => {
                i += 1;
                a.threads = Some(args.get(i).and_then(|v| v.parse::<usize>().ok()).expect("--threads N").max(1));
            }
            other => panic!("unknown argument {other}"),
        }
        i += 1;
    }
    if a.out.as_os_str().is_empty() {
        a.out = PathBuf::from(format!("results/latest/{}", a.map.id()));
    }
    a
}

// ---- what the render world reports back each frame -------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Phase {
    /// Map loading, camera off. Frame count depends on disk and thread timing, so only totals count.
    #[default]
    Load,
    /// Camera on at the first flight pose until no pipeline is waiting. Only totals count.
    Warmup,
    /// The recorded flight: one row per frame.
    Flight,
    /// After the flight (screenshot). Not counted.
    Post,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Load => "load",
            Phase::Warmup => "warmup",
            Phase::Flight => "flight",
            Phase::Post => "post",
        }
    }
}

/// Which frame the main world is on; extracted so the render world can label its counts.
#[derive(Resource, Clone, Copy, Debug, Default, ExtractResource)]
struct FrameTag {
    phase: Phase,
    index: u32,
}

/// wgpu's own (hal) counters: live objects and, on Vulkan/DX12, driver memory.
#[derive(Clone, Debug, Default)]
struct Hal {
    buffer_memory: i64,
    texture_memory: i64,
    memory_allocations: i64,
    buffers: i64,
    textures: i64,
    texture_views: i64,
    bind_groups: i64,
    render_pipelines: i64,
    compute_pipelines: i64,
    shader_modules: i64,
    samplers: i64,
}

#[derive(Clone, Debug, Default)]
struct FrameRecord {
    phase: Phase,
    index: u32,
    c: FrameCounts,
    hal: Hal,
    cache_ok: u32,
    cache_waiting: u32,
    cache_err: u32,
    /// Wall-clock time from the previous record to this one (timing only; never compared).
    wall_ms: f64,
}

#[derive(Default)]
struct SharedState {
    frames: Vec<FrameRecord>,
    last_waiting: u32,
    stats_supported: Option<bool>,
    adapter: String,
    last_instant: Option<Instant>,
}

#[derive(Resource, Clone, Default)]
struct Shared(Arc<Mutex<SharedState>>);

// ---- the app ---------------------------------------------------------------------------------

#[derive(Component)]
struct FlightCam;

#[derive(Resource)]
struct Run {
    phase: Phase,
    index: u32,
    path: SampledPath,
    built_frames: u32,
    started: Instant,
    phase_started: Instant,
    times: Vec<(Phase, f64)>,
    screenshot_done: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Resource, Default)]
struct MapState {
    root: Option<Entity>,
    ready_frames: Option<u32>,
    built: bool,
    triangles: usize,
    meshes: usize,
}

#[derive(Resource)]
struct Output(Handle<Image>);

fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.out).expect("create output dir");
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let shared = Shared::default();

    let tasks = match args.threads {
        Some(n) => TaskPoolPlugin { task_pool_options: TaskPoolOptions::with_num_threads(n) },
        None => TaskPoolPlugin::default(),
    };
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(tasks)
            .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, close_when_requested: false, ..default() })
            .set(AssetPlugin { file_path: repo_root.to_string_lossy().into_owned(), ..default() })
            // Compile shader pipelines on the spot (not on background threads), so whether a
            // pipeline is ready never depends on thread timing. Always the case on macOS.
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                // Lets us read back Bevy's indirect draw arguments to count their triangles.
                debug_flags: RenderDebugFlags::ALLOW_COPIES_FROM_INDIRECT_PARAMETERS,
                ..default()
            })
            .set(LogPlugin { filter: "wgpu=error,naga=warn,bevy_render=info,headless_render_counts=info".into(), ..default() })
            // Render right after each main-world update, in the same `app.update()`.
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO))
    .add_plugins(ExtractResourcePlugin::<FrameTag>::default())
    // Every frame advances time by exactly 1/60 s, whatever the wall clock says.
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
    .insert_resource(ClearColor(Color::LinearRgba(LinearRgba::rgb(0.14, 0.22, 0.42))))
    .insert_resource(GlobalAmbientLight { brightness: 1500.0, ..default() })
    .insert_resource(shared.clone())
    .insert_resource(Run {
        phase: Phase::Load,
        index: 0,
        path: SampledPath::new(match args.map {
            MapKind::Bando => &PATHS[0],
            MapKind::SkatePark => &PATHS[1],
        }),
        built_frames: 0,
        started: Instant::now(),
        phase_started: Instant::now(),
        times: Vec::new(),
        screenshot_done: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    })
    .insert_resource(args)
    .init_resource::<FrameTag>()
    .init_resource::<MapState>()
    .add_observer(on_map_ready)
    .add_systems(Startup, setup)
    .add_systems(Update, (build_map, drive).chain());

    app.sub_app_mut(RenderApp)
        .insert_resource(shared)
        .add_systems(Render, record_frame.in_set(RenderSystems::Cleanup));
    app.run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut images: ResMut<Assets<Image>>, args: Res<Args>, mut state: ResMut<MapState>) {
    let img = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    let handle = images.add(img);
    commands.insert_resource(Output(handle.clone()));
    commands.spawn((
        Camera3d::default(),
        // Off until the Map is in, so loading never draws a half-built Map.
        Camera { is_active: false, ..default() },
        RenderTarget::Image(handle.into()),
        // Flat 1.5 rad vertical FOV (118 deg horizontal at 16:9), like the #28 camera's source render.
        Projection::Perspective(PerspectiveProjection { fov: 1.5, near: 0.004, far: 3000.0, ..default() }),
        Tonemapping::TonyMcMapface,
        Exposure { ev100: 15.0 },
        Msaa::Sample4,
        Transform::default(),
        FlightCam,
    ));
    // Sun from the south-south-east, 50 degrees up, with shadows (as in the #28 prototype).
    let el = 50f32.to_radians();
    let az = 157.5f32.to_radians();
    let to_sun = b(az.sin() * el.cos(), az.cos() * el.cos(), el.sin());
    commands.spawn((
        DirectionalLight { illuminance: 100_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(to_sun * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 3, first_cascade_far_bound: 8.0, maximum_distance: 120.0, ..default() }.build(),
    ));
    let path = format!("{GLB_DIR}/{}", args.map.glb());
    let root = commands.spawn(WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path)))).id();
    state.root = Some(root);
}

fn on_map_ready(ev: On<WorldInstanceReady>, mut state: ResMut<MapState>) {
    if state.root == Some(ev.entity) {
        state.ready_frames = Some(0);
    }
}

/// Two frames after the scene spawned: world-scale detail UVs and a shared detail texture on
/// every material, as the #28 prototype does (a stand-in for the CC0 textures).
fn build_map(
    mut state: ResMut<MapState>,
    children: Query<&Children>,
    mesh_q: Query<(&Mesh3d, &GlobalTransform, Option<&MeshMaterial3d<StandardMaterial>>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
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
    let detail = images.add(detail_texture());
    let mut done_materials = std::collections::HashSet::new();
    let mut done_meshes = std::collections::HashSet::new();
    for e in children.iter_descendants(root) {
        let Ok((mesh3d, gt, mat)) = mesh_q.get(e) else { continue };
        if !done_meshes.insert(mesh3d.0.id()) {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else { continue };
        state.meshes += 1;
        state.triangles += mesh.indices().map(|i| i.len()).unwrap_or(0) / 3;
        let affine = gt.affine();
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
            let nrm = (wpos[t + 1] - wpos[t]).cross(wpos[t + 2] - wpos[t]).abs();
            for k in 0..3 {
                let p = wpos[t + k] * 0.5;
                uvs[t + k] = if nrm.y >= nrm.x && nrm.y >= nrm.z {
                    [p.x, p.z]
                } else if nrm.x >= nrm.z {
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
    info!("map built: {} meshes, {} triangles, {} materials", state.meshes, state.triangles, done_materials.len());
    state.built = true;
}

/// Camera pose for flight frame `i` of `n`: evenly spaced along the path, looking 1 m ahead.
fn pose(path: &SampledPath, i: u32, n: u32) -> Transform {
    let len = path.length();
    let d = len * i as f32 / (n - 1) as f32;
    let (p, _) = path.at(d);
    let (mut ahead, _) = path.at(d + 1.0);
    if (ahead - p).length() < 0.05 {
        let (back, _) = path.at(d - 1.0);
        ahead = p + (p - back);
    }
    Transform::from_translation(p).looking_at(ahead, Vec3::Y)
}

fn drive(
    mut run: ResMut<Run>,
    mut tag: ResMut<FrameTag>,
    state: Res<MapState>,
    shared: Res<Shared>,
    args: Res<Args>,
    output: Res<Output>,
    mut cams: Query<(&mut Camera, &mut Transform), With<FlightCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let n = args.frames;
    let (last_waiting, last_new_pipelines) = {
        let s = shared.0.lock().unwrap();
        let newp = s.frames.last().map(|f| f.c.render_pipelines_created + f.c.compute_pipelines_created).unwrap_or(0);
        (s.last_waiting, newp)
    };
    let next = |run: &mut Run, phase: Phase| {
        let now = Instant::now();
        let prev = run.phase;
        run.times.push((prev, now.duration_since(run.phase_started).as_secs_f64()));
        run.phase_started = now;
        run.phase = phase;
        run.index = 0;
        info!("phase {} -> {}", prev.name(), phase.name());
    };
    match run.phase {
        Phase::Load => {
            if state.built {
                run.built_frames += 1;
            }
            if run.built_frames >= 2 {
                next(&mut run, Phase::Warmup);
                let p = pose(&run.path, 0, n);
                for (mut cam, mut tf) in &mut cams {
                    cam.is_active = true;
                    *tf = p;
                }
            } else {
                run.index += 1;
                if run.index > 6000 {
                    panic!("the Map never finished loading");
                }
            }
        }
        Phase::Warmup => {
            // Ready once a frame has drawn with nothing waiting and nothing new compiled.
            if run.index >= 3 && last_waiting == 0 && last_new_pipelines == 0 {
                next(&mut run, Phase::Flight);
            } else {
                run.index += 1;
                if run.index > 600 {
                    panic!("pipelines never settled: {last_waiting} still waiting");
                }
            }
        }
        Phase::Flight => {
            run.index += 1;
            if run.index >= n {
                next(&mut run, Phase::Post);
                // Screenshot of the middle of the flight, taken after it so it can't touch the counts.
                let mid = pose(&run.path, n / 2, n);
                for (_, mut tf) in &mut cams {
                    *tf = mid;
                }
                if !args.no_screenshot {
                    let done = run.screenshot_done.clone();
                    let path = args.out.join("mid-flight.png");
                    commands.spawn(Screenshot::image(output.0.clone())).observe(save_to_disk(path)).observe(
                        move |_: On<bevy::render::view::screenshot::ScreenshotCaptured>| {
                            done.store(true, std::sync::atomic::Ordering::Release);
                        },
                    );
                } else {
                    run.screenshot_done.store(true, std::sync::atomic::Ordering::Release);
                }
            }
        }
        Phase::Post => {
            run.index += 1;
            if run.screenshot_done.load(std::sync::atomic::Ordering::Acquire) && run.index > 3 || run.index > 120 {
                let total = run.started.elapsed().as_secs_f64();
                write_outputs(&args, &shared, &run, &state, total);
                exit.write(AppExit::Success);
            }
        }
    }
    if run.phase == Phase::Flight {
        let p = pose(&run.path, run.index, n);
        for (_, mut tf) in &mut cams {
            *tf = p;
        }
    }
    *tag = FrameTag { phase: run.phase, index: run.index };
}

/// Render world, after the frame was submitted: wait for the GPU, then snapshot every count.
fn record_frame(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    adapter: Res<RenderAdapterInfo>,
    tag: Option<Res<FrameTag>>,
    shared: Res<Shared>,
    mut installed: Local<bool>,
) {
    let dev = device.wgpu_device();
    let q: &wgpu::Queue = &queue;
    if !*installed {
        *installed = true;
        let ok = wgpu::od_counters::install_pipeline_statistics(dev);
        let mut s = shared.0.lock().unwrap();
        s.stats_supported = Some(ok);
        s.adapter = format!(
            "{} | backend {:?} | device type {:?} | driver {} {}",
            adapter.name, adapter.backend, adapter.device_type, adapter.driver, adapter.driver_info
        );
        info!("adapter: {} | pipeline statistics: {ok}", s.adapter);
    } else {
        wgpu::od_counters::resolve_pipeline_statistics(dev, q);
    }
    wgpu::od_counters::resolve_indirect_draws(dev, q);
    let _ = dev.poll(wgpu::PollType::wait_indefinitely());
    let c = wgpu::od_counters::take_frame();
    let ic = dev.get_internal_counters().hal;
    let hal = Hal {
        buffer_memory: ic.buffer_memory.read() as i64,
        texture_memory: ic.texture_memory.read() as i64,
        memory_allocations: ic.memory_allocations.read() as i64,
        buffers: ic.buffers.read() as i64,
        textures: ic.textures.read() as i64,
        texture_views: ic.texture_views.read() as i64,
        bind_groups: ic.bind_groups.read() as i64,
        render_pipelines: ic.render_pipelines.read() as i64,
        compute_pipelines: ic.compute_pipelines.read() as i64,
        shader_modules: ic.shader_modules.read() as i64,
        samplers: ic.samplers.read() as i64,
    };
    let (mut ok, mut err) = (0, 0);
    for p in cache.pipelines() {
        match p.state {
            CachedPipelineState::Ok(_) => ok += 1,
            CachedPipelineState::Err(_) => err += 1,
            _ => {}
        }
    }
    let waiting = cache.waiting_pipelines().count() as u32;
    let tag = tag.map(|t| *t).unwrap_or_default();
    let mut s = shared.0.lock().unwrap();
    let now = Instant::now();
    let wall_ms = s.last_instant.map(|t| now.duration_since(t).as_secs_f64() * 1000.0).unwrap_or(0.0);
    s.last_instant = Some(now);
    s.last_waiting = waiting;
    s.frames.push(FrameRecord {
        phase: tag.phase,
        index: tag.index,
        c,
        hal,
        cache_ok: ok,
        cache_waiting: waiting,
        cache_err: err,
        wall_ms,
    });
}

// ---- output ----------------------------------------------------------------------------------

/// One row of frames.csv: per-frame counts plus end-of-frame gauges.
#[derive(Clone, Default)]
struct Row {
    label: String,
    frames: u32,
    v: Vec<(&'static str, i64)>,
}

fn gpu_sums(passes: &[PassRecord]) -> [u64; 5] {
    let mut out = [0u64; 5];
    for p in passes {
        if let Some(s) = p.stats {
            for k in 0..5 {
                out[k] += s[k];
            }
        }
    }
    out
}

fn values(f: &FrameRecord) -> Vec<(&'static str, i64)> {
    let c = &f.c;
    let g = gpu_sums(&c.passes);
    let stats_passes = c.passes.iter().filter(|p| p.stats.is_some()).count() as i64;
    vec![
        ("render_passes", c.render_passes as i64),
        ("compute_passes", c.compute_passes as i64),
        ("target_pixels", c.target_pixels as i64),
        ("draws", c.draws as i64),
        ("draws_direct", c.draws_direct as i64),
        ("draws_indirect", c.draws_indirect as i64),
        ("indirect_calls", c.indirect_calls as i64),
        ("indirect_unread", c.indirect_unread as i64),
        ("draws_nonempty", c.draws_nonempty as i64),
        ("triangles", (c.triangles_direct + c.triangles_indirect) as i64),
        ("triangles_direct", c.triangles_direct as i64),
        ("triangles_indirect", c.triangles_indirect as i64),
        ("vertices_direct", c.vertices_direct as i64),
        ("dispatches", c.dispatches as i64),
        ("render_pipelines_new", c.render_pipelines_created as i64),
        ("compute_pipelines_new", c.compute_pipelines_created as i64),
        ("shader_modules_new", c.shader_modules_created as i64),
        ("bind_groups_new", c.bind_groups_created as i64),
        ("buffers_new", c.buffers_created as i64),
        ("buffer_bytes_new", c.buffer_bytes_created as i64),
        ("textures_new", c.textures_created as i64),
        ("texture_bytes_new", c.texture_bytes_created as i64),
        ("upload_bytes", c.upload_bytes as i64),
        ("submits", c.submits as i64),
        ("bundles_executed", c.bundles_executed as i64),
        ("gpu_passes_with_stats", stats_passes),
        ("gpu_vertex_invocations", g[0] as i64),
        ("gpu_clipper_invocations", g[1] as i64),
        ("gpu_clipper_primitives_out", g[2] as i64),
        ("gpu_fragment_invocations", g[3] as i64),
        // Gauges at the end of the frame.
        ("live_buffers", c.buffers_live),
        ("live_buffer_bytes", c.buffer_bytes_live),
        ("live_textures", c.textures_live),
        ("live_texture_bytes", c.texture_bytes_live),
        ("hal_buffer_memory", f.hal.buffer_memory),
        ("hal_texture_memory", f.hal.texture_memory),
        ("hal_memory_allocations", f.hal.memory_allocations),
        ("hal_buffers", f.hal.buffers),
        ("hal_textures", f.hal.textures),
        ("hal_texture_views", f.hal.texture_views),
        ("hal_bind_groups", f.hal.bind_groups),
        ("hal_render_pipelines", f.hal.render_pipelines),
        ("hal_compute_pipelines", f.hal.compute_pipelines),
        ("hal_shader_modules", f.hal.shader_modules),
        ("hal_samplers", f.hal.samplers),
        ("cache_pipelines_ok", f.cache_ok as i64),
        ("cache_pipelines_waiting", f.cache_waiting as i64),
        ("cache_pipelines_err", f.cache_err as i64),
    ]
}

fn is_gauge(name: &str) -> bool {
    name.starts_with("live_") || name.starts_with("hal_") || name.starts_with("cache_")
}

/// Load and warm-up become one row each: per-frame counts summed, gauges from their last frame.
fn aggregate(label: &str, frames: &[&FrameRecord]) -> Row {
    let mut row = Row { label: label.into(), frames: frames.len() as u32, v: Vec::new() };
    for f in frames {
        let v = values(f);
        if row.v.is_empty() {
            row.v = v;
        } else {
            for (acc, (name, x)) in row.v.iter_mut().zip(v) {
                if is_gauge(name) {
                    acc.1 = x;
                } else {
                    acc.1 += x;
                }
            }
        }
    }
    row
}

/// Passes of a frame (or phase), grouped by label and sorted, so recording order (which can vary
/// with Bevy's parallel command encoding) never shows up as a difference.
fn pass_groups(passes: &[&PassRecord]) -> Vec<(String, Vec<i64>)> {
    let mut map: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
    for p in passes {
        let key = format!(
            "{}|{}|{}x{}|{}x|{}c|{}",
            if p.compute { "compute" } else { "render" },
            p.label,
            p.width,
            p.height,
            p.samples,
            p.color_targets,
            if p.depth { "depth" } else { "nodepth" }
        );
        let s = p.stats.unwrap_or_default();
        // Keep in step with PASS_COLS and the P_* indices.
        let add = [
            1,
            (p.width as u64 * p.height as u64) as i64,
            p.draws as i64,
            p.draws_indirect as i64,
            p.draws_nonempty as i64,
            (p.triangles_direct + p.triangles_indirect) as i64,
            p.dispatches as i64,
            p.stats.is_some() as i64,
            s[0] as i64,
            s[1] as i64,
            s[2] as i64,
            s[3] as i64,
        ];
        let e = map.entry(key).or_insert_with(|| vec![0; add.len()]);
        for (a, x) in e.iter_mut().zip(add) {
            *a += x;
        }
    }
    map.into_iter().collect()
}

const PASS_COLS: &str = "count,target_pixels,draws,draws_indirect,draws_nonempty,triangles,dispatches,gpu_stats,gpu_vertex_invocations,gpu_clipper_invocations,gpu_clipper_primitives_out,gpu_fragment_invocations";
const P_COUNT: usize = 0;
const P_PIXELS: usize = 1;
const P_DRAWS: usize = 2;
const P_NONEMPTY: usize = 4;
const P_TRIS: usize = 5;
const P_STATS: usize = 7;
const P_GPU_CLIP_IN: usize = 9;
const P_GPU_FRAG: usize = 11;

fn write_outputs(args: &Args, shared: &Shared, run: &Run, state: &MapState, total_s: f64) {
    let s = shared.0.lock().unwrap();
    let by = |ph: Phase| s.frames.iter().filter(|f| f.phase == ph).collect::<Vec<_>>();
    let (load, warm, flight) = (by(Phase::Load), by(Phase::Warmup), by(Phase::Flight));
    assert_eq!(flight.len() as u32, args.frames, "every flight frame must have exactly one record");
    for (i, f) in flight.iter().enumerate() {
        assert_eq!(f.index as usize, i, "flight records out of order");
    }

    // frames.csv: warm-up as one row, then one row per flight frame. load.csv: the loading
    // frames as one row. Loading takes a varying number of frames (disk and thread timing), and
    // some counts grow with every frame (submits, uniform uploads), so load.csv isn't compared;
    // its end-of-load gauges carry into the warm-up row anyway.
    let mut rows = vec![aggregate("load", &load), aggregate("warmup", &warm)];
    for f in &flight {
        rows.push(Row { label: format!("flight-{:04}", f.index), frames: 1, v: values(f) });
    }
    let header = {
        let mut h = String::from("row,frames");
        for (name, _) in &rows[2].v {
            write!(h, ",{name}").unwrap();
        }
        h.push('\n');
        h
    };
    let line = |r: &Row| {
        let mut l = format!("{},{}", r.label, r.frames);
        for (_, x) in &r.v {
            write!(l, ",{x}").unwrap();
        }
        l.push('\n');
        l
    };
    std::fs::write(args.out.join("load.csv"), header.clone() + &line(&rows[0])).unwrap();
    let mut csv = header;
    for r in &rows[1..] {
        csv += &line(r);
    }
    std::fs::write(args.out.join("frames.csv"), &csv).unwrap();

    // passes.csv: per row, passes grouped by label and target.
    let mut pcsv = format!("row,kind,label,target,samples,colors,depth,{PASS_COLS}\n");
    let mut emit = |label: &str, recs: &[&FrameRecord]| {
        let passes: Vec<&PassRecord> = recs.iter().flat_map(|f| f.c.passes.iter()).collect();
        for (key, v) in pass_groups(&passes) {
            let k: Vec<&str> = key.split('|').collect();
            write!(pcsv, "{label},{},{},{},{},{},{}", k[0], k[1], k[2], k[3], k[4], k[5]).unwrap();
            for x in v {
                write!(pcsv, ",{x}").unwrap();
            }
            pcsv.push('\n');
        }
    };
    emit("warmup", &warm);
    for f in &flight {
        emit(&format!("flight-{:04}", f.index), std::slice::from_ref(f));
    }
    std::fs::write(args.out.join("passes.csv"), &pcsv).unwrap();

    // summary.md: totals and per-frame min / max / mean over the flight.
    let mut md = String::new();
    writeln!(md, "# Render Work Counts: {}\n", args.map.label()).unwrap();
    writeln!(md, "PROTOTYPE output for issue #29. Counts are per frame at {WIDTH}x{HEIGHT}, offscreen, MSAA 4x, sun shadows (3 cascades), flat 118 deg camera, Bevy 0.19.1.\n").unwrap();
    writeln!(md, "- Adapter: {}", s.adapter).unwrap();
    writeln!(md, "- GPU pipeline statistics: {}", if s.stats_supported == Some(true) { "yes (per render pass)" } else { "not supported on this backend" }).unwrap();
    writeln!(md, "- Map: {} meshes, {} triangles in the file", state.meshes, state.triangles).unwrap();
    writeln!(md, "- Flight: {} frames evenly spaced along \"{}\" ({:.0} m)", args.frames, run_path_name(args.map), run.path.length()).unwrap();
    writeln!(md, "- Frames before the flight: {} loading (varies with disk and threads), {} warm-up", load.len(), warm.len()).unwrap();
    writeln!(md, "\n{SOURCES}").unwrap();
    let flight_ms: Vec<f64> = flight.iter().map(|f| f.wall_ms).collect();
    let mean_ms = flight_ms.iter().sum::<f64>() / flight_ms.len().max(1) as f64;
    writeln!(md, "\n## Time (not a Work Count; never compared)\n").unwrap();
    for (ph, secs) in &run.times {
        writeln!(md, "- {}: {:.2} s", ph.name(), secs).unwrap();
    }
    writeln!(md, "- whole run (app start to exit, not counting the build): {total_s:.2} s").unwrap();
    writeln!(md, "- flight frame: mean {:.1} ms, max {:.1} ms (wall clock, includes waiting for the GPU every frame)", mean_ms, flight_ms.iter().cloned().fold(0.0, f64::max)).unwrap();

    writeln!(md, "\n## Per flight frame\n").unwrap();
    writeln!(md, "| Count | Total | Min | Max | Mean |").unwrap();
    writeln!(md, "|---|---:|---:|---:|---:|").unwrap();
    let names: Vec<&'static str> = rows[2].v.iter().map(|(n, _)| *n).collect();
    for (k, name) in names.iter().enumerate() {
        let xs: Vec<i64> = rows[2..].iter().map(|r| r.v[k].1).collect();
        let tot: i64 = xs.iter().sum();
        let mn = *xs.iter().min().unwrap();
        let mx = *xs.iter().max().unwrap();
        let mean = tot as f64 / xs.len() as f64;
        let total = if is_gauge(name) { "-".to_string() } else { tot.to_string() };
        writeln!(md, "| {name} | {total} | {mn} | {mx} | {mean:.1} |").unwrap();
    }

    writeln!(md, "\n## Before the flight (load + warm-up)\n").unwrap();
    writeln!(md, "| Count | Load | Warm-up |").unwrap();
    writeln!(md, "|---|---:|---:|").unwrap();
    for (k, name) in names.iter().enumerate() {
        writeln!(md, "| {name} | {} | {} |", rows[0].v[k].1, rows[1].v[k].1).unwrap();
    }
    let pipes = |r: &Row| r.v.iter().filter(|(n, _)| *n == "render_pipelines_new" || *n == "compute_pipelines_new").map(|(_, x)| *x).sum::<i64>();
    let pre = pipes(&rows[0]) + pipes(&rows[1]);
    let during: i64 = rows[2..].iter().map(pipes).sum();
    writeln!(md, "\nShader pipelines compiled: {} before the flight, {} during it, {} in all.", pre, during, pre + during).unwrap();

    writeln!(md, "\n## Passes over the whole flight\n").unwrap();
    writeln!(md, "Totals over all {} flight frames.\n", flight.len()).unwrap();
    writeln!(md, "| Kind | Label | Target | Samples | Passes | Target pixels | Draws | Non-empty draws | Triangles | GPU triangles in | GPU fragments |").unwrap();
    writeln!(md, "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|").unwrap();
    let passes: Vec<&PassRecord> = flight.iter().flat_map(|f| f.c.passes.iter()).collect();
    for (key, v) in pass_groups(&passes) {
        let k: Vec<&str> = key.split('|').collect();
        let label = if k[1].is_empty() { "(no label)" } else { k[1] };
        let gpu = |x: i64| if v[P_STATS] > 0 { x.to_string() } else { "-".into() };
        writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            k[0],
            label,
            k[2],
            k[3],
            v[P_COUNT],
            v[P_PIXELS],
            v[P_DRAWS],
            v[P_NONEMPTY],
            v[P_TRIS],
            gpu(v[P_GPU_CLIP_IN]),
            gpu(v[P_GPU_FRAG])
        )
        .unwrap();
    }
    std::fs::write(args.out.join("summary.md"), &md).unwrap();
    info!("wrote {}", args.out.display());
}

const SOURCES: &str = "## Where each count comes from

- `render_passes`, `compute_passes`, `draws*`, `indirect_calls`, `dispatches`, `*_new`, `upload_bytes`, `submits`: counted at wgpu's API by our patch (`wgpu-counting.patch`), every caller included.
- `target_pixels`: width x height of each render pass's first attachment, summed (MSAA samples not multiplied in).
- `triangles_direct`: from each direct draw's arguments. `triangles_indirect` and `draws_nonempty`: Bevy's GPU-driven draws, from their arguments read back after the frame (after GPU culling).
- `gpu_*`: a pipeline-statistics query around every render pass (Vulkan and DX12 only): vertex shader runs, triangles reaching the clipper, triangles leaving it, and fragment shader runs (the pixels actually shaded).
- `live_*`: buffers and textures alive at the end of the frame, sized from their descriptors (textures: every mip, layer and sample).
- `hal_*`: wgpu's own counters (`counters` feature). `hal_buffer_memory` and `hal_texture_memory` are the driver allocations on Vulkan and DX12; they read 0 on Metal.
- `cache_*`: Bevy's pipeline cache: shader pipelines ready, waiting, failed.";

fn run_path_name(map: MapKind) -> &'static str {
    match map {
        MapKind::Bando => PATHS[0].name,
        MapKind::SkatePark => PATHS[1].name,
    }
}

/// A tileable concrete-ish texture (2 m per tile) with a faint 1 m grid. Copied from the #28
/// prototype: a stand-in for the CC0 textures, so texture memory and sampling are exercised.
fn detail_texture() -> Image {
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
