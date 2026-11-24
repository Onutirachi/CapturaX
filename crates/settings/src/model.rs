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
/// Entries kept in `audio_devices` (preview audio, contracts/settings-file.md rule 3).
pub const AUDIO_DEVICES_LIMIT: usize = 32;
/// The preview's volume when none is stored or it is not a number (preview audio FR-012).
pub const DEFAULT_VOLUME: u8 = 80;
/// The ambient light's intensity in percent when none is stored or it is not an integer
/// (preview extras FR-001a).
pub const DEFAULT_AMBIENT_INTENSITY: u8 = 100;
/// `[screenshot] folder` for the system's Pictures folder plus `CapturaX` (screenshot FR-020).
pub const DEFAULT_FOLDER: &str = "default";
/// The screenshot key when none is stored (screenshot FR-012).
pub const DEFAULT_SCREENSHOT_KEY: &str = "Ctrl+Shift+F8";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    pub general: General,
    pub window: WindowSettings,
    pub preview: Preview,
    pub overlay: Overlay,
    /// Choices per device id, most recently used first.
    pub devices: Vec<(String, DeviceChoices)>,
    pub audio: Audio,
    /// Audio endpoint flags per endpoint id, most recently used first.
    pub audio_devices: Vec<(String, AudioDevice)>,
    pub screenshot: Screenshot,
    pub hotkeys: Hotkeys,
}

/// The `[screenshot]` table (screenshot contracts/settings-file.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screenshot {
    /// [`DEFAULT_FOLDER`] or an absolute path; anything else reads as the default (rule 1).
    pub folder: String,
}

impl Default for Screenshot {
    fn default() -> Self {
        Self {
            folder: DEFAULT_FOLDER.into(),
        }
    }
}

/// The `[hotkeys]` table (screenshot contracts/settings-file.md): each action's key as text, read
/// by the app (`Hotkey::parse`), which also checks that it is valid and allowed and that no two
/// actions share a key (rules 2 and 3). `""` is no key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkeys {
    pub screenshot: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            screenshot: DEFAULT_SCREENSHOT_KEY.into(),
        }
    }
}

/// The `[audio]` table (preview audio data-model.md "Output choice", "Volume").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audio {
    /// `"default"` or an output endpoint id, read by the audio crate (`OutputChoice::parse`). An
    /// id that is not present now is kept (rule 1).
    pub output: String,
    /// 0–100.
    pub volume: u8,
    pub muted: bool,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            output: "default".into(),
            volume: DEFAULT_VOLUME,
            muted: false,
        }
    }
}

/// An `[audio_devices."<id>"]` entry (preview audio data-model.md "Exclusive flags").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioDevice {
    /// For people reading the file; not used to match.
    pub name: String,
    /// Windows only; ignored on Linux. Entries are written only when true (rule 3).
    pub exclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct General {
    pub language: LanguageSetting,
    pub theme: ThemeSetting,
    pub start_fullscreen: bool,
    pub keep_in_tray: bool,
    /// The corner of the video area the in-app notices show in (device preview FR-032).
    pub notification_position: NotificationPosition,
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

/// A corner of the video area (device preview FR-032).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationPosition {
    TopLeft,
    TopRight,
    #[default]
    BottomLeft,
    BottomRight,
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
    /// The ambient light around the video (preview extras FR-001); applies to every device.
    pub ambient: bool,
    /// The ambient light's intensity in percent, 0 to 100 (preview extras FR-001a).
    pub ambient_intensity: u8,
    /// The preview is hidden and no device is opened (preview extras FR-011).
    pub hidden: bool,
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
            ambient: false,
            ambient_intensity: DEFAULT_AMBIENT_INTENSITY,
            hidden: false,
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
    /// Only while the ambient light is on, fading with it (preview extras FR-005).
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
    pub mode: ModeDisplay,
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

/// The mode bubble: the size alone or with its name and aspect ratio tags (FR-017a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModeDisplay {
    Size,
    SizeName,
    SizeAspect,
    #[default]
    SizeNameAspect,
}

