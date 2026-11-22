//! Global hotkeys (spec 20261121-134830-screenshot, contracts/platform.md "Hotkeys"). This part
//! is pure: the key value, its text form and its codes on each system (data-model.md "Hotkey",
//! research R14).

use std::fmt;

/// A modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    /// The Windows key on Windows, Super on Linux.
    Super,
}

impl Modifier {
    /// In the order of the text form.
    pub const ALL: [Modifier; 4] = [Modifier::Ctrl, Modifier::Alt, Modifier::Shift, Modifier::Super];

    fn bit(self) -> u8 {
        match self {
            Modifier::Ctrl => 1,
            Modifier::Alt => 2,
            Modifier::Shift => 4,
            Modifier::Super => 8,
        }
    }

    /// The name in the settings file.
    fn name(self) -> &'static str {
        match self {
            Modifier::Ctrl => "Ctrl",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
            Modifier::Super => "Super",
        }
    }
}

/// A set of modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);

    pub fn contains(self, modifier: Modifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    pub fn with(self, modifier: Modifier) -> Modifiers {
        Modifiers(self.0 | modifier.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The modifiers present, in the text form's order.
    pub fn iter(self) -> impl Iterator<Item = Modifier> {
        Modifier::ALL.into_iter().filter(move |m| self.contains(*m))
    }

    /// `RegisterHotKey`'s `MOD_*` flags (without `MOD_NOREPEAT`).
    pub fn windows_mod(self) -> u32 {
        const MOD_ALT: u32 = 0x1;
        const MOD_CONTROL: u32 = 0x2;
        const MOD_SHIFT: u32 = 0x4;
        const MOD_WIN: u32 = 0x8;
        self.iter()
            .map(|m| match m {
                Modifier::Ctrl => MOD_CONTROL,
                Modifier::Alt => MOD_ALT,
                Modifier::Shift => MOD_SHIFT,
                Modifier::Super => MOD_WIN,
            })
            .fold(0, |a, b| a | b)
    }

    /// The X11 modifier mask (`ShiftMask`, `ControlMask`, `Mod1Mask` for Alt, `Mod4Mask` for
    /// Super), without Num Lock or Caps Lock.
    pub fn x11_mask(self) -> u16 {
        self.iter()
            .map(|m| match m {
                Modifier::Shift => 1 << 0,
                Modifier::Ctrl => 1 << 2,
                Modifier::Alt => 1 << 3,
                Modifier::Super => 1 << 6,
            })
            .fold(0, |a, b| a | b)
    }
}

impl FromIterator<Modifier> for Modifiers {
    fn from_iter<I: IntoIterator<Item = Modifier>>(iter: I) -> Self {
        iter.into_iter().fold(Modifiers::NONE, Modifiers::with)
    }
}

/// A key of the closed list (research R14): keys that sit in the same place and mean the same
/// thing on every layout and system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// `A`–`Z`, as the uppercase ASCII letter.
    Letter(u8),
    /// `0`–`9`, as the number.
    Digit(u8),
    /// `F1`–`F24`, as the number.
    F(u8),
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Pause,
    PrintScreen,
}

/// The named keys with their file name, Windows virtual-key code, X11 keysym and portal (XKB)
/// name.
const NAMED: [(Key, &str, u16, u32, &str); 13] = [
    (Key::Space, "Space", 0x20, 0x0020, "space"),
    (Key::Insert, "Insert", 0x2D, 0xff63, "Insert"),
    (Key::Delete, "Delete", 0x2E, 0xffff, "Delete"),
    (Key::Home, "Home", 0x24, 0xff50, "Home"),
    (Key::End, "End", 0x23, 0xff57, "End"),
    (Key::PageUp, "PageUp", 0x21, 0xff55, "Page_Up"),
    (Key::PageDown, "PageDown", 0x22, 0xff56, "Page_Down"),
    (Key::Up, "Up", 0x26, 0xff52, "Up"),
    (Key::Down, "Down", 0x28, 0xff54, "Down"),
    (Key::Left, "Left", 0x25, 0xff51, "Left"),
    (Key::Right, "Right", 0x27, 0xff53, "Right"),
    (Key::Pause, "Pause", 0x13, 0xff13, "Pause"),
    (Key::PrintScreen, "PrintScreen", 0x2C, 0xff61, "Print"),
];

impl Key {
    /// The key for a name of the settings file, ignoring case; None when not in the list.
    pub fn parse(name: &str) -> Option<Key> {
        let upper = name.to_ascii_uppercase();
        match upper.as_bytes() {
            [c @ b'A'..=b'Z'] => return Some(Key::Letter(*c)),
            [c @ b'0'..=b'9'] => return Some(Key::Digit(c - b'0')),
            [b'F', rest @ ..] if !rest.is_empty() && rest[0] != b'0' => {
                if let Some(n) = upper[1..].parse::<u8>().ok().filter(|n| (1..=24).contains(n)) {
                    return Some(Key::F(n));
                }
            }
            _ => {}
        }
        NAMED
            .iter()
            .find(|(_, file, ..)| file.eq_ignore_ascii_case(name))
            .map(|(key, ..)| *key)
    }

    fn named(self) -> Option<&'static (Key, &'static str, u16, u32, &'static str)> {
        NAMED.iter().find(|(key, ..)| *key == self)
    }

    /// The name in the settings file: `A`, `0`, `F8`, `PageUp`…
    pub fn name(self) -> String {
        match self {
            Key::Letter(c) => char::from(c).to_string(),
            Key::Digit(n) => n.to_string(),
            Key::F(n) => format!("F{n}"),
            other => other.named().map_or("", |n| n.1).to_string(),
        }
    }

    /// The Windows virtual-key code (`VK_*`).
    pub fn windows_vk(self) -> u16 {
        match self {
            Key::Letter(c) => u16::from(c),
            Key::Digit(n) => 0x30 + u16::from(n),
            Key::F(n) => 0x70 + u16::from(n) - 1,
            other => other.named().map_or(0, |n| n.2),
        }
    }

    /// The X11 keysym (letters lowercase, as the keyboard mapping lists them first).
    pub fn x11_keysym(self) -> u32 {
        match self {
            Key::Letter(c) => u32::from(c.to_ascii_lowercase()),
            Key::Digit(n) => 0x30 + u32::from(n),
            Key::F(n) => 0xffbe + u32::from(n) - 1,
            other => other.named().map_or(0, |n| n.3),
        }
    }

    /// The key's name in a portal trigger (its XKB keysym name: `a`, `F8`, `Page_Up`…).
    fn portal_name(self) -> String {
        match self {
            Key::Letter(c) => char::from(c.to_ascii_lowercase()).to_string(),
            Key::Digit(_) | Key::F(_) => self.name(),
            other => other.named().map_or("", |n| n.4).to_string(),
        }
    }

    /// A letter, digit or Space: typed text, which needs Ctrl, Alt or Super to be a hotkey.
    fn types_text(self) -> bool {
        matches!(self, Key::Letter(_) | Key::Digit(_) | Key::Space)
    }
}

