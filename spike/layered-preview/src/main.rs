//! Own render loop (research R13, T047): winit window + wgpu swap chain presenting each video frame
//! as soon as it arrives, with a Slint UI layer (software renderer) composited over the video.
//!
//! F11 or F toggles fullscreen on the current monitor; Esc or Q quits.

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use capture::{audio, cli::CommonArgs, devices, encoder, stats, video};
use clap::Parser;
use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent as SlintEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
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

    /// UI drawn over the video: overlay, render-test (what the software renderer draws, T048) or
    /// theme-test (the dark and light palettes, T027) or components (the shared components, T028)
    #[arg(long, default_value = "overlay")]
    ui: String,
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
    Error(String),
}

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
                    visibility: wgpu::ShaderStages::VERTEX,
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
        Ok(Self { surface, device, queue, config, pipeline, layout, sampler, video: None, ui: None })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn make_layer(&self, width: u32, height: u32) -> Layer {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layer"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let rect = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: rect.as_entire_binding() },
            ],
        });
        Layer { texture, bind_group, rect, width, height }
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

    /// Composites the video (letterboxed) and the UI layer, then presents immediately.
    fn render(&self, backdrop: wgpu::Color) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
        };
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        if let Some(video) = &self.video {
            let scale = (w / video.width as f32).min(h / video.height as f32);
            let (rw, rh) = (video.width as f32 * scale / w, video.height as f32 * scale / h);
            self.queue.write_buffer(&video.rect, 0, bytemuck::cast_slice(&[-rw, rh, rw, -rh]));
        }
        if let Some(ui) = &self.ui {
            self.queue.write_buffer(&ui.rect, 0, bytemuck::cast_slice(&[-1.0f32, 1.0, 1.0, -1.0]));
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
            for layer in [&self.video, &self.ui].into_iter().flatten() {
                pass.set_bind_group(0, &layer.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        true
    }
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
    ui_pixels: Vec<PremultipliedRgbaColor>,
    slot: Arc<Mutex<Option<Frame>>>,
    pipeline: gst::Pipeline,
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
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Reads the refresh interval of the monitor the window is on.
    fn update_refresh(&mut self, window: &Window) {
        if let Some(mhz) = window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).filter(|r| *r > 0) {
            self.refresh = Duration::from_secs_f64(1000.0 / mhz as f64);
        }
    }

    /// Draws the UI layer if Slint says it changed, then composites and presents.
    fn redraw(&mut self) -> bool {
        self.draw_ui();
        let Some(gpu) = self.gpu.as_ref() else { return false };
        let presented = gpu.render((self.backdrop)());
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
            "[loop]  {:>4} fps presented | frame ready -> presented median {median:.1} ms | UI {:>3} redraws, median {ui_median:.1} ms, max {ui_max:.1} ms",
            self.presented,
            self.ui_ms.len()
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
        if let Err(err) = self.pipeline.set_state(gst::State::Playing) {
            eprintln!("Could not start capture: {err}");
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Error(message) => {
                eprintln!("{message}");
                event_loop.exit();
            }
            UserEvent::FrameReady => {
                let Some(frame) = self.slot.lock().unwrap().take() else { return };
                if let Some(gpu) = self.gpu.as_mut() {
                    if gpu.video.as_ref().is_none_or(|v| v.width != frame.width || v.height != frame.height) {
                        gpu.video = Some(gpu.make_layer(frame.width, frame.height));
                    }
                    gpu.upload(gpu.video.as_ref().unwrap(), &frame.data);
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
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (position, button) = (self.cursor, PointerEventButton::Left);
                self.dispatch(match state {
                    ElementState::Pressed => SlintEvent::PointerPressed { position, button },
                    ElementState::Released => SlintEvent::PointerReleased { position, button },
                });
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => match event.logical_key {
                Key::Named(NamedKey::F11) => toggle_fullscreen(self.window.as_deref()),
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Character(c) if c.eq_ignore_ascii_case("f") => toggle_fullscreen(self.window.as_deref()),
                Key::Character(c) if c.eq_ignore_ascii_case("q") => event_loop.exit(),
                _ => {}
            },
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
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            } else {
                wake = Some(wake.map_or(next, |w| w.min(next)));
            }
        } else if self.draw_ui() {
            // A timer changed the UI (the chrome hiding): present it now.
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

/// Shows a UI component; returns it, to keep it alive, and a reader of its theme's backdrop color.
fn show<C>(ui: C) -> anyhow::Result<(Box<dyn std::any::Any>, Box<dyn Fn() -> wgpu::Color>)>
where
    C: slint::ComponentHandle + 'static,
    for<'a> Theme<'a>: slint::Global<'a, C>,
{
    ui.show()?;
    let weak = ui.as_weak();
    let backdrop = move || {
        let deep = weak.upgrade().map(|ui| ui.global::<Theme>().get_c().deep).unwrap_or_default();
        let channel = |v: u8| v as f64 / 255.0;
        wgpu::Color { r: channel(deep.red()), g: channel(deep.green()), b: channel(deep.blue()), a: 1.0 }
    };
    Ok((Box::new(ui), Box::new(backdrop)))
}

fn toggle_fullscreen(window: Option<&Window>) {
    if let Some(w) = window {
        w.set_fullscreen(if w.fullscreen().is_some() { None } else { Some(Fullscreen::Borderless(None)) });
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("{}", capture::init()?);

    let all = devices::video_devices()?;
    if args.common.list {
        devices::print_list(&all);
        return Ok(());
    }
    let device = devices::pick(all, args.common.device.as_deref())?;
    let mode = video::choose_mode(&device, args.common.mode.as_ref())?;
    println!("{}: {mode} -> layered", device.name);

    // Slint draws into our buffer; this must be set before creating any Slint component.
    let slint_window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(SpikePlatform { window: slint_window.clone(), start: Instant::now() }))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let info = format!("{} · {mode}", device.name);
    let (ui, backdrop) = match args.ui.as_str() {
        "overlay" => {
            let ui = Overlay::new()?;
            ui.set_info(info.into());
            show(ui)?
        }
        "render-test" => {
            let ui = RenderTest::new()?;
            ui.set_info(info.into());
            show(ui)?
        }
        "theme-test" => {
            let ui = ThemeTest::new()?;
            ui.set_info(info.into());
            show(ui)?
        }
        "components" => {
            let ui = ComponentsTest::new()?;
            ui.set_info(info.into());
            show(ui)?
        }
        other => anyhow::bail!("unknown UI {other:?} (overlay, render-test, theme-test, components)"),
    };

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let slot = Arc::new(Mutex::new(None));

    // Built before the video source takes the device.
    let sound = if args.audio {
        let input = audio::paired_input(audio::audio_devices("Audio/Source")?, &device.name)
            .ok_or_else(|| anyhow::anyhow!("no audio input paired with {} (see audio-preview --list-audio)", device.name))?;
        println!("audio: {} -> system default", input.name);
        Some(audio::build_preview(&input, None, &audio::PreviewOptions::chosen())?)
    } else {
        None
    };

    let source = video::build(&device, mode.clone(), args.common.range)?;
    let convert = gst::parse::bin_from_description(
        if cfg!(windows) { "d3d12upload ! d3d12convert ! d3d12download" } else { "videoconvert" },
        true,
    )?;
    let sink = appsink(slot.clone(), proxy.clone());
    let pipeline = gst::Pipeline::with_name("layered-preview");
    // The preview always goes through a tee and its own queue, so a run with --encoder differs from
    // a run without it only by the encoder branch.
    let tee = gst::ElementFactory::make("tee").build()?;
    let preview_queue = gst::ElementFactory::make("queue")
        .property("max-size-buffers", 1u32)
        .property("max-size-bytes", 0u32)
        .property("max-size-time", 0u64)
        .property_from_str("leaky", "downstream")
        .build()?;
    pipeline.add_many([source.bin.upcast_ref::<gst::Element>(), &tee, &preview_queue, convert.upcast_ref(), sink.upcast_ref()])?;
    gst::Element::link_many([source.bin.upcast_ref::<gst::Element>(), &tee, &preview_queue, convert.upcast_ref(), sink.upcast_ref()])?;
    let encoder = if args.encoder {
        let branch = encoder::build()?;
        println!("encoder: {}", branch.name);
        pipeline.add(&branch.bin)?;
        tee.link(&branch.bin)?;
        if args.common.stats || args.common.record {
            branch.report();
        }
        Some(branch)
    } else {
        None
    };

    let _stats = if args.common.stats || args.common.record {
        let variant = if encoder.is_some() { "encoder-on" } else { "software-ui" };
        let info = stats::RunInfo { path: "slint-layered".into(), variant: variant.into(), mode: mode.to_string() };
        Some(stats::StatsReporter::start(&pipeline, sink.upcast_ref(), info, args.common.record)?)
    } else {
        None
    };

    let bus = pipeline.bus().expect("pipeline has a bus");
    let error_proxy = proxy.clone();
    std::thread::spawn(move || {
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            if let gst::MessageView::Error(err) = msg.view() {
                let text = format!("Error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug());
                let _ = error_proxy.send_event(UserEvent::Error(text));
                break;
            }
        }
    });

    println!("F11 or F: fullscreen on the current monitor. Esc or Q: quit.");
    let mut app = App {
        present: args.present,
        backend: args.backend,
        window: None,
        gpu: None,
        slint_window,
        _ui: ui,
        backdrop,
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
    pipeline.set_state(gst::State::Null)?;
    if let Some(sound) = &sound {
        sound.pipeline.set_state(gst::State::Null)?;
    }
    Ok(())
}
