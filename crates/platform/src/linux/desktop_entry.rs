//! The app's Desktop Entry (`capturax.desktop`): the reading and quoting helpers the autostart
//! entry uses too, and [`ensure`], which makes sure an applications entry exists so the
//! GlobalShortcuts portal accepts the app id (spec 20261121-134830-screenshot, research R7 "Probe
//! result", contracts/platform.md portal rules).
//!
//! An installed entry (a package's, or one the user made) is used as it is. When none exists the
//! app writes a hidden one (`NoDisplay=true`) marked `X-CapturaX-Written=true`; later only that
//! marked entry's `Exec` line is replaced when the executable moved, so a user who unhid it keeps
//! that. A marked entry that shadows a package's entry is removed.

use std::io;
use std::path::{Path, PathBuf};

/// The entry's file name: the app id plus `.desktop` (the Wayland app id, research R12).
pub const FILE: &str = "capturax.desktop";
/// The key that marks an entry the app wrote itself.
const MARK: &str = "X-CapturaX-Written";

/// The entry's `[Desktop Entry]` keys and values, in order.
pub fn keys(text: &str) -> impl Iterator<Item = (&str, &str)> {
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

/// The `Exec` value for `exe` followed by `args` (none when empty): quoted as the Desktop Entry
/// spec asks (`"`, `` ` ``, `$` and `\` escaped inside the quotes, then `\` escaped again as a
/// string value, `%` doubled).
pub fn exec(exe: &Path, args: &str) -> String {
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
    quoted.push('"');
    if !args.is_empty() {
        quoted.push(' ');
        quoted.push_str(args);
    }
    quoted
}

/// The program an `Exec` value starts: its first argument, unescaped.
pub fn program(value: &str) -> Option<PathBuf> {
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

/// The program an entry should start: the AppImage file when running from one (the executable
/// itself is inside a mount that changes on every start), else this executable.
pub fn executable() -> io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(image) if !image.is_empty() => Ok(PathBuf::from(image)),
        _ => std::env::current_exe(),
    }
}

/// What [`ensure`] found or did, with the entry's path.
#[derive(Debug, PartialEq, Eq)]
pub enum Ensured {
    /// An entry the app did not write (a package's or the user's): used as it is.
    Installed(PathBuf),
    /// The app's own entry, already naming this executable.
    Current(PathBuf),
    /// No entry existed: the hidden one was written.
    Written(PathBuf),
    /// The app's own entry named another program: its `Exec` line was replaced.
    Updated(PathBuf),
}

/// Makes sure an applications entry for the app exists (see the module's rules), for `exe`.
pub fn ensure(exe: &Path) -> io::Result<Ensured> {
    let home = dirs::data_dir().ok_or_else(|| io::Error::other("no HOME or XDG_DATA_HOME"))?;
    ensure_in(&home, &data_dirs(), exe)
}

/// `$XDG_DATA_DIRS` (absolute entries only), else `/usr/local/share` and `/usr/share`.
fn data_dirs() -> Vec<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::var_os("XDG_DATA_DIRS")
        .map(|value| std::env::split_paths(&value).filter(|dir| dir.is_absolute()).collect())
        .unwrap_or_default();
    if dirs.is_empty() {
        vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]
    } else {
        dirs
    }
}

/// [`ensure`] with the data folders given (`home`: `$XDG_DATA_HOME`).
fn ensure_in(home: &Path, dirs: &[PathBuf], exe: &Path) -> io::Result<Ensured> {
    let own = home.join("applications").join(FILE);
    let installed = dirs
        .iter()
        .map(|dir| dir.join("applications").join(FILE))
        .find(|path| path.is_file());
    match std::fs::read_to_string(&own) {
        Ok(text) if !written_by_app(&text) => Ok(Ensured::Installed(own)),
        Ok(_) if installed.is_some() => {
            // A package installed its entry after the app wrote its own, which would hide it.
            std::fs::remove_file(&own)?;
            Ok(Ensured::Installed(installed.unwrap_or_default()))
        }
        Ok(text) => {
            let current = keys(&text)
                .find(|(key, _)| *key == "Exec")
                .and_then(|(_, value)| program(value));
            if current.as_deref() == Some(exe) {
                return Ok(Ensured::Current(own));
            }
            std::fs::write(&own, with_exec(&text, exe))?;
            Ok(Ensured::Updated(own))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => match installed {
            Some(path) => Ok(Ensured::Installed(path)),
            None => {
                if let Some(folder) = own.parent() {
                    std::fs::create_dir_all(folder)?;
                }
                std::fs::write(&own, entry(exe))?;
                Ok(Ensured::Written(own))
            }
        },
        Err(e) => Err(e),
    }
}

fn written_by_app(text: &str) -> bool {
    keys(text).any(|(key, value)| key == MARK && value == "true")
}

/// The hidden entry the app writes for `exe`.
fn entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=CapturaX\nExec={}\nTerminal=false\n\
         Categories=AudioVideo;Video;\nNoDisplay=true\n{MARK}=true\n",
        exec(exe, "")
    )
}

