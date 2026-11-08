//! The latency estimate and the bubble texts (data-model.md "Latency estimate" and "Bubble
//! displays"). Pure: the platform is passed in, so `app` keeps no platform conditions (research
//! R12).

use std::time::Duration;

use capture::mode;
use settings::{FpsDisplay, LatencyDisplay, ModeDisplay};

/// The platform the estimate is for: on Linux the USB transfer is already inside the measured part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
}

/// Card processing and the monitor's own delay on Windows (calibrated in T040).
pub const C_WINDOWS: Duration = Duration::from_millis(5);

/// Card processing and the monitor's own delay on Linux (calibrated in T040).
pub const C_LINUX: Duration = Duration::from_millis(5);

impl Platform {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if platform::IS_WINDOWS {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }

    /// The calibrated constant of this platform.
    pub fn calibration(self) -> Duration {
        match self {
            Platform::Windows => C_WINDOWS,
            Platform::Linux => C_LINUX,
        }
    }
}

/// The part of glass-to-glass latency the app cannot measure:
/// `F + (Windows ? F : 0) + C_platform + (vrr ? 0 : R / 2)`.
pub fn estimate(
    platform: Platform,
    source_period: Duration,
    refresh_period: Duration,
    vrr: bool,
    c_platform: Duration,
) -> Duration {
    // The card receiving the whole frame from HDMI.
    let mut total = source_period;
    // The USB transfer, already inside the measured part on Linux.
    if platform == Platform::Windows {
        total += source_period;
    }
    total += c_platform;
    // The average wait for the next refresh.
    if !vrr {
        total += refresh_period / 2;
    }
    total
}

/// The latency bubble: "3 ms", "3 ms + ~24 ms", "~27 ms" (the estimate marked with "~").
pub fn latency_text(display: LatencyDisplay, measured: Duration, estimated: Duration) -> String {
    let parts = LatencyParts::new(measured, estimated);
    match display {
        LatencyDisplay::Measured => parts.measured,
        LatencyDisplay::MeasuredEstimated => format!("{} + {}", parts.measured, parts.estimated),
        LatencyDisplay::Total => parts.total,
    }
}

/// Each part of the latency as the bubble writes it, for its tooltip: "3 ms", "~24 ms", "~27 ms".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatencyParts {
    pub measured: String,
    pub estimated: String,
    /// Rounded once, after the sum.
    pub total: String,
}

impl LatencyParts {
    pub fn new(measured: Duration, estimated: Duration) -> Self {
        Self {
            measured: format!("{} ms", ms(measured)),
            estimated: format!("~{} ms", ms(estimated)),
            total: format!("~{} ms", ms(measured + estimated)),
        }
    }
}

/// The frame rate bubble: "60 FPS", "111 / 120 FPS", "120 FPS"; the nominal rate as the mode tags
/// write it (rounded unless fractional: "59.94").
pub fn fps_text(display: FpsDisplay, presented: u32, nominal: (i32, i32)) -> String {
    match display {
        FpsDisplay::Presented => format!("{presented} FPS"),
        FpsDisplay::PresentedNominal => format!("{presented} / {} FPS", mode::rate_text(nominal)),
        FpsDisplay::Nominal => format!("{} FPS", mode::rate_text(nominal)),
    }
}

/// The latency display as the bubble's menu numbers it (info-bubbles.slint): 0 measured, 1
/// measured + estimated, 2 total; an unknown number is the default.
pub fn latency_display(index: i32) -> LatencyDisplay {
    match index {
        0 => LatencyDisplay::Measured,
        2 => LatencyDisplay::Total,
        _ => LatencyDisplay::MeasuredEstimated,
    }
}

pub fn latency_index(display: LatencyDisplay) -> i32 {
    match display {
        LatencyDisplay::Measured => 0,
        LatencyDisplay::MeasuredEstimated => 1,
        LatencyDisplay::Total => 2,
    }
}

/// The frame rate display as the bubble's menu numbers it: 0 presented, 1 presented / nominal,
/// 2 nominal; an unknown number is the default.
pub fn fps_display(index: i32) -> FpsDisplay {
    match index {
        0 => FpsDisplay::Presented,
        2 => FpsDisplay::Nominal,
        _ => FpsDisplay::PresentedNominal,
    }
}

pub fn fps_index(display: FpsDisplay) -> i32 {
    match display {
        FpsDisplay::Presented => 0,
        FpsDisplay::PresentedNominal => 1,
        FpsDisplay::Nominal => 2,
    }
}

/// The mode display as the bubble's menu numbers it: 0 size, 1 size + name, 2 size + aspect,
/// 3 size + name + aspect; an unknown number is the default.
pub fn mode_display(index: i32) -> ModeDisplay {
    match index {
        0 => ModeDisplay::Size,
        1 => ModeDisplay::SizeName,
        2 => ModeDisplay::SizeAspect,
        _ => ModeDisplay::SizeNameAspect,
    }
}

