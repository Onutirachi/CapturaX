//! The system's wave formats (`WAVEFORMATEX`, `WAVEFORMATEXTENSIBLE`) in the platform's terms,
//! and the streams' sample conversions (the spike's `Sample`, any rate and channel count).

use windows::Win32::Media::Audio::{WAVE_FORMAT_PCM, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0};
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};

use crate::audio::{Format, SampleFormat};

/// How a format's samples are coded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coding {
    Integer,
    Float,
}

/// The sample format of a coding, its container size and its valid bits; None for anything the
/// app does not read or write.
fn sample_format(coding: Coding, bits: u16, valid_bits: u16) -> Option<SampleFormat> {
    match (coding, bits, valid_bits) {
        (Coding::Float, 32, 32) => Some(SampleFormat::F32),
        (Coding::Integer, 16, 16) => Some(SampleFormat::I16),
        (Coding::Integer, 24, 24) => Some(SampleFormat::I24),
        (Coding::Integer, 32, 24) => Some(SampleFormat::I24In32),
        (Coding::Integer, 32, 32) => Some(SampleFormat::I32),
        _ => None,
    }
}

/// The format a `WAVEFORMATEX` describes; None when its coding is not plain integer or float
/// samples the app handles.
///
/// # Safety
///
/// `wave` points to a valid `WAVEFORMATEX` followed by its `cbSize` extra bytes, as WASAPI hands
/// them out.
pub(super) unsafe fn format_of(wave: *const WAVEFORMATEX) -> Option<Format> {
    // SAFETY: the caller's guarantee; the struct is packed, so it is read unaligned.
    let head = unsafe { wave.read_unaligned() };
    let (tag, bits) = (u32::from(head.wFormatTag), head.wBitsPerSample);
    let (coding, valid_bits) = if tag == WAVE_FORMAT_EXTENSIBLE {
        if usize::from(head.cbSize) < size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>() {
            return None;
        }
        // SAFETY: `cbSize` says the extensible part follows the head.
        let extensible = unsafe { wave.cast::<WAVEFORMATEXTENSIBLE>().read_unaligned() };
        let sub = extensible.SubFormat;
        let coding = if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            Coding::Float
        } else if sub == KSDATAFORMAT_SUBTYPE_PCM {
            Coding::Integer
        } else {
            return None;
        };
        // SAFETY: every member of the union is a u16; PCM and float formats use the valid bits.
        let valid = unsafe { extensible.Samples.wValidBitsPerSample };
        (coding, if valid == 0 { bits } else { valid })
    } else if tag == WAVE_FORMAT_IEEE_FLOAT {
        (Coding::Float, bits)
    } else if tag == WAVE_FORMAT_PCM {
        (Coding::Integer, bits)
    } else {
        return None;
    };
    Some(Format {
        rate: head.nSamplesPerSec,
        channels: head.nChannels,
        sample: sample_format(coding, bits, valid_bits)?,
    })
}

/// The formats exclusive mode tries, in order (research R2).
pub(super) const EXCLUSIVE_SAMPLES: [SampleFormat; 5] = [
    SampleFormat::F32,
    SampleFormat::I32,
    SampleFormat::I24In32,
    SampleFormat::I24,
    SampleFormat::I16,
];

/// Bytes of one sample.
pub(super) fn bytes(sample: SampleFormat) -> usize {
    match sample {
        SampleFormat::F32 | SampleFormat::I32 | SampleFormat::I24In32 => 4,
        SampleFormat::I24 => 3,
        SampleFormat::I16 => 2,
    }
}

/// The usual speaker mask of a channel count (`KSAUDIO_SPEAKER_*`); 0 (no positions) for counts
/// without one.
pub(super) fn standard_mask(channels: u16) -> u32 {
    match channels {
        1 => 0x4,   // front center
        2 => 0x3,   // front left, right
        4 => 0x33,  // quad
        6 => 0x3f,  // 5.1
        8 => 0x63f, // 7.1 surround
        _ => 0,
    }
}

/// The speaker mask of a wave format: an extensible format's own, else the standard one.
///
/// # Safety
///
/// As [`format_of`].
pub(super) unsafe fn mask_of(wave: *const WAVEFORMATEX) -> u32 {
    // SAFETY: the caller's guarantee; read unaligned (packed struct).
    let head = unsafe { wave.read_unaligned() };
    if u32::from(head.wFormatTag) == WAVE_FORMAT_EXTENSIBLE
        && usize::from(head.cbSize) >= size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()
    {
        // SAFETY: `cbSize` says the extensible part follows.
        unsafe { wave.cast::<WAVEFORMATEXTENSIBLE>().read_unaligned() }.dwChannelMask
    } else {
        standard_mask(head.nChannels)
    }
}

