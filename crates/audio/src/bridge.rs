//! The bridge between one input stream and one output stream (research R4, contracts/audio.md
//! "Bridge"), ported from the spike's WASAPI follower: the input thread pushes its packets into a
//! ring, the output thread pulls what its device asks for, read at a rate slightly off nominal so
//! the audio waiting in the app stays a 2 ms margin above what the output takes.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::resample::{ChannelMap, Format, interpolate};
use crate::volume::Ramp;

/// Spare audio held above what the output takes.
const MARGIN_MS: f64 = 2.0;
/// The read rate is set this often, in output time.
const CONTROL_MS: f64 = 500.0;
/// The rate corrects the margin's error in about this long…
const CORRECTION_SECONDS: f64 = 2.0;
/// …never beyond ±1000 ppm.
const MAX_CORRECTION: f64 = 0.001;
/// Above this much spare audio (a stall, not drift), the oldest is dropped.
const CUT_ABOVE_MS: f64 = 40.0;
/// The most input audio the ring keeps while the output does not take it (not open yet).
const RING_MS: f64 = 200.0;
/// Waiting levels kept for the median between two `take_stats` (one per output event; at
/// 2.67 ms per event, a second is 375).
const WAITS: usize = 4096;
/// Frames the interpolation reads before the position and after it.
const BEFORE: usize = 1;
const AFTER: usize = 2;

fn frames(format: Format, ms: f64) -> usize {
    (f64::from(format.rate) * ms / 1000.0).round() as usize
}

fn ms(format: Format, frames: f64) -> f64 {
    frames * 1000.0 / f64::from(format.rate)
}

/// Locks without giving up on a poisoned lock: the data stays usable audio.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the bridge did since the last `take_stats`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BridgeStats {
    /// Median audio waiting in the app at an output event, in ms.
    pub wait_ms_median: f64,
    /// Most audio waiting in the app at an output event, in ms.
    pub wait_ms_max: f64,
    /// Mean read rate correction, in ppm (positive reads faster).
    pub rate_ppm: f64,
    /// Silence played for lack of audio (before priming or after an empty ring), in ms.
    pub gap_ms: f64,
    /// Input audio thrown away (priming, a stall, or the ring full), in ms.
    pub drop_ms: f64,
}

/// The waiting input frames, interleaved.
struct Ring {
    samples: Vec<f32>,
    /// Most samples kept.
    capacity: usize,
    /// Input frames thrown away since the last stats.
    dropped: u64,
}

impl Ring {
    fn drop_front(&mut self, samples: usize, channels: usize) {
        self.samples.drain(..samples);
        self.dropped += (samples / channels) as u64;
    }
}

/// The output side's state, touched only by `pull` (and `take_stats`).
struct Reader {
    /// Read rate correction around 1.
    ratio: f64,
    /// Fractional input frame of the next output frame, from the ring's start.
    position: f64,
    primed: bool,
    /// Lowest spare input frames seen at an output event since the last control.
    lowest: f64,
    /// Output frames since the last control.
    since: usize,
    ramp: Ramp,
    /// One interpolated input frame.
    frame: Vec<f32>,
    // Stats since the last `take_stats`.
    waits: Vec<f64>,
    wait_max: f64,
    ratio_sum: f64,
    pulls: u64,
    gap: u64,
}

impl Reader {
    fn prime(&mut self) {
        self.primed = true;
        self.position = BEFORE as f64;
        self.lowest = f64::MAX;
        self.since = 0;
    }
}

/// Carries interleaved `f32` samples from an input stream to an output stream of any rate and
/// channel count. Shared by the two device threads (`Arc`); `push` and `pull` hold the ring's lock
/// only for the copy and never allocate once the bridge exists.
pub struct Bridge {
    input: Format,
    output: Format,
    map: ChannelMap,
    /// Input frames per output frame at the nominal rates.
    step: f64,
    /// In input frames.
    margin: usize,
    cut_above: usize,
    /// In output frames.
    control: usize,
    ring: Mutex<Ring>,
    reader: Mutex<Reader>,
    /// Target gain (`f32` bits).
    gain: AtomicU32,
    /// The input's packet size, in frames.
    input_period: AtomicU32,
}

