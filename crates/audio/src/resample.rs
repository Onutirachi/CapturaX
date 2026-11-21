//! Rate and channel conversion for the bridge (research R4): 4-point cubic (Catmull-Rom)
//! interpolation of interleaved frames at a fractional position, which serves both a device
//! rate change (48 ↔ 44.1 kHz) and the rate follower's ±1000 ppm, and the map from the input's
//! channels to the output's.

use std::f32::consts::FRAC_1_SQRT_2;

/// −3 dB, for the channels mixed into both sides or into their side.
const MINUS_3_DB: f32 = FRAC_1_SQRT_2;

/// The rate and channel count of a device stream (samples are interleaved `f32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub rate: u32,
    pub channels: u16,
}

/// Catmull-Rom between `p1` (at `t` = 0) and `p2` (at `t` = 1).
fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
    let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
    let c = -0.5 * p0 + 0.5 * p2;
    ((a * t + b) * t + c) * t + p1
}

/// Writes into `out` (one frame of `channels` samples) the frame at the fractional frame
/// `position` of the interleaved `samples`. The four frames around it must be present:
/// `1 ≤ position` and `position + 2 < frames`.
pub fn interpolate(samples: &[f32], channels: usize, position: f64, out: &mut [f32]) {
    let index = position.floor() as usize;
    let t = (position - index as f64) as f32;
    debug_assert!(index >= 1 && (index + 2) * channels < samples.len());
    let start = (index - 1) * channels;
    for (channel, sample) in out[..channels].iter_mut().enumerate() {
        let at = |frame: usize| samples[start + frame * channels + channel];
        *sample = cubic(at(0), at(1), at(2), at(3), t);
    }
}

/// How one input frame becomes one output frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelMap {
    input: usize,
    output: usize,
}

impl ChannelMap {
    pub fn new(input: u16, output: u16) -> Self {
        Self {
            input: usize::from(input.max(1)),
            output: usize::from(output.max(1)),
        }
    }

    /// Maps one `input` frame to one `output` frame: mono to every output channel; up to the
    /// output's count, each input channel to the same output channel (stereo to 0 and 1) and the
    /// others silent; more input channels than the output's mixed down to stereo (channels 0
    /// and 1 kept, center and LFE (2, 3) added at −3 dB to both sides, the rest added at −3 dB
    /// to their side by even/odd index) and clamped to ±1, averaged for a mono output.
    pub fn apply(&self, input: &[f32], output: &mut [f32]) {
        let input = &input[..self.input];
        let output = &mut output[..self.output];
        if self.input == 1 {
            output.fill(input[0]);
        } else if self.input <= self.output {
            output[..self.input].copy_from_slice(input);
            output[self.input..].fill(0.0);
        } else {
            let (left, right) = mix_down(input);
            if self.output == 1 {
                output[0] = (left + right) * 0.5;
            } else {
                output[0] = left;
                output[1] = right;
                output[2..].fill(0.0);
            }
        }
    }
}

