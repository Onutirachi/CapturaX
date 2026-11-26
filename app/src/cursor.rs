//! The app's own grab hands where the system has none (preview extras FR-015, research R8).
//!
//! Windows has no grab cursor (winit shows the four-way arrow for `Grab` and `Grabbing`), so the
//! maintainer's drawings (app/ui/assets/cursors) are used there, as browsers do. build.rs
//! rasterizes them for each size in `SIZES`; the drawing's square takes 22 of every 32 pixels.

use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, CustomCursor};

/// The cursor sides in pixels, one per common display scale (100 % to 300 %). Keep in step with
/// build.rs `CURSOR_SIZES`.
const SIZES: [u32; 8] = [32, 40, 48, 56, 64, 72, 80, 96];

macro_rules! images {
    ($kind:literal) => {
        [
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-32.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-40.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-48.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-56.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-64.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-72.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-80.rgba")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/cursor-", $kind, "-96.rgba")).as_slice(),
        ]
    };
}

const GRAB: [&[u8]; 8] = images!("grab");
const GRABBING: [&[u8]; 8] = images!("grabbing");

/// The click point in a 32 px cursor: the open palm's middle, (0.5625, 0.625) of the drawing's
/// 22 px square, which starts 5 px in.
const HOTSPOT_32: (f64, f64) = (5.0 + 0.5625 * 22.0, 5.0 + 0.625 * 22.0);

/// The index in `SIZES` for a display scale: the size nearest to 32 × scale.
fn size_index(scale: f64) -> usize {
    let want = 32.0 * scale;
    (0..SIZES.len())
        .min_by(|&a, &b| {
            (f64::from(SIZES[a]) - want)
                .abs()
                .total_cmp(&(f64::from(SIZES[b]) - want).abs())
        })
        .unwrap_or(0)
}

/// The click point for a cursor of `size` pixels.
fn hotspot(size: u32) -> (u16, u16) {
    let k = f64::from(size) / 32.0;
    ((HOTSPOT_32.0 * k).round() as u16, (HOTSPOT_32.1 * k).round() as u16)
}

/// The two hands for one display scale.
pub struct GrabCursors {
    scale: f64,
    grab: CustomCursor,
    grabbing: CustomCursor,
}

impl GrabCursors {
    /// The hands for `scale`; None where the system has its own (`platform::has_grab_cursors`)
    /// or an image is refused (logged).
    pub fn new(event_loop: &ActiveEventLoop, scale: f64) -> Option<Self> {
        if platform::has_grab_cursors() {
            return None;
        }
        let i = size_index(scale);
        let size = SIZES[i];
        let side = size as u16;
        let (x, y) = hotspot(size);
        let make = |rgba: &[u8]| match CustomCursor::from_rgba(rgba.to_vec(), side, side, x, y) {
            Ok(source) => Some(event_loop.create_custom_cursor(source)),
            Err(err) => {
                log::warn!("grab cursor {size} px: {err}");
                None
            }
        };
        Some(Self {
            scale,
            grab: make(GRAB[i])?,
            grabbing: make(GRABBING[i])?,
        })
    }

    /// The display scale they were made for.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The app's own image for `icon`, None for a shape the system draws.
    pub fn pick(&self, icon: CursorIcon) -> Option<CustomCursor> {
        match icon {
            CursorIcon::Grab => Some(self.grab.clone()),
            CursorIcon::Grabbing => Some(self.grabbing.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_scale() {
        assert_eq!(SIZES[size_index(1.0)], 32);
        assert_eq!(SIZES[size_index(1.25)], 40);
        assert_eq!(SIZES[size_index(1.5)], 48);
        assert_eq!(SIZES[size_index(1.75)], 56);
        assert_eq!(SIZES[size_index(2.0)], 64);
        assert_eq!(SIZES[size_index(3.0)], 96);
        // Between steps the nearest; past the ends the end.
        assert_eq!(SIZES[size_index(1.1)], 32);
        assert_eq!(SIZES[size_index(0.5)], 32);
        assert_eq!(SIZES[size_index(4.0)], 96);
    }

    #[test]
    fn hotspot_stays_on_the_palm() {
        assert_eq!(hotspot(32), (17, 19));
        assert_eq!(hotspot(64), (35, 38));
    }

    #[test]
    fn images_are_square_rgba() {
        for (i, size) in SIZES.iter().enumerate() {
            let len = (size * size * 4) as usize;
            assert_eq!(GRAB[i].len(), len);
            assert_eq!(GRABBING[i].len(), len);
            // Not blank: the drawing reached the image.
            assert!(GRAB[i].chunks(4).any(|p| p[3] == 255));
            assert!(GRABBING[i].chunks(4).any(|p| p[3] == 255));
        }
    }
}
