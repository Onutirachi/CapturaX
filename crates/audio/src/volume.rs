//! The preview's volume (research R8, data-model.md "Volume"): one app-side gain with the cubic
//! law, the speaker icon, the mouse wheel step and the 10 ms ramp that keeps changes click-free.

/// The slider's step per mouse wheel notch, in percent.
const WHEEL_STEP: i32 = 5;

/// The ramp to a new gain lasts this many output frames per second of rate (10 ms).
const RAMP_PER_SECOND: u32 = 100;

/// The gain for a slider level (0–100 %): `(level/100)³`, 0 when muted. Levels above 100 count
/// as 100.
pub fn gain(level: u8, muted: bool) -> f32 {
    if muted {
        return 0.0;
    }
    (f32::from(level.min(100)) / 100.0).powi(3)
}

/// The speaker icon next to the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeIcon {
    /// Muted or 0 (`volume_off`).
    Off,
    /// Up to 33 (`volume_1`).
    Low,
    /// Up to 66 (`volume_2`).
    Medium,
    /// Above 66 (`volume`).
    High,
}

/// The icon for a level: muted or 0 → `Off`; ≤ 33 → `Low`; ≤ 66 → `Medium`; else `High`.
pub fn icon(level: u8, muted: bool) -> VolumeIcon {
    match level {
        _ if muted => VolumeIcon::Off,
        0 => VolumeIcon::Off,
        1..=33 => VolumeIcon::Low,
        34..=66 => VolumeIcon::Medium,
        _ => VolumeIcon::High,
    }
}

/// The level after turning the mouse wheel: ±5 per notch (positive is up), clamped to 0–100.
pub fn wheel(level: u8, notches: i32) -> u8 {
    let moved = i32::from(level).saturating_add(notches.saturating_mul(WHEEL_STEP));
    moved.clamp(0, 100) as u8
}

/// Moves a gain linearly to its target over 10 ms of output frames, so moving the slider or
/// muting never clicks.
#[derive(Debug, Clone)]
pub struct Ramp {
    current: f32,
    target: f32,
    step: f32,
    /// Output frames left until `current` reaches `target`.
    left: u32,
    /// Output frames in 10 ms.
    length: u32,
}

impl Ramp {
    /// A ramp resting at `gain`, for an output at `rate` frames per second.
    pub fn new(gain: f32, rate: u32) -> Self {
        Self {
            current: gain,
            target: gain,
            step: 0.0,
            left: 0,
            length: (rate / RAMP_PER_SECOND).max(1),
        }
    }

    /// Starts moving from the current gain to `target`; a ramp in progress restarts from where it
    /// is.
    pub fn set_target(&mut self, target: f32) {
        if target == self.target {
            return;
        }
        self.target = target;
        self.left = self.length;
        self.step = (target - self.current) / self.length as f32;
    }

    /// The gain for the next output frame.
    pub fn advance(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.current = if self.left == 0 {
                self.target
            } else {
                self.current + self.step
            };
        }
        self.current
    }

    /// The gain of the last output frame.
    pub fn current(&self) -> f32 {
        self.current
    }

    /// The gain the ramp moves to.
    pub fn target(&self) -> f32 {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_is_cubic_and_zero_when_muted() {
        assert_eq!(gain(100, false), 1.0);
        assert_eq!(gain(0, false), 0.0);
        assert!((gain(50, false) - 0.125).abs() < 1e-6);
        assert!((gain(80, false) - 0.512).abs() < 1e-6);
        assert_eq!(gain(80, true), 0.0);
        assert_eq!(gain(100, true), 0.0);
        assert_eq!(gain(250, false), 1.0);
    }

    #[test]
    fn icon_follows_the_level() {
        assert_eq!(icon(80, true), VolumeIcon::Off);
        assert_eq!(icon(0, false), VolumeIcon::Off);
        assert_eq!(icon(1, false), VolumeIcon::Low);
        assert_eq!(icon(33, false), VolumeIcon::Low);
        assert_eq!(icon(34, false), VolumeIcon::Medium);
        assert_eq!(icon(66, false), VolumeIcon::Medium);
        assert_eq!(icon(67, false), VolumeIcon::High);
        assert_eq!(icon(100, false), VolumeIcon::High);
    }

    #[test]
    fn wheel_moves_five_per_notch_within_bounds() {
        assert_eq!(wheel(80, 1), 85);
        assert_eq!(wheel(80, -2), 70);
        assert_eq!(wheel(80, 0), 80);
        assert_eq!(wheel(98, 1), 100);
        assert_eq!(wheel(3, -1), 0);
        assert_eq!(wheel(50, i32::MAX), 100);
        assert_eq!(wheel(50, i32::MIN), 0);
    }

    #[test]
    fn ramp_reaches_the_target_in_10_ms() {
        let mut ramp = Ramp::new(0.0, 48_000);
        ramp.set_target(1.0);
        let gains: Vec<f32> = (0..480).map(|_| ramp.advance()).collect();
        assert_eq!(gains[479], 1.0);
        assert!(gains[478] < 1.0);
        assert_eq!(ramp.advance(), 1.0);
    }

    #[test]
    fn ramp_steps_stay_small() {
        let mut ramp = Ramp::new(0.0, 48_000);
        ramp.set_target(1.0);
        let mut previous = ramp.current();
        for _ in 0..1000 {
            let next = ramp.advance();
            assert!(next >= previous);
            assert!(next - previous <= 1.0 / 480.0 + 1e-6, "step {}", next - previous);
            previous = next;
        }
    }

    #[test]
    fn ramp_restarts_from_where_it_is() {
        let mut ramp = Ramp::new(1.0, 48_000);
        ramp.set_target(0.0);
        for _ in 0..240 {
            ramp.advance();
        }
        let middle = ramp.current();
        assert!((middle - 0.5).abs() < 1e-3);
        ramp.set_target(1.0);
        let first = ramp.advance();
        assert!((first - middle - 0.5 / 480.0).abs() < 1e-5);
        for _ in 0..479 {
            ramp.advance();
        }
        assert_eq!(ramp.current(), 1.0);
        assert_eq!(ramp.target(), 1.0);
    }

    #[test]
    fn ramp_holds_without_a_new_target() {
        let mut ramp = Ramp::new(0.512, 44_100);
        ramp.set_target(0.512);
        for _ in 0..10 {
            assert_eq!(ramp.advance(), 0.512);
        }
    }
}
