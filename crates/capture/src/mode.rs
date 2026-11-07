//! Modes from a device's caps, the automatic modes and the badges (research R3, data-model.md
//! "Mode" and "Mode choice"), ported from the stack spike's `capture/src/mode.rs`. Pure: no
//! device needed.

use std::fmt;

/// Two frame rates closer than this are the same rate (59.94 and 60).
const SAME_RATE: f64 = 0.5;

/// The pixel format a device delivers a mode in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    Nv12,
    Yuy2,
    Uyvy,
    I420,
    Bgrx,
    Rgba,
    /// `image/jpeg`, decoded in the pipeline.
    Mjpg,
    /// Any other raw format (converted to NV12 in the pipeline), by its GStreamer name.
    Other(String),
}

impl Format {
    /// From a `video/x-raw` format name.
    pub fn from_raw(name: &str) -> Self {
        match name {
            "NV12" => Self::Nv12,
            "YUY2" => Self::Yuy2,
            "UYVY" => Self::Uyvy,
            "I420" => Self::I420,
            "BGRx" => Self::Bgrx,
            "RGBA" => Self::Rgba,
            other => Self::Other(other.to_string()),
        }
    }

    /// The GStreamer name, `MJPG` for JPEG.
    pub fn name(&self) -> &str {
        match self {
            Self::Nv12 => "NV12",
            Self::Yuy2 => "YUY2",
            Self::Uyvy => "UYVY",
            Self::I420 => "I420",
            Self::Bgrx => "BGRx",
            Self::Rgba => "RGBA",
            Self::Mjpg => "MJPG",
            Self::Other(name) => name,
        }
    }

    /// Preference when several formats offer the same size and rate (lower is better): NV12,
    /// YUY2, UYVY, I420, BGRx, MJPG (decoded), then any other raw format.
    fn rank(&self) -> u8 {
        match self {
            Self::Nv12 => 0,
            Self::Yuy2 => 1,
            Self::Uyvy => 2,
            Self::I420 => 3,
            Self::Bgrx => 4,
            Self::Mjpg => 5,
            Self::Rgba => 6,
            Self::Other(_) => 7,
        }
    }
}

/// One way a device can deliver video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    /// Frame rate as a fraction (numerator, denominator); 59.94 is 60000/1001.
    pub fps: (i32, i32),
    pub format: Format,
}

impl Mode {
    pub fn fps_f64(&self) -> f64 {
        fraction(self.fps)
    }

    fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    fn same_size_and_rate(&self, width: u32, height: u32, fps: f64) -> bool {
        self.width == width && self.height == height && (self.fps_f64() - fps).abs() < SAME_RATE
    }

    /// `1920 × 1080 · 60`, `1920 × 1080 · 59.94` when fractional.
    pub fn label(&self) -> String {
        format!("{} × {} · {}", self.width, self.height, rate_text(self.fps))
    }

    /// Caps that open the device in exactly this mode.
    pub fn caps(&self) -> gst::Caps {
        let fps = gst::Fraction::new(self.fps.0, self.fps.1);
        let (width, height) = (to_i32(self.width), to_i32(self.height));
        match &self.format {
            Format::Mjpg => gst::Caps::builder("image/jpeg")
                .field("width", width)
                .field("height", height)
                .field("framerate", fps)
                .build(),
            format => gst::Caps::builder("video/x-raw")
                .field("format", format.name())
                .field("width", width)
                .field("height", height)
                .field("framerate", fps)
                .build(),
        }
    }
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn fraction((num, den): (i32, i32)) -> f64 {
    if den == 0 { 0.0 } else { f64::from(num) / f64::from(den) }
}

/// A frame rate as people read it: `60`, `59.94`.
pub fn rate_text(fps: (i32, i32)) -> String {
    let value = fraction(fps);
    if (value - value.round()).abs() < 0.005 {
        format!("{}", value.round())
    } else {
        format!("{value:.2}")
    }
}

/// The input a device reports receiving (none of this spec's devices do: research R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal {
    pub width: u32,
    pub height: u32,
    pub fps: (i32, i32),
}

/// Frame rates tried inside a range a driver reports (for example 5–30).
const RANGE_STEPS: [i32; 9] = [24, 25, 30, 50, 60, 90, 100, 120, 144];

