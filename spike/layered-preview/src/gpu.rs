//! The video thread (research R15, T052): it owns the GPU and composites the ambient light, the
//! video and the UI layer, presenting as soon as a video frame or a UI image arrives. The appsink
//! hands it the frames directly and the window's thread hands it the UI, so the video never waits
//! for the window's event queue, which Windows holds while the window is moved or resized.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use winit::window::Window;

use crate::{Area, Frame, ambient};

/// What the video thread draws besides the layers.
#[derive(Clone, Copy, PartialEq)]
pub struct Scene {
    /// The theme's deepest tone: the ground around the video.
    pub backdrop: wgpu::Color,
    /// Where the video is letterboxed, in physical pixels (none: the whole surface).
    pub area: Option<Area>,
    /// The ambient light's strength, when it is on.
    pub ambient: Option<f32>,
}

/// A finished UI layer: premultiplied RGBA, the size of the surface it was drawn for.
pub struct UiImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Time the window's thread took to draw and copy it, for the stats.
    pub cost_ms: f64,
}

/// What is waiting for the video thread; only the newest of each is kept.
#[derive(Default)]
struct Inbox {
    frame: Option<Frame>,
    ui: Option<UiImage>,
    scene: Option<Scene>,
    size: (u32, u32),
    /// The UI, the scene or the size changed since the video thread last looked.
    changed: bool,
    /// Window moves since the last look, for the stats (T037).
    moves: u32,
    stop: bool,
}

/// Hands frames and UI images to the video thread and wakes it.
#[derive(Default)]
pub struct Mailbox {
    inbox: Mutex<Inbox>,
    wake: Condvar,
}

impl Mailbox {
    /// From the appsink: replaces a frame not yet presented.
    pub fn send_frame(&self, frame: Frame) {
        self.inbox.lock().unwrap().frame = Some(frame);
        self.wake.notify_one();
    }

    /// From the window's thread: the UI image when it was redrawn, the scene and the surface size.
    pub fn send_ui(&self, ui: Option<UiImage>, scene: Scene, size: (u32, u32)) {
        let mut inbox = self.inbox.lock().unwrap();
        if ui.is_some() {
            inbox.ui = ui;
        }
        inbox.scene = Some(scene);
        inbox.size = size;
        inbox.changed = true;
        drop(inbox);
        self.wake.notify_one();
    }

    pub fn count_move(&self) {
        self.inbox.lock().unwrap().moves += 1;
    }
}

/// The running video thread; stopped before the window closes.
pub struct VideoThread {
    mailbox: Arc<Mailbox>,
    handle: Option<JoinHandle<()>>,
}

impl VideoThread {
    pub fn start(gpu: Gpu, mailbox: Arc<Mailbox>, stats: bool) -> Self {
        let inbox = mailbox.clone();
        let handle = std::thread::Builder::new()
            .name("video".into())
            .spawn(move || run(gpu, &inbox, Stats::new(stats)))
            .expect("video thread");
        Self { mailbox, handle: Some(handle) }
    }

