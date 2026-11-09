//! Freedesktop notifications over D-Bus (research R15): `Notify` with the icon's pixels, the
//! previous notice closed first so one shows at a time; the button comes back as the
//! `ActionInvoked` signal, read on a thread.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type;
use zbus::zvariant::Value;
use zbus::{MatchRule, Message};

use crate::notify::{Notice, NoticeIcon};

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const APP_NAME: &str = "CapturaX";
/// The `.desktop` file's name the packaged app installs (research R12).
const DESKTOP_ENTRY: &str = "capturax";
/// The action key of the button.
const OPEN: &str = "open";
/// A missing notification service may be started by the bus first: wait for it at most this.
const TIMEOUT: Duration = Duration::from_secs(2);

/// The session bus, connected on the first notice.
static BUS: Mutex<Option<Connection>> = Mutex::new(None);
/// The notice on screen: its id from the server and its tag.
static SHOWN: Mutex<Option<(u32, String)>> = Mutex::new(None);
/// Set once a reason for not showing notifications was logged.
static LOGGED: AtomicBool = AtomicBool::new(false);

fn log_once(why: &zbus::Error) {
    if !LOGGED.swap(true, Ordering::Relaxed) {
        log::warn!("notify: no system notifications: {why}");
    }
}

pub fn init() {}

pub fn show(notice: &Notice, icon: NoticeIcon) -> bool {
    let mut bus = BUS.lock().unwrap_or_else(|e| e.into_inner());
    if bus.is_none() {
        match connect() {
            Ok(connection) => *bus = Some(connection),
            Err(e) => {
                log_once(&e);
                return false;
            }
        }
    }
    let Some(connection) = bus.as_ref() else {
        return false;
    };
    let proxy = match Proxy::new(connection, DESTINATION, PATH, INTERFACE) {
        Ok(proxy) => proxy,
        Err(e) => {
            log_once(&e);
            return false;
        }
    };
    // Closed, not replaced: KDE Plasma updates a notice already gone from the screen in its
    // history, with no popup (T061).
    let previous = lock_shown().take();
    if let Some((id, _)) = previous
        && let Err(e) = proxy.call::<_, _, ()>("CloseNotification", &(id,))
    {
        log::debug!("notify: cannot close notice {id}: {e}");
    }
    match proxy.call::<_, _, u32>("Notify", &arguments(notice, icon)) {
        Ok(id) => {
            *lock_shown() = Some((id, notice.tag.clone()));
            true
        }
        Err(e) => {
            log_once(&e);
            false
        }
    }
}

fn lock_shown() -> std::sync::MutexGuard<'static, Option<(u32, String)>> {
    SHOWN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Connects to the session bus and starts reading the service's signals.
fn connect() -> zbus::Result<Connection> {
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(INTERFACE)?
        .path(PATH)?
        .build();
    let signals = MessageIterator::for_match_rule(rule, &connection, None)?;
    std::thread::Builder::new()
        .name("notify signals".into())
        .spawn(move || {
            for message in signals {
                match message {
                    Ok(message) => signal(&message),
                    Err(e) => log::debug!("notify: bad signal: {e}"),
                }
            }
        })
        .map_err(|e| zbus::Error::Failure(format!("cannot start the signal thread: {e}")))?;
    Ok(connection)
}

/// `ActionInvoked` on the shown notice's button reports its tag; `NotificationClosed` forgets it.
fn signal(message: &Message) {
    let header = message.header();
    let Some(member) = header.member() else {
        return;
    };
    match member.as_str() {
        "ActionInvoked" => {
            let Ok((id, key)) = message.body().deserialize::<(u32, String)>() else {
                return;
            };
            let tag = lock_shown()
                .as_ref()
                .filter(|(shown, _)| *shown == id)
                .map(|(_, tag)| tag.clone());
            if let (Some(tag), OPEN) = (tag, key.as_str()) {
                crate::notify::report_action(tag);
            }
        }
        "NotificationClosed" => {
            if let Ok((id, _reason)) = message.body().deserialize::<(u32, u32)>() {
                let mut shown = lock_shown();
                if shown.as_ref().is_some_and(|(shown, _)| *shown == id) {
                    *shown = None;
                }
            }
        }
        _ => {}
    }
}

type NotifyArgs<'a> = (
    &'a str,
    u32,
    &'a str,
    &'a str,
    &'a str,
    Vec<&'a str>,
    HashMap<&'a str, Value<'a>>,
    i32,
);

/// `Notify`'s arguments: app name, replaced id (none: always a new notice), app icon, summary,
/// body, actions (key, label pairs), hints, expiry (-1: the server's default).
fn arguments(notice: &Notice, icon: NoticeIcon) -> NotifyArgs<'_> {
    let actions = match &notice.action {
        Some(label) => vec![OPEN, label.as_str()],
        None => Vec::new(),
    };
    let side = icon.size as i32;
    // (width, height, rowstride, has alpha, bits per sample, channels, data)
    let image = Value::from((side, side, side * 4, true, 8i32, 4i32, icon.rgba));
    let hints = HashMap::from([("image-data", image), ("desktop-entry", Value::from(DESKTOP_ENTRY))]);
    (APP_NAME, 0, "", notice.title.as_str(), "", actions, hints, -1)
}

#[cfg(test)]
mod tests {
    use super::*;

    static RGBA: [u8; 16] = [7; 16];
    const ICON: NoticeIcon = NoticeIcon { size: 2, rgba: &RGBA };

    fn notice(action: Option<&str>) -> Notice {
        Notice {
            title: "C920 connected".into(),
            action: action.map(String::from),
            tag: "usb:1234".into(),
        }
    }

    #[test]
    fn notice_with_a_button() {
        let notice = notice(Some("Abrir"));
        let (app, replaces, app_icon, summary, body, actions, hints, expiry) = arguments(&notice, ICON);
        assert_eq!(
            (app, replaces, app_icon, summary, body, expiry),
            ("CapturaX", 0, "", "C920 connected", "", -1)
        );
        assert_eq!(actions, ["open", "Abrir"]);
        assert_eq!(hints["desktop-entry"], Value::from("capturax"));
        assert_eq!(hints["image-data"].value_signature().to_string(), "(iiibiiay)");
        assert_eq!(
            hints["image-data"],
            Value::from((2i32, 2i32, 8i32, true, 8i32, 4i32, &RGBA[..]))
        );
    }

    #[test]
    fn notice_without_a_button() {
        let notice = notice(None);
        let (_, _, _, _, _, actions, _, _) = arguments(&notice, ICON);
        assert!(actions.is_empty());
    }
}