/// Whether the caps declare NV12 in full range (colorimetry range 1, `0_255`). With
/// `device.api = mediafoundation` this marks the Camera Frame Server decoding MJPG (research R3);
/// the caller combines both and passes the result to [`modes_from_caps`].
pub fn nv12_declared_full(caps: &gst::CapsRef) -> bool {
    caps.iter().any(|s| {
        s.name() == "video/x-raw"
            && formats(s).iter().any(|f| f == "NV12")
            && s.get::<&str>("colorimetry")
                .is_ok_and(|c| c.starts_with("1:") || c == "jpeg")
    })
}

/// The modes of a device's caps: one per resolution × frame rate, in the best format, sorted by
/// resolution then frame rate, largest first (research R3). Ranges give their ends plus the
/// common steps inside. Caps with features other than system memory (DMA-BUF) are skipped.
///
/// `declared_full_nv12`: the Windows exception. When true and the device offers MJPG and NV12 at
/// exactly the same sizes, NV12 is the Frame Server decoding MJPG, so YUY2 goes before NV12 at
/// a size where it is offered.
pub fn modes_from_caps(caps: &gst::CapsRef, declared_full_nv12: bool) -> Vec<Mode> {
    let mut all = Vec::new();
    for (s, features) in caps.iter_with_features() {
        if !features.is_empty() && !features.contains("memory:SystemMemory") {
            continue;
        }
        let names = match s.name().as_str() {
            "video/x-raw" => formats(s).into_iter().map(|f| Format::from_raw(&f)).collect(),
            "image/jpeg" => vec![Format::Mjpg],
            _ => continue,
        };
        let (Ok(width), Ok(height)) = (s.get::<i32>("width"), s.get::<i32>("height")) else {
            continue;
        };
        let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
            continue;
        };
        let Ok(value) = s.value("framerate") else {
            continue;
        };
        for fps in frame_rates(value) {
            if fps.0 <= 0 {
                continue;
            }
            for format in &names {
                all.push(Mode {
                    width,
                    height,
                    fps,
                    format: format.clone(),
                });
            }
        }
    }
    best_per_size_and_rate(all, declared_full_nv12)
}

/// Keeps one mode per resolution × frame rate, in the best format, sorted largest first.
pub fn best_per_size_and_rate(all: Vec<Mode>, declared_full_nv12: bool) -> Vec<Mode> {
    let frame_server = declared_full_nv12 && same_sizes(&all, &Format::Nv12, &Format::Mjpg);
    let rank = |m: &Mode| -> u8 {
        if frame_server && m.format == Format::Yuy2 {
            0
        } else if frame_server && m.format == Format::Nv12 {
            1
        } else {
            m.format.rank().saturating_add(1)
        }
    };
    let mut best: Vec<Mode> = Vec::new();
    for mode in all {
        match best
            .iter_mut()
            .find(|b| b.same_size_and_rate(mode.width, mode.height, mode.fps_f64()))
        {
            Some(kept) if rank(&mode) < rank(kept) => *kept = mode,
            Some(_) => {}
            None => best.push(mode),
        }
    }
    best.sort_by(|a, b| {
        b.area()
            .cmp(&a.area())
            .then(b.width.cmp(&a.width))
            .then(b.fps_f64().total_cmp(&a.fps_f64()))
    });
    best
}

/// Whether two formats are offered at exactly the same set of sizes (both offered).
fn same_sizes(modes: &[Mode], a: &Format, b: &Format) -> bool {
    let sizes = |format: &Format| {
        let mut sizes: Vec<(u32, u32)> = modes
            .iter()
            .filter(|m| &m.format == format)
            .map(|m| (m.width, m.height))
            .collect();
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    };
    let (a, b) = (sizes(a), sizes(b));
    !a.is_empty() && a == b
}

/// The format names of a `video/x-raw` structure: one, or each of a list.
fn formats(s: &gst::StructureRef) -> Vec<String> {
    if let Ok(name) = s.get::<&str>("format") {
        return vec![name.to_string()];
    }
    if let Ok(list) = s.get::<gst::List>("format") {
        return list
            .iter()
            .filter_map(|v| v.get::<&str>().ok().map(str::to_string))
            .collect();
    }
    Vec::new()
}

