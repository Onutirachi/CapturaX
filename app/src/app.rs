//! The winit application (plan, startup and quit sequences): the window, the event loop on the main
//! thread, the Slint UI drawn there and handed to the video thread with the scene.

use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Global};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, ResizeDirection, Window, WindowId};

use crate::cli::StartupOptions;
use crate::frame::{self, Escape};
use crate::notices::{Kind, Notices};
use crate::render::{Presentation, Scene, VideoThread};
use crate::test_video::TestVideo;
use crate::ui::{Menus, NoticeKind, Texts, Ui, WindowState};

/// Exit code when no renderer could start (contracts/cli.md).
pub const EXIT_NO_RENDERER: i32 = 2;
/// Exit code when `--test-video` cannot play: GStreamer or `videotestsrc` is missing.
pub const EXIT_NO_TEST_VIDEO: i32 = 69;

/// Events reaching the main thread from the UI's callbacks and, later, from the platform layer.
#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Window(WindowAction),
    DismissNotice,
}

/// What the UI asks the window to do.
#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    Minimize,
    ToggleMaximize,
    Close,
    ToggleFullscreen,
    StartMove,
}

pub struct App {
    options: StartupOptions,
    process_start: Instant,
    store: settings::Store,
    ui: Ui,
    notices: Notices,
    window: Option<Arc<Window>>,
    video: Option<VideoThread>,
    test_video: Option<TestVideo>,
    /// The scene last handed to the video thread.
    scene: Option<Scene>,
    /// When the UI was last handed over and the monitor's refresh interval: animations redraw at
    /// most once per refresh.
    last_ui: Instant,
    refresh: Duration,
    /// The monitor the refresh interval was read from.
    monitor: Option<MonitorHandle>,
    /// The edge or corner under the pointer, where a press resizes the window.
    resize: Option<ResizeDirection>,
    pub exit_code: i32,
}

impl App {
    pub fn new(
        options: StartupOptions,
        process_start: Instant,
        store: settings::Store,
        ui: Ui,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        let state = ui.app.global::<WindowState>();
        let send = move |event: UserEvent| {
            let _ = proxy.send_event(event);
        };
        let on = send.clone();
        state.on_minimize(move || on(UserEvent::Window(WindowAction::Minimize)));
        let on = send.clone();
        state.on_toggle_maximize(move || on(UserEvent::Window(WindowAction::ToggleMaximize)));
        let on = send.clone();
        state.on_close(move || on(UserEvent::Window(WindowAction::Close)));
        let on = send.clone();
        state.on_toggle_fullscreen(move || on(UserEvent::Window(WindowAction::ToggleFullscreen)));
        let on = send.clone();
        state.on_start_move(move || on(UserEvent::Window(WindowAction::StartMove)));
        ui.app.on_dismiss_notice(move || send(UserEvent::DismissNotice));
        Self {
            options,
            process_start,
            store,
            ui,
            notices: Notices::default(),
            window: None,
            video: None,
            test_video: None,
            scene: None,
            last_ui: Instant::now(),
            refresh: Duration::from_millis(16),
            monitor: None,
            resize: None,
            exit_code: 0,
        }
    }

    /// The command line this run started with.
    pub fn options(&self) -> &StartupOptions {
        &self.options
    }

    /// Queues a notice; it shows at once if none is on screen.
    #[expect(dead_code, reason = "the settings location notices use it (spec US3)")]
    pub fn notify(&mut self, notice: crate::notices::Notice) {
        if self.notices.push(notice) {
            self.show_notice();
        }
    }

