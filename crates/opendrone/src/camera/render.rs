//! The FPV Camera's GPU side (ADR-0009).
//!
//! - The Map is drawn once per frame by a normal camera into a flat render,
//!   wider than the picture ([`SourceTexture`]). Bevy's own Auto-exposure and
//!   tonemapping run there.
//! - A second camera, [`FpvDisplay`], then runs two steps of ours: a mip chain
//!   of the flat render, so the warp can filter the squashed fisheye edges,
//!   and ONE merged pass that does the Lens and the whole Video Look straight
//!   into the window.
//! - When asked ([`CostProbe`]), the GPU's own timestamps around those two
//!   steps measure what the camera costs a frame.
//!
//! Reimplemented from the #28 prototype's render code on Bevy 0.20.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

use bevy::asset::{embedded_asset, load_embedded_asset};
use bevy::camera::ManualTextureViewHandle;

use super::params::FpvParams;
use bevy::core_pipeline::{
    Core2dSystems, FullscreenShader, core_2d::main_transparent_pass_2d, schedule::Core2d,
};
use bevy::prelude::*;
use bevy::render::{
    Render, RenderApp, RenderStartup, RenderSystems,
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_resource::{
        AddressMode, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
        BufferDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites,
        Extent3d, FilterMode, FragmentState, LoadOp, MapMode, MipmapFilterMode, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
        Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StoreOp, Texture,
        TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureView, TextureViewDescriptor, UniformBuffer, VertexState,
        binding_types::{sampler, texture_2d, uniform_buffer},
    },
    renderer::{
        PendingCommandBuffers, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems,
        RenderQueue, ViewQuery,
    },
    texture::{ManualTextureView, ManualTextureViews},
    view::ViewTarget,
};
use bevy::shader::Shader;

/// The flat render's handle as a camera's render target.
pub const SOURCE_VIEW: ManualTextureViewHandle = ManualTextureViewHandle(2800);

/// The flat render's pixel format.
const SOURCE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// The flat render and its views, one per mip level.
#[derive(Clone)]
pub struct SourceViews {
    pub full: TextureView,
    pub mips: Vec<TextureView>,
    pub size: UVec2,
}

/// The main world's hold on the flat render's texture, so it lives as long as
/// the camera draws into it.
#[derive(Resource, Default)]
pub struct SourceTexture {
    texture: Option<Texture>,
    views: Option<SourceViews>,
}

impl SourceTexture {
    pub fn views(&self) -> Option<&SourceViews> {
        self.views.as_ref()
    }

    pub fn size(&self) -> UVec2 {
        self.views.as_ref().map_or(UVec2::ZERO, |v| v.size)
    }

    /// Makes the flat render this size, if it isn't already, and offers its
    /// first mip level to the camera as [`SOURCE_VIEW`].
    pub fn ensure(&mut self, device: &RenderDevice, manual: &mut ManualTextureViews, size: UVec2) {
        if self.texture.is_some() && self.size() == size {
            return;
        }
        let mip_count = 32 - size.x.max(size.y).max(1).leading_zeros();
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("fpv_source"),
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: SOURCE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let full = texture.create_view(&TextureViewDescriptor::default());
        let mips: Vec<TextureView> = (0..mip_count)
            .map(|level| {
                texture.create_view(&TextureViewDescriptor {
                    label: Some("fpv_source_mip"),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..default()
                })
            })
            .collect();
        manual.insert(
            SOURCE_VIEW,
            ManualTextureView {
                texture_view: mips[0].clone(),
                size,
                view_format: SOURCE_FORMAT,
            },
        );
        self.texture = Some(texture);
        self.views = Some(SourceViews { full, mips, size });
    }
}

/// What the render world is handed each frame.
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct FpvFrame {
    pub params: FpvParams,
    pub source: Option<SourceViews>,
}

/// Marks the camera that runs the merged pass into the window.
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct FpvDisplay;

/// Asks for the camera steps' GPU time to be measured (a developer
/// command's job, not the game's).
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct CostProbe(pub bool);

/// One frame's GPU time, in milliseconds, of the camera's own steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraCost {
    /// The mip chain of the flat render.
    pub mips: f32,
    /// The merged pass.
    pub look: f32,
}

