//! Screenshots (spec 20261121-134830-screenshot): the requests' state (data-model.md "Screenshot
//! request"), the file writing (research R3, data-model.md "File name") and the worker (research R5).

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::notices::{EventNotice, Reason};
use crate::render::Frame;

/// At most this many screenshots wait for a frame or for their file (FR-007).
pub const MAX_WAITING: u32 = 4;

/// The screenshots under way (data-model.md "Screenshot request").
#[derive(Debug, Default)]
pub struct Screenshots {
    /// Requests sent or being written, 0 to [`MAX_WAITING`].
    waiting: u32,
    /// The number of the next request.
    serial: u64,
}

/// What a request or a taken frame asks of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Ask the video thread for the next frame for this screenshot.
    Take(u64),
    PlaySound,
    /// Hand the screenshot's frame to the worker.
    Write(u64),
}

/// Why a request takes no frame. Only logged: the missing capture sound tells the user (FR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The video area shows a state, or the window is hidden to the tray.
    NoImage,
    /// [`MAX_WAITING`] screenshots are already under way.
    Busy,
}

/// What the worker did with a screenshot's image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The file, or why there is none.
    pub saved: Result<PathBuf, Reason>,
    pub copied: bool,
}

impl Screenshots {
    /// The key, the button or the debug flag asks for a screenshot; `shows_image` tells whether
    /// the video area shows an image (not a state). The command to take it, or why not.
    pub fn request(&mut self, shows_image: bool) -> Result<Command, Refused> {
        if !shows_image {
            return Err(Refused::NoImage);
        }
        if self.waiting >= MAX_WAITING {
            return Err(Refused::Busy);
        }
        self.waiting += 1;
        self.serial += 1;
        Ok(Command::Take(self.serial))
    }

    /// The video thread handed over screenshot `serial`'s frame: the sound and the flash start
    /// now (FR-010), the file is written next.
    pub fn taken(&self, serial: u64) -> [Command; 2] {
        [Command::PlaySound, Command::Write(serial)]
    }

    /// No frame came for screenshot `serial` (500 ms, or the source closed); no notice (FR-008).
    pub fn not_taken(&mut self, _serial: u64) {
        self.waiting = self.waiting.saturating_sub(1);
    }

    /// The worker is done with screenshot `serial`: its notice (FR-008).
    pub fn written(&mut self, _serial: u64, written: Written) -> EventNotice {
        self.waiting = self.waiting.saturating_sub(1);
        match (written.saved, written.copied) {
            (Ok(path), true) => EventNotice::Saved(path),
            (Ok(path), false) => EventNotice::SavedNotCopied(path),
            (Err(reason), true) => EventNotice::CopiedNotSaved(reason),
            (Err(reason), false) => EventNotice::Failed(reason),
        }
    }
}

/// A local date and time to the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl LocalTime {
    /// `time` in the system's time zone.
    pub fn at(time: SystemTime) -> Self {
        let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        Self::from_local_seconds(secs + i64::from(platform::utc_offset(time)))
    }

    /// From seconds since 1970-01-01 00:00 local time.
    fn from_local_seconds(secs: i64) -> Self {
        let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        let (year, month, day) = crate::log::civil_from_days(days);
        let rest = rest as u32;
        Self {
            year,
            month,
            day,
            hour: rest / 3600,
            minute: rest % 3600 / 60,
            second: rest % 60,
        }
    }
}

/// `CapturaX YYYY-MM-DD HH-MM-SS.png`, with ` (n)` before the extension for `n` ≥ 2 (FR-005).
pub fn file_name(time: &LocalTime, n: u32) -> String {
    let LocalTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    } = time;
    let number = if n >= 2 { format!(" ({n})") } else { String::new() };
    format!("CapturaX {year:04}-{month:02}-{day:02} {hour:02}-{minute:02}-{second:02}{number}.png")
}

/// Why a file was not written (data-model.md "Outcome").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    FolderNotFound,
    NoPermission,
    DiskFull,
    Other,
}

impl WriteError {
    fn from_io(err: &io::Error) -> Self {
        // std gives StorageFull for ENOSPC and Windows' ERROR_DISK_FULL and ERROR_HANDLE_DISK_FULL.
        match err.kind() {
            io::ErrorKind::NotFound => Self::FolderNotFound,
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => Self::NoPermission,
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => Self::DiskFull,
            _ => Self::Other,
        }
    }
}

