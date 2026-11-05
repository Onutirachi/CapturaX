//! Where the window opens (spec FR-012, data-model.md "Window placement"): the saved placement
//! fitted to the monitors connected now. Pure, so the rules are tested without a display.

use settings::{MIN_HEIGHT, MIN_WIDTH, WindowSettings};

/// A rectangle in physical pixels, desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn contains(&self, (x, y): (i32, i32)) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        let (left, top) = (i64::from(self.x), i64::from(self.y));
        x >= left && y >= top && x < left + i64::from(self.width) && y < top + i64::from(self.height)
    }
}

/// A connected monitor, as the OS reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub name: Option<String>,
    /// The whole monitor, physical pixels.
    pub bounds: Rect,
    /// The part not taken by the taskbar or panels, physical pixels; the bounds where the OS does
    /// not say.
    pub work_area: Rect,
    pub scale: f64,
}

/// The window's placement at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// Physical pixels, desktop coordinates; none when no monitor is known (the OS places it).
    pub position: Option<(i32, i32)>,
    /// Inner size in physical pixels at the chosen monitor's scale.
    pub size: (u32, u32),
    pub maximized: bool,
    /// Index into the monitors given to [`fit`]; none when there were none.
    pub monitor: Option<usize>,
}

/// Fits `saved` to `monitors`: the saved monitor if it is connected, else the primary (an index
/// out of range means the first); the size reduced to the monitor's work area (never below the
/// minimum window size); the window moved so it is fully visible there. Without a saved
/// position, or when the saved monitor is gone, it is centered on the chosen monitor.
pub fn fit(saved: &WindowSettings, monitors: &[Monitor], primary: usize) -> Placement {
    let wanted = (saved.width.max(MIN_WIDTH), saved.height.max(MIN_HEIGHT));
    let saved_position = saved.x.zip(saved.y);
    let Some((index, on_saved)) = choose(saved, saved_position, monitors, primary) else {
        return Placement {
            position: None,
            size: wanted,
            maximized: saved.maximized,
            monitor: None,
        };
    };
    let monitor = &monitors[index];
    let area = monitor.work_area;
    let physical = |logical: u32| (f64::from(logical) * monitor.scale).round() as u32;
    let size = (
        physical(wanted.0).min(area.width).max(physical(MIN_WIDTH)),
        physical(wanted.1).min(area.height).max(physical(MIN_HEIGHT)),
    );
    let position = match saved_position.filter(|_| on_saved) {
        Some((x, y)) => (
            clamp(x, area.x, area.width, size.0),
            clamp(y, area.y, area.height, size.1),
        ),
        None => (
            centered(area.x, area.width, size.0),
            centered(area.y, area.height, size.1),
        ),
    };
    Placement {
        position: Some(position),
        size,
        maximized: saved.maximized,
        monitor: Some(index),
    }
}

/// The monitor to open on, and whether it is the saved one (so the saved position applies).
fn choose(
    saved: &WindowSettings,
    position: Option<(i32, i32)>,
    monitors: &[Monitor],
    primary: usize,
) -> Option<(usize, bool)> {
    if monitors.is_empty() {
        return None;
    }
    let primary = if primary < monitors.len() { primary } else { 0 };
    let Some(name) = &saved.monitor else {
        // No saved monitor means the primary.
        return Some((primary, true));
    };
    let named: Vec<usize> = (0..monitors.len())
        .filter(|&i| monitors[i].name.as_deref() == Some(name.as_str()))
        .collect();
    // Two monitors with one name: the one holding the saved position.
    let holding = position.and_then(|p| named.iter().copied().find(|&i| monitors[i].bounds.contains(p)));
    match holding.or(named.first().copied()) {
        Some(index) => Some((index, true)),
        None => Some((primary, false)),
    }
}

/// `start` moved so `[start, start + size)` lies inside `[area, area + area_size)`; a size larger
/// than the area keeps the start of the area (the title bar stays reachable).
fn clamp(start: i32, area: i32, area_size: u32, size: u32) -> i32 {
    let last = i64::from(area) + i64::from(area_size) - i64::from(size);
    let start = i64::from(start).min(last).max(i64::from(area));
    start as i32
}

