//! Start with the system on Linux (FR-014b, research R11): the XDG autostart entry
//! `$XDG_CONFIG_HOME/autostart/capturax.desktop` with `Exec="<exe>" --autostart`. Desktop tools
//! turn it off by setting `Hidden=true` or `X-GNOME-Autostart-enabled=false` rather than deleting
//! it, so the state reads both.

use std::io;
use std::path::{Path, PathBuf};

const FILE: &str = "capturax.desktop";

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

/// The entry's `[Desktop Entry]` keys and values, in order.
fn keys(text: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut in_group = false;
    text.lines().filter_map(move |line| {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            return None;
        }
        if !in_group || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        Some((key.trim(), value.trim()))
    })
}

/// Whether an entry starts the app: not hidden and not switched off by GNOME's key.
fn enabled(text: &str) -> bool {
    !keys(text).any(|(key, value)| {
        (key == "Hidden" && value == "true") || (key == "X-GNOME-Autostart-enabled" && value == "false")
    })
}

/// The `Exec` line for `exe`: quoted as the Desktop Entry spec asks (`"`, `` ` ``, `$` and `\`
/// escaped inside the quotes, then `\` escaped again as a string value, `%` doubled).
fn exec(exe: &Path) -> String {
    let mut quoted = String::from("\"");
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str(r"\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str(r"\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push_str("\" --autostart");
    quoted
}

/// The program an `Exec` value starts: its first argument, unescaped.
fn program(value: &str) -> Option<PathBuf> {
    // String-value escapes first (`\\` → `\`), then the argument quoting.
    let mut unescaped = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match (c, c == '\\') {
            (_, true) => match chars.next()? {
                's' => unescaped.push(' '),
                'n' => unescaped.push('\n'),
                't' => unescaped.push('\t'),
                'r' => unescaped.push('\r'),
                other => unescaped.push(other),
            },
            _ => unescaped.push(c),
        }
    }
    let mut path = String::new();
    let mut chars = unescaped.trim_start().chars();
    if unescaped.trim_start().starts_with('"') {
        chars.next();
        loop {
            match chars.next()? {
                '"' => break,
                '\\' => path.push(chars.next()?),
                c => path.push(c),
            }
        }
    } else {
        path.extend(chars.take_while(|&c| c != ' '));
    }
    let path = path.replace("%%", "%");
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The whole entry file for `exe`.
fn entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=CapturaX\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec(exe)
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

/// The program an entry should start: the AppImage file when running from one (the executable
/// itself is inside a mount that changes on every start), else this executable.
pub fn executable() -> io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(image) if !image.is_empty() => Ok(PathBuf::from(image)),
        _ => std::env::current_exe(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_round_trips() {
        for exe in [
            "/home/me/Apps/CapturaX.AppImage",
            "/home/me/My Apps/capturax",
            r#"/odd/"quote"/$dollar/`tick`/back\slash/100%/capturax"#,
        ] {
            let exe = Path::new(exe);
            assert_eq!(program(&exec(exe)).as_deref(), Some(exe), "{}", exec(exe));
        }
        assert_eq!(exec(Path::new("/a b/c")), r#""/a b/c" --autostart"#);
    }

    #[test]
    fn program_reads_unquoted_and_rejects_broken() {
        assert_eq!(
            program("/usr/bin/capturax --autostart"),
            Some(PathBuf::from("/usr/bin/capturax"))
        );
        assert_eq!(program(r#""/usr/bin/capturax"#), None);
        assert_eq!(program(""), None);
    }

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
