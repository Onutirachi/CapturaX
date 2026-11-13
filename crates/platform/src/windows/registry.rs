//! Values of the current user's registry (`HKEY_CURRENT_USER`): the autostart entry and the
//! notification app id.

use std::io;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_ROUTINE_FLAGS, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(code: WIN32_ERROR) -> io::Result<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32))
    }
}

/// Reads a value of the user's `key`; None when the key or value is missing or unreadable.
pub fn read(key: &str, value: &str, kind: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
    let (key, value) = (wide(key), wide(value));
    let mut len = 0u32;
    // SAFETY: NUL-terminated strings that outlive the call; a null buffer asks for the size.
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            kind,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut len,
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    let mut data = vec![0u8; len as usize];
    // SAFETY: `data` has the `len` bytes the call reports writing at most.
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            kind,
            std::ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    data.truncate(len as usize);
    Some(data)
}

/// Reads a string value; None when it is missing or not a string.
pub fn read_string(key: &str, value: &str) -> Option<String> {
    let data = read(key, value, RRF_RT_REG_SZ)?;
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

/// Writes a string value, creating the key when it is missing.
pub fn set_string(key: &str, value: &str, data: &str) -> io::Result<()> {
    let (key, value, data) = (wide(key), wide(value), wide(data));
    // SAFETY: NUL-terminated strings that outlive the call; `data` is REG_SZ with its NUL.
    check(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        )
    })
}

/// Deletes a value; a missing one is not an error.
pub fn delete(key: &str, value: &str) -> io::Result<()> {
    let (key, value) = (wide(key), wide(value));
    // SAFETY: NUL-terminated strings that outlive the call.
    let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(code)
}
