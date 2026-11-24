//! The ambient light around the video (preview extras research R1–R4a, data-model.md "Glow"):
//! the pure parts. The glow eases a small color field of the video over time, draws it only in
//! the bars between the letterboxed video and its area, and fades with the luminance gate and the
//! toggle.

use std::time::{Duration, Instant};

// The look's tune values (research R2): they start from the mockup and are set with the
// maintainer on the ezcap with the PS5 (T008). A change in feel goes into the spec's FR-002 only
// if the variant changes.

/// How much the field is enlarged on each side of the video area: 5 % (the mockup's −5 %, ×1.1).
pub const EDGE_GROW: f32 = 0.05;
/// The Gaussian blur's deviation on the 32 × 18 field, in texels.
pub const BLUR_SIGMA: f32 = 2.0;
/// The saturation applied when drawing.
pub const SATURATION: f32 = 1.4;
/// The glow's alpha before the gate, the video's opacity and the toggle fade.
pub const ALPHA: f32 = 0.7;
/// The luminance where the gate starts to open.
pub const GATE_LOW: f32 = 0.04;
/// The luminance where the gate is fully open.
pub const GATE_HIGH: f32 = 0.20;
/// How long the toggle takes to fade the glow in or out.
pub const FADE: Duration = Duration::from_millis(250);

/// The easing's time constant (research R1, the mockup's τ).
pub const TAU: Duration = Duration::from_millis(150);
/// The longest time step the easing takes; a longer pause fills the history instead (R4a).
pub const MAX_STEP: Duration = Duration::from_millis(100);
/// The shortest time between two updates (research R1): every frame at 60 fps even with the
/// frames' arrival jitter, every other frame at 120 fps.
pub const UPDATE_INTERVAL: Duration = Duration::from_millis(15);

/// How much of the new field one update blends into the history: `1 − exp(−dt / τ)`, with `dt`
/// clamped to [`MAX_STEP`], so the easing does not depend on the frame rate.
pub fn ease_factor(dt: Duration) -> f32 {
    let dt = dt.min(MAX_STEP);
    1.0 - (-dt.as_secs_f32() / TAU.as_secs_f32()).exp()
}

/// The luminance gate: 0 at or below [`GATE_LOW`], 1 at or above [`GATE_HIGH`], linear between,
/// so a dark frame fades to the plain background instead of a grey haze (FR-003).
pub fn gate(luminance: f32) -> f32 {
    ((luminance - GATE_LOW) / (GATE_HIGH - GATE_LOW)).clamp(0.0, 1.0)
}

/// The bars between the letterboxed `video` (x, y, width, height in physical pixels, as the
/// letterbox gives it) and its `area` (the video area's scissor rectangle): two side bars, or a
/// top and a bottom bar, as whole-pixel rectangles (research R3). A bar not wider than 1 physical
/// pixel is none, so a video that fills the area has no bar and costs nothing.
pub fn bars(area: [u32; 4], video: [f32; 4]) -> [Option<[u32; 4]>; 2] {
    let [ax, ay, aw, ah] = area.map(i64::from);
    let (ax1, ay1) = (ax + aw, ay + ah);
    // The video's edges, whole pixels, inside the area.
    let edge = |v: f32, low: i64, high: i64| (v.round() as i64).clamp(low, high);
    let vx0 = edge(video[0], ax, ax1);
    let vx1 = edge(video[0] + video[2], vx0, ax1);
    let vy0 = edge(video[1], ay, ay1);
    let vy1 = edge(video[1] + video[3], vy0, ay1);
    let bar = |x: i64, y: i64, w: i64, h: i64, thickness: i64| {
        (thickness > 1 && w > 0 && h > 0).then(|| [x, y, w, h].map(|v| v as u32))
    };
    let side = (vx0 - ax).max(ax1 - vx1);
    let top = (vy0 - ay).max(ay1 - vy1);
    if side >= top {
        [
            bar(ax, ay, vx0 - ax, ah, vx0 - ax),
            bar(vx1, ay, ax1 - vx1, ah, ax1 - vx1),
        ]
    } else {
        [
            bar(ax, ay, aw, vy0 - ay, vy0 - ay),
            bar(ax, vy1, aw, ay1 - vy1, ay1 - vy1),
        ]
    }
}

/// What an update does now (research R1, R4a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Due {
    /// Too soon after the last update: skip it.
    Wait,
    /// Ease the history towards the field with the real time step since the last update.
    Ease(Duration),
    /// Set the history to the field directly (factor 1): the first update, or the first after a
    /// pause longer than [`MAX_STEP`], so a stale field never shows.
    Fill,
}

