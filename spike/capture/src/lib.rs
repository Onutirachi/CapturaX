//! Capture code shared by the spike prototypes: devices, modes, sources and stats.

pub mod audio;
pub mod cli;
pub mod devices;
pub mod encoder;
pub mod mode;
pub mod stats;
pub mod video;

/// Initializes GStreamer and returns its version string.
pub fn init() -> anyhow::Result<String> {
    gst::init()?;
    Ok(gst::version_string().to_string())
}
