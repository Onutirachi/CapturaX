//! Pairing keys probe for the preview audio (spec 20261113-103922-preview-audio, research R5,
//! quickstart "Pairing"): lists every audio input and output with its name, id and the raw
//! sources of its link to a physical device, and the same for the video devices, so the
//! maintainer can check the pairing rule on each system before it is written.
//!
//! `cargo run -p capturax-platform --example audio_endpoints [-- <video device path>...]`
//!
//! - **Windows**: audio endpoints from `IMMDeviceEnumerator` with `PKEY_Device_ContainerId` from
//!   each endpoint's property store; video devices are the present `KSCATEGORY_VIDEO_CAMERA`
//!   interfaces (the symbolic links Media Foundation lists), plus any path given as an argument
//!   (as `capturax --list-devices` prints it, with or without `mf:`), each with its container id
//!   through `DEVPKEY_Device_InstanceId` → `CM_Locate_DevNodeW` → `DEVPKEY_Device_ContainerId`.
//!   Rule: the ezcap's video and its audio input show the same container id; the C920's too.
//! - **Linux**: the PipeWire `Audio/Source`, `Audio/Sink` and `Video/Source` nodes with their
//!   parent device's `device.bus-path` and the key the rule compares (the path without a trailing
//!   `:<n>.<n>`). The app's video comes from GStreamer, not PipeWire; compare with the bus path
//!   at the end of the id the app prints (`v4l2:…@<bus path>`):
//!
//!   ```sh
//!   capturax --list-devices | grep 'id:'
//!   ```
//!
//!   (`gst-device-monitor-1.0` lists PipeWire's entry for the device, which has no
//!   `device.bus_path`.) Rule: for the ezcap and the C920, the V4L2 bus path and the audio
//!   node's `device.bus-path` are equal after removing a trailing `:<n>.<n>` from both.

#[cfg(windows)]
fn main() {
    windows_keys::main();
}

#[cfg(target_os = "linux")]
fn main() {
    linux_keys::main();
}

#[cfg(not(any(windows, target_os = "linux")))]
fn main() {
    eprintln!("audio_endpoints runs on Windows and Linux only");
    std::process::exit(1);
}

