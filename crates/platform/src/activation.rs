//! Bringing the window to the front (research R15, T077): on Wayland a window comes to the front
//! only with an XDG activation token from a user action, which the notification service and the
//! tray host hand over with their click. The last token is kept here, used once, while fresh.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use winit::window::Window;

/// A token older than this is not used: the click it came with is long gone.
const FRESH: Duration = Duration::from_secs(10);

static TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// Keeps the activation token a click came with; called by the OS side from its own thread.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn offer(token: String) {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some((token, Instant::now()));
}

/// The kept token, once, when it is fresh.
fn take() -> Option<String> {
    TOKEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .filter(|(_, at)| at.elapsed() < FRESH)
        .map(|(token, _)| token)
}

/// Brings `window` to the front where the platform allows it: with the token of the click that
/// asked for it on Wayland (which also restores a minimized window), else the toolkit's focus.
pub fn raise_window(window: &Window) {
    if let Some(token) = take() {
        match crate::os::activate(window, &token) {
            Ok(true) => {
                log::info!("window activated with the click's token");
                return;
            }
            Ok(false) => {}
            Err(e) => log::warn!("cannot activate the window with the click's token: {e}"),
        }
    }
    window.focus_window();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_used_once() {
        offer("abc".into());
        assert_eq!(take().as_deref(), Some("abc"));
        assert_eq!(take(), None);
    }
}
