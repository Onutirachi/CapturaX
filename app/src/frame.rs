//! The frameless window's own frame (FR-006, FR-007, research R5), ported from the stack spike's
//! `src/window.rs`: where the pointer resizes the window, its cursor there, and what Esc does.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{CursorIcon, ResizeDirection};

/// Width of the resize band along the window's edges and corners, in logical pixels.
const RESIZE_BAND: f64 = 6.0;

/// Which edge or corner the pointer is on, if any. `resizable` is false while the window is
/// maximized or fullscreen: no band then.
pub fn resize_direction(
    size: PhysicalSize<u32>,
    scale: f64,
    position: PhysicalPosition<f64>,
    resizable: bool,
) -> Option<ResizeDirection> {
    if !resizable {
        return None;
    }
    let band = RESIZE_BAND * scale;
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    let (left, right) = (position.x < band, position.x >= width - band);
    let (top, bottom) = (position.y < band, position.y >= height - band);
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

/// The pointer's shape: the UI's own (`ui`, main.slint's `pointer-cursor`: 1 grab, 2 grabbing,
/// else none) and the edge or corner under the pointer (research R8). An edge's shape wins over
/// the grab hand, since a press there resizes; the closed hand wins over the edge, since a bubble
/// is being dragged and its release is not a resize.
pub fn cursor(direction: Option<ResizeDirection>, ui: i32) -> CursorIcon {
    match (direction, ui) {
        (_, 2) => CursorIcon::Grabbing,
        (Some(ResizeDirection::North | ResizeDirection::South), _) => CursorIcon::NsResize,
        (Some(ResizeDirection::East | ResizeDirection::West), _) => CursorIcon::EwResize,
        (Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast), _) => CursorIcon::NwseResize,
        (Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest), _) => CursorIcon::NeswResize,
        (None, 1) => CursorIcon::Grab,
        (None, _) => CursorIcon::Default,
    }
}

/// The part of the maximize button that the system treats as its own for the snap layouts flyout
/// (FR-011): the button's rectangle without the top resize band, so a press there still resizes.
/// `None` when the band covers all of it.
pub fn snap_button(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale: f64,
    resizable: bool,
) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    let top = if resizable {
        position.y.max((RESIZE_BAND * scale).ceil() as i32)
    } else {
        position.y
    };
    let bottom = position.y + size.height as i32;
    (size.width > 0 && bottom > top).then(|| {
        (
            PhysicalPosition::new(position.x, top),
            PhysicalSize::new(size.width, (bottom - top) as u32),
        )
    })
}

/// What one Esc press does (FR-007): it never quits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escape {
    /// A menu is open: Slint gets the key and the menu closes itself.
    ToSlint,
    CloseSettings,
    LeaveFullscreen,
    /// Nothing to close: the key still reaches Slint.
    Nothing,
}

pub fn escape(menu_open: bool, settings_open: bool, fullscreen: bool) -> Escape {
    if menu_open {
        Escape::ToSlint
    } else if settings_open {
        Escape::CloseSettings
    } else if fullscreen {
        Escape::LeaveFullscreen
    } else {
        Escape::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: PhysicalSize<u32> = PhysicalSize::new(1000, 600);

    fn at(x: f64, y: f64, scale: f64) -> Option<ResizeDirection> {
        resize_direction(SIZE, scale, PhysicalPosition::new(x, y), true)
    }

    #[test]
    fn edges_and_corners() {
        assert_eq!(at(0.0, 0.0, 1.0), Some(ResizeDirection::NorthWest));
        assert_eq!(at(999.0, 0.0, 1.0), Some(ResizeDirection::NorthEast));
        assert_eq!(at(0.0, 599.0, 1.0), Some(ResizeDirection::SouthWest));
        assert_eq!(at(999.0, 599.0, 1.0), Some(ResizeDirection::SouthEast));
        assert_eq!(at(3.0, 300.0, 1.0), Some(ResizeDirection::West));
        assert_eq!(at(995.0, 300.0, 1.0), Some(ResizeDirection::East));
        assert_eq!(at(500.0, 5.9, 1.0), Some(ResizeDirection::North));
        assert_eq!(at(500.0, 594.0, 1.0), Some(ResizeDirection::South));
        assert_eq!(at(500.0, 300.0, 1.0), None);
        assert_eq!(at(6.0, 300.0, 1.0), None);
    }

    #[test]
    fn band_follows_the_scale() {
        assert_eq!(at(8.0, 300.0, 1.0), None);
        assert_eq!(at(8.0, 300.0, 1.5), Some(ResizeDirection::West));
        assert_eq!(at(11.9, 300.0, 2.0), Some(ResizeDirection::West));
        assert_eq!(at(12.0, 300.0, 2.0), None);
    }

    #[test]
    fn no_band_when_not_resizable() {
        assert_eq!(
            resize_direction(SIZE, 1.0, PhysicalPosition::new(0.0, 0.0), false),
            None
        );
    }

    #[test]
    fn cursors() {
        assert_eq!(cursor(None, 0), CursorIcon::Default);
        assert_eq!(cursor(Some(ResizeDirection::North), 0), CursorIcon::NsResize);
        assert_eq!(cursor(Some(ResizeDirection::West), 0), CursorIcon::EwResize);
        assert_eq!(cursor(Some(ResizeDirection::SouthEast), 0), CursorIcon::NwseResize);
        assert_eq!(cursor(Some(ResizeDirection::NorthEast), 0), CursorIcon::NeswResize);
    }

    #[test]
    fn grab_cursors() {
        assert_eq!(cursor(None, 1), CursorIcon::Grab);
        assert_eq!(cursor(None, 2), CursorIcon::Grabbing);
        // An unknown value is no shape of the UI's own.
        assert_eq!(cursor(None, 7), CursorIcon::Default);
        // The edge wins over the open hand, the closed hand over the edge.
        assert_eq!(cursor(Some(ResizeDirection::South), 1), CursorIcon::NsResize);
        assert_eq!(cursor(Some(ResizeDirection::South), 2), CursorIcon::Grabbing);
    }

    #[test]
    fn snap_button_leaves_the_band() {
        let at = |y, height, scale, resizable| {
            snap_button(
                PhysicalPosition::new(900, y),
                PhysicalSize::new(44, height),
                scale,
                resizable,
            )
        };
        let rect = |y, height| Some((PhysicalPosition::new(900, y), PhysicalSize::new(44, height)));
        assert_eq!(at(4, 24, 1.0, true), rect(6, 22));
        assert_eq!(at(4, 24, 1.0, false), rect(4, 24));
        assert_eq!(at(6, 36, 1.5, true), rect(9, 33));
        assert_eq!(at(8, 48, 2.0, true), rect(12, 44));
        assert_eq!(at(0, 6, 1.0, true), None);
        assert_eq!(at(10, 20, 1.0, true), rect(10, 20));
    }

    #[test]
    fn escape_order() {
        assert_eq!(escape(true, true, true), Escape::ToSlint);
        assert_eq!(escape(false, true, true), Escape::CloseSettings);
        assert_eq!(escape(false, false, true), Escape::LeaveFullscreen);
        assert_eq!(escape(false, false, false), Escape::Nothing);
    }
}
