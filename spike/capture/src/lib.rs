//! Capture code shared by the spike prototypes: devices, modes, sources and stats.

/// Initializes GStreamer and returns its version string.
pub fn init() -> anyhow::Result<String> {
    gst::init()?;
    Ok(gst::version_string().to_string())
}