impl From<WriteError> for Reason {
    fn from(err: WriteError) -> Self {
        match err {
            WriteError::FolderNotFound => Reason::FolderNotFound,
            WriteError::NoPermission => Reason::NoPermission,
            WriteError::DiskFull => Reason::DiskFull,
            WriteError::Other => Reason::WriteError,
        }
    }
}

/// A `.part` file older than this was left by a crash (research R3).
const STALE_PART: Duration = Duration::from_secs(60);

/// Names past this are not tried (a folder full of one second's screenshots).
const MAX_NUMBER: u32 = 10_000;

/// Writes `bytes` (a PNG) into `folder` under [`file_name`] for `time`, never replacing a file and
/// never showing a partial one under the final name (FR-005, FR-006, research R3). The folder is
/// made when its parent exists.
pub fn write_png(folder: &Path, time: &LocalTime, bytes: &[u8]) -> Result<PathBuf, WriteError> {
    make_folder(folder)?;
    remove_stale_parts(folder);
    let part = folder.join(format!(".{}.part", file_name(time, 1)));
    let fail = |err: io::Error, what: &str| {
        log::warn!("screenshot: {what} {}: {err}", part.display());
        let _ = fs::remove_file(&part);
        WriteError::from_io(&err)
    };
    let mut file = File::create(&part).map_err(|err| fail(err, "cannot create"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|err| fail(err, "cannot write"))?;
    drop(file);
    for n in 1..=MAX_NUMBER {
        let target = folder.join(file_name(time, n));
        match fs::hard_link(&part, &target) {
            Ok(()) => {
                let _ = fs::remove_file(&part);
                return Ok(target);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            // No hard links on this file system (FAT32, exFAT; the error differs by system): the
            // name is checked, then the file renamed. The worker is the only writer.
            Err(link_err) => {
                if target.symlink_metadata().is_ok() {
                    continue;
                }
                return match fs::rename(&part, &target) {
                    Ok(()) => Ok(target),
                    Err(err) => {
                        log::debug!("screenshot: hard link refused: {link_err}");
                        Err(fail(err, "cannot rename"))
                    }
                };
            }
        }
    }
    Err(fail(io::Error::other("every name is taken"), "cannot name"))
}

/// Removes the `.part` files a crash left in `folder`.
fn remove_stale_parts(folder: &Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(".CapturaX ") && name.ends_with(".part")) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age > STALE_PART));
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Makes the screenshots folder when it is missing and its parent exists (FR-021). A folder that
/// is missing and cannot be made is "folder not found" whatever the create's error: on Linux a
/// removed drive's mount point is gone and its parent belongs to root, so the create is refused.
pub fn make_folder(folder: &Path) -> Result<(), WriteError> {
    if folder.is_dir() {
        return Ok(());
    }
    if !folder.parent().is_some_and(Path::is_dir) {
        return Err(WriteError::FolderNotFound);
    }
    fs::create_dir(folder)
        .or_else(|err| if folder.is_dir() { Ok(()) } else { Err(err) })
        .map_err(|err| {
            log::warn!("screenshot: cannot make {}: {err}", folder.display());
            WriteError::FolderNotFound
        })
}

/// A picked folder as kept in the settings (FR-022): without a trailing separator, except a root
/// (`/`, `C:\`).
pub fn picked_folder(text: &str) -> &str {
    let trimmed = text.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        // A root: keep its first separator.
        &text[..(trimmed.len() + 1).min(text.len())]
    } else {
        trimmed
    }
}

/// The folder for `[screenshot] folder` (data-model.md "Screenshots folder"): the path, or the
/// system's Pictures folder plus `CapturaX`.
pub fn folder(setting: &str) -> PathBuf {
    if setting == settings::DEFAULT_FOLDER {
        platform::default_screenshots_folder()
    } else {
        PathBuf::from(setting)
    }
}

/// Folders longer than this, in characters, are shortened in the settings (FR-020).
const SHORT_FOLDER: usize = 40;

