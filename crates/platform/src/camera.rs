//! The system's camera privacy switch (FR-019, FR-021, research R6): whether desktop apps may use
//! the camera, and the settings page that changes it. Windows only; Linux has no such switch
//! (a device node's permissions show up as an error naming the node instead).

use crate::os;

/// Whether the system lets this app use cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Allowed, or at least not denied anywhere the app can read.
    Allowed,
    /// Denied by a privacy switch: Windows hides every camera from the app then.
    Denied,
    /// No switch to read (Linux, or the settings are missing).
    Unknown,
}

/// Reads the camera privacy switches (Windows: the camera consent for all apps and for desktop
/// apps, per user and per machine; any denial wins). `Unknown` on Linux.
pub fn camera_access() -> Access {
    os::camera::access()
}

/// Opens the system's camera privacy page (Windows: Settings → Privacy & security → Camera).
/// Does nothing on Linux, where the preview state names the device node instead.
pub fn open_camera_privacy() {
    os::camera::open_privacy();
}
