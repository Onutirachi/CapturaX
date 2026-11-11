//! The GPU side of the video thread, ported from the stack spike: presentation choice (research R4),
//! surface, layers and the composite (video letterboxed with rounded corners, UI over it).

use std::fmt;
use std::sync::Arc;

use winit::window::Window;

use capture::{Frame, Matrix, PixelFormat, Range};

use super::{Presentation, Scene};
use crate::cli::{Backend, PresentMode};
use crate::ui::UiImage;

/// No backend and present mode could be set up (spec FR-036).
#[derive(Debug)]
pub struct RendererError(pub String);

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RendererError {}

/// The backends and present modes to try, in order (research R4). The platform's order follows
/// the backends this build has: Direct3D 12 on Windows, Vulkan then OpenGL on Linux. A forced
/// backend or present mode is the only one tried, even if this build lacks it, so a forced choice
/// never falls back.
fn order(built: &[Backend], forced: Presentation) -> Vec<(Backend, Vec<PresentMode>)> {
    use PresentMode::{Fifo, Immediate, Mailbox};
    let has_dx12 = built.contains(&Backend::Dx12);
    let platform = [
        (Backend::Dx12, vec![Mailbox, Immediate, Fifo]),
        // Vulkan is the fallback on Windows (no Immediate: it tears through the compositor) and
        // the first choice on Linux.
        (
            Backend::Vulkan,
            if has_dx12 {
                vec![Mailbox, Fifo]
            } else {
                vec![Mailbox, Immediate, Fifo]
            },
        ),
        (Backend::Gl, vec![Fifo]),
    ];
    let mut plan: Vec<(Backend, Vec<PresentMode>)> = match forced.backend {
        Some(backend) => {
            let modes = platform
                .iter()
                .find(|(b, _)| *b == backend)
                .map(|(_, m)| m.clone())
                .unwrap_or_default();
            vec![(backend, modes)]
        }
        None => platform.into_iter().filter(|(b, _)| built.contains(b)).collect(),
    };
    if let Some(present) = forced.present {
        for (_, modes) in &mut plan {
            *modes = vec![present];
        }
    }
    plan
}

/// The backends this build of wgpu has.
fn built_backends() -> Vec<Backend> {
    let enabled = wgpu::Instance::enabled_backend_features();
    [
        (Backend::Dx12, wgpu::Backends::DX12),
        (Backend::Vulkan, wgpu::Backends::VULKAN),
        (Backend::Gl, wgpu::Backends::GL),
    ]
    .into_iter()
    .filter(|(_, b)| enabled.contains(*b))
    .map(|(backend, _)| backend)
    .collect()
}

fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Dx12 => "dx12",
        Backend::Vulkan => "vulkan",
        Backend::Gl => "gl",
    }
}

fn mode_name(mode: PresentMode) -> &'static str {
    match mode {
        PresentMode::Mailbox => "mailbox",
        PresentMode::Immediate => "immediate",
        PresentMode::Fifo => "fifo",
    }
}

fn wgpu_mode(mode: PresentMode) -> wgpu::PresentMode {
    match mode {
        PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        PresentMode::Immediate => wgpu::PresentMode::Immediate,
        PresentMode::Fifo => wgpu::PresentMode::Fifo,
    }
}

/// Logs the Vulkan swap chain's image count (research R9): wgpu asks for the frame latency plus
/// one images, but never fewer than the driver's minimum.
fn log_image_count(adapter: &wgpu::Adapter, surface: &wgpu::Surface<'_>, latency: u32) {
    use wgpu::hal::Adapter as _;
    use wgpu::hal::api::Vulkan;
    // SAFETY: the handles are only read for their capabilities; nothing is destroyed or kept.
    let caps = unsafe {
        match (adapter.as_hal::<Vulkan>(), surface.as_hal::<Vulkan>()) {
            (Some(adapter), Some(surface)) => adapter.surface_capabilities(&surface),
            _ => None,
        }
    };
    match caps {
        Some(caps) => {
            let minimum = caps.maximum_frame_latency.start() + 1;
            log::info!(
                "presentation: vulkan swap chain of {} images (frame latency {latency}, driver minimum {minimum})",
                (latency + 1).max(minimum)
            );
        }
        None => log::info!("presentation: vulkan swap chain image count unknown"),
    }
}

