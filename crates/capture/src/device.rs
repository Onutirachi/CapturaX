//! Devices, their stable identity, numbering and the start order (research R1, R2, data-model.md
//! "Device").

use gst::prelude::*;

use crate::mode::{Mode, Signal};

/// A capture device from the platform's preferred provider.
#[derive(Debug, Clone)]
pub struct Device {
    /// Stable key (research R2): `mf:<path>` or `v4l2:<vid>:<pid>:<serial>@<bus path>`.
    pub id: String,
    /// The name shown, numbered " (2)", " (3)" when repeated (FR-003).
    pub name: String,
    /// The name the system gives, before numbering.
    pub base_name: String,
    /// One per resolution × frame rate, largest first.
    pub modes: Vec<Mode>,
    /// The input the device reports; always none in this spec (research R3).
    pub signal: Option<Signal>,
    /// The GStreamer device, to create its source element. `None` only for devices built by
    /// tests.
    pub gst: Option<gst::Device>,
}

impl Device {
    /// A device without a GStreamer object (tests and tools).
    pub fn described(id: &str, name: &str, modes: Vec<Mode>) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_name: name.to_string(),
            modes,
            signal: None,
            gst: None,
        }
    }
}

/// Whether a device comes from the preferred provider (research R1): Media Foundation on
/// Windows, V4L2 on Linux (the PipeWire provider lists the same card again and is skipped).
pub fn is_preferred(device: &gst::Device) -> bool {
    let api = device.properties().and_then(|p| p.get::<String>("device.api").ok());
    preferred(api.as_deref(), device.type_().name())
}

/// [`is_preferred`] over the values it reads.
pub fn preferred(api: Option<&str>, type_name: &str) -> bool {
    api == Some("mediafoundation") || type_name == "GstV4l2Device"
}

/// The property values of a device that its identity is built from.
pub fn properties(device: &gst::Device) -> Vec<(String, String)> {
    let Some(props) = device.properties() else {
        return Vec::new();
    };
    props
        .iter()
        .filter_map(|(key, value)| {
            let text = value
                .get::<String>()
                .ok()
                .or_else(|| value.serialize().ok().map(|s| s.to_string()))?;
            Some((key.to_string(), text))
        })
        .collect()
}

/// The stable key of a device from its property values (research R2); `/dev/videoN` is never
/// used. `None` when the properties do not identify it.
pub fn identity(props: &[(String, String)]) -> Option<String> {
    let get = |key: &str| {
        props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim())
            .filter(|v| !v.is_empty())
    };
    match get("device.api")? {
        "mediafoundation" => Some(format!("mf:{}", get("device.path")?.to_lowercase())),
        "v4l2" => {
            let bus_path = get("device.bus_path")?;
            Some(format!(
                "v4l2:{}:{}:{}@{bus_path}",
                get("device.vendor.id").unwrap_or(""),
                get("device.product.id").unwrap_or(""),
                get("device.serial").unwrap_or(""),
            ))
        }
        _ => None,
    }
}

/// Numbers repeated names " (2)", " (3)" in list order (FR-003); the first keeps its name.
pub fn numbered(devices: &mut [Device]) {
    for i in 0..devices.len() {
        let earlier = devices[..i]
            .iter()
            .filter(|d| d.base_name == devices[i].base_name)
            .count();
        devices[i].name = if earlier == 0 {
            devices[i].base_name.clone()
        } else {
            format!("{} ({})", devices[i].base_name, earlier + 1)
        };
    }
}

/// The device with exactly this id.
pub fn find<'a>(id: &str, devices: &'a [Device]) -> Option<&'a Device> {
    devices.iter().find(|d| d.id == id)
}

