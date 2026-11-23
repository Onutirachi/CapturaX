//! The file manager and the folder dialog on Windows (research R12): Explorer with the file
//! selected through `SHOpenFolderAndSelectItems`, a folder through `ShellExecuteW("open")`, the
//! folder dialog through `IFileOpenDialog` with `FOS_PICKFOLDERS`.

use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, ILCreateFromPathW, ILFree, IShellItem,
    SHCreateItemFromParsingName, SHOpenFolderAndSelectItems, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, PCWSTR};
use winit::window::Window;

use crate::files::FolderPicked;
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

/// The dialog runs on its own STA thread, modal to the main window (which it disables while
/// open); the event loop keeps running. Always true: Windows always has the dialog.
pub fn pick_folder(owner: &Window, start: &Path, done: FolderPicked) -> bool {
    // A window handle is a plain value; it crosses threads as an integer.
    let owner = super::hwnd(owner).map_or(0, |hwnd| hwnd as isize);
    let start = start.to_path_buf();
    let spawned = std::thread::Builder::new().name("folder dialog".into()).spawn(move || {
        let picked = match show_folder_dialog(HWND(owner as *mut _), &start) {
            Ok(picked) => picked,
            Err(err) => {
                log::warn!("files: folder dialog failed: {err}");
                None
            }
        };
        done(picked);
    });
    if let Err(err) = spawned {
        log::warn!("files: cannot start the folder dialog thread: {err}");
    }
    true
}

/// The folder chosen, None when cancelled.
fn show_folder_dialog(owner: HWND, start: &Path) -> windows::core::Result<Option<PathBuf>> {
    // SAFETY: COM is set up on this thread for the dialog's life and torn down after its last
    // interface is released (the inner function returns them all); `start` is NUL-terminated
    // and outlives the call; the path the dialog hands out is freed once, after it is copied.
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok();
        let picked = (|| {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)?;
            // A start folder that no longer exists leaves the dialog where it last was.
            let start = wide(start);
            if let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(start.as_ptr()), None) {
                dialog.SetFolder(&folder)?;
            }
            match dialog.Show(Some(owner)) {
                Ok(()) => {}
                Err(err) if err.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => return Ok(None),
                Err(err) => return Err(err),
            }
            let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = PathBuf::from(std::ffi::OsString::from_wide(name.as_wide()));
            CoTaskMemFree(Some(name.0 as *const _));
            Ok(Some(path))
        })();
        if com {
            CoUninitialize();
        }
        picked
    }
}
