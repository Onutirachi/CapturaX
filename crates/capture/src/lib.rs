//! Capture devices, their modes and the sessions that deliver their frames (spec
//! 20261106-170507-device-preview, contracts/capture.md). No OS calls: GStreamer chooses the
//! platform's provider, and anything else goes through `crates/platform` in the app.

use std::fmt;

pub mod analysis;
pub mod device;
pub mod frame;
pub mod mode;
pub mod monitor;
pub mod session;
pub mod test_source;

pub use analysis::RangeChoice;
pub use device::Device;
pub use frame::{Frame, Matrix, PixelFormat, Plane, Range};
pub use mode::{Mode, ModeChoice};
pub use monitor::{Monitor, MonitorEvent};
pub use session::{DeviceError, Session, SessionEvent};

/// GStreamer cannot start, or an element the capture needs is missing (exit code 69,
/// contracts/cli.md).
#[derive(Debug, Clone)]
pub struct Unavailable(pub String);

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unavailable {}