pub fn mode_index(display: ModeDisplay) -> i32 {
    match display {
        ModeDisplay::Size => 0,
        ModeDisplay::SizeName => 1,
        ModeDisplay::SizeAspect => 2,
        ModeDisplay::SizeNameAspect => 3,
    }
}

/// Whole milliseconds, rounded to the nearest.
fn ms(d: Duration) -> u128 {
    (d.as_micros() + 500) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// 60 fps source and 60 Hz monitor, in microseconds to keep the sums exact.
    const F60: Duration = Duration::from_micros(16_667);
    const R60: Duration = Duration::from_micros(16_666);

    #[test]
    fn windows_counts_the_usb_transfer() {
        let e = estimate(Platform::Windows, F60, R60, false, ms(5));
        assert_eq!(e, F60 + F60 + ms(5) + R60 / 2);
    }

    #[test]
    fn linux_leaves_the_usb_transfer_to_the_measured_part() {
        let e = estimate(Platform::Linux, F60, R60, false, ms(5));
        assert_eq!(e, F60 + ms(5) + R60 / 2);
    }

    #[test]
    fn variable_refresh_has_no_refresh_wait() {
        assert_eq!(estimate(Platform::Windows, F60, R60, true, ms(5)), F60 + F60 + ms(5));
        assert_eq!(estimate(Platform::Linux, F60, R60, true, ms(5)), F60 + ms(5));
    }

    #[test]
    fn the_constant_is_added_as_given() {
        let base = estimate(Platform::Linux, F60, R60, false, Duration::ZERO);
        assert_eq!(estimate(Platform::Linux, F60, R60, false, ms(12)), base + ms(12));
    }

    #[test]
    fn calibration_follows_the_platform() {
        assert_eq!(Platform::Windows.calibration(), C_WINDOWS);
        assert_eq!(Platform::Linux.calibration(), C_LINUX);
    }

    #[test]
    fn latency_texts() {
        let (measured, estimated) = (ms(3), ms(24));
        assert_eq!(latency_text(LatencyDisplay::Measured, measured, estimated), "3 ms");
        assert_eq!(
            latency_text(LatencyDisplay::MeasuredEstimated, measured, estimated),
            "3 ms + ~24 ms"
        );
        assert_eq!(latency_text(LatencyDisplay::Total, measured, estimated), "~27 ms");
        assert_eq!(
            LatencyParts::new(measured, estimated),
            LatencyParts {
                measured: "3 ms".into(),
                estimated: "~24 ms".into(),
                total: "~27 ms".into(),
            }
        );
    }

    #[test]
    fn latency_rounds_to_the_nearest_millisecond() {
        let (measured, estimated) = (Duration::from_micros(2_600), Duration::from_micros(24_400));
        assert_eq!(
            latency_text(LatencyDisplay::MeasuredEstimated, measured, estimated),
            "3 ms + ~24 ms"
        );
        // The total is rounded once, after the sum.
        assert_eq!(latency_text(LatencyDisplay::Total, measured, estimated), "~27 ms");
        assert_eq!(
            latency_text(LatencyDisplay::Measured, Duration::from_micros(2_499), ms(0)),
            "2 ms"
        );
    }

    #[test]
    fn display_numbers_round_trip() {
        for d in [
            LatencyDisplay::Measured,
            LatencyDisplay::MeasuredEstimated,
            LatencyDisplay::Total,
        ] {
            assert_eq!(latency_display(latency_index(d)), d);
        }
        for d in [FpsDisplay::Presented, FpsDisplay::PresentedNominal, FpsDisplay::Nominal] {
            assert_eq!(fps_display(fps_index(d)), d);
        }
        for d in [
            ModeDisplay::Size,
            ModeDisplay::SizeName,
            ModeDisplay::SizeAspect,
            ModeDisplay::SizeNameAspect,
        ] {
            assert_eq!(mode_display(mode_index(d)), d);
        }
        assert_eq!(mode_display(9), ModeDisplay::SizeNameAspect);
        assert_eq!(latency_display(7), LatencyDisplay::MeasuredEstimated);
        assert_eq!(fps_display(-1), FpsDisplay::PresentedNominal);
    }

    #[test]
    fn fps_texts() {
        assert_eq!(fps_text(FpsDisplay::Presented, 60, (60, 1)), "60 FPS");
        assert_eq!(fps_text(FpsDisplay::PresentedNominal, 111, (120, 1)), "111 / 120 FPS");
        assert_eq!(fps_text(FpsDisplay::Nominal, 111, (120, 1)), "120 FPS");
    }

    #[test]
    fn fractional_nominal_rates_keep_their_decimals() {
        assert_eq!(fps_text(FpsDisplay::Nominal, 60, (60_000, 1_001)), "59.94 FPS");
        assert_eq!(
            fps_text(FpsDisplay::PresentedNominal, 59, (60_000, 1_001)),
            "59 / 59.94 FPS"
        );
        // A rate a driver reports as a near-whole fraction is rounded.
        assert_eq!(fps_text(FpsDisplay::Nominal, 30, (10_000_000, 333_333)), "30 FPS");
    }
}
