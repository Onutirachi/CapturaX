//! Codificador AAC-LC do Media Foundation (já vem no Windows, sem dependência externa).

use std::mem::ManuallyDrop;
use std::sync::OnceLock;
use windows::core::Result;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

use crate::audio::{CHANNELS, SAMPLE_RATE};

pub const FRAME_SAMPLES: u32 = 1024;

/// AudioSpecificConfig do AAC-LC, 48 kHz, estéreo (vai no `esds` do MP4).
pub const AUDIO_SPECIFIC_CONFIG: [u8; 2] = [0x11, 0x90];

fn startup() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
            eprintln!("MFStartup failed: {error}");
        }
    });
}

pub struct AacEncoder {
    transform: IMFTransform,
    output_size: u32,
    samples_in: i64,
    pub bitrate: u32,
}

// SAFETY: criado e usado só na thread da sessão de mídia (MTA).
unsafe impl Send for AacEncoder {}

impl AacEncoder {
    /// `bitrate_kbps` é arredondado para os valores que o encoder aceita (96/128/160/192).
    pub fn new(bitrate_kbps: u32) -> Result<Self> {
        startup();
        let bytes_per_second = match bitrate_kbps {
            0..=111 => 12_000,
            112..=143 => 16_000,
            144..=175 => 20_000,
            _ => 24_000,
        };
        unsafe {
            let transform: IMFTransform = CoCreateInstance(&AACMFTEncoder, None, CLSCTX_INPROC_SERVER)?;

            let input = MFCreateMediaType()?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE)?;
            input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS as u32)?;
            input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, CHANNELS as u32 * 2)?;
            input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, SAMPLE_RATE * CHANNELS as u32 * 2)?;

            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            output.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE)?;
            output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS as u32)?;
            output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_second)?;
            output.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?; // AAC cru (sem ADTS), como o MP4 espera
            output.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;

            // A documentação do encoder não é clara sobre a ordem; tenta as duas.
            if transform.SetInputType(0, &input, 0).is_ok() {
                transform.SetOutputType(0, &output, 0)?;
            } else {
                transform.SetOutputType(0, &output, 0)?;
                transform.SetInputType(0, &input, 0)?;
            }
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            let output_size = transform.GetOutputStreamInfo(0)?.cbSize.max(8192);

            Ok(Self { transform, output_size, samples_in: 0, bitrate: bytes_per_second * 8 })
        }
    }

    /// Recebe PCM f32 intercalado e devolve os frames AAC prontos (cada um = 1024 amostras).
    pub fn encode(&mut self, samples: &[f32], frames_out: &mut Vec<Vec<u8>>) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let pcm: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
        let frames = (samples.len() / CHANNELS) as i64;
        unsafe {
            let buffer = MFCreateMemoryBuffer(pcm.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), data, pcm.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(pcm.len() as u32)?;

            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(self.samples_in * 10_000_000 / SAMPLE_RATE as i64)?;
            sample.SetSampleDuration(frames * 10_000_000 / SAMPLE_RATE as i64)?;
            self.samples_in += frames;

            if let Err(error) = self.transform.ProcessInput(0, &sample, 0) {
                if error.code() != MF_E_NOTACCEPTING {
                    return Err(error);
                }
                self.drain_output(frames_out)?;
                self.transform.ProcessInput(0, &sample, 0)?;
            }
        }
        self.drain_output(frames_out)
    }

    fn drain_output(&mut self, frames_out: &mut Vec<Vec<u8>>) -> Result<()> {
        loop {
            unsafe {
                let buffer = MFCreateMemoryBuffer(self.output_size)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buffer)?;
                let mut outputs = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(sample.clone())),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0;
                let result = self.transform.ProcessOutput(0, &mut outputs, &mut status);
                ManuallyDrop::drop(&mut outputs[0].pSample);
                ManuallyDrop::drop(&mut outputs[0].pEvents);
                match result {
                    Err(error) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(error) => return Err(error),
                    Ok(()) => {
                        let contiguous = sample.ConvertToContiguousBuffer()?;
                        let mut data = std::ptr::null_mut();
                        let mut length = 0u32;
                        contiguous.Lock(&mut data, None, Some(&mut length))?;
                        if length > 0 {
                            frames_out.push(std::slice::from_raw_parts(data, length as usize).to_vec());
                        }
                        contiguous.Unlock()?;
                    }
                }
            }
        }
    }
}
