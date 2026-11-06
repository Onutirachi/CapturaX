//! Modes of real devices, from caps recorded with `gst-device-monitor-1.0` (tests/data/README.md):
//! the format chosen per mode and the automatic modes (research R3).

use std::path::Path;

use capturax_capture::mode::{Format, Mode, ModeChoice, modes_from_caps, nv12_declared_full, resolve, shown_automatic};

/// The caps and the properties of one recorded device block.
struct Recorded {
    caps: gst::Caps,
    api: String,
}

fn recorded(file: &str) -> Recorded {
    gst::init().expect("GStreamer initializes");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(file);
    let text = std::fs::read_to_string(&path).expect("recorded caps are readable");
    let mut structures = Vec::new();
    let mut in_caps = false;
    let mut api = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(first) = line.strip_prefix("caps  :") {
            in_caps = true;
            structures.push(first.trim().to_string());
        } else if line.starts_with("properties:") {
            in_caps = false;
        } else if in_caps && !line.is_empty() {
            structures.push(line.to_string());
        } else if let Some(value) = line.strip_prefix("device.api = ") {
            api = value.trim().to_string();
        }
    }
    let caps = structures.join("; ").parse().expect("recorded caps parse");
    Recorded { caps, api }
}

fn modes(file: &str) -> Vec<Mode> {
    let device = recorded(file);
    let frame_server = device.api == "mediafoundation" && nv12_declared_full(&device.caps);
    modes_from_caps(&device.caps, frame_server)
}

fn format_of(modes: &[Mode], width: u32, height: u32, fps: i32) -> Format {
    modes
        .iter()
        .find(|m| m.width == width && m.height == height && (m.fps_f64() - f64::from(fps)).abs() < 0.5)
        .unwrap_or_else(|| panic!("{width}x{height}@{fps} listed"))
        .format
        .clone()
}

#[test]
fn ezcap_on_linux() {
    let modes = modes("ezcap-linux.txt");
    let labels: Vec<String> = modes.iter().map(Mode::label).collect();
    assert_eq!(
        labels,
        [
            "3840 × 2160 · 30",
            "2560 × 1440 · 60",
            "2560 × 1440 · 50",
            "1920 × 1080 · 120",
            "1920 × 1080 · 60",
            "1920 × 1080 · 30",
            "1280 × 720 · 60",
        ]
    );
    assert_eq!(format_of(&modes, 1920, 1080, 120), Format::Nv12);
    assert_eq!(format_of(&modes, 1920, 1080, 60), Format::Nv12);
    assert_eq!(format_of(&modes, 2560, 1440, 50), Format::Yuy2);
    assert_eq!(format_of(&modes, 1920, 1080, 30), Format::Bgrx);

    let max_fps = resolve(&ModeChoice::AutoMaxFps, &modes, None).expect("Auto max FPS");
    assert_eq!(max_fps.label(), "1920 × 1080 · 120");
    let max_res = resolve(&ModeChoice::AutoMaxResolution, &modes, None).expect("Auto max res");
    assert_eq!(max_res.label(), "2560 × 1440 · 60");

    let shown: Vec<ModeChoice> = shown_automatic(&modes, None).into_iter().map(|s| s.0).collect();
    assert_eq!(shown, [ModeChoice::AutoMaxResolution, ModeChoice::AutoMaxFps]);
}

#[test]
fn c920_on_windows() {
    let device = recorded("c920-windows.txt");
    assert_eq!(device.api, "mediafoundation");
    assert!(nv12_declared_full(&device.caps));
    let modes = modes("c920-windows.txt");

    // YUY2 before the Frame Server's NV12 where both exist; NV12 where YUY2 is too slow.
    assert_eq!(format_of(&modes, 640, 480, 30), Format::Yuy2);
    assert_eq!(format_of(&modes, 1920, 1080, 30), Format::Nv12);

    let max_res = resolve(&ModeChoice::AutoMaxResolution, &modes, None).expect("Auto max res");
    assert_eq!(max_res.label(), "1920 × 1080 · 30");
    let max_fps = resolve(&ModeChoice::AutoMaxFps, &modes, None).expect("Auto max FPS");
    assert_eq!(max_fps.label(), "1920 × 1080 · 30");

    // Both resolve to 1080p30: only Auto max resolution is shown, and no Auto source.
    let shown: Vec<ModeChoice> = shown_automatic(&modes, None).into_iter().map(|s| s.0).collect();
    assert_eq!(shown, [ModeChoice::AutoMaxResolution]);

    // Largest first, one entry per size and rate.
    assert_eq!(modes[0].width, 2304);
    for (i, a) in modes.iter().enumerate() {
        for b in &modes[i + 1..] {
            let same = a.width == b.width && a.height == b.height && (a.fps_f64() - b.fps_f64()).abs() < 0.5;
            assert!(!same, "{} listed twice", a.label());
        }
    }
}
