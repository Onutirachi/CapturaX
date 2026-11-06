//! The rule that picks the dark or light theme (spec FR-016 and FR-017, data-model.md "Theme",
//! research R8). Pure, so it is tested without a system; the system's theme comes from
//! `platform::theme`.

use settings::ThemeSetting;

/// The theme the system reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTheme {
    Dark,
    Light,
}

impl From<platform::theme::Theme> for SystemTheme {
    fn from(theme: platform::theme::Theme) -> Self {
        match theme {
            platform::theme::Theme::Dark => Self::Dark,
            platform::theme::Theme::Light => Self::Light,
        }
    }
}

/// Whether to show the dark theme: `Dark` and `Light` are used as set; `Auto` follows the
/// system, and an unknown system theme gives dark.
pub fn resolve(setting: ThemeSetting, system: Option<SystemTheme>) -> bool {
    match setting {
        ThemeSetting::Dark => true,
        ThemeSetting::Light => false,
        ThemeSetting::Auto => system != Some(SystemTheme::Light),
    }
}

/// The settings panel's Theme choices, in its order (Automatic, Dark, Light).
const CHOICES: [ThemeSetting; 3] = [ThemeSetting::Auto, ThemeSetting::Dark, ThemeSetting::Light];

/// The position of `setting` in the settings panel's Theme select.
pub fn choice_index(setting: ThemeSetting) -> i32 {
    CHOICES.iter().position(|c| *c == setting).unwrap_or(0) as i32
}

/// The setting at `index` of the settings panel's Theme select, if there is one.
pub fn choice(index: i32) -> Option<ThemeSetting> {
    usize::try_from(index).ok().and_then(|i| CHOICES.get(i).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choices_round_trip() {
        for setting in CHOICES {
            assert_eq!(choice(choice_index(setting)), Some(setting));
        }
        assert_eq!(choice_index(ThemeSetting::Auto), 0);
        assert_eq!(choice(-1), None);
        assert_eq!(choice(3), None);
    }

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
