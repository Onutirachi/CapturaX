//! The rule that picks the dark or light theme (spec FR-016 and FR-017, data-model.md "Theme",
//! research R8). Pure, so it is tested without a system; the system's theme comes from winit or
//! the XDG portal (T045).

use settings::ThemeSetting;

/// The theme the system reports.
#[cfg_attr(not(test), expect(dead_code, reason = "the system theme reader uses it (T045)"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTheme {
    Dark,
    Light,
}

/// Whether to show the dark theme: `Dark` and `Light` are used as set; `Auto` follows the
/// system, and an unknown system theme gives dark.
#[cfg_attr(not(test), expect(dead_code, reason = "the theme selection uses it (T045)"))]
pub fn resolve(setting: ThemeSetting, system: Option<SystemTheme>) -> bool {
    match setting {
        ThemeSetting::Dark => true,
        ThemeSetting::Light => false,
        ThemeSetting::Auto => system != Some(SystemTheme::Light),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system() {
        assert!(resolve(ThemeSetting::Auto, Some(SystemTheme::Dark)));
        assert!(!resolve(ThemeSetting::Auto, Some(SystemTheme::Light)));
    }

    #[test]
    fn auto_is_dark_when_the_system_is_unknown() {
        assert!(resolve(ThemeSetting::Auto, None));
    }

    #[test]
    fn a_set_theme_wins_over_the_system() {
        for system in [Some(SystemTheme::Dark), Some(SystemTheme::Light), None] {
            assert!(resolve(ThemeSetting::Dark, system));
            assert!(!resolve(ThemeSetting::Light, system));
        }
    }
}