impl CameraCost {
    /// Both steps together: what counts against the 2 ms (#28 §6).
    pub fn camera_steps(&self) -> f32 {
        self.mips + self.look
    }
}

/// What the GPU timer has read back, shared between the render world and the
/// main world.
#[derive(Default)]
pub struct CostSamples {
    /// Whether the GPU can take timestamps. `None` until the renderer starts.
    pub supported: Option<bool>,
    pub frames: Vec<CameraCost>,
}

#[derive(Resource, Clone, Default)]
pub struct CostLink(pub Arc<Mutex<CostSamples>>);

/// The camera's render plugin. Add it with Bevy's rendering.
pub struct FpvRenderPlugin;

impl Plugin for FpvRenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "fpv_look.wesl");
        embedded_asset!(app, "fpv_downsample.wesl");
        let link = CostLink::default();
        app.insert_resource(link.clone())
            .init_resource::<SourceTexture>()
            .init_resource::<FpvFrame>()
            .init_resource::<CostProbe>()
            .add_plugins((
                ExtractResourcePlugin::<FpvFrame>::default(),
                ExtractResourcePlugin::<CostProbe>::default(),
                ExtractComponentPlugin::<FpvDisplay>::default(),
            ));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(link)
            .add_systems(RenderStartup, (init_pipelines, init_timer))
            .add_systems(Render, prepare_uniform.in_set(RenderSystems::Prepare))
            .add_systems(RenderGraph, frame_begin.in_set(RenderGraphSystems::Begin))
            .add_systems(
                RenderGraph,
                frame_end
                    .after(RenderGraphSystems::Render)
                    .before(RenderGraphSystems::Submit),
            )
            .add_systems(
                Core2d,
                display_pass
                    .in_set(Core2dSystems::MainPass)
                    .after(main_transparent_pass_2d),
            );
    }
}

#[derive(Resource)]
struct FpvPipelines {
    look_layout: BindGroupLayoutDescriptor,
    down_layout: BindGroupLayoutDescriptor,
    /// The merged pass, once for each format the display camera's main
    /// texture has had: the window's, usually the only one.
    look: Vec<(TextureFormat, CachedRenderPipelineId)>,
    look_shader: Handle<Shader>,
    vertex: VertexState,
    down: CachedRenderPipelineId,
    /// Anisotropic, for the fisheye's squashed edges.
    aniso: Sampler,
    linear: Sampler,
    /// Plain trilinear, for Analog's soft taps.
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
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let target = |format| {
        vec![Some(ColorTargetState {
            format,
            blend: None,
            write_mask: ColorWrites::ALL,
        })]
    };
    let down = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("fpv_downsample_pipeline".into()),
        layout: vec![down_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "fpv_downsample.wesl"),
            targets: target(SOURCE_FORMAT),
            ..default()
        }),
        ..default()
    });
    let sampler_of = |label, mipmap, anisotropy| {
        device.create_sampler(&SamplerDescriptor {
            label: Some(label),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: mipmap,
            anisotropy_clamp: anisotropy,
            ..default()
        })
    };
    let mut pipelines = FpvPipelines {
        look_layout,
        down_layout,
        look: Vec::new(),
        look_shader: load_embedded_asset!(asset_server.as_ref(), "fpv_look.wesl"),
        vertex: fullscreen.to_vertex_state(),
        down,
        aniso: sampler_of("fpv_aniso", MipmapFilterMode::Linear, 16),
        linear: sampler_of("fpv_linear", MipmapFilterMode::Nearest, 1),
        trilinear: sampler_of("fpv_trilinear", MipmapFilterMode::Linear, 1),
        uniform: UniformBuffer::default(),
    };
    // The window's usual format: sRGB, so the merged pass writes linear light.
    pipelines.look_pipeline(&cache, TextureFormat::Rgba8UnormSrgb);
    commands.insert_resource(pipelines);
}