/// A key with its modifiers (data-model.md "Hotkey").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Hotkey {
    /// The key for its text form (`Ctrl+Shift+F8`), ignoring case and spaces; `Win` is read as
    /// Super. None for an unknown key or modifier, a repeated modifier, or no key.
    pub fn parse(text: &str) -> Option<Hotkey> {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let mut parts: Vec<&str> = compact.split('+').collect();
        let key = Key::parse(parts.pop()?)?;
        let mut modifiers = Modifiers::NONE;
        for part in parts {
            let modifier = match part.to_ascii_lowercase().as_str() {
                "ctrl" => Modifier::Ctrl,
                "alt" => Modifier::Alt,
                "shift" => Modifier::Shift,
                "super" | "win" => Modifier::Super,
                _ => return None,
            };
            if modifiers.contains(modifier) {
                return None;
            }
            modifiers = modifiers.with(modifier);
        }
        Some(Hotkey { modifiers, key })
    }

    /// Whether the key may be a hotkey (spec FR-016): F1–F24 alone; any other key with a
    /// modifier; A–Z, 0–9 and Space need Ctrl, Alt or Super (Shift alone would take a capital
    /// letter from every app).
    pub fn allowed(&self) -> bool {
        let m = self.modifiers;
        match self.key {
            Key::F(_) => true,
            key if key.types_text() => {
                m.contains(Modifier::Ctrl) || m.contains(Modifier::Alt) || m.contains(Modifier::Super)
            }
            _ => !m.is_empty(),
        }
    }

    /// The trigger for the GlobalShortcuts portal: `CTRL+SHIFT+F8`, `LOGO` for Super (research
    /// R7).
    pub fn to_portal(&self) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| {
                match m {
                    Modifier::Ctrl => "CTRL",
                    Modifier::Alt => "ALT",
                    Modifier::Shift => "SHIFT",
                    Modifier::Super => "LOGO",
                }
                .to_string()
            })
            .collect();
        parts.push(self.key.portal_name());
        parts.join("+")
    }

    /// The text shown to the user: the file form with `Win` for Super on Windows. Key names the
    /// locale changes (`PgUp`…) are the UI's.
    pub fn shown(&self, windows: bool) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| {
                if m == Modifier::Super && windows {
                    "Win"
                } else {
                    m.name()
                }
                .to_string()
            })
            .collect();
        parts.push(self.key.name());
        parts.join("+")
    }
}

