//! The settings and their lenient reading from a TOML table (data-model.md, contracts/settings-file.md).

use toml::{Table, Value};

/// Format version this build writes.
pub const VERSION: i64 = 1;
/// Smallest window, in logical pixels (FR-009).
pub const MIN_WIDTH: u32 = 640;
pub const MIN_HEIGHT: u32 = 360;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    pub general: General,
    pub window: WindowSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct General {
    pub language: LanguageSetting,
    pub theme: ThemeSetting,
    pub start_fullscreen: bool,
    pub keep_in_tray: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LanguageSetting {
    /// Follow the system's language.
    #[default]
    Auto,
    /// A code from the app's language list.
    Code(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeSetting {
    /// Follow the system's theme.
    #[default]
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSettings {
    /// Logical pixels, at least [`MIN_WIDTH`] × [`MIN_HEIGHT`].
    pub width: u32,
    pub height: u32,
    /// Physical pixels in desktop coordinates; none = centered on the chosen monitor.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
    /// The monitor's name as the OS reports it; none = primary.
    pub monitor: Option<String>,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            x: None,
            y: None,
            maximized: false,
            monitor: None,
        }
    }
}

impl Settings {
    /// Reads the known values from `table`, each on its own: a missing or invalid value takes its
    /// default, an unknown language or theme becomes automatic (rule 3). `known_languages` are
    /// the codes the app has.
    pub fn read(table: &Table, known_languages: &[&str]) -> Self {
        let defaults = Settings::default();
        let general = table.get("general").and_then(Value::as_table);
        let window = table.get("window").and_then(Value::as_table);
        fn get<'a>(t: Option<&'a Table>, key: &str) -> Option<&'a Value> {
            t.and_then(|t| t.get(key))
        }
        let flag =
            |t: Option<&Table>, key: &str, default: bool| get(t, key).and_then(Value::as_bool).unwrap_or(default);
        let size = |key: &str, min: u32, default: u32| match get(window, key).and_then(Value::as_integer) {
            Some(v) if v < 0 => default,
            Some(v) => u32::try_from(v).map_or(default, |v| v.max(min)),
            None => default,
        };
        let position = |key: &str| {
            get(window, key)
                .and_then(Value::as_integer)
                .and_then(|v| i32::try_from(v).ok())
        };

        let language = match get(general, "language").and_then(Value::as_str) {
            Some(code) if known_languages.contains(&code) => LanguageSetting::Code(code.to_string()),
            _ => LanguageSetting::Auto,
        };
        let theme = match get(general, "theme").and_then(Value::as_str) {
            Some("dark") => ThemeSetting::Dark,
            Some("light") => ThemeSetting::Light,
            _ => ThemeSetting::Auto,
        };
        Settings {
            general: General {
                language,
                theme,
                start_fullscreen: flag(general, "start_fullscreen", defaults.general.start_fullscreen),
                keep_in_tray: flag(general, "keep_in_tray", defaults.general.keep_in_tray),
            },
            window: WindowSettings {
                width: size("width", MIN_WIDTH, defaults.window.width),
                height: size("height", MIN_HEIGHT, defaults.window.height),
                x: position("x"),
                y: position("y"),
                maximized: flag(window, "maximized", defaults.window.maximized),
                monitor: get(window, "monitor").and_then(Value::as_str).map(str::to_string),
            },
        }
    }

    /// Writes the known values into `table`, keeping every other key and table (rule 4). A higher
    /// format version already in the table is kept.
    pub fn write_into(&self, table: &mut Table) {
        if table
            .get("version")
            .and_then(Value::as_integer)
            .is_none_or(|v| v < VERSION)
        {
            table.insert("version".into(), Value::Integer(VERSION));
        }
        let general = category(table, "general");
        let language = match &self.general.language {
            LanguageSetting::Auto => "auto",
            LanguageSetting::Code(code) => code,
        };
        general.insert("language".into(), Value::String(language.into()));
        let theme = match self.general.theme {
            ThemeSetting::Auto => "auto",
            ThemeSetting::Dark => "dark",
            ThemeSetting::Light => "light",
        };
        general.insert("theme".into(), Value::String(theme.into()));
        general.insert("start_fullscreen".into(), Value::Boolean(self.general.start_fullscreen));
        general.insert("keep_in_tray".into(), Value::Boolean(self.general.keep_in_tray));

        let w = &self.window;
        let window = category(table, "window");
        window.insert("width".into(), Value::Integer(w.width.into()));
        window.insert("height".into(), Value::Integer(w.height.into()));
        set_or_remove(window, "x", w.x.map(|v| Value::Integer(v.into())));
        set_or_remove(window, "y", w.y.map(|v| Value::Integer(v.into())));
        window.insert("maximized".into(), Value::Boolean(w.maximized));
        set_or_remove(window, "monitor", w.monitor.clone().map(Value::String));
    }
}

/// The table named `key`, created (or replacing a value that is not a table) when needed.
fn category<'a>(table: &'a mut Table, key: &str) -> &'a mut Table {
    if !table.get(key).is_some_and(Value::is_table) {
        table.insert(key.into(), Value::Table(Table::new()));
    }
    table.get_mut(key).and_then(Value::as_table_mut).expect("just inserted")
}

