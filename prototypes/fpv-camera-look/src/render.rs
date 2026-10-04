//! PROTOTYPE (#28). The camera's GPU side.
//!
//! - The Map is drawn once per frame by a normal (rectilinear) 3D camera into `SourceTexture`,
//!   wider than the picture (ADR-0009). Bevy's own tonemapping and auto-exposure run there.
//! - The display camera then runs two steps of our own: a mip chain for the source (so the warp
//!   can filter the squashed fisheye edges), and ONE merged pass that does the fisheye warp and
//!   the whole Video Look (Analog or Digital, Breakup included) straight into the window.
//! - Metal timestamps around those steps and around the whole frame give the GPU times.
//! - A latency probe records when a tagged input's frame was presented and finished on the GPU.

use bevy::{
    camera::ManualTextureViewHandle,
    core_pipeline::{
        core_2d::main_transparent_pass_2d, schedule::Core2d, Core2dSystems, FullscreenShader,
    },
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            AddressMode, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            Buffer, BufferDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, Extent3d, FilterMode, FragmentState, LoadOp,
            MapMode, MipmapFilterMode, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, ShaderType, StoreOp, Texture, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
            TextureViewDescriptor, UniformBuffer,
        },
        renderer::{
            PendingCommandBuffers, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems,
            RenderQueue, ViewQuery,
        },
        texture::{ManualTextureView, ManualTextureViews},
        view::ViewTarget,
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex,
};

pub const SOURCE_VIEW: ManualTextureViewHandle = ManualTextureViewHandle(2800);
const LOOK_SHADER: &str = "prototypes/fpv-camera-look/assets/fpv_look.wgsl";
const DOWN_SHADER: &str = "prototypes/fpv-camera-look/assets/fpv_downsample.wgsl";

/// Timestamps per frame, all taken at the END of a pass (Apple GPUs start a pass's vertex work
/// early, so start-of-pass times overlap earlier passes; ends are reliable because each of these
/// passes reads what the previous step wrote):
/// 0 frame begin marker, 1 scene done (marker reading the Map render), 2 mip chain done,
/// 3 merged pass done, 4 merged pass start (info only).
const N_TS: u32 = 5;
/// Timestamp slots (one per frame in flight).
const SLOTS: usize = 3;

/// How many frames may be queued on the GPU (Bevy's window default is 2). Set from `--frames-in-flight`.
pub static MAX_IN_FLIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2);

/// Everything the shader needs, packed in vec4s.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct FpvParams {
    /// x0, y0, w, h of the picture in target pixels.
    pub picture: Vec4,
    /// R (frame half-diagonal in px), theta_max (rad), lens k, g(theta_max).
    pub lens: Vec4,
    /// tan half-extent x, y of the source render; source w, h in px.
    pub source: Vec4,
    /// look (0 analog, 1 digital), time (s), frame counter, reduce motion.
    pub mode: Vec4,
    /// output px per analog px, softness, grain, colour bleed (analog px).
    pub analog: Vec4,
    /// contrast, saturation, brightness, flicker allowed (0/1).
    pub analog2: Vec4,
    /// Analog Breakup: static, colour loss, tearing, roll.
    pub breakup_a: Vec4,
    /// Digital Breakup: smear, blocks, -, black.
    pub breakup_d: Vec4,
    /// Digital: sharpen, contrast, saturation, brightness.
    pub digital: Vec4,
    /// debug view, spare...
    pub misc: Vec4,
    /// Camera: dynamic-range factor (8.6 / stops), tone curve (0 ACES, 1 soft, 2 hard clip),
    /// output px per analog line (vertical), output px per transmitted Digital pixel.
    pub cam: Vec4,
}

#[derive(Clone)]
pub struct SourceViews {
    pub full: TextureView,
    pub mips: Vec<TextureView>,
    pub size: UVec2,
}

/// Main-world copy of the source texture (kept alive here).
#[derive(Resource)]
pub struct SourceTexture {
    pub texture: Option<Texture>,
    pub views: Option<SourceViews>,
    pub size: UVec2,
    pub mips_enabled: bool,
}

/// What the render world gets each frame.
#[derive(Resource, Clone, ExtractResource)]
pub struct FpvFrame {
    pub params: FpvParams,
    pub source: Option<SourceViews>,
    pub mips_enabled: bool,
    pub frame_id: u64,
    /// mach-clock ns of an input step first used by this frame (latency probe).
    pub latency_t0_ns: Option<u64>,
    /// Cost probe: draw the merged pass this many times (1 = normal).
    pub repeat_look: u32,
}