impl FpvPipelines {
    /// The merged pass's pipeline for a target format, queued the first time
    /// the format comes up.
    fn look_pipeline(
        &mut self,
        cache: &PipelineCache,
        format: TextureFormat,
    ) -> CachedRenderPipelineId {
        if let Some((_, id)) = self.look.iter().find(|(f, _)| *f == format) {
            return *id;
        }
        let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("fpv_look_pipeline".into()),
            layout: vec![self.look_layout.clone()],
            vertex: self.vertex.clone(),
            fragment: Some(FragmentState {
                shader: self.look_shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        });
        self.look.push((format, id));
        id
    }
}

fn prepare_uniform(
    frame: Option<Res<FpvFrame>>,
    pipelines: Option<ResMut<FpvPipelines>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(frame), Some(mut pipelines)) = (frame, pipelines) else {
        return;
    };
    pipelines.uniform.set(frame.params);
    pipelines.uniform.write_buffer(&device, &queue);
}

// ---------------------------------------------------------------------------------------------
// The GPU timer.
//
// Timestamps per frame, all taken at the END of a pass: Apple GPUs start a pass's vertex work
// early, so the start of a pass overlaps the one before it, while the end of each pass is
// reliable because each of these reads what the one before wrote.
//   0  the Map's render is done (a tiny marker pass that reads it)
//   1  the mip chain is done
//   2  the merged pass is done
//
// The queries of a frame are resolved at the end of the NEXT frame. The last pass of a frame has
// nothing after it that reads it, and Metal may finish its fragment work after a resolve in the
// same submission: the resolve then reads the sample of the frame before. (Marker passes after
// the merged pass, even in the resolve's own command buffer, didn't help: the samples of any
// pass after the merged pass came late.) A frame later, the GPU is done with it. That also
// means the timer can't be used in the last frame of a run: it is never resolved.

const TIMESTAMPS: u32 = 3;
/// Frames the timer follows at once.
const SLOTS: usize = 3;

const FREE: u8 = 0;
const IN_FLIGHT: u8 = 1;
const MAPPED: u8 = 2;

#[derive(Resource)]
struct GpuTimer {
    queries: Option<wgpu::QuerySet>,
    resolve: Option<Buffer>,
    readback: Vec<Buffer>,
    state: Vec<Arc<AtomicU8>>,
    /// Which timestamps each slot's frame wrote, as bits.
    written: Vec<u8>,
    period_ns: f64,
    /// The slot this frame writes to, if it is being timed.
    slot: Option<usize>,
    counter: u64,
    marker: TextureView,
    /// The slot whose queries are resolved at the end of this frame.
    to_resolve: Option<usize>,
}