impl fmt::Display for Hotkey {
    /// The settings file's form: `Ctrl+Alt+Shift+Super+<Key>` with only the modifiers present.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for m in self.modifiers.iter() {
            write!(f, "{}+", m.name())?;
        }
        f.write_str(&self.key.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Hotkey {
        Hotkey::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    /// Every key of the list.
    fn all_keys() -> Vec<Key> {
        let mut keys: Vec<Key> = (b'A'..=b'Z').map(Key::Letter).collect();
        keys.extend((0..=9).map(Key::Digit));
        keys.extend((1..=24).map(Key::F));
        keys.extend(NAMED.iter().map(|n| n.0));
        keys
    }

    #[test]
    fn parse_ignores_case_and_spaces() {
        let ctrl_f8 = Hotkey {
            modifiers: Modifiers::NONE.with(Modifier::Ctrl),
            key: Key::F(8),
        };
        assert_eq!(key("Ctrl+F8"), ctrl_f8);
        assert_eq!(key("ctrl + f8"), ctrl_f8);
        assert_eq!(key(" CTRL+F8 "), ctrl_f8);
        assert_eq!(key("Shift+Ctrl+PageUp"), key("Ctrl+Shift+pageup"));
        assert_eq!(key("Win+A"), key("Super+A"));
        assert_eq!(key("F24").key, Key::F(24));
        assert_eq!(key("Ctrl+F").key, Key::Letter(b'F'));
        assert_eq!(key("Alt+5").key, Key::Digit(5));
        assert_eq!(key("Ctrl+printscreen").key, Key::PrintScreen);
    }

    #[test]
    fn invalid_text_does_not_parse() {
        for text in [
            "",
            "+",
            "Ctrl+",
            "Ctrl",
            "F0",
            "F25",
            "F08",
            "Ctrl+Nope",
            "Hyper+F8",
            "Ctrl+Ctrl+F8",
            "F8+Ctrl",
            "Ctrl+F8+A",
            "Ctrl+é",
            "Ctrl+-",
            "AA",
        ] {
            assert_eq!(Hotkey::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn display_is_the_file_form_and_round_trips() {
        assert_eq!(key("super+shift+alt+ctrl+f8").to_string(), "Ctrl+Alt+Shift+Super+F8");
        assert_eq!(key("shift+pagedown").to_string(), "Shift+PageDown");
        assert_eq!(key("ctrl+a").to_string(), "Ctrl+A");
        assert_eq!(key("f12").to_string(), "F12");
        for k in all_keys() {
            for modifiers in [Modifiers::NONE, Modifier::ALL.into_iter().collect()] {
                let hotkey = Hotkey { modifiers, key: k };
                assert_eq!(Hotkey::parse(&hotkey.to_string()), Some(hotkey), "{hotkey}");
            }
        }
    }

    #[test]
    fn allowed_follows_fr_016() {
        // Function keys alone.
        assert!(key("F1").allowed() && key("F24").allowed() && key("Shift+F8").allowed());
        // Other keys need a modifier.
        for text in [
            "Insert",
            "Delete",
            "Home",
            "End",
            "PageUp",
            "PageDown",
            "Up",
            "Pause",
            "PrintScreen",
        ] {
            assert!(!key(text).allowed(), "{text}");
            assert!(key(&format!("Shift+{text}")).allowed(), "{text}");
        }
        // Text keys need Ctrl, Alt or Super; Shift alone is not enough.
        for text in ["A", "Z", "0", "9", "Space"] {
            assert!(!key(text).allowed(), "{text}");
            assert!(!key(&format!("Shift+{text}")).allowed(), "{text}");
            for m in ["Ctrl", "Alt", "Super", "Ctrl+Shift"] {
                assert!(key(&format!("{m}+{text}")).allowed(), "{m}+{text}");
            }
        }
    }

    #[test]
    fn portal_triggers() {
        assert_eq!(key("Ctrl+F8").to_portal(), "CTRL+F8");
        assert_eq!(key("Ctrl+Shift+F8").to_portal(), "CTRL+SHIFT+F8");
        assert_eq!(key("Super+Alt+A").to_portal(), "ALT+LOGO+a");
        assert_eq!(key("Ctrl+Space").to_portal(), "CTRL+space");
        assert_eq!(key("Ctrl+PageUp").to_portal(), "CTRL+Page_Up");
        assert_eq!(key("Alt+PrintScreen").to_portal(), "ALT+Print");
        assert_eq!(key("Ctrl+7").to_portal(), "CTRL+7");
    }

    #[test]
    fn windows_codes() {
        assert_eq!(Key::Letter(b'A').windows_vk(), 0x41);
        assert_eq!(Key::Letter(b'Z').windows_vk(), 0x5A);
        assert_eq!(Key::Digit(0).windows_vk(), 0x30);
        assert_eq!(Key::F(1).windows_vk(), 0x70);
        assert_eq!(Key::F(8).windows_vk(), 0x77);
        assert_eq!(Key::F(24).windows_vk(), 0x87);
        assert_eq!(Key::PrintScreen.windows_vk(), 0x2C);
        assert_eq!(Key::Pause.windows_vk(), 0x13);
        assert_eq!(key("Ctrl+Alt+Shift+Super+F8").modifiers.windows_mod(), 0xF);
        assert_eq!(key("Ctrl+F8").modifiers.windows_mod(), 0x2);
        assert_eq!(key("Super+F8").modifiers.windows_mod(), 0x8);
        assert!(all_keys().iter().all(|k| k.windows_vk() != 0));
    }

    #[test]
    fn x11_codes() {
        assert_eq!(Key::Letter(b'A').x11_keysym(), 0x61);
        assert_eq!(Key::Digit(9).x11_keysym(), 0x39);
        assert_eq!(Key::F(1).x11_keysym(), 0xffbe);
        assert_eq!(Key::F(8).x11_keysym(), 0xffc5);
        assert_eq!(Key::F(24).x11_keysym(), 0xffd5);
        assert_eq!(Key::Space.x11_keysym(), 0x20);
        assert_eq!(Key::PrintScreen.x11_keysym(), 0xff61);
        assert_eq!(key("Ctrl+F8").modifiers.x11_mask(), 0x4);
        assert_eq!(key("Shift+Alt+Super+F8").modifiers.x11_mask(), 0x1 | 0x8 | 0x40);
        assert!(all_keys().iter().all(|k| k.x11_keysym() != 0));
    }

    #[test]
    fn shown_per_platform() {
        let k = key("Ctrl+Super+F8");
        assert_eq!(k.shown(true), "Ctrl+Win+F8");
        assert_eq!(k.shown(false), "Ctrl+Super+F8");
        assert_eq!(key("Shift+PageUp").shown(true), "Shift+PageUp");
    }
}
