//! The settings and their lenient reading from a TOML table (data-model.md, contracts/settings-file.md).

use toml::{Table, Value};

/// Format version this build writes.
pub const VERSION: i64 = 1;
/// Smallest window, in logical pixels (FR-009).
pub const MIN_WIDTH: u32 = 640;
pub const MIN_HEIGHT: u32 = 360;
/// Device ids kept in `preview.recent` (device preview, contracts/settings-file.md).
pub const RECENT_LIMIT: usize = 8;
/// Device entries kept in `devices` (rule 3); the least recently used is dropped.
pub const DEVICES_LIMIT: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    pub general: General,
    pub window: WindowSettings,
    pub preview: Preview,
    pub overlay: Overlay,
    /// Choices per device id, most recently used first.
    pub devices: Vec<(String, DeviceChoices)>,
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

/// The `[preview]` table (device preview data-model.md "Settings additions").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The last device the user chose.
    pub device: Option<String>,
    /// Device ids, most recent first, at most [`RECENT_LIMIT`].
    pub recent: Vec<String>,
    pub presentation: Presentation,
    /// The ground around the video (F56).
    pub background: Background,
    /// The X in the video background (F57).
    pub watermark: bool,
    /// The shadow around the video (F58).
    pub shadow: Shadow,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            device: None,
            recent: Vec::new(),
            presentation: Presentation::default(),
            background: Background::default(),
            watermark: true,
            shadow: Shadow::default(),
        }
    }
}

/// The ground around the video (F56).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Background {
    /// The theme's deepest tone.
    #[default]
    Theme,
    Black,
}

/// When the video has a shadow around it (F58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shadow {
    #[default]
    Always,
    /// Only over the ambient light (spec #6); none until then.
    Ambient,
    Off,
}

/// How the preview meets a variable refresh monitor (F52).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Presentation {
    #[default]
    LowestLatency,
    FollowMonitor,
}

