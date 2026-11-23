//! Start with the system on Linux (FR-014b, research R11): the XDG autostart entry
//! `$XDG_CONFIG_HOME/autostart/capturax.desktop` with `Exec="<exe>" --autostart`. Desktop tools
//! turn it off by setting `Hidden=true` or `X-GNOME-Autostart-enabled=false` rather than deleting
//! it, so the state reads both.

use std::io;
use std::path::{Path, PathBuf};

use super::desktop_entry::{FILE, exec, keys, program};

/// `$XDG_CONFIG_HOME` when set to an absolute path, else `~/.config`.
fn config_home() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    match xdg {
        Some(dir) if dir.is_absolute() => Some(dir),
        _ => std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(|home| PathBuf::from(home).join(".config")),
    }
}

fn entry_path() -> Option<PathBuf> {
    config_home().map(|dir| dir.join("autostart").join(FILE))
}

fn read_entry() -> Option<String> {
    std::fs::read_to_string(entry_path()?).ok()
}

/// Whether an entry starts the app: not hidden and not switched off by GNOME's key.
fn enabled(text: &str) -> bool {
    !keys(text).any(|(key, value)| {
        (key == "Hidden" && value == "true") || (key == "X-GNOME-Autostart-enabled" && value == "false")
    })
}

/// The whole entry file for `exe`.
fn entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=CapturaX\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec(exe, "--autostart")
    )
}

/// Whether the app starts at sign-in: the entry exists and is not switched off.
pub fn state() -> bool {
    read_entry().is_some_and(|text| enabled(&text))
}

/// Turns starting at sign-in on (for `exe`, rewriting the entry so it is enabled) or off
/// (removing the entry).
pub fn set(on: bool, exe: &Path) -> io::Result<()> {
    let path = entry_path().ok_or_else(|| io::Error::other("no HOME or XDG_CONFIG_HOME"))?;
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, entry(exe))
}

/// The program the entry starts, when there is an entry.
pub fn target() -> Option<PathBuf> {
    let text = read_entry()?;
    keys(&text)
        .find(|(key, _)| *key == "Exec")
        .and_then(|(_, value)| program(value))
}

pub use super::desktop_entry::executable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_entry_is_enabled_and_points_to_exe() {
        let text = entry(Path::new("/opt/capturax"));
        assert!(enabled(&text));
        let exec = keys(&text).find(|(k, _)| *k == "Exec").unwrap().1;
        assert_eq!(program(exec), Some(PathBuf::from("/opt/capturax")));
    }

    #[test]
    fn switched_off_entries() {
        let base = "[Desktop Entry]\nType=Application\nExec=capturax\n";
        assert!(enabled(base));
        assert!(!enabled(&format!("{base}Hidden=true\n")));
        assert!(!enabled(&format!("{base}X-GNOME-Autostart-enabled = false\n")));
        assert!(enabled(&format!(
            "{base}Hidden=false\nX-GNOME-Autostart-enabled=true\n"
        )));
        // Keys of other groups do not count.
        assert!(enabled(&format!("{base}[Desktop Action x]\nHidden=true\n")));
    }
}