    fn show_notice(&mut self) {
        let app = &self.ui.app;
        match self.notices.current() {
            Some(n) => {
                app.set_notice_kind(match n.kind {
                    Kind::Info => NoticeKind::Info,
                    Kind::Warning => NoticeKind::Warning,
                });
                app.set_notice_text(n.text.as_str().into());
                app.set_notice_visible(true);
            }
            None => app.set_notice_visible(false),
        }
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Reads the refresh interval of the monitor the window is on, when it is another monitor:
    /// reading it is slow on Windows, and a window move sends many events.
    fn update_refresh(&mut self, window: &Window) {
        let monitor = window.current_monitor();
        if monitor == self.monitor {
            return;
        }
        self.monitor = monitor;
        if let Some(mhz) = self
            .monitor
            .as_ref()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|r| *r > 0)
        {
            self.refresh = Duration::from_secs_f64(1000.0 / f64::from(mhz));
        }
    }

    /// Advances timers and animations, draws the UI if Slint says it changed, and hands it to the
    /// video thread with the scene when either changed.
    fn update_ui(&mut self) {
        let Some(video) = &self.video else { return };
        self.ui.update_timers();
        let image = self.ui.draw_if_needed();
        let size = self.ui.size();
        let scene = Scene {
            video: self.ui.video_area(),
            backdrop: self.ui.backdrop(),
            size: (size.width, size.height),
        };
        if image.is_some() || self.scene != Some(scene) {
            video.inbox().send_ui(image, scene);
            self.scene = Some(scene);
            self.last_ui = Instant::now();
        }
    }

    fn act(&mut self, event_loop: &ActiveEventLoop, action: WindowAction) {
        let Some(w) = self.window.clone() else { return };
        match action {
            WindowAction::Close => event_loop.exit(),
            WindowAction::Minimize => w.set_minimized(true),
            WindowAction::ToggleMaximize => w.set_maximized(!w.is_maximized()),
            WindowAction::ToggleFullscreen => self.set_fullscreen(&w, w.fullscreen().is_none()),
            WindowAction::StartMove => {
                // A maximized window restores under the pointer: the system does it.
                if let Err(err) = w.drag_window() {
                    log::warn!("window move: {err}");
                }
            }
        }
        self.request_redraw();
    }

    /// Borderless fullscreen on the monitor the window is on (research R5); leaving it returns to
    /// the window's state before, maximized or not.
    fn set_fullscreen(&mut self, w: &Window, on: bool) {
        if on {
            platform::set_fullscreen_frame(w, true);
        }
        w.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        if !on {
            platform::set_fullscreen_frame(w, false);
        }
        self.ui.app.global::<WindowState>().set_fullscreen(on);
        self.set_resize(w, None);
    }

    fn set_resize(&mut self, w: &Window, direction: Option<ResizeDirection>) {
        if direction != self.resize {
            self.resize = direction;
            w.set_cursor(frame::cursor(direction));
        }
    }

