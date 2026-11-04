//! The local log (FR-032, contracts/settings-file.md "Log"): `capturax.log` next to the settings,
//! one line per event, at most 1 MiB; when full it becomes `capturax.log.1` and a new file starts.
//! Never sent anywhere. Without a folder (settings in memory) it writes to stderr only.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FILE_NAME: &str = "capturax.log";
const OLD_FILE_NAME: &str = "capturax.log.1";
const MAX_BYTES: u64 = 1024 * 1024;

pub struct Logger {
    level: log::LevelFilter,
    out: Mutex<Output>,
}

enum Output {
    Stderr,
    File {
        folder: PathBuf,
        file: Option<File>,
        len: u64,
        max: u64,
    },
}

impl Logger {
    pub fn new(folder: Option<&Path>, level: log::LevelFilter) -> Self {
        Self::with_limit(folder, level, MAX_BYTES)
    }

    fn with_limit(folder: Option<&Path>, level: log::LevelFilter, max: u64) -> Self {
        let out = match folder {
            None => Output::Stderr,
            Some(folder) => Output::File {
                folder: folder.to_path_buf(),
                file: None,
                len: 0,
                max,
            },
        };
        Self {
            level,
            out: Mutex::new(out),
        }
    }

    /// Installs this logger as the `log` crate's logger.
    pub fn install(self) -> Result<(), log::SetLoggerError> {
        log::set_max_level(self.level);
        log::set_boxed_logger(Box::new(self))
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {} {} {}\n",
            timestamp(SystemTime::now()),
            record.level(),
            record.target(),
            record.args()
        );
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *out {
            Output::Stderr => eprint!("{line}"),
            Output::File { folder, file, len, max } => {
                if file.is_some() && *len + line.len() as u64 > *max {
                    *file = None;
                    let _ = std::fs::rename(folder.join(FILE_NAME), folder.join(OLD_FILE_NAME));
                }
                if file.is_none() {
                    match OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(folder.join(FILE_NAME))
                    {
                        Ok(f) => {
                            *len = f.metadata().map(|m| m.len()).unwrap_or(0);
                            *file = Some(f);
                            // An existing file that is already full rotates before this line.
                            if *len > 0 && *len + line.len() as u64 > *max {
                                *file = None;
                                let _ = std::fs::rename(folder.join(FILE_NAME), folder.join(OLD_FILE_NAME));
                                if let Ok(f) = OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(folder.join(FILE_NAME))
                                {
                                    *len = 0;
                                    *file = Some(f);
                                }
                            }
                        }
                        Err(_) => {
                            eprint!("{line}");
                            return;
                        }
                    }
                }
                if let Some(f) = file {
                    if f.write_all(line.as_bytes()).is_ok() {
                        *len += line.len() as u64;
                    } else {
                        eprint!("{line}");
                    }
                }
            }
        }
    }

    fn flush(&self) {
        if let Output::File { file: Some(f), .. } = &mut *self.out.lock().unwrap_or_else(|e| e.into_inner()) {
            let _ = f.flush();
        }
    }
}

/// RFC 3339 in UTC with milliseconds, for example `2026-11-04T17:38:02.125Z`.
fn timestamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        since.subsec_millis()
    )
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;
    use std::time::Duration;

    fn write(logger: &Logger, message: &str) {
        logger.log(
            &log::Record::builder()
                .level(log::Level::Info)
                .target("test")
                .args(format_args!("{message}"))
                .build(),
        );
    }

    #[test]
    fn timestamps() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // 2024-02-29T12:34:56.789Z (a leap day)
        let t = UNIX_EPOCH + Duration::from_millis(1_709_210_096_789);
        assert_eq!(timestamp(t), "2024-02-29T12:34:56.789Z");
        // 2026-11-04T00:00:00Z
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::from_secs(1_793_750_400)),
            "2026-11-04T00:00:00.000Z"
        );
    }

    #[test]
    fn line_format() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(Some(dir.path()), log::LevelFilter::Info);
        write(&logger, "hello");
        logger.flush();
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        let (time, rest) = text.split_once(' ').unwrap();
        assert!(time.ends_with('Z') && time.len() == 24, "{time}");
        assert_eq!(rest, "INFO test hello\n");
    }

    #[test]
    fn level_filter() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(Some(dir.path()), log::LevelFilter::Warn);
        write(&logger, "info is filtered");
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn rotation() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::with_limit(Some(dir.path()), log::LevelFilter::Info, 200);
        for i in 0..10 {
            write(&logger, &format!("line {i} {}", "x".repeat(20)));
        }
        let current = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        let old = std::fs::read_to_string(dir.path().join(OLD_FILE_NAME)).unwrap();
        assert!(current.len() <= 200 && old.len() <= 200);
        assert!(current.ends_with("line 9 xxxxxxxxxxxxxxxxxxxx\n"));
        // The previous `.1` is replaced: only the two newest files exist.
        assert!(!old.contains("line 0 "));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn full_file_from_a_previous_run_rotates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "y".repeat(190)).unwrap();
        let logger = Logger::with_limit(Some(dir.path()), log::LevelFilter::Info, 200);
        write(&logger, "new run");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(OLD_FILE_NAME)).unwrap(),
            "y".repeat(190)
        );
        assert!(
            std::fs::read_to_string(dir.path().join(FILE_NAME))
                .unwrap()
                .ends_with("new run\n")
        );
    }
}