impl Default for FpvFrame {
    fn default() -> Self {
        Self { params: FpvParams::default(), source: None, mips_enabled: true, frame_id: 0, latency_t0_ns: None, repeat_look: 1 }
    }
}

#[derive(Component, Clone, ExtractComponent)]
pub struct FpvDisplay;

/// One frame's GPU times, in ms.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameGpu {
    pub frame_id: u64,
    pub total: f32,
    /// Frame begin to mips begin: everything before our camera steps (shadows, Map, auto-exposure,
    /// tonemapping, Bevy's copy into the source texture).
    pub scene: f32,
    pub mips: f32,
    pub look: f32,
    /// Start to end of the merged pass as Metal reports it (includes waiting; info only).
    pub tail: f32,
    /// GPU merged-pass end minus CPU submit time (checks the clocks agree).
    pub end_minus_submit: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LatencySample {
    pub frame_id: u64,
    /// Input to the moment present() returned on the CPU.
    pub to_present_ms: f32,
    /// Input to the GPU finishing that frame (only if the GPU clock matches the CPU clock).
    pub to_gpu_done_ms: Option<f32>,
}

#[derive(Default)]
pub struct TimingShared {
    pub frames: Vec<FrameGpu>,
    pub latency: Vec<LatencySample>,
    pub timestamps_supported: Option<bool>,
    /// Shaders still compiling (Metal has no pipeline cache, so the first run compiles a lot).
    pub pipelines_waiting: usize,
    pub pipelines_busy_since: Option<std::time::Instant>,
}

#[derive(Resource, Clone, Default)]
pub struct TimingLink(pub Arc<Mutex<TimingShared>>);

pub struct FpvRenderPlugin;

impl Plugin for FpvRenderPlugin {
    fn build(&self, app: &mut App) {
        let link = TimingLink::default();
        app.insert_resource(link.clone())
            .init_resource::<FpvFrame>()
            .add_plugins((ExtractResourcePlugin::<FpvFrame>::default(), ExtractComponentPlugin::<FpvDisplay>::default()));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(link)
            .add_systems(RenderStartup, (init_pipelines, init_timer))
            .add_systems(Render, (prepare_uniform, pipeline_census).in_set(RenderSystems::Prepare))
            .add_systems(RenderGraph, frame_begin.in_set(RenderGraphSystems::Begin))
            .add_systems(
                RenderGraph,
                frame_end.after(RenderGraphSystems::Render).before(RenderGraphSystems::Submit),
            )
            .add_systems(
                Core2d,
                display_pass.in_set(Core2dSystems::MainPass).after(main_transparent_pass_2d),
            )
            .add_systems(
                Render,
                after_present.after(bevy::render::renderer::render_system).in_set(RenderSystems::Render),
            );
    }
}

// ---------------------------------------------------------------------------------------------
// Main world: the source texture.

pub fn create_source(device: &RenderDevice, size: UVec2, mips_enabled: bool) -> (Texture, SourceViews) {
    let mip_count = if mips_enabled { 32 - size.x.max(size.y).max(1).leading_zeros() } else { 1 };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("fpv_source"),
        size: Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let full = texture.create_view(&TextureViewDescriptor::default());
    let mips = (0..mip_count)
        .map(|i| {
            texture.create_view(&TextureViewDescriptor {
                label: Some("fpv_source_mip"),
                base_mip_level: i,
                mip_level_count: Some(1),
                ..default()
            })
        })
        .collect();
    (texture, SourceViews { full, mips, size })
}

/// Recreates the source texture when its size changes and registers mip 0 as the 3D camera's target.
pub fn ensure_source(
    device: &RenderDevice,
    src: &mut SourceTexture,
    manual: &mut ManualTextureViews,
    size: UVec2,
    mips_enabled: bool,
) {
    if src.texture.is_some() && src.size == size && src.mips_enabled == mips_enabled {
        return;
    }
    let (tex, views) = create_source(device, size, mips_enabled);
    manual.insert(
        SOURCE_VIEW,
        ManualTextureView { texture_view: views.mips[0].clone(), size, view_format: TextureFormat::Rgba8UnormSrgb },
    );
    src.texture = Some(tex);
    src.views = Some(views);
    src.size = size;
    src.mips_enabled = mips_enabled;
}

// ---------------------------------------------------------------------------------------------
// Render world.

