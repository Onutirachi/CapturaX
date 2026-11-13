//! The system's wave formats (`WAVEFORMATEX`, `WAVEFORMATEXTENSIBLE`) in the platform's terms.
//! T018 adds the sample conversions of the streams here.

use windows::Win32::Media::Audio::{WAVE_FORMAT_PCM, WAVEFORMATEX, WAVEFORMATEXTENSIBLE};
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

#[cfg(test)]
mod tests {
    use windows::Win32::Media::Audio::WAVEFORMATEXTENSIBLE_0;

    use super::*;

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

    fn read(wave: &WAVEFORMATEXTENSIBLE) -> Option<String> {
        // SAFETY: a whole extensible format, its cbSize set.
        unsafe { format_of((&raw const *wave).cast()) }.map(|f| f.to_string())
    }

    #[test]
    fn reads_the_mix_format_and_device_formats() {
        // The shared mix format WASAPI hands out.
        let mix = extensible(2, 48000, 32, 32, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT);
        assert_eq!(read(&mix).as_deref(), Some("48000/2/f32"));
        let i24in32 = extensible(8, 96000, 32, 24, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read(&i24in32).as_deref(), Some("96000/8/i24in32"));
        let i24 = extensible(2, 44100, 24, 24, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read(&i24).as_deref(), Some("44100/2/i24"));
        let i32 = extensible(2, 48000, 32, 0, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read(&i32).as_deref(), Some("48000/2/i32"));
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
        assert_eq!(read(&f64), None);
        let u8 = extensible(2, 48000, 8, 8, KSDATAFORMAT_SUBTYPE_PCM);
        assert_eq!(read(&u8), None);
        let mut short = extensible(2, 48000, 32, 32, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT);
        short.Format.cbSize = 0;
        assert_eq!(read(&short), None);
        let compressed = wave(0x0092, 2, 48000, 16);
        // SAFETY: a plain format with cbSize 0.
        assert_eq!(unsafe { format_of(&raw const compressed) }, None);
    }
}
