//! Loading and saving the settings file (contracts/settings-file.md rules 5 and 6, research R6).

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toml::Table;

/// What was found at the settings path.
#[derive(Debug)]
pub enum LoadOutcome {
    /// No file yet: defaults.
    NoFile,
    /// A valid TOML document (its values are read leniently by `Settings::read`).
    Loaded(Table),
    /// Not valid TOML, not UTF-8, or unreadable: defaults are used and the file is renamed to
    /// `<name>.damaged-<unix seconds>` (`kept_as`; none if it could not be renamed).
    Damaged { kept_as: Option<PathBuf> },
}

pub fn load(path: &Path) -> LoadOutcome {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return LoadOutcome::NoFile,
        Err(err) => {
            log::warn!("settings file {} unreadable: {err}", path.display());
            return damaged(path);
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            log::warn!("settings file {} is not UTF-8", path.display());
            return damaged(path);
        }
    };
    match text.parse::<Table>() {
        Ok(table) => LoadOutcome::Loaded(table),
        Err(err) => {
            log::warn!("settings file {} is not valid TOML: {err}", path.display());
            damaged(path)
        }
    }
}

fn damaged(path: &Path) -> LoadOutcome {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".damaged-{secs}"));
    let kept = path.with_file_name(name);
    match std::fs::rename(path, &kept) {
        Ok(()) => {
            log::warn!("damaged settings file kept as {}", kept.display());
            LoadOutcome::Damaged { kept_as: Some(kept) }
        }
        Err(err) => {
            log::warn!("could not set the damaged settings file aside: {err}");
            LoadOutcome::Damaged { kept_as: None }
        }
    }
}

/// Prefix and suffix of the temporary files [`save`] writes.
const TEMP_PREFIX: &str = ".capturax-";
const TEMP_SUFFIX: &str = ".tmp";

/// Removes the temporary files interrupted saves left in `folder` (rule 6). Call only while no
/// other instance can be saving there (after the single-instance claim). Returns how many went.
pub fn remove_temps(folder: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(TEMP_PREFIX) && name.ends_with(TEMP_SUFFIX)) || !entry.path().is_file() {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => removed += 1,
            Err(err) => log::warn!("cannot remove {}: {err}", entry.path().display()),
        }
    }
    removed
}

/// Saves `table` to `path` atomically: a temporary file in the same folder is written and synced,
/// then replaces the target, so an interruption leaves either the old or the new file (FR-029).
pub fn save(path: &Path, table: &Table) -> io::Result<()> {
    let text = toml::to_string(table).map_err(io::Error::other)?;
    let folder = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let mut temp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(TEMP_SUFFIX)
        .tempfile_in(folder)?;
    temp.write_all(text.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    // Makes the rename itself durable where folders can be synced (Linux). Windows cannot open a
    // folder this way and needs no such step: the open fails and is ignored.
    if let Ok(dir) = File::open(folder) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Table {
        text.parse().unwrap()
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(&dir.path().join("capturax.toml")), LoadOutcome::NoFile));
    }

    #[test]
    fn loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        std::fs::write(&path, "version = 1\n[general]\ntheme = \"dark\"\n").unwrap();
        match load(&path) {
            LoadOutcome::Loaded(t) => assert_eq!(t["general"]["theme"].as_str(), Some("dark")),
            other => panic!("{other:?}"),
        }
    }

    fn assert_damaged(contents: &[u8]) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        std::fs::write(&path, contents).unwrap();
        match load(&path) {
            LoadOutcome::Damaged { kept_as: Some(kept) } => {
                assert!(!path.exists());
                assert_eq!(std::fs::read(&kept).unwrap(), contents);
                let name = kept.file_name().unwrap().to_string_lossy().into_owned();
                let secs = name.strip_prefix("capturax.toml.damaged-").expect(&name);
                assert!(secs.parse::<u64>().is_ok(), "{name}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn not_toml_is_damaged() {
        assert_damaged(b"this is [not toml");
    }

    #[test]
    fn not_utf8_is_damaged() {
        assert_damaged(&[0x76, 0x3d, 0xff, 0xfe, 0x00]);
    }

    #[test]
    fn save_creates_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        save(&path, &table("version = 1\na = 1")).unwrap();
        save(&path, &table("version = 1\na = 2")).unwrap();
        match load(&path) {
            LoadOutcome::Loaded(t) => assert_eq!(t["a"].as_integer(), Some(2)),
            other => panic!("{other:?}"),
        }
        // No temporary file is left behind.
        assert_eq!(files(dir.path()), ["capturax.toml"]);
    }

    #[test]
    fn save_keeps_key_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        save(&path, &table("version = 1\nzeta = 1\nalpha = 2")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.find("zeta").unwrap() < text.find("alpha").unwrap(), "{text}");
    }

    #[test]
    fn remove_temps_keeps_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            ".capturax-a1B2c3.tmp",
            ".capturax-x.tmp",
            "capturax.toml",
            "capturax.log",
            "other.tmp",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join(".capturax-dir.tmp")).unwrap();
        assert_eq!(remove_temps(dir.path()), 2);
        assert_eq!(
            files(dir.path()),
            [".capturax-dir.tmp", "capturax.log", "capturax.toml", "other.tmp"]
        );
        assert_eq!(remove_temps(&dir.path().join("missing")), 0);
    }
}