/// A `[devices."<id>"]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceChoices {
    /// For people reading the file; not used to match.
    pub name: String,
    /// The mode choice as text (`auto-max-fps`, `1920x1080@60`…), read by the capture crate;
    /// empty when none was chosen. An unknown value is kept and opens the default mode (rule 2).
    pub mode: String,
    pub color_range: ColorRange,
    /// The audio choice as text (`"auto"`, `"none"` or an input endpoint id), read by the audio
    /// crate (`AudioChoice::parse`). A missing or empty value is `"auto"` (preview audio rule 1).
    pub audio: String,
}

impl Default for DeviceChoices {
    fn default() -> Self {
        Self {
            name: String::new(),
            mode: String::new(),
            color_range: ColorRange::default(),
            audio: "auto".into(),
        }
    }
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

text_values!(NotificationPosition {
    TopLeft = "top-left",
    TopRight = "top-right",
    BottomLeft = "bottom-left",
    BottomRight = "bottom-right"
});
text_values!(Presentation { LowestLatency = "lowest-latency", FollowMonitor = "follow-monitor" });
text_values!(Background { Theme = "theme", Black = "black" });
text_values!(Shadow { Always = "always", Ambient = "ambient", Off = "off" });
text_values!(LatencyDisplay { Measured = "measured", MeasuredEstimated = "measured+estimated", Total = "total" });
text_values!(FpsDisplay { Presented = "presented", PresentedNominal = "presented/nominal", Nominal = "nominal" });
text_values!(ModeDisplay {
    Size = "size",
    SizeName = "size+name",
    SizeAspect = "size+aspect",
    SizeNameAspect = "size+name+aspect"
});
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

    /// The flags stored for an audio endpoint.
    pub fn audio_device(&self, id: &str) -> Option<&AudioDevice> {
        self.audio_devices.iter().find(|(key, _)| key == id).map(|(_, d)| d)
    }

    /// The flags of an audio endpoint, created with defaults when missing, moved to the front
    /// (most recently used) with its name updated. Keeps at most [`AUDIO_DEVICES_LIMIT`] entries.
    pub fn touch_audio_device(&mut self, id: &str, name: &str) -> &mut AudioDevice {
        let entry = match self.audio_devices.iter().position(|(key, _)| key == id) {
            Some(index) => self.audio_devices.remove(index),
            None => (id.to_string(), AudioDevice::default()),
        };
        self.audio_devices.insert(0, entry);
        self.audio_devices.truncate(AUDIO_DEVICES_LIMIT);
        let device = &mut self.audio_devices[0].1;
        device.name = name.to_string();
        device
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
        let audio = table.get("audio").and_then(Value::as_table);
        let screenshot = table.get("screenshot").and_then(Value::as_table);
        let hotkeys = table.get("hotkeys").and_then(Value::as_table);
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
                notification_position: NotificationPosition::parse(
                    get(general, "notification_position").and_then(Value::as_str),
                ),
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
                // Rule 1 (preview extras): missing or not a boolean reads as off.
                ambient: flag(preview, "ambient", false),
                ambient_intensity: read_ambient_intensity(get(preview, "ambient_intensity")),
                // Rule 1 (preview extras): missing or not a boolean reads as shown.
                hidden: flag(preview, "hidden", false),
            },
            overlay: Overlay {
                latency: LatencyDisplay::parse(get(overlay, "latency").and_then(Value::as_str)),
                fps: FpsDisplay::parse(get(overlay, "fps").and_then(Value::as_str)),
                mode: ModeDisplay::parse(get(overlay, "mode").and_then(Value::as_str)),
            },
            devices: read_devices(table.get("devices").and_then(Value::as_table)),
            audio: Audio {
                output: get(audio, "output")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map_or(defaults.audio.output, str::to_string),
                volume: read_volume(get(audio, "volume")),
                muted: flag(audio, "muted", defaults.audio.muted),
            },
            audio_devices: read_audio_devices(table.get("audio_devices").and_then(Value::as_table)),
            screenshot: Screenshot {
                folder: read_folder(get(screenshot, "folder")),
            },
            hotkeys: Hotkeys {
                screenshot: get(hotkeys, "screenshot")
                    .and_then(Value::as_str)
                    .map_or(defaults.hotkeys.screenshot, str::to_string),
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
        general.insert(
            "notification_position".into(),
            Value::String(self.general.notification_position.as_str().into()),
        );

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
        preview.insert("ambient".into(), Value::Boolean(p.ambient));
        preview.insert(
            "ambient_intensity".into(),
            Value::Integer(p.ambient_intensity.min(100).into()),
        );
        preview.insert("hidden".into(), Value::Boolean(p.hidden));

        let overlay = category(table, "overlay");
        overlay.insert("latency".into(), Value::String(self.overlay.latency.as_str().into()));
        overlay.insert("fps".into(), Value::String(self.overlay.fps.as_str().into()));
        overlay.insert("mode".into(), Value::String(self.overlay.mode.as_str().into()));

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
            entry.insert("audio".into(), Value::String(choices.audio.clone()));
            devices.insert(id.clone(), Value::Table(entry));
        }
        table.insert("devices".into(), Value::Table(devices));

