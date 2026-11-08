//! Modes of real devices, from caps recorded with `gst-device-monitor-1.0` (tests/data/README.md):
//! the format chosen per mode and the automatic modes (research R3).

use std::path::Path;

use capturax_capture::Device;
use capturax_capture::mode::{Format, Mode, ModeChoice, default_choice, resolve, shown_automatic};

fn recorded(file: &str) -> Device {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(file);
    let text = std::fs::read_to_string(&path).expect("recorded caps are readable");
    capturax_capture::recorded::device(&text).expect("a recorded device")
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
    let device = recorded("ezcap-linux.txt");
    assert_eq!(
        device.id,
        "v4l2:32ed:3201:ezcap_ezcap_Live_GAMER_ULTRA_00000001@pci-0000:03:00.0-usb-0:3:1.0"
    );
    let modes = device.modes;
    let labels: Vec<String> = modes.iter().map(Mode::text).collect();
    assert_eq!(
        labels,
        [
            "3840x2160@30",
            "2560x1440@60",
            "2560x1440@50",
            "1920x1080@120",
            "1920x1080@60",
            "1920x1080@30",
            "1280x720@60",
        ]
    );
    assert_eq!(format_of(&modes, 1920, 1080, 120), Format::Nv12);
    assert_eq!(format_of(&modes, 1920, 1080, 60), Format::Nv12);
    assert_eq!(format_of(&modes, 2560, 1440, 50), Format::Yuy2);
    assert_eq!(format_of(&modes, 1920, 1080, 30), Format::Bgrx);

    let max_fps = resolve(&ModeChoice::AutoMaxFps, &modes, None).expect("Auto max FPS");
    assert_eq!(max_fps.text(), "1920x1080@120");
    let max_res = resolve(&ModeChoice::AutoMaxResolution, &modes, None).expect("Auto max res");
    assert_eq!(max_res.text(), "2560x1440@60");

    let shown: Vec<ModeChoice> = shown_automatic(&modes, None).into_iter().map(|s| s.0).collect();
    assert_eq!(shown, [ModeChoice::AutoMaxResolution, ModeChoice::AutoMaxFps]);

    // The default is the 1080p60 mode from the list (FR-008, T029).
    let default = resolve(&default_choice(None, &modes), &modes, None).expect("default");
    assert_eq!(default.text(), "1920x1080@60");
    assert_eq!(default.format, Format::Nv12);
}

#[test]
fn c920_on_windows() {
    // Media Foundation, NV12 declared full range: the Frame Server rule applies (research R3).
    let device = recorded("c920-windows.txt");
    assert_eq!(device.name, "HD Pro Webcam C920");
    assert!(device.id.starts_with(r"mf:\\?\usb#vid_046d&pid_082d"), "{}", device.id);
    let modes = device.modes;

    // YUY2 before the Frame Server's NV12 where both exist; NV12 where YUY2 is too slow.
    assert_eq!(format_of(&modes, 640, 480, 30), Format::Yuy2);
    assert_eq!(format_of(&modes, 1920, 1080, 30), Format::Nv12);

    let max_res = resolve(&ModeChoice::AutoMaxResolution, &modes, None).expect("Auto max res");
    assert_eq!(max_res.text(), "1920x1080@30");
    let max_fps = resolve(&ModeChoice::AutoMaxFps, &modes, None).expect("Auto max FPS");
    assert_eq!(max_fps.text(), "1920x1080@30");

    // Both resolve to 1080p30: only Auto max resolution is shown, and no Auto source.
    let shown: Vec<ModeChoice> = shown_automatic(&modes, None).into_iter().map(|s| s.0).collect();
    assert_eq!(shown, [ModeChoice::AutoMaxResolution]);

    // No 1080p near 60: the default is Auto max resolution (FR-008).
    assert_eq!(default_choice(None, &modes), ModeChoice::AutoMaxResolution);

    // Largest first, one entry per size and rate.
    assert_eq!(modes[0].width, 2304);
    for (i, a) in modes.iter().enumerate() {
        for b in &modes[i + 1..] {
            let same = a.width == b.width && a.height == b.height && (a.fps_f64() - b.fps_f64()).abs() < 0.5;
            assert!(!same, "{} listed twice", a.text());
        }
    }
}
