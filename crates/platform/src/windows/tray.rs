//! Tray on Windows: `tray-icon` (Shell_NotifyIcon) with a `muda` menu, driven by the winit
//! loop's message pump (research R11).

use std::io;
use std::sync::{Arc, Mutex, Once, PoisonError};

use tray_icon::menu::{MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSMICON;

use crate::tray::{Handler, IconImage, Labels, TrayEvent, pick};

const OPEN: &str = "open";
const QUIT: &str = "quit";

/// Where the library's handlers forward to. `tray-icon` and `muda` keep their handlers in
/// set-once cells, so they are installed once and this slot follows the current tray.
static HANDLER: Mutex<Option<Arc<Handler>>> = Mutex::new(None);
static INSTALL: Once = Once::new();

fn send(event: TrayEvent) {
    let handler = HANDLER.lock().unwrap_or_else(PoisonError::into_inner).clone();
    if let Some(handler) = handler {
        handler(event);
    }
}

fn install_handlers() {
    INSTALL.call_once(|| {
        TrayIconEvent::set_event_handler(Some(|event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                send(TrayEvent::Open);
            }
        }));
        MenuEvent::set_event_handler(Some(|event: MenuEvent| match event.id.as_ref() {
            OPEN => send(TrayEvent::Open),
            QUIT => send(TrayEvent::Quit),
            _ => {}
        }));
    });
}

/// `&` marks an access key in Windows menus; texts show it as is.
fn menu_text(text: &str) -> String {
    text.replace('&', "&&")
}

fn other(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

pub struct Tray {
    icon: TrayIcon,
    open: MenuItem,
    quit: MenuItem,
}

impl Tray {
    pub fn create(icons: &[IconImage<'_>], labels: &Labels, handler: Handler) -> io::Result<Self> {
        // The notification area's icon size at the system's scale.
        // SAFETY: plain queries with no pointers.
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
        let image = pick(icons, u32::try_from(size).unwrap_or(16)).ok_or_else(|| other("no icon"))?;
        let icon = tray_icon::Icon::from_rgba(image.rgba.to_vec(), image.size, image.size).map_err(other)?;

        let open = MenuItem::with_id(OPEN, menu_text(&labels.open), true, None);
        let quit = MenuItem::with_id(QUIT, menu_text(&labels.quit), true, None);
        let menu =
            tray_icon::menu::Menu::with_items(&[&open, &PredefinedMenuItem::separator(), &quit]).map_err(other)?;

        install_handlers();
        *HANDLER.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(handler));
        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(&labels.title)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(other)?;
        Ok(Self { icon, open, quit })
    }

    pub fn set_labels(&self, labels: &Labels) {
        self.open.set_text(menu_text(&labels.open));
        self.quit.set_text(menu_text(&labels.quit));
        if let Err(e) = self.icon.set_tooltip(Some(&labels.title)) {
            log::warn!("tray tooltip: {e}");
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // The icon itself goes when `icon` drops; no event reaches the old handler after this.
        *HANDLER.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

pub fn available() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ampersands_show_as_typed() {
        assert_eq!(menu_text("Open"), "Open");
        assert_eq!(menu_text("Save & quit"), "Save && quit");
    }
}
