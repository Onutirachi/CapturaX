//! The maximize button's part in Windows 11's snap layouts flyout (FR-011, research R5, best
//! effort). The system shows the flyout over a window's maximize button only when the window
//! answers its hit test with `HTMAXBUTTON` there. The pointer over the button is then outside the
//! client area, so the app no longer sees it: the button's hover, press and click come back as
//! [`SnapEvent`]s. Nothing to do on Linux.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::Window;

use crate::os;

/// What the pointer does on the maximize button while the system owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapEvent {
    /// The pointer entered (true) or left (false) the button.
    Hover(bool),
    /// The left button went down (true) on the button, or the press ended (false).
    Press(bool),
    /// Pressed and released on the button: maximize or restore.
    Click,
}

/// The maximize button's rectangle in physical pixels, relative to the window's client area.
pub type ButtonRect = (PhysicalPosition<i32>, PhysicalSize<u32>);

/// The link between a window and the system's flyout; the window keeps it until it is destroyed.
pub struct Snap(os::snap::Snap);

/// Starts answering the system's hit test for `window`. `on_event` runs on the window's thread,
/// inside its message handling: send the event on, do not touch the window from it.
pub fn attach(window: &Window, on_event: impl Fn(SnapEvent) + 'static) -> Snap {
    Snap(os::snap::attach(window, Box::new(on_event)))
}

impl Snap {
    /// Where the maximize button is now; None while there is none to point at (fullscreen, a menu
    /// open that takes every click).
    pub fn set_button(&self, rect: Option<ButtonRect>) {
        self.0.set_button(rect);
    }
}