fn centered(area: i32, area_size: u32, size: u32) -> i32 {
    let offset = (i64::from(area_size) - i64::from(size)).max(0) / 2;
    (i64::from(area) + offset) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect { x, y, width, height }
    }

    /// A monitor with a 48 px taskbar at the bottom.
    fn monitor(name: &str, x: i32, y: i32, width: u32, height: u32, scale: f64) -> Monitor {
        Monitor {
            name: Some(name.into()),
            bounds: rect(x, y, width, height),
            work_area: rect(x, y, width, height - 48),
            scale,
        }
    }

    fn saved(width: u32, height: u32, position: Option<(i32, i32)>, monitor: Option<&str>) -> WindowSettings {
        WindowSettings {
            width,
            height,
            x: position.map(|p| p.0),
            y: position.map(|p| p.1),
            maximized: false,
            monitor: monitor.map(Into::into),
        }
    }

    fn two_monitors() -> Vec<Monitor> {
        vec![
            monitor("DISPLAY1", 0, 0, 1920, 1080, 1.0),
            monitor("DISPLAY2", 1920, -200, 2560, 1440, 1.5),
        ]
    }

    #[test]
    fn restores_a_placement_that_fits() {
        let placement = fit(&saved(1280, 720, Some((100, 50)), Some("DISPLAY1")), &two_monitors(), 0);
        assert_eq!(
            placement,
            Placement {
                position: Some((100, 50)),
                size: (1280, 720),
                maximized: false,
                monitor: Some(0),
            }
        );
    }

    #[test]
    fn a_missing_monitor_opens_centered_on_the_primary() {
        let placement = fit(
            &saved(1280, 720, Some((3000, 100)), Some("DISPLAY3")),
            &two_monitors(),
            0,
        );
        assert_eq!(placement.monitor, Some(0));
        // Work area 1920 × 1032.
        assert_eq!(placement.position, Some((320, 156)));
        assert_eq!(placement.size, (1280, 720));
    }

    #[test]
    fn an_oversized_window_is_reduced_to_the_work_area() {
        let placement = fit(&saved(2560, 1440, Some((0, 0)), Some("DISPLAY1")), &two_monitors(), 0);
        assert_eq!(placement.size, (1920, 1032));
        assert_eq!(placement.position, Some((0, 0)));
    }

    #[test]
    fn a_window_partly_off_screen_is_moved_inside() {
        let monitors = two_monitors();
        // Right and bottom edges past the first monitor's work area.
        let placement = fit(&saved(1280, 720, Some((1000, 600)), Some("DISPLAY1")), &monitors, 0);
        assert_eq!(placement.position, Some((640, 312)));
        // Left of and above the monitor.
        let placement = fit(&saved(1280, 720, Some((-500, -300)), Some("DISPLAY1")), &monitors, 0);
        assert_eq!(placement.position, Some((0, 0)));
    }

    #[test]
    fn no_saved_position_centers_on_the_saved_monitor() {
        let placement = fit(&saved(1280, 720, None, Some("DISPLAY2")), &two_monitors(), 0);
        assert_eq!(placement.monitor, Some(1));
        // 1920 × 1080 physical at 150% inside the work area 2560 × 1392 at (1920, -200).
        assert_eq!(placement.size, (1920, 1080));
        assert_eq!(placement.position, Some((2240, -44)));
    }

    #[test]
    fn no_saved_monitor_means_the_primary() {
        let placement = fit(&saved(1280, 720, None, None), &two_monitors(), 1);
        assert_eq!(placement.monitor, Some(1));
        let placement = fit(&saved(1280, 720, Some((10, 10)), None), &two_monitors(), 0);
        assert_eq!(placement.position, Some((10, 10)));
    }

    #[test]
    fn mixed_scales_size_the_window_at_the_chosen_monitor_scale() {
        let monitors = two_monitors();
        let at_100 = fit(&saved(1600, 900, Some((0, 0)), Some("DISPLAY1")), &monitors, 0);
        assert_eq!(at_100.size, (1600, 900));
        let at_150 = fit(&saved(1600, 900, Some((2000, 0)), Some("DISPLAY2")), &monitors, 0);
        // 2400 × 1350 physical, reduced to the 1392 px high work area only where needed.
        assert_eq!(at_150.size, (2400, 1350));
        assert_eq!(at_150.position, Some((2000, -158)));
    }

    #[test]
    fn the_size_never_goes_below_the_minimum() {
        let small = vec![monitor("SMALL", 0, 0, 1280, 720, 2.0)];
        let placement = fit(&saved(1280, 720, Some((0, 0)), Some("SMALL")), &small, 0);
        // The minimum 640 × 360 at 200% is larger than the work area: it wins, the top left stays.
        assert_eq!(placement.size, (1280, 720));
        assert_eq!(placement.position, Some((0, 0)));
    }

    #[test]
    fn two_monitors_with_one_name_prefer_the_one_holding_the_position() {
        let monitors = vec![
            monitor("SAME", 0, 0, 1920, 1080, 1.0),
            monitor("SAME", 1920, 0, 1920, 1080, 1.0),
        ];
        let placement = fit(&saved(1280, 720, Some((2000, 100)), Some("SAME")), &monitors, 0);
        assert_eq!(placement.monitor, Some(1));
        assert_eq!(placement.position, Some((2000, 100)));
    }

    #[test]
    fn no_monitors_leaves_the_position_to_the_os() {
        let placement = fit(&saved(1280, 720, Some((10, 10)), Some("DISPLAY1")), &[], 0);
        assert_eq!(placement.position, None);
        assert_eq!(placement.size, (1280, 720));
        assert_eq!(placement.monitor, None);
    }

    #[test]
    fn maximized_is_kept() {
        let mut window = saved(1280, 720, None, None);
        window.maximized = true;
        assert!(fit(&window, &two_monitors(), 0).maximized);
    }

    #[test]
    fn a_primary_index_out_of_range_means_the_first() {
        let placement = fit(&saved(1280, 720, None, None), &two_monitors(), 9);
        assert_eq!(placement.monitor, Some(0));
    }
}
