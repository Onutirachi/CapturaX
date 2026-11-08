//! The CapturaX platform layer: the only crate that calls OS-specific APIs (constitution IV,
//! contracts/platform.md). Each function has a Windows and a Linux implementation selected at
//! compile time; the app sees one interface.

use std::path::{Path, PathBuf};

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

/// Built for Windows: the latency estimate counts the USB transfer there (device-preview
/// data-model.md "Latency estimate"), so the app needs no platform condition of its own.
pub const IS_WINDOWS: bool = cfg!(windows);

mod camera;
pub use camera::{Access, camera_access, open_camera_privacy};

mod gpu;
pub use gpu::{PerformanceGuard, gpu_performance};

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

/// The per-user folder the settings go to when the application folder is not writable (FR-027):
/// `%APPDATA%\CapturaX` on Windows, `$XDG_CONFIG_HOME/capturax` or `~/.config/capturax` on
/// Linux. None when the system gives no such folder.
pub fn user_config_folder() -> Option<PathBuf> {
    os::user_config_folder()
}

/// Whether the app can keep files in `folder` (FR-026): creates the folder if it is missing,
/// then creates and removes a probe file in it.
pub fn is_writable(folder: &Path) -> bool {
    const PROBE: &str = ".capturax-write-probe";
    if let Err(err) = std::fs::create_dir_all(folder) {
        log::debug!("{} not writable: {err}", folder.display());
        return false;
    }
    let probe = folder.join(PROBE);
    // Not `create_new`: a probe left by a killed start must not make the folder look read-only.
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe);
    match created.map(drop).and_then(|()| std::fs::remove_file(&probe)) {
        Ok(()) => true,
        Err(err) => {
            log::debug!("{} not writable: {err}", folder.display());
            false
        }
    }
}

/// Whether a window of this app may bring itself to the front when another start asks it to
/// (FR-013): true on Windows and X11, false on Wayland, where only the user's action raises a
/// window.
pub fn can_raise_window() -> bool {
    os::can_raise_window()
}

/// What a system message is about; it sets the icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// The app cannot go on (FR-036).
    Error,
    /// Something the user should know, nothing failed (FR-013).
    Warning,
}

/// Shows a blocking message outside the window, for what the window cannot show (FR-013,
/// FR-036). Falls back to stderr where no message box exists.
pub fn show_system_message(kind: MessageKind, title: &str, text: &str) {
    match kind {
        MessageKind::Error => log::error!("{title}: {text}"),
        MessageKind::Warning => log::warn!("{title}: {text}"),
    }
    os::show_system_message(kind, title, text);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("capturax-platform-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn writable_folder_is_created_and_left_clean() {
        let folder = scratch("writable").join("nested");
        assert!(is_writable(&folder));
        assert!(folder.is_dir());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
        // A probe left behind by a killed start does not count against the folder.
        std::fs::write(folder.join(".capturax-write-probe"), b"x").unwrap();
        assert!(is_writable(&folder));
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    #[test]
    fn folder_under_a_file_is_not_writable() {
        let base = scratch("file");
        std::fs::create_dir_all(&base).unwrap();
        let file = base.join("plain");
        std::fs::write(&file, b"x").unwrap();
        assert!(!is_writable(&file.join("inside")));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
