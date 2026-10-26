//! Video capture devices and their modes, from GStreamer's device monitor.

use gst::prelude::*;
use crate::mode::{self, Mode};

pub struct VideoDevice {
    pub name: String,
    pub device: gst::Device,
    pub modes: Vec<Mode>,
}

/// Whether a device comes from the platform's preferred API: Media Foundation on Windows, V4L2
/// (`v4l2src`) on Linux. On Linux the PipeWire provider hides the V4L2 one and lists the card as a
/// PipeWire camera whose `device.api` is also "v4l2", so the device's type is checked there.
fn preferred(device: &gst::Device) -> bool {
    if cfg!(windows) {
        let api = device.properties().and_then(|p| p.get::<String>("device.api").ok()).unwrap_or_default();
        api == "mediafoundation"
    } else {
        device.type_().name() == "GstV4l2Device"
    }
}

/// Lists video capture devices through the platform's preferred API, with their modes.
pub fn video_devices() -> anyhow::Result<Vec<VideoDevice>> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Video/Source"), None);
    // Providers hidden by another one (V4L2 behind PipeWire) are listed too.
    monitor.set_show_all_devices(true);
    monitor.start()?;
    let devices = monitor.devices();
    monitor.stop();

    let mut found = Vec::new();
    for device in devices {
        if !preferred(&device) {
            continue;
        }
        let modes = device.caps().map(|c| mode::from_caps(&c)).unwrap_or_default();
        found.push(VideoDevice { name: device.display_name().to_string(), device, modes });
    }
    Ok(found)
}

/// Finds a device by index (`0`) or by part of its name (case-insensitive).
pub fn pick(devices: Vec<VideoDevice>, wanted: Option<&str>) -> anyhow::Result<VideoDevice> {
    if devices.is_empty() {
        anyhow::bail!("no video capture device found: connect a capture card or a webcam");
    }
    let Some(wanted) = wanted else {
        return Ok(devices.into_iter().next().unwrap());
    };
    if let Ok(index) = wanted.parse::<usize>() {
        return devices.into_iter().nth(index).ok_or_else(|| anyhow::anyhow!("no device at index {index}"));
    }
    let lower = wanted.to_lowercase();
    devices
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&lower))
        .ok_or_else(|| anyhow::anyhow!("no device named like {wanted:?} (use --list)"))
}

/// Prints every device with its modes, marking the Auto choice.
pub fn print_list(devices: &[VideoDevice]) {
    if devices.is_empty() {
        println!("No video capture device found. Connect a capture card or a webcam.");
        return;
    }
    for (index, device) in devices.iter().enumerate() {
        let auto = mode::auto(&device.modes);
        println!("[{index}] {}", device.name);
        for m in &device.modes {
            let mark = if Some(m) == auto.as_ref() { "  <- Auto" } else { "" };
            println!("      {m}{mark}");
        }
    }
}
