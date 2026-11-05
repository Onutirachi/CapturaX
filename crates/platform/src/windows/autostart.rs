//! Start with the system on Windows (FR-014b, research R11): the value `CapturaX` in the user's
//! `Run` key holds `"<exe>" --autostart`. Task Manager's Startup switch does not remove it: it
//! marks it disabled in `Explorer\StartupApproved\Run`, so the state reads both.

use std::io;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_ROUTINE_FLAGS, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE: &str = "CapturaX";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(code: WIN32_ERROR) -> io::Result<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32))
    }
}

/// Reads a value of the user's key; None when the key or value is missing or unreadable.
fn read(key: &str, kind: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
    let (key, value) = (wide(key), wide(VALUE));
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

fn read_string(key: &str) -> Option<String> {
    let data = read(key, RRF_RT_REG_SZ)?;
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

/// Deletes the value; a missing one is not an error.
fn delete(key: &str) -> io::Result<()> {
    let (key, value) = (wide(key), wide(VALUE));
    // SAFETY: NUL-terminated strings that outlive the call.
    let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(code)
}

/// The command written in the `Run` value.
fn command(exe: &Path) -> String {
    format!("\"{}\" --autostart", exe.display())
}

/// The program a `Run` command starts: the quoted first part, or up to the first space.
fn program(command: &str) -> Option<PathBuf> {
    let command = command.trim_start();
    let path = match command.strip_prefix('"') {
        Some(rest) => &rest[..rest.find('"')?],
        None => command.split(' ').next()?,
    };
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Task Manager's mark: the first byte's low bit set means disabled (02 enabled, 03 disabled).
fn approved(mark: Option<&[u8]>) -> bool {
    mark.and_then(|m| m.first()).is_none_or(|b| b & 1 == 0)
}

/// Whether the app starts at sign-in: the `Run` value exists and Task Manager has not disabled it.
pub fn state() -> bool {
    read_string(RUN).is_some() && approved(read(APPROVED, RRF_RT_REG_BINARY).as_deref())
}

/// Turns starting at sign-in on (for `exe`) or off. Turning it on also clears Task Manager's
/// disabled mark, so the switch in the app wins.
pub fn set(on: bool, exe: &Path) -> io::Result<()> {
    if !on {
        delete(RUN)?;
        return delete(APPROVED);
    }
    let (key, value) = (wide(RUN), wide(VALUE));
    let data = wide(&command(exe));
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
    })?;
    delete(APPROVED)
}

/// The program the entry starts, when there is an entry.
pub fn target() -> Option<PathBuf> {
    program(&read_string(RUN)?)
}

/// The program an entry should start: this executable.
pub fn executable() -> io::Result<PathBuf> {
    std::env::current_exe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_round_trips() {
        let exe = Path::new(r"D:\Portable Apps\CapturaX\capturax.exe");
        assert_eq!(command(exe), r#""D:\Portable Apps\CapturaX\capturax.exe" --autostart"#);
        assert_eq!(program(&command(exe)).as_deref(), Some(exe));
    }

    #[test]
    fn program_reads_unquoted_and_rejects_broken() {
        assert_eq!(
            program(r"C:\x\capturax.exe --autostart"),
            Some(PathBuf::from(r"C:\x\capturax.exe"))
        );
        assert_eq!(program(r#""C:\x\capturax.exe"#), None);
        assert_eq!(program(""), None);
    }

    #[test]
    fn task_manager_mark() {
        assert!(approved(None));
        assert!(approved(Some(&[])));
        assert!(approved(Some(&[2, 0, 0, 0])));
        assert!(approved(Some(&[6, 0])));
        assert!(!approved(Some(&[3, 0, 0, 0])));
        assert!(!approved(Some(&[7])));
    }
}
