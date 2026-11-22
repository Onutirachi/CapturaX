//! The screenshots folder and the system's file manager (screenshot spec, research R12,
//! contracts/platform.md "Files").

use std::path::{Path, PathBuf};

/// The system's Pictures folder plus `CapturaX`; the home folder plus `Pictures/CapturaX` when the
/// system reports none.
pub fn default_screenshots_folder() -> PathBuf {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Pictures")))
        .unwrap_or_else(|| PathBuf::from("Pictures"))
        .join("CapturaX")
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

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
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
    fn the_default_folder_ends_in_capturax() {
        let folder = default_screenshots_folder();
        assert!(folder.ends_with("CapturaX"), "{}", folder.display());
    }
}