/// The extensible wave format of `format` with the speaker mask `mask`.
pub(super) fn wave_format(format: Format, mask: u32) -> WAVEFORMATEXTENSIBLE {
    let size = bytes(format.sample) as u16;
    let bits = size * 8;
    let valid = match format.sample {
        SampleFormat::I24In32 | SampleFormat::I24 => 24,
        _ => bits,
    };
    let block_align = format.channels * size;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: format.channels,
            nSamplesPerSec: format.rate,
            nAvgBytesPerSec: format.rate * u32::from(block_align),
            nBlockAlign: block_align,
            wBitsPerSample: bits,
            cbSize: (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 {
            wValidBitsPerSample: valid,
        },
        dwChannelMask: mask,
        SubFormat: if format.sample == SampleFormat::F32 {
            KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
        } else {
            KSDATAFORMAT_SUBTYPE_PCM
        },
    }
}

const I16_SCALE: f64 = 32_768.0;
const I24_SCALE: f64 = 8_388_608.0;
const I32_SCALE: f64 = 2_147_483_648.0;

/// Appends the device's little-endian samples `data` to `out` as `f32` in [-1, 1).
pub(super) fn read(sample: SampleFormat, data: &[u8], out: &mut Vec<f32>) {
    let chunks = data.chunks_exact(bytes(sample));
    let word = |b: &[u8]| f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    match sample {
        SampleFormat::F32 => out.extend(chunks.map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
        // The low byte of i24in32 is padding; reading the whole word gives the same value.
        SampleFormat::I32 | SampleFormat::I24In32 => out.extend(chunks.map(|b| (word(b) / I32_SCALE) as f32)),
        SampleFormat::I24 => {
            out.extend(chunks.map(|b| (f64::from(i32::from_le_bytes([0, b[0], b[1], b[2]])) / I32_SCALE) as f32));
        }
        SampleFormat::I16 => {
            out.extend(chunks.map(|b| (f64::from(i16::from_le_bytes([b[0], b[1]])) / I16_SCALE) as f32));
        }
    }
}

/// Writes `samples` into the device's little-endian `data`, clamped to the format's range; `data`
/// holds `samples.len()` samples.
pub(super) fn write(sample: SampleFormat, samples: &[f32], data: &mut [u8]) {
    let scaled = |value: f32, scale: f64| (f64::from(value) * scale).round().clamp(-scale, scale - 1.0);
    for (&value, b) in samples.iter().zip(data.chunks_exact_mut(bytes(sample))) {
        match sample {
            SampleFormat::F32 => b.copy_from_slice(&value.to_le_bytes()),
            SampleFormat::I32 => b.copy_from_slice(&(scaled(value, I32_SCALE) as i32).to_le_bytes()),
            SampleFormat::I24In32 => b.copy_from_slice(&((scaled(value, I24_SCALE) as i32) << 8).to_le_bytes()),
            SampleFormat::I24 => b.copy_from_slice(&(scaled(value, I24_SCALE) as i32).to_le_bytes()[..3]),
            SampleFormat::I16 => b.copy_from_slice(&(scaled(value, I16_SCALE) as i16).to_le_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_round_trip_in_every_format() {
        // Multiples of 2^-15 in [-1, 1): exact in every format.
        let values: Vec<f32> = [-32768, -12345, -1, 0, 1, 4096, 16384, 32767]
            .iter()
            .map(|&k| k as f32 / 32768.0)
            .collect();
        for sample in EXCLUSIVE_SAMPLES {
            let mut data = vec![0u8; values.len() * bytes(sample)];
            write(sample, &values, &mut data);
            let mut back = Vec::new();
            read(sample, &data, &mut back);
            assert_eq!(back, values, "{sample}");
        }
    }

    #[test]
    fn integer_writes_clamp_out_of_range_samples() {
        let loud = [1.5f32, -2.0, 1.0];
        for sample in EXCLUSIVE_SAMPLES.into_iter().filter(|&s| s != SampleFormat::F32) {
            let mut data = vec![0u8; loud.len() * bytes(sample)];
            write(sample, &loud, &mut data);
            let mut back = Vec::new();
            read(sample, &data, &mut back);
            assert!((0.999..=1.0).contains(&back[0]), "{sample}: {}", back[0]);
            assert_eq!(back[1], -1.0, "{sample}");
            assert_eq!(back[2], back[0], "{sample}");
        }
    }

    #[test]
    fn integer_layouts_match_the_device() {
        let mut data = [0u8; 4];
        write(SampleFormat::I24In32, &[0.5], &mut data);
        assert_eq!(data, [0x00, 0x00, 0x00, 0x40]);
        let mut data = [0u8; 3];
        write(SampleFormat::I24, &[-0.5], &mut data);
        assert_eq!(data, [0x00, 0x00, 0xc0]);
        let mut data = [0u8; 2];
        write(SampleFormat::I16, &[0.25], &mut data);
        assert_eq!(data, [0x00, 0x20]);
    }

    #[test]
    fn built_formats_read_back() {
        for sample in EXCLUSIVE_SAMPLES {
            let format = Format {
                rate: 44100,
                channels: 6,
                sample,
            };
            let wave = wave_format(format, standard_mask(6));
            // SAFETY: a whole extensible format, its cbSize set.
            let (back, mask) = unsafe { (format_of((&raw const wave).cast()), mask_of((&raw const wave).cast())) };
            assert_eq!(back, Some(format));
            assert_eq!(mask, 0x3f);
        }
        let plain = wave(WAVE_FORMAT_PCM, 1, 16000, 16);
        // SAFETY: a plain format with cbSize 0.
        assert_eq!(unsafe { mask_of(&raw const plain) }, 0x4);
    }

    fn wave(tag: u32, channels: u16, rate: u32, bits: u16) -> WAVEFORMATEX {
        WAVEFORMATEX {
            wFormatTag: tag as u16,
            nChannels: channels,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * u32::from(channels) * u32::from(bits / 8),
            nBlockAlign: channels * bits / 8,
            wBitsPerSample: bits,
            cbSize: 0,
        }
    }

    fn extensible(channels: u16, rate: u32, bits: u16, valid: u16, sub: windows::core::GUID) -> WAVEFORMATEXTENSIBLE {
        let mut head = wave(WAVE_FORMAT_EXTENSIBLE, channels, rate, bits);
        head.cbSize = (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16;
        WAVEFORMATEXTENSIBLE {
            Format: head,
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: valid,
            },
            dwChannelMask: 3,
            SubFormat: sub,
        }
    }

    fn read_wave(wave: &WAVEFORMATEXTENSIBLE) -> Option<String> {
        // SAFETY: a whole extensible format, its cbSize set.
        unsafe { format_of((&raw const *wave).cast()) }.map(|f| f.to_string())
    }

    #[test]
    fn reads_the_mix_format_and_device_formats() {
        // The shared mix format WASAPI hands out.
        let mix = extensible(2, 48000, 32, 32, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT);
        assert_eq!(read_wave(&mix).as_deref(), Some("48000/2/f32"));
        let i24in32 = extensible(8, 96000, 32, 24, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read_wave(&i24in32).as_deref(), Some("96000/8/i24in32"));
        let i24 = extensible(2, 44100, 24, 24, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read_wave(&i24).as_deref(), Some("44100/2/i24"));
        let i32 = extensible(2, 48000, 32, 0, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read_wave(&i32).as_deref(), Some("48000/2/i32"));
    }

    #[test]
    fn reads_plain_tags() {
        let plain = |w: WAVEFORMATEX| {
            // SAFETY: a plain format with cbSize 0.
            unsafe { format_of(&raw const w) }.map(|f| f.to_string())
        };
        assert_eq!(
            plain(wave(WAVE_FORMAT_PCM, 1, 16000, 16)).as_deref(),
            Some("16000/1/i16")
        );
        assert_eq!(
            plain(wave(WAVE_FORMAT_IEEE_FLOAT, 2, 48000, 32)).as_deref(),
            Some("48000/2/f32")
        );
    }

    #[test]
    fn refuses_what_the_app_cannot_handle() {
        // 64-bit float, 8-bit integer, a compressed tag, a short extensible block.
        let f64 = extensible(2, 48000, 64, 64, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT);
        assert_eq!(read_wave(&f64), None);
        let u8 = extensible(2, 48000, 8, 8, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read_wave(&u8), None);
        let mut short = extensible(2, 48000, 32, 32, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT);
        short.Format.cbSize = 0;
        assert_eq!(read_wave(&short), None);
        let compressed = wave(0x0092, 2, 48000, 16);
        // SAFETY: a plain format with cbSize 0.
        assert_eq!(unsafe { format_of(&raw const compressed) }, None);
    }
}
