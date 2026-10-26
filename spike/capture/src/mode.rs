//! Capture modes: parsing `--mode WxH@FPS[:FORMAT]`, reading them from caps, and the Auto choice.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    /// Frame rate as a fraction (numerator, denominator).
    pub fps: (i32, i32),
    /// Raw format (`NV12`, `YUY2`…) or `MJPG` for `image/jpeg`.
    pub format: String,
}

/// Formats preferred when several share the same size and frame rate: 8-bit 4:2:0 first.
const FORMAT_PREFERENCE: [&str; 5] = ["NV12", "YUY2", "BGRx", "MJPG", "P010_10LE"];

/// The Auto threshold: "largest resolution with ≥ 59 fps" (59.94 counts as 60).
const AUTO_MIN_FPS: f64 = 59.0;

impl Mode {
    pub fn fps_f64(&self) -> f64 {
        self.fps.0 as f64 / self.fps.1 as f64
    }

    fn format_rank(&self) -> usize {
        FORMAT_PREFERENCE.iter().position(|f| *f == self.format).unwrap_or(FORMAT_PREFERENCE.len())
    }

    /// Caps that open the device in exactly this mode.
    pub fn caps(&self) -> gst::Caps {
        let fps = gst::Fraction::new(self.fps.0, self.fps.1);
        if self.format == "MJPG" {
            gst::Caps::builder("image/jpeg")
                .field("width", self.width)
                .field("height", self.height)
                .field("framerate", fps)
                .build()
        } else {
            gst::Caps::builder("video/x-raw")
                .field("format", self.format.as_str())
                .field("width", self.width)
                .field("height", self.height)
                .field("framerate", fps)
                .build()
        }
    }

    /// Whether `other` (a requested mode) matches this one; a request without format matches any.
    fn matches(&self, other: &Mode) -> bool {
        self.width == other.width
            && self.height == other.height
            && (self.fps_f64() - other.fps_f64()).abs() < 0.5
            && (other.format.is_empty() || self.format.eq_ignore_ascii_case(&other.format))
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fps = self.fps_f64();
        if fps.fract().abs() < 0.01 {
            write!(f, "{}x{}@{}", self.width, self.height, fps.round())?;
        } else {
            write!(f, "{}x{}@{:.2}", self.width, self.height, fps)?;
        }
        if !self.format.is_empty() {
            write!(f, ":{}", self.format)?;
        }
        Ok(())
    }
}

/// Parses `WxH@FPS[:FORMAT]`, e.g. `1920x1080@60:NV12` or `2560x1440@60`.
pub fn parse(text: &str) -> anyhow::Result<Mode> {
    let usage = || anyhow::anyhow!("mode must look like 1920x1080@60 or 1920x1080@60:NV12, got {text:?}");
    let (size, rest) = text.split_once('@').ok_or_else(usage)?;
    let (fps, format) = rest.split_once(':').unwrap_or((rest, ""));
    let (width, height) = size.split_once('x').ok_or_else(usage)?;
    let fps: f64 = fps.parse().map_err(|_| usage())?;
    Ok(Mode {
        width: width.parse().map_err(|_| usage())?,
        height: height.parse().map_err(|_| usage())?,
        fps: ((fps * 1000.0).round() as i32, 1000),
        format: format.to_ascii_uppercase(),
    })
}

/// Frame rates tried inside a range reported by a driver (for example 60–120).
const RANGE_STEPS: [i32; 9] = [24, 25, 30, 50, 60, 90, 100, 120, 144];

/// Every mode described by a device's caps. Ranges become their ends plus common steps inside.
pub fn from_caps(caps: &gst::CapsRef) -> Vec<Mode> {
    let mut modes = Vec::new();
    for s in caps.iter() {
        let format = match s.name().as_str() {
            "video/x-raw" => match s.get::<&str>("format") {
                Ok(f) => f.to_string(),
                Err(_) => continue,
            },
            "image/jpeg" => "MJPG".to_string(),
            _ => continue,
        };
        let (Ok(width), Ok(height)) = (s.get::<i32>("width"), s.get::<i32>("height")) else {
            continue;
        };
        let Ok(value) = s.value("framerate") else { continue };
        for fps in frame_rates(value) {
            let mode = Mode { width, height, fps, format: format.clone() };
            if !modes.contains(&mode) {
                modes.push(mode);
            }
        }
    }
    modes
}

fn frame_rates(value: &gst::glib::SendValue) -> Vec<(i32, i32)> {
    if let Ok(f) = value.get::<gst::Fraction>() {
        return vec![(f.numer(), f.denom())];
    }
    if let Ok(range) = value.get::<gst::FractionRange>() {
        let (min, max) = (range.min(), range.max());
        let (lo, hi) = (min.numer() as f64 / min.denom() as f64, max.numer() as f64 / max.denom() as f64);
        let mut rates = vec![(max.numer(), max.denom())];
        rates.extend(RANGE_STEPS.iter().filter(|r| (**r as f64) > lo && (**r as f64) < hi).map(|r| (*r, 1)));
        rates.push((min.numer(), min.denom()));
        return rates;
    }
    if let Ok(list) = value.get::<gst::List>() {
        return list.iter().filter_map(|v| v.get::<gst::Fraction>().ok()).map(|f| (f.numer(), f.denom())).collect();
    }
    Vec::new()
}

/// Auto: the largest resolution that still reaches ≥ 59 fps, at its highest frame rate, in the
/// preferred format. If no mode reaches 59 fps (most webcams), the largest resolution at the
/// device's highest frame rate (a C920 gets 1080p30, not 2304x1536 at 2 fps).
pub fn auto(modes: &[Mode]) -> Option<Mode> {
    let fast: Vec<&Mode> = modes.iter().filter(|m| m.fps_f64() >= AUTO_MIN_FPS).collect();
    let pool: Vec<&Mode> = if fast.is_empty() {
        let top = modes.iter().map(Mode::fps_f64).fold(0.0, f64::max);
        modes.iter().filter(|m| (m.fps_f64() - top).abs() < 0.5).collect()
    } else {
        fast
    };
    pool.into_iter()
        .max_by(|a, b| {
            (a.width * a.height)
                .cmp(&(b.width * b.height))
                .then(a.fps_f64().total_cmp(&b.fps_f64()))
                .then(b.format_rank().cmp(&a.format_rank()))
        })
        .cloned()
}

/// The device mode matching a request, preferring the best format when none was given.
pub fn find(modes: &[Mode], request: &Mode) -> Option<Mode> {
    modes.iter().filter(|m| m.matches(request)).min_by_key(|m| m.format_rank()).cloned()
}
