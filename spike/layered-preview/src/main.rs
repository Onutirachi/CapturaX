//! Own render loop (research R13, T047): winit window + wgpu swap chain presenting each video frame
//! as soon as it arrives, with a Slint UI layer (software renderer) composited over the video.
//!
//! F11 toggles fullscreen on the current monitor; Esc closes an open menu, then leaves fullscreen.
//! Close the window to quit.

mod ambient;

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use capture::mode::{self, Mode};
use capture::{audio, cli::CommonArgs, devices, encoder, stats, video};
use clap::Parser;
use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent as SlintEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

slint::include_modules!();

#[derive(Parser, Debug)]
#[command(about = "Own render loop: video presented on arrival, Slint UI as a layer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Present mode: immediate, mailbox or fifo (default: first supported, in that order)
    #[arg(long)]
    present: Option<String>,

    /// Graphics API: vulkan, dx12 or gl (default: wgpu's choice)
    #[arg(long)]
    backend: Option<String>,

    /// Also encode the video with a hardware H.264 encoder, discarding the output (load test)
    #[arg(long)]
    encoder: bool,

    /// Also play the audio input paired with the video device on the system default output
    #[arg(long)]
    audio: bool,

    /// UI drawn over the video: overlay, render-test (what the software renderer draws, T048),
    /// theme-test (the dark and light palettes, T027) or float (the Float direction, T032)
    #[arg(long, default_value = "overlay")]
    ui: String,

    /// Starting theme: dark or light (switchable at runtime in the Float direction)
    #[arg(long, default_value = "dark")]
    theme: String,

    /// Show a live test pattern in --mode (default 1920x1080@60) instead of a device
    #[arg(long)]
    test_video: bool,

    /// Start with the ambient light on (Float direction; the dock's button toggles it)
    #[arg(long)]
    ambient: bool,

    /// Ambient light strength, from 0 to 1 (the previous app used 0.9)
    #[arg(long, default_value_t = 0.9)]
    ambient_strength: f32,
}

/// One RGBA frame, rows packed without padding.
struct Frame {
    width: u32,
    height: u32,
    data: Vec<u8>,
    ready: Instant,
}

enum UserEvent {
    /// A new frame is waiting in the shared slot.
    FrameReady,
    /// The UI asked for fullscreen on or off (the dock's button).
    ToggleFullscreen,
    Error(String),
}

/// Where the video goes, in physical pixels: x, y, width, height, corner radius.
type Area = [f32; 5];

/// Slint platform: one software-rendered window whose pixels we composite ourselves.
struct SpikePlatform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl slint::platform::Platform for SpikePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
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