/// Whether an update runs at `now`, given the `last` one: at most every [`UPDATE_INTERVAL`] (at
/// 120 fps every other frame), with the real time step, or a fill after a pause.
pub fn due(last: Option<Instant>, now: Instant) -> Due {
    let Some(last) = last else {
        return Due::Fill;
    };
    let dt = now.saturating_duration_since(last);
    if dt > MAX_STEP {
        Due::Fill
    } else if dt < UPDATE_INTERVAL {
        Due::Wait
    } else {
        Due::Ease(dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: f64) -> Duration {
        Duration::from_micros((v * 1000.0).round() as u64)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn ease_factor_follows_the_time_constant() {
        assert_eq!(ease_factor(Duration::ZERO), 0.0);
        assert!(close(ease_factor(MAX_STEP), 1.0 - (-100.0f32 / 150.0).exp()));
        assert!(close(ease_factor(ms(16.7)), 1.0 - (-16.7f32 / 150.0).exp()));
        // Two 120 fps steps give the same as one 60 fps step: the rate does not change the feel.
        let half = ease_factor(ms(8.35));
        assert!(close(1.0 - (1.0 - half) * (1.0 - half), ease_factor(ms(16.7))));
    }

    #[test]
    fn ease_factor_clamps_the_step() {
        assert_eq!(ease_factor(Duration::from_secs(5)), ease_factor(MAX_STEP));
        assert!(ease_factor(MAX_STEP) < 0.5);
    }

    #[test]
    fn one_white_frame_does_not_flash() {
        // FR-003: a black history meets one white field at 60 fps, then black again.
        let bound = 1.0 - (-16.7f32 / 150.0).exp();
        let mut history = 0.0f32;
        let mut highest = 0.0f32;
        for field in [1.0, 0.0, 0.0, 0.0] {
            history += (field - history) * ease_factor(ms(16.7));
            highest = highest.max(history);
        }
        assert!(highest <= bound + 1e-6, "{highest} > {bound}");
        assert!(highest > 0.1 && highest < 0.11, "{highest}");
    }

    #[test]
    fn gate_values() {
        assert_eq!(gate(0.0), 0.0);
        assert_eq!(gate(GATE_LOW), 0.0);
        assert!(close(gate(0.12), 0.5));
        assert_eq!(gate(GATE_HIGH), 1.0);
        assert_eq!(gate(1.0), 1.0);
        assert_eq!(gate(-1.0), 0.0);
        assert_eq!(gate(GATE_HIGH), ((GATE_HIGH - 0.04) / 0.16).clamp(0.0, 1.0));
    }

    #[test]
    fn no_bars_when_the_video_fills_the_area() {
        assert_eq!(bars([0, 0, 1920, 1080], [0.0, 0.0, 1920.0, 1080.0]), [None, None]);
        // A 1 pixel bar from rounding is none.
        assert_eq!(bars([0, 32, 1921, 1080], [0.0, 32.0, 1920.0, 1080.0]), [None, None]);
        assert_eq!(bars([0, 0, 1922, 1080], [1.0, 0.0, 1920.0, 1080.0]), [None, None]);
    }

    #[test]
    fn side_bars() {
        // 4:3 video in a 16:9 area with an offset (the letterbox test's case).
        assert_eq!(
            bars([0, 32, 1600, 900], [200.0, 32.0, 1200.0, 900.0]),
            [Some([0, 32, 200, 900]), Some([1400, 32, 200, 900])]
        );
        // Uneven by rounding: each bar keeps its own width.
        assert_eq!(
            bars([10, 0, 805, 600], [12.0, 0.0, 800.0, 600.0]),
            [Some([10, 0, 2, 600]), Some([812, 0, 3, 600])]
        );
        // Only one side wider than 1 pixel.
        assert_eq!(
            bars([0, 0, 803, 600], [1.0, 0.0, 800.0, 600.0]),
            [None, Some([801, 0, 2, 600])]
        );
    }

    #[test]
    fn top_and_bottom_bars() {
        // 16:9 video in a 4:3 area.
        assert_eq!(
            bars([0, 0, 800, 600], [0.0, 75.0, 800.0, 450.0]),
            [Some([0, 0, 800, 75]), Some([0, 525, 800, 75])]
        );
        // A 1 pixel side gap from rounding does not hide the top and bottom bars.
        assert_eq!(
            bars([0, 0, 801, 600], [0.0, 75.0, 800.0, 450.0]),
            [Some([0, 0, 801, 75]), Some([0, 525, 801, 75])]
        );
    }

    #[test]
    fn bars_stay_inside_the_area() {
        // A video larger than the area (it never is, but the scissor clips it anyway).
        assert_eq!(bars([0, 0, 800, 600], [-10.0, -10.0, 820.0, 620.0]), [None, None]);
        assert_eq!(bars([0, 0, 0, 0], [0.0, 0.0, 0.0, 0.0]), [None, None]);
    }

    #[test]
    fn pacing() {
        let start = Instant::now();
        let at = |v: f64| start + ms(v);
        assert_eq!(due(None, start), Due::Fill);
        // 120 fps: every other frame updates, with the real step.
        assert_eq!(due(Some(start), at(8.3)), Due::Wait);
        assert_eq!(due(Some(start), at(14.9)), Due::Wait);
        assert_eq!(due(Some(start), at(16.7)), Due::Ease(ms(16.7)));
        // 60 fps with arrival jitter: a frame a little early still updates.
        assert_eq!(due(Some(start), at(15.0)), Due::Ease(ms(15.0)));
        assert_eq!(due(Some(start), at(16.0)), Due::Ease(ms(16.0)));
        // 30 fps: every frame.
        assert_eq!(due(Some(start), at(33.3)), Due::Ease(ms(33.3)));
        // Up to the longest step it still eases; past it, it fills.
        assert_eq!(due(Some(start), at(100.0)), Due::Ease(MAX_STEP));
        assert_eq!(due(Some(start), at(100.1)), Due::Fill);
        assert_eq!(due(Some(start), at(60_000.0)), Due::Fill);
        // A clock that went back waits instead of easing by a negative step.
        assert_eq!(due(Some(at(20.0)), start), Due::Wait);
    }
}