    /// The window's own keys, before Slint sees them: F11 and Esc (FR-007). Returns true when the
    /// key was used here.
    fn window_key(&mut self, w: &Window, key: &Key) -> bool {
        match key {
            Key::Named(NamedKey::F11) => {
                self.set_fullscreen(w, w.fullscreen().is_none());
                true
            }
            Key::Named(NamedKey::Escape) => {
                let open = self.ui.app.global::<Menus>().get_open();
                match frame::escape(open, self.ui.app.get_settings_open(), w.fullscreen().is_some()) {
                    Escape::ToSlint | Escape::Nothing => false,
                    Escape::CloseSettings => {
                        self.ui.app.set_settings_open(false);
                        true
                    }
                    Escape::LeaveFullscreen => {
                        self.set_fullscreen(w, false);
                        true
                    }
                }
            }
            _ => false,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<Arc<Window>> {
        let saved = self.store.get().window;
        let attrs = Window::default_attributes()
            .with_title(Texts::get(&self.ui.app).get_app_name().as_str())
            .with_visible(false)
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(saved.width, saved.height))
            .with_min_inner_size(LogicalSize::new(settings::MIN_WIDTH, settings::MIN_HEIGHT))
            .with_window_icon(crate::icon::window_icon());
        Ok(Arc::new(event_loop.create_window(platform::window_attributes(attrs))?))
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match self.create_window(event_loop) {
            Ok(w) => w,
            Err(err) => {
                log::error!("window: {err:#}");
                self.exit_code = EXIT_NO_RENDERER;
                event_loop.exit();
                return;
            }
        };
        self.ui.set_scale(window.scale_factor() as f32);
        self.ui.set_size(window.inner_size());
        self.update_refresh(&window);
        let presentation = Presentation {
            backend: self.options.backend,
            present: self.options.present,
        };
        let video = match VideoThread::start(window.clone(), presentation, self.process_start, self.options.stats) {
            Ok(video) => video,
            Err(err) => {
                // The translated system message comes with the state message work (spec FR-036).
                log::error!("no usable renderer: {err}");
                self.exit_code = EXIT_NO_RENDERER;
                event_loop.exit();
                return;
            }
        };
        if let Some(size) = self.options.test_video {
            match TestVideo::start(size, video.inbox().clone()) {
                Ok(test_video) => self.test_video = Some(test_video),
                Err(err) => {
                    log::error!("{err}");
                    eprintln!("{err}");
                    self.video = Some(video);
                    self.exit_code = EXIT_NO_TEST_VIDEO;
                    event_loop.exit();
                    return;
                }
            }
        }
        self.video = Some(video);
        self.window = Some(window.clone());
        self.update_ui();
        window.set_visible(true);
        log::info!(
            "window shown {:.0} ms after start",
            self.process_start.elapsed().as_secs_f64() * 1e3
        );
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Window(action) => self.act(event_loop, action),
            UserEvent::DismissNotice => {
                self.notices.dismiss();
                self.show_notice();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.ui.set_size(size);
                if let Some(w) = &self.window {
                    self.ui.app.global::<WindowState>().set_maximized(w.is_maximized());
                }
                self.update_ui();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.ui.set_scale(scale_factor as f32);
                self.request_redraw();
            }
            WindowEvent::Moved(_) => {
                if let Some(video) = &self.video {
                    video.inbox().window_moved();
                }
                if let Some(window) = self.window.clone() {
                    self.update_refresh(&window);
                }
            }
            WindowEvent::RedrawRequested => self.update_ui(),
            WindowEvent::KeyboardInput { ref event, .. }
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && self
                        .window
                        .clone()
                        .is_some_and(|w| self.window_key(&w, &event.logical_key)) =>
            {
                self.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(w) = self.window.clone() {
                    let resizable = !w.is_maximized() && w.fullscreen().is_none();
                    let direction = frame::resize_direction(w.inner_size(), w.scale_factor(), position, resizable);
                    self.set_resize(&w, direction);
                }
                if self.ui.window_event(&event) {
                    self.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if self.resize.is_some() => {
                if let (Some(w), Some(direction)) = (&self.window, self.resize)
                    && let Err(err) = w.drag_resize_window(direction)
                {
                    log::warn!("window resize: {err}");
                }
            }
            other => {
                if self.ui.window_event(&other) {
                    self.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Keep Slint timers and animations running between video frames, so they do not step at
        // the video's frame rate. Read before updating: an update clears the flag until the next
        // draw reads the animated properties again.
        let mut animating = self.ui.has_active_animations();
        if !animating || Instant::now() >= self.last_ui + self.refresh {
            self.update_ui();
            // A timer fired in this update may have started an animation (the chrome hiding):
            // its first frame was just drawn, so the flag is set again.
            animating |= self.ui.has_active_animations();
        }
        let mut wake = self.ui.next_timer();
        if animating {
            let next = (self.last_ui + self.refresh).max(Instant::now() + Duration::from_millis(1));
            wake = Some(wake.map_or(next, |w| w.min(next)));
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Quit sequence (FR-014): the test video stops first so no frame arrives during the stop;
        // the video thread holds the surface, so it stops while the window still exists; then the
        // settings are flushed and the window destroyed.
        if let Some(mut test_video) = self.test_video.take() {
            test_video.stop();
        }
        if let Some(mut video) = self.video.take() {
            video.stop();
        }
        self.store.flush();
        self.window = None;
        log::info!("quit");
    }
}