/// Stereo from more than two channels in the WAVEFORMATEXTENSIBLE/PipeWire order
/// (FL FR FC LFE …), clamped to ±1.
fn mix_down(input: &[f32]) -> (f32, f32) {
    let (mut left, mut right) = (input[0], input[1]);
    for (index, &sample) in input.iter().enumerate().skip(2) {
        let sample = sample * MINUS_3_DB;
        match index {
            2 | 3 => {
                left += sample;
                right += sample;
            }
            _ if index % 2 == 0 => left += sample,
            _ => right += sample,
        }
    }
    (left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const AMPLITUDE: f64 = 0.5;

    /// One second of a 1 kHz sine at 48 kHz, mono.
    fn sine_48k() -> Vec<f32> {
        (0..48_000)
            .map(|n| (AMPLITUDE * (TAU * 1000.0 * n as f64 / 48_000.0).sin()) as f32)
            .collect()
    }

    /// The sine read at a 48000/44100 step: about one second at 44.1 kHz.
    fn resampled() -> Vec<f64> {
        let input = sine_48k();
        let step = 48_000.0 / 44_100.0;
        let mut out = Vec::new();
        let mut position = 1.0;
        let mut frame = [0.0f32];
        while position + 2.0 < input.len() as f64 {
            interpolate(&input, 1, position, &mut frame);
            out.push(f64::from(frame[0]));
            position += step;
        }
        out
    }

    /// Frequency from the first to the last rising zero crossing (linearly placed).
    fn frequency(samples: &[f64], rate: f64) -> f64 {
        let crossings: Vec<f64> = samples
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
            .map(|(n, pair)| n as f64 + pair[0] / (pair[0] - pair[1]))
            .collect();
        let periods = (crossings.len() - 1) as f64;
        periods * rate / (crossings[crossings.len() - 1] - crossings[0])
    }

    /// Least-squares fit of `a·sin + b·cos + c` at a known frequency: (level, residual) RMS.
    fn fit(samples: &[f64], frequency: f64, rate: f64) -> (f64, f64) {
        let basis = |n: usize| {
            let phase = TAU * frequency * n as f64 / rate;
            [phase.sin(), phase.cos(), 1.0]
        };
        let mut matrix = [[0.0f64; 3]; 3];
        let mut vector = [0.0f64; 3];
        for (n, &y) in samples.iter().enumerate() {
            let row = basis(n);
            for ((line, total), a) in matrix.iter_mut().zip(&mut vector).zip(row) {
                *total += a * y;
                for (cell, b) in line.iter_mut().zip(row) {
                    *cell += a * b;
                }
            }
        }
        let coefficients = solve(matrix, vector);
        let mut residual = 0.0;
        for (n, &y) in samples.iter().enumerate() {
            let row = basis(n);
            let model: f64 = (0..3).map(|i| coefficients[i] * row[i]).sum();
            residual += (y - model).powi(2);
        }
        let amplitude = coefficients[0].hypot(coefficients[1]);
        let rms = amplitude / 2f64.sqrt();
        (rms, (residual / samples.len() as f64).sqrt())
    }

    /// A 3×3 system by Cramer's rule.
    fn solve(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
        let det = |m: [[f64; 3]; 3]| {
            m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
        };
        let whole = det(m);
        std::array::from_fn(|column| {
            let mut replaced = m;
            for (row, value) in replaced.iter_mut().zip(v) {
                row[column] = value;
            }
            det(replaced) / whole
        })
    }

    fn db(ratio: f64) -> f64 {
        20.0 * ratio.log10()
    }

    #[test]
    fn rate_change_keeps_the_frequency() {
        let out = resampled();
        let measured = frequency(&out, 44_100.0);
        assert!((measured - 1000.0).abs() < 0.01, "{measured} Hz");
    }

    #[test]
    fn rate_change_keeps_the_level_and_adds_little_distortion() {
        let out = resampled();
        let (level, residual) = fit(&out, 1000.0, 44_100.0);
        let expected = AMPLITUDE / 2f64.sqrt();
        let change = db(level / expected);
        let distortion = db(residual / level);
        assert!(change.abs() < 0.1, "level {change} dB");
        assert!(distortion < -60.0, "distortion {distortion} dB");
    }

    #[test]
    fn whole_positions_return_the_samples() {
        let samples = [0.0, 10.0, 1.0, 11.0, 2.0, 12.0, 3.0, 13.0];
        let mut frame = [0.0f32; 2];
        interpolate(&samples, 2, 1.0, &mut frame);
        assert_eq!(frame, [1.0, 11.0]);
        interpolate(&samples, 2, 1.5, &mut frame);
        assert_eq!(frame, [1.5, 11.5]);
    }

    #[test]
    fn mono_goes_to_every_output_channel() {
        let mut out = [9.0f32; 2];
        ChannelMap::new(1, 2).apply(&[0.25], &mut out);
        assert_eq!(out, [0.25, 0.25]);
        let mut out = [9.0f32; 6];
        ChannelMap::new(1, 6).apply(&[-0.5], &mut out);
        assert_eq!(out, [-0.5; 6]);
    }

    #[test]
    fn stereo_goes_to_the_front_pair() {
        let mut out = [9.0f32; 2];
        ChannelMap::new(2, 2).apply(&[0.1, 0.2], &mut out);
        assert_eq!(out, [0.1, 0.2]);
        let mut out = [9.0f32; 6];
        ChannelMap::new(2, 6).apply(&[0.1, 0.2], &mut out);
        assert_eq!(out, [0.1, 0.2, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn surround_center_reaches_both_sides_at_minus_3_db() {
        let mut out = [9.0f32; 2];
        ChannelMap::new(6, 2).apply(&[0.0, 0.0, 0.5, 0.0, 0.0, 0.0], &mut out);
        assert_eq!(out[0], out[1]);
        let change = db(f64::from(out[0]) / 0.5);
        assert!((change + 3.0).abs() < 0.02, "{change} dB");
    }

    #[test]
    fn surround_rears_go_to_their_side() {
        let mut out = [9.0f32; 2];
        ChannelMap::new(6, 2).apply(&[0.0, 0.0, 0.0, 0.0, 0.5, 0.0], &mut out);
        assert!(out[0] > 0.35 && out[1] == 0.0);
        ChannelMap::new(6, 2).apply(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.5], &mut out);
        assert!(out[0] == 0.0 && out[1] > 0.35);
    }

    #[test]
    fn mix_down_is_clamped() {
        let mut out = [9.0f32; 2];
        ChannelMap::new(6, 2).apply(&[1.0, -1.0, 0.0, 0.0, 1.0, -1.0], &mut out);
        assert_eq!(out, [1.0, -1.0]);
    }

    #[test]
    fn surround_to_mono_averages_the_sides() {
        let mut out = [9.0f32; 1];
        ChannelMap::new(6, 1).apply(&[0.4, 0.2, 0.0, 0.0, 0.0, 0.0], &mut out);
        assert!((out[0] - 0.3).abs() < 1e-6);
    }
}
