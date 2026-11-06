//! The CapturaX platform layer: the only crate that calls OS-specific APIs (constitution IV,
//! contracts/platform.md). Each function has a Windows and a Linux implementation selected at
//! compile time; the app sees one interface.

use std::path::PathBuf;

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowAttributes};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;

pub use os::autostart;

pub mod snap;
pub mod theme;
pub mod tray;

mod single_instance;
pub use single_instance::{Answer, Claim, Client, Listener, Serving, claim_instance, instance_name};

/// The folder the app's settings and log go to first (FR-026): the executable's folder, or on
/// Linux the folder of the AppImage when running from one.
pub fn app_folder() -> PathBuf {
    os::app_folder().unwrap_or_else(|| PathBuf::from("."))
}

/// Whether a window of this app may bring itself to the front when another start asks it to
/// (FR-013): true on Windows and X11, false on Wayland, where only the user's action raises a
/// window.
pub fn can_raise_window() -> bool {
    os::can_raise_window()
}

/// Shows a blocking error message outside the window (FR-036), for problems the window cannot
/// show. Falls back to stderr where no message box exists.
pub fn show_system_message(title: &str, text: &str) {
    log::error!("{title}: {text}");
    os::show_system_message(title, text);
}

/// The window's platform attributes: the system shadow and rounded corners of a frameless window
/// on Windows, the app id on Linux (contracts/platform.md).
pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    os::window_attributes(attrs)
}

/// Keeps a window that is being dragged or resized onto another monitor's scale on the
/// rectangle the system suggests (Windows; research R5). Call once after creating the window.
pub fn follow_dpi_rect(window: &Window) {
    os::follow_dpi_rect(window);
}

/// Prepares the frameless window's frame for fullscreen or for leaving it: on Windows the shadow's
/// 1 px top strip is removed while fullscreen (research R5). Call before entering fullscreen and
/// after leaving it. Nothing to do on Linux.
pub fn set_fullscreen_frame(window: &Window, fullscreen: bool) {
    os::set_fullscreen_frame(window, fullscreen);
}

/// The size to draw the window's content at, physical pixels: on Windows the inner size without
/// the row the undecorated shadow pushes below the window, while neither maximized nor
/// fullscreen; drawn at the full inner size, the system would squeeze it into the visible rows
/// and blur everything (research R5). The inner size elsewhere.
pub fn drawn_size(window: &Window) -> PhysicalSize<u32> {
    os::drawn_size(window)
}

/// The part of `monitor` not taken by the taskbar or panels, physical pixels in desktop
/// coordinates (FR-012). None where the OS does not say (Linux): use the monitor's bounds.
pub fn work_area(monitor: &MonitorHandle) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    os::work_area(monitor)
}

/// Keeps the system's timers precise while it lives: Windows wakes sleeping threads every 15.6 ms
/// by default, which makes GStreamer's live sources deliver frames in bursts (research R9).
/// Nothing to do on Linux.
#[must_use = "timers stay precise only while the guard lives"]
pub struct TimerGuard {
    _os: os::TimerGuard,
}

/// Asks for 1 ms timer resolution until the guard drops.
pub fn precise_timers() -> TimerGuard {
    TimerGuard {
        _os: os::TimerGuard::new(),
    }
}

/// The running executable's folder.
fn exe_folder() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}
