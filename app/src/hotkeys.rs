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

/// The action's default key.
fn default_key(action: &str) -> Option<Hotkey> {
    ACTIONS
        .iter()
        .find(|(name, _)| *name == action)
        .and_then(|(_, default)| Hotkey::parse(default))
}

/// Writes the action's key into the settings (`""`: no key, FR-018).
#[cfg_attr(not(test), expect(dead_code, reason = "the Hotkeys category calls it (T031)"))]
pub fn set_setting(hotkeys: &mut settings::Hotkeys, action: &str, key: Option<Hotkey>) {
    let text = key.map(|key| key.to_string()).unwrap_or_default();
    match action {
        SCREENSHOT => hotkeys.screenshot = text,
        _ => log::warn!("hotkeys: no setting for action {action}"),
    }
}

/// A key press while recording, from the window (research R14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Esc: cancels.
    Escape,
    /// A key that maps to a hotkey (allowed or not).
    Key(Hotkey),
    /// A key outside the closed list (a symbol, Enter…).
    Other,
}

/// The press a window key event makes while recording; None for what the recorder ignores: a
/// release, an auto-repeat, a modifier alone.
#[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
fn press(modifiers: ModifiersState, event: &KeyEvent) -> Option<Press> {
    if event.state != winit::event::ElementState::Pressed || event.repeat {
        return None;
    }
    press_of(modifiers, &event.key_without_modifiers(), event.physical_key)
}

/// [`press`] from the modifiers, the key without modifiers and the physical key.
fn press_of(modifiers: ModifiersState, key: &winit::keyboard::Key, physical: PhysicalKey) -> Option<Press> {
    use winit::keyboard::Key as WinitKey;
    match key {
        WinitKey::Named(NamedKey::Escape) => Some(Press::Escape),
        WinitKey::Named(
            NamedKey::Control
            | NamedKey::Shift
            | NamedKey::Alt
            | NamedKey::AltGraph
            | NamedKey::Super
            | NamedKey::Meta
            | NamedKey::Hyper,
        ) => None,
        _ => Some(from_window(modifiers, key, physical).map_or(Press::Other, Press::Key)),
    }
}

/// Why a key cannot be the action's (FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Not a key the rules allow (a letter without Ctrl, Alt or Super, a key outside the list…).
    NotAllowed,
    /// Another action has it.
    Taken(&'static str),
}

/// What a press did to the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// The key is the action's now: save and register it (FR-018).
    Accepted { action: &'static str, key: Hotkey },
    /// Refused; still recording, the row shows why.
    Refused(Refusal),
    /// Esc: the key is unchanged.
    Cancelled,
}

