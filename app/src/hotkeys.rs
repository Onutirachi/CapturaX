//! The app's hotkeys (screenshot spec FR-012–FR-015, data-model.md "Hotkey binding", research
//! R8): the bindings read from the settings, the platform's `hotkeys` thread that takes them, and
//! where global keys cannot work (Wayland without the portal) the window's own key events matched
//! against them while it has the focus.

use platform::{Hotkey, HotkeyEvent, Key, Modifier, Modifiers};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

/// The screenshot action (FR-013).
pub const SCREENSHOT: &str = "screenshot";

/// Every action with its default key, in the settings table's order: the first of two actions
/// with the same key keeps it. Specs #9 and #10 add `record` and `live` here (FR-013).
const ACTIONS: [(&str, &str); 1] = [(SCREENSHOT, settings::DEFAULT_SCREENSHOT_KEY)];

/// The action's key text in the settings (`""`: no key).
fn setting<'a>(hotkeys: &'a settings::Hotkeys, action: &str) -> &'a str {
    match action {
        SCREENSHOT => &hotkeys.screenshot,
        _ => "",
    }
}

/// Each action's key from the settings (contracts/settings-file.md rules 2 and 3): `""` is no key;
/// an invalid or not allowed text takes the default; a key an earlier action already has reads as
/// no key. Both are logged.
pub fn read(hotkeys: &settings::Hotkeys) -> Vec<(&'static str, Option<Hotkey>)> {
    resolve(
        ACTIONS
            .iter()
            .map(|&(action, default)| (action, default, setting(hotkeys, action))),
    )
}

