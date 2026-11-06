//! The system theme from the XDG Settings portal (see `crate::theme`).

use std::time::Duration;

use winit::window::{Theme, Window};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

/// A missing portal may be started by the bus first: the window waits for this at most.
const TIMEOUT: Duration = Duration::from_millis(500);

pub fn current(_window: &Window) -> Option<Theme> {
    let read = || -> zbus::Result<Option<Theme>> {
        let connection = zbus::blocking::connection::Builder::session()?
            .method_timeout(TIMEOUT)
            .build()?;
        let proxy = settings(&connection)?;
        // `ReadOne` (portal version 2) answers the value; the older `Read` wraps it once more.
        let value: OwnedValue = proxy
            .call("ReadOne", &(NAMESPACE, KEY))
            .or_else(|_| proxy.call("Read", &(NAMESPACE, KEY)))?;
        Ok(scheme(&value))
    };
    read().unwrap_or_else(|e| {
        log::info!("theme: no settings portal: {e}");
        None
    })
}

pub fn watch(on_change: impl Fn(Option<Theme>) + Send + 'static) {
    let run = move || -> zbus::Result<()> {
        let connection = Connection::session()?;
        let proxy = settings(&connection)?;
        for message in proxy.receive_signal("SettingChanged")? {
            let (namespace, key, value): (String, String, OwnedValue) = message.body().deserialize()?;
            if namespace == NAMESPACE && key == KEY {
                on_change(scheme(&value));
            }
        }
        Ok(())
    };
    let spawned = std::thread::Builder::new().name("theme watch".into()).spawn(move || {
        if let Err(e) = run() {
            log::info!("theme: not following the settings portal: {e}");
        }
    });
    if let Err(e) = spawned {
        log::warn!("theme: cannot start the watch: {e}");
    }
}

fn settings(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(connection, DESTINATION, PATH, INTERFACE)
}

/// `color-scheme`: 1 prefers dark, 2 prefers light, 0 (no preference) or anything else says
/// nothing.
fn scheme(value: &Value<'_>) -> Option<Theme> {
    match value {
        Value::Value(inner) => scheme(inner),
        Value::U32(1) => Some(Theme::Dark),
        Value::U32(2) => Some(Theme::Light),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_color_scheme() {
        assert_eq!(scheme(&Value::U32(1)), Some(Theme::Dark));
        assert_eq!(scheme(&Value::U32(2)), Some(Theme::Light));
        assert_eq!(scheme(&Value::U32(0)), None);
        assert_eq!(scheme(&Value::U32(7)), None);
        assert_eq!(scheme(&Value::from("dark")), None);
    }

    #[test]
    fn unwraps_the_older_read_answer() {
        assert_eq!(scheme(&Value::Value(Box::new(Value::U32(2)))), Some(Theme::Light));
    }
}
