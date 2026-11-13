//! Audio devices and streams on Windows (WASAPI; preview audio research R2, R5, R6, R7).
//!
//! - [`endpoints`]: the monitor (`IMMDeviceEnumerator` and an `IMMNotificationClient`).
//! - [`container`]: the container id that links a video device to its audio (pairing).
//! - [`format`]: the system's wave formats in the platform's terms, and sample conversions.
//! - [`stream`]: the session, one thread per side.

mod container;
mod endpoints;
mod format;
mod stream;

pub use container::video_link;
pub use endpoints::Monitor;
pub use stream::Session;

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// The multithreaded COM apartment for the life of the thread.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: once, at the start of the thread that owns the guard.
        Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx; every interface is released first
            // (declared after this guard).
            unsafe { CoUninitialize() };
        }
    }
}