/// Whether `key` can be `action`'s among `bindings` (FR-016).
fn check(bindings: &[(&'static str, Option<Hotkey>)], action: &str, key: Hotkey) -> Result<(), Refusal> {
    if !key.allowed() {
        return Err(Refusal::NotAllowed);
    }
    match bindings
        .iter()
        .find(|(other, bound)| *other != action && *bound == Some(key))
    {
        Some((other, _)) => Err(Refusal::Taken(other)),
        None => Ok(()),
    }
}

/// Gives `action` its new key in `bindings`.
fn assign(bindings: &mut [(&'static str, Option<Hotkey>)], action: &str, key: Option<Hotkey>) {
    if let Some(binding) = bindings.iter_mut().find(|(name, _)| *name == action) {
        binding.1 = key;
    }
}

/// The key recorder (FR-016, FR-017): which action is recording and the last refusal shown.
#[derive(Debug, Default)]
pub struct Recorder {
    recording: Option<(&'static str, Option<Refusal>)>,
}

impl Recorder {
    /// The action recording, if any, with the refusal of its last press.
    pub fn recording(&self) -> Option<(&'static str, Option<Refusal>)> {
        self.recording
    }

    /// Starts recording `action`'s key (a new start replaces another recording).
    fn start(&mut self, action: &'static str) {
        self.recording = Some((action, None));
    }

    /// Stops without a change (the panel closed, the window lost the focus).
    fn cancel(&mut self) -> bool {
        self.recording.take().is_some()
    }

    /// A press while recording; None when not recording.
    fn press(&mut self, bindings: &[(&'static str, Option<Hotkey>)], press: Press) -> Option<Recorded> {
        let (action, _) = self.recording?;
        let outcome = match press {
            Press::Escape => Recorded::Cancelled,
            Press::Other => Recorded::Refused(Refusal::NotAllowed),
            Press::Key(key) => match check(bindings, action, key) {
                Ok(()) => Recorded::Accepted { action, key },
                Err(refusal) => Recorded::Refused(refusal),
            },
        };
        self.recording = match outcome {
            Recorded::Refused(refusal) => Some((action, Some(refusal))),
            Recorded::Accepted { .. } | Recorded::Cancelled => None,
        };
        Some(outcome)
    }
}

/// The running hotkeys: the bindings and the platform's `hotkeys` thread.
pub struct AppHotkeys {
    hotkeys: platform::Hotkeys,
    bindings: Vec<(&'static str, Option<Hotkey>)>,
    recorder: Recorder,
    /// The actions' descriptions for the platform, kept for re-registering after a change.
    descriptions: Vec<String>,
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
        let mut app = AppHotkeys {
            hotkeys: platform::Hotkeys::start(send),
            bindings: read(hotkeys),
            recorder: Recorder::default(),
            descriptions: Vec::new(),
            window_only: false,
            modifiers: ModifiersState::empty(),
        };
        app.register(description);
        app
    }

    /// Gives the platform the keys again with the actions' descriptions (also after a language
    /// change: the portal's list shows them).
    pub fn register(&mut self, description: impl Fn(&str) -> String) {
        self.descriptions = self.bindings.iter().map(|(action, _)| description(action)).collect();
        self.send();
    }

    /// Gives the platform the bindings with the descriptions last registered.
    fn send(&self) {
        let bindings = self
            .bindings
            .iter()
            .zip(&self.descriptions)
            .map(|((action, key), description)| (action.to_string(), *key, description.clone()))
            .collect();
        self.hotkeys.set(bindings);
    }

    /// Starts recording `action`'s key: every hotkey is released until it ends (FR-017). False
    /// for an unknown action.
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn start_recording(&mut self, action: &str) -> bool {
        let Some(action) = self.action(action) else {
            return false;
        };
        log::info!("hotkeys: recording {action}'s key");
        self.recorder.start(action);
        self.hotkeys.suspend(true);
        true
    }

    /// The recording, if any: the action and the refusal of its last press.
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn recording(&self) -> Option<(&'static str, Option<Refusal>)> {
        self.recorder.recording()
    }

    /// Ends a recording without a change (the panel closed, the window lost the focus).
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn cancel_recording(&mut self) {
        if self.recorder.cancel() {
            log::info!("hotkeys: recording cancelled");
            self.hotkeys.suspend(false);
        }
    }

    /// A key event of the window while recording: what it did, None when not recording or when
    /// the recorder ignores it (a release, a repeat, a modifier alone). An accepted key is
    /// registered at once (FR-018); the caller saves it with [`set_setting`].
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn record(&mut self, event: &KeyEvent) -> Option<Recorded> {
        self.recorder.recording()?;
        let outcome = self.recorder.press(&self.bindings, press(self.modifiers, event)?)?;
        match outcome {
            Recorded::Accepted { action, key } => {
                log::info!("hotkeys: {action} key set to {key}");
                assign(&mut self.bindings, action, Some(key));
                self.send();
                self.hotkeys.suspend(false);
            }
            Recorded::Cancelled => {
                log::info!("hotkeys: recording cancelled");
                self.hotkeys.suspend(false);
            }
            Recorded::Refused(refusal) => log::info!("hotkeys: key refused ({refusal:?})"),
        }
        Some(outcome)
    }

    /// Leaves `action` with no key (FR-016, FR-018); the caller saves `None`.
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn clear(&mut self, action: &str) {
        if let Some(action) = self.action(action) {
            log::info!("hotkeys: {action} key cleared");
            assign(&mut self.bindings, action, None);
            self.send();
        }
    }

    /// Puts back `action`'s default key; refused when another action has it. The caller saves the
    /// key returned.
    #[expect(dead_code, reason = "the Hotkeys category calls it (T031)")]
    pub fn reset(&mut self, action: &str) -> Result<Option<Hotkey>, Refusal> {
        let Some(action) = self.action(action) else {
            return Ok(None);
        };
        let key = default_key(action);
        if let Some(key) = key {
            check(&self.bindings, action, key)?;
        }
        log::info!("hotkeys: {action} key reset");
        assign(&mut self.bindings, action, key);
        self.send();
        Ok(key)
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
        if !self.window_only
            || self.recorder.recording().is_some()
            || event.state != winit::event::ElementState::Pressed
            || event.repeat
        {
            return None;
        }
        let pressed = from_window(self.modifiers, &event.key_without_modifiers(), event.physical_key)?;
        self.bindings
            .iter()
            .find(|(_, key)| *key == Some(pressed))
            .map(|(action, _)| *action)
    }

    /// The key bound to `action` in the settings, if any (shown in the dock button's tooltip,
    /// FR-011).
    pub fn key(&self, action: &str) -> Option<Hotkey> {
        self.bindings
            .iter()
            .find(|(name, _)| *name == action)
            .and_then(|(_, key)| *key)
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

    fn pressed(text: &str) -> Press {
        Press::Key(key(text).unwrap())
    }

    /// Screenshot on Ctrl+F8, record on Ctrl+F10, live with no key.
    fn three() -> Vec<(&'static str, Option<Hotkey>)> {
        vec![
            ("screenshot", key("Ctrl+F8")),
            ("record", key("Ctrl+F10")),
            ("live", None),
        ]
    }

    #[test]
    fn an_allowed_key_is_accepted_and_ends_the_recording() {
        let mut recorder = Recorder::default();
        recorder.start("screenshot");
        assert_eq!(recorder.recording(), Some(("screenshot", None)));
        assert_eq!(
            recorder.press(&three(), pressed("Ctrl+Shift+S")),
            Some(Recorded::Accepted {
                action: "screenshot",
                key: key("Ctrl+Shift+S").unwrap()
            })
        );
        assert_eq!(recorder.recording(), None);
        assert_eq!(recorder.press(&three(), pressed("F9")), None);
    }

    #[test]
    fn the_action_s_own_key_is_accepted() {
        let mut recorder = Recorder::default();
        recorder.start("screenshot");
        assert!(matches!(
            recorder.press(&three(), pressed("Ctrl+F8")),
            Some(Recorded::Accepted { .. })
        ));
    }

    #[test]
    fn a_key_the_rules_refuse_keeps_recording() {
        let mut recorder = Recorder::default();
        recorder.start("screenshot");
        for press in [
            pressed("A"),
            pressed("Shift+A"),
            pressed("Delete"),
            pressed("Space"),
            Press::Other,
        ] {
            assert_eq!(
                recorder.press(&three(), press),
                Some(Recorded::Refused(Refusal::NotAllowed)),
                "{press:?}"
            );
            assert_eq!(recorder.recording(), Some(("screenshot", Some(Refusal::NotAllowed))));
        }
        assert!(matches!(
            recorder.press(&three(), pressed("Alt+A")),
            Some(Recorded::Accepted { .. })
        ));
    }

    #[test]
    fn a_key_of_another_action_is_refused_naming_it() {
        let mut recorder = Recorder::default();
        recorder.start("live");
        assert_eq!(
            recorder.press(&three(), pressed("Ctrl+F10")),
            Some(Recorded::Refused(Refusal::Taken("record")))
        );
        assert_eq!(recorder.recording(), Some(("live", Some(Refusal::Taken("record")))));
    }

    #[test]
    fn esc_cancels_and_cancel_stops() {
        let mut recorder = Recorder::default();
        recorder.start("screenshot");
        recorder.press(&three(), pressed("A"));
        assert_eq!(recorder.press(&three(), Press::Escape), Some(Recorded::Cancelled));
        assert_eq!(recorder.recording(), None);
        assert!(!recorder.cancel());
        recorder.start("screenshot");
        assert!(recorder.cancel());
        assert_eq!(recorder.recording(), None);
    }

    #[test]
    fn clear_and_reset_change_the_binding() {
        let mut bindings = three();
        assign(&mut bindings, "screenshot", None);
        assert_eq!(bindings[0], ("screenshot", None));
        // The default is free again: a reset may take it.
        assert_eq!(
            check(&bindings, "screenshot", default_key("screenshot").unwrap()),
            Ok(())
        );
        // Another action has it: refused, naming that action.
        assign(&mut bindings, "record", default_key("screenshot"));
        assert_eq!(
            check(&bindings, "screenshot", default_key("screenshot").unwrap()),
            Err(Refusal::Taken("record"))
        );
        assert_eq!(default_key("unknown"), None);
    }

    #[test]
    fn the_setting_takes_the_file_form() {
        let mut hotkeys = keys("Ctrl+F8");
        set_setting(&mut hotkeys, SCREENSHOT, key("super + shift + s"));
        assert_eq!(hotkeys.screenshot, "Shift+Super+S");
        set_setting(&mut hotkeys, SCREENSHOT, None);
        assert_eq!(hotkeys.screenshot, "");
        assert_eq!(read(&hotkeys), vec![(SCREENSHOT, None)]);
    }

    #[test]
    fn window_presses_for_the_recorder() {
        let code = |code| PhysicalKey::Code(code);
        let ctrl = ModifiersState::CONTROL;
        assert_eq!(
            press_of(ctrl, &WinitKey::Named(NamedKey::Escape), code(KeyCode::Escape)),
            Some(Press::Escape)
        );
        assert_eq!(
            press_of(ctrl, &WinitKey::Named(NamedKey::F8), code(KeyCode::F8)),
            Some(pressed("Ctrl+F8"))
        );
        assert_eq!(
            press_of(ctrl | ModifiersState::SHIFT, &character("&"), code(KeyCode::Digit1)),
            Some(pressed("Ctrl+Shift+1"))
        );
        assert_eq!(
            press_of(ctrl, &WinitKey::Named(NamedKey::Enter), code(KeyCode::Enter)),
            Some(Press::Other)
        );
        for (named, physical) in [
            (NamedKey::Control, KeyCode::ControlLeft),
            (NamedKey::Shift, KeyCode::ShiftRight),
            (NamedKey::Alt, KeyCode::AltLeft),
            (NamedKey::AltGraph, KeyCode::AltRight),
            (NamedKey::Super, KeyCode::SuperLeft),
        ] {
            assert_eq!(
                press_of(ctrl, &WinitKey::Named(named), code(physical)),
                None,
                "{named:?}"
            );
        }
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