    pub fn stop(&mut self) {
        self.mailbox.inbox.lock().unwrap().stop = true;
        self.mailbox.wake.notify_one();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run(mut gpu: Gpu, mailbox: &Mailbox, mut stats: Stats) {
    let mut scene = None;
    loop {
        let (frame, ui) = {
            let mut inbox = mailbox.inbox.lock().unwrap();
            while inbox.frame.is_none() && !inbox.changed && !inbox.stop {
                inbox = mailbox.wake.wait(inbox).unwrap();
            }
            if inbox.stop {
                return;
            }
            inbox.changed = false;
            stats.moves += std::mem::take(&mut inbox.moves);
            scene = inbox.scene.or(scene);
            let (w, h) = (inbox.size.0.max(1), inbox.size.1.max(1));
            if inbox.size != (0, 0) && (w, h) != (gpu.config.width, gpu.config.height) {
                gpu.resize(w, h);
            }
            (inbox.frame.take(), inbox.ui.take())
        };
        if let Some(ui) = ui {
            if gpu.ui.as_ref().is_none_or(|l| l.width != ui.width || l.height != ui.height) {
                gpu.ui = Some(gpu.make_layer(ui.width, ui.height));
            }
            gpu.upload(gpu.ui.as_ref().unwrap(), &ui.pixels);
            stats.ui_ms.push(ui.cost_ms);
        }
        let ready = frame.map(|frame| {
            if gpu.video.as_ref().is_none_or(|v| v.width != frame.width || v.height != frame.height) {
                gpu.video = Some(gpu.make_layer(frame.width, frame.height));
            }
            gpu.upload(gpu.video.as_ref().unwrap(), &frame.data);
            gpu.fresh_video = true;
            frame.ready
        });
        // Nothing to draw until the window's thread has drawn the UI once.
        let Some(scene) = scene else { continue };
        if gpu.render(scene.backdrop, scene.area, scene.ambient)
            && let Some(ready) = ready
        {
            stats.presented(ready);
        }
        stats.report(scene.ambient.is_some());
    }
}

/// Per second: video frames presented, time from frame ready to present, the longest gap between
/// two presented frames, UI images and their cost, window moves.
struct Stats {
    enabled: bool,
    presented: u64,
    delays_ms: Vec<f64>,
    last_video: Option<Instant>,
    max_gap_ms: f64,
    ui_ms: Vec<f64>,
    moves: u32,
    last_report: Instant,
}

impl Stats {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            presented: 0,
            delays_ms: Vec::new(),
            last_video: None,
            max_gap_ms: 0.0,
            ui_ms: Vec::new(),
            moves: 0,
            last_report: Instant::now(),
        }
    }

    fn presented(&mut self, ready: Instant) {
        let now = Instant::now();
        self.presented += 1;
        self.delays_ms.push((now - ready).as_secs_f64() * 1e3);
        if let Some(last) = self.last_video.replace(now) {
            self.max_gap_ms = self.max_gap_ms.max((now - last).as_secs_f64() * 1e3);
        }
    }

    fn report(&mut self, ambient: bool) {
        if !self.enabled || self.last_report.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.delays_ms.sort_by(f64::total_cmp);
        let median = self.delays_ms.get(self.delays_ms.len() / 2).copied().unwrap_or(f64::NAN);
        let max = self.delays_ms.last().copied().unwrap_or(f64::NAN);
        self.ui_ms.sort_by(f64::total_cmp);
        let ui_median = self.ui_ms.get(self.ui_ms.len() / 2).copied().unwrap_or(0.0);
        let ui_max = self.ui_ms.last().copied().unwrap_or(0.0);
        println!(
            "[loop]  {:>4} fps presented | frame ready -> presented median {median:.1} ms, max {max:.1} ms | longest gap {:.1} ms | UI {:>3} redraws, median {ui_median:.1} ms, max {ui_max:.1} ms | {} moves | ambient {}",
            self.presented,
            self.max_gap_ms,
            self.ui_ms.len(),
            self.moves,
            if ambient { "on" } else { "off" }
        );
        self.presented = 0;
        self.delays_ms.clear();
        self.max_gap_ms = 0.0;
        self.ui_ms.clear();
        self.moves = 0;
        self.last_report = Instant::now();
    }
}

/// A texture drawn as one rectangle (video or UI layer).
struct Layer {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    rect: wgpu::Buffer,
    width: u32,
    height: u32,
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
    /// A video frame was uploaded since the last render: the ambient light takes it.
    fresh_video: bool,
    ambient: ambient::Ambient,
    glow: Layer,
}