fn frame_rates(value: &gst::glib::SendValue) -> Vec<(i32, i32)> {
    if let Ok(f) = value.get::<gst::Fraction>() {
        return vec![(f.numer(), f.denom())];
    }
    if let Ok(range) = value.get::<gst::FractionRange>() {
        let (min, max) = (range.min(), range.max());
        let (lo, hi) = (
            fraction((min.numer(), min.denom())),
            fraction((max.numer(), max.denom())),
        );
        let mut rates = vec![(max.numer(), max.denom())];
        rates.extend(
            RANGE_STEPS
                .iter()
                .filter(|r| f64::from(**r) > lo && f64::from(**r) < hi)
                .rev()
                .map(|r| (*r, 1)),
        );
        rates.push((min.numer(), min.denom()));
        return rates;
    }
    if let Ok(list) = value.get::<gst::List>() {
        return list
            .iter()
            .filter_map(|v| v.get::<gst::Fraction>().ok())
            .map(|f| (f.numer(), f.denom()))
            .collect();
    }
    Vec::new()
}

/// What the user chose for a device (data-model.md "Mode choice").
#[derive(Debug, Clone, PartialEq)]
pub enum ModeChoice {
    AutoSource,
    AutoMaxResolution,
    AutoMaxFps,
    Manual { width: u32, height: u32, fps: (i32, i32) },
}

impl ModeChoice {
    /// The settings text: `auto-source`, `auto-max-resolution`, `auto-max-fps`, `1920x1080@60`,
    /// `1920x1080@59.94`. Anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "auto-source" => return Some(Self::AutoSource),
            "auto-max-resolution" => return Some(Self::AutoMaxResolution),
            "auto-max-fps" => return Some(Self::AutoMaxFps),
            _ => {}
        }
        let (size, rate) = text.trim().split_once('@')?;
        let (width, height) = size.split_once('x')?;
        let (width, height): (u32, u32) = (width.parse().ok()?, height.parse().ok()?);
        if width == 0 || height == 0 {
            return None;
        }
        let fps = parse_rate(rate)?;
        Some(Self::Manual { width, height, fps })
    }

    /// The manual choice of a mode.
    pub fn manual(mode: &Mode) -> Self {
        Self::Manual {
            width: mode.width,
            height: mode.height,
            fps: mode.fps,
        }
    }

    pub fn is_automatic(&self) -> bool {
        !matches!(self, Self::Manual { .. })
    }
}

/// `60` → 60/1, `59.94` → 5994/100 (matched within 0.5 like any rate).
fn parse_rate(text: &str) -> Option<(i32, i32)> {
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (whole, decimals) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || decimals.len() > 3 || decimals.contains('.') {
        return None;
    }
    let den = 10_i32.pow(decimals.len() as u32);
    let num: i32 = format!("{whole}{decimals}").parse().ok()?;
    (num > 0).then_some((num, den))
}

impl fmt::Display for ModeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AutoSource => f.write_str("auto-source"),
            Self::AutoMaxResolution => f.write_str("auto-max-resolution"),
            Self::AutoMaxFps => f.write_str("auto-max-fps"),
            Self::Manual { width, height, fps } => {
                write!(f, "{width}x{height}@{}", rate_text(*fps))
            }
        }
    }
}

/// The mode a choice opens, if the device has one (data-model.md "Mode choice"). A manual mode
/// the device lacks is `None`: the caller falls back to [`default_choice`].
pub fn resolve(choice: &ModeChoice, modes: &[Mode], signal: Option<Signal>) -> Option<Mode> {
    let found = match choice {
        ModeChoice::AutoSource => {
            let s = signal?;
            find(modes, s.width, s.height, fraction(s.fps))
        }
        ModeChoice::AutoMaxResolution => max_resolution(modes),
        ModeChoice::AutoMaxFps => max_fps(modes),
        ModeChoice::Manual { width, height, fps } => find(modes, *width, *height, fraction(*fps)),
    };
    found.cloned()
}

fn find(modes: &[Mode], width: u32, height: u32, fps: f64) -> Option<&Mode> {
    modes.iter().find(|m| m.same_size_and_rate(width, height, fps))
}