/// `text` with the `[Desktop Entry]` group's `Exec` line naming `exe` (added when missing); every
/// other line kept as it is.
fn with_exec(text: &str, exe: &Path) -> String {
    let line = format!("Exec={}", exec(exe, ""));
    let mut out = String::new();
    let mut in_group = false;
    let mut done = false;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            if in_group && !done {
                out.push_str(&line);
                out.push('\n');
                done = true;
            }
            in_group = trimmed == "[Desktop Entry]";
        } else if in_group && !done && trimmed.split_once('=').is_some_and(|(key, _)| key.trim() == "Exec") {
            out.push_str(&line);
            out.push('\n');
            done = true;
            continue;
        }
        out.push_str(raw);
        out.push('\n');
    }
    if in_group && !done {
        out.push_str(&line);
        out.push('\n');
    }
    out
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
            for args in ["", "--autostart"] {
                let value = exec(exe, args);
                assert_eq!(program(&value).as_deref(), Some(exe), "{value}");
            }
        }
        assert_eq!(exec(Path::new("/a b/c"), "--autostart"), r#""/a b/c" --autostart"#);
        assert_eq!(exec(Path::new("/a b/c"), ""), r#""/a b/c""#);
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
    fn the_written_entry_is_hidden_marked_and_names_the_exe() {
        let text = entry(Path::new("/opt/My Apps/capturax"));
        let get = |name: &str| keys(&text).find(|(key, _)| *key == name).map(|(_, value)| value);
        assert_eq!(get("NoDisplay"), Some("true"));
        assert!(written_by_app(&text));
        assert_eq!(
            get("Exec").and_then(program),
            Some(PathBuf::from("/opt/My Apps/capturax"))
        );
    }

    #[test]
    fn with_exec_replaces_only_the_main_group_s_exec() {
        let text = "# mine\n[Desktop Entry]\nName=CapturaX\nExec=/old/capturax\nNoDisplay=false\n\
                    X-CapturaX-Written=true\n[Desktop Action x]\nExec=/old/other\n";
        let changed = with_exec(text, Path::new("/new/capturax"));
        assert_eq!(
            changed,
            "# mine\n[Desktop Entry]\nName=CapturaX\nExec=\"/new/capturax\"\nNoDisplay=false\n\
             X-CapturaX-Written=true\n[Desktop Action x]\nExec=/old/other\n"
        );
        // A group with no Exec gets one, before the next group.
        let added = with_exec("[Desktop Entry]\nName=A\n[Other]\nB=1", Path::new("/c"));
        assert_eq!(added, "[Desktop Entry]\nName=A\nExec=\"/c\"\n[Other]\nB=1\n");
        let added = with_exec("[Desktop Entry]\nName=A", Path::new("/c"));
        assert_eq!(added, "[Desktop Entry]\nName=A\nExec=\"/c\"\n");
    }

    /// A temporary data layout: `home` and one system folder.
    struct Folders {
        root: PathBuf,
    }

    impl Folders {
        fn new(name: &str) -> Folders {
            let root = std::env::temp_dir().join(format!("capturax-entry-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Folders { root }
        }
        fn home(&self) -> PathBuf {
            self.root.join("home")
        }
        fn system(&self) -> PathBuf {
            self.root.join("system")
        }
        fn own(&self) -> PathBuf {
            self.home().join("applications").join(FILE)
        }
        fn put(&self, path: &Path, text: &str) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        fn ensure(&self, exe: &str) -> Ensured {
            ensure_in(&self.home(), &[self.system()], Path::new(exe)).unwrap()
        }
    }

    impl Drop for Folders {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn writes_then_keeps_then_follows_the_exe() {
        let f = Folders::new("own");
        assert_eq!(f.ensure("/a/capturax"), Ensured::Written(f.own()));
        assert_eq!(f.ensure("/a/capturax"), Ensured::Current(f.own()));
        // The user unhid it, then the folder moved: only Exec changes.
        let text = std::fs::read_to_string(f.own())
            .unwrap()
            .replace("NoDisplay=true", "NoDisplay=false");
        f.put(&f.own(), &text);
        assert_eq!(f.ensure("/b/capturax"), Ensured::Updated(f.own()));
        let text = std::fs::read_to_string(f.own()).unwrap();
        assert!(text.contains("NoDisplay=false"));
        assert!(text.contains("Exec=\"/b/capturax\""));
    }

    #[test]
    fn an_entry_the_app_did_not_write_is_left_alone() {
        let f = Folders::new("user");
        let mine = "[Desktop Entry]\nName=Mine\nExec=/somewhere/else\n";
        f.put(&f.own(), mine);
        assert_eq!(f.ensure("/a/capturax"), Ensured::Installed(f.own()));
        assert_eq!(std::fs::read_to_string(f.own()).unwrap(), mine);
    }

    #[test]
    fn a_package_s_entry_is_used_and_the_app_s_own_removed() {
        let f = Folders::new("package");
        let package = f.system().join("applications").join(FILE);
        f.put(&package, "[Desktop Entry]\nName=CapturaX\nExec=/usr/bin/capturax\n");
        assert_eq!(f.ensure("/a/capturax"), Ensured::Installed(package.clone()));
        assert!(!f.own().exists());
        // Written before the package came: removed so it does not hide the package's.
        f.put(&f.own(), &entry(Path::new("/a/capturax")));
        assert_eq!(f.ensure("/a/capturax"), Ensured::Installed(package));
        assert!(!f.own().exists());
    }
}
