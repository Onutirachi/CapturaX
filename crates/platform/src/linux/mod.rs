//! Linux implementation of the platform layer.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{Window, WindowAttributes};

pub mod autostart;
pub mod instance;
pub mod snap;
pub mod theme;
pub mod tray;

/// The app id: matches the `.desktop` file the packaged app installs (research R12).
const APP_ID: &str = "capturax";

pub fn app_folder() -> Option<PathBuf> {
    // An AppImage runs from a read-only mount; its settings belong next to the AppImage file.
    match std::env::var_os("APPIMAGE") {
        Some(image) if !image.is_empty() => Path::new(&image).parent().map(PathBuf::from),
        _ => super::exe_folder(),
    }
}

/// winit picks Wayland whenever a Wayland display is set, and X11 otherwise.
pub fn can_raise_window() -> bool {
    let set = |var| std::env::var_os(var).is_some_and(|v| !v.is_empty());
    !(set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET"))
}

pub fn show_system_message(title: &str, text: &str) {
    let shown = run("kdialog", &["--title", title, "--error", text])
        || run("zenity", &["--error", "--title", title, "--text", text]);
    if !shown {
        eprintln!("{title}: {text}");
    }
}

/// Runs a dialog program and waits for it; false when it is missing or failed to show.
fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|status| status.code().is_some_and(|c| c == 0 || c == 1))
}

/// Linux timers are already precise.
pub struct TimerGuard;

impl TimerGuard {
    pub fn new() -> Self {
        Self
    }
}

pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    let attrs = WindowAttributesExtWayland::with_name(attrs, APP_ID, APP_ID);
    WindowAttributesExtX11::with_name(attrs, APP_ID, APP_ID)
}

pub fn set_fullscreen_frame(_window: &Window, _fullscreen: bool) {}

pub fn follow_dpi_rect(_window: &Window) {}

pub fn drawn_size(window: &Window) -> PhysicalSize<u32> {
    window.inner_size()
}

/// Wayland does not tell clients about panels; X11's `_NET_WORKAREA` covers all monitors at once.
pub fn work_area(_monitor: &MonitorHandle) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    None
}