/// The folder as the settings show it (FR-020): whole when short, else its start (the drive or
/// root and its first folder), "…" and as many of its last folders as fit in [`SHORT_FOLDER`]
/// characters, the last one always.
pub fn short_folder(path: &Path) -> String {
    shorten(&path.display().to_string(), std::path::MAIN_SEPARATOR)
}

fn shorten(text: &str, sep: char) -> String {
    let len = |s: &str| s.chars().count();
    if len(text) <= SHORT_FOLDER {
        return text.to_string();
    }
    let sep_text = sep.to_string();
    let parts: Vec<&str> = text.split(sep).collect();
    // The start ends with its second named part: `C:\Users`, `/home/me`, `\\server\share`.
    let Some(head_end) = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| !part.is_empty())
        .nth(1)
        .map(|(i, _)| i)
    else {
        return text.to_string();
    };
    let head = parts[..=head_end].join(&sep_text);
    // The start, "…" between two separators, then the tail.
    let fits = |start: usize| len(&head) + 3 + len(&parts[start..].join(&sep_text)) <= SHORT_FOLDER;
    let mut tail_start = parts.len() - 1;
    while tail_start > head_end + 2 && fits(tail_start - 1) {
        tail_start -= 1;
    }
    if tail_start <= head_end + 1 {
        return text.to_string();
    }
    format!("{head}{sep}…{sep}{}", parts[tail_start..].join(&sep_text))
}

/// A taken frame for the worker (research R5).
pub struct Job {
    pub serial: u64,
    pub frame: Arc<Frame>,
    /// From the request to the frame's hand-off.
    pub taken: Duration,
    /// Resolved from the settings when the frame was taken ([`folder`]).
    pub folder: PathBuf,
    /// When the frame was taken, for the file name.
    pub time: LocalTime,
}

/// Puts an RGBA image of `width × height` on the clipboard, at most 2 s (platform
/// `Clipboard::set_image`, T013); Err: a short reason for the log.
pub type CopyImage = Box<dyn Fn(u32, u32, Arc<[u8]>, Duration) -> Result<(), String> + Send>;

/// How long the clipboard may take (research R5).
pub const COPY_TIMEOUT: Duration = Duration::from_secs(2);

/// How long the quit waits for the screenshots under way (research R5).
pub const QUIT_WAIT: Duration = Duration::from_secs(3);

/// The `screenshot` thread: converts, encodes, writes and copies the screenshots one at a time,
/// in the order they were taken (FR-005, research R5).
pub struct Worker {
    jobs: Option<mpsc::Sender<Job>>,
    /// Disconnected when the thread ends.
    ended: mpsc::Receiver<()>,
}

impl Worker {
    /// Starts the thread; `done` gets each screenshot's outcome (the app posts it to the event
    /// loop).
    pub fn start(copy: CopyImage, done: impl Fn(u64, Written) + Send + 'static) -> io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let (alive, ended) = mpsc::channel::<()>();
        std::thread::Builder::new().name("screenshot".into()).spawn(move || {
            let _alive = alive;
            for job in queue {
                let serial = job.serial;
                done(serial, process(job, &copy));
            }
        })?;
        Ok(Self {
            jobs: Some(jobs),
            ended,
        })
    }

    /// Queues a taken frame; Err gives it back when the thread is gone.
    pub fn write(&self, job: Job) -> Result<(), Job> {
        match &self.jobs {
            Some(jobs) => jobs.send(job).map_err(|err| err.0),
            None => Err(job),
        }
    }

    /// Lets the queued screenshots finish, waiting at most `limit`; true when they did.
    pub fn stop(&mut self, limit: Duration) -> bool {
        self.jobs = None;
        match self.ended.recv_timeout(limit) {
            Err(mpsc::RecvTimeoutError::Disconnected) | Ok(()) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                log::warn!("screenshot: still writing after {} s; quitting", limit.as_secs());
                false
            }
        }
    }
}

