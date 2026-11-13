//! Start with the system on Windows (FR-014b, research R11): the value `CapturaX` in the user's
//! `Run` key holds `"<exe>" --autostart`. Task Manager's Startup switch does not remove it: it
//! marks it disabled in `Explorer\StartupApproved\Run`, so the state reads both.

use std::io;
use std::path::{Path, PathBuf};

use windows_sys::Win32::System::Registry::RRF_RT_REG_BINARY;

use super::registry;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE: &str = "CapturaX";

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
    registry::read_string(RUN, VALUE).is_some()
        && approved(registry::read(APPROVED, VALUE, RRF_RT_REG_BINARY).as_deref())
}

/// Turns starting at sign-in on (for `exe`) or off. Turning it on also clears Task Manager's
/// disabled mark, so the switch in the app wins.
pub fn set(on: bool, exe: &Path) -> io::Result<()> {
    if !on {
        registry::delete(RUN, VALUE)?;
        return registry::delete(APPROVED, VALUE);
    }
    registry::set_string(RUN, VALUE, &command(exe))?;
    registry::delete(APPROVED, VALUE)
}

/// The program the entry starts, when there is an entry.
pub fn target() -> Option<PathBuf> {
    program(&registry::read_string(RUN, VALUE)?)
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
