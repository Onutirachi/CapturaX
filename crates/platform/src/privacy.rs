//! The system's privacy switches for the camera (device preview FR-019, FR-021, research R6) and
//! the microphone (preview audio research R7): whether desktop apps may use the device, and the
//! settings page that changes it. Windows only; Linux has no such switch (a device node's
//! permissions show up as an error naming the node instead).

use crate::os;

/// A device the system's privacy settings guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Camera,
    Microphone,
}

/// Whether the system lets this app use a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Allowed, or at least not denied anywhere the app can read.
    Allowed,
    /// Denied by a privacy switch: Windows hides the devices from the app (cameras) or refuses
    /// to open them (microphones).
    Denied,
    /// No switch to read (Linux, or the settings are missing).
    Unknown,
}

/// Reads the capability's privacy switches (Windows: its consent store for all apps and for
/// desktop apps, per user and per machine; any denial wins). `Unknown` on Linux.
pub fn access(capability: Capability) -> Access {
    os::privacy::access(capability)
}

/// Opens the system's privacy page for the capability (Windows: Settings → Privacy & security →
/// Camera or Microphone). Does nothing on Linux, where the state names the device instead.
pub fn open_privacy(capability: Capability) {
    os::privacy::open_privacy(capability);
}
