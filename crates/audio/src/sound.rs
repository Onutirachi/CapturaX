//! The app's event sounds (screenshot spec, research R10, data-model.md "Sound"): a small WAV
//! reader, the conversion of a sound to an output's format, and the one voice that plays it into
//! an output's samples. Pure code; the app embeds the files and routes the voice to an output.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::resample::{ChannelMap, Format, interpolate};

/// The fixed level of every sound, about -6 dB; not the preview's volume nor its mute (FR-010).
pub const GAIN: f32 = 0.5;

/// A sound the app plays. Its WAV is the app's (embedded there); a later sound is one more value
/// and one more file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sound {
    Screenshot,
}

/// A WAV file's samples as `f32` interleaved in its own format.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub format: Format,
    pub samples: Vec<f32>,
}

/// Why a WAV file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WavError {
    /// Not a RIFF/WAVE file, or cut short.
    NotWave,
    /// No `fmt ` chunk before the `data` chunk, or no `data` chunk.
    Missing(&'static str),
    /// A sample format the reader does not take (with its description).
    Unsupported(String),
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotWave => f.write_str("not a WAV file"),
            Self::Missing(chunk) => write!(f, "no {chunk} chunk"),
            Self::Unsupported(what) => write!(f, "unsupported format: {what}"),
        }
    }
}

impl std::error::Error for WavError {}