fn set_or_remove(table: &mut Table, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            table.insert(key.into(), v);
        }
        None => {
            table.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: &[&str] = &["en", "pt-BR"];

    fn parse(text: &str) -> Table {
        text.parse().unwrap()
    }

    fn read(text: &str) -> Settings {
        Settings::read(&parse(text), LANGUAGES)
    }

    #[test]
    fn empty_table_gives_defaults() {
        let s = read("");
        assert_eq!(s, Settings::default());
        assert_eq!(s.general.language, LanguageSetting::Auto);
        assert_eq!(s.general.theme, ThemeSetting::Auto);
        assert_eq!((s.window.width, s.window.height), (1280, 720));
        assert_eq!((s.window.x, s.window.y, &s.window.monitor), (None, None, &None));
    }

    #[test]
    fn valid_values() {
        let s = read(
            r#"
            version = 1
            [general]
            language = "pt-BR"
            theme = "light"
            start_fullscreen = true
            keep_in_tray = true
            [window]
            width = 1600
            height = 900
            x = -1920
            y = 40
            maximized = true
            monitor = "\\\\.\\DISPLAY2"
            "#,
        );
        assert_eq!(s.general.language, LanguageSetting::Code("pt-BR".into()));
        assert_eq!(s.general.theme, ThemeSetting::Light);
        assert!(s.general.start_fullscreen && s.general.keep_in_tray);
        assert_eq!(
            s.window,
            WindowSettings {
                width: 1600,
                height: 900,
                x: Some(-1920),
                y: Some(40),
                maximized: true,
                monitor: Some(r"\\.\DISPLAY2".into())
            }
        );
    }

    #[test]
    fn invalid_values_take_their_default_one_by_one() {
        let s = read(
            r#"
            [general]
            language = 5
            theme = true
            start_fullscreen = "yes"
            keep_in_tray = true
            [window]
            width = "wide"
            height = -3
            x = 1.5
            y = 99999999999
            maximized = 1
            monitor = 7
            "#,
        );
        // The one valid value still applies.
        assert!(s.general.keep_in_tray);
        let mut expected = Settings::default();
        expected.general.keep_in_tray = true;
        assert_eq!(s, expected);
    }

    #[test]
    fn categories_of_the_wrong_type_are_ignored() {
        assert_eq!(read("general = 3\nwindow = \"big\""), Settings::default());
    }

    #[test]
    fn unknown_language_and_theme_become_auto() {
        let s = read("[general]\nlanguage = \"fr\"\ntheme = \"sepia\"");
        assert_eq!(s.general.language, LanguageSetting::Auto);
        assert_eq!(s.general.theme, ThemeSetting::Auto);
        assert_eq!(
            read("[general]\nlanguage = \"auto\"").general.language,
            LanguageSetting::Auto
        );
        assert_eq!(read("[general]\ntheme = \"dark\"").general.theme, ThemeSetting::Dark);
    }

    #[test]
    fn size_below_minimum_becomes_the_minimum() {
        let s = read("[window]\nwidth = 100\nheight = 0");
        assert_eq!((s.window.width, s.window.height), (MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn higher_version_is_read_leniently_and_kept() {
        let mut table = parse("version = 7\n[general]\ntheme = \"dark\"\nnew_key = 1");
        let s = Settings::read(&table, LANGUAGES);
        assert_eq!(s.general.theme, ThemeSetting::Dark);
        s.write_into(&mut table);
        assert_eq!(table["version"].as_integer(), Some(7));
        assert_eq!(table["general"]["new_key"].as_integer(), Some(1));
    }

    #[test]
    fn unknown_keys_and_tables_survive_a_round_trip() {
        let text = r#"
            version = 1
            top = "kept"
            [general]
            theme = "light"
            future_option = [1, 2]
            [window]
            width = 800
            later = { a = 1 }
            [future]
            name = "kept too"
            [future.nested]
            deep = true
        "#;
        let mut table = parse(text);
        let mut s = Settings::read(&table, LANGUAGES);
        s.general.theme = ThemeSetting::Dark;
        s.window.x = Some(10);
        s.write_into(&mut table);
        let written: Table = toml::to_string(&table).unwrap().parse().unwrap();
        assert_eq!(written["top"].as_str(), Some("kept"));
        assert_eq!(written["general"]["future_option"].as_array().map(Vec::len), Some(2));
        assert_eq!(written["window"]["later"]["a"].as_integer(), Some(1));
        assert_eq!(written["future"]["name"].as_str(), Some("kept too"));
        assert_eq!(written["future"]["nested"]["deep"].as_bool(), Some(true));
        let again = Settings::read(&written, LANGUAGES);
        assert_eq!(again, s);
    }

    #[test]
    fn write_into_an_empty_table() {
        let mut table = Table::new();
        Settings::default().write_into(&mut table);
        assert_eq!(table["version"].as_integer(), Some(VERSION));
        assert_eq!(table["general"]["language"].as_str(), Some("auto"));
        assert_eq!(table["general"]["theme"].as_str(), Some("auto"));
        assert_eq!(table["window"]["width"].as_integer(), Some(1280));
        // Absent position and monitor are not written.
        assert!(table["window"].get("x").is_none());
        assert!(table["window"].get("monitor").is_none());
    }

    #[test]
    fn cleared_position_is_removed() {
        let mut table = parse("[window]\nx = 5\ny = 6\nmonitor = \"A\"");
        Settings::default().write_into(&mut table);
        assert!(table["window"].get("x").is_none());
        assert!(table["window"].get("y").is_none());
        assert!(table["window"].get("monitor").is_none());
    }
}
