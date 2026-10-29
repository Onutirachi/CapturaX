//! Frameless window (T037): the UI draws its own title bar; the system still moves, resizes,
//! snaps, maximizes and minimizes the window.

use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowAttributes};

/// Width of the resize band along the window's edges, in logical pixels.
const BORDER: f64 = 6.0;

/// Smallest window where the Float direction fits without overlaps: the toolbar's three
/// dropdowns across, the right dock between the toolbar and the bottom bubbles. A compact layout
/// for smaller windows is left to the main screen's spec.
const MIN_SIZE: LogicalSize<f64> = LogicalSize::new(800.0, 520.0);

/// What the title bar asks the window to do.
#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    /// Start a system move (the pointer left the press point on the bar).
    Drag,
    Minimize,
    ToggleMaximize,
    Close,
}

/// The window's attributes: without the system frame when the UI has its own title bar.
pub fn attributes(frameless: bool) -> WindowAttributes {
    let attrs = Window::default_attributes()
        .with_title("CapturaX spike (layered)")
        .with_inner_size(LogicalSize::new(1280.0, 720.0))
        .with_min_inner_size(MIN_SIZE)
        .with_decorations(!frameless);
    #[cfg(windows)]
    let attrs = {
        use winit::platform::windows::WindowAttributesExtWindows;
        // Keeps the system shadow (and the rounded corners on Windows 11) without the frame.
        attrs.with_undecorated_shadow(true)
    };
    attrs
}

/// Which edge or corner the pointer is on, if the window can be resized from there.
pub fn resize_direction(window: &Window, position: PhysicalPosition<f64>) -> Option<ResizeDirection> {
    if window.is_maximized() || window.fullscreen().is_some() || !window.is_resizable() {
        return None;
    }
    let size = window.inner_size();
    let border = BORDER * window.scale_factor();
    let (left, right) = (position.x < border, position.x >= size.width as f64 - border);
    let (top, bottom) = (position.y < border, position.y >= size.height as f64 - border);
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}

/// The pointer shape over an edge or corner (the default elsewhere).
pub fn cursor(direction: Option<ResizeDirection>) -> CursorIcon {
    match direction {
        Some(ResizeDirection::North | ResizeDirection::South) => CursorIcon::NsResize,
        Some(ResizeDirection::East | ResizeDirection::West) => CursorIcon::EwResize,
        Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => CursorIcon::NwseResize,
        Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => CursorIcon::NeswResize,
        None => CursorIcon::Default,
    }
}

/// Does what the title bar asked; false when the window should close.
pub fn act(window: &Window, action: WindowAction) -> bool {
    match action {
        WindowAction::Drag => {
            // Dragging a maximized window restores it under the pointer (system behavior).
            if let Err(err) = window.drag_window() {
                eprintln!("window move: {err}");
            }
        }
        WindowAction::Minimize => window.set_minimized(true),
        WindowAction::ToggleMaximize => window.set_maximized(!window.is_maximized()),
        WindowAction::Close => return false,
    }
    true
}