struct Gpu {
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
    fn new(window: Arc<Window>, wanted: Option<&str>, backend: Option<&str>) -> anyhow::Result<Self> {
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

/// Appsink keeping only the newest RGBA frame in `slot`, waking the loop when the slot fills.
fn appsink(slot: Arc<Mutex<Option<Frame>>>, proxy: EventLoopProxy<UserEvent>) -> gst_app::AppSink {
    let caps = gst_video::VideoCapsBuilder::new().format(gst_video::VideoFormat::Rgba).build();
    let sink = gst_app::AppSink::builder().caps(&caps).sync(false).max_buffers(1).drop(true).build();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer_owned().ok_or(gst::FlowError::Error)?;
                let info = sample
                    .caps()
                    .and_then(|c| gst_video::VideoInfo::from_caps(c).ok())
                    .ok_or(gst::FlowError::NotNegotiated)?;
                let frame = gst_video::VideoFrame::from_buffer_readable(buffer, &info).map_err(|_| gst::FlowError::Error)?;
                let (width, height) = (frame.width(), frame.height());
                let stride = frame.plane_stride()[0] as usize;
                let row = width as usize * 4;
                let src = frame.plane_data(0).map_err(|_| gst::FlowError::Error)?;
                let mut data = vec![0u8; row * height as usize];
                for y in 0..height as usize {
                    data[y * row..(y + 1) * row].copy_from_slice(&src[y * stride..y * stride + row]);
                }
                let was_empty = slot.lock().unwrap().replace(Frame { width, height, data, ready: Instant::now() }).is_none();
                if was_empty {
                    let _ = proxy.send_event(UserEvent::FrameReady);
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    sink
}

struct App {
    present: Option<String>,
    backend: Option<String>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    slint_window: Rc<MinimalSoftwareWindow>,
    /// The Slint component shown; kept alive for the whole run.
    _ui: Box<dyn std::any::Any>,
    /// The theme's deepest tone, read each frame: the ground around the video follows the theme.
    backdrop: Box<dyn Fn() -> wgpu::Color>,
    /// Where the UI wants the video, in logical pixels (none: the whole window).
    video_area: Box<dyn Fn() -> Option<Area>>,
    /// Tells the UI whether the window is fullscreen.
    set_fullscreen: Box<dyn Fn(bool)>,
    /// Closes an open menu; false when none was open (Esc then leaves fullscreen).
    close_menu: Box<dyn Fn() -> bool>,
    /// Whether the UI has the ambient light on, and its strength.
    ambient: Box<dyn Fn() -> bool>,
    ambient_strength: f32,
    ui_pixels: Vec<PremultipliedRgbaColor>,
    slot: Arc<Mutex<Option<Frame>>>,
    /// None when there is no video source (no device found).
    pipeline: Option<gst::Pipeline>,
    cursor: slint::LogicalPosition,
    scale: f32,
    // Stats: frames presented and time from frame ready to present, per second.
    stats: bool,
    presented: u64,
    delays_ms: Vec<f64>,
    // UI layer redraws and their cost (software render + texture upload), per second (T049).
    ui_ms: Vec<f64>,
    last_report: Instant,
    /// When the last frame was presented and the monitor's refresh interval: animations redraw at
    /// most once per refresh.
    last_present: Instant,
    refresh: Duration,
}

impl App {
    fn dispatch(&self, event: SlintEvent) {
        self.slint_window.window().dispatch_event(event);
        self.request_redraw();
    }

    /// Reads the refresh interval of the monitor the window is on.
    fn update_refresh(&mut self, window: &Window) {
        if let Some(mhz) = window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).filter(|r| *r > 0) {
            self.refresh = Duration::from_secs_f64(1000.0 / mhz as f64);
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Draws the UI layer if Slint says it changed, then composites and presents.
    fn redraw(&mut self) -> bool {
        self.draw_ui();
        let scale = self.scale;
        let area = (self.video_area)().map(|a| a.map(|v| v * scale));
        let ambient = (self.ambient)().then_some(self.ambient_strength);
        let backdrop = (self.backdrop)();
        let Some(gpu) = self.gpu.as_mut() else { return false };
        let presented = gpu.render(backdrop, area, ambient);
        if presented {
            self.last_present = Instant::now();
        }
        presented
    }

    /// Advances timers and animations and draws the UI layer if it changed; true when it drew.
    fn draw_ui(&mut self) -> bool {
        let Some(gpu) = self.gpu.as_mut() else { return false };
        slint::platform::update_timers_and_animations();
        let (w, h) = (gpu.config.width, gpu.config.height);
        let ui_pixels = &mut self.ui_pixels;
        if ui_pixels.len() != (w * h) as usize {
            *ui_pixels = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
        }
        let started = Instant::now();
        let drawn = self.slint_window.draw_if_needed(|renderer| {
            renderer.render(ui_pixels, w as usize);
        });
        if drawn || gpu.ui.as_ref().is_none_or(|ui| ui.width != w || ui.height != h) {
            if gpu.ui.as_ref().is_none_or(|ui| ui.width != w || ui.height != h) {
                gpu.ui = Some(gpu.make_layer(w, h));
            }
            gpu.upload(gpu.ui.as_ref().unwrap(), bytemuck::cast_slice(ui_pixels));
            self.ui_ms.push(started.elapsed().as_secs_f64() * 1e3);
            return true;
        }
        false
    }

    fn report(&mut self) {
        if !self.stats || self.last_report.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.delays_ms.sort_by(f64::total_cmp);
        let median = self.delays_ms.get(self.delays_ms.len() / 2).copied().unwrap_or(f64::NAN);
        self.ui_ms.sort_by(f64::total_cmp);
        let ui_median = self.ui_ms.get(self.ui_ms.len() / 2).copied().unwrap_or(0.0);
        let ui_max = self.ui_ms.last().copied().unwrap_or(0.0);
        println!(
            "[loop]  {:>4} fps presented | frame ready -> presented median {median:.1} ms | UI {:>3} redraws, median {ui_median:.1} ms, max {ui_max:.1} ms | ambient {}",
            self.presented,
            self.ui_ms.len(),
            if (self.ambient)() { "on" } else { "off" }
        );
        self.presented = 0;
        self.delays_ms.clear();
        self.ui_ms.clear();
        self.last_report = Instant::now();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("CapturaX spike (layered)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let size = window.inner_size();
        self.scale = window.scale_factor() as f32;
        self.update_refresh(&window);
        self.slint_window.window().dispatch_event(SlintEvent::ScaleFactorChanged { scale_factor: self.scale });
        self.slint_window.set_size(slint::PhysicalSize::new(size.width, size.height));
        match Gpu::new(window.clone(), self.present.as_deref(), self.backend.as_deref()) {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(err) => {
                eprintln!("GPU setup failed: {err}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        if let Some(Err(err)) = self.pipeline.as_ref().map(|p| p.set_state(gst::State::Playing)) {
            eprintln!("Could not start capture: {err}");
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Error(message) => {
                eprintln!("{message}");
                event_loop.exit();
            }
            UserEvent::ToggleFullscreen => self.toggle_fullscreen(),
            UserEvent::FrameReady => {
                let Some(frame) = self.slot.lock().unwrap().take() else { return };
                if let Some(gpu) = self.gpu.as_mut() {
                    if gpu.video.as_ref().is_none_or(|v| v.width != frame.width || v.height != frame.height) {
                        gpu.video = Some(gpu.make_layer(frame.width, frame.height));
                    }
                    gpu.upload(gpu.video.as_ref().unwrap(), &frame.data);
                    gpu.fresh_video = true;
                }
                if self.redraw() {
                    self.presented += 1;
                    self.delays_ms.push(frame.ready.elapsed().as_secs_f64() * 1e3);
                }
                self.report();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                }
                self.slint_window.set_size(slint::PhysicalSize::new(size.width, size.height));
                self.redraw();
            }
            WindowEvent::Moved(_) => {
                if let Some(window) = self.window.clone() {
                    self.update_refresh(&window);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                self.dispatch(SlintEvent::ScaleFactorChanged { scale_factor: self.scale });
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = slint::LogicalPosition::new(position.x as f32 / self.scale, position.y as f32 / self.scale);
                self.dispatch(SlintEvent::PointerMoved { position: self.cursor });
            }
            WindowEvent::CursorLeft { .. } => self.dispatch(SlintEvent::PointerExited),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => PointerEventButton::Left,
                    MouseButton::Right => PointerEventButton::Right,
                    MouseButton::Middle => PointerEventButton::Middle,
                    _ => PointerEventButton::Other,
                };
                let position = self.cursor;
                self.dispatch(match state {
                    ElementState::Pressed => SlintEvent::PointerPressed { position, button },
                    ElementState::Released => SlintEvent::PointerReleased { position, button },
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Lines as 60 logical pixels, as Slint's own winit backend does.
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 60.0, y * 60.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / self.scale, p.y as f32 / self.scale),
                };
                self.dispatch(SlintEvent::PointerScrolled { position: self.cursor, delta_x, delta_y });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(text) = slint_key(&event) {
                    self.dispatch(match event.state {
                        ElementState::Pressed => SlintEvent::KeyPressed { text },
                        ElementState::Released => SlintEvent::KeyReleased { text },
                    });
                }
                if event.state == ElementState::Pressed {
                    match event.logical_key {
                        Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
                        // Esc never quits: it closes an open menu, then leaves fullscreen.
                        Key::Named(NamedKey::Escape) => {
                            if (self.close_menu)() {
                                self.request_redraw();
                            } else if self.window.as_ref().is_some_and(|w| w.fullscreen().is_some()) {
                                self.toggle_fullscreen();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Keep Slint timers and animations running between video frames, so they do not step at
        // the video's frame rate. Read before updating: an update clears the flag until the next
        // draw reads the animated properties again.
        let mut wake = slint::platform::duration_until_next_timer_update().map(|wait| Instant::now() + wait);
        if self.slint_window.window().has_active_animations() {
            let next = self.last_present + self.refresh;
            if Instant::now() >= next {
                self.request_redraw();
            } else {
                wake = Some(wake.map_or(next, |w| w.min(next)));
            }
        } else if self.draw_ui() {
            // A timer changed the UI (the chrome hiding): present it now.
            self.request_redraw();
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}


/// Slint's key text for a winit key, if Slint knows it.
fn slint_key(event: &KeyEvent) -> Option<slint::SharedString> {
    use slint::platform::Key as K;
    let key = match &event.logical_key {
        Key::Character(text) => return Some(text.as_str().into()),
        Key::Named(NamedKey::Escape) => K::Escape,
        Key::Named(NamedKey::Enter) => K::Return,
        Key::Named(NamedKey::Tab) => K::Tab,
        Key::Named(NamedKey::Space) => K::Space,
        Key::Named(NamedKey::Backspace) => K::Backspace,
        Key::Named(NamedKey::Delete) => K::Delete,
        Key::Named(NamedKey::ArrowLeft) => K::LeftArrow,
        Key::Named(NamedKey::ArrowRight) => K::RightArrow,
        Key::Named(NamedKey::ArrowUp) => K::UpArrow,
        Key::Named(NamedKey::ArrowDown) => K::DownArrow,
        Key::Named(NamedKey::Home) => K::Home,
        Key::Named(NamedKey::End) => K::End,
        Key::Named(NamedKey::PageUp) => K::PageUp,
        Key::Named(NamedKey::PageDown) => K::PageDown,
        Key::Named(NamedKey::Shift) => K::Shift,
        Key::Named(NamedKey::Control) => K::Control,
        Key::Named(NamedKey::Alt) => K::Alt,
        Key::Named(NamedKey::Super) => K::Meta,
        _ => return None,
    };
    Some(key.into())
}

/// A shown UI component and what the loop reads from it or tells it.
struct Ui {
    /// Kept alive for the whole run.
    component: Box<dyn std::any::Any>,
    backdrop: Box<dyn Fn() -> wgpu::Color>,
    video_area: Box<dyn Fn() -> Option<Area>>,
    set_fullscreen: Box<dyn Fn(bool)>,
    close_menu: Box<dyn Fn() -> bool>,
    ambient: Box<dyn Fn() -> bool>,
}

/// What a UI offers the loop beyond the shared globals; the defaults suit a UI without them.
struct Hooks {
    /// Where it wants the video (none: the whole window).
    video_area: Box<dyn Fn() -> Option<Area>>,
    /// Closes an open menu, false when none was open.
    close_menu: Box<dyn Fn() -> bool>,
    /// Whether the ambient light is on.
    ambient: Box<dyn Fn() -> bool>,
}

impl Default for Hooks {
    fn default() -> Self {
        Self { video_area: Box::new(|| None), close_menu: Box::new(|| false), ambient: Box::new(|| false) }
    }
}

/// Shows a UI component in the starting theme.
fn show<C>(ui: C, hooks: Hooks, dark: bool, proxy: EventLoopProxy<UserEvent>) -> anyhow::Result<Ui>
where
    C: slint::ComponentHandle + 'static,
    for<'a> Theme<'a>: slint::Global<'a, C>,
    for<'a> WindowState<'a>: slint::Global<'a, C>,
{
    ui.global::<Theme>().set_dark(dark);
    ui.global::<WindowState>().on_toggle_fullscreen(move || {
        let _ = proxy.send_event(UserEvent::ToggleFullscreen);
    });
    ui.show()?;
    let weak = ui.as_weak();
    let backdrop = move || {
        let deep = weak.upgrade().map(|ui| ui.global::<Theme>().get_c().deep).unwrap_or_default();
        let channel = |v: u8| v as f64 / 255.0;
        wgpu::Color { r: channel(deep.red()), g: channel(deep.green()), b: channel(deep.blue()), a: 1.0 }
    };
    let weak = ui.as_weak();
    let set_fullscreen = move |on: bool| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<WindowState>().set_fullscreen(on);
        }
    };
    Ok(Ui {
        component: Box::new(ui),
        backdrop: Box::new(backdrop),
        video_area: hooks.video_area,
        set_fullscreen: Box::new(set_fullscreen),
        close_menu: hooks.close_menu,
        ambient: hooks.ambient,
    })
}

impl App {
    fn toggle_fullscreen(&mut self) {
        if let Some(w) = &self.window {
            let on = w.fullscreen().is_none();
            w.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
            (self.set_fullscreen)(on);
            w.request_redraw();
        }
    }
}

/// Resolution badge of a mode, by its height.
fn tier(m: &Mode) -> (Tier, &'static str) {
    match m.height {
        h if h >= 2160 => (Tier::Uhd, "4K"),
        h if h >= 1440 => (Tier::Qhd, "QHD"),
        h if h >= 1080 => (Tier::FullHd, "Full HD"),
        h if h >= 720 => (Tier::Hd, "HD"),
        _ => (Tier::Other, ""),
    }
}

fn size_text(m: &Mode) -> String {
    format!("{} × {}", m.width, m.height)
}

fn mode_text(m: &Mode) -> String {
    let fps = m.fps_f64();
    let fps = if fps.fract().abs() < 0.01 { format!("{fps:.0}") } else { format!("{fps:.2}") };
    format!("{} · {fps} FPS", size_text(m))
}

/// A picker item; the badge comes from `m` when given.
fn item(group: &str, name: &str, detail: &str, m: Option<&Mode>) -> DropItem {
    let (tier, badge) = m.map(tier).unwrap_or((Tier::Other, ""));
    DropItem { group: group.into(), name: name.into(), detail: detail.into(), badge: badge.into(), tier }
}

/// Same size and frame rate: the Mode list hides the format.
fn same_shape(a: &Mode, b: &Mode) -> bool {
    a.width == b.width && a.height == b.height && (a.fps_f64() - b.fps_f64()).abs() < 0.01
}

/// The Mode list (product F3, without Auto source: the device reports no signal here): automatic
/// modes, then each size and frame rate once, largest first. Also returns the running entry.
fn mode_items(modes: &[Mode], running: &Mode, requested: bool) -> (Vec<DropItem>, i32) {
    let mut items = Vec::new();
    let max_resolution = mode::auto(modes);
    // Highest frame rate at 1080p, or at the largest resolution below it.
    let at_most_1080: Vec<&Mode> = modes.iter().filter(|m| m.height <= 1080).collect();
    let pool = if at_most_1080.is_empty() { modes.iter().collect() } else { at_most_1080 };
    let max_fps = pool
        .into_iter()
        .max_by(|a, b| a.fps_f64().total_cmp(&b.fps_f64()).then((a.width * a.height).cmp(&(b.width * b.height))))
        .cloned();
    if let Some(m) = &max_resolution {
        items.push(item("Automatic", "Auto max resolution", &mode_text(m), Some(m)));
    }
    // An automatic mode that resolves to the same mode as another is not shown.
    if let Some(m) = max_fps.as_ref().filter(|m| max_resolution.as_ref().is_none_or(|r| !same_shape(r, m))) {
        items.push(item("Automatic", "Auto max FPS", &mode_text(m), Some(m)));
    }
    let automatic = items.len();
    let mut shapes: Vec<&Mode> = Vec::new();
    for m in modes {
        if !shapes.iter().any(|s| same_shape(s, m)) {
            shapes.push(m);
        }
    }
    shapes.sort_by(|a, b| (b.width * b.height).cmp(&(a.width * a.height)).then(b.fps_f64().total_cmp(&a.fps_f64())));
    items.extend(shapes.iter().map(|m| item("Modes", &mode_text(m), "", Some(m))));
    let current = if requested { shapes.iter().position(|m| same_shape(m, running)).map(|i| i + automatic) } else { None };
    (items, current.unwrap_or(0) as i32)
}

fn model(items: Vec<DropItem>) -> slint::ModelRc<DropItem> {
    slint::ModelRc::new(slint::VecModel::from(items))
}

/// The video source chosen from the options.
enum Input {
    Device(devices::VideoDevice),
    Test,
    /// No device found: the Float direction says so instead of failing.
    Nothing,
}

/// Fills the Float direction's pickers and info from the real devices and the running mode.
fn fill_float(ui: &Float, input: &Input, mode: &Mode, requested: bool, audio_on: bool, all: &[(String, Option<Mode>)]) -> anyhow::Result<()> {
    // Every device with its default mode as the detail.
    let mut devices: Vec<DropItem> =
        all.iter().map(|(name, auto)| item("", name, &auto.as_ref().map(mode_text).unwrap_or_default(), auto.as_ref())).collect();
    let (device_modes, device_index, source_name) = match input {
        Input::Device(device) => {
            (device.modes.clone(), all.iter().position(|(n, _)| *n == device.name).unwrap_or(0), device.name.as_str())
        }
        Input::Test => {
            devices.insert(0, item("", "Test pattern", &mode_text(mode), Some(mode)));
            (vec![mode.clone()], 0, "")
        }
        Input::Nothing => (Vec::new(), 0, ""),
    };
    let (modes, mode_index) = mode_items(&device_modes, mode, requested);
    ui.set_devices(model(devices));
    ui.set_device(device_index as i32);
    ui.set_modes(model(modes));
    ui.set_mode(mode_index);

    let inputs: Vec<String> = audio::audio_devices("Audio/Source")?.into_iter().map(|d| d.name).collect();
    let paired = inputs.iter().find(|n| !source_name.is_empty() && n.to_lowercase().contains(&source_name.to_lowercase()));
    let mut audio_items = vec![match paired {
        Some(name) => item("", &format!("Auto ({name})"), "", None),
        None => item("", "Auto", "No paired input", None),
    }];
    audio_items.push(item("", "None", "", None));
    audio_items.extend(inputs.iter().map(|n| item("", n, "", None)));
    ui.set_audio_inputs(model(audio_items));
    ui.set_audio_input(if audio_on { 0 } else { 1 });

    if let Input::Nothing = input {
        ui.set_notice(
            "No capture device found. Connect a capture card or a webcam and start again, or run with --test-video.".into(),
        );
    } else {
        let (tier, badge) = tier(mode);
        ui.set_resolution(size_text(mode).into());
        ui.set_badge(badge.into());
        ui.set_tier(tier);
        ui.set_fps(mode.fps_f64().round() as i32);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("{}", capture::init()?);

    let all = devices::video_devices()?;
    if args.common.list {
        devices::print_list(&all);
        return Ok(());
    }
    let dark = match args.theme.as_str() {
        "dark" => true,
        "light" => false,
        other => anyhow::bail!("unknown theme {other:?} (dark, light)"),
    };
    // For the device picker, read before picking takes the list.
    let listed: Vec<(String, Option<Mode>)> = all.iter().map(|d| (d.name.clone(), mode::auto(&d.modes))).collect();
    let input = if args.test_video {
        Input::Test
    } else if all.is_empty() && args.ui == "float" {
        Input::Nothing
    } else {
        Input::Device(devices::pick(all, args.common.device.as_deref())?)
    };
    let mode = match &input {
        Input::Device(device) => video::choose_mode(device, args.common.mode.as_ref())?,
        _ => args.common.mode.clone().map_or_else(|| mode::parse("1920x1080@60:NV12"), Ok)?,
    };
    let source_name = match &input {
        Input::Device(device) => device.name.clone(),
        Input::Test => "Test pattern".to_string(),
        Input::Nothing => "No device".to_string(),
    };

    // Slint draws into our buffer; this must be set before creating any Slint component.
    let slint_window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SpikePlatform { window: slint_window.clone(), start: Instant::now() }))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let info = format!("{source_name} · {mode}");
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let ui = match args.ui.as_str() {
        "overlay" => {
            let ui = Overlay::new()?;
            ui.set_info(info.into());
            show(ui, Hooks::default(), dark, event_loop.create_proxy())?
        }
        "render-test" => {
            let ui = RenderTest::new()?;
            ui.set_info(info.into());
            show(ui, Hooks::default(), dark, event_loop.create_proxy())?
        }
        "theme-test" => {
            let ui = ThemeTest::new()?;
            ui.set_info(info.into());
            show(ui, Hooks::default(), dark, event_loop.create_proxy())?
        }
        "float" => {
            let ui = Float::new()?;
            fill_float(&ui, &input, &mode, args.common.mode.is_some(), args.audio, &listed)?;
            let weak = ui.as_weak();
            let video_area =
                move || weak.upgrade().map(|ui| ui.get_video_area()).map(|a| [a.x, a.y, a.width, a.height, a.radius]);
            let weak = ui.as_weak();
            let close_menu = move || {
                weak.upgrade().is_some_and(|ui| {
                    let open = ui.get_menu_open();
                    ui.invoke_close_menus();
                    open
                })
            };
            ui.set_ambient_light(args.ambient);
            let weak = ui.as_weak();
            let ambient = move || weak.upgrade().is_some_and(|ui| ui.get_ambient_light());
            let hooks =
                Hooks { video_area: Box::new(video_area), close_menu: Box::new(close_menu), ambient: Box::new(ambient) };
            show(ui, hooks, dark, event_loop.create_proxy())?
        }
        other => anyhow::bail!("unknown UI {other:?} (overlay, render-test, theme-test, float)"),
    };

    let proxy = event_loop.create_proxy();
    let slot = Arc::new(Mutex::new(None));

    // Built before the video source takes the device.
    let sound = match (&input, args.audio) {
        (Input::Device(device), true) => {
            let input = audio::paired_input(audio::audio_devices("Audio/Source")?, &device.name)
                .ok_or_else(|| anyhow::anyhow!("no audio input paired with {} (see audio-preview --list-audio)", device.name))?;
            println!("audio: {} -> system default", input.name);
            Some(audio::build_preview(&input, None, &audio::PreviewOptions::chosen())?)
        }
        (_, true) => anyhow::bail!("--audio needs a capture device"),
        _ => None,
    };

    let source = match &input {
        Input::Device(device) => Some(video::build(device, mode.clone(), args.common.range)?),
        Input::Test => Some(video::test(mode.clone())?),
        Input::Nothing => None,
    };
    let mut _encoder = None;
    let mut _stats = None;
    let pipeline = match source {
        Some(source) => {
            println!("{source_name}: {} -> layered", source.mode);
            let convert = gst::parse::bin_from_description(
                if cfg!(windows) { "d3d12upload ! d3d12convert ! d3d12download" } else { "videoconvert" },
                true,
            )?;
            let sink = appsink(slot.clone(), proxy.clone());
            let pipeline = gst::Pipeline::with_name("layered-preview");
            // The preview always goes through a tee and its own queue, so a run with --encoder
            // differs from a run without it only by the encoder branch.
            let tee = gst::ElementFactory::make("tee").build()?;
            let preview_queue = gst::ElementFactory::make("queue")
                .property("max-size-buffers", 1u32)
                .property("max-size-bytes", 0u32)
                .property("max-size-time", 0u64)
                .property_from_str("leaky", "downstream")
                .build()?;
            let chain = [source.bin.upcast_ref::<gst::Element>(), &tee, &preview_queue, convert.upcast_ref(), sink.upcast_ref()];
            pipeline.add_many(chain)?;
            gst::Element::link_many(chain)?;
            if args.encoder {
                let branch = encoder::build()?;
                println!("encoder: {}", branch.name);
                pipeline.add(&branch.bin)?;
                tee.link(&branch.bin)?;
                if args.common.stats || args.common.record {
                    branch.report();
                }
                _encoder = Some(branch);
            }
            if args.common.stats || args.common.record {
                let variant = if _encoder.is_some() { "encoder-on" } else { "software-ui" };
                let info = stats::RunInfo { path: "slint-layered".into(), variant: variant.into(), mode: mode.to_string() };
                _stats = Some(stats::StatsReporter::start(&pipeline, sink.upcast_ref(), info, args.common.record)?);
            }

            let bus = pipeline.bus().expect("pipeline has a bus");
            let error_proxy = proxy.clone();
            std::thread::spawn(move || {
                for msg in bus.iter_timed(gst::ClockTime::NONE) {
                    if let gst::MessageView::Error(err) = msg.view() {
                        let text =
                            format!("Error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug());
                        let _ = error_proxy.send_event(UserEvent::Error(text));
                        break;
                    }
                }
            });
            Some(pipeline)
        }
        None => {
            println!("No video capture device found: connect a capture card or a webcam, or use --test-video.");
            None
        }
    };

    println!("F11: fullscreen on the current monitor. Esc: close a menu, then leave fullscreen.");
    let mut app = App {
        present: args.present,
        backend: args.backend,
        window: None,
        gpu: None,
        slint_window,
        _ui: ui.component,
        backdrop: ui.backdrop,
        video_area: ui.video_area,
        set_fullscreen: ui.set_fullscreen,
        close_menu: ui.close_menu,
        ambient: ui.ambient,
        ambient_strength: args.ambient_strength.clamp(0.0, 1.0),
        ui_pixels: Vec::new(),
        slot,
        pipeline: pipeline.clone(),
        cursor: slint::LogicalPosition::new(0.0, 0.0),
        scale: 1.0,
        stats: args.common.stats || args.common.record,
        presented: 0,
        delays_ms: Vec::new(),
        ui_ms: Vec::new(),
        last_report: Instant::now(),
        last_present: Instant::now(),
        refresh: Duration::from_millis(16),
    };
    if let Some(sound) = &sound {
        let bus = sound.pipeline.bus().expect("pipeline has a bus");
        std::thread::spawn(move || {
            for msg in bus.iter_timed(gst::ClockTime::NONE) {
                if let gst::MessageView::Error(err) = msg.view() {
                    eprintln!("audio stopped: {} ({:?})", err.error(), err.debug());
                    break;
                }
            }
        });
        sound.pipeline.set_state(gst::State::Playing)?;
    }
    event_loop.run_app(&mut app)?;
    // GStreamer's pipewiresink deadlocks when stopped in about half the runs (CachyOS, PipeWire
    // 1.6.10), and a client stuck that way stalls every other app's sound. The audio no longer
    // uses it, but in case another element does the same: leave without running destructors,
    // which could wait on the same lock, if stopping takes too long.
    #[cfg(unix)]
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(3));
        eprintln!("stopping the pipelines took over 3 s (GStreamer PipeWire deadlock): leaving anyway");
        unsafe { libc::_exit(0) };
    });
    if let Some(pipeline) = &pipeline {
        pipeline.set_state(gst::State::Null)?;
    }
    if let Some(sound) = &sound {
        sound.pipeline.set_state(gst::State::Null)?;
    }
    Ok(())
}
