//! The app icon (`brand/X.svg`), rasterized by build.rs at the window and tray sizes.

/// One icon size: straight RGBA, rows packed.
pub struct Icon {
    pub size: u32,
    pub rgba: &'static [u8],
}

macro_rules! icon {
    ($size:literal) => {
        Icon {
            size: $size,
            rgba: include_bytes!(concat!(env!("OUT_DIR"), "/icon-", $size, ".rgba")),
        }
    };
}

pub const ICONS: [Icon; 7] = [
    icon!(16),
    icon!(24),
    icon!(32),
    icon!(48),
    icon!(64),
    icon!(128),
    icon!(256),
];

/// The smallest icon at least `size` pixels wide (the largest if none is).
pub fn at_least(size: u32) -> &'static Icon {
    ICONS.iter().find(|i| i.size >= size).unwrap_or(&ICONS[ICONS.len() - 1])
}

/// The icon for the window and taskbar.
pub fn window_icon() -> Option<winit::window::Icon> {
    let icon = at_least(256);
    winit::window::Icon::from_rgba(icon.rgba.to_vec(), icon.size, icon.size).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_is_complete_and_not_empty() {
        for icon in &ICONS {
            assert_eq!(icon.rgba.len(), (icon.size * icon.size * 4) as usize);
            assert!(icon.rgba.chunks(4).any(|p| p[3] > 0), "icon {} is empty", icon.size);
        }
        assert_eq!(at_least(20).size, 24);
        assert_eq!(at_least(1000).size, 256);
    }
}