/// What a layer's textures hold; the shader's `picture.x` (composite.wgsl).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Premultiplied RGBA with its alpha (the UI).
    Ui,
    /// Opaque RGB (BGRx, RGBA video).
    Rgb(PixelFormat),
    Nv12,
    I420,
    Yuy2,
    Uyvy,
    /// A coverage mask drawn in one color (the watermark).
    Mask,
    /// The video's shadow, computed in the shader (its texture is unused).
    Shadow,
}

impl Kind {
    fn of(format: PixelFormat) -> Self {
        match format {
            PixelFormat::Nv12 => Self::Nv12,
            PixelFormat::I420 => Self::I420,
            PixelFormat::Yuy2 => Self::Yuy2,
            PixelFormat::Uyvy => Self::Uyvy,
            PixelFormat::Bgrx | PixelFormat::Rgba => Self::Rgb(format),
        }
    }

    fn code(self) -> f32 {
        match self {
            Self::Ui => 0.0,
            Self::Rgb(_) => 1.0,
            Self::Nv12 => 2.0,
            Self::I420 => 3.0,
            Self::Yuy2 => 4.0,
            Self::Uyvy => 5.0,
            Self::Mask => 6.0,
            Self::Shadow => 7.0,
        }
    }

    /// The texture of each plane: format, width and height in texels, bytes per texel.
    fn planes(self, width: u32, height: u32) -> Vec<(wgpu::TextureFormat, u32, u32, u32)> {
        use wgpu::TextureFormat as T;
        let (half_w, half_h) = (width.div_ceil(2), height.div_ceil(2));
        match self {
            Self::Ui | Self::Rgb(PixelFormat::Rgba) => vec![(T::Rgba8Unorm, width, height, 4)],
            Self::Rgb(_) => vec![(T::Bgra8Unorm, width, height, 4)],
            Self::Nv12 => vec![(T::R8Unorm, width, height, 1), (T::Rg8Unorm, half_w, half_h, 2)],
            Self::Mask | Self::Shadow => vec![(T::R8Unorm, width, height, 1)],
            Self::I420 => vec![
                (T::R8Unorm, width, height, 1),
                (T::R8Unorm, half_w, half_h, 1),
                (T::R8Unorm, half_w, half_h, 1),
            ],
            Self::Yuy2 | Self::Uyvy => vec![(T::Rgba8Unorm, half_w, height, 4)],
        }
    }
}

/// One plane's texture with its size in texels and bytes per texel.
struct PlaneTexture {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    texel: u32,
}

/// Textures drawn as one rectangle (video or UI layer).
struct Layer {
    planes: Vec<PlaneTexture>,
    bind_group: wgpu::BindGroup,
    rect: wgpu::Buffer,
    kind: Kind,
    /// The picture's size in pixels.
    width: u32,
    height: u32,
    /// The YUV to RGB rows of the last frame uploaded.
    rows: [[f32; 4]; 3],
}

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    video: Option<Layer>,
    ui: Option<Layer>,
    /// The watermark's mask, uploaded once when first drawn.
    watermark: Option<Layer>,
    /// The video's shadow, made when first drawn.
    shadow: Option<Layer>,
    /// For the `[present]` line of contracts/stats.md.
    backend: Backend,
    adapter: String,
    /// The present mode chosen at start (research R4), used with Lowest latency.
    mode: PresentMode,
    /// The surface presents in Fifo for Follow the monitor (research R9).
    follow: bool,
    /// The adapter's PCI bus id when the backend gives one (e.g. `0000:01:00.0`).
    pci_bus_id: Option<String>,
    /// Presents so far, for the present log's `frame`.
    presents: u64,
}

