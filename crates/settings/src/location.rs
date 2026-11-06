//! Where the settings and the log are kept (data-model.md "Settings location", research R6):
//! the application folder when it is writable, else the per-user folder, else memory only.
//! Resolved once at start from probe results the platform layer gives, so the rules are pure.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::FILE_NAME;

/// Where the settings are saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationKind {
    /// The application folder (FR-026).
    AppFolder(PathBuf),
    /// The per-user folder, because the application folder is not writable (FR-027).
    UserFolder(PathBuf),
    /// Nowhere: neither folder is writable, changes last until the app quits (FR-028).
    Memory,
}

/// The resolved location and the file to load at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub kind: LocationKind,
    /// A file to load instead of the location's own: a readable `capturax.toml` left in a
    /// read-only application folder, unless the per-user folder holds a newer one. `None` loads
    /// the location's file (nothing in memory).
    pub read_from: Option<PathBuf>,
}

/// Modification times of the `capturax.toml` files found at start; `None` when a folder has no
/// readable file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileTimes {
    pub app: Option<SystemTime>,
    pub user: Option<SystemTime>,
}

impl Location {
    /// The folder the settings and the log go to; `None` in memory.
    pub fn folder(&self) -> Option<&Path> {
        match &self.kind {
            LocationKind::AppFolder(path) | LocationKind::UserFolder(path) => Some(path),
            LocationKind::Memory => None,
        }
    }

    /// The settings file saves go to; `None` in memory.
    pub fn file(&self) -> Option<PathBuf> {
        self.folder().map(|folder| folder.join(FILE_NAME))
    }

    /// The file to load at start, if any.
    pub fn load_path(&self) -> Option<PathBuf> {
        self.read_from.clone().or_else(|| self.file())
    }
}

/// Picks the location in the order of data-model.md. `user_folder` is `None` when the system
/// gives no per-user folder.
pub fn resolve(
    app_folder: &Path,
    app_writable: bool,
    user_folder: Option<&Path>,
    user_writable: bool,
    times: FileTimes,
) -> Location {
    if app_writable {
        return Location {
            kind: LocationKind::AppFolder(app_folder.to_path_buf()),
            read_from: None,
        };
    }
    let app_file = times.app.map(|time| (time, app_folder.join(FILE_NAME)));
    let user_file = user_folder
        .zip(times.user)
        .map(|(folder, time)| (time, folder.join(FILE_NAME)));
    // The per-user file wins only when it is newer: it then holds the user's later changes.
    let newest = match (app_file, user_file) {
        (Some(app), Some(user)) => Some(if user.0 > app.0 { user } else { app }),
        (app, user) => app.or(user),
    };
    match user_folder {
        Some(folder) if user_writable => {
            let own = folder.join(FILE_NAME);
            let read_from = newest.map(|(_, path)| path).filter(|path| *path != own);
            Location {
                kind: LocationKind::UserFolder(folder.to_path_buf()),
                read_from,
            }
        }
        _ => Location {
            kind: LocationKind::Memory,
            read_from: newest.map(|(_, path)| path),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const APP: &str = "/opt/capturax";
    const USER: &str = "/home/u/.config/capturax";

    fn at(seconds: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    fn app_file() -> Option<PathBuf> {
        Some(Path::new(APP).join(FILE_NAME))
    }

    fn user_file() -> Option<PathBuf> {
        Some(Path::new(USER).join(FILE_NAME))
    }

    #[derive(Clone, Copy)]
    enum Kind {
        App,
        User,
        Memory,
    }

    #[derive(Clone, Copy)]
    enum Read {
        Own,
        AppFile,
        UserFile,
    }

    /// (case, app folder writable, per-user folder: None or Some(writable), app file time, user
    /// file time, expected kind, expected file to read)
    type Case = (&'static str, bool, Option<bool>, Option<u64>, Option<u64>, Kind, Read);

    #[test]
    fn every_branch() {
        use Kind::*;
        use Read::*;
        let cases: &[Case] = &[
            ("app writable", true, Some(true), Some(1), Some(2), App, Own),
            ("app writable, no user folder", true, None, None, None, App, Own),
            ("user, no files", false, Some(true), None, None, User, Own),
            ("user, only its own file", false, Some(true), None, Some(5), User, Own),
            (
                "user, read-only app file",
                false,
                Some(true),
                Some(5),
                None,
                User,
                AppFile,
            ),
            (
                "user, app file newer",
                false,
                Some(true),
                Some(9),
                Some(5),
                User,
                AppFile,
            ),
            (
                "user, same time: app file",
                false,
                Some(true),
                Some(5),
                Some(5),
                User,
                AppFile,
            ),
            ("user, user file newer", false, Some(true), Some(5), Some(9), User, Own),
            ("memory, user not writable", false, Some(false), None, None, Memory, Own),
            ("memory, no user folder", false, None, None, None, Memory, Own),
            (
                "memory, no user folder, app file",
                false,
                None,
                Some(5),
                None,
                Memory,
                AppFile,
            ),
            ("memory, app file", false, Some(false), Some(5), None, Memory, AppFile),
            ("memory, user file", false, Some(false), None, Some(5), Memory, UserFile),
            (
                "memory, newer user file",
                false,
                Some(false),
                Some(5),
                Some(9),
                Memory,
                UserFile,
            ),
            (
                "memory, newer app file",
                false,
                Some(false),
                Some(9),
                Some(5),
                Memory,
                AppFile,
            ),
        ];
        let at = |time: Option<u64>| time.map(|s| SystemTime::UNIX_EPOCH + Duration::from_secs(s));
        for &(case, app_writable, user, app_time, user_time, kind, read) in cases {
            let times = FileTimes {
                app: at(app_time),
                user: at(user_time),
            };
            let user_folder = user.map(|_| Path::new(USER));
            let location = resolve(Path::new(APP), app_writable, user_folder, user == Some(true), times);
            let kind = match kind {
                App => LocationKind::AppFolder(APP.into()),
                User => LocationKind::UserFolder(USER.into()),
                Memory => LocationKind::Memory,
            };
            let read_from = match read {
                Own => None,
                AppFile => app_file(),
                UserFile => user_file(),
            };
            assert_eq!(location.kind, kind, "{case}");
            assert_eq!(location.read_from, read_from, "{case}");
        }
    }

    #[test]
    fn paths() {
        let app = resolve(Path::new(APP), true, None, false, FileTimes::default());
        assert_eq!(app.folder(), Some(Path::new(APP)));
        assert_eq!(app.file(), app_file());
        assert_eq!(app.load_path(), app_file());

        let user = resolve(
            Path::new(APP),
            false,
            Some(Path::new(USER)),
            true,
            FileTimes { app: at(1), user: None },
        );
        assert_eq!(user.file(), user_file());
        assert_eq!(user.load_path(), app_file());

        let memory = resolve(Path::new(APP), false, None, false, FileTimes::default());
        assert_eq!(memory.folder(), None);
        assert_eq!(memory.file(), None);
        assert_eq!(memory.load_path(), None);
    }
}
