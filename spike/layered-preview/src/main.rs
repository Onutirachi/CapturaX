//! Own render loop (research R13, T047): winit window + wgpu swap chain presenting each video frame
//! as soon as it arrives, with a Slint UI layer (software renderer) composited over the video.
//!
//! F11 toggles fullscreen on the current monitor; Esc closes an open menu, then leaves fullscreen.
//! Close the window to quit. The Float direction draws its own title bar on a frameless window.

mod ambient;
mod gpu;
mod window;

use std::rc::Rc;
use std::sync::Arc;
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
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, ResizeDirection, Window, WindowId};

use gpu::{Gpu, Mailbox, Scene, UiImage, VideoThread};
use window::WindowAction;

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

    /// Starting language: en or pt-BR (default: the system's; switchable at runtime in the Float
    /// direction)
    #[arg(long)]
    lang: Option<String>,

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
    /// The UI asked for fullscreen on or off (the dock's button).
    ToggleFullscreen,
    /// The title bar asked the window to move, minimize, maximize or close.
    Window(WindowAction),
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

/// Appsink handing each RGBA frame to the video thread.
fn appsink(mailbox: Arc<Mailbox>) -> gst_app::AppSink {
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
                mailbox.send_frame(Frame { width, height, data, ready: Instant::now() });
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
    /// The window's size in physical pixels, which the UI is drawn at.
    size: winit::dpi::PhysicalSize<u32>,
    /// Presents the video and the UI layer (research R15); started with the window.
    video: Option<VideoThread>,
    mailbox: Arc<Mailbox>,
    /// The scene last handed to the video thread.
    scene: Option<Scene>,
    slint_window: Rc<MinimalSoftwareWindow>,
    /// The Slint component shown; kept alive for the whole run.
    _ui: Box<dyn std::any::Any>,
    /// The theme's deepest tone, read each frame: the ground around the video follows the theme.
    backdrop: Box<dyn Fn() -> wgpu::Color>,
    /// Where the UI wants the video, in logical pixels (none: the whole window).
    video_area: Box<dyn Fn() -> Option<Area>>,
    /// Tells the UI whether the window is fullscreen.
    set_fullscreen: Box<dyn Fn(bool)>,
    /// Tells the UI whether the window is maximized.
    set_maximized: Box<dyn Fn(bool)>,
    /// Closes an open menu; false when none was open (Esc then leaves fullscreen).
    close_menu: Box<dyn Fn() -> bool>,
    /// The UI draws its own title bar: the window has no system frame and resizes from its edges.
    frameless: bool,
    /// The edge or corner under the pointer, where a left press resizes the window.
    resize: Option<ResizeDirection>,
    /// The left button is held on the UI: the edges do not take the pointer until it is released,
    /// so a fast drag of the title bar toward an edge keeps the normal cursor.
    pressed: bool,
    /// Whether the UI has the ambient light on, and its strength.
    ambient: Box<dyn Fn() -> bool>,
    ambient_strength: f32,
    ui_pixels: Vec<PremultipliedRgbaColor>,
    /// None when there is no video source (no device found).
    pipeline: Option<gst::Pipeline>,
    cursor: slint::LogicalPosition,
    scale: f32,
    /// The video thread prints its stats each second.
    stats: bool,
    /// When the UI was last handed over and the monitor's refresh interval: animations redraw at
    /// most once per refresh.
    last_ui: Instant,
    refresh: Duration,
    /// The monitor the refresh interval was read from.
    monitor: Option<MonitorHandle>,
}

impl App {
    fn dispatch(&self, event: SlintEvent) {
        self.slint_window.window().dispatch_event(event);
        self.request_redraw();
    }