impl Bridge {
    /// An empty, unprimed bridge; its gain starts at 0 and ramps to the first `set_gain`.
    pub fn new(input: Format, output: Format) -> Self {
        let input = Format {
            rate: input.rate.max(1),
            channels: input.channels.max(1),
        };
        let output = Format {
            rate: output.rate.max(1),
            channels: output.channels.max(1),
        };
        let channels = usize::from(input.channels);
        let capacity = frames(input, RING_MS) * channels;
        Self {
            input,
            output,
            map: ChannelMap::new(input.channels, output.channels),
            step: f64::from(input.rate) / f64::from(output.rate),
            margin: frames(input, MARGIN_MS),
            cut_above: frames(input, CUT_ABOVE_MS),
            control: frames(output, CONTROL_MS),
            ring: Mutex::new(Ring {
                samples: Vec::with_capacity(capacity),
                capacity,
                dropped: 0,
            }),
            reader: Mutex::new(Reader {
                ratio: 1.0,
                position: BEFORE as f64,
                primed: false,
                lowest: f64::MAX,
                since: 0,
                ramp: Ramp::new(0.0, output.rate),
                frame: vec![0.0; channels],
                waits: Vec::with_capacity(WAITS),
                wait_max: 0.0,
                ratio_sum: 0.0,
                pulls: 0,
                gap: 0,
            }),
            gain: AtomicU32::new(0.0f32.to_bits()),
            input_period: AtomicU32::new(0),
        }
    }

    /// The input's format.
    pub fn input(&self) -> Format {
        self.input
    }

    /// The output's format.
    pub fn output(&self) -> Format {
        self.output
    }

    /// The input's packet size in frames, which priming keeps waiting on top of the margin.
    pub fn set_input_period(&self, frames: u32) {
        self.input_period.store(frames, Ordering::Relaxed);
    }