/// How the file stores a sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Int(u16),
    Float32,
}

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// The part of `KSDATAFORMAT_SUBTYPE_PCM`/`_IEEE_FLOAT` after its first two bytes (the format
/// tag): `-0000-0010-8000-00AA00389B71`.
const SUBFORMAT_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Reads a RIFF/WAVE file: PCM 16-, 24- and 32-bit integer and 32-bit float, plain or
/// `WAVE_FORMAT_EXTENSIBLE`, any rate and channel count. Other chunks are skipped; a `data`
/// chunk longer than the file (a writer that never set it) is read to the file's end, in whole
/// frames.
pub fn decode(bytes: &[u8]) -> Result<Decoded, WavError> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(WavError::NotWave);
    }
    let mut at = 12;
    let mut format: Option<(Format, Encoding)> = None;
    while let (Some(id), Some(size)) = (bytes.get(at..at + 4), u32_at(bytes, at + 4)) {
        let body = at + 8;
        let end = body.saturating_add(size as usize);
        match id {
            b"fmt " => format = Some(read_format(bytes.get(body..end).ok_or(WavError::NotWave)?)?),
            b"data" => {
                let (format, encoding) = format.ok_or(WavError::Missing("fmt "))?;
                let data = &bytes[body..end.min(bytes.len())];
                return Ok(Decoded {
                    format,
                    samples: read_samples(data, format.channels, encoding),
                });
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        at = end.saturating_add(size as usize & 1);
    }
    Err(WavError::Missing("data"))
}

fn read_format(chunk: &[u8]) -> Result<(Format, Encoding), WavError> {
    let field = |at| u16_at(chunk, at).ok_or(WavError::NotWave);
    let mut tag = field(0)?;
    let channels = field(2)?;
    let rate = u32_at(chunk, 4).ok_or(WavError::NotWave)?;
    let block_align = field(12)?;
    let bits = field(14)?;
    if tag == WAVE_FORMAT_EXTENSIBLE {
        let sub = chunk.get(24..40).ok_or(WavError::NotWave)?;
        if sub[2..] != SUBFORMAT_TAIL {
            return Err(WavError::Unsupported("extensible with a non-PCM subformat".into()));
        }
        tag = u16::from_le_bytes([sub[0], sub[1]]);
    }
    let encoding = match (tag, bits) {
        (WAVE_FORMAT_PCM, 16 | 24 | 32) => Encoding::Int(bits),
        (WAVE_FORMAT_IEEE_FLOAT, 32) => Encoding::Float32,
        _ => return Err(WavError::Unsupported(format!("tag {tag:#x}, {bits} bits"))),
    };
    if channels == 0 || rate == 0 || block_align != channels * (bits / 8) {
        return Err(WavError::Unsupported(format!(
            "{channels} channels, {rate} Hz, block {block_align}"
        )));
    }
    Ok((Format { rate, channels }, encoding))
}

fn read_samples(data: &[u8], channels: u16, encoding: Encoding) -> Vec<f32> {
    let width = match encoding {
        Encoding::Int(bits) => usize::from(bits / 8),
        Encoding::Float32 => 4,
    };
    let frame = width * usize::from(channels);
    let whole = data.len() / frame * frame;
    data[..whole]
        .chunks_exact(width)
        .map(|s| match encoding {
            Encoding::Int(16) => f32::from(i16::from_le_bytes([s[0], s[1]])) / 32_768.0,
            // Into the high bytes of an i32, so the sign comes along.
            Encoding::Int(24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
            Encoding::Int(_) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
            Encoding::Float32 => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
        })
        .collect()
}

/// A sound converted to one output's format; cheap to clone.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub format: Format,
    pub samples: Arc<[f32]>,
}

/// Converts a sound to the output's rate (4-point cubic, as the bridge) and channels (the
/// bridge's channel map). Made off the audio thread, when the output's format becomes known.
pub fn prepare(decoded: &Decoded, output: Format) -> Prepared {
    let input = decoded.format;
    let (in_channels, out_channels) = (usize::from(input.channels), usize::from(output.channels));
    let frames = decoded.samples.len() / in_channels;
    let map = ChannelMap::new(input.channels, output.channels);
    let mut samples = Vec::new();
    if input.rate == output.rate {
        samples.resize(frames * out_channels, 0.0);
        for (from, to) in decoded
            .samples
            .chunks_exact(in_channels)
            .zip(samples.chunks_exact_mut(out_channels))
        {
            map.apply(from, to);
        }
    } else {
        // One silent frame before and two after, so every position has its four neighbours.
        let mut padded = vec![0.0; in_channels];
        padded.extend_from_slice(&decoded.samples[..frames * in_channels]);
        padded.extend(std::iter::repeat_n(0.0, 2 * in_channels));
        let count = (frames as u64 * u64::from(output.rate)).div_ceil(u64::from(input.rate)) as usize;
        let step = f64::from(input.rate) / f64::from(output.rate);
        samples.resize(count * out_channels, 0.0);
        let mut frame = vec![0.0; in_channels];
        for (n, to) in samples.chunks_exact_mut(out_channels).enumerate() {
            interpolate(&padded, in_channels, 1.0 + n as f64 * step, &mut frame);
            map.apply(&frame, to);
        }
    }
    Prepared {
        format: output,
        samples: samples.into(),
    }
}

/// The one voice of a sound on one output: `play` from any thread, `mix` from the audio thread.
/// A new `play` restarts the sound, so a burst never layers into a longer one. `mix` takes no
/// lock and allocates nothing.
#[derive(Debug)]
pub struct Voice {
    sound: Prepared,
    /// The next sample to mix; the sound's length when silent.
    position: AtomicUsize,
    restart: AtomicBool,
}

impl Voice {
    /// A silent voice for the prepared sound.
    pub fn new(sound: Prepared) -> Self {
        let end = sound.samples.len();
        Self {
            sound,
            position: AtomicUsize::new(end),
            restart: AtomicBool::new(false),
        }
    }

    /// The output format the voice was prepared for.
    pub fn format(&self) -> Format {
        self.sound.format
    }

    /// Plays the sound from its start at the next `mix`.
    pub fn play(&self) {
        self.restart.store(true, Ordering::Release);
    }

    /// Whether a sound is waiting to start or still playing.
    pub fn playing(&self) -> bool {
        self.restart.load(Ordering::Acquire) || self.position.load(Ordering::Relaxed) < self.sound.samples.len()
    }

    /// Adds the sound at [`GAIN`] from where it is into `out` (interleaved samples in `format`)
    /// and clamps what it touched to ±1. Nothing is added when `format` is not the prepared one.
    pub fn mix(&self, out: &mut [f32], format: Format) {
        if self.restart.swap(false, Ordering::Acquire) {
            self.position.store(0, Ordering::Relaxed);
        }
        let position = self.position.load(Ordering::Relaxed);
        let samples = &self.sound.samples;
        if position >= samples.len() || format != self.sound.format {
            return;
        }
        let count = out.len().min(samples.len() - position);
        for (o, s) in out[..count].iter_mut().zip(&samples[position..position + count]) {
            *o = (*o + s * GAIN).clamp(-1.0, 1.0);
        }
        self.position.store(position + count, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONO_48K: Format = Format {
        rate: 48_000,
        channels: 1,
    };
    const STEREO_48K: Format = Format {
        rate: 48_000,
        channels: 2,
    };

    /// A WAV file: `fmt ` (plain, or extensible when `extensible`), a `LIST` chunk of odd length
    /// to skip, then `data`.
    fn wav(tag: u16, channels: u16, rate: u32, bits: u16, data: &[u8], extensible: bool) -> Vec<u8> {
        let block = channels * bits / 8;
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&(if extensible { WAVE_FORMAT_EXTENSIBLE } else { tag }).to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
        fmt.extend_from_slice(&block.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if extensible {
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&bits.to_le_bytes());
            fmt.extend_from_slice(&0u32.to_le_bytes());
            fmt.extend_from_slice(&tag.to_le_bytes());
            fmt.extend_from_slice(&SUBFORMAT_TAIL);
        }
        let mut body = b"WAVE".to_vec();
        for (id, chunk) in [(b"fmt ", &fmt[..]), (b"LIST", b"abc"), (b"data", data)] {
            body.extend_from_slice(id);
            body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            body.extend_from_slice(chunk);
            if chunk.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut file = b"RIFF".to_vec();
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend(body);
        file
    }

    fn assert_close(got: &[f32], want: &[f32]) {
        assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{got:?} vs {want:?}");
        }
    }

    #[test]
    fn reads_each_sample_format() {
        let want = [0.5, -0.5, -1.0, 0.25];
        let i16s: Vec<u8> = [16_384i16, -16_384, -32_768, 8_192]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let i24s: Vec<u8> = [0x40_0000i32, -0x40_0000, -0x80_0000, 0x20_0000]
            .iter()
            .flat_map(|s| s.to_le_bytes()[..3].to_vec())
            .collect();
        let i32s: Vec<u8> = [0x4000_0000i32, -0x4000_0000, i32::MIN, 0x2000_0000]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let f32s: Vec<u8> = want.iter().flat_map(|s: &f32| s.to_le_bytes()).collect();
        for extensible in [false, true] {
            for (tag, bits, data) in [
                (WAVE_FORMAT_PCM, 16, &i16s),
                (WAVE_FORMAT_PCM, 24, &i24s),
                (WAVE_FORMAT_PCM, 32, &i32s),
                (WAVE_FORMAT_IEEE_FLOAT, 32, &f32s),
            ] {
                let decoded = decode(&wav(tag, 2, 44_100, bits, data, extensible)).unwrap();
                assert_eq!(
                    decoded.format,
                    Format {
                        rate: 44_100,
                        channels: 2
                    }
                );
                assert_close(&decoded.samples, &want);
            }
        }
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        assert_eq!(decode(b"RIFF\0\0\0\0AVI "), Err(WavError::NotWave));
        assert_eq!(decode(b""), Err(WavError::NotWave));
        assert!(matches!(
            decode(&wav(WAVE_FORMAT_PCM, 1, 48_000, 8, &[0, 0], false)),
            Err(WavError::Unsupported(_))
        ));
        assert!(matches!(
            decode(&wav(2 /* ADPCM */, 1, 48_000, 16, &[0, 0], false)),
            Err(WavError::Unsupported(_))
        ));
        let mut no_data = wav(WAVE_FORMAT_PCM, 1, 48_000, 16, &[], false);
        no_data.truncate(no_data.len() - 8);
        assert_eq!(decode(&no_data), Err(WavError::Missing("data")));
    }

    #[test]
    fn a_data_chunk_longer_than_the_file_is_read_in_whole_frames() {
        let mut file = wav(WAVE_FORMAT_PCM, 2, 48_000, 16, &[0, 64, 0, 192, 0, 32], false);
        // The data size field is the file's last 4 + 6 bytes back: say "huge".
        let at = file.len() - 6 - 4;
        file[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let decoded = decode(&file).unwrap();
        assert_close(&decoded.samples, &[0.5, -0.5]);
    }

    #[test]
    fn the_placeholder_shape_reads() {
        // 48 kHz mono 16-bit, as tools/make_screenshot_sound.py writes it.
        let data: Vec<u8> = (0..5760i16).flat_map(|n| (n % 100).to_le_bytes()).collect();
        let decoded = decode(&wav(WAVE_FORMAT_PCM, 1, 48_000, 16, &data, false)).unwrap();
        assert_eq!(decoded.format, MONO_48K);
        assert_eq!(decoded.samples.len(), 5760);
    }

    #[test]
    fn resampling_keeps_the_length_in_time() {
        let decoded = Decoded {
            format: Format {
                rate: 44_100,
                channels: 1,
            },
            samples: (0..4410).map(|n| (n as f32 * 0.01).sin() * 0.5).collect(),
        };
        let prepared = prepare(&decoded, MONO_48K);
        assert_eq!(prepared.samples.len(), 4800);
        // Same signal at the same time: frame 480 at 48 kHz is frame 441 at 44.1 kHz.
        assert!((prepared.samples[480] - decoded.samples[441]).abs() < 1e-3);
        let back = prepare(&prepared_as_decoded(&prepared), decoded.format);
        assert_eq!(back.samples.len(), 4410);
    }

    fn prepared_as_decoded(prepared: &Prepared) -> Decoded {
        Decoded {
            format: prepared.format,
            samples: prepared.samples.to_vec(),
        }
    }

    #[test]
    fn mono_goes_to_both_sides() {
        let decoded = Decoded {
            format: MONO_48K,
            samples: vec![0.1, 0.2, 0.3],
        };
        let prepared = prepare(&decoded, STEREO_48K);
        assert_close(&prepared.samples, &[0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
        assert_eq!(prepared.format, STEREO_48K);
    }

    fn voice(samples: &[f32]) -> Voice {
        Voice::new(prepare(
            &Decoded {
                format: MONO_48K,
                samples: samples.to_vec(),
            },
            MONO_48K,
        ))
    }

    #[test]
    fn a_voice_is_silent_until_played_and_after_its_end() {
        let voice = voice(&[1.0, 0.5, -0.5]);
        let mut out = [0.0; 4];
        voice.mix(&mut out, MONO_48K);
        assert_eq!(out, [0.0; 4]);
        assert!(!voice.playing());
        voice.play();
        assert!(voice.playing());
        let mut out = [0.1; 2];
        voice.mix(&mut out, MONO_48K);
        assert_close(&out, &[0.6, 0.35]);
        let mut out = [0.0; 3];
        voice.mix(&mut out, MONO_48K);
        assert_close(&out, &[-0.25, 0.0, 0.0]);
        assert!(!voice.playing());
        let mut out = [0.0; 3];
        voice.mix(&mut out, MONO_48K);
        assert_eq!(out, [0.0; 3]);
    }

    #[test]
    fn play_restarts_and_mix_clamps() {
        let voice = voice(&[1.0, 1.0, 1.0, 1.0]);
        voice.play();
        let mut out = [0.0; 2];
        voice.mix(&mut out, MONO_48K);
        voice.play();
        let mut out = [0.8, -0.2, 0.0, 0.0, 0.0];
        voice.mix(&mut out, MONO_48K);
        // Restarted: four samples again, the first clamped.
        assert_close(&out, &[1.0, 0.3, 0.5, 0.5, 0.0]);
    }

    #[test]
    fn a_voice_adds_nothing_to_another_format() {
        let voice = voice(&[1.0]);
        voice.play();
        let mut out = [0.0; 2];
        voice.mix(&mut out, STEREO_48K);
        assert_eq!(out, [0.0; 2]);
    }
}
