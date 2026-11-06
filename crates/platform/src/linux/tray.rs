//! Tray on Linux: a StatusNotifierItem through `ksni` (blocking API, its own thread, no GTK;
//! research R11).

use std::io;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};

use crate::tray::{Handler, IconImage, Labels, TrayEvent, argb};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";

/// The item `ksni` serves; read on its thread.
struct Item {
    labels: Labels,
    pixmaps: Vec<ksni::Icon>,
    handler: Handler,
}

/// An underscore marks an access key in StatusNotifierItem menus; texts show it as is.
fn menu_text(text: &str) -> String {
    text.replace('_', "__")
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        "capturax".into()
    }

    fn title(&self) -> String {
        self.labels.title.clone()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.pixmaps.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.labels.title.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        (self.handler)(TrayEvent::Open);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: menu_text(&self.labels.open),
                activate: Box::new(|item: &mut Self| (item.handler)(TrayEvent::Open)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: menu_text(&self.labels.quit),
                activate: Box::new(|item: &mut Self| (item.handler)(TrayEvent::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct Tray(Handle<Item>);

impl Tray {
    pub fn create(icons: &[IconImage<'_>], labels: &Labels, handler: Handler) -> io::Result<Self> {
        let pixmaps = icons
            .iter()
            .map(|i| ksni::Icon {
                width: i.size as i32,
                height: i.size as i32,
                data: argb(i.rgba),
            })
            .collect();
        let item = Item {
            labels: labels.clone(),
            pixmaps,
            handler,
        };
        item.spawn().map(Tray).map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn set_labels(&self, labels: &Labels) {
        let labels = labels.clone();
        self.0.update(move |item| item.labels = labels);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.0.shutdown().wait();
    }
}

pub fn available() -> bool {
    let has_owner = || -> zbus::Result<bool> {
        let connection = zbus::blocking::Connection::session()?;
        let bus = zbus::blocking::fdo::DBusProxy::new(&connection)?;
        Ok(bus.name_has_owner(WATCHER.try_into()?)?)
    };
    has_owner().unwrap_or_else(|e| {
        log::info!("tray: no session bus or watcher: {e}");
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underscores_show_as_typed() {
        assert_eq!(menu_text("Open"), "Open");
        assert_eq!(menu_text("snake_case"), "snake__case");
    }
}
