//! Windows implementation of the platform layer.

use std::path::PathBuf;

use windows_sys::Win32::Media::{TIMERR_NOERROR, timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
    PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW};
use winit::platform::windows::{WindowAttributesExtWindows, WindowExtWindows};
use winit::window::{Window, WindowAttributes};

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

/// 1 ms timer resolution while it lives; only ended if it was granted.
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

pub fn set_fullscreen_frame(window: &Window, fullscreen: bool) {
    // Without the shadow, winit no longer moves the client area 1 px down (WM_NCCALCSIZE).
    window.set_undecorated_shadow(!fullscreen);
}