    /// Reads the refresh interval of the monitor the window is on, when it is another monitor:
    /// reading it is slow on Windows, and a window move sends many events.
    fn update_refresh(&mut self, window: &Window) {
        let monitor = window.current_monitor();
        if monitor == self.monitor {
            return;
        }
        self.monitor = monitor;
        if let Some(mhz) = self.monitor.as_ref().and_then(|m| m.refresh_rate_millihertz()).filter(|r| *r > 0) {
            self.refresh = Duration::from_secs_f64(1000.0 / mhz as f64);
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Advances timers and animations, draws the UI if Slint says it changed, and hands it to the
    /// video thread with the scene when either changed.
    fn update_ui(&mut self) {
        if self.video.is_none() {
            return;
        }
        slint::platform::update_timers_and_animations();
        let (w, h) = (self.size.width.max(1), self.size.height.max(1));
        let ui_pixels = &mut self.ui_pixels;
        if ui_pixels.len() != (w * h) as usize {
            *ui_pixels = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
        }
        let started = Instant::now();
        let drawn = self.slint_window.draw_if_needed(|renderer| {
            renderer.render(ui_pixels, w as usize);
        });
        let ui = drawn.then(|| {
            let pixels = bytemuck::cast_slice(&self.ui_pixels).to_vec();
            UiImage { width: w, height: h, pixels, cost_ms: started.elapsed().as_secs_f64() * 1e3 }
        });
        let scale = self.scale;
        let scene = Scene {
            backdrop: (self.backdrop)(),
            area: (self.video_area)().map(|a| a.map(|v| v * scale)),
            ambient: (self.ambient)().then_some(self.ambient_strength),
        };
        if ui.is_some() || self.scene != Some(scene) {
            self.mailbox.send_ui(ui, scene, (self.size.width, self.size.height));
            self.scene = Some(scene);
            self.last_ui = Instant::now();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(event_loop.create_window(window::attributes(self.frameless)).expect("window"));
        let size = window.inner_size();
        self.size = size;
        self.scale = window.scale_factor() as f32;
        self.update_refresh(&window);
        self.slint_window.window().dispatch_event(SlintEvent::ScaleFactorChanged { scale_factor: self.scale });
        self.slint_window.set_size(slint::PhysicalSize::new(size.width, size.height));
        match Gpu::new(window.clone(), self.present.as_deref(), self.backend.as_deref()) {
            Ok(gpu) => self.video = Some(VideoThread::start(gpu, self.mailbox.clone(), self.stats)),
            Err(err) => {
                eprintln!("GPU setup failed: {err}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.update_ui();
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
            UserEvent::Window(action) => {
                let Some(w) = self.window.clone() else { return };
                if !window::act(&w, action) {
                    event_loop.exit();
                } else if let WindowAction::Drag = action {
                    // The system move swallows the button release: let the bar drop its press.
                    self.pressed = false;
                    self.dispatch(SlintEvent::PointerExited);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.size = size;
                self.slint_window.set_size(slint::PhysicalSize::new(size.width, size.height));
                if let Some(w) = &self.window {
                    (self.set_maximized)(w.is_maximized());
                }
                self.update_ui();
            }
            WindowEvent::Moved(_) => {
                self.mailbox.count_move();
                if let Some(window) = self.window.clone() {
                    self.update_refresh(&window);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                self.dispatch(SlintEvent::ScaleFactorChanged { scale_factor: self.scale });
            }
            WindowEvent::RedrawRequested => self.update_ui(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(w) = self.window.as_ref().filter(|_| self.frameless && !self.pressed) {
                    let resize = window::resize_direction(w, position);
                    if resize != self.resize {
                        w.set_cursor(window::cursor(resize));
                        self.resize = resize;
                    }
                }
                self.cursor = slint::LogicalPosition::new(position.x as f32 / self.scale, position.y as f32 / self.scale);
                self.dispatch(SlintEvent::PointerMoved { position: self.cursor });
            }
            WindowEvent::CursorLeft { .. } => {
                self.resize = None;
                self.dispatch(SlintEvent::PointerExited);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.resize.is_some() =>
            {
                // The edges belong to the window, not the UI.
                if let (Some(w), Some(direction)) = (&self.window, self.resize)
                    && let Err(err) = w.drag_resize_window(direction)
                {
                    eprintln!("window resize: {err}");
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    self.pressed = state == ElementState::Pressed;
                }
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
        let animating = self.slint_window.window().has_active_animations();
        if !animating || Instant::now() >= self.last_ui + self.refresh {
            self.update_ui();
        }
        let mut wake = slint::platform::duration_until_next_timer_update().map(|wait| Instant::now() + wait);
        if animating {
            let next = (self.last_ui + self.refresh).max(Instant::now() + Duration::from_millis(1));
            wake = Some(wake.map_or(next, |w| w.min(next)));
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // The video thread holds the surface: it stops while the window still exists.
        if let Some(video) = self.video.as_mut() {
            video.stop();
        }
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
    set_maximized: Box<dyn Fn(bool)>,
    close_menu: Box<dyn Fn() -> bool>,
    ambient: Box<dyn Fn() -> bool>,
    frameless: bool,
}

/// What a UI offers the loop beyond the shared globals; the defaults suit a UI without them.
struct Hooks {
    /// Where it wants the video (none: the whole window).
    video_area: Box<dyn Fn() -> Option<Area>>,
    /// Closes an open menu, false when none was open.
    close_menu: Box<dyn Fn() -> bool>,
    /// Whether the ambient light is on.
    ambient: Box<dyn Fn() -> bool>,
    /// Whether it draws its own title bar (the window then has no system frame).
    title_bar: bool,
}

impl Default for Hooks {
    fn default() -> Self {
        Self {
            video_area: Box::new(|| None),
            close_menu: Box::new(|| false),
            ambient: Box::new(|| false),
            title_bar: false,
        }
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
    let state = ui.global::<WindowState>();
    let send = move |event: UserEvent| {
        let _ = proxy.send_event(event);
    };
    let on = send.clone();
    state.on_toggle_fullscreen(move || on(UserEvent::ToggleFullscreen));
    let on = send.clone();
    state.on_drag(move || on(UserEvent::Window(WindowAction::Drag)));
    let on = send.clone();
    state.on_minimize(move || on(UserEvent::Window(WindowAction::Minimize)));
    let on = send.clone();
    state.on_toggle_maximize(move || on(UserEvent::Window(WindowAction::ToggleMaximize)));
    state.on_close(move || send(UserEvent::Window(WindowAction::Close)));
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
    let weak = ui.as_weak();
    let set_maximized = move |on: bool| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<WindowState>().set_maximized(on);
        }
    };
    Ok(Ui {
        component: Box::new(ui),
        backdrop: Box::new(backdrop),
        video_area: hooks.video_area,
        set_fullscreen: Box::new(set_fullscreen),
        set_maximized: Box::new(set_maximized),
        close_menu: hooks.close_menu,
        ambient: hooks.ambient,
        frameless: hooks.title_bar,
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
        Some(name) => item("", "Auto", name, None),
        None => item("", "Auto", "No paired input", None),
    }];
    audio_items.push(item("", "None", "", None));
    audio_items.extend(inputs.iter().map(|n| item("", n, "", None)));
    ui.set_audio_inputs(model(audio_items));
    ui.set_audio_input(if audio_on { 0 } else { 1 });

    if let Input::Nothing = input {
        ui.set_no_device(true);
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
            // Picked after the component exists; the Slint text follows at once.
            if let Some(lang) = &args.lang {
                slint::select_bundled_translation(lang).map_err(|e| anyhow::anyhow!("language {lang:?}: {e}"))?;
            }
            ui.on_select_language(|lang| {
                if let Err(e) = slint::select_bundled_translation(&lang) {
                    eprintln!("language {lang:?}: {e}");
                }
            });
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
            let hooks = Hooks {
                video_area: Box::new(video_area),
                close_menu: Box::new(close_menu),
                ambient: Box::new(ambient),
                title_bar: true,
            };
            show(ui, hooks, dark, event_loop.create_proxy())?
        }
        other => anyhow::bail!("unknown UI {other:?} (overlay, render-test, theme-test, float)"),
    };

    let proxy = event_loop.create_proxy();
    let mailbox = Arc::new(Mailbox::default());

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
            let sink = appsink(mailbox.clone());
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
        size: Default::default(),
        video: None,
        mailbox,
        scene: None,
        slint_window,
        _ui: ui.component,
        backdrop: ui.backdrop,
        video_area: ui.video_area,
        set_fullscreen: ui.set_fullscreen,
        set_maximized: ui.set_maximized,
        close_menu: ui.close_menu,
        frameless: ui.frameless,
        resize: None,
        pressed: false,
        ambient: ui.ambient,
        ambient_strength: args.ambient_strength.clamp(0.0, 1.0),
        ui_pixels: Vec::new(),
        pipeline: pipeline.clone(),
        cursor: slint::LogicalPosition::new(0.0, 0.0),
        scale: 1.0,
        stats: args.common.stats || args.common.record,
        last_ui: Instant::now(),
        refresh: Duration::from_millis(16),
        monitor: None,
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
