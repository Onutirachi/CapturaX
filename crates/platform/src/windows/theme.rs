//! winit reports the system theme on Windows (see `crate::theme`).

use winit::window::{Theme, Window};

pub fn current(window: &Window) -> Option<Theme> {
    window.theme()
}

pub fn watch(_on_change: impl Fn(Option<Theme>) + Send + 'static) {}