fn init_timer(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    link: Res<CostLink>,
) {
    let supported = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    link.0.lock().expect("the cost link").supported = Some(supported);
    let (queries, resolve, readback) = if supported {
        let queries = device
            .wgpu_device()
            .create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("fpv_timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: TIMESTAMPS * SLOTS as u32,
            });
        let resolve = device.create_buffer(&BufferDescriptor {
            label: Some("fpv_timestamps_resolve"),
            size: wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * SLOTS as u64,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = (0..SLOTS)
            .map(|_| {
                device.create_buffer(&BufferDescriptor {
                    label: Some("fpv_timestamps_readback"),
                    size: u64::from(TIMESTAMPS) * 8,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        (Some(queries), Some(resolve), readback)
    } else {
        (None, None, Vec::new())
    };
    let marker = device
        .create_texture(&TextureDescriptor {
            label: Some("fpv_marker"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: SOURCE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&TextureViewDescriptor::default());
    commands.insert_resource(GpuTimer {
        queries,
        resolve,
        readback,
        state: (0..SLOTS).map(|_| Arc::new(AtomicU8::new(FREE))).collect(),
        written: vec![0; SLOTS],
        period_ns: f64::from(queue.get_timestamp_period()),
        slot: None,
        counter: 0,
        marker,
        to_resolve: None,
    });
}

/// Reads back what the GPU finished, oldest first.
fn harvest(timer: &mut GpuTimer, link: &CostLink) {
    let done: Vec<usize> = (0..SLOTS)
        .filter(|&s| timer.state[s].load(Ordering::Acquire) == MAPPED)
        .collect();
    for slot in done {
        let stamps: Vec<u64> = match timer.readback[slot].slice(..).get_mapped_range() {
            Ok(view) => view
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_le_bytes(*c))
                .collect(),
            Err(_) => Vec::new(),
        };
        timer.readback[slot].unmap();
        timer.state[slot].store(FREE, Ordering::Release);
        let wrote = timer.written[slot];
        if stamps.len() == TIMESTAMPS as usize && wrote == 0b111 {
            let ms = |a: usize, b: usize| {
                if stamps[b] >= stamps[a] {
                    ((stamps[b] - stamps[a]) as f64 * timer.period_ns / 1e6) as f32
                } else {
                    f32::NAN
                }
            };
            let cost = CameraCost {
                mips: ms(0, 1),
                look: ms(1, 2),
            };
            if cost.mips.is_finite() && cost.look.is_finite() {
                let mut samples = link.0.lock().expect("the cost link");
                samples.frames.push(cost);
                if samples.frames.len() > 20_000 {
                    samples.frames.drain(..5_000);
                }
            }
        }
    }
}

fn frame_begin(
    timer: Option<ResMut<GpuTimer>>,
    probe: Option<Res<CostProbe>>,
    link: Res<CostLink>,
    device: Res<RenderDevice>,
) {
    let Some(mut timer) = timer else { return };
    timer.slot = None;
    if !probe.is_some_and(|p| p.0) || timer.queries.is_none() {
        return;
    }
    harvest(&mut timer, &link);
    timer.counter += 1;
    let slot = (timer.counter as usize) % SLOTS;
    // Without a window nothing stops the CPU running far ahead of the GPU:
    // wait for this slot's older frame, as a swapchain would.
    let mut guard = 0;
    while timer.state[slot].load(Ordering::Acquire) == IN_FLIGHT && guard < 1000 {
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        guard += 1;
    }
    harvest(&mut timer, &link);
    if timer.state[slot].load(Ordering::Acquire) != FREE {
        return;
    }
    timer.slot = Some(slot);
    timer.written[slot] = 0;
}

fn frame_end(
    timer: Option<ResMut<GpuTimer>>,
    device: Res<RenderDevice>,
    mut pending: ResMut<PendingCommandBuffers>,
) {
    let Some(mut timer) = timer else { return };
    // Resolve the frame BEFORE this one (see the note on the GPU timer).
    if let (Some(slot), Some(queries), Some(resolve)) = (
        timer.to_resolve.take(),
        timer.queries.clone(),
        timer.resolve.clone(),
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fpv_timestamps_end"),
        });
        let first = slot as u32 * TIMESTAMPS;
        let offset = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT * slot as u64;
        encoder.resolve_query_set(&queries, first..first + TIMESTAMPS, &resolve, offset);
        encoder.copy_buffer_to_buffer(
            &resolve,
            offset,
            &timer.readback[slot],
            0,
            u64::from(TIMESTAMPS) * 8,
        );
        let flag = timer.state[slot].clone();
        flag.store(IN_FLIGHT, Ordering::Release);
        encoder.map_buffer_on_submit(&timer.readback[slot], MapMode::Read, .., move |result| {
            flag.store(
                if result.is_ok() { MAPPED } else { FREE },
                Ordering::Release,
            );
        });
        pending.push_encoder(encoder, "fpv_timestamps_end");
    }
    timer.to_resolve = timer.slot;
}

/// A tiny draw into a 1×1 texture that reads `source`, with a timestamp at
/// its end. It can only finish after whatever wrote `source`, so its end
/// marks that work done.
fn marker_pass(
    encoder: &mut wgpu::CommandEncoder,
    device: &RenderDevice,
    cache: &PipelineCache,
    pipelines: &FpvPipelines,
    timer: &GpuTimer,
    source: &TextureView,
    stamp: (&wgpu::QuerySet, u32),
) -> bool {
    let Some(pipeline) = cache.get_render_pipeline(pipelines.down) else {
        return false;
    };
    let layout = cache.get_bind_group_layout(&pipelines.down_layout);
    let bind_group = device.create_bind_group(
        "fpv_marker_bind_group",
        &layout,
        &BindGroupEntries::sequential((source, &pipelines.linear)),
    );
    let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("fpv_marker"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &timer.marker,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Default::default()),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
            query_set: stamp.0,
            beginning_of_pass_write_index: None,
            end_of_pass_write_index: Some(stamp.1),
        }),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
    true
}

/// The camera's two steps: the flat render's mip chain, then the merged pass.
fn display_pass(
    view: ViewQuery<&ViewTarget, With<FpvDisplay>>,
    frame: Option<Res<FpvFrame>>,
    pipelines: Option<ResMut<FpvPipelines>>,
    timer: Option<ResMut<GpuTimer>>,
    cache: Res<PipelineCache>,
    mut warned: Local<bool>,
    mut ctx: RenderContext,
) {
    let view_target = view.into_inner();
    if !view_target.main_texture_format().is_srgb() && !*warned {
        *warned = true;
        warn!(
            "the window's picture is {:?}, not sRGB: the camera's light will look too dark",
            view_target.main_texture_format()
        );
    }
    let (Some(frame), Some(mut pipelines)) = (frame, pipelines) else {
        return;
    };
    let look_id = pipelines.look_pipeline(&cache, view_target.main_texture_format());
    let pipelines = pipelines.into_inner();
    let Some(source) = frame.source.as_ref() else {
        return;
    };
    let (Some(look_pipeline), Some(down_pipeline)) = (
        cache.get_render_pipeline(look_id),
        cache.get_render_pipeline(pipelines.down),
    ) else {
        return;
    };
    let Some(uniform) = pipelines.uniform.binding() else {
        return;
    };
    let mut timer = timer;
    let timed = timer.as_ref().and_then(|t| t.slot.zip(t.queries.clone()));
    let device = ctx.render_device().clone();

    // The Map's render is done.
    if let (Some(t), Some((slot, queries))) = (timer.as_mut(), timed.as_ref())
        && marker_pass(
            ctx.command_encoder(),
            &device,
            &cache,
            pipelines,
            t,
            &source.mips[0],
            (queries, *slot as u32 * TIMESTAMPS),
        )
    {
        t.written[*slot] |= 0b001;
    }

    // Step 1: the mip chain.
    let levels = source.mips.len();
    let down_layout = cache.get_bind_group_layout(&pipelines.down_layout);
    for level in 1..levels {
        let bind_group = device.create_bind_group(
            "fpv_down_bind_group",
            &down_layout,
            &BindGroupEntries::sequential((&source.mips[level - 1], &pipelines.linear)),
        );
        let timestamps = timed.as_ref().and_then(|(slot, queries)| {
            (level == levels - 1).then_some(wgpu::RenderPassTimestampWrites {
                query_set: queries,
                beginning_of_pass_write_index: None,
                end_of_pass_write_index: Some(*slot as u32 * TIMESTAMPS + 1),
            })
        });
        let mut pass = ctx
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("fpv_downsample"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &source.mips[level],
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Default::default()),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timestamps,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        pass.set_pipeline(down_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    if let (Some(t), Some((slot, _))) = (timer.as_mut(), timed.as_ref())
        && levels > 1
    {
        t.written[*slot] |= 0b010;
    }

    // Step 2: the merged pass, Lens and Video Look in one.
    let look_layout = cache.get_bind_group_layout(&pipelines.look_layout);
    let bind_group = device.create_bind_group(
        "fpv_look_bind_group",
        &look_layout,
        &BindGroupEntries::sequential((
            &source.full,
            &pipelines.aniso,
            uniform,
            &pipelines.trilinear,
        )),
    );
    let timestamps = timed
        .as_ref()
        .map(|(slot, queries)| wgpu::RenderPassTimestampWrites {
            query_set: queries,
            beginning_of_pass_write_index: None,
            end_of_pass_write_index: Some(*slot as u32 * TIMESTAMPS + 2),
        });
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("fpv_look"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: view_target.main_texture_view(),
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Default::default()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(look_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
    drop(pass);
    if let (Some(t), Some((slot, _))) = (timer.as_mut(), timed.as_ref()) {
        t.written[*slot] |= 0b100;
    }
}
