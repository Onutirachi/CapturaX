//! Global hotkeys on Linux: X11 grabs in an X11 session (research R9); elsewhere the keys work
//! only in the window (R8) until the GlobalShortcuts portal comes (T017, R7).

mod x11;

use crate::hotkeys::{Binding, HotkeyEvent, OnHotkeyEvent};

pub enum Hotkeys {
    X11(x11::X11),
    WindowOnly,
}

/// An X11 session: `XDG_SESSION_TYPE=x11`, or an X display without a Wayland one.
fn x11_session() -> bool {
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    match var("XDG_SESSION_TYPE").as_deref() {
        Some("x11") => true,
        Some("wayland") => false,
        _ => var("WAYLAND_DISPLAY").is_none() && var("DISPLAY").is_some(),
    }
}

impl Hotkeys {
    pub fn start(on_event: OnHotkeyEvent) -> Hotkeys {
        if x11_session() {
            match x11::X11::start(on_event.clone()) {
                Ok(x11) => return Hotkeys::X11(x11),
                Err(err) => log::warn!("hotkeys: X11 unavailable: {err}"),
            }
        }
        log::info!("hotkeys: no global keys here; the keys work while the window has the focus");
        on_event(HotkeyEvent::WindowOnly);
        Hotkeys::WindowOnly
    }

    pub fn set(&self, bindings: Vec<Binding>) {
        if let Hotkeys::X11(x11) = self {
            x11.set(bindings);
        }
    }

    pub fn suspend(&self, suspended: bool) {
        if let Hotkeys::X11(x11) = self {
            x11.suspend(suspended);
        }
    }

    pub fn configure(&self) -> bool {
        false
    }

    pub fn changed_by_desktop(&self) -> bool {
        false
    }
}