/// Largest resolution at 58–62 fps, at its highest such rate; none there → the largest
/// resolution at the device's highest rate (C920: 1080p30, not 2304 × 1536 at 2 fps).
fn max_resolution(modes: &[Mode]) -> Option<&Mode> {
    let near_60 = |m: &&Mode| (58.0..=62.0).contains(&m.fps_f64());
    fn largest(pool: Vec<&Mode>) -> Option<&Mode> {
        pool.into_iter().max_by(|a, b| {
            a.area()
                .cmp(&b.area())
                .then(a.width.cmp(&b.width))
                .then(a.fps_f64().total_cmp(&b.fps_f64()))
        })
    }
    let pool: Vec<&Mode> = modes.iter().filter(near_60).collect();
    if !pool.is_empty() {
        return largest(pool);
    }
    let top = modes.iter().map(Mode::fps_f64).fold(0.0, f64::max);
    largest(modes.iter().filter(|m| (m.fps_f64() - top).abs() < SAME_RATE).collect())
}

/// Highest rate at 1080p; else at the largest resolution below 1080p; else at the smallest
/// above.
fn max_fps(modes: &[Mode]) -> Option<&Mode> {
    fn fastest(pool: Vec<&Mode>) -> Option<&Mode> {
        pool.into_iter()
            .max_by(|a, b| a.fps_f64().total_cmp(&b.fps_f64()).then(a.area().cmp(&b.area())))
    }
    let at_1080: Vec<&Mode> = modes.iter().filter(|m| m.height == 1080).collect();
    if !at_1080.is_empty() {
        return fastest(at_1080);
    }
    let size = |m: &Mode| (m.area(), m.width);
    let below = modes.iter().filter(|m| m.height < 1080).map(size).max();
    let chosen = below.or_else(|| modes.iter().filter(|m| m.height > 1080).map(size).min())?;
    fastest(modes.iter().filter(|m| size(m) == chosen).collect())
}

/// The automatic entries to list, in the order Auto source (only with a signal), Auto max
/// resolution, Auto max FPS, each with the mode it opens; one that opens the same mode as an
/// earlier one is skipped (FR-007).
pub fn shown_automatic(modes: &[Mode], signal: Option<Signal>) -> Vec<(ModeChoice, Mode)> {
    let mut shown: Vec<(ModeChoice, Mode)> = Vec::new();
    for choice in [
        ModeChoice::AutoSource,
        ModeChoice::AutoMaxResolution,
        ModeChoice::AutoMaxFps,
    ] {
        if let Some(mode) = resolve(&choice, modes, signal)
            && !shown.iter().any(|(_, m)| *m == mode)
        {
            shown.push((choice, mode));
        }
    }
    shown
}

/// The default choice (FR-008): Auto source when the device reports its input; else 1920 × 1080
/// at its highest rate within 58–62 fps (60 before 59.94) when it has one; else Auto max
/// resolution. Auto max FPS with a 60 Hz source measured about 5 ms slower than the 60 mode
/// (T029).
pub fn default_choice(signal: Option<Signal>, modes: &[Mode]) -> ModeChoice {
    if signal.is_some() {
        return ModeChoice::AutoSource;
    }
    modes
        .iter()
        .filter(|m| m.width == 1920 && m.height == 1080 && (58.0..=62.0).contains(&m.fps_f64()))
        .max_by(|a, b| a.fps_f64().total_cmp(&b.fps_f64()))
        .map_or(ModeChoice::AutoMaxResolution, ModeChoice::manual)
}

/// The resolution badge of a mode (F43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    Uhd4k,
    Qhd,
    FullHd,
    Hd,
}

impl Badge {
    /// The id the UI uses (`4k`, `qhd`, `fhd`, `hd`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Uhd4k => "4k",
            Self::Qhd => "qhd",
            Self::FullHd => "fhd",
            Self::Hd => "hd",
        }
    }
}

