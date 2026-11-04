//! The CapturaX platform layer: the only crate that calls OS-specific APIs (constitution IV,
//! contracts/platform.md). Each function has a Windows and a Linux implementation selected at
//! compile time; the app sees one interface.

use std::path::PathBuf;

use winit::window::WindowAttributes;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;

pub use os::{autostart, instance, tray};

/// The folder the app's settings and log go to first (FR-026): the executable's folder, or on
/// Linux the folder of the AppImage when running from one.
pub fn app_folder() -> PathBuf {
    os::app_folder().unwrap_or_else(|| PathBuf::from("."))
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

/// The running executable's folder.
fn exe_folder() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}
