//! A device from the block `gst-device-monitor-1.0` prints for it (name, caps, properties), kept
//! as printed (`crates/capture/tests/data`). For tests and tools: the modes of a real device
//! without the device.

use crate::device::{self, Device};
use crate::mode;

/// The block's device: its identity, name and modes, read as the monitor reads a live device.
/// `None` when the block has no name, no caps that parse or no identity. Initializes GStreamer
/// (caps parsing needs it).
pub fn device(text: &str) -> Option<Device> {
    gst::init().ok()?;
    let mut name = None;
    let mut structures = Vec::new();
    let mut props = Vec::new();
    let mut section = Section::Head;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name  :") {
            name = Some(value.trim().to_string());
            section = Section::Head;
        } else if let Some(first) = line.strip_prefix("caps  :") {
            structures.push(first.trim().to_string());
            section = Section::Caps;
        } else if line.starts_with("properties:") {
            section = Section::Properties;
        } else if line.is_empty() || line.starts_with("gst-launch") {
            section = Section::Head;
        } else if section == Section::Caps {
            structures.push(line.to_string());
        } else if section == Section::Properties
            && let Some((key, value)) = line.split_once(" = ")
        {
            props.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    let caps: gst::Caps = structures.join("; ").parse().ok()?;
    let api = props.iter().find(|(k, _)| k == "device.api").map(|(_, v)| v.as_str());
    let frame_server = api == Some("mediafoundation") && mode::nv12_declared_full(&caps);
    let id = device::identity(&props)?;
    Some(Device::described(
        &id,
        &name?,
        mode::modes_from_caps(&caps, frame_server),
    ))
}

#[derive(PartialEq, Eq)]
enum Section {
    Head,
    Caps,
    Properties,
}
