//! The screenshots folder and the system's file manager (screenshot spec, research R12,
//! contracts/platform.md "Files").

use std::path::{Path, PathBuf};

use winit::window::Window;

/// The system's Pictures folder plus `CapturaX`; the home folder plus `Pictures/CapturaX` when the
/// system reports none.
pub fn default_screenshots_folder() -> PathBuf {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Pictures")))
        .unwrap_or_else(|| PathBuf::from("Pictures"))
        .join("CapturaX")
}

/// Where [`pick_folder`] hands its answer: the folder chosen, None when cancelled or failed.
pub type FolderPicked = Box<dyn FnOnce(Option<PathBuf>) + Send + 'static>;

/// Opens the system's folder dialog over `owner`, starting at `start` (FR-020): Windows
/// `IFileOpenDialog`, Linux the desktop portal's `FileChooser`. Returns at once; the answer comes
/// through `done` on another thread, once. False, and `done` dropped unused, when the system has
/// no folder dialog (Linux without the portal).
pub fn pick_folder(owner: &Window, start: &Path, done: impl FnOnce(Option<PathBuf>) + Send + 'static) -> bool {
    crate::os::files::pick_folder(owner, start, Box::new(done))
}

/// Opens the file manager on the file's folder with the file selected where the platform allows
/// it, else on the folder (FR-009). Returns at once: the work runs on a short-lived thread.
pub fn show_file(file: &Path) {
    let file = file.to_path_buf();
    spawn("show file", move || crate::os::files::show_file(&file));
}

/// Opens the file manager on `folder`. Returns at once, as [`show_file`].
pub fn open_folder(folder: &Path) {
    let folder = folder.to_path_buf();
    spawn("open folder", move || crate::os::files::open_folder(&folder));
}

pub(crate) fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(err) = std::thread::Builder::new().name(name.into()).spawn(work) {
        log::warn!("files: cannot start the {name} thread: {err}");
    }
}

/// `file://` and the absolute path, every byte outside the unreserved set and `/` percent-encoded
/// (RFC 8089).
#[cfg(any(target_os = "linux", test))]
pub(crate) fn file_uri(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut uri = String::from("file://");
    for &b in bytes {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

/// The path of a `file://` URI (the portal's answer), percent-decoded; None for another scheme
/// or a host other than empty or `localhost`.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut input = rest.bytes();
    while let Some(b) = input.next() {
        if b == b'%' {
            let hex = [input.next()?, input.next()?];
            bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            bytes.push(b);
        }
    }
    // Any bytes form a path on Unix; elsewhere (the tests on Windows) they must be UTF-8.
    #[cfg(unix)]
    let path = PathBuf::from(<std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(bytes));
    #[cfg(not(unix))]
    let path = PathBuf::from(String::from_utf8(bytes).ok()?);
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_encode_what_is_not_plain() {
        assert_eq!(
            file_uri(Path::new(
                "/home/me/Pictures/CapturaX/CapturaX 2026-11-22 10-00-00 (2).png"
            )),
            "file:///home/me/Pictures/CapturaX/CapturaX%202026-11-22%2010-00-00%20%282%29.png"
        );
        assert_eq!(
            file_uri(Path::new("/home/me/Imagens/ação")),
            "file:///home/me/Imagens/a%C3%A7%C3%A3o"
        );
    }

    #[test]
    fn file_uris_decode_back_to_paths() {
        for path in ["/home/me/Pictures/CapturaX (2)", "/home/me/Imagens/ação", "/mnt/d/100%"] {
            assert_eq!(
                path_from_file_uri(&file_uri(Path::new(path))),
                Some(PathBuf::from(path))
            );
        }
        assert_eq!(
            path_from_file_uri("file://localhost/tmp/a%20b"),
            Some(PathBuf::from("/tmp/a b"))
        );
        assert_eq!(path_from_file_uri("file:///tmp/x"), Some(PathBuf::from("/tmp/x")));
        assert_eq!(path_from_file_uri("https://example.com/x"), None);
        assert_eq!(path_from_file_uri("file://server/share"), None);
        assert_eq!(path_from_file_uri("file:///tmp/%2"), None);
        assert_eq!(path_from_file_uri("file:///tmp/%zz"), None);
    }

    #[test]
    fn the_default_folder_ends_in_capturax() {
        let folder = default_screenshots_folder();
        assert!(folder.ends_with("CapturaX"), "{}", folder.display());
    }
}