        let a = &self.audio;
        let audio = category(table, "audio");
        audio.insert("output".into(), Value::String(a.output.clone()));
        audio.insert("volume".into(), Value::Integer(a.volume.min(100).into()));
        audio.insert("muted".into(), Value::Boolean(a.muted));

        // Only exclusive or chosen entries are written, most recently used first, at most the
        // limit (rule 3); `exclusive` only when true; the unknown keys of the kept ones survive.
        let mut old = match table.remove("audio_devices") {
            Some(Value::Table(old)) => old,
            _ => Table::new(),
        };
        let chosen = |id: &str| a.output == id || self.devices.iter().any(|(_, d)| d.audio == id);
        let mut audio_devices = Table::new();
        for (id, device) in self
            .audio_devices
            .iter()
            .filter(|(id, d)| d.exclusive || chosen(id))
            .take(AUDIO_DEVICES_LIMIT)
        {
            let mut entry = match old.remove(id) {
                Some(Value::Table(entry)) => entry,
                _ => Table::new(),
            };
            entry.insert("name".into(), Value::String(device.name.clone()));
            if device.exclusive {
                entry.insert("exclusive".into(), Value::Boolean(true));
            } else {
                entry.remove("exclusive");
            }
            audio_devices.insert(id.clone(), Value::Table(entry));
        }
        table.insert("audio_devices".into(), Value::Table(audio_devices));

        let screenshot = category(table, "screenshot");
        screenshot.insert("folder".into(), Value::String(self.screenshot.folder.clone()));
        let hotkeys = category(table, "hotkeys");
        hotkeys.insert("screenshot".into(), Value::String(self.hotkeys.screenshot.clone()));
    }
}

/// `[screenshot] folder`: [`DEFAULT_FOLDER`] or an absolute path; a missing, empty, relative or
/// non-text value reads as the default (rule 1, logged when present).
fn read_folder(value: Option<&Value>) -> String {
    match value {
        None => DEFAULT_FOLDER.into(),
        Some(Value::String(text)) if text == DEFAULT_FOLDER => DEFAULT_FOLDER.into(),
        Some(Value::String(text)) if std::path::Path::new(text).is_absolute() => text.clone(),
        Some(other) => {
            log::warn!("screenshot folder {other} is not an absolute path; using the default");
            DEFAULT_FOLDER.into()
        }
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
                    audio: text("audio")
                        .filter(|text| !text.is_empty())
                        .map_or_else(|| DeviceChoices::default().audio, str::to_string),
                },
            ))
        })
        .collect()
}

/// `[audio] volume`: clamped to 0–100; a non-number takes [`DEFAULT_VOLUME`] (rule 2).
fn read_volume(value: Option<&Value>) -> u8 {
    let volume = match value {
        Some(Value::Integer(v)) => (*v).clamp(0, 100),
        Some(Value::Float(v)) if !v.is_nan() => v.round().clamp(0.0, 100.0) as i64,
        _ => return DEFAULT_VOLUME,
    };
    u8::try_from(volume).unwrap_or(DEFAULT_VOLUME)
}