/// ≥ 2160 4K, ≥ 1440 QHD, ≥ 1080 Full HD, ≥ 720 HD, none below.
pub fn badge(height: u32) -> Option<Badge> {
    match height {
        2160.. => Some(Badge::Uhd4k),
        1440.. => Some(Badge::Qhd),
        1080.. => Some(Badge::FullHd),
        720.. => Some(Badge::Hd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u32, height: u32, fps: i32, format: Format) -> Mode {
        Mode {
            width,
            height,
            fps: (fps, 1),
            format,
        }
    }

    /// An NV12 mode at a fractional rate (59.94 = 60000/1001).
    fn mode_rate(width: u32, height: u32, fps: (i32, i32)) -> Mode {
        Mode {
            fps,
            ..mode(width, height, 0, Format::Nv12)
        }
    }

    fn caps(text: &str) -> gst::Caps {
        gst::init().expect("GStreamer initializes");
        text.parse().expect("caps parse")
    }

    /// The ezcap's NV12 modes (research R3).
    fn ezcap() -> Vec<Mode> {
        best_per_size_and_rate(
            vec![
                mode(3840, 2160, 30, Format::Nv12),
                mode(2560, 1440, 60, Format::Nv12),
                mode(1920, 1080, 120, Format::Nv12),
                mode(1920, 1080, 60, Format::Nv12),
                mode(1280, 720, 60, Format::Nv12),
            ],
            false,
        )
    }

    /// A webcam that never reaches 58 fps.
    fn webcam() -> Vec<Mode> {
        best_per_size_and_rate(
            vec![
                mode(2304, 1536, 2, Format::Yuy2),
                mode(1920, 1080, 30, Format::Nv12),
                mode(1280, 720, 30, Format::Nv12),
                mode(640, 480, 30, Format::Yuy2),
            ],
            false,
        )
    }

    #[test]
    fn best_format_per_size_and_rate() {
        let modes = best_per_size_and_rate(
            vec![
                mode(1920, 1080, 60, Format::Mjpg),
                mode(1920, 1080, 60, Format::Other("P010_10LE".into())),
                mode(1920, 1080, 60, Format::Yuy2),
                mode(1920, 1080, 60, Format::Bgrx),
                mode(1280, 720, 60, Format::Uyvy),
                mode(1280, 720, 60, Format::I420),
                mode(1280, 720, 60, Format::Rgba),
            ],
            false,
        );
        assert_eq!(
            modes,
            vec![mode(1920, 1080, 60, Format::Yuy2), mode(1280, 720, 60, Format::Uyvy)]
        );
        let modes = best_per_size_and_rate(
            vec![mode(640, 480, 30, Format::Mjpg), mode(640, 480, 30, Format::Nv12)],
            false,
        );
        assert_eq!(modes[0].format, Format::Nv12);
    }

    #[test]
    fn rates_within_half_a_frame_are_the_same() {
        let modes = best_per_size_and_rate(
            vec![
                Mode {
                    width: 1920,
                    height: 1080,
                    fps: (60000, 1001),
                    format: Format::Yuy2,
                },
                mode(1920, 1080, 60, Format::Nv12),
            ],
            false,
        );
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].format, Format::Nv12);
    }

    #[test]
    fn sorted_largest_first() {
        let order: Vec<String> = ezcap().iter().map(Mode::label).collect();
        assert_eq!(
            order,
            [
                "3840 × 2160 · 30",
                "2560 × 1440 · 60",
                "1920 × 1080 · 120",
                "1920 × 1080 · 60",
                "1280 × 720 · 60"
            ]
        );
    }

    #[test]
    fn frame_server_rule() {
        let all = vec![
            mode(1920, 1080, 30, Format::Mjpg),
            mode(1920, 1080, 30, Format::Nv12),
            mode(640, 480, 30, Format::Mjpg),
            mode(640, 480, 30, Format::Nv12),
            mode(640, 480, 30, Format::Yuy2),
        ];
        let modes = best_per_size_and_rate(all.clone(), true);
        assert_eq!(modes[0].format, Format::Nv12, "no YUY2 at 1080p30");
        assert_eq!(modes[1].format, Format::Yuy2, "YUY2 before NV12 at 640 × 480");
        let modes = best_per_size_and_rate(all.clone(), false);
        assert_eq!(modes[1].format, Format::Nv12, "not declared full range");
        // Different sizes for MJPG and NV12: not the Frame Server.
        let mut other = all;
        other.push(mode(320, 240, 30, Format::Mjpg));
        assert_eq!(best_per_size_and_rate(other, true)[1].format, Format::Nv12);
    }

    #[test]
    fn modes_from_caps_lists_ranges_and_skips_dmabuf() {
        let caps = caps(
            "video/x-raw(memory:DMABuf), format=DMA_DRM, drm-format=NV12, width=3840, height=2160, framerate=30/1; \
             video/x-raw, format={ (string)YUY2, (string)P010_10LE }, width=1920, height=1080, framerate=60/1; \
             video/x-raw, format=NV12, width=1920, height=1080, framerate={ (fraction)120/1, (fraction)60/1 }; \
             image/jpeg, width=640, height=480, framerate=[ 5/1, 30/1 ]",
        );
        let labels: Vec<String> = modes_from_caps(&caps, false)
            .iter()
            .map(|m| format!("{} {}", m.label(), m.format.name()))
            .collect();
        assert_eq!(
            labels,
            [
                "1920 × 1080 · 120 NV12",
                "1920 × 1080 · 60 NV12",
                "640 × 480 · 30 MJPG",
                "640 × 480 · 25 MJPG",
                "640 × 480 · 24 MJPG",
                "640 × 480 · 5 MJPG"
            ]
        );
    }

    #[test]
    fn nv12_full_range_declared() {
        let full = caps("video/x-raw, format=NV12, width=640, height=480, framerate=30/1, colorimetry=1:4:0:1");
        let limited = caps("video/x-raw, format=NV12, width=640, height=480, framerate=30/1, colorimetry=2:4:0:1");
        assert!(nv12_declared_full(&full));
        assert!(!nv12_declared_full(&limited));
    }

    #[test]
    fn labels() {
        assert_eq!(mode(1920, 1080, 60, Format::Nv12).label(), "1920 × 1080 · 60");
        let ntsc = Mode {
            width: 1920,
            height: 1080,
            fps: (60000, 1001),
            format: Format::Nv12,
        };
        assert_eq!(ntsc.label(), "1920 × 1080 · 59.94");
        let quarter = Mode {
            fps: (10_000_000, 1_333_333),
            ..mode(160, 90, 0, Format::Mjpg)
        };
        assert_eq!(quarter.label(), "160 × 90 · 7.50");
    }

    #[test]
    fn choice_text_round_trip() {
        for text in [
            "auto-source",
            "auto-max-resolution",
            "auto-max-fps",
            "1920x1080@60",
            "1920x1080@59.94",
        ] {
            let choice = ModeChoice::parse(text).expect(text);
            assert_eq!(choice.to_string(), text);
        }
        for bad in [
            "",
            "auto",
            "1920x1080",
            "1920@60",
            "0x1080@60",
            "1920x1080@",
            "axb@60",
            "1920x1080@-60",
        ] {
            assert_eq!(ModeChoice::parse(bad), None, "{bad}");
        }
        let ntsc = Mode {
            width: 1920,
            height: 1080,
            fps: (60000, 1001),
            format: Format::Nv12,
        };
        assert_eq!(ModeChoice::manual(&ntsc).to_string(), "1920x1080@59.94");
    }

    #[test]
    fn manual_resolves_within_half_a_frame() {
        let modes = vec![Mode {
            width: 1920,
            height: 1080,
            fps: (60000, 1001),
            format: Format::Nv12,
        }];
        let choice = ModeChoice::parse("1920x1080@60").unwrap();
        assert_eq!(resolve(&choice, &modes, None), Some(modes[0].clone()));
        let missing = ModeChoice::parse("1280x720@60").unwrap();
        assert_eq!(resolve(&missing, &modes, None), None);
    }

    #[test]
    fn auto_max_resolution() {
        assert_eq!(
            resolve(&ModeChoice::AutoMaxResolution, &ezcap(), None).unwrap().label(),
            "2560 × 1440 · 60"
        );
        assert_eq!(
            resolve(&ModeChoice::AutoMaxResolution, &webcam(), None)
                .unwrap()
                .label(),
            "1920 × 1080 · 30"
        );
        // 120 fps does not count as 58–62.
        let fast = vec![mode(1920, 1080, 120, Format::Nv12), mode(1280, 720, 60, Format::Nv12)];
        assert_eq!(
            resolve(&ModeChoice::AutoMaxResolution, &fast, None).unwrap().label(),
            "1280 × 720 · 60"
        );
        assert_eq!(resolve(&ModeChoice::AutoMaxResolution, &[], None), None);
    }

    #[test]
    fn auto_max_fps() {
        assert_eq!(
            resolve(&ModeChoice::AutoMaxFps, &ezcap(), None).unwrap().label(),
            "1920 × 1080 · 120"
        );
        let below = vec![
            mode(3840, 2160, 60, Format::Nv12),
            mode(1280, 720, 30, Format::Nv12),
            mode(1280, 720, 60, Format::Nv12),
            mode(640, 480, 90, Format::Nv12),
        ];
        assert_eq!(
            resolve(&ModeChoice::AutoMaxFps, &below, None).unwrap().label(),
            "1280 × 720 · 60"
        );
        let above = vec![
            mode(3840, 2160, 60, Format::Nv12),
            mode(2560, 1440, 30, Format::Nv12),
            mode(2560, 1440, 50, Format::Nv12),
        ];
        assert_eq!(
            resolve(&ModeChoice::AutoMaxFps, &above, None).unwrap().label(),
            "2560 × 1440 · 50"
        );
    }

    #[test]
    fn auto_source_needs_a_signal() {
        let signal = Signal {
            width: 1920,
            height: 1080,
            fps: (60, 1),
        };
        assert_eq!(resolve(&ModeChoice::AutoSource, &ezcap(), None), None);
        assert_eq!(
            resolve(&ModeChoice::AutoSource, &ezcap(), Some(signal))
                .unwrap()
                .label(),
            "1920 × 1080 · 60"
        );
        assert_eq!(default_choice(Some(signal), &ezcap()), ModeChoice::AutoSource);
    }

    #[test]
    fn default_is_1080p_near_60_else_auto_max_resolution() {
        let at_60 = ModeChoice::Manual {
            width: 1920,
            height: 1080,
            fps: (60, 1),
        };
        assert_eq!(default_choice(None, &ezcap()), at_60);
        assert_eq!(default_choice(None, &webcam()), ModeChoice::AutoMaxResolution);
        assert_eq!(default_choice(None, &[]), ModeChoice::AutoMaxResolution);

        // 60 before 59.94; 59.94 alone is taken.
        let both = vec![mode_rate(1920, 1080, (60000, 1001)), mode(1920, 1080, 60, Format::Nv12)];
        assert_eq!(default_choice(None, &both), at_60);
        let ntsc = vec![
            mode(1920, 1080, 120, Format::Nv12),
            mode_rate(1920, 1080, (60000, 1001)),
        ];
        assert_eq!(
            default_choice(None, &ntsc),
            ModeChoice::Manual {
                width: 1920,
                height: 1080,
                fps: (60000, 1001),
            }
        );
        // 1080p only at 50 or 120: not near 60.
        let pal = vec![mode(1920, 1080, 120, Format::Nv12), mode(1920, 1080, 50, Format::Nv12)];
        assert_eq!(default_choice(None, &pal), ModeChoice::AutoMaxResolution);
    }

    #[test]
    fn shown_automatic_skips_duplicates() {
        let shown: Vec<ModeChoice> = shown_automatic(&ezcap(), None).into_iter().map(|s| s.0).collect();
        assert_eq!(shown, [ModeChoice::AutoMaxResolution, ModeChoice::AutoMaxFps]);
        // On a webcam both resolve to 1080p30: only the first is shown.
        let shown = shown_automatic(&webcam(), None);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].0, ModeChoice::AutoMaxResolution);
        // With a signal Auto source comes first, and Auto max resolution resolving to the same
        // mode is skipped.
        let signal = Signal {
            width: 2560,
            height: 1440,
            fps: (60, 1),
        };
        let shown: Vec<ModeChoice> = shown_automatic(&ezcap(), Some(signal))
            .into_iter()
            .map(|s| s.0)
            .collect();
        assert_eq!(shown, [ModeChoice::AutoSource, ModeChoice::AutoMaxFps]);
        assert!(shown_automatic(&[], None).is_empty());
    }

    #[test]
    fn badges() {
        assert_eq!(badge(2160), Some(Badge::Uhd4k));
        assert_eq!(badge(1536), Some(Badge::Qhd));
        assert_eq!(badge(1440), Some(Badge::Qhd));
        assert_eq!(badge(1080), Some(Badge::FullHd));
        assert_eq!(badge(720), Some(Badge::Hd));
        assert_eq!(badge(719), None);
    }
}
