//! The CapturaX settings file (contracts/settings-file.md of the app shell spec): the settings and
//! their lenient reading, loading and atomic saving, and the store that saves after each change.

pub mod file;
pub mod location;
mod model;
mod store;

pub use file::LoadOutcome;
pub use location::{FileTimes, Location, LocationKind};
pub use model::{
    AUDIO_DEVICES_LIMIT, Audio, AudioDevice, Background, ColorRange, DEFAULT_VOLUME, DEVICES_LIMIT, DeviceChoices,
    FpsDisplay, General, LanguageSetting, LatencyDisplay, MIN_HEIGHT, MIN_WIDTH, ModeDisplay, NotificationPosition,
    Overlay, Presentation, Preview, RECENT_LIMIT, Settings, Shadow, ThemeSetting, VERSION, WindowSettings,
};
pub use store::Store;

/// The settings file's name.
pub const FILE_NAME: &str = "capturax.toml";
