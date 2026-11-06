//! Capture devices, their modes and the sessions that deliver their frames (spec
//! 20261106-170507-device-preview, contracts/capture.md). No OS calls: GStreamer chooses the
//! platform's provider, and anything else goes through `crates/platform` in the app.

pub mod analysis;
pub mod device;
pub mod frame;
pub mod mode;
pub mod monitor;
pub mod session;
pub mod test_source;