/// The `[overlay]` table: what the info bubbles show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Overlay {
    pub latency: LatencyDisplay,
    pub fps: FpsDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LatencyDisplay {
    Measured,
    #[default]
    MeasuredEstimated,
    Total,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FpsDisplay {
    Presented,
    #[default]
    PresentedNominal,
    Nominal,
}

/// A `[devices."<id>"]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceChoices {
    /// For people reading the file; not used to match.
    pub name: String,
    /// The mode choice as text (`auto-max-fps`, `1920x1080@60`…), read by the capture crate;
    /// empty when none was chosen. An unknown value is kept and opens the default mode (rule 2).
    pub mode: String,
    pub color_range: ColorRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorRange {
    #[default]
    Auto,
    Full,
    Limited,
}

/// Text values of the enums, both ways.
macro_rules! text_values {
    ($ty:ty { $($variant:ident = $text:literal),+ $(,)? }) => {
        impl $ty {
            /// The value in the settings file.
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            /// The value for a text; unknown text takes the default (rule 1).
            pub fn parse(text: Option<&str>) -> Self {
                match text { $(Some($text) => Self::$variant,)+ _ => Self::default() }
            }
        }
    };
}

text_values!(Presentation { LowestLatency = "lowest-latency", FollowMonitor = "follow-monitor" });
text_values!(Background { Theme = "theme", Black = "black" });
text_values!(Shadow { Always = "always", Ambient = "ambient", Off = "off" });
text_values!(LatencyDisplay { Measured = "measured", MeasuredEstimated = "measured+estimated", Total = "total" });
text_values!(FpsDisplay { Presented = "presented", PresentedNominal = "presented/nominal", Nominal = "nominal" });
text_values!(ColorRange { Auto = "auto", Full = "full", Limited = "limited" });

impl Settings {
    /// The choices stored for a device.
    pub fn device(&self, id: &str) -> Option<&DeviceChoices> {
        self.devices.iter().find(|(key, _)| key == id).map(|(_, c)| c)
    }

    /// The choices of a device, created with defaults when missing, moved to the front (most
    /// recently used) with its name updated. Keeps at most [`DEVICES_LIMIT`] entries.
    pub fn touch_device(&mut self, id: &str, name: &str) -> &mut DeviceChoices {
        let entry = match self.devices.iter().position(|(key, _)| key == id) {
            Some(index) => self.devices.remove(index),
            None => (id.to_string(), DeviceChoices::default()),
        };
        self.devices.insert(0, entry);
        self.devices.truncate(DEVICES_LIMIT);
        let choices = &mut self.devices[0].1;
        choices.name = name.to_string();
        choices
    }

    /// Records a device the user chose: `preview.device`, the front of `preview.recent` (at most
    /// [`RECENT_LIMIT`]) and its device entry.
    pub fn remember_device(&mut self, id: &str, name: &str) {
        self.preview.device = Some(id.to_string());
        self.preview.recent.retain(|r| r != id);
        self.preview.recent.insert(0, id.to_string());
        self.preview.recent.truncate(RECENT_LIMIT);
        self.touch_device(id, name);
    }

    /// Reads the known values from `table`, each on its own: a missing or invalid value takes its
    /// default, an unknown language or theme becomes automatic (rule 3). `known_languages` are
    /// the codes the app has.
    pub fn read(table: &Table, known_languages: &[&str]) -> Self {
        let defaults = Settings::default();
        let general = table.get("general").and_then(Value::as_table);
        let window = table.get("window").and_then(Value::as_table);
        let preview = table.get("preview").and_then(Value::as_table);
        let overlay = table.get("overlay").and_then(Value::as_table);
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
            preview: Preview {
                device: get(preview, "device").and_then(Value::as_str).map(str::to_string),
                recent: get(preview, "recent")
                    .and_then(Value::as_array)
                    .map(|ids| {
                        let mut recent: Vec<String> = Vec::new();
                        for id in ids.iter().filter_map(Value::as_str) {
                            if !recent.iter().any(|r| r == id) {
                                recent.push(id.to_string());
                            }
                        }
                        recent.truncate(RECENT_LIMIT);
                        recent
                    })
                    .unwrap_or_default(),
                presentation: Presentation::parse(get(preview, "presentation").and_then(Value::as_str)),
                background: Background::parse(get(preview, "background").and_then(Value::as_str)),
                watermark: flag(preview, "watermark", defaults.preview.watermark),
                shadow: Shadow::parse(get(preview, "shadow").and_then(Value::as_str)),
            },
            overlay: Overlay {
                latency: LatencyDisplay::parse(get(overlay, "latency").and_then(Value::as_str)),
                fps: FpsDisplay::parse(get(overlay, "fps").and_then(Value::as_str)),
            },
            devices: read_devices(table.get("devices").and_then(Value::as_table)),
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

        let p = &self.preview;
        let preview = category(table, "preview");
        set_or_remove(preview, "device", p.device.clone().map(Value::String));
        let recent = p.recent.iter().take(RECENT_LIMIT).cloned().map(Value::String).collect();
        preview.insert("recent".into(), Value::Array(recent));
        preview.insert("presentation".into(), Value::String(p.presentation.as_str().into()));
        preview.insert("background".into(), Value::String(p.background.as_str().into()));
        preview.insert("watermark".into(), Value::Boolean(p.watermark));
        preview.insert("shadow".into(), Value::String(p.shadow.as_str().into()));

        let overlay = category(table, "overlay");
        overlay.insert("latency".into(), Value::String(self.overlay.latency.as_str().into()));
        overlay.insert("fps".into(), Value::String(self.overlay.fps.as_str().into()));

        // Rebuilt in order, most recently used first: entries past the limit are dropped (rule 3)
        // and the unknown keys of the kept ones survive.
        let mut old = match table.remove("devices") {
            Some(Value::Table(old)) => old,
            _ => Table::new(),
        };
        let mut devices = Table::new();
        for (id, choices) in self.devices.iter().take(DEVICES_LIMIT) {
            let mut entry = match old.remove(id) {
                Some(Value::Table(entry)) => entry,
                _ => Table::new(),
            };
            entry.insert("name".into(), Value::String(choices.name.clone()));
            let mode = (!choices.mode.is_empty()).then(|| Value::String(choices.mode.clone()));
            set_or_remove(&mut entry, "mode", mode);
            entry.insert("color_range".into(), Value::String(choices.color_range.as_str().into()));
            devices.insert(id.clone(), Value::Table(entry));
        }
        table.insert("devices".into(), Value::Table(devices));
    }
}

/// The `[devices."<id>"]` entries in file order (most recently used first); entries that are not
/// tables are skipped.
fn read_devices(devices: Option<&Table>) -> Vec<(String, DeviceChoices)> {
    let Some(devices) = devices else {
        return Vec::new();
    };
    devices
        .iter()
        .filter_map(|(id, entry)| {
            let entry = entry.as_table()?;
            let text = |key: &str| entry.get(key).and_then(Value::as_str);
            Some((
                id.clone(),
                DeviceChoices {
                    name: text("name").unwrap_or_default().to_string(),
                    mode: text("mode").unwrap_or_default().to_string(),
                    color_range: ColorRange::parse(text("color_range")),
                },
            ))
        })
        .collect()
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

    /// The C920's id as contracts/settings-file.md shows ids: backslashes and `#`.
    const C920: &str =
        r"mf:\\?\usb#vid_046d&pid_082d&mi_00#d&41d17f6&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";

    #[test]
    fn device_preview_tables_default_when_empty() {
        let s = read("[preview]\n[overlay]\n[devices]");
        assert_eq!(s.preview, Preview::default());
        assert_eq!(s.preview.presentation, Presentation::LowestLatency);
        assert_eq!(
            (s.preview.background, s.preview.watermark, s.preview.shadow),
            (Background::Theme, true, Shadow::Always)
        );
        assert_eq!(s.overlay.latency, LatencyDisplay::MeasuredEstimated);
        assert_eq!(s.overlay.fps, FpsDisplay::PresentedNominal);
        assert!(s.devices.is_empty());
        assert_eq!(read(""), Settings::default());
    }

    #[test]
    fn device_preview_values() {
        let s = read(
            r#"
            [preview]
            device = "v4l2:32ed:3201:x@pci"
            recent = ["v4l2:32ed:3201:x@pci", "mf:a", "mf:a", 5]
            presentation = "follow-monitor"
            background = "black"
            watermark = false
            shadow = "always"
            [overlay]
            latency = "total"
            fps = "nominal"
            [devices."v4l2:32ed:3201:x@pci"]
            name = "ezcap"
            mode = "1920x1080@59.94"
            color_range = "full"
            [devices."mf:a"]
            name = "C920"
            "#,
        );
        assert_eq!(s.preview.device.as_deref(), Some("v4l2:32ed:3201:x@pci"));
        assert_eq!(s.preview.recent, ["v4l2:32ed:3201:x@pci", "mf:a"]);
        assert_eq!(s.preview.presentation, Presentation::FollowMonitor);
        assert_eq!(
            (s.preview.background, s.preview.watermark, s.preview.shadow),
            (Background::Black, false, Shadow::Always)
        );
        assert_eq!(
            s.overlay,
            Overlay {
                latency: LatencyDisplay::Total,
                fps: FpsDisplay::Nominal
            }
        );
        assert_eq!(s.devices.len(), 2);
        let ezcap = s.device("v4l2:32ed:3201:x@pci").unwrap();
        assert_eq!((ezcap.name.as_str(), ezcap.mode.as_str()), ("ezcap", "1920x1080@59.94"));
        assert_eq!(ezcap.color_range, ColorRange::Full);
        let c920 = s.device("mf:a").unwrap();
        assert_eq!((c920.mode.as_str(), c920.color_range), ("", ColorRange::Auto));
    }

    #[test]
    fn device_preview_invalid_values_take_their_default() {
        let s = read(
            r#"
            [preview]
            device = 3
            recent = "mf:a"
            presentation = "fast"
            background = "white"
            watermark = "yes"
            shadow = 2
            [overlay]
            latency = "exact"
            fps = 60
            [devices]
            broken = 1
            [devices."mf:a"]
            mode = "whatever"
            color_range = "studio"
            "#,
        );
        assert_eq!(s.preview, Preview::default());
        assert_eq!(s.overlay, Overlay::default());
        assert_eq!(s.devices.len(), 1);
        let a = s.device("mf:a").unwrap();
        // An unknown mode is kept as text (rule 2: replaced only when the user picks again).
        assert_eq!(a.mode, "whatever");
        assert_eq!(a.color_range, ColorRange::Auto);
    }

    #[test]
    fn recent_keeps_eight() {
        let ids: Vec<String> = (0..10).map(|i| format!("\"d{i}\"")).collect();
        let s = read(&format!("[preview]\nrecent = [{}]", ids.join(", ")));
        assert_eq!(s.preview.recent.len(), RECENT_LIMIT);
        let mut s = Settings::default();
        for i in 0..10 {
            s.remember_device(&format!("d{i}"), "D");
        }
        s.remember_device("d3", "D");
        assert_eq!(s.preview.recent.len(), RECENT_LIMIT);
        assert_eq!(&s.preview.recent[..2], ["d3", "d9"]);
        assert_eq!(s.preview.device.as_deref(), Some("d3"));
        assert_eq!(s.devices[0].0, "d3");
    }

    #[test]
    fn thirty_three_devices_keep_thirty_two() {
        let mut s = Settings::default();
        for i in 0..33 {
            s.touch_device(&format!("d{i}"), &format!("Device {i}"));
        }
        assert_eq!(s.devices.len(), DEVICES_LIMIT);
        assert_eq!(s.devices[0].0, "d32");
        assert!(s.device("d0").is_none(), "the least recently used is dropped");
        assert!(s.device("d1").is_some());

        // The same on write, for a file that already holds 33 entries.
        let entries: String = (0..33).map(|i| format!("[devices.d{i}]\nname = \"D\"\n")).collect();
        let mut table = parse(&entries);
        let read_back = Settings::read(&table, LANGUAGES);
        assert_eq!(read_back.devices.len(), 33);
        read_back.write_into(&mut table);
        let devices = table["devices"].as_table().unwrap();
        assert_eq!(devices.len(), DEVICES_LIMIT);
        assert!(devices.get("d32").is_none());
        assert_eq!(devices.keys().next().map(String::as_str), Some("d0"));
    }

    #[test]
    fn touch_moves_a_device_to_the_front() {
        let mut s = Settings::default();
        s.touch_device("a", "A").mode = "auto-max-fps".into();
        s.touch_device("b", "B");
        s.touch_device("a", "A renamed");
        let order: Vec<&str> = s.devices.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(order, ["a", "b"]);
        assert_eq!(s.device("a").unwrap().mode, "auto-max-fps");
        assert_eq!(s.device("a").unwrap().name, "A renamed");
    }

    #[test]
    fn device_tables_round_trip_with_quoted_ids_and_unknown_keys() {
        let mut s = Settings::default();
        s.remember_device(C920, "HD Pro Webcam C920");
        s.touch_device(C920, "HD Pro Webcam C920").mode = "1920x1080@30".into();
        s.overlay.fps = FpsDisplay::Presented;
        s.preview.presentation = Presentation::FollowMonitor;
        s.preview.background = Background::Black;
        s.preview.watermark = false;
        s.preview.shadow = Shadow::Off;
        s.devices.push((
            "mf:old".into(),
            DeviceChoices {
                name: "Old".into(),
                ..Default::default()
            },
        ));
        let mut table = parse("[devices.\"mf:old\"]\nname = \"Old\"\nfuture = 1\n[preview]\nlater = true");
        s.write_into(&mut table);
        let text = toml::to_string(&table).unwrap();
        assert!(text.contains("[devices.\"mf:"), "{text}");
        let written: Table = text.parse().unwrap();
        assert_eq!(written["preview"]["later"].as_bool(), Some(true));
        assert_eq!(written["devices"]["mf:old"]["future"].as_integer(), Some(1));
        let again = Settings::read(&written, LANGUAGES);
        assert_eq!(again, s);
        assert_eq!(again.device(C920).unwrap().mode, "1920x1080@30");
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
