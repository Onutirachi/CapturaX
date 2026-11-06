//! Tray icon and menu (FR-014a, research R11): the icon, a menu with Open and Quit, and a click
//! that opens the window. Windows uses `tray-icon`, Linux `ksni` (StatusNotifierItem over D-Bus).
//!
//! On Windows the tray must be created on the thread that runs the event loop (the main thread),
//! after the loop exists; its events arrive there. On Linux `ksni` runs its own thread. Either way
//! `on_event` must be cheap: it sends a user event to the main loop.

use std::io;

use crate::os::tray as os;

/// What the user asked for from the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// A click on the icon, or the menu's Open: show the window.
    Open,
    /// The menu's Quit: run the quit sequence.
    Quit,
}

/// The tray's texts, read from `Texts` in the current language.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels {
    /// The tooltip, and the item's title on Linux.
    pub title: String,
    pub open: String,
    pub quit: String,
}

/// One size of the icon: straight (not premultiplied) RGBA, rows packed.
#[derive(Clone, Copy, Debug)]
pub struct IconImage<'a> {
    pub size: u32,
    pub rgba: &'a [u8],
}

/// Whether the desktop shows tray icons: always on Windows; on Linux when a StatusNotifierWatcher
/// owns its name on the session bus (GNOME without the extension has none). Without one the
/// Keep in tray setting is hidden and closing quits.
pub fn available() -> bool {
    os::available()
}

/// Shows the tray icon until the returned [`Tray`] drops. `icons` are the sizes the app has; each
/// platform takes the ones it needs.
pub fn create(
    icons: &[IconImage<'_>],
    labels: &Labels,
    on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
) -> io::Result<Tray> {
    os::Tray::create(icons, labels, Box::new(on_event)).map(Tray)
}

/// The tray icon; dropping it removes the icon (quit sequence).
pub struct Tray(os::Tray);

impl Tray {
    /// Changes the texts, for a language change.
    pub fn set_labels(&self, labels: &Labels) {
        self.0.set_labels(labels);
    }
}

/// The handler type the OS parts receive.
pub(crate) type Handler = Box<dyn Fn(TrayEvent) + Send + Sync>;

/// The smallest icon at least `size` pixels wide, else the largest.
#[cfg_attr(not(windows), allow(dead_code, reason = "used on Windows and in tests"))]
pub(crate) fn pick<'a>(icons: &[IconImage<'a>], size: u32) -> Option<IconImage<'a>> {
    icons
        .iter()
        .filter(|i| i.size >= size)
        .min_by_key(|i| i.size)
        .or_else(|| icons.iter().max_by_key(|i| i.size))
        .copied()
}

/// Straight RGBA to the ARGB32 in network byte order that StatusNotifierItem pixmaps use.
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "used on Linux and in tests"))]
pub(crate) fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, a]| [a, r, g, b])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icons() -> Vec<IconImage<'static>> {
        [16, 24, 32, 48]
            .iter()
            .map(|&size| IconImage { size, rgba: &[] })
            .collect()
    }

    #[test]
    fn pick_takes_the_smallest_large_enough() {
        let icons = icons();
        assert_eq!(pick(&icons, 16).unwrap().size, 16);
        assert_eq!(pick(&icons, 20).unwrap().size, 24);
        assert_eq!(pick(&icons, 32).unwrap().size, 32);
        assert_eq!(pick(&icons, 64).unwrap().size, 48);
        assert!(pick(&[], 16).is_none());
    }

    #[test]
    fn argb_moves_alpha_first() {
        assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
        assert!(argb(&[]).is_empty());
    }
}
