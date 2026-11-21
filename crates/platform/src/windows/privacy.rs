//! Camera and microphone privacy on Windows (device preview research R6, preview audio R7): the
//! `Value` of the capability's consent store (`webcam` or `microphone`), for all apps and for
//! desktop (non-packaged) apps, under the user's and the machine's keys. Settings writes `Allow`
//! or `Deny` there; a `Deny` in any of them hides the cameras from this app or makes WASAPI refuse
//! the microphone.

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::privacy::{Access, Capability};

const CONSENT_STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

/// The consent store's subkey and the Settings page of a capability.
fn names(capability: Capability) -> (&'static str, &'static str) {
    match capability {
        Capability::Camera => ("webcam", "ms-settings:privacy-webcam"),
        Capability::Microphone => ("microphone", "ms-settings:privacy-microphone"),
    }
}

/// The capability's keys for all apps and for desktop apps.
fn keys(capability: Capability) -> [String; 2] {
    let store = format!(r"{CONSENT_STORE}\{}", names(capability).0);
    let desktop_apps = format!(r"{store}\NonPackaged");
    [store, desktop_apps]
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The string `Value` of `key` under `root`; None when the key or value is missing or unreadable.
fn read_value(root: HKEY, key: &str) -> Option<String> {
    let (key, value) = (wide(key), wide("Value"));
    let mut len = 0u32;
    // SAFETY: NUL-terminated strings that outlive the call; a null buffer asks for the size.
    let code = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut len,
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    let mut data = vec![0u16; (len as usize).div_ceil(2)];
    // SAFETY: `data` holds at least the `len` bytes the call reports writing at most.
    let code = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    let units: Vec<u16> = data.into_iter().take_while(|&u| u != 0).collect();
    String::from_utf16(&units).ok()
}

/// Any `Deny` → Denied; nothing readable → Unknown; else Allowed.
fn decide(values: &[Option<String>]) -> Access {
    if values.iter().flatten().any(|v| v.eq_ignore_ascii_case("Deny")) {
        Access::Denied
    } else if values.iter().all(Option::is_none) {
        Access::Unknown
    } else {
        Access::Allowed
    }
}

pub fn access(capability: Capability) -> Access {
    let [store, desktop_apps] = keys(capability);
    let values = [
        read_value(HKEY_CURRENT_USER, &store),
        read_value(HKEY_CURRENT_USER, &desktop_apps),
        read_value(HKEY_LOCAL_MACHINE, &store),
        read_value(HKEY_LOCAL_MACHINE, &desktop_apps),
    ];
    let access = decide(&values);
    log::debug!("{capability:?} access {access:?} from {values:?}");
    access
}

pub fn open_privacy(capability: Capability) {
    let page_name = names(capability).1;
    let (verb, page) = (wide("open"), wide(page_name));
    // SAFETY: NUL-terminated strings that outlive the call; no owner window, no parameters.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            page.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success; the rest are error codes.
    if result as isize <= 32 {
        log::warn!("cannot open {page_name}: ShellExecuteW returned {}", result as isize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(v: [Option<&str>; 4]) -> Vec<Option<String>> {
        v.iter().map(|s| s.map(String::from)).collect()
    }

    #[test]
    fn all_allowed_is_allowed() {
        let v = values([Some("Allow"), Some("Allow"), Some("Allow"), Some("Allow")]);
        assert_eq!(decide(&v), Access::Allowed);
    }

    #[test]
    fn any_deny_is_denied() {
        for i in 0..4 {
            let mut v = values([Some("Allow"), Some("Allow"), Some("Allow"), None]);
            v[i] = Some("Deny".into());
            assert_eq!(decide(&v), Access::Denied, "deny at {i}");
        }
    }

    #[test]
    fn deny_among_missing_is_denied() {
        let v = values([None, Some("Deny"), None, None]);
        assert_eq!(decide(&v), Access::Denied);
    }

    #[test]
    fn all_missing_is_unknown() {
        assert_eq!(decide(&values([None; 4])), Access::Unknown);
    }

    #[test]
    fn some_missing_is_allowed() {
        // This PC: the machine's desktop apps key has no value.
        let v = values([Some("Allow"), Some("Allow"), Some("Allow"), None]);
        assert_eq!(decide(&v), Access::Allowed);
    }

    #[test]
    fn keys_and_pages_per_capability() {
        let base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
        assert_eq!(
            keys(Capability::Camera),
            [format!(r"{base}\webcam"), format!(r"{base}\webcam\NonPackaged")]
        );
        assert_eq!(
            keys(Capability::Microphone),
            [format!(r"{base}\microphone"), format!(r"{base}\microphone\NonPackaged")]
        );
        assert_eq!(names(Capability::Camera).1, "ms-settings:privacy-webcam");
        assert_eq!(names(Capability::Microphone).1, "ms-settings:privacy-microphone");
    }

    #[test]
    fn reads_this_pc() {
        // Whatever the switches say here, the registry calls must work and the user's keys exist
        // on any Windows 11 that has opened the privacy settings.
        for capability in [Capability::Camera, Capability::Microphone] {
            let [store, _] = keys(capability);
            let user = read_value(HKEY_CURRENT_USER, &store);
            println!(
                "HKCU {capability:?} Value = {user:?}, access = {:?}",
                access(capability)
            );
            assert!(user.is_none_or(|v| v == "Allow" || v == "Deny"));
        }
    }

    #[test]
    fn unknown_text_is_not_a_denial() {
        let v = values([Some("Prompt"), None, None, None]);
        assert_eq!(decide(&v), Access::Allowed);
    }
}
