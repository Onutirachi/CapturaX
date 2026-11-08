//! The tags of a mode (F43, data-model.md "Mode"): the name of the nearest standard resolution at
//! or above it, by the old CapturaX rule, and its aspect ratio. Pure.

use std::fmt;

/// An aspect ratio, width:height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    pub width: u32,
    pub height: u32,
}

impl Ratio {
    const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn value(self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.width, self.height)
    }
}

/// The ratios shown by name; a real ratio within [`SNAP`] of one of them shows it instead.
pub const RATIOS: [Ratio; 12] = [
    Ratio::new(1, 1),
    Ratio::new(5, 4),
    Ratio::new(11, 9),
    Ratio::new(4, 3),
    Ratio::new(3, 2),
    Ratio::new(25, 16),
    Ratio::new(16, 10),
    Ratio::new(5, 3),
    Ratio::new(16, 9),
    Ratio::new(17, 9),
    Ratio::new(21, 9),
    Ratio::new(32, 9),
];

/// Relative difference of width / height under which a ratio is shown as the listed one.
const SNAP: f64 = 0.03;

/// The standard resolutions, by name (data-model.md "Mode").
pub const STANDARDS: [(&str, u32, u32); 55] = [
    ("8K FF", 8192, 4320),
    ("8K UHD", 7680, 4320),
    ("5K UHD", 5120, 2880),
    ("WQUXGA", 3840, 2400),
    ("QUXGA", 3200, 2400),
    ("DQHD", 5120, 1440),
    ("DFHD", 3840, 1080),
    ("5K UW", 5120, 2160),
    ("4K DCI", 4096, 2160),
    ("4K UHD", 3840, 2160),
    ("WQSXGA", 3200, 2048),
    ("QSXGA", 2560, 2048),
    ("SQFHD", 1920, 1920),
    ("QHD+", 3200, 1800),
    ("WQXGA", 2560, 1600),
    ("QXGA", 2048, 1536),
    ("UWQHD", 3440, 1440),
    ("WQHD", 2560, 1440),
    ("WUXGA", 1920, 1200),
    ("UXGA", 1600, 1200),
    ("UWFHD", 2560, 1080),
    ("2K DCI", 2048, 1080),
    ("Full HD", 1920, 1080),
    ("HDV", 1440, 1080),
    ("WSXGA+", 1680, 1050),
    ("SXGA+", 1400, 1050),
    ("SXGA", 1280, 1024),
    ("QuadVGA", 1280, 960),
    ("HD+", 1600, 900),
    ("WXGA+", 1440, 900),
    ("XGA+", 1152, 864),
    ("WXGA 800", 1280, 800),
    ("FWXGA", 1366, 768),
    ("WXGA", 1280, 768),
    ("XGA", 1024, 768),
    ("HD", 1280, 720),
    ("WSVGA", 1024, 600),
    ("SVGA", 800, 600),
    ("WSVGA PAL", 1024, 576),
    ("PAL", 768, 576),
    ("SD 576p", 720, 576),
    ("qHD", 960, 540),
    ("FWVGA", 854, 480),
    ("WVGA", 800, 480),
    ("SD 480p", 720, 480),
    ("VGA", 640, 480),
    ("nHD", 640, 360),
    ("HVGA", 480, 320),
    ("SIF", 384, 288),
    ("CIF", 352, 288),
    ("QVGA", 320, 240),
    ("CGA", 320, 200),
    ("QCIF", 176, 144),
    ("QQVGA", 160, 120),
    ("SQCIF", 128, 96),
];

/// The tags of a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tags {
    /// The standard's name; `None` is Custom.
    pub name: Option<&'static str>,
    /// The ratio family of that standard, which gives the name tag its fill; `None` for Custom.
    pub family: Option<Ratio>,
    /// The aspect ratio shown.
    pub aspect: Ratio,
}

/// The tags of a `width` × `height` mode.
pub fn tags(width: u32, height: u32) -> Tags {
    let standard = nearest_standard(width, height);
    Tags {
        name: standard.map(|s| s.0),
        family: standard.map(|s| aspect(s.1, s.2)),
        aspect: aspect(width, height),
    }
}

fn standards() -> impl Iterator<Item = &'static (&'static str, u32, u32)> {
    STANDARDS.iter()
}

