//! The winit application (plan, startup and quit sequences): the window, the event loop on the main
//! thread, the Slint UI drawn there and handed to the video thread with the scene.

use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Global};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, ResizeDirection, Window, WindowId};

use crate::cli::StartupOptions;
use crate::frame::{self, Escape};
use crate::icon;
use crate::notices::{Kind, Notices};
use crate::placement::{self, Rect};
use crate::render::{Presentation, Scene, VideoThread};
use crate::test_video::TestVideo;
use crate::ui::{Menus, NoticeKind, Prefs, Texts, Ui, WindowState};
use platform::autostart;
use platform::snap::{self, ButtonRect, Snap, SnapEvent};
use platform::tray::{self, IconImage, Labels, Tray, TrayEvent};

/// Exit code when no renderer could start (contracts/cli.md).
pub const EXIT_NO_RENDERER: i32 = 2;
/// Exit code when `--test-video` cannot play: GStreamer or `videotestsrc` is missing.
pub const EXIT_NO_TEST_VIDEO: i32 = 69;

/// Events reaching the main thread from the UI's callbacks and, later, from the platform layer.
#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Window(WindowAction),
    DismissNotice,
    Setting(SettingChange),
    /// Another start asked for the window (FR-013): show it, also from the tray, and focus it.
    ShowWindow,
    /// A click or a menu entry of the tray icon (FR-014a).
    Tray(TrayEvent),
    /// The pointer on the maximize button while the system owns it (FR-011).
    Snap(SnapEvent),
}

/// A change made in the settings panel.
#[derive(Debug, Clone, Copy)]
pub enum SettingChange {
    StartFullscreen(bool),
    KeepInTray(bool),
    StartWithSystem(bool),
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
    /// The window moved or resized since the placement was last saved.
    placement_changed: bool,
    /// The single-instance listener; dropped in the quit sequence, which releases the name.
    instance: Option<platform::Serving>,
    /// The tray icon, while Keep in tray is on and the desktop has a tray (FR-014a): closing the
    /// window then hides it instead of quitting.
    tray: Option<Tray>,
    tray_available: bool,
    /// The snap layouts flyout's hold on the window (FR-011, Windows 11) and the maximize button's
    /// rectangle it was last given.
    snap: Option<Snap>,
    snap_button: Option<ButtonRect>,
    proxy: EventLoopProxy<UserEvent>,
    pub exit_code: i32,
}

