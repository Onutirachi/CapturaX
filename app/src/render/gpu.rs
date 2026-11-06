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
    /// The `[present]` line of contracts/stats.md for the presentation in use.
    header: String,
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
        let header = crate::stats::header(
            backend_name(backend),
            mode_name(mode),
            &adapter.get_info().name,
            (config.width, config.height),
        );
        log::info!("presentation: {header}");
        Ok(Self::with_device(surface, device, queue, config, header))
    }

    fn with_device(
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        header: String,
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
            header,
        }
    }

    /// The `[present]` header of the presentation in use.
    pub fn header(&self) -> String {
        self.header.clone()
    }

    /// Reconfigures the surface when the window's size changed.
    pub fn resize(&mut self, width: u32, height: u32) {
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

    /// Composites the video (letterboxed in the scene's video area, corners rounded) and the UI
    /// layer over the backdrop, then presents at once. False when the surface was not ready.
    pub fn render(&mut self, scene: &Scene) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
        };
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        if let Some(video) = &self.video {
            let rect = letterbox(scene.video, (video.width, video.height));
            let values = uniform(rect, scene.video[4], (w, h), video);
            self.queue.write_buffer(&video.rect, 0, bytemuck::cast_slice(&values));
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
            for layer in [self.video.as_ref(), self.ui.as_ref()].into_iter().flatten() {
                pass.set_bind_group(0, &layer.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        true
    }
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
