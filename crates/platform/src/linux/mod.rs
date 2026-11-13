//! Linux implementation of the platform layer.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{Window, WindowAttributes};

mod activation;
pub use activation::activate;
pub mod autostart;
pub mod camera;
pub mod instance;
pub mod notify;
mod nvml;
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

/// `$XDG_CONFIG_HOME/capturax` when it is an absolute path, else `~/.config/capturax`.
pub fn user_config_folder() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join(APP_ID))
}

/// winit picks Wayland whenever a Wayland display is set, and X11 otherwise.
pub fn can_raise_window() -> bool {
    let set = |var| std::env::var_os(var).is_some_and(|v| !v.is_empty());
    !(set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET"))
}

pub fn show_system_message(kind: crate::MessageKind, title: &str, text: &str) {
    // kdialog's `--sorry` is its warning box.
    let (kdialog, zenity) = match kind {
        crate::MessageKind::Error => ("--error", "--error"),
        crate::MessageKind::Warning => ("--sorry", "--warning"),
    };
    let shown = run("kdialog", &["--title", title, kdialog, text])
        || run("zenity", &[zenity, "--title", title, "--text", text]);
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
pub use nvml::PerformanceGuard;

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

/// `CLOCK_MONOTONIC` in nanoseconds. Declared here rather than through the `libc` crate: one call,
/// to the C library std already links.
pub fn qpc() -> i64 {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    const CLOCK_MONOTONIC: i32 = 1;
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut time = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `time` matches `struct timespec` on 64-bit Linux (64-bit `time_t` and `long`) and
    // outlives the call; CLOCK_MONOTONIC always exists, so the call does not fail.
    unsafe { clock_gettime(CLOCK_MONOTONIC, &mut time) };
    time.tv_sec * 1_000_000_000 + time.tv_nsec
}

/// KWin restores a tile itself after fullscreen (checked in the app shell).
pub fn arranged_rect(_window: &Window) -> Option<crate::Rect> {
    None
}

pub fn set_outer_rect(_window: &Window, _rect: crate::Rect) {}

/// Wayland does not tell clients about panels; X11's `_NET_WORKAREA` covers all monitors at once.
pub fn work_area(_monitor: &MonitorHandle) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    None
}