/// The reading rules over `(action, default, text)` in the table's order.
fn resolve<'a>(
    actions: impl IntoIterator<Item = (&'static str, &'a str, &'a str)>,
) -> Vec<(&'static str, Option<Hotkey>)> {
    let mut bindings: Vec<(&'static str, Option<Hotkey>)> = Vec::new();
    for (action, default, text) in actions {
        let key = if text.trim().is_empty() {
            None
        } else if let Some(key) = Hotkey::parse(text).filter(Hotkey::allowed) {
            Some(key)
        } else {
            log::warn!("hotkeys: {action} key \"{text}\" is not valid, using {default}");
            Hotkey::parse(default)
        };
        let key = key.filter(|key| match bindings.iter().find(|(_, other)| *other == Some(*key)) {
            Some((other, _)) => {
                log::warn!("hotkeys: {key} is already {other}'s key, {action} has none");
                false
            }
            None => true,
        });
        bindings.push((action, key));
    }
    bindings
}

/// The hotkey a key press in the window makes, from the modifiers held, the key as the layout
/// types it without modifiers, and the physical key (digits on layouts where they need Shift,
/// such as AZERTY). None for a key that cannot be a hotkey (a modifier alone, a symbol).
pub fn from_window(modifiers: ModifiersState, key: &winit::keyboard::Key, physical: PhysicalKey) -> Option<Hotkey> {
    use winit::keyboard::Key as WinitKey;
    let key = match key {
        WinitKey::Named(named) => named_key(*named),
        WinitKey::Character(text) => match text.as_bytes() {
            [c] if c.is_ascii_alphabetic() => Some(Key::Letter(c.to_ascii_uppercase())),
            [c] if c.is_ascii_digit() => Some(Key::Digit(c - b'0')),
            [b' '] => Some(Key::Space),
            _ => None,
        },
        _ => None,
    }
    .or_else(|| physical_digit(physical))?;
    let held = [
        (modifiers.control_key(), Modifier::Ctrl),
        (modifiers.alt_key(), Modifier::Alt),
        (modifiers.shift_key(), Modifier::Shift),
        (modifiers.super_key(), Modifier::Super),
    ];
    let modifiers: Modifiers = held.iter().filter(|(on, _)| *on).map(|(_, m)| *m).collect();
    Some(Hotkey { modifiers, key })
}

/// The hotkey key for a winit named key.
fn named_key(named: NamedKey) -> Option<Key> {
    const F: [NamedKey; 24] = [
        NamedKey::F1,
        NamedKey::F2,
        NamedKey::F3,
        NamedKey::F4,
        NamedKey::F5,
        NamedKey::F6,
        NamedKey::F7,
        NamedKey::F8,
        NamedKey::F9,
        NamedKey::F10,
        NamedKey::F11,
        NamedKey::F12,
        NamedKey::F13,
        NamedKey::F14,
        NamedKey::F15,
        NamedKey::F16,
        NamedKey::F17,
        NamedKey::F18,
        NamedKey::F19,
        NamedKey::F20,
        NamedKey::F21,
        NamedKey::F22,
        NamedKey::F23,
        NamedKey::F24,
    ];
    if let Some(index) = F.iter().position(|f| *f == named) {
        return Some(Key::F(index as u8 + 1));
    }
    Some(match named {
        NamedKey::Space => Key::Space,
        NamedKey::Insert => Key::Insert,
        NamedKey::Delete => Key::Delete,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::Pause => Key::Pause,
        NamedKey::PrintScreen => Key::PrintScreen,
        _ => return None,
    })
}

/// The digit of the top row's physical key.
fn physical_digit(physical: PhysicalKey) -> Option<Key> {
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    match physical {
        PhysicalKey::Code(code) => DIGITS.iter().position(|d| *d == code).map(|n| Key::Digit(n as u8)),
        PhysicalKey::Unidentified(_) => None,
    }
}

/// The running hotkeys: the bindings and the platform's `hotkeys` thread.
pub struct AppHotkeys {
    hotkeys: platform::Hotkeys,
    bindings: Vec<(&'static str, Option<Hotkey>)>,
    /// No global keys: key presses in the window are matched here (research R8).
    window_only: bool,
    /// The modifiers held, from the window's events (for `window_only`).
    modifiers: ModifiersState,
}

impl AppHotkeys {
    /// Starts the `hotkeys` thread with the settings' keys; its events go to `send` (from that
    /// thread). `description` names an action for the desktop's shortcut list.
    pub fn start(
        hotkeys: &settings::Hotkeys,
        description: impl Fn(&str) -> String,
        send: impl Fn(HotkeyEvent) + Send + Sync + 'static,
    ) -> AppHotkeys {
        let app = AppHotkeys {
            hotkeys: platform::Hotkeys::start(send),
            bindings: read(hotkeys),
            window_only: false,
            modifiers: ModifiersState::empty(),
        };
        app.register(description);
        app
    }

    /// Gives the platform the keys again with the actions' descriptions (also after a language
    /// change: the portal's list shows them).
    pub fn register(&self, description: impl Fn(&str) -> String) {
        let bindings = self
            .bindings
            .iter()
            .map(|(action, key)| (action.to_string(), *key, description(action)))
            .collect();
        self.hotkeys.set(bindings);
    }

    /// A platform event; returns the action to run for a press.
    pub fn event(&mut self, event: HotkeyEvent) -> Option<&'static str> {
        match event {
            HotkeyEvent::Pressed(action) => {
                let found = self.action(&action);
                if found.is_none() {
                    log::warn!("hotkeys: press of unknown action {action}");
                }
                found
            }
            HotkeyEvent::Bound { action, shown } => {
                match shown {
                    Some(key) => log::info!("hotkeys: {action} on {key}"),
                    None => log::info!("hotkeys: {action} has no key"),
                }
                None
            }
            HotkeyEvent::WindowOnly => {
                log::info!("hotkeys: no global keys here, they work only while the window has the focus");
                self.window_only = true;
                None
            }
        }
    }

    /// The modifiers held changed (window event).
    pub fn set_modifiers(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    /// A key event of the focused window: the action whose key it is, only where global keys do
    /// not work (elsewhere the system already reports the press; matching it too would run the
    /// action twice). Once per press: repeats and releases are ignored (FR-014).
    pub fn window_key(&self, event: &KeyEvent) -> Option<&'static str> {
        if !self.window_only || event.state != winit::event::ElementState::Pressed || event.repeat {
            return None;
        }
        let pressed = from_window(self.modifiers, &event.key_without_modifiers(), event.physical_key)?;
        self.bindings
            .iter()
            .find(|(_, key)| *key == Some(pressed))
            .map(|(action, _)| *action)
    }

    fn action(&self, name: &str) -> Option<&'static str> {
        self.bindings
            .iter()
            .map(|(action, _)| *action)
            .find(|action| *action == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{Key as WinitKey, SmolStr};

    fn keys(screenshot: &str) -> settings::Hotkeys {
        settings::Hotkeys {
            screenshot: screenshot.into(),
        }
    }

    fn key(text: &str) -> Option<Hotkey> {
        Hotkey::parse(text)
    }

    #[test]
    fn a_valid_key_is_kept() {
        assert_eq!(read(&keys("Ctrl+Shift+S")), vec![(SCREENSHOT, key("Ctrl+Shift+S"))]);
        assert_eq!(read(&keys("F9")), vec![(SCREENSHOT, key("F9"))]);
    }

    #[test]
    fn an_empty_key_is_no_key() {
        assert_eq!(read(&keys("")), vec![(SCREENSHOT, None)]);
        assert_eq!(read(&keys("  ")), vec![(SCREENSHOT, None)]);
    }

    #[test]
    fn an_invalid_or_refused_key_takes_the_default() {
        for text in ["Ctrl+", "Hyper+F8", "Banana", "Shift+A", "A", "Delete"] {
            assert_eq!(read(&keys(text)), vec![(SCREENSHOT, key("Ctrl+Shift+F8"))], "{text}");
        }
    }

    #[test]
    fn the_first_action_keeps_a_shared_key() {
        let bindings = resolve([
            ("screenshot", "Ctrl+F8", "Ctrl+F9"),
            ("record", "Ctrl+F10", "ctrl + f9"),
            ("live", "Ctrl+F9", "Banana"),
        ]);
        assert_eq!(
            bindings,
            vec![("screenshot", key("Ctrl+F9")), ("record", None), ("live", None)]
        );
    }

    #[test]
    fn a_cleared_key_frees_it_for_a_later_action() {
        let bindings = resolve([("screenshot", "Ctrl+F8", ""), ("record", "Ctrl+F10", "Ctrl+F8")]);
        assert_eq!(bindings, vec![("screenshot", None), ("record", key("Ctrl+F8"))]);
    }

    fn character(text: &str) -> WinitKey {
        WinitKey::Character(SmolStr::new(text))
    }

    #[test]
    fn window_keys_map_to_hotkeys() {
        let none = PhysicalKey::Code(KeyCode::KeyQ);
        assert_eq!(
            from_window(ModifiersState::CONTROL, &WinitKey::Named(NamedKey::F8), none),
            key("Ctrl+F8")
        );
        assert_eq!(
            from_window(ModifiersState::empty(), &WinitKey::Named(NamedKey::F24), none),
            key("F24")
        );
        assert_eq!(
            from_window(ModifiersState::CONTROL | ModifiersState::SHIFT, &character("s"), none),
            key("Ctrl+Shift+S")
        );
        assert_eq!(
            from_window(ModifiersState::ALT | ModifiersState::SUPER, &character("7"), none),
            key("Alt+Super+7")
        );
        assert_eq!(
            from_window(ModifiersState::CONTROL, &WinitKey::Named(NamedKey::PageUp), none),
            key("Ctrl+PageUp")
        );
        assert_eq!(
            from_window(ModifiersState::CONTROL, &WinitKey::Named(NamedKey::Space), none),
            key("Ctrl+Space")
        );
    }

    #[test]
    fn a_digit_comes_from_the_physical_key_when_the_layout_types_a_symbol() {
        // AZERTY: the top row types `&` without Shift.
        assert_eq!(
            from_window(
                ModifiersState::CONTROL,
                &character("&"),
                PhysicalKey::Code(KeyCode::Digit1)
            ),
            key("Ctrl+1")
        );
    }

    #[test]
    fn other_keys_are_not_hotkeys() {
        let code = PhysicalKey::Code(KeyCode::ControlLeft);
        assert_eq!(
            from_window(ModifiersState::CONTROL, &WinitKey::Named(NamedKey::Control), code),
            None
        );
        assert_eq!(
            from_window(
                ModifiersState::CONTROL,
                &character("é"),
                PhysicalKey::Code(KeyCode::Digit2)
            ),
            key("Ctrl+2")
        );
        assert_eq!(
            from_window(
                ModifiersState::CONTROL,
                &character(";"),
                PhysicalKey::Code(KeyCode::Semicolon)
            ),
            None
        );
    }
}
