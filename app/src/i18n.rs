//! The app's languages (spec FR-022 to FR-024, data-model.md "Language", research R7): the list
//! the settings panel shows and the rule that picks one. Pure except `system_locale`, so the rule
//! is tested without a system. Adding a language is its `.po` catalog and one line in
//! `LANGUAGES`.

/// A language the app has a catalog for (English is the source and has none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// The code saved in the settings and passed to `slint::select_bundled_translation`.
    pub code: &'static str,
    /// The language's name in itself, shown as is in every interface language.
    pub native_name: &'static str,
}

/// Every language of the app, in the order the settings panel lists them.
pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        native_name: "English",
    },
    Language {
        code: "pt-BR",
        native_name: "Português (Brasil)",
    },
];

/// The language used when the system's is not one of ours.
const FALLBACK: &str = "en";

/// The codes of `LANGUAGES`, for reading the settings (an unknown code becomes automatic).
pub fn codes() -> Vec<&'static str> {
    LANGUAGES.iter().map(|language| language.code).collect()
}

/// The language to show: `setting` is `auto` or a code (from the settings or `--lang`);
/// `system_locale` is the system's locale, if known. `auto`, and a code not in `LANGUAGES`,
/// follow the system: any Portuguese locale gives `pt-BR`, anything else, or none, gives `en`.
#[cfg_attr(not(test), expect(dead_code, reason = "the language selection uses it (T044)"))]
pub fn resolve(setting: &str, system_locale: Option<&str>) -> &'static str {
    if let Some(language) = LANGUAGES.iter().find(|language| language.code == setting) {
        return language.code;
    }
    match system_locale {
        Some(locale) if is_portuguese(locale) => "pt-BR",
        _ => FALLBACK,
    }
}

/// Whether the locale's language subtag is Portuguese: `pt`, `pt-PT`, `pt_BR.UTF-8`, any case.
fn is_portuguese(locale: &str) -> bool {
    let subtag = locale.split(['-', '_', '.', '@']).next().unwrap_or_default();
    subtag.eq_ignore_ascii_case("pt")
}

/// The system's locale as a BCP 47 tag (`pt-BR`), if the system reports one.
#[expect(dead_code, reason = "the language selection uses it (T044)")]
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_a_portuguese_system() {
        assert_eq!(resolve("auto", Some("pt-BR")), "pt-BR");
        assert_eq!(resolve("auto", Some("pt-PT")), "pt-BR");
        assert_eq!(resolve("auto", Some("pt")), "pt-BR");
        assert_eq!(resolve("auto", Some("pt_BR.UTF-8")), "pt-BR");
        assert_eq!(resolve("auto", Some("PT-br")), "pt-BR");
    }

    #[test]
    fn auto_uses_english_for_any_other_system() {
        assert_eq!(resolve("auto", Some("en-US")), "en");
        assert_eq!(resolve("auto", Some("fr-FR")), "en");
        // A language that only starts with the letters "pt" is not Portuguese.
        assert_eq!(resolve("auto", Some("ptx")), "en");
        assert_eq!(resolve("auto", Some("")), "en");
    }

    #[test]
    fn auto_uses_english_without_a_system_locale() {
        assert_eq!(resolve("auto", None), "en");
    }

    #[test]
    fn a_known_code_wins_over_the_system() {
        assert_eq!(resolve("en", Some("pt-BR")), "en");
        assert_eq!(resolve("pt-BR", Some("en-US")), "pt-BR");
        assert_eq!(resolve("pt-BR", None), "pt-BR");
    }

    #[test]
    fn an_unknown_code_is_treated_as_auto() {
        assert_eq!(resolve("fr", Some("pt-BR")), "pt-BR");
        assert_eq!(resolve("fr", Some("en-US")), "en");
        assert_eq!(resolve("pt", None), "en");
        // Codes are exact: the settings and `--lang` use the list's spelling.
        assert_eq!(resolve("pt-br", Some("en-US")), "en");
    }

    #[test]
    fn the_list_has_unique_codes_and_the_fallback() {
        let codes = codes();
        assert!(codes.contains(&FALLBACK));
        assert!(codes.contains(&"pt-BR"));
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len());
    }
}
