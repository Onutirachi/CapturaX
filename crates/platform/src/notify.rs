//! System notifications for device notices (FR-027, research R15): a Windows toast or a
//! freedesktop notification with at most one button, one at a time (a new one replaces the
//! previous), its button reported back through the callback given at init. The app shows its own
//! notice instead while its window has focus; this is for when it does not.

use std::sync::OnceLock;

use crate::os;

/// One system notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The text shown, already in the user's language ("C920 connected").
    pub title: String,
    /// The button's label, when the notice has one ("Open").
    pub action: Option<String>,
    /// Handed back to `on_action` when the button is pressed (the device id).
    pub tag: String,
}

/// The app icon as straight RGBA pixels, rows packed, `size` × `size`.
#[derive(Debug, Clone, Copy)]
pub struct NoticeIcon {
    pub size: u32,
    pub rgba: &'static [u8],
}

type OnAction = Box<dyn Fn(String) + Send + Sync>;

struct Setup {
    icon: NoticeIcon,
    on_action: OnAction,
}

static SETUP: OnceLock<Setup> = OnceLock::new();

/// Sets the icon notifications show and the callback their button calls, with the notice's tag,
/// from any thread. Call once at start, before any window: on Windows it also sets the process's
/// app id (`CapturaX`) the toasts are shown under. Later calls are ignored.
pub fn init_notifications(icon: NoticeIcon, on_action: impl Fn(String) + Send + Sync + 'static) {
    let setup = Setup {
        icon,
        on_action: Box::new(on_action),
    };
    if SETUP.set(setup).is_err() {
        log::warn!("notify: already initialized");
        return;
    }
    os::notify::init();
}

/// Shows `notice`, replacing the previous one. False when notifications are unavailable or off
/// for the app (logged once) or when `init_notifications` was not called.
pub fn notify(notice: &Notice) -> bool {
    let Some(setup) = SETUP.get() else {
        log::warn!("notify: not initialized");
        return false;
    };
    os::notify::show(notice, setup.icon)
}

/// Reports a pressed button; called by the OS side from its own thread.
pub(crate) fn report_action(tag: String) {
    log::info!("notify: action for {tag}");
    if let Some(setup) = SETUP.get() {
        (setup.on_action)(tag);
    }
}
