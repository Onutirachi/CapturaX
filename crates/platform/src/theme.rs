//! The system's dark or light preference (FR-017, research R8). On Windows winit reports it:
//! `Window::theme()` and `WindowEvent::ThemeChanged`. winit 0.30 reports neither on Wayland or
//! X11, so on Linux it comes from the XDG Settings portal (`org.freedesktop.appearance`
//! `color-scheme`) and its `SettingChanged` signal.

use winit::window::Window;

use crate::os;

pub use winit::window::Theme;

/// The system's theme now, if it says. On Linux this asks the portal (short timeout).
pub fn current(window: &Window) -> Option<Theme> {
    os::theme::current(window)
}

/// Calls `on_change` from a thread with the new theme each time the system's changes, where
/// winit does not report it (Linux); None means the system no longer says. Nothing on Windows:
/// `WindowEvent::ThemeChanged` arrives instead. The watch lasts until the process exits.
pub fn watch(on_change: impl Fn(Option<Theme>) + Send + 'static) {
    os::theme::watch(on_change);
}
