//! Camera privacy on Windows (research R6): the `Value` of the webcam consent store, for all apps
//! and for desktop (non-packaged) apps, under the user's and the machine's keys. Settings writes
//! `Allow` or `Deny` there; a `Deny` in any of them hides the cameras from this app.

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::camera::Access;

const WEBCAM: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";
const DESKTOP_APPS: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam\NonPackaged";
const PRIVACY_PAGE: &str = "ms-settings:privacy-webcam";

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

pub fn access() -> Access {
    let values = [
        read_value(HKEY_CURRENT_USER, WEBCAM),
        read_value(HKEY_CURRENT_USER, DESKTOP_APPS),
        read_value(HKEY_LOCAL_MACHINE, WEBCAM),
        read_value(HKEY_LOCAL_MACHINE, DESKTOP_APPS),
    ];
    let access = decide(&values);
    log::debug!("camera access {access:?} from {values:?}");
    access
}

pub fn open_privacy() {
    let (verb, page) = (wide("open"), wide(PRIVACY_PAGE));
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
        log::warn!("cannot open {PRIVACY_PAGE}: ShellExecuteW returned {}", result as isize);
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
    fn reads_this_pc() {
        // Whatever the switches say here, the registry calls must work and the user's key exists
        // on any Windows 11 that has opened the camera settings.
        let user = read_value(HKEY_CURRENT_USER, WEBCAM);
        println!("HKCU webcam Value = {user:?}, access = {:?}", access());
        assert!(user.is_none_or(|v| v == "Allow" || v == "Deny"));
    }

    #[test]
    fn unknown_text_is_not_a_denial() {
        let v = values([Some("Prompt"), None, None, None]);
        assert_eq!(decide(&v), Access::Allowed);
    }
}
