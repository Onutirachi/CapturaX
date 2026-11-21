//! The container id: one GUID for every function of one physical device (research R5). An audio
//! endpoint and a video device with the same container id are the same box.

use windows::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, CM_Get_Device_Interface_PropertyW, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
    CR_SUCCESS,
};
use windows_sys::Win32::Devices::Properties::{
    DEVPKEY_Device_ContainerId, DEVPKEY_Device_InstanceId, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING, DEVPROPTYPE,
};
use windows_sys::Win32::Foundation::DEVPROPKEY;

/// Windows' container id for "no physical container", which every virtual or software endpoint
/// reports (measurements/pairing.md); it links nothing.
const NULL_CONTAINER: &str = "00000000-0000-0000-ffff-ffffffffffff";

/// A container id as [`crate::audio::Endpoint::link`] holds it: lowercase, without braces. None
/// for an empty text or the null container id.
pub(super) fn link(container_id: &str) -> Option<String> {
    let text = container_id
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_lowercase();
    (!text.is_empty() && text != NULL_CONTAINER).then_some(text)
}

/// The container id of a Media Foundation video device (`mf:<symbolic link>`): the interface's
/// device instance, located in the device tree, then its container id. None for other ids, a
/// device that is gone, or one without a container.
pub fn video_link(video_id: &str) -> Option<String> {
    let symbolic_link = wide(video_id.strip_prefix("mf:")?);
    let instance = match interface_property(&symbolic_link, &DEVPKEY_Device_InstanceId)? {
        Property::Text(text) => wide(&text),
        Property::Guid(_) => return None,
    };
    let mut node = 0u32;
    // SAFETY: valid out pointer and NUL-terminated id.
    let code = unsafe { CM_Locate_DevNodeW(&mut node, instance.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) };
    if code != CR_SUCCESS {
        return None;
    }
    match node_property(node, &DEVPKEY_Device_ContainerId)? {
        Property::Guid(guid) => link(&format!("{guid:?}")),
        Property::Text(_) => None,
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

enum Property {
    Text(String),
    Guid(GUID),
}

/// Reads a property with one of the two CfgMgr32 getters: the size first, then the value. Only
/// text and GUID values are kept.
fn read(get: impl Fn(*mut DEVPROPTYPE, *mut u8, *mut u32) -> u32) -> Option<Property> {
    let (mut kind, mut size) = (0, 0u32);
    get(&mut kind, std::ptr::null_mut(), &mut size);
    if size == 0 {
        return None;
    }
    let mut bytes = vec![0u8; size as usize];
    if get(&mut kind, bytes.as_mut_ptr(), &mut size) != CR_SUCCESS {
        return None;
    }
    if kind == DEVPROP_TYPE_GUID && bytes.len() == 16 {
        // A GUID's in-memory layout: three little-endian fields, then eight bytes.
        let field = |range: std::ops::Range<usize>| bytes[range].iter().rev().fold(0u32, |n, &b| n << 8 | u32::from(b));
        let tail: [u8; 8] = bytes[8..].try_into().ok()?;
        return Some(Property::Guid(GUID::from_values(
            field(0..4),
            field(4..6) as u16,
            field(6..8) as u16,
            tail,
        )));
    }
    if kind != DEVPROP_TYPE_STRING {
        return None;
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    Some(Property::Text(String::from_utf16_lossy(&units[..end])))
}

fn interface_property(symbolic_link: &[u16], key: &DEVPROPKEY) -> Option<Property> {
    // SAFETY: NUL-terminated link and buffers that outlive each call.
    read(|kind, buffer, size| unsafe {
        CM_Get_Device_Interface_PropertyW(symbolic_link.as_ptr(), key, kind, buffer, size, 0)
    })
}

fn node_property(node: u32, key: &DEVPROPKEY) -> Option<Property> {
    // SAFETY: buffers that outlive each call.
    read(|kind, buffer, size| unsafe { CM_Get_DevNode_PropertyW(node, key, kind, buffer, size, 0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_lowercase_without_braces() {
        assert_eq!(
            link("{B6F921B6-5905-5B63-B1A1-34D4A41A75C2}").as_deref(),
            Some("b6f921b6-5905-5b63-b1a1-34d4a41a75c2")
        );
        assert_eq!(
            link(&format!(
                "{:?}",
                GUID::from_u128(0xef06585b_1df1_51ae_b61b_8b660c165769)
            ))
            .as_deref(),
            Some("ef06585b-1df1-51ae-b61b-8b660c165769")
        );
    }

    #[test]
    fn the_null_container_links_nothing() {
        assert_eq!(link("{00000000-0000-0000-FFFF-FFFFFFFFFFFF}"), None);
        assert_eq!(link(""), None);
    }

    #[test]
    fn only_media_foundation_ids_have_a_video_link() {
        assert_eq!(video_link("v4l2:32ed:3201:x@pci"), None);
        assert_eq!(video_link("test"), None);
    }
}
