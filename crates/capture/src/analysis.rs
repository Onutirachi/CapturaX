//! Color range detection from sampled luma (research R5, data-model.md "Color range choice").
//! Pure; the session runs it on the streaming thread once every 500 ms, never on the video
//! thread (FR-015).

use crate::frame::{PixelFormat, Range};

/// What the user chose for a device's color range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RangeChoice {
    #[default]
    Auto,
    Full,
    Limited,
}

/// Samples taken on a grid of every 8th pixel of every 8th row.
const GRID: usize = 8;

/// Luma values of a frame's first plane on a 1/8 grid (about 32 000 samples at 1080p). For YUY2
/// and UYVY the Y bytes of the packed plane; RGB formats have no luma and give none.
pub fn sample_luma(plane: &[u8], stride: usize, width: u32, height: u32, format: PixelFormat) -> Vec<u8> {
    let (width, height) = (width as usize, height as usize);
    let (step, offset) = match format {
        PixelFormat::Nv12 | PixelFormat::I420 => (1, 0),
        PixelFormat::Yuy2 => (2, 0),
        PixelFormat::Uyvy => (2, 1),
        PixelFormat::Bgrx | PixelFormat::Rgba => return Vec::new(),
    };
    let mut samples = Vec::with_capacity(width.div_ceil(GRID) * height.div_ceil(GRID));
    for y in (0..height).step_by(GRID) {
        let row = y * stride;
        for x in (0..width).step_by(GRID) {
            if let Some(value) = plane.get(row + x * step + offset) {
                samples.push(*value);
            }
        }
    }
    samples
}

/// Decides the range in use. Auto starts at the declared range (Limited when undeclared); one
/// sampling with at least 0.1% of values below 16 makes it Full until [`RangeDetector::reset`]
/// (device or mode change). Full and Limited set it directly.
#[derive(Debug, Clone)]
pub struct RangeDetector {
    choice: RangeChoice,
    declared: Option<Range>,
    seen_full: bool,
}

impl RangeDetector {
    pub fn new(choice: RangeChoice, declared: Option<Range>) -> Self {
        Self {
            choice,
            declared,
            seen_full: false,
        }
    }

    /// The range frames are converted with now.
    pub fn in_use(&self) -> Range {
        match self.choice {
            RangeChoice::Full => Range::Full,
            RangeChoice::Limited => Range::Limited,
            RangeChoice::Auto if self.seen_full => Range::Full,
            RangeChoice::Auto => self.declared.unwrap_or(Range::Limited),
        }
    }

    /// Whether the range in use was inferred from the picture (`(auto)` in the stats).
    pub fn inferred(&self) -> bool {
        self.choice == RangeChoice::Auto
    }

    pub fn choice(&self) -> RangeChoice {
        self.choice
    }

    pub fn set_choice(&mut self, choice: RangeChoice) {
        self.choice = choice;
    }

    /// Feeds one sampling; true when the range in use changed.
    pub fn observe(&mut self, samples: &[u8]) -> bool {
        if self.choice != RangeChoice::Auto || self.seen_full || samples.is_empty() {
            return false;
        }
        let before = self.in_use();
        let dark = samples.iter().filter(|v| **v < 16).count();
        // At least 0.1% of the samples: dark * 1000 >= len.
        if dark > 0 && dark * 1000 >= samples.len() {
            self.seen_full = true;
        }
        self.in_use() != before
    }

    /// Forgets what the picture showed (device or mode change).
    pub fn reset(&mut self, declared: Option<Range>) {
        self.declared = declared;
        self.seen_full = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_every_8th_pixel_of_every_8th_row() {
        let (width, height) = (32u32, 16u32);
        let plane: Vec<u8> = (0..width * height).map(|i| (i % 251) as u8).collect();
        let samples = sample_luma(&plane, 32, width, height, PixelFormat::Nv12);
        // Row 8 starts at byte 256, stored as 256 % 251 = 5.
        assert_eq!(samples, vec![0, 8, 16, 24, 5, 13, 21, 29]);
    }

    #[test]
    fn packed_formats_sample_the_y_bytes() {
        // Two pixels of YUY2: Y0 U Y1 V; of UYVY: U Y0 V Y1. Width 16: x = 0 and 8.
        let mut yuy2 = vec![128u8; 32];
        yuy2[0] = 10;
        yuy2[16] = 20;
        assert_eq!(sample_luma(&yuy2, 32, 16, 1, PixelFormat::Yuy2), vec![10, 20]);
        let mut uyvy = vec![128u8; 32];
        uyvy[1] = 30;
        uyvy[17] = 40;
        assert_eq!(sample_luma(&uyvy, 32, 16, 1, PixelFormat::Uyvy), vec![30, 40]);
        assert!(sample_luma(&[0; 64], 64, 16, 1, PixelFormat::Bgrx).is_empty());
    }

    #[test]
    fn auto_starts_at_the_declared_range() {
        assert_eq!(RangeDetector::new(RangeChoice::Auto, None).in_use(), Range::Limited);
        assert_eq!(
            RangeDetector::new(RangeChoice::Auto, Some(Range::Full)).in_use(),
            Range::Full
        );
    }

    #[test]
    fn dark_samples_make_auto_full() {
        let mut detector = RangeDetector::new(RangeChoice::Auto, Some(Range::Limited));
        let mut samples = vec![100u8; 10_000];
        // 9 of 10 000 below 16: under 0.1%.
        samples[..9].fill(5);
        assert!(!detector.observe(&samples));
        assert_eq!(detector.in_use(), Range::Limited);
        // 10 of 10 000: exactly 0.1%.
        samples[9] = 15;
        assert!(detector.observe(&samples));
        assert_eq!(detector.in_use(), Range::Full);
        // Stays Full with a bright picture.
        assert!(!detector.observe(&[200; 100]));
        assert_eq!(detector.in_use(), Range::Full);
        // 16 is not below 16.
        detector.reset(None);
        assert_eq!(detector.in_use(), Range::Limited);
        assert!(!detector.observe(&[16; 100]));
        assert!(!detector.observe(&[]));
    }

    #[test]
    fn manual_choices_ignore_the_picture() {
        let mut detector = RangeDetector::new(RangeChoice::Limited, Some(Range::Full));
        assert_eq!(detector.in_use(), Range::Limited);
        assert!(!detector.observe(&[0; 100]));
        assert_eq!(detector.in_use(), Range::Limited);
        detector.set_choice(RangeChoice::Full);
        assert_eq!(detector.in_use(), Range::Full);
        assert!(!detector.inferred());
        detector.set_choice(RangeChoice::Auto);
        assert!(detector.inferred());
    }
}
