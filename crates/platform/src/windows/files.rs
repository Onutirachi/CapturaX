//! The file manager on Windows (research R12): Explorer with the file selected through
//! `SHOpenFolderAndSelectItems`, a folder through `ShellExecuteW("open")`.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
use windows::core::PCWSTR;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn show_file(file: &Path) {
    let path = wide(file);
    // SAFETY: COM is set up on this short-lived thread and torn down at its end; `path` is
    // NUL-terminated and outlives the calls; the item list is freed once, after its last use.
    let shown = unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let item = ILCreateFromPathW(PCWSTR(path.as_ptr()));
        let shown = if item.is_null() {
            Err(windows::core::Error::from_thread())
        } else {
            // The file's own item with no children: Explorer opens its folder and selects it.
            let shown = SHOpenFolderAndSelectItems(item, None, 0);
            ILFree(Some(item));
            shown
        };
        if com {
            CoUninitialize();
        }
        shown
    };
    if let Err(err) = shown {
        log::warn!("files: cannot show {}: {err}; opening its folder", file.display());
        if let Some(folder) = file.parent() {
            open_folder(folder);
        }
    }
}

pub fn open_folder(folder: &Path) {
    let (verb, path) = (wide(Path::new("open")), wide(folder));
    // SAFETY: NUL-terminated strings that outlive the call; no owner window, no parameters.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success; the rest are error codes.
    if result as isize <= 32 {
        log::warn!(
            "files: cannot open {}: ShellExecuteW returned {}",
            folder.display(),
            result as isize
        );
    }
}