/// One screenshot from frame to outcome: the frame converted (R2), encoded, written (R3), then
/// its RGBA copied (R4). A PNG that cannot be made is still copied. Logs one line as
/// contracts/cli.md shows.
fn process(job: Job, copy: &dyn Fn(u32, u32, Arc<[u8]>, Duration) -> Result<(), String>) -> Written {
    let Job {
        serial,
        frame,
        taken,
        folder,
        time,
    } = job;
    let ms = |d: Duration| d.as_millis();
    let mut line = format!(
        "screenshot {serial}: {}x{} taken +{} ms",
        frame.width,
        frame.height,
        ms(taken)
    );
    let at = Instant::now();
    let still = capture::still(&frame);
    // The frame's buffers go back to the pool now.
    drop(frame);
    line += &format!(", rgb {} ms", ms(at.elapsed()));

    let at = Instant::now();
    let png = still.png();
    line += &format!(", png {} ms", ms(at.elapsed()));
    let saved = match png {
        Ok(bytes) => {
            let at = Instant::now();
            let file = write_png(&folder, &time, &bytes).map_err(Reason::from);
            line += &format!(", written {} ms", ms(at.elapsed()));
            file
        }
        Err(err) => {
            log::warn!("screenshot {serial}: cannot encode the PNG: {err}");
            Err(Reason::ImageError)
        }
    };

    let at = Instant::now();
    let (width, height) = (still.width, still.height);
    let rgba: Arc<[u8]> = still.rgba().into();
    drop(still);
    let copied = copy(width, height, rgba, COPY_TIMEOUT);
    line += &format!(", copied {} ms", ms(at.elapsed()));

    line += &match (&saved, &copied) {
        (Ok(_), Ok(())) => ", saved and copied".to_string(),
        (Ok(_), Err(err)) => format!(", saved, not copied ({err})"),
        (Err(reason), Ok(())) => format!(", not saved ({reason:?}), copied"),
        (Err(reason), Err(err)) => format!(", not saved ({reason:?}), not copied ({err})"),
    };
    log::info!("{line}");
    Written {
        saved,
        copied: copied.is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A 2 × 2 RGBA frame: red, green, blue, white.
    fn frame() -> Arc<Frame> {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        Arc::new(Frame::new(
            2,
            2,
            capture::PixelFormat::Rgba,
            vec![capture::Plane {
                bytes: pixels.into(),
                stride: 8,
            }],
            capture::Matrix::Bt709,
            capture::Range::Full,
            Instant::now(),
            Instant::now(),
            1,
        ))
    }

    fn job(serial: u64, folder: &Path) -> Job {
        Job {
            serial,
            frame: frame(),
            taken: Duration::from_millis(14),
            folder: folder.to_path_buf(),
            time: time(),
        }
    }

    type Copies = Arc<Mutex<Vec<(u32, u32, Vec<u8>)>>>;

    /// A clipboard that keeps what it gets, or refuses.
    fn clipboard(works: bool) -> (Copies, CopyImage) {
        let copies = Copies::default();
        let kept = copies.clone();
        let copy: CopyImage = Box::new(move |w, h, rgba: Arc<[u8]>, timeout| {
            assert_eq!(timeout, COPY_TIMEOUT);
            kept.lock().unwrap().push((w, h, rgba.to_vec()));
            if works { Ok(()) } else { Err("no clipboard".into()) }
        });
        (copies, copy)
    }

    #[test]
    fn a_screenshot_is_written_as_png_and_copied_as_rgba() {
        let dir = tempfile::tempdir().unwrap();
        let (copies, copy) = clipboard(true);
        let written = process(job(1, dir.path()), &copy);
        let file = dir.path().join("CapturaX 2026-11-22 09-05-07.png");
        assert_eq!(
            written,
            Written {
                saved: Ok(file.clone()),
                copied: true
            }
        );
        let decoder = png::Decoder::new(io::BufReader::new(File::open(&file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut rgb = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut rgb).unwrap();
        assert_eq!(rgb, [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        let copies = copies.lock().unwrap();
        assert_eq!(copies.len(), 1);
        let (w, h, rgba) = &copies[0];
        assert_eq!((*w, *h), (2, 2));
        assert_eq!(rgba[..8], [255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn a_missing_folder_still_copies_and_a_refused_copy_still_saves() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone").join("CapturaX");
        let (_, copy) = clipboard(true);
        assert_eq!(
            process(job(1, &gone), &copy),
            Written {
                saved: Err(Reason::FolderNotFound),
                copied: true
            }
        );
        let (_, refuse) = clipboard(false);
        let written = process(job(2, dir.path()), &refuse);
        assert!(written.saved.is_ok() && !written.copied, "{written:?}");
        assert_eq!(
            process(job(3, &gone), &refuse),
            Written {
                saved: Err(Reason::FolderNotFound),
                copied: false
            }
        );
    }

    #[test]
    fn the_worker_writes_in_order_and_finishes_the_queue_at_stop() {
        let dir = tempfile::tempdir().unwrap();
        let (_, copy) = clipboard(true);
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let kept = outcomes.clone();
        let mut worker = Worker::start(copy, move |serial, written| {
            kept.lock().unwrap().push((serial, written.saved.unwrap()));
        })
        .unwrap();
        for serial in 1..=4 {
            assert!(worker.write(job(serial, dir.path())).is_ok());
        }
        assert!(worker.stop(QUIT_WAIT));
        let name = |n| dir.path().join(file_name(&time(), n));
        assert_eq!(
            *outcomes.lock().unwrap(),
            [(1, name(1)), (2, name(2)), (3, name(3)), (4, name(4))]
        );
        assert!(worker.write(job(5, dir.path())).is_err(), "stopped");
    }

    #[test]
    fn the_quit_waits_no_longer_than_its_limit() {
        let dir = tempfile::tempdir().unwrap();
        let copy: CopyImage = Box::new(|_, _, _, _| {
            std::thread::sleep(Duration::from_millis(500));
            Ok(())
        });
        let mut worker = Worker::start(copy, |_, _| {}).unwrap();
        assert!(worker.write(job(1, dir.path())).is_ok());
        let at = Instant::now();
        assert!(!worker.stop(Duration::from_millis(50)));
        assert!(at.elapsed() < Duration::from_millis(400));
    }

    #[test]
    fn short_folders_stay_whole() {
        let windows = r"C:\Users\clandini\Pictures\CapturaX";
        assert_eq!(shorten(windows, '\\'), windows);
        assert_eq!(shorten("/home/me/Pictures/CapturaX", '/'), "/home/me/Pictures/CapturaX");
    }

    #[test]
    fn long_folders_keep_their_start_and_end() {
        assert_eq!(
            shorten(r"D:\Arquivos\Mídia\Capturas de tela\Jogos\2026\PlayStation 5", '\\'),
            r"D:\Arquivos\…\Jogos\2026\PlayStation 5"
        );
        assert_eq!(
            shorten("/home/carlos/Documentos/Projetos/Vídeos/Capturas/CapturaX", '/'),
            "/home/carlos/…/Vídeos/Capturas/CapturaX"
        );
        assert_eq!(
            shorten(r"\\server\share\equipe\mídia\capturas\CapturaX 2026", '\\'),
            r"\\server\share\…\capturas\CapturaX 2026"
        );
        // A last folder too long to fit is kept whole; the panel elides what does not fit.
        let long = "/home/me/a/b/a folder whose name alone is longer than the limit";
        assert_eq!(
            shorten(long, '/'),
            "/home/me/…/a folder whose name alone is longer than the limit"
        );
        // Nothing to take out between the start and the last folder.
        let two = "/home/me/a folder whose name alone is longer than the limit";
        assert_eq!(shorten(two, '/'), two);
    }

    #[test]
    fn the_default_folder_is_the_systems() {
        assert_eq!(folder("default"), platform::default_screenshots_folder());
        let own = std::env::temp_dir().join("shots");
        assert_eq!(folder(&own.to_string_lossy()), own);
    }

    fn time() -> LocalTime {
        LocalTime {
            year: 2026,
            month: 11,
            day: 22,
            hour: 9,
            minute: 5,
            second: 7,
        }
    }

    fn files(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn names_carry_the_local_time_and_a_number_from_2() {
        assert_eq!(file_name(&time(), 1), "CapturaX 2026-11-22 09-05-07.png");
        assert_eq!(file_name(&time(), 2), "CapturaX 2026-11-22 09-05-07 (2).png");
        assert_eq!(file_name(&time(), 12), "CapturaX 2026-11-22 09-05-07 (12).png");
        // 2024-02-29 23:59:59, and the second after it.
        let leap = 1_709_251_199;
        let at = LocalTime::from_local_seconds(leap);
        assert_eq!(file_name(&at, 1), "CapturaX 2024-02-29 23-59-59.png");
        let next = LocalTime::from_local_seconds(leap + 1);
        assert_eq!((next.month, next.day, next.hour), (3, 1, 0));
        // Before 1970 (a negative local offset near the epoch).
        let before = LocalTime::from_local_seconds(-1);
        assert_eq!(
            (before.year, before.month, before.day, before.second),
            (1969, 12, 31, 59)
        );
    }

    #[test]
    fn the_local_time_follows_the_system_offset() {
        let now = SystemTime::now();
        let offset = platform::utc_offset(now);
        assert!(offset.abs() <= 14 * 3600, "{offset}");
        let secs = now.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(
            LocalTime::at(now),
            LocalTime::from_local_seconds(secs + i64::from(offset))
        );
    }

    #[test]
    fn files_are_numbered_and_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        fs::write(folder.join("CapturaX 2026-11-22 09-05-07.png"), b"old").unwrap();
        let second = write_png(folder, &time(), b"new").unwrap();
        assert_eq!(second, folder.join("CapturaX 2026-11-22 09-05-07 (2).png"));
        let third = write_png(folder, &time(), b"newer").unwrap();
        assert_eq!(third, folder.join("CapturaX 2026-11-22 09-05-07 (3).png"));
        assert_eq!(
            fs::read(folder.join("CapturaX 2026-11-22 09-05-07.png")).unwrap(),
            b"old"
        );
        assert_eq!(fs::read(&second).unwrap(), b"new");
        assert_eq!(fs::read(&third).unwrap(), b"newer");
        // No `.part` is left.
        assert_eq!(files(folder).len(), 3, "{:?}", files(folder));
    }

    #[test]
    fn the_folder_is_made_only_under_an_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("CapturaX");
        let file = write_png(&folder, &time(), b"png").unwrap();
        assert_eq!(file, folder.join("CapturaX 2026-11-22 09-05-07.png"));
        let deep = dir.path().join("gone").join("CapturaX");
        assert_eq!(write_png(&deep, &time(), b"png"), Err(WriteError::FolderNotFound));
        assert!(!dir.path().join("gone").exists());
    }

    #[test]
    fn a_folder_that_cannot_be_made_is_not_found() {
        // The create fails (here a file is in the way; on Linux a removed drive's mount point
        // under a root-owned parent): still "folder not found", not the create's error.
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("CapturaX");
        std::fs::write(&blocked, b"not a folder").unwrap();
        assert_eq!(make_folder(&blocked), Err(WriteError::FolderNotFound));
        assert_eq!(write_png(&blocked, &time(), b"png"), Err(WriteError::FolderNotFound));
    }

    #[test]
    fn a_picked_folder_loses_its_trailing_separator_but_a_root_keeps_it() {
        assert_eq!(picked_folder("/mnt/capxusb/"), "/mnt/capxusb");
        assert_eq!(picked_folder("/mnt/capxusb//"), "/mnt/capxusb");
        assert_eq!(picked_folder("/home/me/Shots"), "/home/me/Shots");
        assert_eq!(picked_folder("/"), "/");
        assert_eq!(picked_folder("//"), "/");
        assert_eq!(picked_folder(r"D:\Shots\"), r"D:\Shots");
        assert_eq!(picked_folder(r"C:\"), r"C:\");
        assert_eq!(picked_folder("C:"), "C:");
        assert_eq!(picked_folder(r"\\server\share\"), r"\\server\share");
    }

    #[test]
    fn parts_left_by_a_crash_go_after_a_minute() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        let old = folder.join(".CapturaX 2026-11-21 10-00-00.png.part");
        let fresh = folder.join(".CapturaX 2026-11-22 09-04-59.png.part");
        let other = folder.join(".other.part");
        for path in [&old, &fresh, &other] {
            fs::write(path, b"x").unwrap();
        }
        let long_ago = SystemTime::now() - Duration::from_secs(120);
        for path in [&old, &other] {
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }
        write_png(folder, &time(), b"png").unwrap();
        assert!(!old.exists(), "an old part of ours goes");
        assert!(fresh.exists(), "a recent one may still be written");
        assert!(other.exists(), "not ours");
    }

    #[test]
    fn io_errors_map_to_reasons() {
        let kind = |kind| WriteError::from_io(&io::Error::from(kind));
        assert_eq!(kind(io::ErrorKind::NotFound), WriteError::FolderNotFound);
        assert_eq!(kind(io::ErrorKind::PermissionDenied), WriteError::NoPermission);
        assert_eq!(kind(io::ErrorKind::ReadOnlyFilesystem), WriteError::NoPermission);
        assert_eq!(kind(io::ErrorKind::StorageFull), WriteError::DiskFull);
        assert_eq!(kind(io::ErrorKind::Interrupted), WriteError::Other);
        assert_eq!(kind(io::ErrorKind::QuotaExceeded), WriteError::DiskFull);
        assert_eq!(Reason::from(WriteError::Other), Reason::WriteError);
        assert_eq!(Reason::from(WriteError::DiskFull), Reason::DiskFull);
    }

    #[test]
    fn every_failure_has_its_notice() {
        use crate::notices::NoticeAction;
        // A write error (data-model.md "Outcome") and the conversion's.
        let reasons = [
            (WriteError::FolderNotFound.into(), Reason::FolderNotFound),
            (WriteError::NoPermission.into(), Reason::NoPermission),
            (WriteError::DiskFull.into(), Reason::DiskFull),
            (WriteError::Other.into(), Reason::WriteError),
            (Reason::ImageError, Reason::ImageError),
        ];
        let mut shots = Screenshots::default();
        for (reason, expected) in reasons {
            assert_eq!(reason, expected);
            for copied in [true, false] {
                let Ok(Command::Take(serial)) = shots.request(true) else {
                    panic!("refused");
                };
                let notice = shots.written(
                    serial,
                    Written {
                        saved: Err(reason),
                        copied,
                    },
                );
                if copied {
                    assert_eq!(notice, EventNotice::CopiedNotSaved(expected));
                    assert_eq!(notice.kind(), "copied-not-saved");
                    assert_eq!(notice.action(), Some(NoticeAction::Settings));
                } else {
                    assert_eq!(notice, EventNotice::Failed(expected));
                    assert_eq!(notice.kind(), "failed");
                    assert_eq!(notice.action(), None);
                }
            }
        }
        assert_eq!(shots.waiting, 0);
    }

    #[test]
    fn requests_follow_the_rules() {
        let mut shots = Screenshots::default();
        assert_eq!(shots.request(false), Err(Refused::NoImage));
        assert_eq!(shots.request(true), Ok(Command::Take(1)));
        assert_eq!(shots.taken(1), [Command::PlaySound, Command::Write(1)]);
        let file = PathBuf::from("a.png");
        let written = |saved: Result<PathBuf, Reason>, copied| Written { saved, copied };
        assert_eq!(
            shots.written(1, written(Ok(file.clone()), true)),
            EventNotice::Saved(file.clone())
        );
        assert_eq!(shots.request(true), Ok(Command::Take(2)));
        shots.not_taken(2);
        assert_eq!(shots.waiting, 0);
        // Every outcome.
        for (saved, copied, notice) in [
            (Ok(file.clone()), false, EventNotice::SavedNotCopied(file.clone())),
            (
                Err(Reason::DiskFull),
                true,
                EventNotice::CopiedNotSaved(Reason::DiskFull),
            ),
            (Err(Reason::ImageError), false, EventNotice::Failed(Reason::ImageError)),
        ] {
            let Ok(Command::Take(serial)) = shots.request(true) else {
                panic!("refused");
            };
            assert_eq!(shots.written(serial, written(saved, copied)), notice);
        }
        assert_eq!(shots.waiting, 0);
    }

    #[test]
    fn a_burst_of_6_takes_4_and_refuses_2() {
        let mut shots = Screenshots::default();
        let answers: Vec<_> = (0..6).map(|_| shots.request(true)).collect();
        assert_eq!(
            answers,
            [
                Ok(Command::Take(1)),
                Ok(Command::Take(2)),
                Ok(Command::Take(3)),
                Ok(Command::Take(4)),
                Err(Refused::Busy),
                Err(Refused::Busy),
            ]
        );
        // One done makes room for one more, with the next number.
        shots.not_taken(1);
        assert_eq!(shots.request(true), Ok(Command::Take(5)));
        assert_eq!(shots.request(true), Err(Refused::Busy));
    }
}
