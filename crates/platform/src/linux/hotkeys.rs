//! Global hotkeys on Linux: X11 grabs in an X11 session (research R9); under Wayland the desktop
//! portal's GlobalShortcuts (R7), which reports `WindowOnly` itself when it cannot work; elsewhere
//! the keys work only in the window (R8).

mod portal;
mod x11;

use crate::hotkeys::{Binding, HotkeyEvent, OnHotkeyEvent};

pub enum Hotkeys {
    X11(x11::X11),
    Portal(portal::Portal),
    WindowOnly,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// An X11 session: `XDG_SESSION_TYPE=x11`, or an X display without a Wayland one.
fn x11_session() -> bool {
    match var("XDG_SESSION_TYPE").as_deref() {
        Some("x11") => true,
        Some("wayland") => false,
        _ => var("WAYLAND_DISPLAY").is_none() && var("DISPLAY").is_some(),
    }
}

/// A Wayland session: `XDG_SESSION_TYPE=wayland`, or a Wayland display.
fn wayland_session() -> bool {
    match var("XDG_SESSION_TYPE").as_deref() {
        Some("wayland") => true,
        Some("x11") => false,
        _ => var("WAYLAND_DISPLAY").is_some(),
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
        if wayland_session() {
            return Hotkeys::Portal(portal::Portal::start(on_event));
        }
        log::info!("hotkeys: no global keys here; the keys work while the window has the focus");
        on_event(HotkeyEvent::WindowOnly);
        Hotkeys::WindowOnly
    }

    pub fn set(&self, bindings: Vec<Binding>) {
        match self {
            Hotkeys::X11(x11) => x11.set(bindings),
            Hotkeys::Portal(portal) => portal.set(bindings),
            Hotkeys::WindowOnly => {}
        }
    }

    pub fn suspend(&self, suspended: bool) {
        match self {
            Hotkeys::X11(x11) => x11.suspend(suspended),
            Hotkeys::Portal(portal) => portal.suspend(suspended),
            Hotkeys::WindowOnly => {}
        }
    }

    pub fn configure(&self) -> bool {
        match self {
            Hotkeys::Portal(portal) => portal.configure(),
            _ => false,
        }
    }

    pub fn changed_by_desktop(&self) -> bool {
        match self {
            Hotkeys::Portal(portal) => portal.changed_by_desktop(),
            _ => false,
        }
    }
}
