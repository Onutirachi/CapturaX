//! Windows 11 snap layouts flyout over the app's own maximize button (FR-011, research R5).
//!
//! Two things make the system show the flyout over a button it does not draw:
//! - a window subclass answers `WM_NCHITTEST` with `HTMAXBUTTON` inside the button's rectangle;
//! - the DWM frame is extended into the client area down to the button's bottom edge
//!   (`DwmExtendFrameIntoClientArea`). Without it the system takes the caption to be the 1 px strip
//!   winit leaves above the client for the shadow, and never opens the flyout below that strip.
//!   The video covers the extended frame with opaque pixels, so nothing of it shows.
//!
//! Once the frame is extended, the extension gives the window its shadow, so the subclass also
//! takes back winit's 1 px strip (`WM_NCCALCSIZE`: the client is the whole window while not
//! maximized). Left in place, the strip would show the desktop through the extended frame.
//!
//! Over the button the system then sends non-client messages: their hover, press and click go
//! back to the app as `SnapEvent`s. Presses are not passed on, so the system does not draw or track
//! a classic caption button; moves are, as they start the flyout and the system's own leave
//! tracking (`WM_NCMOUSELEAVE`).
//!
//! Also the snapped place a window returns to after fullscreen (FR-025, research R10).

use std::cell::Cell;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HTCLIENT, HTMAXBUTTON, IsWindowArranged, IsZoomed, NCCALCSIZE_PARAMS, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST,
    WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCRBUTTONDBLCLK,
    WM_NCRBUTTONDOWN, WM_NCRBUTTONUP,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::Rect;
use crate::snap::{ButtonRect, SnapEvent};

const SUBCLASS_ID: usize = 0x5A4B;

/// The button's rectangle, shared by the subclass and the app's handle.
type Button = Rc<Cell<Option<ButtonRect>>>;

/// How far the DWM frame is extended into the client, physical pixels; 0 while not extended.
type Extended = Rc<Cell<i32>>;

pub struct Snap {
    button: Button,
    extended: Extended,
    /// None when the subclass could not be set: then nothing is extended either.
    hwnd: Option<HWND>,
}

impl Snap {
    /// Without a button (fullscreen, a menu open) only the hit test stops: the frame stays
    /// extended, since it now gives the window its shadow.
    pub fn set_button(&self, rect: Option<ButtonRect>) {
        self.button.set(rect);
        let (Some(hwnd), Some((pos, size))) = (self.hwnd, rect) else {
            return;
        };
        let bottom = pos.y + size.height as i32;
        let before = self.extended.get();
        if bottom == before {
            return;
        }
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: bottom,
            cyBottomHeight: 0,
        };
        // SAFETY: the handle is the live window the subclass was set on (the app drops this
        // handle with the window); `margins` outlives the call.
        let result = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
        if result < 0 {
            log::warn!("snap layouts: frame not extended (0x{result:08x}), no flyout");
            return;
        }
        self.extended.set(bottom);
        if before == 0 {
            // Recomputes the client area without winit's strip (WM_NCCALCSIZE below).
            let flags = SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
            // SAFETY: as above; no window is moved, sized or reordered.
            unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, flags) };
        }
    }
}

struct State {
    button: Button,
    extended: Extended,
    on_event: Box<dyn Fn(SnapEvent)>,
    hover: bool,
    pressed: bool,
}

impl State {
    /// The pointer left the button: ends the hover and any press.
    fn leave(&mut self) {
        if self.pressed {
            self.pressed = false;
            (self.on_event)(SnapEvent::Press(false));
        }
        if self.hover {
            self.hover = false;
            (self.on_event)(SnapEvent::Hover(false));
        }
    }
}

pub fn attach(window: &Window, on_event: Box<dyn Fn(SnapEvent)>) -> Snap {
    let button = Button::default();
    let extended = Extended::default();
    let mut snap = Snap {
        button: button.clone(),
        extended: extended.clone(),
        hwnd: None,
    };
    let Ok(handle) = window.window_handle() else {
        return snap;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return snap;
    };
    let hwnd = handle.hwnd.get() as HWND;
    let state = Box::into_raw(Box::new(State {
        button,
        extended,
        on_event,
        hover: false,
        pressed: false,
    }));
    // SAFETY: the window handle is valid for the window's life; the state box is owned by the
    // subclass and freed in WM_NCDESTROY, or here if the subclass could not be set.
    let set = unsafe { SetWindowSubclass(hwnd, Some(proc), SUBCLASS_ID, state as usize) };
    if set == 0 {
        // SAFETY: the subclass was not set, so nothing else holds the pointer.
        drop(unsafe { Box::from_raw(state) });
        log::warn!("snap layouts: window subclass not set, no flyout on the maximize button");
    } else {
        snap.hwnd = Some(hwnd);
    }
    snap
}

