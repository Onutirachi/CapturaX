//! Keeps a dragged window on the system's suggested rectangle when it changes monitor scale
//! (research R5, "Dragging across monitor scales").
//!
//! During a drag, winit answers `WM_DPICHANGED` with a position of its own. That position can put
//! the window back on the old monitor, which starts a nested scale change. After it the system's
//! move loop keeps a wrong size. A window subclass rewrites the `WM_WINDOWPOSCHANGING` that winit
//! sends while handling `WM_DPICHANGED` inside a move or resize loop, so that the window takes the
//! suggested rectangle.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SWP_NOMOVE, SWP_NOSIZE, WINDOWPOS, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCDESTROY,
    WM_WINDOWPOSCHANGING,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

const SUBCLASS_ID: usize = 0xC0D1;

#[derive(Default)]
struct State {
    /// Between `WM_ENTERSIZEMOVE` and `WM_EXITSIZEMOVE`.
    in_loop: bool,
    /// The suggested rectangle while a `WM_DPICHANGED` inside the loop is being handled.
    suggested: Option<RECT>,
}

pub fn follow_dpi_rect(window: &Window) {
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let state = Box::into_raw(Box::<State>::default());
    // SAFETY: the window handle is valid for the window's life; the state box is owned by the
    // subclass and freed in WM_NCDESTROY, or here if the subclass could not be set.
    let set = unsafe { SetWindowSubclass(handle.hwnd.get() as HWND, Some(proc), SUBCLASS_ID, state as usize) };
    if set == 0 {
        // SAFETY: the subclass was not set, so nothing else holds the pointer.
        drop(unsafe { Box::from_raw(state) });
        log::warn!("window subclass: not set, a drag across monitor scales may resize the window");
    }
}

unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` is the State box set in follow_dpi_rect, alive until WM_NCDESTROY below; the
    // window's messages run on its own thread, one at a time apart from nested sends, and no
    // reference to the state is held across DefSubclassProc.
    let state = || unsafe { &mut *(data as *mut State) };
    match message {
        WM_ENTERSIZEMOVE => state().in_loop = true,
        WM_EXITSIZEMOVE => state().in_loop = false,
        WM_DPICHANGED if state().in_loop => {
            // SAFETY: for WM_DPICHANGED, lparam points to the suggested RECT.
            let suggested = unsafe { *(lparam as *const RECT) };
            let previous = state().suggested.replace(suggested);
            // SAFETY: forwards the message unchanged to winit's window procedure.
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            state().suggested = previous;
            return result;
        }
        WM_WINDOWPOSCHANGING => {
            if let Some(rect) = state().suggested {
                // SAFETY: for WM_WINDOWPOSCHANGING, lparam points to a writable WINDOWPOS.
                let pos = unsafe { &mut *(lparam as *mut WINDOWPOS) };
                if pos.flags & (SWP_NOMOVE | SWP_NOSIZE) == 0 {
                    pos.x = rect.left;
                    pos.y = rect.top;
                    pos.cx = rect.right - rect.left;
                    pos.cy = rect.bottom - rect.top;
                }
            }
        }
        WM_NCDESTROY => {
            // SAFETY: removes this subclass, then frees the state it owned; nothing uses it after.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(proc), id);
                drop(Box::from_raw(data as *mut State));
                return DefSubclassProc(hwnd, message, wparam, lparam);
            }
        }
        _ => {}
    }
    // SAFETY: forwards every other message unchanged.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
