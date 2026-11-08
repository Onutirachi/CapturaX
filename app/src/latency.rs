//! The latency estimate and the bubble texts (data-model.md "Latency estimate" and "Bubble
//! displays"). Pure: the platform is passed in, so `app` keeps no platform conditions (research
//! R12).

use std::time::Duration;

use capture::mode;
use settings::{FpsDisplay, LatencyDisplay};

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
    match display {
        LatencyDisplay::Measured => format!("{} ms", ms(measured)),
        LatencyDisplay::MeasuredEstimated => format!("{} ms + ~{} ms", ms(measured), ms(estimated)),
        LatencyDisplay::Total => format!("~{} ms", ms(measured + estimated)),
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