/// The outer rectangle of a snapped window (research R10).
pub fn arranged_rect(window: &Window) -> Option<Rect> {
    let hwnd = super::hwnd(window)?;
    let mut r = RECT::default();
    // SAFETY: the handle is valid for the window's life; the out pointer is to a local.
    let snapped = unsafe { IsWindowArranged(hwnd) != 0 && GetWindowRect(hwnd, &mut r) != 0 };
    if !snapped {
        return None;
    }
    let size = PhysicalSize::new(
        u32::try_from(r.right - r.left).ok()?,
        u32::try_from(r.bottom - r.top).ok()?,
    );
    Some((PhysicalPosition::new(r.left, r.top), size))
}

pub fn set_outer_rect(window: &Window, (pos, size): Rect) {
    let Some(hwnd) = super::hwnd(window) else {
        return;
    };
    let (Ok(width), Ok(height)) = (i32::try_from(size.width), i32::try_from(size.height)) else {
        return;
    };
    // SAFETY: the handle is valid for the window's life; no z-order or activation change.
    let moved = unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            pos.x,
            pos.y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    if moved == 0 {
        log::warn!("window not moved back to its snapped place");
    }
}

/// The screen point packed in a non-client message's lparam (signed: monitors left of or above
/// the primary have negative coordinates).
fn screen_point(lparam: LPARAM) -> POINT {
    POINT {
        x: i32::from(lparam as u16 as i16),
        y: i32::from((lparam >> 16) as u16 as i16),
    }
}

fn inside(rect: ButtonRect, p: POINT) -> bool {
    let (pos, size) = rect;
    p.x >= pos.x && p.y >= pos.y && p.x < pos.x + size.width as i32 && p.y < pos.y + size.height as i32
}

unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` is the State box set in attach, alive until WM_NCDESTROY below; the window's
    // messages run on its own thread, and no reference to the state is held across
    // DefSubclassProc or across `on_event` (which only posts an event).
    let state = || unsafe { &mut *(data as *mut State) };
    let on_button = wparam == HTMAXBUTTON as usize;
    match message {
        WM_NCCALCSIZE if wparam != 0 && state().extended.get() > 0 => {
            let params = lparam as *mut NCCALCSIZE_PARAMS;
            // SAFETY: with wparam set, lparam points to a writable NCCALCSIZE_PARAMS for the
            // whole message; no reference to it is held across winit's handling.
            let proposed = unsafe { (*params).rgrc[0] };
            // SAFETY: winit's handling first (maximized: the work area), then its strip undone.
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            // SAFETY: plain query on this window; the write goes to the same params as above.
            if unsafe { IsZoomed(hwnd) } == 0 {
                unsafe { (*params).rgrc[0] = proposed };
            }
            return result;
        }
        WM_NCHITTEST => {
            // SAFETY: winit's answer first; only its client area can become the button.
            let hit = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            if hit == HTCLIENT as LRESULT
                && let Some(rect) = state().button.get()
            {
                let mut p = screen_point(lparam);
                // SAFETY: the handle is this window's; `p` is a local.
                if unsafe { ScreenToClient(hwnd, &mut p) } != 0 && inside(rect, p) {
                    return HTMAXBUTTON as LRESULT;
                }
            }
            return hit;
        }
        WM_NCMOUSEMOVE => {
            // SAFETY: the system's handling first: it starts the flyout and the leave tracking.
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            let s = state();
            if on_button && !s.hover {
                s.hover = true;
                (s.on_event)(SnapEvent::Hover(true));
            } else if !on_button {
                s.leave();
            }
            return result;
        }
        WM_NCMOUSELEAVE => state().leave(),
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if on_button => {
            let s = state();
            s.pressed = true;
            (s.on_event)(SnapEvent::Press(true));
            return 0;
        }
        WM_NCLBUTTONUP if on_button => {
            let s = state();
            if s.pressed {
                s.pressed = false;
                (s.on_event)(SnapEvent::Press(false));
                (s.on_event)(SnapEvent::Click);
            }
            return 0;
        }
        // No system menu from a right click on the button.
        WM_NCRBUTTONDOWN | WM_NCRBUTTONUP | WM_NCRBUTTONDBLCLK if on_button => return 0,
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