impl App {
    pub fn new(
        options: StartupOptions,
        process_start: Instant,
        store: settings::Store,
        ui: Ui,
        proxy: EventLoopProxy<UserEvent>,
        instance: Option<platform::Serving>,
    ) -> Self {
        let state = ui.app.global::<WindowState>();
        let tray_available = tray::available();
        ui.app.global::<Prefs>().set_tray_available(tray_available);
        let sender = proxy.clone();
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
        let on = send.clone();
        ui.app.on_dismiss_notice(move || on(UserEvent::DismissNotice));
        bind_prefs(&ui, &store, send);
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
            placement_changed: false,
            instance,
            tray: None,
            tray_available,
            snap: None,
            snap_button: None,
            proxy: sender,
            exit_code: 0,
        }
    }

    /// Shows or removes the tray icon to match Keep in tray (FR-014a). Called on the main thread
    /// once the event loop runs (the Windows tray needs its message pump).
    fn sync_tray(&mut self) {
        let wanted = self.tray_available && self.store.get().general.keep_in_tray;
        if wanted == self.tray.is_some() {
            return;
        }
        if !wanted {
            self.tray = None;
            return;
        }
        let icons: Vec<IconImage<'_>> = icon::ICONS
            .iter()
            .map(|i| IconImage {
                size: i.size,
                rgba: i.rgba,
            })
            .collect();
        let proxy = self.proxy.clone();
        match tray::create(&icons, &self.tray_labels(), move |event| {
            let _ = proxy.send_event(UserEvent::Tray(event));
        }) {
            Ok(tray) => self.tray = Some(tray),
            // Without the icon, closing quits: nothing would bring the window back.
            Err(err) => log::warn!("tray: {err}"),
        }
    }

    fn tray_labels(&self) -> Labels {
        let texts = Texts::get(&self.ui.app);
        Labels {
            title: texts.get_app_name().into(),
            open: texts.get_tray_open().into(),
            quit: texts.get_tray_quit().into(),
        }
    }

    /// Closing the window: hidden in the tray while there is one, else the app quits (FR-014,
    /// FR-014a). Hidden, the video thread stops presenting.
    fn close(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(_)) = (&self.window, &self.tray) else {
            event_loop.exit();
            return;
        };
        window.set_visible(false);
        if let Some(video) = &self.video {
            video.set_visible(false);
        }
        log::info!("window hidden in the tray");
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

    /// Shows the window for another start (FR-013): out of the tray or the taskbar, then to the
    /// front where the platform allows it (winit's focus needs a visible, restored window). Before
    /// the window exists there is nothing to do: it is about to be shown.
    fn show_window(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Some(video) = &self.video {
            video.set_visible(true);
        }
        window.set_visible(true);
        window.set_minimized(false);
        window.focus_window();
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
        self.sync_snap_button();
    }

    /// Hands the maximize button's rectangle, as just laid out, to the snap layouts flyout
    /// (FR-011). None in fullscreen (no title bar) and while a menu is open (it takes every click).
    fn sync_snap_button(&mut self) {
        let (Some(snap), Some(w)) = (&self.snap, &self.window) else {
            return;
        };
        let app = &self.ui.app;
        let shown = w.fullscreen().is_none() && !app.global::<Menus>().get_open();
        let button = shown
            .then(|| {
                let position = PhysicalPosition::new(app.get_max_button_x(), app.get_max_button_y());
                let size = PhysicalSize::new(
                    u32::try_from(app.get_max_button_width()).unwrap_or(0),
                    u32::try_from(app.get_max_button_height()).unwrap_or(0),
                );
                frame::snap_button(position, size, w.scale_factor(), !w.is_maximized())
            })
            .flatten();
        if button == self.snap_button {
            return;
        }
        snap.set_button(button);
        self.snap_button = button;
        if button.is_none() {
            let state = app.global::<WindowState>();
            state.set_max_hover(false);
            state.set_max_pressed(false);
        }
    }

    fn act(&mut self, event_loop: &ActiveEventLoop, action: WindowAction) {
        let Some(w) = self.window.clone() else { return };
        match action {
            WindowAction::Close => self.close(event_loop),
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

    /// Saves where the window is (FR-012, data-model.md "Window placement"): not while fullscreen
    /// or minimized; while maximized only the flag, so the size to restore to is kept. Called once
    /// the events of a change are handled: on Windows, maximizing sends `Moved` before winit's
    /// `is_maximized` turns true.
    fn save_placement(&mut self, w: &Window) {
        let size = w.inner_size();
        if w.fullscreen().is_some() || w.is_minimized() == Some(true) || size.width == 0 || size.height == 0 {
            return;
        }
        self.update_refresh(w);
        let monitor = self.monitor.as_ref().and_then(MonitorHandle::name);
        let maximized = w.is_maximized();
        let scale = w.scale_factor();
        // None where the OS does not give it (Wayland).
        let position = w.outer_position().ok();
        let logical = |physical: u32| (f64::from(physical) / scale).round() as u32;
        self.store.update(|s| {
            let saved = &mut s.window;
            saved.maximized = maximized;
            if maximized {
                // Maximized on another monitor: restore centered there.
                if monitor != saved.monitor {
                    saved.monitor = monitor;
                    saved.x = None;
                    saved.y = None;
                }
                return;
            }
            saved.width = logical(size.width).max(settings::MIN_WIDTH);
            saved.height = logical(size.height).max(settings::MIN_HEIGHT);
            saved.x = position.map(|p| p.x);
            saved.y = position.map(|p| p.y);
            saved.monitor = monitor;
        });
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<Arc<Window>> {
        let handles: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        let primary = event_loop
            .primary_monitor()
            .and_then(|p| handles.iter().position(|m| *m == p))
            .unwrap_or(0);
        let monitors: Vec<placement::Monitor> = handles.iter().map(describe).collect();
        let placed = placement::fit(&self.store.get().window, &monitors, primary);
        log::info!("window placement {placed:?}");
        // Logical size at the chosen monitor's scale: winit converts it with the scale of the
        // monitor the window is created on, also on Wayland where the scale is not known yet.
        let scale = placed.monitor.map_or(1.0, |i| monitors[i].scale);
        let size = LogicalSize::new(f64::from(placed.size.0) / scale, f64::from(placed.size.1) / scale);
        let mut attrs = Window::default_attributes()
            .with_title(Texts::get(&self.ui.app).get_app_name().as_str())
            .with_visible(false)
            .with_decorations(false)
            .with_inner_size(size)
            .with_maximized(placed.maximized)
            .with_min_inner_size(LogicalSize::new(settings::MIN_WIDTH, settings::MIN_HEIGHT))
            .with_window_icon(crate::icon::window_icon());
        if let Some((x, y)) = placed.position {
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        Ok(Arc::new(event_loop.create_window(platform::window_attributes(attrs))?))
    }
}

/// Shows the saved settings in the settings panel and saves each change from it (FR-025, FR-029).
fn bind_prefs(ui: &Ui, store: &settings::Store, send: impl Fn(UserEvent) + Clone + 'static) {
    let prefs = ui.app.global::<Prefs>();
    let general = store.get().general;
    prefs.set_start_fullscreen(general.start_fullscreen);
    prefs.set_keep_in_tray(general.keep_in_tray);
    // Read from the system each time the panel opens: the user may change it in the system's
    // own tools (FR-014b).
    let app = ui.app.as_weak();
    prefs.on_opened(move || {
        if let Some(app) = app.upgrade() {
            app.global::<Prefs>().set_start_with_system(autostart::state());
        }
    });
    prefs.set_version(env!("CARGO_PKG_VERSION").into());
    let on = send.clone();
    prefs.on_start_fullscreen_changed(move |v| on(UserEvent::Setting(SettingChange::StartFullscreen(v))));
    let on = send.clone();
    prefs.on_keep_in_tray_changed(move |v| on(UserEvent::Setting(SettingChange::KeepInTray(v))));
    prefs.on_start_with_system_changed(move |v| send(UserEvent::Setting(SettingChange::StartWithSystem(v))));
}

/// A connected monitor for [`placement::fit`].
fn describe(m: &MonitorHandle) -> placement::Monitor {
    let rect = |p: PhysicalPosition<i32>, s: winit::dpi::PhysicalSize<u32>| Rect {
        x: p.x,
        y: p.y,
        width: s.width,
        height: s.height,
    };
    let bounds = rect(m.position(), m.size());
    placement::Monitor {
        name: m.name(),
        bounds,
        work_area: platform::work_area(m).map_or(bounds, |(p, s)| rect(p, s)),
        scale: m.scale_factor(),
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
        self.ui.set_size(platform::drawn_size(&window));
        platform::follow_dpi_rect(&window);
        let proxy = self.proxy.clone();
        self.snap = Some(snap::attach(&window, move |event| {
            let _ = proxy.send_event(UserEvent::Snap(event));
        }));
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
        self.sync_tray();
        // Started by the system with Keep in tray on: only the tray icon, whatever Start in
        // fullscreen says (FR-014b). Without a tray the window opens as usual.
        if self.options.autostart && self.tray.is_some() {
            if let Some(video) = &self.video {
                video.set_visible(false);
            }
            log::info!("started hidden in the tray");
            return;
        }
        // The start mode (FR-012a): fullscreen on the monitor the window was placed on; leaving it
        // returns to that placement. The command line's choice is not saved.
        if self
            .options
            .starts_fullscreen(self.store.get().general.start_fullscreen)
        {
            self.set_fullscreen(&window, true);
        }
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
            UserEvent::Setting(change) => match change {
                SettingChange::StartFullscreen(on) => self.store.update(|s| s.general.start_fullscreen = on),
                SettingChange::KeepInTray(on) => {
                    self.store.update(|s| s.general.keep_in_tray = on);
                    self.sync_tray();
                }
                // The system's autostart entry holds it, not the settings file (FR-014b).
                SettingChange::StartWithSystem(on) => {
                    let set = autostart::executable().and_then(|exe| autostart::set(on, &exe));
                    if let Err(err) = set {
                        log::warn!("start with the system: {err}");
                    }
                    self.ui.app.global::<Prefs>().set_start_with_system(autostart::state());
                }
            },
            UserEvent::ShowWindow | UserEvent::Tray(TrayEvent::Open) => self.show_window(),
            UserEvent::Tray(TrayEvent::Quit) => event_loop.exit(),
            UserEvent::Snap(event) => {
                let state = self.ui.app.global::<WindowState>();
                match event {
                    SnapEvent::Hover(on) => state.set_max_hover(on),
                    SnapEvent::Press(on) => state.set_max_pressed(on),
                    SnapEvent::Click => self.act(event_loop, WindowAction::ToggleMaximize),
                }
                self.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close(event_loop),
            WindowEvent::Resized(_) => {
                if let Some(w) = self.window.clone() {
                    self.ui.set_size(platform::drawn_size(&w));
                    self.ui.app.global::<WindowState>().set_maximized(w.is_maximized());
                }
                self.placement_changed = true;
                // Paced as other UI images: each size is a new image, texture and surface setup.
                if Instant::now() >= self.last_ui + self.refresh {
                    self.update_ui();
                } else {
                    self.request_redraw();
                }
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
                self.placement_changed = true;
            }
            // Paced to the refresh as in `about_to_wait`, which draws it later when not due yet.
            WindowEvent::RedrawRequested => {
                if Instant::now() >= self.last_ui + self.refresh {
                    self.update_ui();
                }
            }
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
                    let direction = frame::resize_direction(self.ui.size(), w.scale_factor(), position, resizable);
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
        if std::mem::take(&mut self.placement_changed)
            && let Some(w) = self.window.clone()
        {
            // Read again once the change's events are handled: the shadow's hidden row comes and
            // goes with a maximize after its `Resized` (research R5).
            self.ui.set_size(platform::drawn_size(&w));
            self.save_placement(&w);
        }
        // Keep Slint timers and animations running between video frames, so they do not step at
        // the video's frame rate. Read before updating: an update clears the flag until the next
        // draw reads the animated properties again.
        let mut animating = self.ui.has_active_animations();
        // At most one UI image per refresh (research R3), also for input: each image is uploaded
        // by the video thread, and pointer moves alone asked for hundreds a second.
        let due = Instant::now() >= self.last_ui + self.refresh;
        if due {
            self.update_ui();
            // A timer fired in this update may have started an animation (the chrome hiding):
            // its first frame was just drawn, so the flag is set again.
            animating |= self.ui.has_active_animations();
        }
        let mut wake = self.ui.next_timer();
        if animating || !due {
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
        // Releasing the name before the window is destroyed lets a start during the rest of the
        // quit become the new instance.
        self.tray = None;
        self.instance = None;
        self.snap = None;
        self.window = None;
        log::info!("quit");
    }
}
