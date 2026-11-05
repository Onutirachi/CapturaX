//! No snap layouts flyout on Linux (FR-011): nothing to attach.

use winit::window::Window;

use crate::snap::{ButtonRect, SnapEvent};

pub struct Snap;

impl Snap {
    pub fn set_button(&self, _rect: Option<ButtonRect>) {}
}

pub fn attach(_window: &Window, _on_event: Box<dyn Fn(SnapEvent)>) -> Snap {
    Snap
}
