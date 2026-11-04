//! Windows implementation of the platform layer.

use std::path::PathBuf;

use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::WindowAttributes;

pub mod autostart;
pub mod instance;
pub mod tray;

pub fn app_folder() -> Option<PathBuf> {
    super::exe_folder()
}

pub fn show_system_message(title: &str, text: &str) {
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call; no owner window.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    // Keeps the system shadow (and the rounded corners on Windows 11) without the frame.
    attrs.with_undecorated_shadow(true)
}