/// The standard of the same size; else the smallest standard height ≥ `height`, among those the
/// narrowest width ≥ `width`, else the widest; `None` above the tallest or below the shortest.
fn nearest_standard(width: u32, height: u32) -> Option<&'static (&'static str, u32, u32)> {
    let tallest = standards().map(|s| s.2).max()?;
    let shortest = standards().map(|s| s.2).min()?;
    if height > tallest || height < shortest {
        return None;
    }
    if let Some(exact) = standards().find(|s| s.1 == width && s.2 == height) {
        return Some(exact);
    }
    let row = standards().map(|s| s.2).filter(|&h| h >= height).min()?;
    let same_height = || standards().filter(move |s| s.2 == row);
    same_height()
        .filter(|s| s.1 >= width)
        .min_by_key(|s| s.1)
        .or_else(|| same_height().max_by_key(|s| s.1))
}

/// The aspect ratio shown for a size: a listed ratio when it is exactly one, else the nearest
/// listed one within [`SNAP`], else the real ratio reduced.
pub fn aspect(width: u32, height: u32) -> Ratio {
    if width == 0 || height == 0 {
        return Ratio::new(width, height);
    }
    let exact = |r: &&Ratio| u64::from(r.width) * u64::from(height) == u64::from(r.height) * u64::from(width);
    if let Some(listed) = RATIOS.iter().find(exact) {
        return *listed;
    }
    let real = f64::from(width) / f64::from(height);
    let distance = |r: &Ratio| (real - r.value()).abs() / r.value();
    if let Some(near) = RATIOS
        .iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .filter(|r| distance(r) <= SNAP)
    {
        return *near;
    }
    let divisor = gcd(width, height);
    Ratio::new(width / divisor, height / divisor)
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(width: u32, height: u32) -> (String, String) {
        let t = tags(width, height);
        (t.name.unwrap_or("Custom").to_string(), t.aspect.to_string())
    }

    fn pair(name: &str, aspect: &str) -> (String, String) {
        (name.to_string(), aspect.to_string())
    }

    #[test]
    fn every_standard_names_itself_and_has_a_listed_family() {
        for &(name, width, height) in standards() {
            let t = tags(width, height);
            assert_eq!(t.name, Some(name), "{width} × {height}");
            assert!(RATIOS.contains(&t.family.unwrap()), "{name}: family {:?}", t.family);
        }
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = standards().map(|s| s.0).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn between_standards_the_nearest_above_wins() {
        assert_eq!(shown(960, 720), pair("HD", "4:3"));
        assert_eq!(shown(1600, 896), pair("HD+", "16:9"));
        assert_eq!(shown(864, 480), pair("FWVGA", "16:9"));
        assert_eq!(shown(1360, 768), pair("FWXGA", "16:9"));
        // Wider than every standard of its height: the widest of them.
        assert_eq!(shown(2304, 1536), pair("QXGA", "3:2"));
        assert_eq!(shown(2304, 1296), pair("WQHD", "16:9"));
    }

    #[test]
    fn outside_the_list_is_custom() {
        assert_eq!(tags(160, 90).name, None);
        assert_eq!(tags(10240, 4400).name, None);
        assert_eq!(tags(10240, 4400).family, None);
        assert_eq!(shown(128, 96), pair("SQCIF", "4:3"));
        assert_eq!(shown(8192, 4320), pair("8K FF", "17:9"));
    }

    #[test]
    fn aspect_is_exact_or_snapped_to_a_listed_ratio() {
        assert_eq!(shown(5120, 1440), pair("DQHD", "32:9"));
        assert_eq!(shown(2560, 1080), pair("UWFHD", "21:9"));
        assert_eq!(shown(3440, 1440), pair("UWQHD", "21:9"));
        assert_eq!(shown(1366, 768), pair("FWXGA", "16:9"));
        assert_eq!(shown(1024, 600), pair("WSVGA", "5:3"));
        assert_eq!(shown(4096, 2160), pair("4K DCI", "17:9"));
        assert_eq!(shown(352, 288), pair("CIF", "11:9"));
        assert_eq!(shown(3200, 2048), pair("WQSXGA", "25:16"));
        assert_eq!(shown(720, 480), pair("SD 480p", "3:2"));
        assert_eq!(shown(720, 576), pair("SD 576p", "5:4"));
        assert_eq!(shown(1920, 1200), pair("WUXGA", "16:10"));
        // Nothing within 3%: the real ratio, reduced.
        assert_eq!(aspect(1000, 300).to_string(), "10:3");
        assert_eq!(aspect(0, 0).to_string(), "0:0");
    }
}
