//! Windows implementation of the platform layer.

use std::path::PathBuf;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows_sys::Win32::Media::{TIMERR_NOERROR, timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
    PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowRect, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MessageBoxW,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::platform::windows::{MonitorHandleExtWindows, WindowAttributesExtWindows, WindowExtWindows};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes};

pub mod autostart;
pub mod camera;
mod dpi_rect;
pub mod instance;
pub mod notify;
mod registry;
pub mod snap;
pub mod theme;
pub mod tray;

pub fn app_folder() -> Option<PathBuf> {
    super::exe_folder()
}

/// `%APPDATA%\CapturaX`.
pub fn user_config_folder() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join("CapturaX"))
}

pub fn can_raise_window() -> bool {
    true
}

pub fn show_system_message(kind: crate::MessageKind, title: &str, text: &str) {
    let icon = match kind {
        crate::MessageKind::Error => MB_ICONERROR,
        crate::MessageKind::Warning => MB_ICONWARNING,
    };
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call; no owner window.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | icon | MB_SETFOREGROUND,
        );
    }
}

/// 1 ms timer resolution while it lives; only ended if it was granted.
/// Nothing to request: Windows keeps the GPU's clocks up under the preview's load (research R7).
pub struct PerformanceGuard;

impl PerformanceGuard {
    pub fn new(_pci_bus_id: Option<&str>) -> Self {
        Self
    }
}

pub struct TimerGuard(bool);

impl TimerGuard {
    pub fn new() -> Self {
        // Windows 11 ignores the request while the window is covered or minimized unless the
        // process opts out of that throttling (StateMask 0 = never ignore it).
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        // SAFETY: `state` is a valid PROCESS_POWER_THROTTLING_STATE that outlives the call, and its
        // size is passed; the pseudo handle of the current process needs no closing.
        let kept = unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                (&raw const state).cast(),
                size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        } != 0;
        if !kept {
            log::warn!("timer resolution: may be ignored while the window is covered");
        }
        // SAFETY: plain call; balanced by timeEndPeriod in Drop when granted.
        let granted = unsafe { timeBeginPeriod(1) } == TIMERR_NOERROR;
        if !granted {
            log::warn!("timer resolution: 1 ms not granted");
        }
        Self(granted)
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: matches the granted timeBeginPeriod(1) above.
            unsafe { timeEndPeriod(1) };
        }
    }
}

pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    // Keeps the system shadow (and the rounded corners on Windows 11) without the frame.
    attrs.with_undecorated_shadow(true)
}

pub use dpi_rect::follow_dpi_rect;

pub fn set_fullscreen_frame(window: &Window, fullscreen: bool) {
    // Without the shadow, winit no longer moves the client area 1 px down (WM_NCCALCSIZE).
    window.set_undecorated_shadow(!fullscreen);
}

/// The client area minus what lies past the window's rectangle: for the undecorated shadow winit
/// moves the client 1 px down without shrinking it (WM_NCCALCSIZE), so its last row is outside
/// the window. Measured from the rectangles rather than from winit's flags, which follow a
/// maximize late.
pub fn drawn_size(window: &Window) -> PhysicalSize<u32> {
    let size = window.inner_size();
    let Ok(handle) = window.window_handle() else {
        return size;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return size;
    };
    let hwnd = handle.hwnd.get() as HWND;
    let mut outer = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: the handle is valid for the window's life; the out pointers are to locals.
    let ok = unsafe {
        GetWindowRect(hwnd, &mut outer) != 0
            && GetClientRect(hwnd, &mut client) != 0
            && ClientToScreen(hwnd, &mut origin) != 0
    };
    if !ok {
        return size;
    }
    let hidden_right = (origin.x + client.right - outer.right).max(0) as u32;
    let hidden_below = (origin.y + client.bottom - outer.bottom).max(0) as u32;
    PhysicalSize::new(
        size.width.saturating_sub(hidden_right),
        size.height.saturating_sub(hidden_below),
    )
}

pub fn work_area(monitor: &MonitorHandle) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: the handle comes from winit's monitor list (a stale one makes the call fail), and
    // `info` is a MONITORINFO with its size set that outlives the call.
    if unsafe { GetMonitorInfoW(monitor.hmonitor() as HMONITOR, &raw mut info) } == 0 {
        return None;
    }
    let r = info.rcWork;
    let size = (
        u32::try_from(r.right - r.left).ok()?,
        u32::try_from(r.bottom - r.top).ok()?,
    );
    Some((PhysicalPosition::new(r.left, r.top), PhysicalSize::new(size.0, size.1)))
}
