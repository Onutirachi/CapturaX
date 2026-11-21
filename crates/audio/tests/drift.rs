//! The bridge holding its level for 30 minutes of input and output clocks ±300 ppm apart
//! (research R15): no gap and no drop after the first second; once the follower has settled, the
//! waiting audio steady and the read rate following the offset.

use capturax_audio::bridge::Bridge;
use capturax_audio::resample::Format;

const MINUTES: u32 = 30;
/// 3 ms input packets at 48 kHz.
const INPUT_PERIOD: usize = 144;
/// About 2.67 ms output packets.
const OUTPUT_PERIOD_48K: usize = 128;
const OUTPUT_PERIOD_44K: usize = 118;
/// The follower has settled after this many seconds: priming leaves one input period above the
/// margin, which the follower takes away in about 8 s.
const SETTLED_SECONDS: u32 = 10;

/// Plays `MINUTES` of a stereo input whose clock runs `ppm` fast against the output's, and
/// checks every second's stats.
fn drift(output_rate: u32, output_period: usize, ppm: f64) {
    let input = Format {
        rate: 48_000,
        channels: 2,
    };
    let output = Format {
        rate: output_rate,
        channels: 2,
    };
    let bridge = Bridge::new(input, output);
    bridge.set_input_period(INPUT_PERIOD as u32);
    bridge.set_gain(1.0);
    let packet = vec![0.5f32; INPUT_PERIOD * 2];
    let mut out = vec![0.0f32; output_period * 2];

    // Times in ns of the output's clock.
    let input_step = INPUT_PERIOD as f64 / 48_000.0 * 1e9 / (1.0 + ppm * 1e-6);
    let output_step = output_period as f64 / f64::from(output_rate) * 1e9;
    let (mut next_in, mut next_out) = (0.0f64, 0.0f64);
    let mut settled_wait = None;
    for second in 1..=MINUTES * 60 {
        let end = f64::from(second) * 1e9;
        while next_in < end || next_out < end {
            if next_in <= next_out {
                bridge.push(&packet);
                next_in += input_step;
            } else {
                bridge.pull(&mut out);
                next_out += output_step;
            }
        }
        let stats = bridge.take_stats();
        let case = format!("{output_rate} Hz, {ppm:+} ppm, second {second}: {stats:?}");
        if second == 1 {
            continue;
        }
        assert_eq!(stats.gap_ms, 0.0, "{case}");
        assert_eq!(stats.drop_ms, 0.0, "{case}");
        if second < SETTLED_SECONDS {
            continue;
        }
        let settled = *settled_wait.get_or_insert(stats.wait_ms_median);
        assert!(
            (stats.wait_ms_median - settled).abs() <= 1.0,
            "{case}, at second {SETTLED_SECONDS}: {settled} ms"
        );
        assert!((stats.rate_ppm - ppm).abs() <= 50.0, "{case}");
    }
    assert!(out.iter().all(|&sample| (sample - 0.5).abs() <= 1e-6), "{out:?}");
}

#[test]
fn a_fast_input_at_the_same_rate() {
    drift(48_000, OUTPUT_PERIOD_48K, 300.0);
}

#[test]
fn a_slow_input_at_the_same_rate() {
    drift(48_000, OUTPUT_PERIOD_48K, -300.0);
}

#[test]
fn a_fast_input_resampled_to_44_1_khz() {
    drift(44_100, OUTPUT_PERIOD_44K, 300.0);
}

#[test]
fn a_slow_input_resampled_to_44_1_khz() {
    drift(44_100, OUTPUT_PERIOD_44K, -300.0);
}