#[cfg(windows)]
mod windows_keys {
    use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_ContainerId, PKEY_Device_FriendlyName};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::{
        DEVICE_STATE_ACTIVE, EDataFlow, IMMDevice, IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator, eAll,
        eCapture, eConsole, eRender,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
    };
    use windows::core::{GUID, Interface, PWSTR};
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_DevNode_PropertyW, CM_Get_Device_Interface_List_SizeW,
        CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_PropertyW, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
        CR_SUCCESS,
    };
    use windows_sys::Win32::Devices::Properties::{
        DEVPKEY_Device_ContainerId, DEVPKEY_Device_FriendlyName, DEVPKEY_Device_InstanceId,
        DEVPKEY_DeviceInterface_FriendlyName, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING, DEVPROPTYPE,
    };
    use windows_sys::Win32::Foundation::DEVPROPKEY;

    /// `KSCATEGORY_VIDEO_CAMERA`: the interface class Media Foundation enumerates video capture
    /// devices from.
    const KSCATEGORY_VIDEO_CAMERA: windows_sys::core::GUID =
        windows_sys::core::GUID::from_u128(0xe5323777_f976_4f5b_9b55_b94699c46e44);

    pub fn main() {
        // SAFETY: once on this thread, before any COM call.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if let Err(error) = audio() {
            eprintln!("audio endpoints: {error}");
        }
        println!();
        video(std::env::args().skip(1).collect());
    }

    fn audio() -> windows::core::Result<()> {
        // SAFETY: COM is initialized on this thread; every interface is used on it only.
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let default = |flow: EDataFlow| {
                enumerator
                    .GetDefaultAudioEndpoint(flow, eConsole)
                    .ok()
                    .and_then(|device| device_id(&device))
            };
            let (default_input, default_output) = (default(eCapture), default(eRender));
            let collection = enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)?;
            println!("Audio endpoints (active):");
            for i in 0..collection.GetCount()? {
                let device = collection.Item(i)?;
                let id = device_id(&device).unwrap_or_default();
                let flow = device.cast::<IMMEndpoint>().and_then(|e| e.GetDataFlow());
                let (kind, default) = match flow {
                    Ok(f) if f == eCapture => ("input", &default_input),
                    Ok(f) if f == eRender => ("output", &default_output),
                    _ => ("?", &None),
                };
                let mark = if default.as_deref() == Some(id.as_str()) {
                    " (default)"
                } else {
                    ""
                };
                println!(
                    "  {kind}{mark}: {}",
                    property(&device, &PKEY_Device_FriendlyName).unwrap_or_default()
                );
                println!("    id:           {id}");
                println!(
                    "    container id: {}",
                    property(&device, &PKEY_Device_ContainerId).map_or("(none)".to_owned(), |g| g.to_lowercase())
                );
            }
        }
        Ok(())
    }

    fn device_id(device: &IMMDevice) -> Option<String> {
        // SAFETY: GetId returns a CoTaskMemAlloc'ed string, freed here after copying it.
        unsafe {
            let id = device.GetId().ok()?;
            let text = id.to_string().ok();
            CoTaskMemFree(Some(id.0 as _));
            text
        }
    }

    /// An endpoint property as text (a GUID as `{…}`).
    fn property(device: &IMMDevice, key: &PROPERTYKEY) -> Option<String> {
        // SAFETY: the property store and value live through the calls; the text is freed after
        // copying it.
        unsafe {
            let store = device.OpenPropertyStore(STGM_READ).ok()?;
            let value = store.GetValue(key).ok()?;
            let text: PWSTR = PropVariantToStringAlloc(&value).ok()?;
            let result = text.to_string().ok();
            CoTaskMemFree(Some(text.0 as _));
            result
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(units: &[u16]) -> String {
        String::from_utf16_lossy(&units[..units.iter().position(|&u| u == 0).unwrap_or(units.len())])
    }

    /// The present interfaces of `class`, as symbolic links.
    fn interfaces(class: &windows_sys::core::GUID) -> Vec<String> {
        let mut len = 0u32;
        // SAFETY: valid out pointer and GUID; no device filter.
        let code = unsafe {
            CM_Get_Device_Interface_List_SizeW(&mut len, class, std::ptr::null(), CM_GET_DEVICE_INTERFACE_LIST_PRESENT)
        };
        if code != CR_SUCCESS || len == 0 {
            return Vec::new();
        }
        let mut buffer = vec![0u16; len as usize];
        // SAFETY: `buffer` holds the `len` characters the size call asked for.
        let code = unsafe {
            CM_Get_Device_Interface_ListW(
                class,
                std::ptr::null(),
                buffer.as_mut_ptr(),
                len,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if code != CR_SUCCESS {
            return Vec::new();
        }
        // A list of NUL-terminated strings ending with an empty one.
        buffer
            .split(|&u| u == 0)
            .take_while(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    /// Reads a property into a byte buffer with one of the two CfgMgr32 getters.
    fn read(get: impl Fn(*mut DEVPROPTYPE, *mut u8, *mut u32) -> u32) -> Option<(DEVPROPTYPE, Vec<u8>)> {
        let (mut kind, mut size) = (0, 0u32);
        get(&mut kind, std::ptr::null_mut(), &mut size);
        if size == 0 {
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        (get(&mut kind, buffer.as_mut_ptr(), &mut size) == CR_SUCCESS).then_some((kind, buffer))
    }

    fn as_text((kind, bytes): (DEVPROPTYPE, Vec<u8>)) -> Option<String> {
        if kind == DEVPROP_TYPE_GUID && bytes.len() == 16 {
            let guid = GUID::from_u128(u128::from_be_bytes(guid_bytes(&bytes)));
            return Some(format!("{{{guid:?}}}").to_lowercase());
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
        Some(from_wide(&units))
    }

    /// A GUID's in-memory bytes (little-endian fields) in reading order.
    fn guid_bytes(b: &[u8]) -> [u8; 16] {
        [
            b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15],
        ]
    }

    fn interface_property(link: &[u16], key: &DEVPROPKEY) -> Option<String> {
        // SAFETY: NUL-terminated link and buffers that outlive each call.
        as_text(read(|kind, buffer, size| unsafe {
            CM_Get_Device_Interface_PropertyW(link.as_ptr(), key, kind, buffer, size, 0)
        })?)
    }

    fn node_property(node: u32, key: &DEVPROPKEY) -> Option<String> {
        // SAFETY: buffers that outlive each call.
        as_text(read(|kind, buffer, size| unsafe {
            CM_Get_DevNode_PropertyW(node, key, kind, buffer, size, 0)
        })?)
    }

    fn video(arguments: Vec<String>) {
        let mut links = interfaces(&KSCATEGORY_VIDEO_CAMERA);
        for argument in arguments {
            let link = argument.strip_prefix("mf:").unwrap_or(&argument).to_owned();
            if !links.iter().any(|l| l.eq_ignore_ascii_case(&link)) {
                links.push(link);
            }
        }
        println!("Video devices (KSCATEGORY_VIDEO_CAMERA and arguments):");
        for link in links {
            let w = wide(&link);
            let name = interface_property(&w, &DEVPKEY_DeviceInterface_FriendlyName).unwrap_or_default();
            println!("  video: {name}");
            println!("    link:         {}", link.to_lowercase());
            let Some(instance) = interface_property(&w, &DEVPKEY_Device_InstanceId) else {
                println!("    instance id:  (not found)");
                continue;
            };
            println!("    instance id:  {instance}");
            let mut node = 0u32;
            let instance_w = wide(&instance);
            // SAFETY: valid out pointer and NUL-terminated id.
            let code = unsafe { CM_Locate_DevNodeW(&mut node, instance_w.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) };
            if code != CR_SUCCESS {
                println!("    device node:  not located (CONFIGRET {code})");
                continue;
            }
            if let Some(device) = node_property(node, &DEVPKEY_Device_FriendlyName) {
                println!("    device name:  {device}");
            }
            println!(
                "    container id: {}",
                node_property(node, &DEVPKEY_Device_ContainerId).unwrap_or_else(|| "(none)".into())
            );
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_keys {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use pipewire as pw;
    use pw::types::ObjectType;

    /// The device properties printed: the link candidates and what identifies the device.
    const DEVICE_KEYS: &[&str] = &[
        "device.name",
        "device.description",
        "device.api",
        "device.bus",
        "device.bus-path",
        "device.sysfs.path",
        "device.vendor.id",
        "device.product.id",
        "device.serial",
    ];

    type Props = BTreeMap<String, String>;

    /// A USB bus path without a trailing `:<n>.<n>` (the interface suffix), as research R5
    /// compares it; other paths (a PCI function such as `pci-0000:00:14.0`) are kept.
    fn key(bus_path: &str) -> &str {
        let Some((head, tail)) = bus_path.rsplit_once(':') else {
            return bus_path;
        };
        let is_interface = tail.split_once('.').is_some_and(|(a, b)| {
            !a.is_empty() && !b.is_empty() && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit())
        });
        if is_interface && head.contains("-usb-") {
            head
        } else {
            bus_path
        }
    }

    fn collect(props: &pw::spa::utils::dict::DictRef) -> Props {
        props.iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect()
    }

    /// Runs the loop until the server has answered everything sent so far.
    fn roundtrip(mainloop: &pw::main_loop::MainLoopRc, core: &pw::core::CoreRc) {
        let done = Rc::new(Cell::new(false));
        let pending = core.sync(0).expect("sync");
        let _listener = core
            .add_listener_local()
            .done({
                let (done, mainloop) = (done.clone(), mainloop.clone());
                move |id, seq| {
                    if id == pw::core::PW_ID_CORE && seq == pending {
                        done.set(true);
                        mainloop.quit();
                    }
                }
            })
            .register();
        while !done.get() {
            mainloop.run();
        }
    }

    pub fn main() {
        pw::init();
        let mainloop = pw::main_loop::MainLoopRc::new(None).expect("main loop");
        let context = pw::context::ContextRc::new(&mainloop, None).expect("context");
        let core = context.connect_rc(None).expect("connect to PipeWire");
        let registry = core.get_registry_rc().expect("registry");

        let nodes: Rc<RefCell<Vec<Props>>> = Rc::default();
        let devices: Rc<RefCell<BTreeMap<u32, Props>>> = Rc::default();
        // The registry's global carries only a few of a device's properties (no
        // `device.bus-path`): the full set comes with the bound device's info.
        let bound: Rc<RefCell<Vec<Box<dyn std::any::Any>>>> = Rc::default();
        let _listener = registry
            .add_listener_local()
            .global({
                let (nodes, devices, bound) = (nodes.clone(), devices.clone(), bound.clone());
                let registry = registry.downgrade();
                move |global| {
                    let Some(props) = global.props else { return };
                    match global.type_ {
                        ObjectType::Node => nodes.borrow_mut().push(collect(props)),
                        ObjectType::Device => {
                            let id = global.id;
                            devices.borrow_mut().insert(id, collect(props));
                            let Some(registry) = registry.upgrade() else { return };
                            let Ok(proxy) = registry.bind::<pw::device::Device, _>(global) else {
                                return;
                            };
                            let devices = devices.clone();
                            let listener = proxy
                                .add_listener_local()
                                .info(move |info| {
                                    if let Some(props) = info.props() {
                                        devices.borrow_mut().insert(id, collect(props));
                                    }
                                })
                                .register();
                            bound.borrow_mut().push(Box::new((proxy, listener)));
                        }
                        _ => {}
                    }
                }
            })
            .register();
        // Once for the globals, once for the bound devices' info.
        roundtrip(&mainloop, &core);
        roundtrip(&mainloop, &core);

        let devices = devices.borrow();
        for class in ["Audio/Source", "Audio/Sink", "Video/Source"] {
            println!("{class}:");
            for node in nodes
                .borrow()
                .iter()
                .filter(|n| n.get("media.class").map(String::as_str) == Some(class))
            {
                let get = |k: &str| node.get(k).map_or("", String::as_str);
                println!("  {}", get("node.description"));
                println!("    node.name:       {}", get("node.name"));
                let device = get("device.id").parse::<u32>().ok().and_then(|id| devices.get(&id));
                let Some(device) = device else {
                    println!("    device:          (none)");
                    continue;
                };
                println!("    device.id:       {}", get("device.id"));
                for k in DEVICE_KEYS {
                    if let Some(v) = device.get(*k) {
                        println!("    {:<16} {v}", format!("{k}:"));
                    }
                }
                if let Some(path) = device.get("device.bus-path") {
                    println!("    key:             {}", key(path));
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::key;

        #[test]
        fn removes_only_an_interface_suffix() {
            assert_eq!(key("pci-0000:00:14.0-usb-0:3:1.0"), "pci-0000:00:14.0-usb-0:3");
            assert_eq!(key("pci-0000:00:14.0-usb-0:3"), "pci-0000:00:14.0-usb-0:3");
            assert_eq!(key("pci-0000:00:14.0"), "pci-0000:00:14.0");
        }
    }
}