#[derive(Resource)]
struct FpvPipelines {
    look_layout: BindGroupLayoutDescriptor,
    down_layout: BindGroupLayoutDescriptor,
    look: CachedRenderPipelineId,
    down: CachedRenderPipelineId,
    aniso: Sampler,
    linear: Sampler,
    trilinear: Sampler,
    uniform: UniformBuffer<FpvParams>,
}

fn init_pipelines(
    mut commands: Commands,
    device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let look_layout = BindGroupLayoutDescriptor::new(
        "fpv_look_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<FpvParams>(false),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let down_layout = BindGroupLayoutDescriptor::new(
        "fpv_down_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering)),
        ),
    );
    let target = |format| {
        vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })]
    };
    let look = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("fpv_look_pipeline".into()),
        layout: vec![look_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load(LOOK_SHADER),
            targets: target(TextureFormat::Rgba8UnormSrgb),
            ..default()
        }),
        ..default()
    });
    let down = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("fpv_downsample_pipeline".into()),
        layout: vec![down_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load(DOWN_SHADER),
            targets: target(TextureFormat::Rgba8UnormSrgb),
            ..default()
        }),
        ..default()
    });
    let aniso = device.create_sampler(&SamplerDescriptor {
        label: Some("fpv_aniso"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: MipmapFilterMode::Linear,
        anisotropy_clamp: std::env::var("FPV_ANISO").ok().and_then(|v| v.parse().ok()).unwrap_or(16),
        ..default()
    });
    let linear = device.create_sampler(&SamplerDescriptor {
        label: Some("fpv_linear"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    let trilinear = device.create_sampler(&SamplerDescriptor {
        label: Some("fpv_trilinear"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: MipmapFilterMode::Linear,
        ..default()
    });
    commands.insert_resource(FpvPipelines {
        look_layout,
        down_layout,
        look,
        down,
        aniso,
        linear,
        trilinear,
        uniform: UniformBuffer::default(),
    });
}

fn prepare_uniform(
    frame: Option<Res<FpvFrame>>,
    pipelines: Option<ResMut<FpvPipelines>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(frame), Some(mut p)) = (frame, pipelines) else { return };
    p.uniform.set(frame.params);
    p.uniform.write_buffer(&device, &queue);
}

#[derive(Resource)]
struct GpuTimer {
    qs: Option<wgpu::QuerySet>,
    resolve: Option<Buffer>,
    readback: Vec<Buffer>,
    state: Vec<Arc<AtomicU8>>,
    done_ns: Vec<Arc<std::sync::atomic::AtomicU64>>,
    written: Vec<u8>,
    submit_ns: Vec<u64>,
    frame_ids: Vec<u64>,
    latency_t0: Vec<Option<u64>>,
    present_ns: Vec<Option<u64>>,
    period_ns: f64,
    slot: Option<usize>,
    counter: u64,
    slot_counter: Vec<u64>,
    last_end: Option<(u64, f64)>,
    marker: TextureView,
}

const FREE: u8 = 0;
const IN_FLIGHT: u8 = 1;
const MAPPED: u8 = 2;

fn init_timer(mut commands: Commands, device: Res<RenderDevice>, queue: Res<RenderQueue>, link: Res<TimingLink>) {
    let supported = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    link.0.lock().unwrap().timestamps_supported = Some(supported);
    let (qs, resolve, readback) = if supported {
        let qs = device.wgpu_device().create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("fpv_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: N_TS * SLOTS as u32,
        });
        let resolve = device.create_buffer(&BufferDescriptor {
            label: Some("fpv_ts_resolve"),
            size: wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * SLOTS as u64,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = (0..SLOTS)
            .map(|_| {
                device.create_buffer(&BufferDescriptor {
                    label: Some("fpv_ts_readback"),
                    size: N_TS as u64 * 8,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        (Some(qs), Some(resolve), readback)
    } else {
        (None, None, Vec::new())
    };
    commands.insert_resource(GpuTimer {
        qs,
        resolve,
        readback,
        state: (0..SLOTS).map(|_| Arc::new(AtomicU8::new(FREE))).collect(),
        done_ns: (0..SLOTS).map(|_| Arc::new(std::sync::atomic::AtomicU64::new(0))).collect(),
        written: vec![0; SLOTS],
        submit_ns: vec![0; SLOTS],
        frame_ids: vec![0; SLOTS],
        latency_t0: vec![None; SLOTS],
        present_ns: vec![None; SLOTS],
        period_ns: queue.get_timestamp_period() as f64,
        slot: None,
        counter: 0,
        slot_counter: vec![0; SLOTS],
        last_end: None,
        marker: device
            .create_texture(&TextureDescriptor {
                label: Some("fpv_marker"),
                size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&TextureViewDescriptor::default()),
    });
}

/// A tiny draw into a 1x1 texture that reads `src`, with a timestamp at its end. It can only finish
/// after whatever wrote `src`, so its end time marks "that work is done".
#[allow(clippy::too_many_arguments)]
fn marker_pass(
    enc: &mut wgpu::CommandEncoder,
    device: &RenderDevice,
    cache: &PipelineCache,
    p: &FpvPipelines,
    marker: &TextureView,
    src: &TextureView,
    qs: &wgpu::QuerySet,
    index: u32,
) -> bool {
    let Some(pipe) = cache.get_render_pipeline(p.down) else { return false };
    let layout = cache.get_bind_group_layout(&p.down_layout);
    let bg = device.create_bind_group("fpv_marker_bg", &layout, &BindGroupEntries::sequential((src, &p.linear)));
    let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
        label: Some("fpv_marker"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: marker,
            depth_slice: None,
            resolve_target: None,
            ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: Some(wgpu::RenderPassTimestampWrites { query_set: qs, beginning_of_pass_write_index: None, end_of_pass_write_index: Some(index) }),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipe);
    pass.set_bind_group(0, &bg, &[]);
    pass.draw(0..3, 0..1);
    true
}

pub fn mach_ns() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        unsafe extern "C" {
            fn clock_gettime_nsec_np(clock_id: u32) -> u64;
        }
        // CLOCK_UPTIME_RAW = 8: the mach clock, same as Rust's Instant and SDL's timestamps.
        clock_gettime_nsec_np(8)
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
    }
}

fn harvest(timer: &mut GpuTimer, link: &TimingLink) {
    let mut out = link.0.lock().unwrap();
    // Oldest first, so each frame can be compared with the one before it.
    let mut order: Vec<usize> = (0..SLOTS).filter(|&s| timer.state[s].load(Ordering::Acquire) == MAPPED).collect();
    order.sort_by_key(|&s| timer.slot_counter[s]);
    for s in order {
        let ts: Vec<u64> = {
            let view = timer.readback[s].slice(..).get_mapped_range();
            view.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect()
        };
        timer.readback[s].unmap();
        timer.state[s].store(FREE, Ordering::Release);
        let w = timer.written[s];
        let p = timer.period_ns;
        let ns = |i: usize| ts[i] as f64 * p;
        let ms = |a: usize, b: usize| -> f32 {
            if w & (1 << a) != 0 && w & (1 << b) != 0 && ts[b] >= ts[a] {
                ((ns(b) - ns(a)) / 1e6) as f32
            } else {
                f32::NAN
            }
        };
        let has_mips = w & 0b100 != 0;
        // The previous frame's merged-pass end: with at most 3 frames in flight and the GPU busy,
        // "end of previous frame" to "end of this frame" is this frame's GPU time.
        let prev = timer.last_end.filter(|(c, _)| *c + 1 == timer.slot_counter[s]).map(|(_, t)| t);
        let since_prev = |b: usize| -> f32 {
            match prev {
                Some(t) if w & (1 << b) != 0 && ns(b) >= t => ((ns(b) - t) / 1e6) as f32,
                _ => f32::NAN,
            }
        };
        let fg = FrameGpu {
            frame_id: timer.frame_ids[s],
            total: since_prev(3),
            scene: since_prev(1),
            mips: if has_mips { ms(1, 2) } else { 0.0 },
            look: if has_mips { ms(2, 3) } else { ms(1, 3) },
            tail: ms(4, 3),
            end_minus_submit: ((ns(3) - timer.submit_ns[s] as f64) / 1e6) as f32,
        };
        timer.last_end = if w & (1 << 3) != 0 { Some((timer.slot_counter[s], ns(3))) } else { None };
        out.frames.push(fg);
        if out.frames.len() > 4000 {
            out.frames.drain(..1000);
        }
        if let Some(t0) = timer.latency_t0[s].take() {
            let present = timer.present_ns[s].take();
            // Metal's GPU clock isn't the CPU's mach clock on this machine, so "GPU done" is the CPU
            // time when the GPU's completion was noticed (an upper bound, usually within a frame).
            let done = timer.done_ns[s].load(Ordering::Acquire);
            out.latency.push(LatencySample {
                frame_id: fg.frame_id,
                to_present_ms: present.map(|p| (p.saturating_sub(t0)) as f32 / 1e6).unwrap_or(f32::NAN),
                to_gpu_done_ms: (done > t0).then(|| (done - t0) as f32 / 1e6),
            });
        }
    }
}

fn frame_begin(
    timer: Option<ResMut<GpuTimer>>,
    frame: Option<Res<FpvFrame>>,
    pipelines: Option<Res<FpvPipelines>>,
    cache: Res<PipelineCache>,
    link: Res<TimingLink>,
    device: Res<RenderDevice>,
    mut pending: ResMut<PendingCommandBuffers>,
) {
    let Some(mut timer) = timer else { return };
    let _ = (&pipelines, &cache, &mut pending);
    harvest(&mut timer, &link);
    timer.counter += 1;
    let s = (timer.counter as usize) % SLOTS;
    let Some(_qs) = timer.qs.clone() else {
        timer.slot = None;
        return;
    };
    // At most MAX_IN_FLIGHT frames on the GPU, like a swapchain allows: wait for older ones.
    // (Without a window nothing else stops the CPU from running far ahead of the GPU.)
    let max = MAX_IN_FLIGHT.load(Ordering::Relaxed).clamp(1, SLOTS);
    let mut guard = 0;
    while (timer.state[s].load(Ordering::Acquire) == IN_FLIGHT
        || timer.state.iter().filter(|st| st.load(Ordering::Acquire) == IN_FLIGHT).count() >= max)
        && guard < 1000
    {
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        guard += 1;
    }
    harvest(&mut timer, &link);
    if timer.state[s].load(Ordering::Acquire) != FREE {
        timer.slot = None;
        return;
    }
    timer.slot = Some(s);
    timer.slot_counter[s] = timer.counter;
    timer.written[s] = 0;
    timer.frame_ids[s] = frame.as_ref().map(|f| f.frame_id).unwrap_or(0);
    timer.latency_t0[s] = frame.as_ref().and_then(|f| f.latency_t0_ns);
    timer.present_ns[s] = None;
}

fn frame_end(timer: Option<ResMut<GpuTimer>>, device: Res<RenderDevice>, mut pending: ResMut<PendingCommandBuffers>) {
    let Some(mut timer) = timer else { return };
    let (Some(s), Some(qs), Some(resolve)) = (timer.slot, timer.qs.clone(), timer.resolve.clone()) else {
        return;
    };
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fpv_ts_end") });
    let base = s as u32 * N_TS;
    let off = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * s as u64;
    enc.resolve_query_set(&qs, base..base + N_TS, &resolve, off);
    enc.copy_buffer_to_buffer(&resolve, off, &timer.readback[s], 0, N_TS as u64 * 8);
    let flag = timer.state[s].clone();
    flag.store(IN_FLIGHT, Ordering::Release);
    let done_at = timer.done_ns[s].clone();
    enc.map_buffer_on_submit(&timer.readback[s], MapMode::Read, .., move |r| {
        // The CPU learns here that the GPU finished this frame (an upper bound on when it did).
        done_at.store(mach_ns(), Ordering::Release);
        flag.store(if r.is_ok() { MAPPED } else { FREE }, Ordering::Release);
    });
    timer.submit_ns[s] = mach_ns();
    pending.push_encoder(enc);
}

fn after_present(timer: Option<ResMut<GpuTimer>>) {
    let Some(mut timer) = timer else { return };
    if let Some(s) = timer.slot {
        if timer.latency_t0[s].is_some() {
            timer.present_ns[s] = Some(mach_ns());
        }
    }
}

fn display_pass(
    view: ViewQuery<&ViewTarget, With<FpvDisplay>>,
    frame: Option<Res<FpvFrame>>,
    pipelines: Option<Res<FpvPipelines>>,
    timer: Option<ResMut<GpuTimer>>,
    cache: Res<PipelineCache>,
    mut warned: Local<bool>,
    mut diag_state: Local<(u32, u32)>,
    mut ctx: RenderContext,
) {
    let view_target = view.into_inner();
    let (Some(frame), Some(p)) = (frame, pipelines) else {
        diag(&mut diag_state, 1, "no frame/pipelines resource");
        return;
    };
    let Some(src) = frame.source.as_ref() else {
        diag(&mut diag_state, 2, "no source texture");
        return;
    };
    let (Some(look_pipe), Some(down_pipe)) =
        (cache.get_render_pipeline(p.look), cache.get_render_pipeline(p.down))
    else {
        diag(&mut diag_state, 3, &format!("pipelines not ready: look {:?} down {:?}", cache.get_render_pipeline_state(p.look), cache.get_render_pipeline_state(p.down)));
        return;
    };
    let Some(uniform) = p.uniform.binding() else {
        diag(&mut diag_state, 4, "no uniform");
        return;
    };
    diag(&mut diag_state, 5, "display pass running");
    if view_target.main_texture_format() != TextureFormat::Rgba8UnormSrgb && !*warned {
        *warned = true;
        warn!("display camera main texture is {:?}, pipeline expects Rgba8UnormSrgb", view_target.main_texture_format());
    }
    let mut timer = timer;
    let ts = timer.as_ref().and_then(|t| t.slot.zip(t.qs.clone()));
    let device = ctx.render_device().clone();

    // 0. Marker: the Map render (mip 0) is done.
    if let (Some(t), Some((s, qs))) = (timer.as_mut(), ts.as_ref()) {
        let marker = t.marker.clone();
        if marker_pass(ctx.command_encoder(), &device, &cache, &p, &marker, &src.mips[0], qs, *s as u32 * N_TS + 1) {
            t.written[*s] |= 1 << 1;
        }
    }

    // 1. Mip chain of the source, so the warp can filter the squashed edges.
    let n = src.mips.len();
    if frame.mips_enabled && n > 1 {
        let down_layout = cache.get_bind_group_layout(&p.down_layout);
        for i in 1..n {
            let bg = device.create_bind_group("fpv_down_bg", &down_layout, &BindGroupEntries::sequential((&src.mips[i - 1], &p.linear)));
            let tsw = ts.as_ref().and_then(|(s, qs)| {
                (i == n - 1).then_some(wgpu::RenderPassTimestampWrites {
                    query_set: qs,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(*s as u32 * N_TS + 2),
                })
            });
            let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
                label: Some("fpv_downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &src.mips[i],
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: tsw,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(down_pipe);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..3, 0..1);
        }
        if let (Some(t), Some((s, _))) = (timer.as_mut(), ts.as_ref()) {
            t.written[*s] |= 1 << 2;
        }
    }

    // 2. The one merged pass: fisheye warp + Video Look + Breakup.
    let look_layout = cache.get_bind_group_layout(&p.look_layout);
    let bg = device.create_bind_group(
        "fpv_look_bg",
        &look_layout,
        &BindGroupEntries::sequential((&src.full, &p.aniso, uniform, &p.trilinear)),
    );
    let reps = frame.repeat_look.max(1);
    for r in 0..reps {
        let tsw = ts.as_ref().and_then(|(s, qs)| {
            let b = (r == 0).then_some(*s as u32 * N_TS + 4);
            let e = (r == reps - 1).then_some(*s as u32 * N_TS + 3);
            (b.is_some() || e.is_some()).then_some(wgpu::RenderPassTimestampWrites {
                query_set: qs,
                beginning_of_pass_write_index: b,
                end_of_pass_write_index: e,
            })
        });
        let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
            label: Some("fpv_look"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: view_target.main_texture_view(),
                depth_slice: None,
                resolve_target: None,
                ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: tsw,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(look_pipe);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }
    if let (Some(t), Some((s, _))) = (timer.as_mut(), ts.as_ref()) {
        t.written[*s] |= (1 << 3) | (1 << 4);
    }
}

fn diag(state: &mut (u32, u32), code: u32, msg: &str) {
    state.1 += 1;
    if state.0 != code {
        state.0 = code;
        info!("[fpv display pass] frame {}: {msg}", state.1);
    }
}

/// Every 2 s: how many pipelines exist and how many are still compiling (Metal has no pipeline cache).
fn pipeline_census(
    cache: Res<PipelineCache>,
    link: Res<TimingLink>,
    mut last: Local<Option<std::time::Instant>>,
    mut prev: Local<usize>,
) {
    let waiting = cache.waiting_pipelines().count();
    {
        let mut l = link.0.lock().unwrap();
        l.pipelines_waiting = waiting;
        if waiting > 0 {
            l.pipelines_busy_since = Some(std::time::Instant::now());
        }
    }
    let now = std::time::Instant::now();
    if last.is_some_and(|t| now.duration_since(t).as_secs_f32() < 2.0) {
        return;
    }
    *last = Some(now);
    let total = cache.pipelines().count();
    if total != *prev || waiting > 0 {
        info!("[pipelines] {total} total, {waiting} still compiling");
    }
    *prev = total;
}