/// The present mode's name in the `[present]` line.
fn header_mode(mode: PresentMode, follow: bool) -> &'static str {
    if follow { "fifo-follow" } else { mode_name(mode) }
}

impl Gpu {
    /// Sets up the first backend and present mode of the order that works for `window`.
    pub fn new(window: Arc<Window>, forced: Presentation) -> Result<Self, RendererError> {
        let mut failures = Vec::new();
        for (backend, modes) in order(&built_backends(), forced) {
            match Self::with_backend(&window, backend, &modes) {
                Ok(gpu) => return Ok(gpu),
                Err(err) => {
                    log::warn!("presentation: {} unavailable: {err}", backend_name(backend));
                    failures.push(format!("{}: {err}", backend_name(backend)));
                }
            }
        }
        if failures.is_empty() {
            failures.push("no backend in this build".into());
        }
        Err(RendererError(failures.join("; ")))
    }

    fn with_backend(window: &Arc<Window>, backend: Backend, modes: &[PresentMode]) -> Result<Self, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = match backend {
            Backend::Dx12 => wgpu::Backends::DX12,
            Backend::Vulkan => wgpu::Backends::VULKAN,
            Backend::Gl => wgpu::Backends::GL,
        };
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let mode = modes
            .iter()
            .copied()
            .find(|m| caps.present_modes.contains(&wgpu_mode(*m)))
            .ok_or_else(|| format!("no present mode of {modes:?} (supported {:?})", caps.present_modes))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| e.to_string())?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface not supported by the adapter")?;
        // Plain (non-sRGB) format: the video bytes are shown as they are.
        if let Some(format) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *format;
        }
        config.present_mode = wgpu_mode(mode);
        // At most one frame waiting in the swap chain.
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        if backend == Backend::Vulkan {
            log_image_count(&adapter, &surface, config.desired_maximum_frame_latency);
        }
        let info = adapter.get_info();
        let gpu = Self::with_device(surface, device, queue, config, backend, info.name, mode);
        log::info!("presentation: {}", gpu.header());
        Ok(Self {
            pci_bus_id: Some(info.device_pci_bus_id).filter(|id| !id.is_empty()),
            ..gpu
        })
    }

    fn with_device(
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        backend: Backend,
        adapter: String,
        mode: PresentMode,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("composite.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    // Bounds for the vertices; size and radius for the rounded corners; the kind
                    // and the YUV to RGB rows.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // The second and third planes (the first one's view again when there are none).
                plane_entry(3),
                plane_entry(4),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layer"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("layer"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            sampler,
            video: None,
            ui: None,
            watermark: None,
            shadow: None,
            backend,
            adapter,
            mode,
            follow: false,
            presents: 0,
            pci_bus_id: None,
        }
    }

    /// The `[present]` header of the presentation in use.
    pub fn header(&self) -> String {
        crate::stats::header(
            backend_name(self.backend),
            header_mode(self.mode, self.follow),
            &self.adapter,
            (self.config.width, self.config.height),
        )
    }

    /// Follow the monitor (research R9): the surface presents in Fifo, so acquiring an image waits
    /// for about the next refresh; else it goes back to the mode chosen at start. The frame
    /// latency stays 1 either way.
    pub fn set_follow(&mut self, follow: bool) {
        if follow == self.follow {
            return;
        }
        self.follow = follow;
        self.config.present_mode = wgpu_mode(if follow { PresentMode::Fifo } else { self.mode });
        self.surface.configure(&self.device, &self.config);
        log::info!("presentation: {}", self.header());
    }

    /// The next surface image; `None` when the surface is not ready (it is configured again).
    /// In Fifo this blocks until about the next refresh.
    pub fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        crate::present_log::acquire();
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
            _ => {
                self.surface.configure(&self.device, &self.config);
                None
            }
        }
    }

    /// The adapter's PCI bus id, when the backend gives one.
    pub fn pci_bus_id(&self) -> Option<&str> {
        self.pci_bus_id.as_deref()
    }

    /// Reconfigures the surface when the window's size changed.
    fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn set_ui(&mut self, ui: &UiImage) {
        if self
            .ui
            .as_ref()
            .is_none_or(|l| (l.width, l.height) != (ui.width, ui.height))
        {
            self.ui = Some(self.make_layer(Kind::Ui, ui.width, ui.height));
        }
        let layer = self.ui.as_ref().expect("just made");
        self.upload(&layer.planes[0], &ui.pixels);
    }

    /// A video frame is on the GPU.
    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }

    /// Drops the video layer: only the UI and the ground are drawn until the next frame.
    pub fn clear_video(&mut self) {
        if self.video.take().is_some() {
            log::info!("video layer cleared");
        }
    }

    /// Uploads a frame's planes as they come (research R4); the textures are made again when the
    /// format or the size changes.
    pub fn set_video(&mut self, frame: &Frame) {
        let kind = Kind::of(frame.format);
        if self
            .video
            .as_ref()
            .is_none_or(|l| (l.kind, l.width, l.height) != (kind, frame.width, frame.height))
        {
            log::info!("video layer: {} {}x{}", frame.format.name(), frame.width, frame.height);
            self.video = Some(self.make_layer(kind, frame.width, frame.height));
        }
        let rows = conversion(frame.matrix, frame.range);
        let layer = self.video.as_mut().expect("just made");
        layer.rows = rows;
        let layer = self.video.as_ref().expect("just made");
        for (plane, texture) in frame.planes().iter().zip(&layer.planes) {
            self.upload(texture, plane.bytes.as_slice());
        }
    }

    fn make_layer(&self, kind: Kind, width: u32, height: u32) -> Layer {
        let planes: Vec<PlaneTexture> = kind
            .planes(width, height)
            .into_iter()
            .map(|(format, w, h, texel)| PlaneTexture {
                texture: self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("plane"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }),
                width: w,
                height: h,
                texel,
            })
            .collect();
        let rect = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect"),
            size: (UNIFORM_LEN * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let views: Vec<wgpu::TextureView> = planes
            .iter()
            .map(|p| p.texture.create_view(&Default::default()))
            .collect();
        let view = |i: usize| views.get(i).unwrap_or(&views[0]);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view(0)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: rect.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(view(1)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(view(2)),
                },
            ],
        });
        Layer {
            planes,
            bind_group,
            rect,
            kind,
            width,
            height,
            rows: [[0.0; 4]; 3],
        }
    }

    fn upload(&self, plane: &PlaneTexture, data: &[u8]) {
        let row = plane.width * plane.texel;
        if data.len() < (row * plane.height) as usize {
            log::warn!(
                "a plane of {} bytes is short of {}x{}",
                data.len(),
                plane.width,
                plane.height
            );
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &plane.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: plane.width,
                height: plane.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Composites the video (letterboxed in the scene's video area, corners rounded, at
    /// `video_opacity` over the backdrop) and the UI layer, then presents at once. False when the
    /// surface was not ready.
    pub fn render(&mut self, acquired: Option<wgpu::SurfaceTexture>, scene: &Scene, video_opacity: f32) -> bool {
        // An image acquired at the old size goes unpresented, before the surface is configured.
        let size = (scene.size.0.max(1), scene.size.1.max(1));
        let acquired = acquired.filter(|_| size == (self.config.width, self.config.height));
        self.resize(size.0, size.1);
        let Some(frame) = acquired.or_else(|| self.acquire()) else {
            return false;
        };
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        let watermark = scene.watermark[3] > 0.0 && scene.video[3] > 0.0;
        if watermark && self.watermark.is_none() {
            let layer = self.make_layer(Kind::Mask, WATERMARK_SIZE, WATERMARK_SIZE);
            self.upload(&layer.planes[0], WATERMARK);
            self.watermark = Some(layer);
        }
        let watermark = self.watermark.as_ref().filter(|_| watermark);
        if let Some(mark) = watermark {
            let mut values = uniform(watermark_rect(scene.video), 0.0, (w, h), mark);
            values[OPACITY] = scene.watermark[3];
            values[12..15].copy_from_slice(&scene.watermark[..3]);
            self.queue.write_buffer(&mark.rect, 0, bytemuck::cast_slice(&values));
        }
        let shadow = scene.shadow[1] > 0.0 && self.video.is_some();
        if shadow && self.shadow.is_none() {
            self.shadow = Some(self.make_layer(Kind::Shadow, 1, 1));
        }
        let shadow = self.shadow.as_ref().filter(|_| shadow);
        if let Some(video) = &self.video {
            let rect = letterbox(scene.video, (video.width, video.height));
            let mut values = uniform(rect, scene.video[4], (w, h), video);
            values[OPACITY] = video_opacity;
            self.queue.write_buffer(&video.rect, 0, bytemuck::cast_slice(&values));
            if let Some(layer) = shadow {
                let [scale, strength] = scene.shadow;
                let values = shadow_uniform(rect, scene.video[4], scale, video_opacity * strength, (w, h), layer);
                self.queue.write_buffer(&layer.rect, 0, bytemuck::cast_slice(&values));
            }
        }
        if let Some(ui) = &self.ui {
            let values = uniform([0.0, 0.0, w, h], 0.0, (w, h), ui);
            self.queue.write_buffer(&ui.rect, 0, bytemuck::cast_slice(&values));
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let [r, g, b] = scene.backdrop;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            // The watermark and the shadow stay inside the video area: the watermark's cut side is
            // cut there, and the shadow does not reach under the title bar.
            let backdrop = [watermark, shadow];
            if backdrop.iter().any(Option::is_some)
                && let Some([x, y, sw, sh]) = scissor(scene.video, (self.config.width, self.config.height))
            {
                pass.set_scissor_rect(x, y, sw, sh);
                for layer in backdrop.into_iter().flatten() {
                    pass.set_bind_group(0, &layer.bind_group, &[]);
                    pass.draw(0..4, 0..1);
                }
                pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
            }
            for layer in [self.video.as_ref(), self.ui.as_ref()].into_iter().flatten() {
                pass.set_bind_group(0, &layer.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.presents += 1;
        crate::present_log::present(self.presents);
        true
    }
}

/// The watermark's mask (`build.rs`), `WATERMARK_SIZE` square, one byte of coverage per pixel.
const WATERMARK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/watermark.a8"));
const WATERMARK_SIZE: u32 = 1024;

/// How much of the watermark's width the video area's right edge cuts (data-model.md "Video
/// backdrop").
const WATERMARK_CUT: f32 = 0.12;

/// The watermark's rectangle: a square as tall as the video area, its bottom on the area's bottom,
/// its right side past the area's right edge by [`WATERMARK_CUT`] of its width.
fn watermark_rect(area: [f32; 5]) -> [f32; 4] {
    let [ax, ay, aw, ah, _] = area;
    [ax + aw - ah * (1.0 - WATERMARK_CUT), ay, ah, ah]
}

/// The video's shadow in logical pixels (data-model.md "Video backdrop"; softer than the mockup,
/// maintainer 2026-11-07): strength, spread, blur of the near and the far shadow.
const SHADOWS: [[f32; 3]; 2] = [[0.35, 0.0, 24.0], [0.2, 4.0, 64.0]];

/// Past the video, the shadow's rectangle reaches the far shadow's spread plus one and a half
/// blurs, where it is gone.
fn shadow_margin(scale: f32) -> f32 {
    (SHADOWS[1][1] + SHADOWS[1][2] * 1.5) * scale
}

/// The shadow layer's uniform for the letterboxed video `rect`: its rectangle grown by the margin,
/// the video's corner radius, the shadows at `scale` (physical pixels per logical pixel), faded
/// with the video.
fn shadow_uniform(
    rect: [f32; 4],
    radius: f32,
    scale: f32,
    opacity: f32,
    surface: (f32, f32),
    layer: &Layer,
) -> [f32; UNIFORM_LEN] {
    let margin = shadow_margin(scale);
    let [x, y, w, h] = rect;
    let mut values = uniform(
        [x - margin, y - margin, w + 2.0 * margin, h + 2.0 * margin],
        radius,
        surface,
        layer,
    );
    values[OPACITY] = opacity;
    let [[k1, s1, b1], [k2, s2, b2]] = SHADOWS;
    values[12..16].copy_from_slice(&[margin, k1, s1 * scale, b1 * scale]);
    values[16..20].copy_from_slice(&[k2, s2 * scale, b2 * scale, 0.0]);
    values
}

/// The video area as a scissor rectangle inside the surface, whole pixels; `None` when empty.
fn scissor(area: [f32; 5], (sw, sh): (u32, u32)) -> Option<[u32; 4]> {
    let [ax, ay, aw, ah, _] = area;
    let x0 = (ax.max(0.0).round() as u32).min(sw);
    let y0 = (ay.max(0.0).round() as u32).min(sh);
    let x1 = ((ax + aw).max(0.0).round() as u32).min(sw);
    let y1 = ((ay + ah).max(0.0).round() as u32).min(sh);
    (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
}

/// The video's rectangle (x, y, width, height in pixels) fitted inside `area`, centered, whole
/// pixels.
fn letterbox(area: [f32; 5], (vw, vh): (u32, u32)) -> [f32; 4] {
    let [ax, ay, aw, ah, _] = area;
    let scale = (aw / vw as f32).min(ah / vh as f32);
    let (rw, rh) = ((vw as f32 * scale).round(), (vh as f32 * scale).round());
    [(ax + (aw - rw) / 2.0).round(), (ay + (ah - rh) / 2.0).round(), rw, rh]
}

/// A texture binding of the fragment stage.
fn plane_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Floats in the shader's `Rect` uniform.
const UNIFORM_LEN: usize = 24;

/// Where the layer's opacity is in the uniform.
const OPACITY: usize = 7;

/// A layer's rectangle as the shader's uniform: bounds in clip space, size, corner radius,
/// opacity, then the kind and picture size, then the YUV to RGB rows.
fn uniform(rect: [f32; 4], radius: f32, surface: (f32, f32), layer: &Layer) -> [f32; UNIFORM_LEN] {
    let mut values = [0.0; UNIFORM_LEN];
    values[..8].copy_from_slice(&placement(rect, radius, surface));
    values[8..12].copy_from_slice(&[layer.kind.code(), layer.width as f32, layer.height as f32, 0.0]);
    for (i, row) in layer.rows.iter().enumerate() {
        values[12 + i * 4..16 + i * 4].copy_from_slice(row);
    }
    values
}

/// The rows that turn normalized `(y, u, v, 1)` into RGB for a matrix and range (research R4):
/// `R = Y' + 2(1 − Kr)·Cr`, `B = Y' + 2(1 − Kb)·Cb`, `G = (Y' − Kr·R − Kb·B) / Kg`, with
/// `Y' = (Y − 16/255)·255/219` and `C = (C − 128/255)·255/224` in limited range, `Y' = Y` and
/// `C = C − 128/255` in full range.
pub fn conversion(matrix: Matrix, range: Range) -> [[f32; 4]; 3] {
    let (kr, kb) = match matrix {
        Matrix::Bt601 => (0.299_f64, 0.114_f64),
        Matrix::Bt709 => (0.2126, 0.0722),
    };
    let kg = 1.0 - kr - kb;
    let (ys, yo, cs) = match range {
        Range::Limited => (255.0 / 219.0, -16.0 / 219.0, 255.0 / 224.0),
        Range::Full => (1.0, 0.0, 1.0),
    };
    let co = -128.0 / 255.0 * cs;
    let (rv, bu) = (2.0 * (1.0 - kr), 2.0 * (1.0 - kb));
    let (gu, gv) = (-kb * bu / kg, -kr * rv / kg);
    let row = |u: f64, v: f64| [ys as f32, (u * cs) as f32, (v * cs) as f32, (yo + (u + v) * co) as f32];
    [row(0.0, rv), row(gu, gv), row(bu, 0.0)]
}

/// What the shader gives for 8-bit `y`, `u`, `v`: RGB from 0 to 1, clamped. Mirrors composite.wgsl
/// for the tests.
#[cfg(test)]
fn yuv_to_rgb(y: u8, u: u8, v: u8, matrix: Matrix, range: Range) -> [f32; 3] {
    let p = [f32::from(y) / 255.0, f32::from(u) / 255.0, f32::from(v) / 255.0, 1.0];
    conversion(matrix, range).map(|row| row.iter().zip(p).map(|(a, b)| a * b).sum::<f32>().clamp(0.0, 1.0))
}

/// Bounds in clip space, size, corner radius, opacity.
fn placement([x, y, rw, rh]: [f32; 4], radius: f32, (w, h): (f32, f32)) -> [f32; 8] {
    [
        x / w * 2.0 - 1.0,
        1.0 - y / h * 2.0,
        (x + rw) / w * 2.0 - 1.0,
        1.0 - (y + rh) / h * 2.0,
        rw,
        rh,
        radius,
        1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use Backend::{Dx12, Gl, Vulkan};
    use PresentMode::{Fifo, Immediate, Mailbox};

    const WINDOWS: &[Backend] = &[Dx12, Vulkan];
    const LINUX: &[Backend] = &[Vulkan, Gl];

    fn forced(backend: Option<Backend>, present: Option<PresentMode>) -> Presentation {
        Presentation { backend, present }
    }

    #[test]
    fn the_shadow_rectangle_grows_around_the_video() {
        let layer_values = |scale: f32| {
            let margin = shadow_margin(scale);
            [margin, SHADOWS[0][1] * scale, SHADOWS[1][2] * scale]
        };
        assert_eq!(layer_values(1.0), [100.0, 0.0, 64.0]);
        assert_eq!(layer_values(1.5), [150.0, 0.0, 96.0]);
    }

    #[test]
    fn the_watermark_fills_the_area_height_and_is_cut_on_the_right() {
        let area = [0.0, 32.0, 1920.0, 1048.0, 0.0];
        let [x, y, w, h] = watermark_rect(area);
        assert_eq!((y, w, h), (32.0, 1048.0, 1048.0));
        assert!((x + w - 1920.0 - 0.12 * 1048.0).abs() < 1e-3, "12% past the right edge");
        assert_eq!(scissor(area, (1920, 1080)), Some([0, 32, 1920, 1048]));
        assert_eq!(scissor([0.0, 0.0, 0.0, 0.0, 0.0], (1920, 1080)), None);
        assert_eq!(
            scissor([-10.0, 0.0, 3000.0, 2000.0, 0.0], (1920, 1080)),
            Some([0, 0, 1920, 1080])
        );
    }

    #[test]
    fn windows_order() {
        assert_eq!(
            order(WINDOWS, Presentation::default()),
            vec![(Dx12, vec![Mailbox, Immediate, Fifo]), (Vulkan, vec![Mailbox, Fifo])]
        );
    }

    #[test]
    fn linux_order() {
        assert_eq!(
            order(LINUX, Presentation::default()),
            vec![(Vulkan, vec![Mailbox, Immediate, Fifo]), (Gl, vec![Fifo])]
        );
    }

    #[test]
    fn forced_choices_do_not_fall_back() {
        assert_eq!(
            order(WINDOWS, forced(Some(Vulkan), None)),
            vec![(Vulkan, vec![Mailbox, Fifo])]
        );
        assert_eq!(
            order(WINDOWS, forced(None, Some(Fifo))),
            vec![(Dx12, vec![Fifo]), (Vulkan, vec![Fifo])]
        );
        assert_eq!(order(LINUX, forced(Some(Gl), Some(Mailbox))), vec![(Gl, vec![Mailbox])]);
        // A backend this build lacks is still the only one tried, so it fails.
        assert_eq!(order(WINDOWS, forced(Some(Gl), None)), vec![(Gl, vec![Fifo])]);
        assert_eq!(
            order(LINUX, forced(Some(Dx12), None)),
            vec![(Dx12, vec![Mailbox, Immediate, Fifo])]
        );
    }

    #[test]
    fn letterboxing() {
        // 16:9 video in a 4:3 area: bars above and below.
        assert_eq!(
            letterbox([0.0, 0.0, 800.0, 600.0, 0.0], (1920, 1080)),
            [0.0, 75.0, 800.0, 450.0]
        );
        // 4:3 video in a 16:9 area with an offset: bars on the sides.
        assert_eq!(
            letterbox([0.0, 32.0, 1600.0, 900.0, 0.0], (640, 480)),
            [200.0, 32.0, 1200.0, 900.0]
        );
    }

    #[test]
    fn placement_maps_to_clip_space() {
        assert_eq!(
            placement([0.0, 0.0, 100.0, 50.0], 4.0, (100.0, 50.0)),
            [-1.0, 1.0, 1.0, -1.0, 100.0, 50.0, 4.0, 1.0]
        );
    }

    fn close(actual: [f32; 3], expected: [f32; 3]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 0.01, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn yuv_to_rgb_in_both_matrices_and_ranges() {
        use Matrix::{Bt601, Bt709};
        use Range::{Full, Limited};
        for matrix in [Bt601, Bt709] {
            close(yuv_to_rgb(16, 128, 128, matrix, Limited), [0.0, 0.0, 0.0]);
            close(yuv_to_rgb(235, 128, 128, matrix, Limited), [1.0, 1.0, 1.0]);
            close(yuv_to_rgb(0, 128, 128, matrix, Full), [0.0, 0.0, 0.0]);
            close(yuv_to_rgb(255, 128, 128, matrix, Full), [1.0, 1.0, 1.0]);
            // Limited black read as full is grey, not black.
            assert!(yuv_to_rgb(16, 128, 128, matrix, Full)[0] > 0.05);
        }
        // Red, from the standards' reference values.
        close(yuv_to_rgb(63, 102, 240, Bt709, Limited), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(81, 90, 240, Bt601, Limited), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(54, 99, 255, Bt709, Full), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(76, 85, 255, Bt601, Full), [1.0, 0.0, 0.0]);
        // Green and blue in BT.709 limited.
        close(yuv_to_rgb(173, 42, 26, Bt709, Limited), [0.0, 1.0, 0.0]);
        close(yuv_to_rgb(32, 240, 118, Bt709, Limited), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn plane_textures_per_format() {
        let sizes = |format| -> Vec<(u32, u32, u32)> {
            Kind::of(format)
                .planes(1920, 1080)
                .into_iter()
                .map(|(_, w, h, t)| (w, h, t))
                .collect()
        };
        assert_eq!(sizes(PixelFormat::Nv12), [(1920, 1080, 1), (960, 540, 2)]);
        assert_eq!(
            sizes(PixelFormat::I420),
            [(1920, 1080, 1), (960, 540, 1), (960, 540, 1)]
        );
        assert_eq!(sizes(PixelFormat::Yuy2), [(960, 1080, 4)]);
        assert_eq!(sizes(PixelFormat::Bgrx), [(1920, 1080, 4)]);
        // Row lengths match the capture crate's packed planes.
        for format in [
            PixelFormat::Nv12,
            PixelFormat::I420,
            PixelFormat::Yuy2,
            PixelFormat::Uyvy,
            PixelFormat::Bgrx,
            PixelFormat::Rgba,
        ] {
            let ours: Vec<(usize, usize)> = Kind::of(format)
                .planes(1279, 719)
                .into_iter()
                .map(|(_, w, h, t)| ((w * t) as usize, h as usize))
                .collect();
            assert_eq!(ours, format.plane_sizes(1279, 719), "{format:?}");
        }
    }
}