impl Gpu {
    pub fn new(window: Arc<Window>, wanted: Option<&str>, backend: Option<&str>) -> anyhow::Result<Self> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = match backend {
            None => wgpu::Backends::all(),
            Some("vulkan") => wgpu::Backends::VULKAN,
            Some("dx12") => wgpu::Backends::DX12,
            Some("gl") => wgpu::Backends::GL,
            Some(other) => anyhow::bail!("unknown backend {other:?} (vulkan, dx12, gl)"),
        };
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow::anyhow!("surface not supported by the adapter"))?;
        // Plain (non-sRGB) format: the video bytes are shown as they are.
        if let Some(format) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *format;
        }
        config.present_mode = choose_present_mode(&caps.present_modes, wanted)?;
        // At most one frame waiting in the swap chain.
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        println!(
            "GPU: {} ({:?}) | present mode {:?} | supported {:?}",
            adapter.get_info().name,
            adapter.get_info().backend,
            config.present_mode,
            caps.present_modes
        );

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
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
                    // Bounds for the vertices; size and radius for the rounded corners.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
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
        let ambient = ambient::Ambient::new(&device);
        let glow = new_layer(
            &device,
            &layout,
            &sampler,
            ambient::WIDTH,
            ambient::HEIGHT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            sampler,
            video: None,
            ui: None,
            fresh_video: false,
            ambient,
            glow,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn make_layer(&self, width: u32, height: u32) -> Layer {
        new_layer(&self.device, &self.layout, &self.sampler, width, height, wgpu::TextureUsages::COPY_DST)
    }

    fn upload(&self, layer: &Layer, data: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &layer.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(layer.width * 4), rows_per_image: None },
            wgpu::Extent3d { width: layer.width, height: layer.height, depth_or_array_layers: 1 },
        );
    }

    /// Composites the ambient light (with its strength, when on), the video (letterboxed) and the
    /// UI layer, then presents immediately. `area` is where the video is letterboxed (the whole
    /// surface when the UI gives none); the ambient light fills it.
    fn render(&mut self, backdrop: wgpu::Color, area: Option<Area>, ambient: Option<f32>) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
        };
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        // Pixel rectangle to clip space, plus the size and radius the shader rounds the corners with.
        let uniform = |x: f32, y: f32, rw: f32, rh: f32, radius: f32, opacity: f32| {
            [x / w * 2.0 - 1.0, 1.0 - y / h * 2.0, (x + rw) / w * 2.0 - 1.0, 1.0 - (y + rh) / h * 2.0, rw, rh, radius, opacity]
        };
        let [ax, ay, aw, ah, radius] = area.unwrap_or([0.0, 0.0, w, h, 0.0]);
        if let Some(video) = &self.video {
            let scale = (aw / video.width as f32).min(ah / video.height as f32);
            let (rw, rh) = ((video.width as f32 * scale).round(), (video.height as f32 * scale).round());
            let (x, y) = ((ax + (aw - rw) / 2.0).round(), (ay + (ah - rh) / 2.0).round());
            self.queue.write_buffer(&video.rect, 0, bytemuck::cast_slice(&uniform(x, y, rw, rh, radius, 1.0)));
        }
        if let Some(ui) = &self.ui {
            self.queue.write_buffer(&ui.rect, 0, bytemuck::cast_slice(&uniform(0.0, 0.0, w, h, 0.0, 1.0)));
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let glow = match (ambient, &self.video) {
            (Some(strength), Some(video)) => {
                if std::mem::take(&mut self.fresh_video) {
                    let video_view = video.texture.create_view(&Default::default());
                    let glow_view = self.glow.texture.create_view(&Default::default());
                    self.ambient.update(&self.device, &mut encoder, &video_view, &glow_view);
                }
                let rect = uniform(ax, ay, aw, ah, radius, strength);
                self.queue.write_buffer(&self.glow.rect, 0, bytemuck::cast_slice(&rect));
                self.ambient.ready().then_some(&self.glow)
            }
            _ => {
                self.ambient.reset();
                None
            }
        };
        self.fresh_video = false;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(backdrop), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            for layer in [glow, self.video.as_ref(), self.ui.as_ref()].into_iter().flatten() {
                pass.set_bind_group(0, &layer.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        true
    }
}

/// A layer's texture (sampled, plus `usage`) with its rectangle and bind group.
fn new_layer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    usage: wgpu::TextureUsages,
) -> Layer {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("layer"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | usage,
        view_formats: &[],
    });
    let rect = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rect"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("layer"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: rect.as_entire_binding() },
        ],
    });
    Layer { texture, bind_group, rect, width, height }
}

fn choose_present_mode(supported: &[wgpu::PresentMode], wanted: Option<&str>) -> anyhow::Result<wgpu::PresentMode> {
    use wgpu::PresentMode::*;
    let order: Vec<wgpu::PresentMode> = match wanted {
        Some("immediate") => vec![Immediate],
        Some("mailbox") => vec![Mailbox],
        Some("fifo") => vec![Fifo],
        Some(other) => anyhow::bail!("unknown present mode {other:?} (immediate, mailbox, fifo)"),
        None => vec![Immediate, Mailbox, Fifo],
    };
    order
        .into_iter()
        .find(|m| supported.contains(m))
        .ok_or_else(|| anyhow::anyhow!("present mode {wanted:?} not supported (supported: {supported:?})"))
}