    /// The gain the output moves to over 10 ms, from any thread.
    pub fn set_gain(&self, gain: f32) {
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// Input thread: appends interleaved input frames (a partial frame at the end is ignored).
    /// When the output does not take them, the oldest past 200 ms are dropped.
    pub fn push(&self, samples: &[f32]) {
        let channels = usize::from(self.input.channels);
        let mut samples = &samples[..samples.len() / channels * channels];
        let mut ring = lock(&self.ring);
        if samples.len() > ring.capacity {
            let extra = samples.len() - ring.capacity;
            ring.dropped += (extra / channels) as u64;
            samples = &samples[extra..];
        }
        let over = (ring.samples.len() + samples.len()).saturating_sub(ring.capacity);
        if over > 0 {
            ring.drop_front(over, channels);
        }
        ring.samples.extend_from_slice(samples);
    }

    /// Output thread: fills `out` with interleaved output frames. Silence (counted as a gap)
    /// until enough audio waits, after which it plays with the follower's rate, the
    /// interpolation, the channel map and the gain ramp.
    pub fn pull(&self, out: &mut [f32]) {
        let in_channels = usize::from(self.input.channels);
        let out_channels = usize::from(self.output.channels);
        let count = out.len() / out_channels;
        let out = &mut out[..count * out_channels];
        if count == 0 {
            return;
        }
        let period = self.input_period.load(Ordering::Relaxed) as usize;
        let mut reader = lock(&self.reader);
        let reader = &mut *reader;
        reader
            .ramp
            .set_target(f32::from_bits(self.gain.load(Ordering::Relaxed)));

        let step = reader.ratio * self.step;
        let mut ring = lock(&self.ring);
        let mut level = ring.samples.len() / in_channels;
        // Input frames the `count` output frames read, with the interpolation's neighbours.
        let needed = (reader.position + count as f64 * step).floor() as usize + AFTER + 1;
        // What waits on top of the margin when a packet has just arrived.
        let start = needed + period + self.margin;
        if reader.primed && level > needed + self.cut_above {
            // A stall left far more than the follower handles: drop the oldest instead of
            // playing it late.
            ring.drop_front((level - start) * in_channels, in_channels);
            level = start;
        }
        if !reader.primed && level >= start {
            // What piled up while the output opened would only be played late.
            ring.drop_front((level - start) * in_channels, in_channels);
            level = start;
            reader.prime();
        }
        let played = reader.primed && level >= needed;
        if played {
            for frame in out.chunks_exact_mut(out_channels) {
                interpolate(&ring.samples, in_channels, reader.position, &mut reader.frame);
                self.map.apply(&reader.frame, frame);
                reader.position += step;
            }
            let used = reader.position.floor() as usize - BEFORE;
            ring.samples.drain(..used * in_channels);
            reader.position -= used as f64;
        } else {
            reader.primed = false;
            reader.position = BEFORE as f64;
        }
        drop(ring);

        if played {
            self.observe(reader, (level - needed) as f64, count);
            for frame in out.chunks_exact_mut(out_channels) {
                let gain = reader.ramp.advance();
                frame.iter_mut().for_each(|sample| *sample *= gain);
            }
        } else {
            out.fill(0.0);
            reader.gap += count as u64;
            // The sound comes back from silence with the ramp too, so it does not click.
            reader.ramp = Ramp::new(0.0, self.output.rate);
        }
        let wait = ms(self.input, level as f64);
        if reader.waits.len() < reader.waits.capacity() {
            reader.waits.push(wait);
        }
        reader.wait_max = reader.wait_max.max(wait);
        reader.ratio_sum += reader.ratio;
        reader.pulls += 1;
    }

    /// Records the spare input frames of one output event; every 500 ms of output, sets the read
    /// rate from the lowest of them so the error to the margin is corrected in about 2 s.
    fn observe(&self, reader: &mut Reader, spare: f64, count: usize) {
        reader.lowest = reader.lowest.min(spare);
        reader.since += count;
        if reader.since >= self.control {
            let error = reader.lowest - self.margin as f64;
            let correction = error / (CORRECTION_SECONDS * f64::from(self.input.rate));
            reader.ratio = 1.0 + correction.clamp(-MAX_CORRECTION, MAX_CORRECTION);
            reader.lowest = f64::MAX;
            reader.since = 0;
        }
    }

    /// What happened since the last call, which starts a new count (once per second).
    pub fn take_stats(&self) -> BridgeStats {
        let mut reader = lock(&self.reader);
        let mut waits = reader.waits.clone();
        let stats = BridgeStats {
            wait_ms_median: 0.0,
            wait_ms_max: reader.wait_max,
            rate_ppm: if reader.pulls == 0 {
                0.0
            } else {
                (reader.ratio_sum / reader.pulls as f64 - 1.0) * 1e6
            },
            gap_ms: ms(self.output, reader.gap as f64),
            drop_ms: ms(self.input, std::mem::take(&mut lock(&self.ring).dropped) as f64),
        };
        reader.waits.clear();
        reader.wait_max = 0.0;
        reader.ratio_sum = 0.0;
        reader.pulls = 0;
        reader.gap = 0;
        drop(reader);
        waits.sort_by(f64::total_cmp);
        let middle = waits.len() / 2;
        let median = match waits.len() {
            0 => 0.0,
            n if n % 2 == 0 => (waits[middle - 1] + waits[middle]) / 2.0,
            _ => waits[middle],
        };
        BridgeStats {
            wait_ms_median: median,
            ..stats
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEREO_48K: Format = Format {
        rate: 48_000,
        channels: 2,
    };
    /// 3 ms input packets and 2.67 ms output packets at 48 kHz.
    const INPUT_PERIOD: usize = 144;
    const OUTPUT_PERIOD: usize = 128;

    fn bridge(value: f32) -> (Bridge, Vec<f32>) {
        let bridge = Bridge::new(STEREO_48K, STEREO_48K);
        bridge.set_input_period(INPUT_PERIOD as u32);
        bridge.set_gain(1.0);
        (bridge, vec![value; INPUT_PERIOD * 2])
    }

    /// Runs both clocks for `frames` frames at 48 kHz, pushing `packet` every 3 ms and pulling
    /// every 2.67 ms; returns the last output packet.
    fn run(bridge: &Bridge, packet: &[f32], frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        for t in 0..frames {
            if t % INPUT_PERIOD == 0 {
                bridge.push(packet);
            }
            if t % OUTPUT_PERIOD == 0 {
                bridge.pull(&mut out);
            }
        }
        out
    }

    fn close(a: f64, b: f64, within: f64) -> bool {
        (a - b).abs() <= within
    }

    #[test]
    fn an_empty_bridge_plays_silence_as_a_gap() {
        let (bridge, _) = bridge(0.5);
        let mut out = vec![9.0; OUTPUT_PERIOD * 2];
        bridge.pull(&mut out);
        assert!(out.iter().all(|&sample| sample == 0.0));
        let stats = bridge.take_stats();
        assert!(close(stats.gap_ms, 128.0 / 48.0, 1e-9), "{stats:?}");
        assert_eq!(stats.drop_ms, 0.0);
    }

    #[test]
    fn priming_drops_down_to_one_input_period_and_the_margin() {
        let (bridge, packet) = bridge(0.5);
        for _ in 0..16 {
            bridge.push(&packet); // 48 ms
        }
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        bridge.pull(&mut out);
        // needed = 1 + 128 + 2 + 1 = 132; kept = 132 + 144 + 96 = 372 of 2304 frames.
        let stats = bridge.take_stats();
        assert!(close(stats.drop_ms, (2304.0 - 372.0) / 48.0, 1e-9), "{stats:?}");
        assert!(close(stats.wait_ms_max, 372.0 / 48.0, 1e-9), "{stats:?}");
        assert_eq!(stats.gap_ms, 0.0);
        assert!(out.iter().any(|&sample| sample > 0.0));
    }

    #[test]
    fn an_empty_ring_gaps_and_primes_again() {
        let (bridge, packet) = bridge(0.5);
        run(&bridge, &packet, 48_000);
        bridge.take_stats();
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        for _ in 0..10 {
            bridge.pull(&mut out); // 26.7 ms with no input
        }
        assert!(out.iter().all(|&sample| sample == 0.0));
        let stats = bridge.take_stats();
        assert!(stats.gap_ms > 15.0, "{stats:?}");
        let out = run(&bridge, &packet, 4800);
        assert!(out.iter().all(|&sample| close(f64::from(sample), 0.5, 1e-6)), "{out:?}");
        let stats = bridge.take_stats();
        assert!(stats.gap_ms < 10.0, "{stats:?}");
    }

    #[test]
    fn steady_clocks_play_without_gaps_or_drops() {
        let (bridge, packet) = bridge(0.5);
        run(&bridge, &packet, 48_000);
        bridge.take_stats();
        let out = run(&bridge, &packet, 5 * 48_000);
        let stats = bridge.take_stats();
        assert_eq!(stats.gap_ms, 0.0, "{stats:?}");
        assert_eq!(stats.drop_ms, 0.0, "{stats:?}");
        assert!(stats.rate_ppm.abs() <= 1000.0, "{stats:?}");
        assert!(out.iter().all(|&sample| close(f64::from(sample), 0.5, 1e-6)));
    }

    #[test]
    fn a_burst_is_dropped_once() {
        let (bridge, packet) = bridge(0.5);
        run(&bridge, &packet, 5 * 48_000);
        bridge.take_stats();
        for _ in 0..17 {
            bridge.push(&packet); // 51 ms at once: a stall's backlog
        }
        run(&bridge, &packet, 48_000);
        let stats = bridge.take_stats();
        assert!(stats.drop_ms > 40.0 && stats.drop_ms < 52.0, "{stats:?}");
        assert_eq!(stats.gap_ms, 0.0, "{stats:?}");
        run(&bridge, &packet, 2 * 48_000);
        let stats = bridge.take_stats();
        assert_eq!(stats.drop_ms, 0.0, "{stats:?}");
        assert_eq!(stats.gap_ms, 0.0, "{stats:?}");
    }

    #[test]
    fn the_gain_ramps_to_each_new_value() {
        let (bridge, packet) = bridge(1.0);
        run(&bridge, &packet, 48_000);
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        bridge.set_gain(0.0);
        let mut left = Vec::new();
        for _ in 0..5 {
            bridge.push(&packet);
            bridge.pull(&mut out);
            left.extend(out.iter().step_by(2).copied());
        }
        assert!(left[0] < 1.0 && left[0] > 0.99, "{}", left[0]);
        for pair in left.windows(2) {
            assert!(pair[1] <= pair[0]);
            assert!(pair[0] - pair[1] <= 1.0 / 480.0 + 1e-5);
        }
        assert_eq!(left[479], 0.0);
        assert!(left[478] > 0.0);
        assert!(left[480..].iter().all(|&sample| sample == 0.0));
    }

    /// The first output frames after silence, left channel, once the input is back.
    fn first_sound(bridge: &Bridge, packet: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        loop {
            bridge.push(packet);
            bridge.pull(&mut out);
            if out[0] != 0.0 {
                return out.iter().step_by(2).copied().collect();
            }
        }
    }

    #[test]
    fn the_sound_fades_in_at_the_start_and_after_a_gap() {
        let (bridge, packet) = bridge(1.0);
        let start = first_sound(&bridge, &packet);
        assert!(close(f64::from(start[0]), 1.0 / 480.0, 1e-6), "{}", start[0]);
        run(&bridge, &packet, 48_000);
        let mut out = vec![0.0; OUTPUT_PERIOD * 2];
        for _ in 0..10 {
            bridge.pull(&mut out); // the input stalls: silence
        }
        let back = first_sound(&bridge, &packet);
        assert!(close(f64::from(back[0]), 1.0 / 480.0, 1e-6), "{}", back[0]);
    }

    #[test]
    fn rate_and_channels_are_converted() {
        let input = Format {
            rate: 48_000,
            channels: 1,
        };
        let output = Format {
            rate: 44_100,
            channels: 2,
        };
        let bridge = Bridge::new(input, output);
        bridge.set_input_period(144);
        bridge.set_gain(1.0);
        let packet = vec![0.25f32; 144];
        let mut out = vec![0.0f32; 2 * 128];
        // 144 frames every 3 ms in, 128 frames every 2.9025 ms out: simulate in µs.
        let (mut next_in, mut next_out) = (0.0f64, 0.0f64);
        let step_out = 128.0 / 44_100.0 * 1e6;
        let mut gaps_after_start = 0.0;
        for second in 0..5 {
            while next_in < f64::from(second + 1) * 1e6 || next_out < f64::from(second + 1) * 1e6 {
                if next_in <= next_out {
                    bridge.push(&packet);
                    next_in += 3000.0;
                } else {
                    bridge.pull(&mut out);
                    next_out += step_out;
                }
            }
            let stats = bridge.take_stats();
            if second > 0 {
                gaps_after_start += stats.gap_ms + stats.drop_ms;
            }
        }
        assert_eq!(gaps_after_start, 0.0);
        assert!(
            out.iter().all(|&sample| close(f64::from(sample), 0.25, 1e-6)),
            "{out:?}"
        );
    }

    #[test]
    fn the_ring_keeps_at_most_200_ms_unread() {
        let (bridge, packet) = bridge(0.5);
        for _ in 0..100 {
            bridge.push(&packet); // 300 ms
        }
        let stats = bridge.take_stats();
        assert!(close(stats.drop_ms, 100.0, 1e-9), "{stats:?}");
        assert!(bridge.ring.lock().unwrap().samples.capacity() == 9600 * 2);
    }
}