/// `[preview] ambient_intensity`: clamped to 0–100; a missing or non-integer value takes
/// [`DEFAULT_AMBIENT_INTENSITY`] (preview extras rule 2).
fn read_ambient_intensity(value: Option<&Value>) -> u8 {
    match value {
        Some(Value::Integer(v)) => u8::try_from((*v).clamp(0, 100)).unwrap_or(DEFAULT_AMBIENT_INTENSITY),
        _ => DEFAULT_AMBIENT_INTENSITY,
    }
}

/// The `[audio_devices."<id>"]` entries in file order (most recently used first); entries that
/// are not tables are skipped, a missing or invalid `exclusive` is false.
fn read_audio_devices(devices: Option<&Table>) -> Vec<(String, AudioDevice)> {
    let Some(devices) = devices else {
        return Vec::new();
    };
    devices
        .iter()
        .filter_map(|(id, entry)| {
            let entry = entry.as_table()?;
            Some((
                id.clone(),
                AudioDevice {
                    name: entry
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    exclusive: entry.get("exclusive").and_then(Value::as_bool).unwrap_or(false),
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
        assert_eq!(s.general.notification_position, NotificationPosition::BottomLeft);
    }

    #[test]
    fn notification_position_values() {
        let position = |text: &str| {
            read(&format!("[general]\nnotification_position = {text}"))
                .general
                .notification_position
        };
        assert_eq!(position("\"top-left\""), NotificationPosition::TopLeft);
        assert_eq!(position("\"top-right\""), NotificationPosition::TopRight);
        assert_eq!(position("\"bottom-left\""), NotificationPosition::BottomLeft);
        assert_eq!(position("\"bottom-right\""), NotificationPosition::BottomRight);
        // Unknown or of the wrong type: the default (rule 1).
        assert_eq!(position("\"middle\""), NotificationPosition::BottomLeft);
        assert_eq!(position("3"), NotificationPosition::BottomLeft);

        let mut s = Settings::default();
        s.general.notification_position = NotificationPosition::TopRight;
        let mut table = Table::new();
        s.write_into(&mut table);
        assert_eq!(table["general"]["notification_position"].as_str(), Some("top-right"));
        assert_eq!(Settings::read(&table, LANGUAGES), s);
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
        assert_eq!(s.overlay.mode, ModeDisplay::SizeNameAspect);
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
            mode = "size+aspect"
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
                fps: FpsDisplay::Nominal,
                mode: ModeDisplay::SizeAspect,
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
            mode = "size+tags"
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
        s.overlay.mode = ModeDisplay::Size;
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
    fn ambient_reads_false_unless_a_boolean() {
        assert!(!Preview::default().ambient);
        assert!(!read("").preview.ambient);
        assert!(!read("[preview]").preview.ambient);
        assert!(read("[preview]\nambient = true").preview.ambient);
        assert!(!read("[preview]\nambient = false").preview.ambient);
        for value in ["\"true\"", "1", "\"yes\"", "[true]"] {
            assert!(
                !read(&format!("[preview]\nambient = {value}")).preview.ambient,
                "{value}"
            );
        }
        // A wrong value leaves the other preview keys alone.
        let s = read("[preview]\nambient = 1\nwatermark = false");
        assert!(!s.preview.ambient);
        assert!(!s.preview.watermark);
    }

    #[test]
    fn ambient_round_trips_with_unknown_keys() {
        let mut s = Settings::default();
        s.preview.ambient = true;
        let mut table = parse("[preview]\nlater = true\n[future]\nkey = 1");
        s.write_into(&mut table);
        let written: Table = toml::to_string(&table).unwrap().parse().unwrap();
        assert_eq!(written["preview"]["ambient"].as_bool(), Some(true));
        assert_eq!(written["preview"]["later"].as_bool(), Some(true));
        assert_eq!(written["future"]["key"].as_integer(), Some(1));
        assert_eq!(Settings::read(&written, LANGUAGES), s);

        s.preview.ambient = false;
        s.write_into(&mut table);
        assert_eq!(table["preview"]["ambient"].as_bool(), Some(false));
        assert_eq!(Settings::read(&table, LANGUAGES), s);
    }

    #[test]
    fn hidden_reads_false_unless_a_boolean() {
        assert!(!Preview::default().hidden);
        assert!(!read("").preview.hidden);
        assert!(!read("[preview]").preview.hidden);
        assert!(
            read(
                "[preview]
hidden = true"
            )
            .preview
            .hidden
        );
        assert!(
            !read(
                "[preview]
hidden = false"
            )
            .preview
            .hidden
        );
        for value in ["\"true\"", "1", "\"yes\"", "[true]"] {
            assert!(
                !read(&format!(
                    "[preview]
hidden = {value}"
                ))
                .preview
                .hidden,
                "{value}"
            );
        }
        // A wrong value leaves the other preview keys alone.
        let s = read(
            "[preview]
hidden = 1
ambient = true",
        );
        assert!(!s.preview.hidden);
        assert!(s.preview.ambient);
    }

    #[test]
    fn hidden_round_trips_with_unknown_keys() {
        let mut s = Settings::default();
        s.preview.hidden = true;
        let mut table = parse(
            "[preview]
later = true
[future]
key = 1",
        );
        s.write_into(&mut table);
        let written: Table = toml::to_string(&table).unwrap().parse().unwrap();
        assert_eq!(written["preview"]["hidden"].as_bool(), Some(true));
        assert_eq!(written["preview"]["later"].as_bool(), Some(true));
        assert_eq!(written["future"]["key"].as_integer(), Some(1));
        assert_eq!(Settings::read(&written, LANGUAGES), s);

        s.preview.hidden = false;
        s.write_into(&mut table);
        assert_eq!(table["preview"]["hidden"].as_bool(), Some(false));
        assert_eq!(Settings::read(&table, LANGUAGES), s);
    }

    #[test]
    fn ambient_intensity_reads_an_integer_clamped() {
        let intensity = |text: &str| {
            read(&format!(
                "[preview]
ambient_intensity = {text}"
            ))
            .preview
            .ambient_intensity
        };
        assert_eq!(Preview::default().ambient_intensity, DEFAULT_AMBIENT_INTENSITY);
        assert_eq!(read("").preview.ambient_intensity, 100);
        assert_eq!(read("[preview]").preview.ambient_intensity, 100);
        assert_eq!(intensity("0"), 0);
        assert_eq!(intensity("35"), 35);
        assert_eq!(intensity("100"), 100);
        assert_eq!(intensity("-5"), 0);
        assert_eq!(intensity("250"), 100);
        assert_eq!(intensity("99999999999"), 100);
        for value in ["42.6", "nan", "true", "\"50\"", "[50]"] {
            assert_eq!(intensity(value), 100, "{value}");
        }
        // A wrong value leaves the other preview keys alone.
        let s = read(
            "[preview]
ambient_intensity = \"x\"
ambient = true",
        );
        assert_eq!(s.preview.ambient_intensity, 100);
        assert!(s.preview.ambient);
    }

    #[test]
    fn ambient_intensity_round_trips() {
        let mut s = Settings::default();
        s.preview.ambient_intensity = 40;
        let mut table = parse(
            "[preview]
later = true",
        );
        s.write_into(&mut table);
        let written: Table = toml::to_string(&table).unwrap().parse().unwrap();
        assert_eq!(written["preview"]["ambient_intensity"].as_integer(), Some(40));
        assert_eq!(written["preview"]["later"].as_bool(), Some(true));
        assert_eq!(Settings::read(&written, LANGUAGES), s);

        s.preview.ambient_intensity = 0;
        s.write_into(&mut table);
        assert_eq!(table["preview"]["ambient_intensity"].as_integer(), Some(0));
        assert_eq!(Settings::read(&table, LANGUAGES), s);
    }

    #[test]
    fn cleared_position_is_removed() {
        let mut table = parse("[window]\nx = 5\ny = 6\nmonitor = \"A\"");
        Settings::default().write_into(&mut table);
        assert!(table["window"].get("x").is_none());
        assert!(table["window"].get("y").is_none());
        assert!(table["window"].get("monitor").is_none());
    }

    /// An endpoint id as contracts/settings-file.md (preview audio) shows them.
    const EZCAP_INPUT: &str = "{0.0.1.00000000}.{6a1c0000-0000-0000-0000-000000000001}";

    #[test]
    fn audio_defaults() {
        let s = read("[audio]\n[audio_devices]\n[devices.\"mf:a\"]");
        assert_eq!(s.audio, Audio::default());
        assert_eq!(
            (s.audio.output.as_str(), s.audio.volume, s.audio.muted),
            ("default", 80, false)
        );
        assert!(s.audio_devices.is_empty());
        assert_eq!(s.device("mf:a").unwrap().audio, "auto");
        assert_eq!(DeviceChoices::default().audio, "auto");
        assert_eq!(read(""), Settings::default());

        let mut table = Table::new();
        Settings::default().write_into(&mut table);
        assert_eq!(table["audio"]["output"].as_str(), Some("default"));
        assert_eq!(table["audio"]["volume"].as_integer(), Some(80));
        assert_eq!(table["audio"]["muted"].as_bool(), Some(false));
        assert!(table["audio_devices"].as_table().unwrap().is_empty());
    }

    #[test]
    fn audio_values() {
        let s = read(&format!(
            r#"
            [audio]
            output = "{{0.0.0.00000000}}.{{g533}}"
            volume = 35
            muted = true
            [devices."mf:a"]
            audio = "none"
            [devices."mf:b"]
            audio = "{EZCAP_INPUT}"
            [audio_devices."{EZCAP_INPUT}"]
            name = "Digital Audio Interface (ezcap Live Gamer Ultra)"
            exclusive = true
            "#
        ));
        assert_eq!(
            s.audio,
            Audio {
                output: "{0.0.0.00000000}.{g533}".into(),
                volume: 35,
                muted: true
            }
        );
        assert_eq!(s.device("mf:a").unwrap().audio, "none");
        assert_eq!(s.device("mf:b").unwrap().audio, EZCAP_INPUT);
        let ezcap = s.audio_device(EZCAP_INPUT).unwrap();
        assert_eq!(ezcap.name, "Digital Audio Interface (ezcap Live Gamer Ultra)");
        assert!(ezcap.exclusive);
    }

    #[test]
    fn audio_invalid_values_take_their_default() {
        let s = read(
            r#"
            [audio]
            output = 3
            volume = "loud"
            muted = "yes"
            [devices."mf:a"]
            audio = 1
            [devices."mf:b"]
            audio = ""
            [audio_devices]
            broken = 1
            [audio_devices.x]
            exclusive = "yes"
            "#,
        );
        assert_eq!(s.audio, Audio::default());
        assert_eq!(s.device("mf:a").unwrap().audio, "auto");
        assert_eq!(s.device("mf:b").unwrap().audio, "auto");
        assert_eq!(s.audio_devices.len(), 1);
        assert!(!s.audio_device("x").unwrap().exclusive);
        assert_eq!(read("[audio]\noutput = \"\"").audio.output, "default");
        assert_eq!(read("audio = 5").audio, Audio::default());
    }

    #[test]
    fn audio_ids_not_present_are_kept() {
        // Any text other than the keywords is an id: kept even when no such device is present
        // now (rule 1); the audio crate treats it as missing.
        let mut table = parse("[audio]\noutput = \"gone\"\n[devices.\"mf:a\"]\naudio = \"gone too\"");
        let s = Settings::read(&table, LANGUAGES);
        assert_eq!(s.audio.output, "gone");
        assert_eq!(s.device("mf:a").unwrap().audio, "gone too");
        s.write_into(&mut table);
        assert_eq!(table["audio"]["output"].as_str(), Some("gone"));
        assert_eq!(table["devices"]["mf:a"]["audio"].as_str(), Some("gone too"));
    }

    #[test]
    fn volume_outside_the_range_is_clamped() {
        let volume = |text: &str| read(&format!("[audio]\nvolume = {text}")).audio.volume;
        assert_eq!(volume("0"), 0);
        assert_eq!(volume("100"), 100);
        assert_eq!(volume("-5"), 0);
        assert_eq!(volume("250"), 100);
        assert_eq!(volume("99999999999"), 100);
        assert_eq!(volume("42.6"), 43);
        assert_eq!(volume("1e9"), 100);
        assert_eq!(volume("nan"), DEFAULT_VOLUME);
        assert_eq!(volume("true"), DEFAULT_VOLUME);
        assert_eq!(volume("[50]"), DEFAULT_VOLUME);
    }

    #[test]
    fn only_exclusive_audio_devices_are_written() {
        let mut s = Settings::default();
        s.touch_audio_device("a", "A").exclusive = true;
        s.touch_audio_device("b", "B");
        s.touch_audio_device("c", "C").exclusive = true;
        let mut table = Table::new();
        s.write_into(&mut table);
        let written = table["audio_devices"].as_table().unwrap();
        let ids: Vec<&str> = written.keys().map(String::as_str).collect();
        assert_eq!(ids, ["c", "a"], "most recently used first, `b` not written");
        assert_eq!(written["c"]["name"].as_str(), Some("C"));
        assert_eq!(written["c"]["exclusive"].as_bool(), Some(true));

        // Turning exclusive off removes the entry on the next save.
        s.touch_audio_device("a", "A").exclusive = false;
        s.write_into(&mut table);
        let ids: Vec<&str> = table["audio_devices"]
            .as_table()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(ids, ["c"]);
    }

    #[test]
    fn chosen_audio_devices_keep_their_name() {
        let mut s = Settings::default();
        s.touch_audio_device("out", "1 - ARZOPA");
        s.touch_audio_device("in", "Microphone (C920)");
        s.touch_audio_device("other", "Other");
        s.audio.output = "out".into();
        s.touch_device("mf:a", "ezcap").audio = "in".into();
        let mut table = Table::new();
        s.write_into(&mut table);
        let written = table["audio_devices"].as_table().unwrap();
        let ids: Vec<&str> = written.keys().map(String::as_str).collect();
        assert_eq!(ids, ["in", "out"], "`other` is neither chosen nor exclusive");
        assert_eq!(written["out"]["name"].as_str(), Some("1 - ARZOPA"));
        assert!(written["out"].get("exclusive").is_none(), "written only when true");
        // Read back, the name is there for a chosen output missing at start.
        let back = read(&toml::to_string(&table).unwrap());
        assert_eq!(back.audio_device("out").unwrap().name, "1 - ARZOPA");
    }

    #[test]
    fn thirty_three_audio_devices_keep_thirty_two() {
        let mut s = Settings::default();
        for i in 0..33 {
            s.touch_audio_device(&format!("e{i}"), "E").exclusive = true;
        }
        assert_eq!(s.audio_devices.len(), AUDIO_DEVICES_LIMIT);
        assert!(s.audio_device("e0").is_none(), "the least recently used is dropped");
        s.touch_audio_device("e1", "E");
        assert_eq!(s.audio_devices[0].0, "e1");

        // The same on write, for a file that already holds 33 entries.
        let entries: String = (0..33)
            .map(|i| format!("[audio_devices.e{i}]\nname = \"E\"\nexclusive = true\n"))
            .collect();
        let mut table = parse(&entries);
        let read_back = Settings::read(&table, LANGUAGES);
        assert_eq!(read_back.audio_devices.len(), 33);
        read_back.write_into(&mut table);
        let written = table["audio_devices"].as_table().unwrap();
        assert_eq!(written.len(), AUDIO_DEVICES_LIMIT);
        assert!(written.get("e32").is_none());
        assert_eq!(written.keys().next().map(String::as_str), Some("e0"));
    }

    #[test]
    fn audio_round_trip_with_quoted_ids_and_unknown_keys() {
        let mut s = Settings {
            audio: Audio {
                output: "{0.0.0.00000000}.{g533}".into(),
                volume: 0,
                muted: true,
            },
            ..Default::default()
        };
        s.touch_device(C920, "HD Pro Webcam C920").audio = EZCAP_INPUT.into();
        s.touch_audio_device(EZCAP_INPUT, "Digital Audio Interface (ezcap Live Gamer Ultra)")
            .exclusive = true;
        let mut table = parse(&format!(
            "[audio]\nlater = 1\n[audio_devices.\"{EZCAP_INPUT}\"]\nexclusive = true\nfuture = \"kept\""
        ));
        s.write_into(&mut table);
        let text = toml::to_string(&table).unwrap();
        assert!(text.contains("[audio_devices.\"{0.0.1."), "{text}");
        let written: Table = text.parse().unwrap();
        assert_eq!(written["audio"]["later"].as_integer(), Some(1));
        assert_eq!(written["audio_devices"][EZCAP_INPUT]["future"].as_str(), Some("kept"));
        assert_eq!(Settings::read(&written, LANGUAGES), s);
    }

    #[test]
    fn screenshot_and_hotkeys_defaults() {
        let s = read("");
        assert_eq!(s.screenshot.folder, "default");
        assert_eq!(s.hotkeys.screenshot, "Ctrl+Shift+F8");
        let mut table = Table::new();
        s.write_into(&mut table);
        assert_eq!(table["screenshot"]["folder"].as_str(), Some("default"));
        assert_eq!(table["hotkeys"]["screenshot"].as_str(), Some("Ctrl+Shift+F8"));
    }

    #[test]
    fn screenshot_folder_rule() {
        let folder = |value: &str| read(&format!("[screenshot]\nfolder = {value}")).screenshot.folder;
        // Absolute on every system.
        let absolute = std::env::temp_dir().join("Shots").display().to_string();
        assert_eq!(folder(&format!("'{absolute}'")), absolute);
        assert_eq!(folder("\"default\""), "default");
        // Empty, relative or not text: the default (rule 1).
        assert_eq!(folder("\"\""), "default");
        assert_eq!(folder("\"Shots\""), "default");
        assert_eq!(folder("\"./Shots\""), "default");
        assert_eq!(folder("42"), "default");
        assert_eq!(folder("[\"/a\"]"), "default");
        // Not a table: the default.
        assert_eq!(read("screenshot = 3").screenshot.folder, "default");
    }

    #[test]
    fn hotkey_text_is_kept_for_the_app() {
        let key = |value: &str| read(&format!("[hotkeys]\nscreenshot = {value}")).hotkeys.screenshot;
        assert_eq!(key("\"Ctrl+Shift+F9\""), "Ctrl+Shift+F9");
        // Cleared: kept as no key (rule 2).
        assert_eq!(key("\"\""), "");
        // Invalid text is kept; the app checks it and uses the default.
        assert_eq!(key("\"Ctrl+Nope\""), "Ctrl+Nope");
        // Not text: the default.
        assert_eq!(key("8"), "Ctrl+Shift+F8");
        assert_eq!(
            read("[hotkeys]\nrecord = \"Ctrl+F10\"").hotkeys.screenshot,
            "Ctrl+Shift+F8"
        );
    }

    #[test]
    fn screenshot_and_hotkeys_round_trip_with_unknown_keys() {
        let mut s = Settings::default();
        s.screenshot.folder = std::env::temp_dir().join("Games").display().to_string();
        s.hotkeys.screenshot = String::new();
        let mut table = parse("[hotkeys]\nrecord = \"Ctrl+F10\"\n[screenshot]\nformat = \"png\"");
        s.write_into(&mut table);
        let written: Table = toml::to_string(&table).unwrap().parse().unwrap();
        assert_eq!(written["hotkeys"]["record"].as_str(), Some("Ctrl+F10"));
        assert_eq!(written["hotkeys"]["screenshot"].as_str(), Some(""));
        assert_eq!(written["screenshot"]["format"].as_str(), Some("png"));
        assert_eq!(Settings::read(&written, LANGUAGES), s);
    }
}