/// The device to open at start (data-model.md "Start order"): the last used device if listed;
/// else the most recently used listed device; else the first capture card; else the first
/// webcam. Capture card or webcam is not known in general, so names containing "cam" go last.
pub fn start_order<'a>(devices: &'a [Device], last: Option<&str>, recent: &[String]) -> Option<&'a Device> {
    if let Some(device) = last.and_then(|id| find(id, devices)) {
        log::info!("start: last used device {}", device.id);
        return Some(device);
    }
    if let Some(device) = recent.iter().find_map(|id| find(id, devices)) {
        log::info!("start: recently used device {}", device.id);
        return Some(device);
    }
    let is_webcam = |d: &&Device| d.base_name.to_lowercase().contains("cam");
    if let Some(device) = devices.iter().find(|d| !is_webcam(d)) {
        log::info!("start: first capture card {} (no \"cam\" in its name)", device.id);
        return Some(device);
    }
    let device = devices.first()?;
    log::info!("start: first webcam {}", device.id);
    Some(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn windows_identity_is_the_lowercased_path() {
        let c920 = props(&[
            ("device.api", "mediafoundation"),
            (
                "device.path",
                r"\\?\usb#vid_046d&pid_082d&mi_00#d&41D17F6&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global",
            ),
            ("device.name", "HD Pro Webcam C920"),
        ]);
        assert_eq!(
            identity(&c920).as_deref(),
            Some(r"mf:\\?\usb#vid_046d&pid_082d&mi_00#d&41d17f6&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global")
        );
    }

    #[test]
    fn linux_identity_uses_ids_serial_and_bus_path() {
        let ezcap = props(&[
            ("device.api", "v4l2"),
            ("device.path", "/dev/video0"),
            ("device.bus_path", "pci-0000:03:00.0-usb-0:3:1.0"),
            ("device.vendor.id", "32ed"),
            ("device.product.id", "3201"),
            ("device.serial", "ezcap_ezcap_Live_GAMER_ULTRA_00000001"),
        ]);
        let id = identity(&ezcap).unwrap();
        assert_eq!(
            id,
            "v4l2:32ed:3201:ezcap_ezcap_Live_GAMER_ULTRA_00000001@pci-0000:03:00.0-usb-0:3:1.0"
        );
        assert!(!id.contains("/dev/video"));
        // Without a bus path there is no stable key.
        let no_bus = props(&[("device.api", "v4l2"), ("device.path", "/dev/video0")]);
        assert_eq!(identity(&no_bus), None);
        assert_eq!(identity(&props(&[("device.api", "pipewire")])), None);
    }

    #[test]
    fn preferred_provider() {
        assert!(preferred(Some("mediafoundation"), "GstMFDevice"));
        assert!(preferred(Some("v4l2"), "GstV4l2Device"));
        assert!(!preferred(Some("v4l2"), "GstPipeWireDevice"));
        assert!(!preferred(None, "GstKsDevice"));
    }

    #[test]
    fn repeated_names_are_numbered() {
        let mut devices = vec![
            Device::described("a", "USB Video", vec![]),
            Device::described("b", "C920", vec![]),
            Device::described("c", "USB Video", vec![]),
            Device::described("d", "USB Video", vec![]),
        ];
        numbered(&mut devices);
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["USB Video", "C920", "USB Video (2)", "USB Video (3)"]);
        // After the first one leaves, numbering follows the new order.
        devices.remove(0);
        numbered(&mut devices);
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["C920", "USB Video", "USB Video (2)"]);
    }

    #[test]
    fn find_is_exact() {
        let devices = vec![Device::described("v4l2:a@1", "Card", vec![])];
        assert!(find("v4l2:a@1", &devices).is_some());
        assert!(find("v4l2:a@2", &devices).is_none());
    }

    #[test]
    fn start_order_branches() {
        let devices = vec![
            Device::described("cam", "HD Pro Webcam C920", vec![]),
            Device::described("card", "ezcap Live GAMER ULTRA", vec![]),
            Device::described("other", "USB Video", vec![]),
        ];
        let pick = |last: Option<&str>, recent: &[&str]| {
            let recent: Vec<String> = recent.iter().map(|s| s.to_string()).collect();
            start_order(&devices, last, &recent).map(|d| d.id.clone())
        };
        assert_eq!(pick(Some("other"), &["cam"]).as_deref(), Some("other"));
        assert_eq!(pick(Some("gone"), &["gone2", "cam", "card"]).as_deref(), Some("cam"));
        assert_eq!(pick(None, &[]).as_deref(), Some("card"));
        let webcams = vec![
            Device::described("w1", "Webcam A", vec![]),
            Device::described("w2", "FaceCam", vec![]),
        ];
        assert_eq!(start_order(&webcams, None, &[]).map(|d| d.id.as_str()), Some("w1"));
        assert!(start_order(&[], Some("x"), &[]).is_none());
    }
}
